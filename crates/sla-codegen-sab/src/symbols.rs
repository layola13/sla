//! C3 — symbol interning, binding registers, name mangling and
//! vtable const emission, mirroring `sab_codegen.zig` lines
//! 935–1102:
//! - `intern` / `internStable` (935–946);
//! - `bindingReg` (948–951);
//! - `loweredFuncSymbol` (953–963) and
//!   `hasConcreteFunctionSymbol` (965–973);
//! - `mangleMethodName` / `mangleTraitMethodName` /
//!   `concreteTypeName` / `dynTraitName` / `vtableName`
//!   (975–993), delegated to sla-lowering where it already owns
//!   the fact, or ported here where it does not;
//! - `appendTraitVTableEntries` / `emitTraitVTableDecl`
//!   (995–1050);
//! - `fnPtrVTableName` / `ensureFunctionPointerVTable` /
//!   `appendVTableConst` (1051–1102).
//!
//! Intern-order discipline: vtable emission interns names in
//! exactly the Zig order — trait entries intern each lowered
//! method name *while building* the const (before the vtable name
//! itself is interned after the append), while the fn-ptr path
//! interns the vtable name before the lowered function name. The
//! pool order is part of the emitted bytes, so these orders are
//! not interchangeable.

use std::collections::{HashMap, HashSet};

use sla_ast::{ImplDecl, TraitDecl, Type};
use sla_lowering::CheckInfo;

use crate::emit::ModuleBuilder;
use crate::error::{Error, Result};
use crate::ir::{BytesLiteral, ConstDecl, ConstValue, VTableSlot};

/// Zig `intern` (935–941): the shared symbol namespace. The trunk
/// already implements it (`ModuleBuilder::intern`); this is the
/// C3-named entry point.
pub fn intern(builder: &mut ModuleBuilder, name: &str) -> u32 {
    builder.intern(name)
}

/// Zig `internStable` (943–946): interns a *duplicated* copy of
/// the name so the caller's slice may be freed. Rust strings are
/// owned end-to-end, so stability is structural and this is the
/// same operation as [`intern`] — the distinction exists in Zig
/// only because of arena/slice lifetimes.
pub fn intern_stable(builder: &mut ModuleBuilder, name: &str) -> u32 {
    builder.intern(name)
}

/// Zig `bindingReg` (948–951): a plain binding interns its name;
/// a repeated-`let` binding (C2 `multi_let_bindings` membership,
/// passed in as `is_multi_let` so the caller can resolve the set
/// borrow before touching the builder) gets a fresh tmp register
/// per binding site. Zig interns `newTmp()`'s name; the trunk's
/// `new_tmp` interns as it allocates — same ids.
pub fn binding_reg(builder: &mut ModuleBuilder, name: &str, is_multi_let: bool) -> u32 {
    if is_multi_let {
        builder.new_tmp()
    } else {
        builder.intern(name)
    }
}

/// Zig `loweredFuncSymbol` (953–963) with the checker's function
/// facts supplied explicitly (the shape callers without a
/// `CheckInfo` at hand — e.g. trait vtable building — use):
/// `main` keeps its name; a checker-known extern or no-mangle
/// function keeps its name; anything else is `sla__<name>`.
pub fn lowered_func_symbol_with(
    name: &str,
    checker_func_is_extern_or_no_mangle: Option<bool>,
    checker_is_extern_func: bool,
) -> String {
    if sla_lowering::is_main_name(name) {
        return name.to_string();
    }
    if checker_func_is_extern_or_no_mangle == Some(true) {
        return name.to_string();
    }
    if checker_is_extern_func || name.starts_with("sa_") {
        return name.to_string();
    }
    format!("sla__{name}")
}

/// Zig `loweredFuncSymbol` (953–963) over a [`CheckInfo`]: the
/// `tc.funcs` lookup and the `tc.extern_funcs` membership are the
/// trait's `func` / `is_extern_func` facts.
pub fn lowered_func_symbol(name: &str, check: &dyn CheckInfo) -> String {
    let from_checker = check.func(name).map(|f| f.is_extern || f.no_mangle);
    lowered_func_symbol_with(name, from_checker, check.is_extern_func(name))
}

/// Zig `hasConcreteFunctionSymbol` (965–973): a decl-only
/// prototype is suppressed when a concrete definition of the same
/// name exists in the same program (otherwise the symbol would be
/// both imported and defined).
pub fn has_concrete_function_symbol(decls: &[sla_ast::Node], decl_only_name: &str) -> bool {
    decls.iter().any(|candidate| match candidate {
        sla_ast::Node::FuncDecl(f) => !f.is_decl_only && f.name == decl_only_name,
        _ => false,
    })
}

/// Zig `mangleMethodName` (975–977) = lowering_rules 5757:
/// `{ty}_{method}`.
pub fn mangle_method_name(ty_name: &str, method_name: &str) -> String {
    sla_lowering::mangle_method_name(ty_name, method_name)
}

/// Zig `mangleTraitMethodName` (979–981) = lowering_rules 5761:
/// `{ty}__{trait}_{method}`.
pub fn mangle_trait_method_name(ty_name: &str, trait_name: &str, method_name: &str) -> String {
    sla_lowering::mangle_trait_method_name(ty_name, trait_name, method_name)
}

/// Zig `concreteTypeName` (983–985) = lowering_rules 5765: peel
/// borrow/pointer layers, then the user-defined type's name.
pub fn concrete_type_name(ty: &Type) -> Option<&str> {
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::UserDefined(ud) => Some(&ud.name),
        _ => None,
    }
}

/// Zig `dynTraitName` (987–989) = lowering_rules 5781: the trait
/// name of a `__dyn_<Trait>` user-defined type, if it is one.
pub fn dyn_trait_name(ty: &Type) -> Option<&str> {
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::UserDefined(ud) => ud.name.strip_prefix("__dyn_"),
        _ => None,
    }
}

/// Zig `vtableName` (991–993) = lowering_rules 5788:
/// `VT_{type}_{trait}`.
pub fn vtable_name(trait_name: &str, type_name: &str) -> String {
    format!("VT_{type_name}_{trait_name}")
}

/// Zig `fnPtrVTableName` (1051–1053): `SLA_FNPTR_VT_{func}`.
pub fn fn_ptr_vtable_name(func_name: &str) -> String {
    format!("SLA_FNPTR_VT_{func_name}")
}

/// Facts `appendTraitVTableEntries` reads from the checker: the
/// trait declarations (Zig `tc.traits`) and whether a mangled impl
/// method name resolves to a decl-only function (Zig
/// `tc.funcs.get(mangled).is_decl_only`).
pub struct TraitFacts<'a> {
    pub traits: &'a HashMap<String, TraitDecl>,
    /// `is_decl_only(mangled_name)`: true when the checker knows a
    /// decl-only function under that (mangled) name.
    pub decl_only_funcs: &'a HashSet<String>,
}

/// Zig `appendTraitVTableEntries` (995–1024): supertrait entries
/// first (recursively, in declaration order), then this trait's
/// methods — skipping a slot whose impl method is decl-only
/// (import expansion prunes unreachable imported trait impl
/// methods to decl-only stubs; a slot pointing at one would trap
/// the verifier). Entries are appended to `slots` and to the
/// `literal` text (`{method} = @{lowered}`, ", "-joined across the
/// whole recursion), and each lowered name is interned as it is
/// appended. An unknown trait is Zig's
/// `Error.UnsupportedSabDirectFeature`.
pub fn append_trait_vtable_entries(
    builder: &mut ModuleBuilder,
    facts: &TraitFacts,
    check: &dyn CheckInfo,
    trait_name: &str,
    type_name: &str,
    slots: &mut Vec<VTableSlot>,
    literal: &mut String,
) -> Result<()> {
    let trait_decl = facts.traits.get(trait_name).ok_or_else(|| {
        Error::UnsupportedSabDirectFeature(format!(
            "trait vtable entries for unknown trait `{trait_name}` [group C3 symbols; Zig sab_codegen.zig:1002]"
        ))
    })?;
    for supertrait in &trait_decl.supertraits {
        append_trait_vtable_entries(builder, facts, check, supertrait, type_name, slots, literal)?;
    }
    for method in &trait_decl.methods {
        let mangled = mangle_trait_method_name(type_name, trait_name, &method.name);
        // Zig: `tc.funcs.get(mangled)` with `is_decl_only` skips
        // the slot. Both fact sources are consulted: the explicit
        // set (built by the driver from AST impl methods) and the
        // checker's own function table under the mangled name.
        let decl_only = facts.decl_only_funcs.contains(&mangled)
            || check.func(&mangled).map(|f| f.is_decl_only).unwrap_or(false);
        if decl_only {
            continue;
        }
        if !slots.is_empty() {
            literal.push_str(", ");
        }
        let lowered = lowered_func_symbol(&mangled, check);
        literal.push_str(&format!("{} = @{}", method.name, lowered));
        slots.push(VTableSlot {
            name: method.name.clone(),
            func_name: lowered.clone(),
        });
        builder.intern(&lowered);
    }
    Ok(())
}

/// Zig `emitTraitVTableDecl` (1026–1050): build the
/// `vtable { ... }` const decl for one trait impl and append it.
/// A trait-less impl is a no-op (returns `None`); an impl whose
/// target type has no concrete name is Zig's
/// `Error.UnsupportedSabDirectFeature`. The vtable name is
/// interned after the append, as in Zig.
pub fn emit_trait_vtable_decl(
    builder: &mut ModuleBuilder,
    facts: &TraitFacts,
    check: &dyn CheckInfo,
    decl: &ImplDecl,
) -> Result<Option<String>> {
    let Some(trait_name) = &decl.trait_name else {
        return Ok(None);
    };
    let type_name = concrete_type_name(&decl.target_ty).ok_or_else(|| {
        Error::UnsupportedSabDirectFeature(format!(
            "trait vtable for impl with non-concrete target type [group C3 symbols; Zig sab_codegen.zig:1028]"
        ))
    })?;
    let vt_name = vtable_name(trait_name, type_name);
    let mut slots: Vec<VTableSlot> = Vec::new();
    let mut literal = String::from("vtable { ");
    append_trait_vtable_entries(builder, facts, check, trait_name, type_name, &mut slots, &mut literal)?;
    literal.push_str(" }");
    builder.append_const_decl(ConstDecl {
        source_line: 0,
        expanded_line: 0,
        upstream_loc: None,
        name: vt_name.clone(),
        literal_text: literal,
        value: ConstValue::VTable(slots),
    });
    builder.intern(&vt_name);
    Ok(Some(vt_name))
}

/// Zig `appendVTableConst` (1082–1102): a one-slot
/// `vtable { call = @<worker> }` const decl; the vtable name is
/// interned first, then the worker name, both after the append.
pub fn append_vtable_const(
    builder: &mut ModuleBuilder,
    vtable_name: &str,
    worker_name: &str,
) {
    builder.append_const_decl(ConstDecl {
        source_line: 0,
        expanded_line: 0,
        upstream_loc: None,
        name: vtable_name.to_string(),
        literal_text: format!("vtable {{ call = @{worker_name} }}"),
        value: ConstValue::VTable(vec![VTableSlot {
            name: "call".to_string(),
            func_name: worker_name.to_string(),
        }]),
    });
    builder.intern(vtable_name);
    builder.intern(worker_name);
}

/// Zig `ensureFunctionPointerVTable` (1055–1080): emit the fn-ptr
/// vtable for `func_name` once (tracked in `emitted`, Zig's
/// `fn_ptr_vtables`); the slot points at the *lowered* function
/// symbol. Returns the vtable name either way. Note the Zig
/// intern order on this path: vtable name first, then the lowered
/// name (both after the append) — unlike the trait path.
pub fn ensure_function_pointer_vtable(
    builder: &mut ModuleBuilder,
    emitted: &mut HashSet<String>,
    check: &dyn CheckInfo,
    func_name: &str,
) -> String {
    let vt_name = fn_ptr_vtable_name(func_name);
    if emitted.contains(&vt_name) {
        return vt_name;
    }
    let lowered = lowered_func_symbol(func_name, check);
    builder.append_const_decl(ConstDecl {
        source_line: 0,
        expanded_line: 0,
        upstream_loc: None,
        name: vt_name.clone(),
        literal_text: format!("vtable {{ call = @{lowered} }}"),
        value: ConstValue::VTable(vec![VTableSlot {
            name: "call".to_string(),
            func_name: lowered.clone(),
        }]),
    });
    emitted.insert(vt_name.clone());
    builder.intern(&vt_name);
    builder.intern(&lowered);
    vt_name
}

/// Hex/utf8 byte width helper kept for C1 callers that need the
/// raw-bytes const shape vtable emission shares (the const decl
/// record itself). Not a Zig symbol; a typed constructor for the
/// `ConstValue::Hex` decls C1 appends.
pub fn hex_const_decl(name: &str, literal_text: String, bytes: Vec<u8>) -> ConstDecl {
    ConstDecl {
        source_line: 0,
        expanded_line: 0,
        upstream_loc: None,
        name: name.to_string(),
        literal_text,
        value: ConstValue::Hex(BytesLiteral {
            bytes,
            repeat: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{FuncDecl, Param, Primitive, TraitMethod, UserDefined};
    use sla_lowering::MapCheckInfo;

    fn ud(name: &str) -> Type {
        Type::UserDefined(UserDefined {
            name: name.to_string(),
            generics: Vec::new(),
        })
    }

    fn trait_decl(name: &str, supers: Vec<&str>, methods: Vec<&str>) -> TraitDecl {
        TraitDecl {
            name: name.to_string(),
            supertraits: supers.into_iter().map(|s| s.to_string()).collect(),
            methods: methods
                .into_iter()
                .map(|m| TraitMethod {
                    name: m.to_string(),
                    params: Vec::<Param>::new(),
                    ret_ty: Box::new(Type::Primitive(Primitive::VoidType)),
                })
                .collect(),
        }
    }

    fn func_decl(name: &str, is_extern: bool, no_mangle: bool, decl_only: bool) -> FuncDecl {
        FuncDecl {
            name: name.to_string(),
            is_pub: false,
            is_extern,
            abi: None,
            no_mangle,
            is_decl_only: decl_only,
            generics: Vec::new(),
            params: Vec::new(),
            ret_ty: Box::new(Type::Primitive(Primitive::VoidType)),
            body: Vec::new(),
            is_inline: false,
            is_async: false,
            operator: None,
        }
    }

    #[test]
    fn intern_and_binding_reg() {
        let mut b = ModuleBuilder::new();
        assert_eq!(intern(&mut b, "x"), 0);
        assert_eq!(intern_stable(&mut b, "x"), 0);
        assert_eq!(intern(&mut b, "y"), 1);
        // Plain binding: its own name id.
        assert_eq!(binding_reg(&mut b, "x", false), 0);
        // Multi-let binding: fresh tmp registers per site.
        let r1 = binding_reg(&mut b, "m", true);
        let r2 = binding_reg(&mut b, "m", true);
        assert_ne!(r1, r2);
        assert_eq!(b.symbol_name(r1), "tmp_0");
        assert_eq!(b.symbol_name(r2), "tmp_1");
    }

    #[test]
    fn lowered_symbol_rules() {
        let mut check = MapCheckInfo::new();
        check.add_func(func_decl("ext_f", true, false, true));
        check.add_func(func_decl("plain_f", false, true, true));
        check.add_extern_func("ffi_f");
        assert_eq!(lowered_func_symbol("main", &check), "main");
        assert_eq!(lowered_func_symbol("ext_f", &check), "ext_f");
        assert_eq!(lowered_func_symbol("plain_f", &check), "plain_f");
        assert_eq!(lowered_func_symbol("ffi_f", &check), "ffi_f");
        assert_eq!(lowered_func_symbol("sa_raw", &check), "sa_raw");
        assert_eq!(lowered_func_symbol("user_f", &check), "sla__user_f");
    }

    #[test]
    fn concrete_and_dyn_names() {
        assert_eq!(concrete_type_name(&ud("Sprite")), Some("Sprite"));
        let borrowed = Type::Borrow(Box::new(ud("Sprite")));
        assert_eq!(concrete_type_name(&borrowed), Some("Sprite"));
        assert_eq!(
            concrete_type_name(&Type::Primitive(Primitive::I32)),
            None
        );
        assert_eq!(dyn_trait_name(&ud("__dyn_Draw")), Some("Draw"));
        assert_eq!(dyn_trait_name(&ud("Sprite")), None);
    }

    #[test]
    fn mangles_and_vtable_names() {
        assert_eq!(mangle_method_name("Sprite", "draw"), "Sprite_draw");
        assert_eq!(
            mangle_trait_method_name("Sprite", "Draw", "draw"),
            "Sprite__Draw_draw"
        );
        assert_eq!(vtable_name("Draw", "Sprite"), "VT_Sprite_Draw");
        assert_eq!(fn_ptr_vtable_name("go"), "SLA_FNPTR_VT_go");
    }

    #[test]
    fn has_concrete_symbol() {
        let decls = vec![
            sla_ast::Node::FuncDecl(func_decl("f", true, false, true)),
            sla_ast::Node::FuncDecl(func_decl("g", false, false, false)),
        ];
        assert!(has_concrete_function_symbol(&decls, "g"));
        assert!(!has_concrete_function_symbol(&decls, "f"));
        assert!(!has_concrete_function_symbol(&decls, "h"));
    }

    fn draw_facts() -> (HashMap<String, TraitDecl>, HashSet<String>) {
        let mut traits = HashMap::new();
        traits.insert(
            "Named".to_string(),
            trait_decl("Named", vec![], vec!["name"]),
        );
        traits.insert(
            "Draw".to_string(),
            trait_decl("Draw", vec!["Named"], vec!["draw", "hide"]),
        );
        (traits, HashSet::new())
    }

    #[test]
    fn trait_vtable_entries_supertrait_first_and_decl_only_skip() {
        let (traits, mut decl_only) = draw_facts();
        // The Draw::hide impl is a decl-only stub: its slot is
        // skipped (Zig's verifier-trap guard).
        decl_only.insert("Sprite__Draw_hide".to_string());
        let facts = TraitFacts {
            traits: &traits,
            decl_only_funcs: &decl_only,
        };
        let check = MapCheckInfo::new();
        let mut b = ModuleBuilder::new();
        let impl_decl = ImplDecl {
            trait_name: Some("Draw".to_string()),
            target_ty: Box::new(ud("Sprite")),
            methods: Vec::new(),
        };
        let vt = emit_trait_vtable_decl(&mut b, &facts, &check, &impl_decl)
            .unwrap()
            .expect("trait impl must emit a vtable");
        assert_eq!(vt, "VT_Sprite_Draw");
        let module = b.finish();
        assert_eq!(module.const_decls.len(), 1);
        let decl = &module.const_decls[0];
        assert_eq!(
            decl.literal_text,
            "vtable { name = @sla__Sprite__Named_name, draw = @sla__Sprite__Draw_draw }"
        );
        match &decl.value {
            ConstValue::VTable(slots) => {
                assert_eq!(slots.len(), 2);
                assert_eq!(slots[0].name, "name");
                assert_eq!(slots[0].func_name, "sla__Sprite__Named_name");
                assert_eq!(slots[1].name, "draw");
            }
            other => panic!("expected vtable const, got {other:?}"),
        }
        // Intern order: lowered entry names before the vtable name.
        let pos = |s: &str| module.symbols.iter().position(|x| x == s).unwrap();
        assert!(pos("sla__Sprite__Named_name") < pos("VT_Sprite_Draw"));
        assert!(pos("sla__Sprite__Draw_draw") < pos("VT_Sprite_Draw"));
    }

    #[test]
    fn trait_vtable_unknown_trait_is_loud() {
        let (traits, decl_only) = draw_facts();
        let facts = TraitFacts {
            traits: &traits,
            decl_only_funcs: &decl_only,
        };
        let check = MapCheckInfo::new();
        let mut b = ModuleBuilder::new();
        let impl_decl = ImplDecl {
            trait_name: Some("Missing".to_string()),
            target_ty: Box::new(ud("Sprite")),
            methods: Vec::new(),
        };
        let err = emit_trait_vtable_decl(&mut b, &facts, &check, &impl_decl).unwrap_err();
        assert!(matches!(err, Error::UnsupportedSabDirectFeature(_)));
    }

    #[test]
    fn fn_ptr_vtable_emitted_once() {
        let check = MapCheckInfo::new();
        let mut b = ModuleBuilder::new();
        let mut emitted = HashSet::new();
        let vt1 = ensure_function_pointer_vtable(&mut b, &mut emitted, &check, "worker");
        let vt2 = ensure_function_pointer_vtable(&mut b, &mut emitted, &check, "worker");
        assert_eq!(vt1, "SLA_FNPTR_VT_worker");
        assert_eq!(vt1, vt2);
        let module = b.finish();
        assert_eq!(module.const_decls.len(), 1);
        assert_eq!(
            module.const_decls[0].literal_text,
            "vtable { call = @sla__worker }"
        );
        // Intern order on this path: vtable name before lowered.
        let pos = |s: &str| module.symbols.iter().position(|x| x == s).unwrap();
        assert!(pos("SLA_FNPTR_VT_worker") < pos("sla__worker"));
    }

    #[test]
    fn append_vtable_const_shape() {
        let mut b = ModuleBuilder::new();
        append_vtable_const(&mut b, "VT_X", "worker_fn");
        let module = b.finish();
        assert_eq!(module.const_decls.len(), 1);
        match &module.const_decls[0].value {
            ConstValue::VTable(slots) => {
                assert_eq!(slots.len(), 1);
                assert_eq!(slots[0].name, "call");
                assert_eq!(slots[0].func_name, "worker_fn");
            }
            other => panic!("expected vtable const, got {other:?}"),
        }
    }
}

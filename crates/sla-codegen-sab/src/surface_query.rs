//! D4 — std surface rule queries and the dep-ensuring surface,
//! mirroring `sab_codegen.zig` lines 1884–1997 plus the
//! dep-ensurer family it feeds (`ensureRuleDeps` 2099–2101,
//! `ensureStdDeps` 6108–6136, `ensurePrintlnDeps` 13217–13229,
//! `ensureDebugFormatDeps` 13203–13215): `findStdSurfaceRule`,
//! `stdSurfaceRuleHasArg`, `StdSurfaceValues`,
//! `stdSurfaceArgText`, `stdSurfaceArgMustBeLiteral`,
//! `primTypeMacroSuffix`, `stdSurfaceMacroName`,
//! `emitStdSurfaceRule`, `ensureRuleDeps`.
//!
//! The dep bookkeeping is Zig's exactly: a dep is *missing* when
//! it is neither in `included_imports` nor already pending, and
//! missing deps are queued in request order. One registered
//! structural divergence rides on top (SAB-D26): Zig's
//! `ensureStdDeps` appends immediately when called outside a
//! function body — which is the preload case — through
//! `appendStdDepsNow` (the F-group decoded-module machinery,
//! not yet ported). This port always takes Zig's
//! in-function-body branch (queue into `pending_std_deps`);
//! the F group will replace the queue's consumer
//! (`flushPendingStdDeps`) when it lands, at which point the
//! preload path appends for real. Nothing is silently dropped:
//! every requested dep is observable in the pending queue.
//!
//! `emitStdSurfaceRule`'s emission tail is likewise F-group
//! (`emitStdMacroFragmentWithLiteralArgs`); it sits behind the
//! [`SurfaceEmitter`] trait, whose default implementation fails
//! loudly with F attribution. The rule-side work — dep ensuring,
//! the BTreeSet direct-call special case, argument rendering,
//! `{elem_ty}` macro-name expansion — is fully ported here.

use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::ir::PrimType;
use crate::types::{PendingStdDep, StdSurfaceArgKind, StdSurfaceRule, StdSurfaceRuleKind};

/// Zig `findStdSurfaceRule` (lines 1946–1958): first rule in
/// table order matching kind, type name, and member name
/// (`None` member matches only member-less rules).
pub fn find_std_surface_rule<'r>(
    rules: &'r [StdSurfaceRule],
    kind: StdSurfaceRuleKind,
    type_name: &str,
    member_name: Option<&str>,
) -> Option<&'r StdSurfaceRule> {
    rules.iter().find(|rule| {
        if rule.kind != kind || rule.type_name != type_name {
            return false;
        }
        match member_name {
            Some(member) => rule.member_name.as_deref() == Some(member),
            None => rule.member_name.is_none(),
        }
    })
}

/// Zig `stdSurfaceRuleHasArg` (lines 1960–1965).
pub fn std_surface_rule_has_arg(rule: &StdSurfaceRule, arg: StdSurfaceArgKind) -> bool {
    rule.args.contains(&arg)
}

/// Zig `StdSurfaceValues` (lines 1967–1975): the register/size
/// facts a rule instantiation renders its arguments from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdSurfaceValues {
    pub out: Option<u32>,
    pub ok: Option<u32>,
    pub receiver: Option<u32>,
    pub value: Option<u32>,
    pub index: Option<u32>,
    pub elem_size: usize,
    pub elem_ty: Option<PrimType>,
}

impl Default for StdSurfaceValues {
    /// Zig's struct defaults: every register absent, `elem_size`
    /// one word (8), no element type.
    fn default() -> Self {
        StdSurfaceValues {
            out: None,
            ok: None,
            receiver: None,
            value: None,
            index: None,
            elem_size: 8,
            elem_ty: None,
        }
    }
}

fn unsupported_query(what: &str) -> Error {
    Error::UnsupportedSabDirectFeature(format!(
        "{what} [group D4 surface_query; Zig sab_codegen.zig:1884-1997]"
    ))
}

/// Zig `sig.primTypeName` (sci signature.zig:140–157).
pub fn prim_type_name(ty: PrimType) -> &'static str {
    match ty {
        PrimType::Void => "void",
        PrimType::I1 => "i1",
        PrimType::I8 => "i8",
        PrimType::I16 => "i16",
        PrimType::I32 => "i32",
        PrimType::I64 => "i64",
        PrimType::U8 => "u8",
        PrimType::U16 => "u16",
        PrimType::U32 => "u32",
        PrimType::U64 => "u64",
        PrimType::F32 => "f32",
        PrimType::F64 => "f64",
        PrimType::Ptr => "ptr",
        PrimType::BlobHandle => "blob_handle",
        PrimType::V128 => "v128",
    }
}

/// Zig `stdSurfaceArgText` (lines 1977–1987): register arguments
/// render as their symbol names; a missing register for a
/// requested kind is Zig's `UnsupportedSabDirectFeature`.
pub fn std_surface_arg_text(
    kind: StdSurfaceArgKind,
    values: &StdSurfaceValues,
    symbols: &[String],
) -> Result<String> {
    let reg_text = |reg: Option<u32>| -> Result<String> {
        let reg = reg.ok_or_else(|| unsupported_query("std surface argument register missing"))?;
        symbols
            .get(reg as usize)
            .cloned()
            .ok_or_else(|| unsupported_query("std surface argument register not interned"))
    };
    Ok(match kind {
        StdSurfaceArgKind::Out => reg_text(values.out)?,
        StdSurfaceArgKind::Ok => reg_text(values.ok)?,
        StdSurfaceArgKind::Receiver => reg_text(values.receiver)?,
        StdSurfaceArgKind::Value => reg_text(values.value)?,
        StdSurfaceArgKind::Index => reg_text(values.index)?,
        StdSurfaceArgKind::ElemSize => format!("{}", values.elem_size),
        StdSurfaceArgKind::ElemTy => {
            let ty = values
                .elem_ty
                .ok_or_else(|| unsupported_query("std surface elem_ty missing"))?;
            prim_type_name(ty).to_string()
        }
    })
}

/// Zig `stdSurfaceArgMustBeLiteral`: only `elem_ty` renders as a
/// literal macro argument.
pub fn std_surface_arg_must_be_literal(kind: StdSurfaceArgKind) -> bool {
    matches!(kind, StdSurfaceArgKind::ElemTy)
}

/// Zig `primTypeMacroSuffix` (lines 1993–2010): the uppercase
/// suffix substituted for `{elem_ty}`; void/blob/v128 are loud.
pub fn prim_type_macro_suffix(ty: PrimType) -> Result<&'static str> {
    Ok(match ty {
        PrimType::I1 => "I1",
        PrimType::I8 => "I8",
        PrimType::I16 => "I16",
        PrimType::I32 => "I32",
        PrimType::I64 => "I64",
        PrimType::U8 => "U8",
        PrimType::U16 => "U16",
        PrimType::U32 => "U32",
        PrimType::U64 => "U64",
        PrimType::F32 => "F32",
        PrimType::F64 => "F64",
        PrimType::Ptr => "PTR",
        _ => {
            return Err(unsupported_query(&format!(
                "no macro suffix for prim type {}",
                prim_type_name(ty)
            )))
        }
    })
}

/// Zig `stdSurfaceMacroName`: a macro name without the
/// `{elem_ty}` marker is used verbatim; with it, every occurrence
/// is replaced by the element type's suffix.
pub fn std_surface_macro_name(rule: &StdSurfaceRule, values: &StdSurfaceValues) -> Result<String> {
    const MARKER: &str = "{elem_ty}";
    if !rule.macro_name.contains(MARKER) {
        return Ok(rule.macro_name.clone());
    }
    let elem_ty = values
        .elem_ty
        .ok_or_else(|| unsupported_query("macro name needs elem_ty but none supplied"))?;
    Ok(rule
        .macro_name
        .replace(MARKER, prim_type_macro_suffix(elem_ty)?))
}

/// The dep-ensuring surface (Zig `ensureStdDeps` family). The
/// println/debug dep lists are Zig's verbatim.
pub trait StdDepEnsurer {
    fn ensure_std_deps(&mut self, import_path: &str, deps: &[String]) -> Result<()>;

    /// Zig `ensureRuleDeps`: a rule's deps ride on its import path.
    fn ensure_rule_deps(&mut self, rule: &StdSurfaceRule) -> Result<()> {
        self.ensure_std_deps(&rule.import_path, &rule.deps)
    }

    /// Zig `ensurePrintlnDeps` (lines 13217–13229).
    fn ensure_println_deps(&mut self) -> Result<()> {
        self.ensure_std_deps(
            "sa_std/io/print.sai",
            &["sa_print_bytes".to_string()],
        )?;
        self.ensure_std_deps(
            "sa_std/fmt.sai",
            &[
                "sa_fmt_i64".to_string(),
                "sa_fmt_u64".to_string(),
                "sa_fmt_f64".to_string(),
                "sa_fmt_bool".to_string(),
                "sa_fmt_buffer_data".to_string(),
                "sa_fmt_buffer_len".to_string(),
                "sa_fmt_buffer_free".to_string(),
            ],
        )
    }

    /// Zig `ensureDebugFormatDeps` (lines 13203–13215).
    fn ensure_debug_format_deps(&mut self) -> Result<()> {
        self.ensure_std_deps(
            "sa_std/string_format.sa",
            &[
                "sa_vec_with_capacity".to_string(),
                "sa_vec_reserve".to_string(),
                "sa_vec_free".to_string(),
                "sa_mem_copy".to_string(),
                "sa_mem_set".to_string(),
                "sa_fmt_i64_into".to_string(),
                "sa_fmt_u64_into".to_string(),
                "sa_fmt_f64_into".to_string(),
                "sa_fmt_bool_into".to_string(),
            ],
        )
    }
}

/// The pending-queue dep ensurer over the program state's
/// `included_imports` / `pending_std_deps` (SAB-D26: Zig's
/// queueing branch taken unconditionally until the F group's
/// append machinery lands).
pub struct PendingStdDeps<'a> {
    pub included_imports: &'a mut HashSet<String>,
    pub pending_std_deps: &'a mut Vec<PendingStdDep>,
}

impl<'a> PendingStdDeps<'a> {
    pub fn new(
        included_imports: &'a mut HashSet<String>,
        pending_std_deps: &'a mut Vec<PendingStdDep>,
    ) -> Self {
        PendingStdDeps {
            included_imports,
            pending_std_deps,
        }
    }

    /// Zig `pendingStdDepContains`.
    fn pending_contains(&self, dep: &str) -> bool {
        self.pending_std_deps.iter().any(|p| p.dep == dep)
    }
}

impl StdDepEnsurer for PendingStdDeps<'_> {
    /// Zig `ensureStdDeps` bookkeeping (lines 6108–6136): empty
    /// dep lists are free; already-included or already-pending
    /// deps are not re-queued; the rest queue in request order.
    fn ensure_std_deps(&mut self, import_path: &str, deps: &[String]) -> Result<()> {
        if deps.is_empty() {
            return Ok(());
        }
        let missing: Vec<&String> = deps
            .iter()
            .filter(|dep| {
                !self.included_imports.contains(*dep) && !self.pending_contains(dep)
            })
            .collect();
        for dep in missing {
            self.pending_std_deps.push(PendingStdDep {
                import_path: import_path.to_string(),
                dep: dep.clone(),
            });
        }
        Ok(())
    }
}

/// The F-group emission tail of `emitStdSurfaceRule`, behind a
/// trait so D4's rule logic is testable before F lands.
pub trait SurfaceEmitter {
    /// Zig `emitCallBody` (the BTreeSet special case's tail).
    fn emit_call_body(&mut self, out_reg: Option<u32>, body: String) -> Result<()>;
    /// Zig `emitStdMacroFragmentWithLiteralArgs` (F group).
    fn emit_std_macro_fragment_with_literal_args(
        &mut self,
        import_path: &str,
        macro_name: &str,
        args: &[String],
        literal_args: &[bool],
    ) -> Result<()>;
}

/// The un-wired emitter: every emission attempt is a loud
/// F-group stop, never a silent skip.
pub struct LoudSurfaceEmitter;

impl SurfaceEmitter for LoudSurfaceEmitter {
    fn emit_call_body(&mut self, _out_reg: Option<u32>, _body: String) -> Result<()> {
        Err(Error::UnsupportedSabDirectFeature(
            "std surface call-body emission (decoded std modules) [group F fragments; Zig sab_codegen.zig:6108-6160]".to_string(),
        ))
    }

    fn emit_std_macro_fragment_with_literal_args(
        &mut self,
        _import_path: &str,
        _macro_name: &str,
        _args: &[String],
        _literal_args: &[bool],
    ) -> Result<()> {
        Err(Error::UnsupportedSabDirectFeature(
            "std macro fragment emission [group F fragments; Zig sab_codegen.zig:2066-2097]".to_string(),
        ))
    }
}

/// Zig `emitStdSurfaceRule` (lines 2066–2097): deps first, then
/// the BTreeSet direct-call special case, else the generic
/// fragment instantiation with rendered arguments.
pub fn emit_std_surface_rule(
    emitter: &mut dyn SurfaceEmitter,
    ensurer: &mut dyn StdDepEnsurer,
    rule: &StdSurfaceRule,
    values: &StdSurfaceValues,
    symbols: &[String],
) -> Result<()> {
    ensurer.ensure_rule_deps(rule)?;
    if rule.macro_name == "BTREE_SET_INSERT" || rule.macro_name == "BTREE_SET_CONTAINS" {
        let out_reg = values
            .out
            .ok_or_else(|| unsupported_query("BTreeSet surface rule without out register"))?;
        let receiver = std_surface_arg_text(StdSurfaceArgKind::Receiver, values, symbols)?;
        let value = std_surface_arg_text(StdSurfaceArgKind::Value, values, symbols)?;
        let callee = if rule.macro_name == "BTREE_SET_INSERT" {
            "@sa_btree_set_insert"
        } else {
            "@sa_btree_set_contains"
        };
        return emitter.emit_call_body(Some(out_reg), format!("{callee}(&{receiver}, &{value})"));
    }
    let mut args = Vec::with_capacity(rule.args.len());
    let mut literal_args = Vec::with_capacity(rule.args.len());
    for arg_kind in &rule.args {
        args.push(std_surface_arg_text(*arg_kind, values, symbols)?);
        literal_args.push(std_surface_arg_must_be_literal(*arg_kind));
    }
    let macro_name = std_surface_macro_name(rule, values)?;
    emitter.emit_std_macro_fragment_with_literal_args(
        &rule.import_path,
        &macro_name,
        &args,
        &literal_args,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface_load::parse_std_surface_rules;
    use crate::types::StdSurfaceRuleKind as Kind;

    const META: &str = "\
associated Vec new sa_std/vec.sa VEC_NEW out deps=sa_vec_new
method Vec push sa_std/vec.sa VEC_PUSH receiver,value,elem_size deps=sa_vec_push,sa_mem_copy
index Vec sa_std/vec.sa VEC_GET_TYPED_{elem_ty} out,receiver,index,elem_size
method BTreeSet insert sa_std/btree_set.sa BTREE_SET_INSERT out,receiver,value deps=sa_btree_set_insert
";

    fn rules() -> Vec<StdSurfaceRule> {
        parse_std_surface_rules(META).unwrap()
    }

    #[test]
    fn find_rule_matches_kind_type_member() {
        let rules = rules();
        let push = find_std_surface_rule(&rules, Kind::Method, "Vec", Some("push")).unwrap();
        assert_eq!(push.macro_name, "VEC_PUSH");
        assert!(std_surface_rule_has_arg(push, StdSurfaceArgKind::ElemSize));
        assert!(!std_surface_rule_has_arg(push, StdSurfaceArgKind::Out));
        // Member-less lookup only matches the index rule.
        let index = find_std_surface_rule(&rules, Kind::Index, "Vec", None).unwrap();
        assert_eq!(index.macro_name, "VEC_GET_TYPED_{elem_ty}");
        assert!(find_std_surface_rule(&rules, Kind::Method, "Vec", None).is_none());
        assert!(find_std_surface_rule(&rules, Kind::Method, "Vec", Some("pop")).is_none());
    }

    #[test]
    fn macro_name_expands_elem_ty() {
        let rules = rules();
        let index = find_std_surface_rule(&rules, Kind::Index, "Vec", None).unwrap();
        let mut values = StdSurfaceValues::default();
        assert!(std_surface_macro_name(index, &values).is_err());
        values.elem_ty = Some(PrimType::I64);
        assert_eq!(
            std_surface_macro_name(index, &values).unwrap(),
            "VEC_GET_TYPED_I64"
        );
        assert_eq!(prim_type_macro_suffix(PrimType::Ptr).unwrap(), "PTR");
        assert!(prim_type_macro_suffix(PrimType::Void).is_err());
    }

    #[test]
    fn arg_text_renders_registers_and_sizes() {
        let symbols: Vec<String> = vec!["out_r".into(), "recv_r".into()];
        let values = StdSurfaceValues {
            out: Some(0),
            receiver: Some(1),
            elem_size: 4,
            elem_ty: Some(PrimType::U8),
            ..Default::default()
        };
        assert_eq!(
            std_surface_arg_text(StdSurfaceArgKind::Out, &values, &symbols).unwrap(),
            "out_r"
        );
        assert_eq!(
            std_surface_arg_text(StdSurfaceArgKind::ElemSize, &values, &symbols).unwrap(),
            "4"
        );
        assert_eq!(
            std_surface_arg_text(StdSurfaceArgKind::ElemTy, &values, &symbols).unwrap(),
            "u8"
        );
        assert!(std_surface_arg_text(StdSurfaceArgKind::Value, &values, &symbols).is_err());
    }

    #[test]
    fn pending_deps_dedup_against_included_and_pending() {
        let mut included: HashSet<String> = ["sa_vec_new".to_string()].into_iter().collect();
        let mut pending: Vec<PendingStdDep> = Vec::new();
        {
            let mut ensurer = PendingStdDeps::new(&mut included, &mut pending);
            ensurer.ensure_println_deps().unwrap();
            // Second call queues nothing new.
            ensurer.ensure_println_deps().unwrap();
            let rule = find_std_surface_rule(&rules(), Kind::Associated, "Vec", Some("new"))
                .unwrap()
                .clone();
            ensurer.ensure_rule_deps(&rule).unwrap();
        }
        let deps: Vec<&str> = pending.iter().map(|p| p.dep.as_str()).collect();
        assert_eq!(
            deps,
            vec![
                "sa_print_bytes",
                "sa_fmt_i64",
                "sa_fmt_u64",
                "sa_fmt_f64",
                "sa_fmt_bool",
                "sa_fmt_buffer_data",
                "sa_fmt_buffer_len",
                "sa_fmt_buffer_free"
            ]
        );
        assert_eq!(pending[0].import_path, "sa_std/io/print.sai");
        // sa_vec_new was already included: never queued.
        assert!(!deps.contains(&"sa_vec_new"));
    }

    struct RecordingEmitter {
        calls: Vec<String>,
    }

    impl SurfaceEmitter for RecordingEmitter {
        fn emit_call_body(&mut self, _out: Option<u32>, body: String) -> Result<()> {
            self.calls.push(body);
            Ok(())
        }
        fn emit_std_macro_fragment_with_literal_args(
            &mut self,
            import_path: &str,
            macro_name: &str,
            args: &[String],
            literal_args: &[bool],
        ) -> Result<()> {
            self.calls.push(format!(
                "{import_path} {macro_name} {args:?} {literal_args:?}"
            ));
            Ok(())
        }
    }

    #[test]
    fn emit_rule_btreeset_special_and_generic() {
        let rules = rules();
        let mut included = HashSet::new();
        let mut pending = Vec::new();
        let mut ensurer = PendingStdDeps::new(&mut included, &mut pending);
        let symbols: Vec<String> = vec!["o".into(), "r".into(), "v".into()];
        let values = StdSurfaceValues {
            out: Some(0),
            receiver: Some(1),
            value: Some(2),
            index: Some(2),
            elem_ty: Some(PrimType::I32),
            ..Default::default()
        };
        let mut emitter = RecordingEmitter { calls: vec![] };
        let insert =
            find_std_surface_rule(&rules, Kind::Method, "BTreeSet", Some("insert")).unwrap();
        emit_std_surface_rule(&mut emitter, &mut ensurer, insert, &values, &symbols).unwrap();
        assert_eq!(emitter.calls, vec!["@sa_btree_set_insert(&r, &v)".to_string()]);
        let index = find_std_surface_rule(&rules, Kind::Index, "Vec", None).unwrap();
        emit_std_surface_rule(&mut emitter, &mut ensurer, index, &values, &symbols).unwrap();
        assert_eq!(
            emitter.calls[1],
            "sa_std/vec.sa VEC_GET_TYPED_I32 [\"o\", \"r\", \"v\", \"8\"] [false, false, false, false]"
        );
        // The loud emitter refuses the generic tail with F attribution.
        let mut loud = LoudSurfaceEmitter;
        let err = emit_std_surface_rule(&mut loud, &mut ensurer, index, &values, &symbols)
            .unwrap_err();
        assert!(format!("{err}").contains("group F"));
    }
}

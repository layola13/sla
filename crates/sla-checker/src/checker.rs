//! The `Checker` state and the ownership ledger operations.
//!
//! Mirrors `TypeChecker` (type_checker.zig:121-183) minus the parts
//! Rust makes unrepresentable or this slice excludes:
//!   * Zig stores diagnostics in `last_error_buf: [1024]u8` and passes
//!     kinds through error unions; here diagnostics are return values
//!     (diag.rs, D-C2).
//!   * Zig smuggles the current function's return type through the
//!     symbol table as a fake binding `return_ty_sentinel`, read back
//!     with `.?` unwraps (e.g. type_checker.zig:5252). Here it is the
//!     plain field `current_ret_ty` (D-C5).
//!   * Zig threads loop context twice: fields `current_loop_scope` /
//!     `current_loop_body_terminates` AND a `checkStmt` parameter.
//!     Here it is one explicit stack, `loop_ctx` (D-C4).
//!   * Zig side tables are keyed by `*const ast.Node`; here by
//!     `node_key` (address inside the borrowed tree).

use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use crate::scope::{Binding, BindingId, ScopeStack};
use crate::types_util;
use sla_ast::decls::{FuncDecl, MacroDecl, Param};
use sla_ast::node::Node;
use sla_ast::types::Type;
use std::collections::HashMap;

/// One callable signature (user function or imported signature).
#[derive(Debug, Clone)]
pub struct FuncSig {
    pub params: Vec<ParamSig>,
    pub ret_ty: Type,
    /// Zig `FuncDecl.is_pub`. Only consulted by the `using`
    /// static-extension resolver, which registers exclusively the
    /// target module's pub functions (venue type_checker.zig
    /// `resolveUsingMethodSymbolForType`). Imported signatures
    /// default to `true` (the resolver never consults that table).
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct ParamSig {
    pub name: String,
    pub ty: Type,
    pub is_borrow: bool,
    pub is_move: bool,
}

impl From<&Param> for ParamSig {
    fn from(p: &Param) -> ParamSig {
        ParamSig {
            name: p.name.clone(),
            ty: (*p.ty).clone(),
            is_borrow: p.is_borrow,
            is_move: p.is_move,
        }
    }
}

/// An FFI signature registered from `.sai` contracts (Zig fills
/// `extern_funcs` only from contract files, type_checker.zig:2106).
/// Argument TYPES are not modeled: the Zig call arm checks argument
/// expressions but ignores their types ("bypass signature checks for
/// FFI pointers", type_checker.zig:5072) — only arity, the per-param
/// move contract, and the ABI return string matter.
#[derive(Debug, Clone)]
pub struct ExternSig {
    /// Per-parameter move contract (`^` in the contract): the argument
    /// binding is consumed by the call.
    pub move_params: Vec<bool>,
    /// ABI return type spelling, mapped by [`extern_ret_type`].
    pub ret_abi: String,
}

/// Registered struct shape for field access and Copy derivation.
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub fields: Vec<(String, Type)>,
    pub derives: Vec<String>,
    pub is_union: bool,
    pub is_opaque: bool,
}

impl StructInfo {
    pub fn field_ty(&self, name: &str) -> Option<&Type> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t)
    }

    pub fn has_copy_derive(&self) -> bool {
        self.derives.iter().any(|d| d.eq_ignore_ascii_case("copy"))
    }
}

/// One enum variant's shape, for pattern binding and enum literals.
/// The legacy `Checker::enums` table keeps only variant NAMES (its
/// public shape predates this line); this table carries the fields.
#[derive(Debug, Clone)]
pub struct EnumVariantInfo {
    pub name: String,
    pub fields: Vec<(String, Type)>,
}

/// A registered trait: supertraits plus its declared methods, for
/// the impl-contract check (Zig `traits` + `TraitDecl`).
#[derive(Debug, Clone)]
pub struct TraitInfo {
    pub supertraits: Vec<String>,
    pub methods: Vec<TraitMethodInfo>,
}

/// One trait method signature (Zig `ast.TraitMethod`).
#[derive(Debug, Clone)]
pub struct TraitMethodInfo {
    pub name: String,
    pub params: Vec<ParamSig>,
    pub ret_ty: Type,
}

/// An imported (SA-text) macro registration, mirroring Zig
/// `ImportedMacro` (type_checker.zig:25). The checker consults only
/// `arity` and `leading_outputs`; the masks and callee list are
/// carried for parity with the registration API.
#[derive(Debug, Clone)]
pub struct ImportedMacroInfo {
    pub arity: usize,
    pub leading_outputs: usize,
    pub import_path: Option<String>,
    pub borrowed_arg_mask: u64,
    pub address_slot_arg_mask: u64,
    pub direct_callees: Vec<String>,
}

/// Loop bookkeeping for the LoopConditionalConsume gate. Zig keeps
/// exactly one such pair in loose fields (overwritten by nested
/// loops); the stack top here plays that role.
#[derive(Debug, Clone, Copy)]
pub struct LoopCtx {
    /// Frame index of the loop body's scope.
    pub frame_index: usize,
    /// Whether the loop body syntactically terminates (return / break
    /// / continue / panic tail), which lifts the consumption ban.
    pub body_terminates: bool,
}

/// The checker. One instance checks one program (Zig `TypeChecker`
/// is likewise created per compilation).
#[derive(Debug)]
pub struct Checker {
    pub structs: HashMap<String, StructInfo>,
    pub enums: HashMap<String, Vec<String>>,
    /// Variant shapes by enum name (see [`EnumVariantInfo`]).
    pub enum_variants: HashMap<String, Vec<EnumVariantInfo>>,
    /// Registered traits by name (Zig `traits`).
    pub traits: HashMap<String, TraitInfo>,
    /// `"trait|type"` pairs for every registered trait impl
    /// (Zig `trait_impls`).
    pub trait_impls: std::collections::HashSet<String>,
    /// Operator overload lists keyed `"{Type}|{op_name}"`, values
    /// are mangled symbols `"{Type}__{op_name}"` (Zig `overloads`).
    pub overloads: HashMap<String, Vec<String>>,
    /// Generic parameter names per function (Zig `FuncDecl.generics`;
    /// the checker stores but never substitutes them — D-C11).
    pub func_generics: HashMap<String, Vec<String>>,
    /// Imported SA-text macros (Zig `imported_macros`).
    pub imported_macros: HashMap<String, ImportedMacroInfo>,
    /// Single-`Ty` component aliases, for name resolution.
    pub aliases: HashMap<String, Type>,
    pub funcs: HashMap<String, FuncSig>,
    pub extern_funcs: HashMap<String, ExternSig>,
    pub imported_funcs: HashMap<String, FuncSig>,
    pub macros: HashMap<String, MacroDecl>,
    pub scopes: ScopeStack,
    /// `expr_types` side table (Zig: AutoHashMap(*const Node, *Type)).
    pub expr_types: HashMap<usize, Type>,
    /// `cleanups` side table: bindings to release at a node's exit.
    pub cleanups: HashMap<usize, Vec<String>>,
    /// `phi_cleanups`: bindings consumed on only some branch paths,
    /// released on the paths where they stayed active.
    pub phi_cleanups: HashMap<usize, Vec<String>>,
    /// `await_cleanups` (Zig type_checker.zig:150): bindings to
    /// release if an await suspends, keyed by the await node. Kept
    /// apart from `cleanups`, whose await-adjacent entries mean
    /// something else (the try arm shares `cleanups` in Zig too).
    pub await_cleanups: HashMap<usize, Vec<String>>,
    /// `macro_call_try_cleanups` (Zig type_checker.zig:151):
    /// bindings live across a macro call whose body may `?`-propagate,
    /// keyed by the call node (Zig keys by the `*const CallExpr`
    /// sub-object; the containing node address is the Rust analogue).
    pub macro_call_try_cleanups: HashMap<usize, Vec<String>>,
    /// `dyn_box_coercions` / `dyn_rc_coercions`: value node -> trait
    /// name for `Box<dyn Trait>` / `Rc<dyn Trait>` let coercions.
    pub dyn_box_coercions: HashMap<usize, String>,
    pub dyn_rc_coercions: HashMap<usize, String>,
    pub loop_ctx: Vec<LoopCtx>,
    /// `monitor_calls` side table (Zig type_checker.zig:298):
    /// call nodes the `monitor` arm accepted, keyed like the
    /// other side tables. Zig's codegen consults it to lower
    /// monitor calls; here it is recorded for parity.
    pub monitor_calls: std::collections::HashSet<usize>,
    /// Zig `unsafe_depth` (type_checker.zig:341): incremented
    /// around unsafe blocks; the `ptr::read_volatile` arms gate
    /// on it (D-C21's missing counter, now carried).
    pub unsafe_depth: usize,
    /// Zig `current_fn_is_variadic_def`: whether the function
    /// under check is an exported extern "C" variadic
    /// definition, the `va_next_*` gate. No sla-ast surface can
    /// set it yet (FuncDecl carries no variadic marker and the
    /// type surface has no VaList primitive — D-C24), so it
    /// stays false for parsed programs, exactly the verdict Zig
    /// gives every non-variadic context.
    pub current_fn_is_variadic: bool,
    /// Frame index of the current function's root scope.
    pub fn_root_depth: usize,
    /// The current function's return type (Zig: the
    /// `return_ty_sentinel` symbol; here a field — D-C5).
    pub current_ret_ty: Option<Type>,
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    pub fn new() -> Checker {
        Checker {
            structs: HashMap::new(),
            enums: HashMap::new(),
            enum_variants: HashMap::new(),
            traits: HashMap::new(),
            trait_impls: std::collections::HashSet::new(),
            overloads: HashMap::new(),
            func_generics: HashMap::new(),
            imported_macros: HashMap::new(),
            aliases: HashMap::new(),
            funcs: HashMap::new(),
            extern_funcs: HashMap::new(),
            imported_funcs: HashMap::new(),
            macros: HashMap::new(),
            scopes: ScopeStack::new(),
            expr_types: HashMap::new(),
            cleanups: HashMap::new(),
            phi_cleanups: HashMap::new(),
            await_cleanups: HashMap::new(),
            macro_call_try_cleanups: HashMap::new(),
            dyn_box_coercions: HashMap::new(),
            dyn_rc_coercions: HashMap::new(),
            loop_ctx: Vec::new(),
            monitor_calls: std::collections::HashSet::new(),
            unsafe_depth: 0,
            current_fn_is_variadic: false,
            fn_root_depth: 0,
            current_ret_ty: None,
        }
    }

    // -- registration (mirrors the register* API of TypeChecker) -----

    pub fn register_struct(&mut self, name: &str, info: StructInfo) {
        self.structs.insert(name.to_string(), info);
    }

    pub fn register_enum(&mut self, name: &str, variants: Vec<String>) {
        self.enums.insert(name.to_string(), variants);
    }

    pub fn register_alias(&mut self, name: &str, ty: Type) {
        self.aliases.insert(name.to_string(), ty);
    }

    pub fn register_func(&mut self, func: &FuncDecl) {
        self.register_func_as(&func.name.clone(), func);
    }

    /// Register a function under an explicit (possibly mangled)
    /// symbol name — impl methods and operator overloads live under
    /// keys other than their declaration name in Zig (`funcs` is
    /// keyed by symbol, not by source name).
    pub fn register_func_as(&mut self, symbol: &str, func: &FuncDecl) {
        let sig = FuncSig {
            params: func.params.iter().map(ParamSig::from).collect(),
            ret_ty: (*func.ret_ty).clone(),
            is_pub: func.is_pub,
        };
        self.funcs.insert(symbol.to_string(), sig);
        if !func.generics.is_empty() {
            self.func_generics
                .insert(symbol.to_string(), func.generics.clone());
        }
    }

    /// Record an enum's variant shapes (companion to
    /// [`Checker::register_enum`], which keeps the name list).
    pub fn register_enum_variants(
        &mut self,
        name: &str,
        variants: Vec<EnumVariantInfo>,
    ) {
        self.enum_variants.insert(name.to_string(), variants);
    }

    pub fn register_trait(&mut self, name: &str, info: TraitInfo) {
        self.traits.insert(name.to_string(), info);
    }

    /// Mirrors Zig `registerImportedMacro` (type_checker.zig:2119).
    pub fn register_imported_macro(&mut self, name: &str, info: ImportedMacroInfo) {
        self.imported_macros.insert(name.to_string(), info);
    }

    /// Mirrors Zig `registerImportedFunctionSignature`.
    pub fn register_imported_signature(&mut self, name: &str, sig: FuncSig) {
        self.imported_funcs.insert(name.to_string(), sig);
    }

    /// Mirrors the `.sai` contract registration that fills
    /// `extern_funcs` in Zig (type_checker.zig:2106-2111).
    pub fn register_extern_func(&mut self, name: &str, sig: ExternSig) {
        self.extern_funcs.insert(name.to_string(), sig);
    }

    pub fn register_macro(&mut self, decl: &MacroDecl) {
        self.macros.insert(decl.name.clone(), decl.clone());
    }

    // -- symbol table -------------------------------------------------

    /// Mirrors Zig `defineSymbol`: same-frame redeclaration is an
    /// error; shadowing an outer frame is legal.
    pub fn define_symbol(
        &mut self,
        name: &str,
        ty: Type,
        is_const: bool,
    ) -> CheckResult<BindingId> {
        self.define_symbol_with_state(name, ty, is_const, ValueState::Active, None)
    }

    pub fn define_symbol_with_state(
        &mut self,
        name: &str,
        ty: Type,
        is_const: bool,
        state: ValueState,
        borrow_source: Option<String>,
    ) -> CheckResult<BindingId> {
        self.scopes
            .define(name, ty, is_const, state, borrow_source)
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::Redeclaration,
                    format!("Redeclaration: `{name}` is already defined in this scope"),
                )
            })
    }

    pub fn lookup_ty(&self, name: &str) -> Option<Type> {
        self.scopes.lookup(name).map(|b| b.ty.clone())
    }

    /// Mirrors Zig `currentUsingModules`: every `using` path in
    /// the scope chain, innermost frame first, each frame's list in
    /// append order. Paths are stored pre-normalized (see
    /// [`crate::types_util::normalize_using_path`]).
    pub fn current_using_modules(&self) -> Vec<String> {
        let mut mods = Vec::new();
        for frame in self.scopes.frames.iter().rev() {
            mods.extend(frame.using_modules.iter().cloned());
        }
        mods
    }

    pub fn lookup_state(&self, name: &str) -> Option<ValueState> {
        self.scopes.lookup(name).map(|b| b.state)
    }

    // -- the ownership ledger -----------------------------------------

    /// Mirrors Zig `consumeBinding` (type_checker.zig:254), INCLUDING
    /// the LoopConditionalConsume gate (the DEFECT-A family): inside a
    /// loop body that does not terminate, consuming a binding declared
    /// OUTSIDE the loop is rejected, because the loop may iterate and
    /// re-reach the consumption point.
    ///
    /// Locality test: Zig walks the scope-parent chain comparing
    /// POINTERS against `current_loop_scope` (`bindingIsLocalToLoop`,
    /// line 112). Here frame indices compare directly — same question,
    /// no chain walk.
    pub fn consume_binding(&mut self, name: &str, context: &str) -> CheckResult<()> {
        let frame = self.scopes.frame_of(name).ok_or_else(|| {
            Diagnostic::new(
                ErrorKind::UndefinedVariable,
                format!("UndefinedVariable: `{name}`"),
            )
        })?;
        if let Some(loop_ctx) = self.loop_ctx.last() {
            if !loop_ctx.body_terminates && frame < loop_ctx.frame_index {
                return Err(Diagnostic::marked(
                    ErrorKind::CompileError,
                    "LoopConditionalConsume",
                    format!(
                        "LoopConditionalConsume: binding `{name}` cannot be consumed by {context} inside the loop body"
                    ),
                ));
            }
        }
        let binding = self
            .scopes
            .lookup_mut(name)
            .expect("frame_of and lookup disagree");
        if binding.state == ValueState::Consumed {
            return Err(Diagnostic::new(
                ErrorKind::UseAfterMove,
                format!("UseAfterMove: `{name}` was already consumed"),
            ));
        }
        binding.state = ValueState::Consumed;
        Ok(())
    }

    /// Set a binding's state directly (assignment re-initialization,
    /// branch merge fixups). Locality/gate checks do NOT apply, as in
    /// Zig where merges write `sym.state` directly.
    pub fn set_state(&mut self, name: &str, state: ValueState) {
        if let Some(b) = self.scopes.lookup_mut(name) {
            b.state = state;
        }
    }

    pub fn set_state_by_id(&mut self, id: BindingId, state: ValueState) {
        for frame in &mut self.scopes.frames {
            for b in &mut frame.bindings {
                if b.id == id {
                    b.state = state;
                    return;
                }
            }
        }
    }

    // -- type facts ----------------------------------------------------

    /// Mirrors Zig `typeIsCopy` (type_checker.zig:955) on top of
    /// lowering_rules' derive base: primitives are Copy except
    /// `raw_ptr` / `void_type`; fn_ptr stays NON-Copy in the checker
    /// (deliberate, per the Zig comment); user structs need the Copy
    /// derive gate plus all-Copy fields; tuples recurse; everything
    /// else (arrays, pointers, borrows, closures, futures, unknown
    /// names) is not Copy.
    pub fn type_is_copy(&self, ty: &Type) -> bool {
        match ty {
            Type::Primitive(p) => {
                !matches!(p, sla_ast::types::Primitive::RawPtr | sla_ast::types::Primitive::VoidType)
            }
            Type::Tuple(t) => t.elems.iter().all(|e| self.type_is_copy(e)),
            Type::UserDefined(u) => match self.structs.get(&u.name) {
                Some(info) => {
                    info.has_copy_derive()
                        && !info.is_opaque
                        && !info.is_union
                        && info
                            .fields
                            .iter()
                            .all(|(_, fty)| self.type_is_copy(fty))
                }
                None => false,
            },
            _ => false,
        }
    }

    // -- cleanup collection helpers ----------------------------------

    /// Mirrors Zig `borrowSourceAliasedByOtherActive` (issue013):
    /// some OTHER active binding anywhere in the scope chain borrows
    /// from `name`, so `name` itself must not be released.
    pub fn borrow_source_aliased_by_other_active(&self, name: &str) -> bool {
        for frame in &self.scopes.frames {
            for b in &frame.bindings {
                if b.name == name || b.state != ValueState::Active {
                    continue;
                }
                if b.borrow_source.as_deref() == Some(name) {
                    return true;
                }
            }
        }
        false
    }

    /// Mirrors Zig `functionExitBindingEscapes`: the candidate
    /// escapes through `result_expr` when the result's root binding
    /// IS the candidate, or when following the root's borrow-source
    /// chain reaches the candidate. (Zig bounds the walk by the
    /// total symbol count; the chain here is finite by construction
    /// and the same bound is applied.)
    pub fn binding_escapes_through(&self, result_expr: &Node, candidate: &str) -> bool {
        let root = match result_expr {
            Node::Identifier(name) => name.as_str(),
            Node::MoveExpr(m) => match &*m.expr {
                Node::Identifier(name) => name.as_str(),
                _ => return false,
            },
            _ => return false,
        };
        if root == candidate {
            return true;
        }
        let total: usize = self
            .scopes
            .frames
            .iter()
            .map(|f| f.bindings.len())
            .sum();
        let mut current = root.to_string();
        let mut remaining = total + 1;
        while remaining > 0 {
            remaining -= 1;
            let Some(binding) = self.scopes.lookup(&current) else {
                return false;
            };
            let Some(source) = binding.borrow_source.clone() else {
                return false;
            };
            if source == candidate {
                return true;
            }
            current = source;
        }
        false
    }

    /// Names of all currently active, non-internal bindings,
    /// innermost frame first (Zig walks the scope chain from the
    /// current scope outward; within a frame Zig's order is hash
    /// order, here declaration order — D-C6). Shared by the await,
    /// try, and macro-try cleanup collectors, which differ only in
    /// their exclusion predicate.
    pub fn collect_active_names(&self, exclude: impl Fn(&Binding) -> bool) -> Vec<String> {
        let mut names = Vec::new();
        for frame in self.scopes.frames.iter().rev() {
            for b in &frame.bindings {
                if b.state != ValueState::Active
                    || types_util::is_internal_symbol(&b.name)
                    || exclude(b)
                {
                    continue;
                }
                names.push(b.name.clone());
            }
        }
        names
    }

    /// Mirrors Zig `recordAwaitCleanup` (type_checker.zig:336):
    /// every active non-internal binding, EXCEPT bindings that
    /// escape through the awaited future expression and borrow-like
    /// bindings aliased by another active binding (issue013). A
    /// non-empty list lands in `await_cleanups` keyed by the await
    /// node; Zig stores nothing for an empty list.
    pub fn record_await_cleanup(&mut self, await_node: &Node, future_expr: &Node) {
        let names = self.collect_active_names(|b| {
            self.binding_escapes_through(future_expr, &b.name)
                || (types_util::is_borrow_like_type(&b.ty)
                    && self.borrow_source_aliased_by_other_active(&b.name))
        });
        if !names.is_empty() {
            self.await_cleanups
                .insert(types_util::node_key_of(await_node), names);
        }
    }

    /// Mirrors the try arm's `collectTryCleanup`
    /// (type_checker.zig:1662): every active non-internal binding
    /// except those the try expression itself uses. Stored into the
    /// shared `cleanups` table — Zig has no separate try table.
    pub fn record_try_cleanup(&mut self, try_node: &Node, operand: &Node) {
        let names = self.collect_active_names(|b| {
            crate::rules::expr_uses_identifier_value(operand, &b.name)
        });
        self.cleanups
            .insert(types_util::node_key_of(try_node), names);
    }

    /// Resolve a user-defined name through the alias table.
    pub fn resolve_alias<'a>(&'a self, ty: &'a Type) -> &'a Type {
        if let Type::UserDefined(u) = ty {
            if u.generics.is_empty() {
                if let Some(target) = self.aliases.get(&u.name) {
                    return target;
                }
            }
        }
        ty
    }
}

/// Map an `.sai` ABI return spelling to a type (Zig
/// `setTypeFromAbiReturn`). Unknown spellings degrade to `infer`,
/// which the Zig helper likewise leaves unset/wildcard in practice.
pub fn extern_ret_type(abi: &str) -> Type {
    use sla_ast::types::Primitive as P;
    match abi {
        "" | "void" => types_util::prim(P::VoidType),
        "ptr" | "*u8" | "*const u8" => types_util::prim(P::RawPtr),
        "bool" => types_util::prim(P::Boolean),
        "i8" => types_util::prim(P::I8),
        "i16" => types_util::prim(P::I16),
        "i32" => types_util::prim(P::I32),
        "i64" | "isize" => types_util::prim(P::I64),
        "u8" => types_util::prim(P::U8),
        "u16" => types_util::prim(P::U16),
        "u32" => types_util::prim(P::U32),
        "u64" | "usize" => types_util::prim(P::U64),
        "f32" => types_util::prim(P::F32),
        "f64" => types_util::prim(P::F64),
        _ => Type::Infer,
    }
}

/// Zig `makeBorrowType`: wrap a parameter type in a borrow unless it
/// already is one.
pub fn make_borrow_type(ty: &Type) -> Type {
    match ty {
        Type::Borrow(_) => ty.clone(),
        _ => Type::Borrow(Box::new(ty.clone())),
    }
}

/// Zig `isPanicBuiltinName` (lowering_rules.zig:4108).
pub fn is_panic_builtin_name(name: &str) -> bool {
    name == "panic" || name == "panic_msg"
}

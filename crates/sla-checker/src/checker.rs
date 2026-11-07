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
use crate::scope::{BindingId, ScopeStack};
use crate::types_util;
use sla_ast::decls::{FuncDecl, MacroDecl, Param};
use sla_ast::types::Type;
use std::collections::HashMap;

/// One callable signature (user function or imported signature).
#[derive(Debug, Clone)]
pub struct FuncSig {
    pub params: Vec<ParamSig>,
    pub ret_ty: Type,
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
    pub loop_ctx: Vec<LoopCtx>,
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
            aliases: HashMap::new(),
            funcs: HashMap::new(),
            extern_funcs: HashMap::new(),
            imported_funcs: HashMap::new(),
            macros: HashMap::new(),
            scopes: ScopeStack::new(),
            expr_types: HashMap::new(),
            cleanups: HashMap::new(),
            phi_cleanups: HashMap::new(),
            loop_ctx: Vec::new(),
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
        let sig = FuncSig {
            params: func.params.iter().map(ParamSig::from).collect(),
            ret_ty: (*func.ret_ty).clone(),
        };
        self.funcs.insert(func.name.clone(), sig);
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

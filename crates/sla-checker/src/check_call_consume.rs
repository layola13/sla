//! By-value call-argument consumption and the fallible-extern
//! contract (R-6 families F5, F7-call, F1), mirroring the Zig
//! checker's `checkCallArgsAgainstFunc` gates
//! (def-deepcopy staging type_checker.zig:4146-4157, 4424-4560)
//! and the lang-gap A_min rules (impl-lang-gap phase 1).
//!
//! Two gates run per by-value argument, in Zig's order:
//!   1. the preserved-copy gate — an argument the callee preserves
//!      (auto-clone discipline) whose type closure contains an
//!      uncloneable member is rejected with UncloneableCopy;
//!   2. the consumption gate — an argument that transfers ownership
//!      consumes its source binding, which is marked with the
//!      consuming call's name so a later read produces the
//!      REG-BR-6 UseAfterMove text (see check_expr.rs).

use crate::checker::{Checker, ExternSig, ParamSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use crate::types_util::{is_borrow_like_type, is_string_type, type_desc};
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

impl Checker {
    // -- consumption marks (Zig Symbol.consumed_by_call/_literal) --

    pub fn set_consumed_by_call(&mut self, name: &str, call_name: &str) {
        if let Some(b) = self.scopes.lookup_mut(name) {
            b.consumed_by_call = Some(call_name.to_string());
        }
    }

    pub fn set_consumed_by_literal(&mut self, name: &str, context: String) {
        if let Some(b) = self.scopes.lookup_mut(name) {
            b.consumed_by_literal = Some(context);
        }
    }

    /// Assignment reactivates a slot: Zig clears both consumed
    /// marks when a consumed binding is assigned again.
    pub fn clear_consumed_marks(&mut self, name: &str) {
        if let Some(b) = self.scopes.lookup_mut(name) {
            b.consumed_by_call = None;
            b.consumed_by_literal = None;
        }
    }

    // -- F5/F7: the two call-argument gates --------------------------

    /// Zig `callArgShallowCopyPreserved`: the argument is a plain
    /// identifier of a non-Copy, non-borrow-like struct type that
    /// qualifies as a shallow-copy call-arg value, passed to a
    /// plain (non-borrow, non-move) parameter — the callee receives
    /// a record copy and the source binding stays live.
    fn call_arg_shallow_copy_preserved(
        &self,
        param: &ParamSig,
        arg: &Node,
        arg_ty: &Type,
    ) -> bool {
        if param.is_borrow || param.is_move {
            return false;
        }
        if !matches!(arg, Node::Identifier(_)) {
            return false;
        }
        if !matches!(arg_ty, Type::UserDefined(_)) {
            return false;
        }
        if crate::check_leaves::is_copy_value(self, arg_ty)
            || is_borrow_like_type(arg_ty)
        {
            return false;
        }
        if !self.is_shallow_copy_call_arg(arg_ty, 0) {
            return false;
        }
        self.struct_decl_for_type(arg_ty).is_some()
    }

    /// Whether every payload field of the enum type is a copy
    /// value (Zig `enumPayloadIsCopy`): passing such an enum by
    /// value copies the payload instead of moving the binding.
    fn enum_payload_is_copy(&self, ty: &Type) -> bool {
        let Type::UserDefined(u) = ty else {
            return false;
        };
        let Some(variants) = self.enum_variants.get(&u.name) else {
            return false;
        };
        variants.iter().all(|v| {
            v.fields
                .iter()
                .all(|(_, fty)| crate::check_leaves::is_copy_value(self, fty))
        })
    }

    /// Run both gates for one already-type-checked argument of a
    /// user-function call (Zig `checkCallArgsAgainstFunc` order:
    /// the UncloneableCopy gate fires before consumption).
    pub fn check_call_arg_consumption(
        &mut self,
        call_name: &str,
        param: &ParamSig,
        arg: &Node,
        arg_ty: &Type,
    ) -> CheckResult<()> {
        let preserved = self.call_arg_shallow_copy_preserved(param, arg, arg_ty);
        if preserved {
            if let Some(bad) = self.uncloneable_type_name(arg_ty, 0) {
                return Err(Diagnostic::marked(
                    ErrorKind::CompileError,
                    "UncloneableCopy",
                    format!(
                        "UncloneableCopy: argument of type `{}` passed to `{call_name}` must be copied because the source stays live, but its closure contains {bad}, which has no clone - pass it by borrow, or keep the {bad} outside the copied value",
                        type_desc(arg_ty)
                    ),
                ));
            }
        }
        // consumeCallArgIfMoved.
        let Node::Identifier(arg_name) = arg else {
            return Ok(());
        };
        if matches!(arg_ty, Type::Infer) {
            return Ok(());
        }
        let transfers = !param.is_borrow
            && !param.is_move
            && !is_borrow_like_type(arg_ty)
            && !crate::check_leaves::is_copy_value(self, arg_ty);
        if !transfers {
            return Ok(());
        }
        // Vec / GrowVec arguments are cloned by the emitters.
        if matches!(arg_ty, Type::UserDefined(u) if u.name == "Vec" || u.name == "GrowVec") {
            return Ok(());
        }
        if preserved || self.enum_payload_is_copy(arg_ty) {
            return Ok(());
        }
        let eligible = self
            .scopes
            .lookup(arg_name)
            .map(|b| !b.is_const && b.state == ValueState::Active)
            .unwrap_or(false);
        if !eligible {
            return Ok(());
        }
        let arg_name = arg_name.clone();
        self.consume_binding(&arg_name, "call argument")?;
        self.set_consumed_by_call(&arg_name, call_name);
        Ok(())
    }

    /// The struct-literal field-fill consumption (Zig staging
    /// :7771): a fill from a live, non-const, non-borrowed binding
    /// of a handle-owning type consumes the binding (the loop
    /// guard inside `consume_binding` raises LoopConditionalConsume
    /// where the fill is loop-carried) and marks it with the
    /// literal context for the later UseAfterMove text.
    pub fn consume_struct_literal_field(
        &mut self,
        struct_name: &str,
        field_name: &str,
        value: &Node,
        field_ty: &Type,
    ) -> CheckResult<()> {
        let Node::Identifier(src) = value else {
            return Ok(());
        };
        if !self.literal_field_owns_handles(field_ty, 0) {
            return Ok(());
        }
        let eligible = self
            .scopes
            .lookup(src)
            .map(|b| {
                b.state == ValueState::Active && !b.is_const && b.borrow_source.is_none()
            })
            .unwrap_or(false);
        if !eligible {
            return Ok(());
        }
        let src = src.clone();
        self.consume_binding(&src, "struct literal field fill")?;
        self.set_consumed_by_literal(
            &src,
            format!(
                "struct literal field `{struct_name}.{field_name}` of type `{}`",
                type_desc(field_ty)
            ),
        );
        Ok(())
    }

    /// Zig `literalFieldOwnsHandles`: a struct-literal field fill
    /// from a binding of this type transfers ownership into the
    /// literal (strings, handle collections, Rc/Arc cells, and any
    /// type whose closure contains an uncloneable member, plus
    /// structs/enums/arrays/tuples containing such a field).
    pub fn literal_field_owns_handles(&self, ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        match ty {
            Type::UserDefined(u) => {
                if is_string_type(ty) {
                    return true;
                }
                if matches!(u.name.as_str(), "Vec" | "GrowVec" | "Box" | "Rc" | "Arc")
                    && !u.generics.is_empty()
                {
                    return true;
                }
                if self.uncloneable_type_name(ty, depth).is_some() {
                    return true;
                }
                if let Some(variants) = self.enum_variants.get(&u.name).cloned() {
                    return variants.iter().any(|v| {
                        v.fields
                            .iter()
                            .any(|(_, fty)| self.literal_field_owns_handles(fty, depth + 1))
                    });
                }
                if let Some(info) = self.structs.get(&u.name).cloned() {
                    return info
                        .fields
                        .iter()
                        .any(|(_, fty)| self.literal_field_owns_handles(fty, depth + 1));
                }
                false
            }
            Type::Array(a) => self.literal_field_owns_handles(&a.elem, depth + 1),
            Type::Tuple(t) => t
                .elems
                .iter()
                .any(|e| self.literal_field_owns_handles(e, depth + 1)),
            _ => false,
        }
    }

    // -- F1: fallible externs (A_min) --------------------------------

    /// The return type of an extern call (Zig
    /// `checkExternContractCall`): a fallible extern's payload must
    /// fit the fallible ABI scalar set, and the call then yields
    /// `Result<payload, i32>` instead of the bare payload.
    pub fn extern_call_return(
        &mut self,
        name: &str,
        ext: &ExternSig,
    ) -> CheckResult<Type> {
        if ext.fallible {
            if !fallible_payload_supported(&ext.ret_abi) {
                return Err(Diagnostic::new(
                    ErrorKind::FalliblePayloadUnsupported,
                    format!(
                        "fallible extern `{name}` has payload type `{}` outside the fallible ABI scalar set; the status/payload cell cannot carry it",
                        ext.ret_abi
                    ),
                ));
            }
            return Ok(make_result_type(crate::checker::extern_ret_type(
                &ext.ret_abi,
            )));
        }
        Ok(crate::checker::extern_ret_type(&ext.ret_abi))
    }

    /// Zig `isFallibleProvenanceCall`: the callee is a fallible
    /// extern, or a function whose declaration carries the
    /// fallible sugar flag.
    pub fn is_fallible_provenance_call(&self, call: &CallExpr) -> bool {
        let name = call.func_name.as_str();
        if let Some(ext) = self.extern_funcs.get(name) {
            return ext.fallible;
        }
        if let Some(sig) = self.funcs.get(name) {
            return sig.ret_fallible_sugar;
        }
        if let Some(sig) = self.imported_funcs.get(name) {
            return sig.ret_fallible_sugar;
        }
        false
    }

    /// The expr-statement arm's A_min gate: a bare statement whose
    /// value is a fallible-provenance call typed `Result<_, _>`
    /// discards the status word and is rejected with
    /// FallibleResultDiscarded. `let _ =` discards explicitly and
    /// never reaches this arm.
    pub fn check_fallible_stmt_consumption(
        &mut self,
        inner: &Node,
        ty: &Type,
    ) -> CheckResult<()> {
        let Node::CallExpr(call) = inner else {
            return Ok(());
        };
        let is_result = matches!(
            ty,
            Type::UserDefined(u) if u.name == "Result" && u.generics.len() == 2
        );
        if is_result && self.is_fallible_provenance_call(call) {
            return Err(Diagnostic::new(
                ErrorKind::FallibleResultDiscarded,
                format!(
                    "fallible result of `{}` must be consumed: match it, return it, bind it, or discard it explicitly with `let _ =`; a bare statement discards the status word",
                    call.func_name
                ),
            ));
        }
        Ok(())
    }
}

/// Zig `makeResultType`: `Result<payload, i32>`.
pub(crate) fn make_result_type(payload: Type) -> Type {
    Type::UserDefined(sla_ast::types::UserDefined {
        name: "Result".to_string(),
        generics: vec![
            payload,
            Type::Primitive(Primitive::I32),
        ],
    })
}

/// Zig `abiFalliblePayloadSupported` (lowering_rules): strip one
/// leading `&`/`^`/`*` and one trailing `!`, then test membership
/// in the scalar set the status/payload cell can carry.
pub(crate) fn fallible_payload_supported(raw: &str) -> bool {
    let mut s = raw.trim();
    if let Some(rest) = s
        .strip_prefix('&')
        .or_else(|| s.strip_prefix('^'))
        .or_else(|| s.strip_prefix('*'))
    {
        s = rest;
    }
    if let Some(rest) = s.strip_suffix('!') {
        s = rest;
    }
    matches!(
        s.trim(),
        "ptr"
            | "bool"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "int"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "f32"
            | "f64"
            | "float"
    )
}

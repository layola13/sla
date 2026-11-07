//! Name-special call arms, mirroring the contiguous bare-name
//! block of the Zig checker's `call_expr` arm (venue head
//! 6002b386, type_checker.zig:8805-8895) that sits AFTER the
//! associated-target block and BEFORE closure/fn-ptr locals,
//! overload resolution, user functions, externs and macros —
//! so these names can never be shadowed by user code (venue
//! probes: a user `fn println(x: i32)` does not stop
//! `println(1, 2)` checking rc=0; a user `fn len(a, b)` does not
//! stop 1-arg `len(1)` checking rc=0).
//!
//! Arm inventory (name facts in lowering_rules.zig:4301-4336):
//!   * `vec(...)`        — >=1 arg, all args `typesEqual` to the
//!     first, returns `Vec<elem>`; 0 args is InvalidArgsCount
//!     with Zig's setError text "vec requires at least one
//!     element".
//!   * `len(x)`          — exactly 1 arg; the argument expression
//!     is checked but its TYPE IS NOT inspected (`len(42)`
//!     checks rc=0 on the venue); returns usize. Any other arity
//!     is InvalidArgsCount with setError "len call arity
//!     mismatch: {n}".
//!   * `str_eq(a, b)`    — exactly 2 args, both string-like
//!     (lowering_rules `isStringLikeType`: raw_ptr / array /
//!     String after a borrow/pointer peel — string literals and
//!     arrays both qualify), returns boolean. Arguments are
//!     checked only, never consumed (a String binding survives
//!     two `str_eq` calls, venue probe).
//!   * `format(...)`     — >=1 arg and args[0] must be a string
//!     LITERAL node, else InvalidArgsCount (a String variable as
//!     the format string is rejected, venue probe); remaining
//!     args are checked; returns String.
//!   * `hash(x)`         — exactly 1 arg, `typeIsHash` gate,
//!     returns u64.
//!   * `debug(x)`        — exactly 1 arg, `typeIsDebug` gate,
//!     returns String.
//!   * `println(...)`    — ANY arity (0 included); every argument
//!     is checked, none is consumed (a String binding survives
//!     repeated `println`, venue probe); returns void.
//!   * `std__panic__catch_unwind(c)` / `panic::catch_unwind(c)`
//!     — exactly 1 arg; the argument must check as a 0-parameter
//!     closure whose literal body is a direct `panic` /
//!     `panic_msg` call, else TypeMismatch with setError
//!     "catch_unwind currently only supports direct panic
//!     bodies"; returns `Result<i32, i32>`.
//!
//! The neighbouring `panic` / `panic_msg` arm of the same block
//! (:8869) is NOT duplicated here: this crate already checks
//! those names in the builtin step of `check_call` (same
//! observable verdicts for bare calls).
//!
//! `typeIsHash` / `typeIsDebug` (type_checker.zig:4807/4819) are
//! derive-gated recursions over lowering_rules bases:
//!   * hash base: a primitive is hashable unless raw_ptr /
//!     void / float (f32/f64/float);
//!   * debug base: a primitive is a Copy value unless raw_ptr /
//!     void (floats INCLUDED — `debug(1.5)` checks rc=0 while
//!     `hash(1.5)` is rc=1, venue probes);
//!   * a user-defined STD OWNER type (Vec/String/Box/... —
//!     `userDefinedStdOwnerIsNonCopy`) decides `false` at the
//!     base, so `hash(format_result)` and `debug(string_value)`
//!     are both TypeMismatch on the venue;
//!   * any other user type needs a struct declaration with the
//!     matching `@derive` gate (case-insensitive), non-opaque,
//!     non-union, and every field recursively satisfying the
//!     predicate (a String field poisons `hash`, venue probe);
//!   * every other shape (tuple, array, pointer, ...) is false.
//! Zig's field recursion is unbounded; this port carries a
//! visiting set like `type_is_eq_ord` in check_binary.rs, so a
//! self-referential derive struct decides `false` instead of
//! recursing forever.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{bool_ty, is_string_like_type, types_equal, u64_ty, void_ty};
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};
use std::collections::HashSet;

fn invalid_args(msg: String) -> Diagnostic {
    Diagnostic::new(ErrorKind::InvalidArgsCount, msg)
}

fn type_mismatch(msg: String) -> Diagnostic {
    Diagnostic::new(ErrorKind::TypeMismatch, msg)
}

fn string_ty() -> Type {
    Type::UserDefined(sla_ast::types::UserDefined {
        name: "String".to_string(),
        generics: Vec::new(),
    })
}

/// lowering_rules `userDefinedStdOwnerIsNonCopy` (venue
/// lowering_rules.zig:4972): std container/owner names that
/// decide `false` at every derive base.
fn is_std_owner_name(name: &str) -> bool {
    matches!(
        name,
        "Vec"
            | "GrowVec"
            | "VecDeque"
            | "String"
            | "Box"
            | "Rc"
            | "Arc"
            | "HashMap"
            | "BTreeMap"
            | "HashSet"
            | "BTreeSet"
            | "RefCell"
            | "Mutex"
            | "RwLock"
            | "JoinHandle"
    )
}

/// lowering_rules `primitiveIsHashable` (venue :6563).
fn primitive_is_hashable(p: Primitive) -> bool {
    !matches!(
        p,
        Primitive::RawPtr
            | Primitive::VoidType
            | Primitive::F32
            | Primitive::F64
            | Primitive::Float
    )
}

/// lowering_rules `primitiveIsCopyValue` (venue :6802), the
/// debug arm's primitive base.
fn primitive_is_copy_value(p: Primitive) -> bool {
    !matches!(p, Primitive::RawPtr | Primitive::VoidType)
}

impl Checker {
    /// The bare-name special block. Returns `Some(..)` when the
    /// call's name belongs to the block (the verdict, success or
    /// failure, is final — exactly as Zig returns from the arm);
    /// `None` when the name is not special and resolution must
    /// continue.
    pub fn check_name_special_call(
        &mut self,
        call: &CallExpr,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        match name {
            "vec" => Some(self.check_vec_call(call)),
            "len" => Some(self.check_len_call(call)),
            "str_eq" => Some(self.check_str_eq_call(call)),
            "format" => Some(self.check_format_call(call)),
            "hash" => Some(self.check_hash_call(call)),
            "debug" => Some(self.check_debug_call(call)),
            "println" => Some(self.check_println_call(call)),
            "std__panic__catch_unwind" => {
                Some(self.check_catch_unwind_call(call))
            }
            _ => {
                // Associated spelling `panic::catch_unwind`
                // (Zig's second disjunct); the `std::panic::`
                // source form is normalized to the bare
                // `std__panic__catch_unwind` name by the parser.
                if call.associated_target.as_deref() == Some("panic")
                    && name == "catch_unwind"
                {
                    Some(self.check_catch_unwind_call(call))
                } else {
                    None
                }
            }
        }
    }

    /// Venue :8805 — `vec` constructor arm.
    fn check_vec_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.is_empty() {
            return Err(invalid_args(
                "vec requires at least one element".to_string(),
            ));
        }
        let elem_ty = self.check_expr(&call.args[0])?;
        for arg in &call.args[1..] {
            let arg_ty = self.check_expr(arg)?;
            if !types_equal(&elem_ty, &arg_ty) {
                return Err(type_mismatch(format!(
                    "TypeMismatch: vec element type mismatch in call vec with {} args",
                    call.args.len()
                )));
            }
        }
        Ok(Type::UserDefined(sla_ast::types::UserDefined {
            name: "Vec".to_string(),
            generics: vec![elem_ty],
        }))
    }

    /// Venue :8822 — `len` arm: arity is the ONLY gate; the
    /// argument's type is deliberately not inspected.
    fn check_len_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args(format!(
                "len call arity mismatch: {}",
                call.args.len()
            )));
        }
        self.check_expr(&call.args[0])?;
        Ok(Type::Primitive(Primitive::Usize))
    }

    /// Venue :8832 — `str_eq` arm.
    fn check_str_eq_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 2 {
            return Err(invalid_args(format!(
                "InvalidArgsCount: `str_eq` expects 2 args, got {}",
                call.args.len()
            )));
        }
        let left_ty = self.check_expr(&call.args[0])?;
        let right_ty = self.check_expr(&call.args[1])?;
        if !is_string_like_type(&left_ty) || !is_string_like_type(&right_ty) {
            return Err(type_mismatch(
                "TypeMismatch: str_eq arguments must be string-like".to_string(),
            ));
        }
        Ok(bool_ty())
    }

    /// Venue :8841 — `format` arm: the format string must be a
    /// string literal NODE, not merely a string-typed value.
    fn check_format_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        let first_is_string_literal = matches!(
            call.args.first(),
            Some(Node::Literal(sla_ast::exprs::Literal::StringVal(_)))
        );
        if !first_is_string_literal {
            return Err(invalid_args(format!(
                "InvalidArgsCount: `format` expects a string literal first argument, got {} args",
                call.args.len()
            )));
        }
        for arg in &call.args[1..] {
            self.check_expr(arg)?;
        }
        Ok(string_ty())
    }

    /// Venue :8848 — `hash` arm.
    fn check_hash_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args(format!(
                "InvalidArgsCount: `hash` expects 1 arg, got {}",
                call.args.len()
            )));
        }
        let arg_ty = self.check_expr(&call.args[0])?;
        if !self.type_is_hash(&arg_ty) {
            return Err(type_mismatch(
                "TypeMismatch: hash argument type does not implement Hash".to_string(),
            ));
        }
        Ok(u64_ty())
    }

    /// Venue :8854 — `debug` arm.
    fn check_debug_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args(format!(
                "InvalidArgsCount: `debug` expects 1 arg, got {}",
                call.args.len()
            )));
        }
        let arg_ty = self.check_expr(&call.args[0])?;
        if !self.type_is_debug(&arg_ty) {
            return Err(type_mismatch(
                "TypeMismatch: debug argument type does not implement Debug".to_string(),
            ));
        }
        Ok(string_ty())
    }

    /// Venue :8860 — `println` arm: any arity, arguments checked
    /// and never consumed, void result.
    fn check_println_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        for arg in &call.args {
            self.check_expr(arg)?;
        }
        Ok(void_ty())
    }

    /// Venue :8878 — `catch_unwind` arm.
    fn check_catch_unwind_call(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args(format!(
                "InvalidArgsCount: `catch_unwind` expects 1 arg, got {}",
                call.args.len()
            )));
        }
        let closure_expr = &call.args[0];
        let closure_ty = self.check_expr(closure_expr)?;
        let is_nullary_closure = matches!(
            &closure_ty,
            Type::Closure(c) if c.params.is_empty()
        );
        if !is_nullary_closure {
            return Err(type_mismatch(
                "TypeMismatch: catch_unwind argument must be a nullary closure".to_string(),
            ));
        }
        let body_is_panic_call = match closure_expr {
            Node::ClosureLiteral(lit) => match &*lit.body {
                Node::CallExpr(body_call) => {
                    crate::checker::is_panic_builtin_name(&body_call.func_name)
                }
                _ => false,
            },
            _ => false,
        };
        if !body_is_panic_call {
            return Err(type_mismatch(
                "catch_unwind currently only supports direct panic bodies".to_string(),
            ));
        }
        Ok(Type::UserDefined(sla_ast::types::UserDefined {
            name: "Result".to_string(),
            generics: vec![
                Type::Primitive(Primitive::I32),
                Type::Primitive(Primitive::I32),
            ],
        }))
    }

    /// Zig `typeIsHash` (type_checker.zig:4807).
    pub fn type_is_hash(&self, ty: &Type) -> bool {
        self.type_is_named_derive(ty, false, &mut HashSet::new())
    }

    /// Zig `typeIsDebug` (type_checker.zig:4819).
    pub fn type_is_debug(&self, ty: &Type) -> bool {
        self.type_is_named_derive(ty, true, &mut HashSet::new())
    }

    /// Shared recursion of `typeIsHash` / `typeIsDebug` over
    /// lowering_rules `typeHasNamedDeriveBase` +
    /// `typeHasNamedDeriveStructGate` (venue lowering_rules.zig:
    /// 6635/6651). `debug` selects the Copy-value primitive base
    /// and the "debug" derive name; otherwise the hashable base
    /// and "hash".
    fn type_is_named_derive(
        &self,
        ty: &Type,
        debug: bool,
        visiting: &mut HashSet<String>,
    ) -> bool {
        // Base decision (typeHasNamedDeriveBase).
        match ty {
            Type::Primitive(p) => {
                return if debug {
                    primitive_is_copy_value(*p)
                } else {
                    primitive_is_hashable(*p)
                };
            }
            Type::UserDefined(u) => {
                if is_std_owner_name(&u.name) {
                    return false;
                }
                // Not a base case: fall through to the struct
                // gate below.
            }
            _ => return false,
        }
        // Struct gate: declaration must exist, carry the derive,
        // and be neither opaque nor a union.
        let Some((name, info)) = self.struct_decl_for_type(ty) else {
            return false;
        };
        let wanted = if debug { "debug" } else { "hash" };
        let gated = info
            .derives
            .iter()
            .any(|d| d.eq_ignore_ascii_case(wanted));
        if !gated || info.is_opaque || info.is_union {
            return false;
        }
        if !visiting.insert(name.clone()) {
            return false;
        }
        let ok = info
            .fields
            .iter()
            .all(|(_, fty)| self.type_is_named_derive(fty, debug, visiting));
        visiting.remove(&name);
        ok
    }
}

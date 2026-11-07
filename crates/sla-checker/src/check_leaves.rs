//! Std-surface associated-call leaves (R-1 port): the name-keyed
//! leaf block of the Zig checker's associated-target arm (venue
//! tree 753fd682, type_checker.zig:8940-9490), which sits between
//! the `ptr` leaves and the inherent `{Target}_{name}` lookup.
//! Before this port the Rust checker carried only the consumption
//! specials (`mem::forget`, `Box::into_raw`,
//! `ManuallyDrop::into_inner`), so every static constructor and
//! intrinsic leaf — `Vec::new`, `HashMap::new`, `future::ready`,
//! ... — fell through to "Undefined call" in ordinary fn bodies
//! (census class R-1: the check form never assembled this surface
//! for ANY checked body; @test bodies only appeared to work
//! because `sla check` does not parse them at all).
//!
//! The `future::*` / `poll::*` leaves share their bodies with the
//! bare `future__*` / `poll__*` forms in check_preassoc.rs (the
//! Zig arms are gate-identical), so they delegate there.
//!
//! Registered residuals (docs/divergences/checker.md):
//!   * `Chan::new` lacks the annotation-driven payload table
//!     (Zig `chan_new_pending` / `chan_calls`): the leaf checks
//!     arity and the numeric capacity and yields `Chan<infer>`;
//!     the end-of-program unannotated-creation rejection is not
//!     ported.
//!   * `str::from_utf8` lives at the END of the call dispatch
//!     (check_call.rs tail), mirroring its Zig position after the
//!     imported-macro stage (venue :10526).

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use crate::types_util::{
    bool_ty, i32_ty, is_numeric_type, is_string_like_type, peel_borrow_pointer,
    slice_element_type, u64_ty,
};
use sla_ast::exprs::{CallExpr, Literal};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, TupleType, Type, UserDefined};

pub(crate) fn user_ty(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

pub(crate) fn invalid_args() -> Diagnostic {
    Diagnostic::new(ErrorKind::InvalidArgsCount, "InvalidArgsCount")
}

pub(crate) fn type_mismatch() -> Diagnostic {
    Diagnostic::new(ErrorKind::TypeMismatch, "TypeMismatch")
}

fn arity(call: &CallExpr, n: usize) -> CheckResult<()> {
    if call.args.len() != n {
        return Err(invalid_args());
    }
    Ok(())
}

fn generics_arity(call: &CallExpr, n: usize) -> CheckResult<()> {
    if call.generics.len() != n {
        return Err(invalid_args());
    }
    Ok(())
}

/// lowering_rules single-generic inner of a peeled user-defined
/// type (`Vec<T>` -> `T`, `Task<T>` -> `T`, ...).
pub(crate) fn ud_inner(ty: &Type, name: &str) -> Option<Type> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == name && u.generics.len() == 1 => {
            Some(u.generics[0].clone())
        }
        _ => None,
    }
}

/// The peeled user-defined name of a type, if any.
pub(crate) fn ud_name(ty: &Type) -> Option<&str> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) => Some(u.name.as_str()),
        _ => None,
    }
}

fn result_ty(ok: Type, err: Type) -> Type {
    user_ty("Result", vec![ok, err])
}

/// Mirrors Zig `typeIsCopyValue` on the shapes the leaf block
/// consults: `Chan` handles are copy values, arrays recurse,
/// fn-ptrs are copy-value leaves (lowering_rules
/// `typeIsCopyValueLeaf`); everything else defers to the
/// checker's `typeIsCopy` (which carries the struct derive gate
/// and the tuple recursion).
pub(crate) fn is_copy_value(checker: &Checker, ty: &Type) -> bool {
    match ty {
        Type::UserDefined(u) if u.name == "Chan" => true,
        Type::Array(a) => is_copy_value(checker, &a.elem),
        Type::FnPtr(_) => true,
        _ => checker.type_is_copy(ty),
    }
}

impl Checker {
    /// Zig `consumeConstructorArg`: an identifier argument whose
    /// type is not a copy value is consumed by the constructor
    /// (`Box::new`, `Rc::new`, `Arc::new`). The argument has
    /// already been checked by the caller, so the state reads here
    /// mirror Zig's defensive re-checks.
    pub(crate) fn consume_constructor_arg(
        &mut self,
        arg: &Node,
        arg_ty: &Type,
        context: &str,
    ) -> CheckResult<()> {
        let Node::Identifier(name) = arg else {
            return Ok(());
        };
        if is_copy_value(self, arg_ty) {
            return Ok(());
        }
        match self.lookup_state(name) {
            None => {
                return Err(Diagnostic::new(
                    ErrorKind::UndefinedVariable,
                    format!("UndefinedVariable: `{name}`"),
                ))
            }
            Some(ValueState::Consumed) => {
                return Err(Diagnostic::new(
                    ErrorKind::UseAfterMove,
                    format!("UseAfterMove: `{name}` was already consumed"),
                ))
            }
            Some(ValueState::Uninitialized) => {
                return Err(Diagnostic::new(
                    ErrorKind::UseBeforeInit,
                    format!("UseBeforeInit: var `{name}` is read before assignment"),
                ))
            }
            _ => {}
        }
        self.consume_binding(name, context)
    }

    /// The std associated-leaf dispatch. Returns `Some(..)` when
    /// the (target, name) pair is a leaf (the verdict is final,
    /// as in Zig); `None` when resolution must continue to the
    /// inherent lookup and beyond.
    pub fn check_std_assoc_leaf(
        &mut self,
        target: &str,
        call: &CallExpr,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        Some(match (target, name) {
            ("ManuallyDrop", "new") => self.leaf_manually_drop_new(call),
            ("mpsc", "channel") => self.leaf_mpsc_channel(call),
            ("thread", "spawn") => self.leaf_thread_spawn(call),
            ("future", "ready") => self.leaf_future_ready(call),
            ("future", "defer_ready") => self.leaf_future_defer_ready(call),
            ("future", "pending") => self.leaf_future_pending(call),
            ("future", "join2") => self.check_future_join2_call(call, false),
            ("future", "select2") => self.check_future_join2_call(call, true),
            ("future", "pair_left") => {
                self.check_future_pair_accessor_call(call, true)
            }
            ("future", "pair_right") => {
                self.check_future_pair_accessor_call(call, false)
            }
            ("future", "either_side") => {
                self.check_future_either_side_call(call)
            }
            ("future", "either_left") => {
                self.check_future_either_accessor_call(call, true)
            }
            ("future", "either_right") => {
                self.check_future_either_accessor_call(call, false)
            }
            ("poll", "ready") => self.check_poll_ready_call(call),
            ("poll", "pending") => self.check_poll_pending_call(call),
            ("poll", "is_ready") | ("poll", "is_pending") => {
                self.check_poll_is_call(call)
            }
            ("poll", "value") => self.check_poll_value_call(call),
            ("task", "new") => self.leaf_task_new(call),
            ("task", "poll") | ("task", "is_ready") => {
                self.leaf_task_poll(call)
            }
            ("task", "result") => self.leaf_task_result(call),
            ("task", "state") => self.leaf_task_state(call),
            ("executor", "new") => self.leaf_executor_new(call),
            ("executor", "poll_one") => self.leaf_executor_poll_one(call),
            ("executor", "poll_ready_count") => {
                self.leaf_executor_poll_ready_count(call)
            }
            ("Box", "new") => self.leaf_smart_new(call, "Box"),
            ("Box", "from_raw") => self.leaf_box_from_raw(call),
            ("Rc", "new") => self.leaf_smart_new(call, "Rc"),
            ("Arc", "new") => self.leaf_smart_new(call, "Arc"),
            ("AtomicI32", "new") => self.leaf_atomic_new(call, "AtomicI32"),
            ("AtomicUsize", "new") => self.leaf_atomic_new(call, "AtomicUsize"),
            ("AtomicPtr", "new") => self.leaf_atomic_ptr_new(call),
            ("Cell", "new") => self.leaf_cell_new(call),
            ("RefCell", "new") => self.leaf_refcell_new(call),
            ("Mutex", "new") => self.leaf_lock_new(call, "Mutex"),
            ("RwLock", "new") => self.leaf_lock_new(call, "RwLock"),
            ("File", "open") => self.leaf_file_open(call),
            ("VecDeque", "from") => self.leaf_vecdeque_from(call),
            ("VecDeque", "new") => self.leaf_zero_new(call, "VecDeque"),
            ("HashMap", "new") => self.leaf_hash_map_new(call),
            ("HashSet", "new") => self.leaf_zero_new(call, "HashSet"),
            ("BTreeMap", "new") => self.leaf_btree_map_new(call),
            ("BTreeSet", "new") => self.leaf_zero_new(call, "BTreeSet"),
            ("Vec", "new") => self.leaf_zero_new(call, "Vec"),
            ("GrowVec", "new") => self.leaf_zero_new(call, "GrowVec"),
            ("GrowVec", "with_max") => self.leaf_growvec_with_max(call),
            ("Chan", "new") => self.leaf_chan_new(call),
            _ => return None,
        })
    }

    fn leaf_manually_drop_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let inner = self.check_expr(&call.args[0])?;
        Ok(user_ty("ManuallyDrop", vec![inner]))
    }

    fn leaf_mpsc_channel(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 0)?;
        // Zig hardcodes the element type to i32 (the surface
        // predates generic channels in the checker).
        let elem = i32_ty();
        Ok(Type::Tuple(TupleType {
            elems: vec![
                user_ty("Sender", vec![elem.clone()]),
                user_ty("Receiver", vec![elem]),
            ],
        }))
    }

    fn leaf_thread_spawn(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let closure_ty = self.check_expr(&call.args[0])?;
        let Type::Closure(c) = &closure_ty else {
            return Err(type_mismatch());
        };
        if !c.params.is_empty() {
            return Err(type_mismatch());
        }
        // An integer-typed closure return is coerced to i32.
        let ret = if matches!(*c.ret, Type::Primitive(Primitive::Integer)) {
            i32_ty()
        } else {
            (*c.ret).clone()
        };
        Ok(user_ty("JoinHandle", vec![ret]))
    }

    fn leaf_future_ready(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let inner = self.check_expr(&call.args[0])?;
        Ok(Type::Future(Box::new(inner)))
    }

    fn leaf_future_defer_ready(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        generics_arity(call, 0)?;
        let inner = self.check_expr(&call.args[0])?;
        Ok(Type::Future(Box::new(inner)))
    }

    fn leaf_future_pending(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 0)?;
        generics_arity(call, 1)?;
        Ok(Type::Future(Box::new(call.generics[0].clone())))
    }

    fn leaf_task_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let future_ty = self.check_expr(&call.args[0])?;
        match future_ty {
            Type::Future(inner) => Ok(user_ty("Task", vec![*inner])),
            _ => Err(type_mismatch()),
        }
    }

    fn leaf_task_poll(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let task_ty = self.check_expr(&call.args[0])?;
        ud_inner(&task_ty, "Task").ok_or_else(type_mismatch)?;
        Ok(bool_ty())
    }

    fn leaf_task_result(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let task_ty = self.check_expr(&call.args[0])?;
        ud_inner(&task_ty, "Task").ok_or_else(type_mismatch)
    }

    fn leaf_task_state(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let task_ty = self.check_expr(&call.args[0])?;
        ud_inner(&task_ty, "Task").ok_or_else(type_mismatch)?;
        Ok(u64_ty())
    }

    fn leaf_executor_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        generics_arity(call, 0)?;
        let tasks_ty = self.check_expr(&call.args[0])?;
        // lowering_rules `executorTaskBufferPlan`: a fixed array
        // of Task<T>, or a Vec<Task<T>>, yields the payload T.
        let inner = match peel_borrow_pointer(&tasks_ty) {
            Type::Array(a) => ud_inner(&a.elem, "Task"),
            other => {
                ud_inner(other, "Vec").and_then(|elem| ud_inner(&elem, "Task"))
            }
        }
        .ok_or_else(type_mismatch)?;
        Ok(user_ty("Executor", vec![inner]))
    }

    fn leaf_executor_poll_one(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 2)?;
        generics_arity(call, 0)?;
        let executor_ty = self.check_expr(&call.args[0])?;
        ud_inner(&executor_ty, "Executor").ok_or_else(type_mismatch)?;
        let index_ty = self.check_expr(&call.args[1])?;
        if !is_numeric_type(&index_ty) {
            return Err(type_mismatch());
        }
        Ok(bool_ty())
    }

    fn leaf_executor_poll_ready_count(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        arity(call, 1)?;
        generics_arity(call, 0)?;
        let executor_ty = self.check_expr(&call.args[0])?;
        ud_inner(&executor_ty, "Executor").ok_or_else(type_mismatch)?;
        Ok(u64_ty())
    }

    /// `Box::new` / `Rc::new` / `Arc::new` (predicate-gated on the
    /// exact type name in Zig): the argument is consumed unless
    /// it is a copy value.
    fn leaf_smart_new(
        &mut self,
        call: &CallExpr,
        type_name: &str,
    ) -> CheckResult<Type> {
        arity(call, 1)?;
        let inner = self.check_expr(&call.args[0])?;
        let context = format!("{type_name}::new");
        self.consume_constructor_arg(&call.args[0], &inner, &context)?;
        Ok(user_ty(type_name, vec![inner]))
    }

    fn leaf_box_from_raw(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if self.unsafe_depth == 0 {
            return Err(Diagnostic::new(
                ErrorKind::CompileError,
                "Box::from_raw requires unsafe".to_string(),
            ));
        }
        arity(call, 1)?;
        let ptr_ty = self.check_expr(&call.args[0])?;
        match &ptr_ty {
            Type::Pointer(inner) | Type::Borrow(inner) => {
                Ok(user_ty("Box", vec![(**inner).clone()]))
            }
            _ => Err(type_mismatch()),
        }
    }

    fn leaf_atomic_new(
        &mut self,
        call: &CallExpr,
        type_name: &str,
    ) -> CheckResult<Type> {
        arity(call, 1)?;
        let value_ty = self.check_expr(&call.args[0])?;
        if !is_numeric_type(&value_ty) {
            return Err(type_mismatch());
        }
        Ok(user_ty(type_name, Vec::new()))
    }

    fn leaf_atomic_ptr_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let value_ty = self.check_expr(&call.args[0])?;
        match &value_ty {
            Type::Pointer(inner) => {
                Ok(user_ty("AtomicPtr", vec![(**inner).clone()]))
            }
            _ => Err(type_mismatch()),
        }
    }

    fn leaf_cell_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let value_ty = self.check_expr(&call.args[0])?;
        // lowering_rules `isCellValueType`: numeric or boolean.
        let ok = is_numeric_type(&value_ty)
            || matches!(value_ty, Type::Primitive(Primitive::Boolean));
        if !ok {
            return Err(type_mismatch());
        }
        Ok(user_ty("Cell", vec![value_ty]))
    }

    fn leaf_refcell_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let value_ty = self.check_expr(&call.args[0])?;
        Ok(user_ty("RefCell", vec![value_ty]))
    }

    /// `Mutex::new` / `RwLock::new`: numeric argument; an integer
    /// LITERAL argument is re-typed to i32 in `expr_types` (Zig
    /// rewrites the literal's recorded type).
    fn leaf_lock_new(
        &mut self,
        call: &CallExpr,
        type_name: &str,
    ) -> CheckResult<Type> {
        arity(call, 1)?;
        let value_ty = self.check_expr(&call.args[0])?;
        if !is_numeric_type(&value_ty) {
            return Err(type_mismatch());
        }
        let inner =
            if matches!(&call.args[0], Node::Literal(Literal::IntVal(_))) {
                i32_ty()
            } else {
                value_ty.clone()
            };
        if inner != value_ty {
            self.expr_types.insert(
                crate::types_util::node_key_of(&call.args[0]),
                inner.clone(),
            );
        }
        Ok(user_ty(type_name, vec![inner]))
    }

    fn leaf_file_open(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let path_ty = self.check_expr(&call.args[0])?;
        if !is_string_like_type(&path_ty) {
            return Err(type_mismatch());
        }
        Ok(result_ty(user_ty("File", Vec::new()), i32_ty()))
    }

    fn leaf_vecdeque_from(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let array_ty = self.check_expr(&call.args[0])?;
        match peel_borrow_pointer(&array_ty) {
            Type::Array(a) => Ok(user_ty("VecDeque", vec![(*a.elem).clone()])),
            _ => Err(type_mismatch()),
        }
    }

    /// The zero-argument generic constructors: `Vec::new`,
    /// `VecDeque::new`, `HashSet::new`, `BTreeSet::new`,
    /// `GrowVec::new` — element from the single generic, else
    /// `infer` (concretized by the first insert/push, see the
    /// receiver cascade).
    fn leaf_zero_new(
        &mut self,
        call: &CallExpr,
        type_name: &str,
    ) -> CheckResult<Type> {
        arity(call, 0)?;
        let elem = if call.generics.len() == 1 {
            call.generics[0].clone()
        } else {
            Type::Infer
        };
        Ok(user_ty(type_name, vec![elem]))
    }

    fn leaf_hash_map_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 0)?;
        let key = call.generics.first().cloned().unwrap_or(Type::Infer);
        let value = call.generics.get(1).cloned().unwrap_or(Type::Infer);
        Ok(user_ty("HashMap", vec![key, value]))
    }

    fn leaf_btree_map_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 0)?;
        let key = call.generics.first().cloned().unwrap_or(Type::Infer);
        let value = call.generics.get(1).cloned().unwrap_or(Type::Infer);
        Ok(user_ty("BTreeMap", vec![key, value]))
    }

    fn leaf_growvec_with_max(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let max_ty = self.check_expr(&call.args[0])?;
        if !is_numeric_type(&max_ty) {
            return Err(type_mismatch());
        }
        let elem = if call.generics.len() == 1 {
            call.generics[0].clone()
        } else {
            Type::Infer
        };
        Ok(user_ty("GrowVec", vec![elem]))
    }

    fn leaf_chan_new(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        generics_arity(call, 0)?;
        let cap_ty = self.check_expr(&call.args[0])?;
        if !is_numeric_type(&cap_ty) {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                "Chan::new: capacity must be an integer".to_string(),
            ));
        }
        // The payload normally arrives from the let's `Chan<T>`
        // annotation through Zig's `chan_new_pending` table,
        // which this checker does not carry (registered
        // residual); `infer` unifies with the annotation at the
        // let site.
        Ok(user_ty("Chan", vec![Type::Infer]))
    }

    /// `str::from_utf8(bytes)` — Zig position is AFTER the
    /// imported-macro stage (venue :10518), so check_call.rs
    /// invokes this from its tail, just before the Undefined
    /// fallback. The argument must be a `Slice<u8>` (a borrow of
    /// one is unwrapped the way Zig unwraps the borrow argument).
    pub fn check_str_from_utf8_call(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        arity(call, 1)?;
        let bytes_ty = self.check_expr(&call.args[0])?;
        let elem = match slice_element_type(&bytes_ty) {
            Some(elem) => Some(elem),
            None => match &call.args[0] {
                Node::BorrowExpr(b) => {
                    let borrowed_ty = self.check_expr(&b.expr)?;
                    slice_element_type(&borrowed_ty)
                }
                _ => None,
            },
        }
        .ok_or_else(type_mismatch)?;
        if !matches!(elem, Type::Primitive(Primitive::U8)) {
            return Err(type_mismatch());
        }
        Ok(result_ty(user_ty("String", Vec::new()), i32_ty()))
    }

    /// Mirrors Zig `typeIsShallowCopyCallArgValue` (the DEF-5
    /// by-value argument discipline): a value of this type passed
    /// by value is auto-cloned, so the source binding stays live.
    /// Base facts from lowering_rules: std collections are
    /// shallow-copyable only below the surface (depth > 0);
    /// primitive/pointer/borrow/fn-ptr leaves are always true;
    /// std owner types (String/Box/Rc/Arc/RefCell/...) behave
    /// like collections; enums are true; structs recurse through
    /// their fields behind the !opaque/!union gate.
    pub(crate) fn is_shallow_copy_call_arg(
        &self,
        ty: &Type,
        depth: usize,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        if is_std_collection_type(ty) {
            return depth > 0;
        }
        match ty {
            Type::Primitive(_)
            | Type::Pointer(_)
            | Type::Borrow(_)
            | Type::FnPtr(_) => true,
            Type::Tuple(t) => t
                .elems
                .iter()
                .all(|e| self.is_shallow_copy_call_arg(e, depth + 1)),
            Type::Array(a) => self.is_shallow_copy_call_arg(&a.elem, depth + 1),
            Type::UserDefined(u) => {
                if is_std_owner_non_copy(&u.name)
                    || ud_inner(ty, "Box").is_some()
                    || ud_inner(ty, "Rc").is_some()
                    || ud_inner(ty, "Arc").is_some()
                    || ud_inner(ty, "RefCell").is_some()
                {
                    return depth > 0;
                }
                if self.enums.contains_key(&u.name) {
                    return true;
                }
                match self.struct_decl_for_type(ty) {
                    Some((_, info)) if !info.is_opaque && !info.is_union => {
                        info.fields.iter().all(|(_, fty)| {
                            self.is_shallow_copy_call_arg(fty, depth + 1)
                        })
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

/// lowering_rules `isStdCollectionType`: any of the seven
/// collection shapes extracts an element/key type.
fn is_std_collection_type(ty: &Type) -> bool {
    matches!(ud_name(ty), Some("Vec" | "GrowVec" | "VecDeque"))
        || matches!(
            ud_name(ty),
            Some("HashMap" | "BTreeMap" | "HashSet" | "BTreeSet")
        )
}

/// lowering_rules `userDefinedStdOwnerIsNonCopy` (the name set;
/// the tail of Zig's list — Mutex/RwLock/Cell/Chan/File-class
/// owners — is included on the same principle).
fn is_std_owner_non_copy(name: &str) -> bool {
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
            | "Cell"
            | "Chan"
            | "File"
    )
}

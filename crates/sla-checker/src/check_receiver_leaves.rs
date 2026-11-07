//! Receiver-shape std leaf cascade (R-1 port): the segment of
//! the Zig checker's `call_expr` arm keyed on args[0]'s peeled
//! type (venue tree 753fd682, type_checker.zig:9512-10390) —
//! Option / Result / Atomic / Cell / sync / File / Metadata /
//! channel-handle leaves, the Rc/Arc clone + count leaves, the
//! DEF-deep-copy `.clone()` surface, `as_ptr`, the container
//! leaves (VecDeque / Array / String / Slice / maps / sets),
//! the name-keyed push / pop / remove arms, and the iterable
//! arms (iter / copied / collect / join / map / filter / fold).
//! Before this port none of it existed in the Rust checker, so
//! ordinary fn bodies calling `v.push(x)`, `m.insert(k, v)`,
//! `r.unwrap_or(d)`, `a.fetch_add(1, ord)` all fell to
//! "Undefined call" (census class R-1). The second half of the
//! segment set lives in check_receiver_leaves2.rs.
//!
//! Placement: check_call.rs runs this stage immediately before
//! the receiver-inferred machinery (check_method.rs), which
//! remains the fallback. In Zig the cascade sits before the
//! extern / macro stages; the stage-order divergence is the
//! standing D-C19 family and is registered in checker.md.
//!
//! Registered residuals (checker.md): the iterable arms fire
//! only on iterable receiver shapes (a non-iterable receiver
//! reports the bottom "Undefined call" instead of Zig's
//! TypeMismatch; Zig's inherent method_match precedes those
//! arms, so user methods named map/filter/... resolve
//! identically); push/pop/remove keep Zig's hard-error
//! posture; the arena move-arg route is not modeled (no arena
//! tables in this checker); `consume_insert_arg` uses the push
//! guard set; the struct `.clone()` arm omits Zig's final
//! shallow-copy gate (the UncloneableClone walk IS ported);
//! `__slice_view` is not part of this port.

use crate::check_leaves::{
    invalid_args, is_copy_value, type_mismatch, ud_inner, ud_name, user_ty,
};
use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use crate::types_util::{
    array_type_of, bool_ty, box_inner_type, i32_ty, is_borrow_like_type,
    is_numeric_type, node_key_of, option_inner_type, peel_borrow_pointer,
    prim, result_err_type, result_ok_type, root_identifier_deep,
    slice_element_type, type_desc, type_tag_name, types_equal,
    vec_element_type, void_ty,
};
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::{ClosureType, Primitive, Type};

pub(crate) fn ud2(ty: &Type, name: &str) -> Option<(Type, Type)> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == name && u.generics.len() == 2 => {
            Some((u.generics[0].clone(), u.generics[1].clone()))
        }
        _ => None,
    }
}

pub(crate) fn vec_like_elem(ty: &Type) -> Option<Type> {
    vec_element_type(ty).or_else(|| ud_inner(ty, "GrowVec"))
}

pub(crate) fn iterable_elem(ty: &Type) -> Option<Type> {
    if let Some((elem, _)) = array_type_of(ty) {
        return Some(elem);
    }
    slice_element_type(ty).or_else(|| vec_like_elem(ty))
}

fn is_ordering(ty: &Type) -> bool {
    ud_name(ty) == Some("Ordering")
}

pub(crate) fn unwrapped(ty: &Type) -> Type {
    peel_borrow_pointer(ty).clone()
}

pub(crate) fn arity(call: &CallExpr, n: usize) -> CheckResult<()> {
    if call.args.len() != n {
        return Err(invalid_args());
    }
    Ok(())
}

/// Zig `btreeSetKeySupported`: only raw-pointer primitives and
/// slices are supported B-tree keys on this surface.
pub(crate) fn btree_key_supported(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(Primitive::RawPtr))
        || slice_element_type(ty).is_some()
}

impl Checker {
    /// The cascade entry. `Some(..)` is a final verdict;
    /// `None` falls through to receiver-inferred resolution.
    pub fn check_receiver_leaf_call(
        &mut self,
        call: &CallExpr,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        // Name-keyed container arms first: in Zig they fire on
        // the name alone, ahead of inherent resolution.
        match name {
            "push" => return Some(self.leaf_push(call)),
            "pop" => return Some(self.leaf_pop(call)),
            "remove" => return Some(self.leaf_remove(call)),
            _ => {}
        }
        if call.args.is_empty() {
            return None;
        }
        let recv_ty = match self.check_expr(&call.args[0]) {
            Ok(t) => t,
            Err(e) => return Some(Err(e)),
        };
        let segments: [fn(&mut Checker, &CallExpr, &Type) -> Option<CheckResult<Type>>; 16] = [
            Self::seg_option,
            Self::seg_result,
            Self::seg_atomic_i32,
            Self::seg_atomic_usize,
            Self::seg_cell,
            Self::seg_refcell,
            Self::seg_mutex,
            Self::seg_rwlock,
            Self::seg_file_meta,
            Self::seg_handle_chan,
            Self::seg_clone_counts_asptr,
            Self::seg_atomic_ptr,
            Self::seg_vecdeque,
            Self::seg_array,
            Self::seg_string_slice_ptr,
            Self::seg_maps_sets,
        ];
        for seg in segments {
            if let Some(r) = seg(self, call, &recv_ty) {
                return Some(r);
            }
        }
        self.leaf_iterable(call, &recv_ty)
    }

    // -- closure arguments with context ----------------------------

    /// Zig `checkClosureLiteralWithContext` posture: a closure
    /// literal's `infer` parameters take the context types;
    /// annotated parameters keep their annotation. A non-literal
    /// argument must simply type as a closure.
    pub(crate) fn check_closure_arg(
        &mut self,
        arg: &Node,
        context: &[Type],
    ) -> CheckResult<(Vec<Type>, Type)> {
        if let Node::ClosureLiteral(c) = arg {
            self.scopes.push_frame();
            let mut params: Vec<Type> = Vec::new();
            let mut define_err: Option<Diagnostic> = None;
            for (i, p) in c.params.iter().enumerate() {
                let ty = if matches!(*p.ty, Type::Infer) {
                    context.get(i).cloned().unwrap_or(Type::Infer)
                } else {
                    (*p.ty).clone()
                };
                params.push(ty.clone());
                if define_err.is_none() {
                    if let Err(e) = self.define_symbol_with_state(
                        &p.name,
                        ty,
                        true,
                        ValueState::Active,
                        None,
                    ) {
                        define_err = Some(e);
                    }
                }
            }
            let body_result = match define_err {
                Some(e) => Err(e),
                None => self.check_expr(&c.body),
            };
            self.scopes.pop_frame();
            let ret = body_result?;
            self.expr_types.insert(
                node_key_of(arg),
                Type::Closure(ClosureType {
                    params: params.clone(),
                    ret: Box::new(ret.clone()),
                }),
            );
            return Ok((params, ret));
        }
        match self.check_expr(arg)? {
            Type::Closure(cl) => Ok((cl.params, (*cl.ret).clone())),
            _ => Err(type_mismatch()),
        }
    }

    /// After a closure-typed argument is established: parameter
    /// count and (optionally) first-parameter type gates, in
    /// Zig's order (closure-ness, count, type).
    pub(crate) fn closure_gate(
        params: &[Type],
        want_len: usize,
        want_first: Option<&Type>,
    ) -> CheckResult<()> {
        if params.len() != want_len {
            return Err(invalid_args());
        }
        if let Some(want) = want_first {
            if !types_equal(want, &params[0]) {
                return Err(type_mismatch());
            }
        }
        Ok(())
    }

    // -- consumption / concretization -------------------------------

    /// The push / push_back consumption discipline (Zig venue
    /// push arm): a copy-like element stays live (with the
    /// UncloneableCopy loud rejection); a non-copy, non-borrow
    /// element binding moves into the container.
    pub(crate) fn consume_push_style_arg(
        &mut self,
        recv_arg: &Node,
        arg: &Node,
        arg_ty: &Type,
        context: &str,
    ) -> CheckResult<()> {
        let Node::Identifier(arg_name) = arg else {
            return Ok(());
        };
        if matches!(arg_ty, Type::Infer) {
            return Ok(());
        }
        let copy_like =
            is_copy_value(self, arg_ty) || self.is_shallow_copy_call_arg(arg_ty, 0);
        if copy_like {
            if let Some(bad) = self.uncloneable_type_name(arg_ty, 0) {
                return Err(Diagnostic::marked(
                    ErrorKind::CompileError,
                    "UncloneableCopy",
                    format!(
                        "UncloneableCopy: element of type `{}` pushed here must be copied because the source stays live, but its closure contains {bad}, which has no clone - move an owned value into the container instead, or keep the {bad} outside the copied value",
                        type_desc(arg_ty)
                    ),
                ));
            }
            return Ok(());
        }
        if is_borrow_like_type(arg_ty) {
            return Ok(());
        }
        if let Some(recv_name) = root_identifier_deep(recv_arg) {
            if recv_name == arg_name {
                return Ok(());
            }
        }
        let eligible = matches!(
            self.scopes.lookup(arg_name),
            Some(b) if !b.is_const
                && b.state == ValueState::Active
                && b.borrow_source.is_none()
        );
        if eligible {
            self.consume_binding(arg_name, context)?;
        }
        Ok(())
    }

    /// Zig `consumeInsertArgIfMoved`: insert stores the argument
    /// handle as-is, so owned arguments transfer; copy leaves,
    /// borrow-likes, slice views and Chan handles stay live, as
    /// does a container inserted into itself.
    pub(crate) fn consume_insert_arg(
        &mut self,
        recv_arg: &Node,
        arg: &Node,
        arg_ty: &Type,
    ) -> CheckResult<()> {
        let Node::Identifier(arg_name) = arg else {
            return Ok(());
        };
        if matches!(arg_ty, Type::Infer) {
            return Ok(());
        }
        let copy_leaf = match arg_ty {
            Type::Primitive(p) => {
                !matches!(p, Primitive::RawPtr | Primitive::VoidType)
            }
            Type::FnPtr(_) => true,
            _ => false,
        };
        if copy_leaf || is_borrow_like_type(arg_ty) {
            return Ok(());
        }
        if slice_element_type(arg_ty).is_some()
            || ud_name(arg_ty) == Some("Chan")
        {
            return Ok(());
        }
        if let Some(recv_name) = root_identifier_deep(recv_arg) {
            if recv_name == arg_name {
                return Ok(());
            }
        }
        let eligible = matches!(
            self.scopes.lookup(arg_name),
            Some(b) if !b.is_const
                && b.state == ValueState::Active
                && b.borrow_source.is_none()
        );
        if eligible {
            self.consume_binding(arg_name, "insert argument")?;
        }
        Ok(())
    }

    /// Infer concretization: a receiver binding declared with an
    /// `infer` element type takes the concrete container type
    /// from the first inserting call (Zig writes both the symbol
    /// and the receiver node's recorded type).
    pub(crate) fn concretize_receiver(&mut self, recv_arg: &Node, concrete: Type) {
        if let Some(name) = root_identifier_deep(recv_arg) {
            let name = name.to_string();
            if let Some(b) = self.scopes.lookup_mut(&name) {
                b.ty = concrete.clone();
                self.expr_types.insert(node_key_of(recv_arg), concrete);
            }
        }
    }

    /// Zig `uncloneableTypeName`: the first map/set carrier in a
    /// type's closure (they have no clone), walking Vec/GrowVec/
    /// Box/Array/Tuple and user-defined fields, depth-capped.
    pub(crate) fn uncloneable_type_name(
        &self,
        ty: &Type,
        depth: usize,
    ) -> Option<&'static str> {
        if depth > 8 {
            return None;
        }
        match ud_name(ty) {
            Some("HashMap") => return Some("HashMap"),
            Some("HashSet") => return Some("HashSet"),
            Some("BTreeMap") => return Some("BTreeMap"),
            Some("BTreeSet") => return Some("BTreeSet"),
            _ => {}
        }
        if let Some(e) = vec_element_type(ty) {
            return self.uncloneable_type_name(&e, depth + 1);
        }
        if let Some(e) = ud_inner(ty, "GrowVec") {
            return self.uncloneable_type_name(&e, depth + 1);
        }
        if let Some(e) = box_inner_type(ty) {
            return self.uncloneable_type_name(&e, depth + 1);
        }
        match ty {
            Type::Array(a) => self.uncloneable_type_name(&a.elem, depth + 1),
            Type::Tuple(t) => {
                for e in &t.elems {
                    if let Some(b) = self.uncloneable_type_name(e, depth + 1) {
                        return Some(b);
                    }
                }
                None
            }
            Type::UserDefined(u) => {
                if let Some(variants) = self.enum_variants.get(&u.name) {
                    for v in variants {
                        for (_, fty) in &v.fields {
                            if let Some(b) =
                                self.uncloneable_type_name(fty, depth + 1)
                            {
                                return Some(b);
                            }
                        }
                    }
                    return None;
                }
                let (_, info) = self.struct_decl_for_type(ty)?;
                for (_, fty) in &info.fields {
                    if let Some(b) = self.uncloneable_type_name(fty, depth + 1) {
                        return Some(b);
                    }
                }
                None
            }
            _ => None,
        }
    }
}

// -- name-keyed container arms --------------------------------------

impl Checker {
    fn leaf_push(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 2)?;
        let recv_ty = self.check_expr(&call.args[0])?;
        let elem = vec_like_elem(&recv_ty).ok_or_else(type_mismatch)?;
        let arg_ty = self.check_expr(&call.args[1])?;
        if !types_equal(&elem, &arg_ty) {
            return Err(type_mismatch());
        }
        self.consume_push_style_arg(
            &call.args[0],
            &call.args[1],
            &arg_ty,
            "push element argument",
        )?;
        if matches!(elem, Type::Infer) {
            let concrete = if ud_inner(&recv_ty, "GrowVec").is_some() {
                user_ty("GrowVec", vec![arg_ty])
            } else {
                user_ty("Vec", vec![arg_ty])
            };
            self.concretize_receiver(&call.args[0], concrete);
        }
        Ok(void_ty())
    }

    fn leaf_pop(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 1)?;
        let recv_ty = self.check_expr(&call.args[0])?;
        let elem = vec_element_type(&recv_ty).ok_or_else(type_mismatch)?;
        if matches!(elem, Type::Infer) {
            return Err(type_mismatch());
        }
        Ok(user_ty("Option", vec![elem]))
    }

    fn leaf_remove(&mut self, call: &CallExpr) -> CheckResult<Type> {
        arity(call, 2)?;
        let recv_ty = self.check_expr(&call.args[0])?;
        let elem = vec_element_type(&recv_ty).ok_or_else(type_mismatch)?;
        let index_ty = self.check_expr(&call.args[1])?;
        if !is_numeric_type(&index_ty) || matches!(elem, Type::Infer) {
            return Err(type_mismatch());
        }
        Ok(elem)
    }

    // -- receiver-shape segments (Zig order) -------------------------

    fn seg_option(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let inner = option_inner_type(recv_ty)?;
        let name = call.func_name.as_str();
        Some(match name {
            "is_some" | "is_none" | "is_ok" | "is_err" => {
                arity(call, 1).map(|_| bool_ty())
            }
            "copied" => (|| {
                arity(call, 1)?;
                match &inner {
                    Type::Borrow(b) => {
                        Ok(user_ty("Option", vec![(**b).clone()]))
                    }
                    _ => Err(type_mismatch()),
                }
            })(),
            "map" => (|| {
                arity(call, 2)?;
                let (params, ret) =
                    self.check_closure_arg(&call.args[1], &[inner.clone()])?;
                Self::closure_gate(&params, 1, Some(&inner))?;
                Ok(user_ty("Option", vec![ret]))
            })(),
            "and_then" => (|| {
                arity(call, 2)?;
                let (params, ret) =
                    self.check_closure_arg(&call.args[1], &[inner.clone()])?;
                Self::closure_gate(&params, 1, Some(&inner))?;
                if option_inner_type(&ret).is_none() {
                    return Err(type_mismatch());
                }
                Ok(ret)
            })(),
            "unwrap" => arity(call, 1).map(|_| inner.clone()),
            "unwrap_or" => (|| {
                arity(call, 2)?;
                let default_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&inner, &default_ty) {
                    return Err(type_mismatch());
                }
                Ok(inner.clone())
            })(),
            "unwrap_or_else" => (|| {
                arity(call, 2)?;
                let (params, ret) = self.check_closure_arg(&call.args[1], &[])?;
                Self::closure_gate(&params, 0, None)?;
                if !types_equal(&inner, &ret) {
                    return Err(type_mismatch());
                }
                Ok(inner.clone())
            })(),
            "unwrap_or_default" => arity(call, 1).map(|_| inner.clone()),
            _ => return None,
        })
    }

    fn seg_result(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let ok = result_ok_type(recv_ty)?;
        let name = call.func_name.as_str();
        Some(match name {
            "is_ok" | "is_err" => arity(call, 1).map(|_| bool_ty()),
            "map" => (|| {
                arity(call, 2)?;
                let err = result_err_type(recv_ty).ok_or_else(type_mismatch)?;
                let (params, ret) =
                    self.check_closure_arg(&call.args[1], &[ok.clone()])?;
                Self::closure_gate(&params, 1, Some(&ok))?;
                Ok(user_ty("Result", vec![ret, err]))
            })(),
            "unwrap" => arity(call, 1).map(|_| ok.clone()),
            "unwrap_or" => (|| {
                arity(call, 2)?;
                let default_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&ok, &default_ty) {
                    return Err(type_mismatch());
                }
                Ok(ok.clone())
            })(),
            _ => return None,
        })
    }

    fn seg_atomic_i32(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        if ud_name(recv_ty) != Some("AtomicI32") {
            return None;
        }
        self.leaf_atomic(call, i32_ty(), true)
    }

    fn seg_atomic_usize(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        if ud_name(recv_ty) != Some("AtomicUsize") {
            return None;
        }
        self.leaf_atomic(call, prim(Primitive::Usize), false)
    }

    pub(crate) fn check_ordering_arg(
        &mut self,
        call: &CallExpr,
        idx: usize,
    ) -> CheckResult<()> {
        let o_ty = self.check_expr(&call.args[idx])?;
        if !is_ordering(&o_ty) {
            return Err(type_mismatch());
        }
        Ok(())
    }

    fn leaf_atomic(
        &mut self,
        call: &CallExpr,
        int_ty: Type,
        with_exchange: bool,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        Some(match name {
            "load" => (|| {
                arity(call, 2)?;
                self.check_ordering_arg(call, 1)?;
                Ok(int_ty)
            })(),
            "store" => (|| {
                arity(call, 3)?;
                let v_ty = self.check_expr(&call.args[1])?;
                if !is_numeric_type(&v_ty) {
                    return Err(type_mismatch());
                }
                self.check_ordering_arg(call, 2)?;
                Ok(void_ty())
            })(),
            "fetch_add" => (|| {
                arity(call, 3)?;
                let v_ty = self.check_expr(&call.args[1])?;
                if !is_numeric_type(&v_ty) {
                    return Err(type_mismatch());
                }
                self.check_ordering_arg(call, 2)?;
                Ok(int_ty)
            })(),
            "compare_exchange" if with_exchange => (|| {
                arity(call, 5)?;
                for idx in [1usize, 2] {
                    let t = self.check_expr(&call.args[idx])?;
                    if !is_numeric_type(&t) {
                        return Err(type_mismatch());
                    }
                }
                self.check_ordering_arg(call, 3)?;
                self.check_ordering_arg(call, 4)?;
                Ok(user_ty("Result", vec![i32_ty(), i32_ty()]))
            })(),
            _ => return None,
        })
    }

    fn seg_cell(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let inner = ud_inner(recv_ty, "Cell")?;
        Some(match call.func_name.as_str() {
            "get" => arity(call, 1).map(|_| inner.clone()),
            "set" => (|| {
                arity(call, 2)?;
                let value_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&inner, &value_ty) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "Cell::set type mismatch: cell tag={}, value tag={}",
                            type_tag_name(&inner),
                            type_tag_name(&value_ty)
                        ),
                    ));
                }
                Ok(void_ty())
            })(),
            _ => return None,
        })
    }

    fn seg_refcell(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let inner = ud_inner(recv_ty, "RefCell")?;
        match call.func_name.as_str() {
            "borrow" | "borrow_mut" => {
                Some(arity(call, 1).map(|_| Type::Borrow(Box::new(inner))))
            }
            _ => None,
        }
    }

    fn seg_mutex(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let inner = ud_inner(recv_ty, "Mutex")?;
        match call.func_name.as_str() {
            "lock" => Some(
                arity(call, 1).map(|_| {
                    user_ty(
                        "Result",
                        vec![user_ty("MutexGuard", vec![inner]), i32_ty()],
                    )
                }),
            ),
            _ => None,
        }
    }

    fn seg_rwlock(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let inner = ud_inner(recv_ty, "RwLock")?;
        let guard = match call.func_name.as_str() {
            "read" => "RwLockReadGuard",
            "write" => "RwLockWriteGuard",
            _ => return None,
        };
        Some(
            arity(call, 1).map(|_| {
                user_ty("Result", vec![user_ty(guard, vec![inner]), i32_ty()])
            }),
        )
    }
}

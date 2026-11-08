//! Second half of the receiver-shape std leaf cascade (R-1
//! port; see check_receiver_leaves.rs for the head, placement
//! and residual register): File / Metadata / handle / clone /
//! counts / as_ptr / VecDeque / Array / String / Slice /
//! pointer / map / set segments and the iterable arms.

use crate::check_leaves::{type_mismatch, ud_inner, ud_name, user_ty};
use crate::check_receiver_leaves::{
    arity, btree_key_supported, iterable_elem, ud2, unwrapped, vec_like_elem,
};
use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    array_type_of, bool_ty, box_inner_type, i32_ty, is_integer_type,
    is_numeric_type, is_string_like_type, is_string_type, prim,
    rc_inner_type, slice_element_type,
    type_desc, types_equal, u64_ty, vec_element_type, void_ty,
};
use sla_ast::exprs::CallExpr;
use sla_ast::types::{Primitive, Type};

impl Checker {
    pub(crate) fn seg_file_meta(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        if ud_name(recv_ty) == Some("File") && name == "as_raw_fd" {
            return Some(arity(call, 1).map(|_| i32_ty()));
        }
        if ud_name(recv_ty) == Some("Metadata") {
            return Some(match name {
                "is_file" | "is_dir" | "is_symlink" => {
                    arity(call, 1).map(|_| bool_ty())
                }
                "modified_ms" | "created_ms" => {
                    arity(call, 1).map(|_| prim(Primitive::I64))
                }
                _ => return None,
            });
        }
        // The `metadata()` call arm: any receiver, gated on the
        // receiver being string-like (Zig checks the name first,
        // so a wrong arity is a hard error even here).
        if name == "metadata" {
            if call.args.len() != 1 {
                return Some(Err(crate::check_leaves::invalid_args()));
            }
            if is_string_like_type(recv_ty) {
                return Some(Ok(user_ty(
                    "Result",
                    vec![user_ty("Metadata", Vec::new()), i32_ty()],
                )));
            }
        }
        None
    }

    pub(crate) fn seg_handle_chan(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        if let Some(inner) = ud_inner(recv_ty, "JoinHandle") {
            if name == "join" {
                return Some(
                    arity(call, 1)
                        .map(|_| user_ty("Result", vec![inner, i32_ty()])),
                );
            }
            return None;
        }
        if let Some(inner) = ud_inner(recv_ty, "Sender") {
            return Some(match name {
                "clone" => arity(call, 1).map(|_| unwrapped(recv_ty)),
                "send" => (|| {
                    arity(call, 2)?;
                    let value_ty = self.check_expr(&call.args[1])?;
                    if !types_equal(&inner, &value_ty) {
                        return Err(type_mismatch());
                    }
                    Ok(user_ty("Result", vec![void_ty(), i32_ty()]))
                })(),
                _ => return None,
            });
        }
        if let Some(inner) = ud_inner(recv_ty, "Receiver") {
            if name == "recv" {
                return Some(
                    arity(call, 1)
                        .map(|_| user_ty("Result", vec![inner, i32_ty()])),
                );
            }
        }
        // R-3 goroutine batch: the `Chan<T>` handle itself (the
        // goroutine syntax batch's channel surface). `send`
        // yields the status word, `recv` yields the payload
        // (select arms destructure it as `(value, closed)` at
        // the pattern level), `destroy` releases the handle.
        if let Some(inner) = ud_inner(recv_ty, "Chan") {
            return Some(match name {
                "send" => (|| {
                    arity(call, 2)?;
                    let value_ty = self.check_expr(&call.args[1])?;
                    if !types_equal(&inner, &value_ty) {
                        return Err(type_mismatch());
                    }
                    Ok(i32_ty())
                })(),
                // `recv` yields the (payload, closed) pair:
                // call sites destructure `let (v, closed) =
                // ch.recv();` (the select pattern form binds
                // the same pair at the pattern level).
                "recv" => arity(call, 1).map(|_| {
                    Type::Tuple(sla_ast::types::TupleType {
                        elems: vec![inner.clone(), crate::types_util::bool_ty()],
                    })
                }),
                "destroy" => arity(call, 1).map(|_| void_ty()),
                _ => return None,
            });
        }
        None
    }

    pub(crate) fn seg_clone_counts_asptr(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        match name {
            "clone" => {
                if rc_inner_type(recv_ty).is_some()
                    || ud_inner(recv_ty, "Arc").is_some()
                {
                    return Some(arity(call, 1).map(|_| unwrapped(recv_ty)));
                }
                // DEF-deep-copy clone surface: only when the
                // type has no user `clone` method (which the
                // receiver-inferred stage would resolve).
                let type_name =
                    crate::types_util::concrete_type_name(recv_ty)?;
                let method_key = format!("{type_name}_clone");
                if self.funcs.contains_key(&method_key)
                    || self.imported_funcs.contains_key(&method_key)
                {
                    return None;
                }
                if is_string_type(recv_ty) {
                    return Some(arity(call, 1).map(|_| unwrapped(recv_ty)));
                }
                for container in ["Vec", "GrowVec"] {
                    if let Some(elem) = if container == "Vec" {
                        vec_element_type(recv_ty)
                    } else {
                        ud_inner(recv_ty, "GrowVec")
                    } {
                        return Some((|| {
                            arity(call, 1)?;
                            if let Some(bad) =
                                self.uncloneable_type_name(&elem, 0)
                            {
                                return Err(Diagnostic::marked(
                                    ErrorKind::CompileError,
                                    "UncloneableClone",
                                    format!(
                                        "UncloneableClone: {container} elements of type `{}` cannot be cloned: their closure contains {bad}, which has no clone",
                                        type_desc(&elem)
                                    ),
                                ));
                            }
                            Ok(unwrapped(recv_ty))
                        })());
                    }
                }
                if let Some((_, info)) = self.struct_decl_for_type(recv_ty) {
                    return Some((|| {
                        arity(call, 1)?;
                        for (field_name, field_ty) in &info.fields {
                            if let Some(bad) =
                                self.uncloneable_type_name(field_ty, 0)
                            {
                                return Err(Diagnostic::marked(
                                    ErrorKind::CompileError,
                                    "UncloneableClone",
                                    format!(
                                        "UncloneableClone: struct `{}` cannot be cloned: field `{field_name}` reaches {bad}, which has no clone",
                                        type_desc(recv_ty)
                                    ),
                                ));
                            }
                        }
                        Ok(unwrapped(recv_ty))
                    })());
                }
                None
            }
            "strong_count" | "weak_count" => {
                if rc_inner_type(recv_ty).is_some()
                    || ud_inner(recv_ty, "Arc").is_some()
                {
                    return Some(arity(call, 1).map(|_| u64_ty()));
                }
                None
            }
            "as_ptr" => {
                let inner = box_inner_type(recv_ty)
                    .or_else(|| rc_inner_type(recv_ty))
                    .or_else(|| ud_inner(recv_ty, "Arc"));
                inner.map(|inner| {
                    arity(call, 1)
                        .map(|_| Type::Pointer(Box::new(inner)))
                })
            }
            _ => None,
        }
    }

    pub(crate) fn seg_atomic_ptr(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let inner = ud_inner(recv_ty, "AtomicPtr")?;
        if call.func_name != "load" {
            return None;
        }
        Some((|| {
            arity(call, 2)?;
            self.check_ordering_arg(call, 1)?;
            Ok(Type::Pointer(Box::new(inner.clone())))
        })())
    }

    pub(crate) fn seg_vecdeque(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let elem = ud_inner(recv_ty, "VecDeque")?;
        Some(match call.func_name.as_str() {
            "rotate_left" => (|| {
                arity(call, 2)?;
                let count_ty = self.check_expr(&call.args[1])?;
                if !is_numeric_type(&count_ty) {
                    return Err(type_mismatch());
                }
                Ok(void_ty())
            })(),
            "push_back" => (|| {
                arity(call, 2)?;
                let value_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&elem, &value_ty) {
                    return Err(type_mismatch());
                }
                self.consume_push_style_arg(
                    &call.args[0],
                    &call.args[1],
                    &value_ty,
                    "push_back element argument",
                )?;
                if matches!(elem, Type::Infer) {
                    self.concretize_receiver(
                        &call.args[0],
                        user_ty("VecDeque", vec![value_ty]),
                    );
                }
                Ok(void_ty())
            })(),
            "pop_front" => (|| {
                arity(call, 1)?;
                if matches!(elem, Type::Infer) {
                    return Err(type_mismatch());
                }
                Ok(user_ty("Option", vec![elem.clone()]))
            })(),
            _ => return None,
        })
    }

    pub(crate) fn seg_array(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let (elem, _) = array_type_of(recv_ty)?;
        Some(match call.func_name.as_str() {
            "as_ptr" => {
                arity(call, 1).map(|_| Type::Pointer(Box::new(elem)))
            }
            "fill" => (|| {
                arity(call, 2)?;
                let value_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&elem, &value_ty) {
                    return Err(type_mismatch());
                }
                Ok(void_ty())
            })(),
            _ => return None,
        })
    }

    pub(crate) fn seg_string_slice_ptr(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        if is_string_like_type(recv_ty) {
            match name {
                "as_ptr" => {
                    return Some(
                        arity(call, 1).map(|_| {
                            Type::Pointer(Box::new(prim(Primitive::U8)))
                        }),
                    )
                }
                "as_bytes" | "bytes" => {
                    return Some(
                        arity(call, 1).map(|_| {
                            user_ty("Slice", vec![prim(Primitive::U8)])
                        }),
                    )
                }
                "try_exists" | "is_file" | "is_dir" | "is_symlink" => {
                    return Some(arity(call, 1).map(|_| bool_ty()))
                }
                _ => {}
            }
        }
        if let Some(elem) = slice_element_type(recv_ty) {
            if name == "as_ptr" {
                return Some(
                    arity(call, 1).map(|_| Type::Pointer(Box::new(elem))),
                );
            }
        }
        // Zig checks the UNPEELED receiver tag for the pointer
        // `add` leaf (`recv_ty.* == .pointer`); peeling first
        // would consume the very layer being tested.
        if let Type::Pointer(_) = recv_ty {
            if name == "add" {
                return Some((|| {
                    arity(call, 2)?;
                    let index_ty = self.check_expr(&call.args[1])?;
                    if !is_integer_type(&index_ty) {
                        return Err(type_mismatch());
                    }
                    Ok(recv_ty.clone())
                })());
            }
        }
        None
    }

    pub(crate) fn seg_maps_sets(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        if let Some((key, value)) = ud2(recv_ty, "HashMap") {
            return self.leaf_map(call, key, value, false, "HashMap");
        }
        if let Some((key, value)) = ud2(recv_ty, "BTreeMap") {
            return self.leaf_map(call, key, value, true, "BTreeMap");
        }
        if let Some(key) = ud_inner(recv_ty, "HashSet") {
            return self.leaf_set(call, key, false, "HashSet");
        }
        if let Some(key) = ud_inner(recv_ty, "BTreeSet") {
            return self.leaf_set(call, key, true, "BTreeSet");
        }
        None
    }

    fn leaf_map(
        &mut self,
        call: &CallExpr,
        key: Type,
        value: Type,
        btree: bool,
        type_name: &str,
    ) -> Option<CheckResult<Type>> {
        Some(match call.func_name.as_str() {
            "insert" => (|| {
                arity(call, 3)?;
                let key_ty = self.check_expr(&call.args[1])?;
                let value_ty = self.check_expr(&call.args[2])?;
                if !types_equal(&key, &key_ty)
                    || !types_equal(&value, &value_ty)
                {
                    return Err(type_mismatch());
                }
                if btree && !btree_key_supported(&key_ty) {
                    return Err(type_mismatch());
                }
                if matches!(key, Type::Infer) || matches!(value, Type::Infer)
                {
                    self.concretize_receiver(
                        &call.args[0],
                        user_ty(type_name, vec![key_ty.clone(), value_ty.clone()]),
                    );
                }
                self.consume_insert_arg(&call.args[0], &call.args[1], &key_ty)?;
                self.consume_insert_arg(
                    &call.args[0],
                    &call.args[2],
                    &value_ty,
                )?;
                Ok(user_ty("Option", vec![value_ty]))
            })(),
            "get" => (|| {
                arity(call, 2)?;
                let key_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&key, &key_ty) {
                    return Err(type_mismatch());
                }
                if btree && !btree_key_supported(&key_ty) {
                    return Err(type_mismatch());
                }
                Ok(user_ty(
                    "Option",
                    vec![Type::Borrow(Box::new(value.clone()))],
                ))
            })(),
            "clear" => arity(call, 1).map(|_| void_ty()),
            _ => return None,
        })
    }

    fn leaf_set(
        &mut self,
        call: &CallExpr,
        key: Type,
        btree: bool,
        type_name: &str,
    ) -> Option<CheckResult<Type>> {
        Some(match call.func_name.as_str() {
            "insert" => (|| {
                arity(call, 2)?;
                let key_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&key, &key_ty) {
                    return Err(type_mismatch());
                }
                if btree && !btree_key_supported(&key_ty) {
                    return Err(type_mismatch());
                }
                if matches!(key, Type::Infer) {
                    self.concretize_receiver(
                        &call.args[0],
                        user_ty(type_name, vec![key_ty.clone()]),
                    );
                }
                self.consume_insert_arg(&call.args[0], &call.args[1], &key_ty)?;
                Ok(bool_ty())
            })(),
            "contains" => (|| {
                arity(call, 2)?;
                let key_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&key, &key_ty) {
                    return Err(type_mismatch());
                }
                if btree && !btree_key_supported(&key_ty) {
                    return Err(type_mismatch());
                }
                Ok(bool_ty())
            })(),
            _ => return None,
        })
    }

    // -- iterable arms (shape-gated; see the head file) ---------------

    pub(crate) fn leaf_iterable(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        match name {
            "iter" | "into_iter" => {
                iterable_elem(recv_ty)?;
                Some(arity(call, 1).map(|_| recv_ty.clone()))
            }
            "copied" => {
                iterable_elem(recv_ty)?;
                Some(arity(call, 1).map(|_| recv_ty.clone()))
            }
            "collect" => {
                let elem = iterable_elem(recv_ty)?;
                Some((|| {
                    if call.args.len() != 1 || call.generics.len() != 1 {
                        return Err(crate::check_leaves::invalid_args());
                    }
                    if !is_string_type(&call.generics[0]) {
                        return Err(type_mismatch());
                    }
                    if !matches!(elem, Type::Primitive(Primitive::U8)) {
                        return Err(type_mismatch());
                    }
                    Ok(user_ty("String", Vec::new()))
                })())
            }
            "join" => {
                let elem = iterable_elem(recv_ty)?;
                Some((|| {
                    arity(call, 2)?;
                    if !is_string_like_type(&elem) && !is_string_type(&elem) {
                        return Err(type_mismatch());
                    }
                    let sep_ty = self.check_expr(&call.args[1])?;
                    if !is_string_like_type(&sep_ty)
                        && !is_string_type(&sep_ty)
                    {
                        return Err(type_mismatch());
                    }
                    Ok(user_ty("String", Vec::new()))
                })())
            }
            "map" | "filter" | "fold" => {
                // Zig derives the element from the array shape or
                // the Vec-like shapes (slices excluded here).
                let (elem, arr_len) = match array_type_of(recv_ty) {
                    Some((e, len)) => (e, Some(len)),
                    None => (vec_like_elem(recv_ty)?, None),
                };
                Some(match name {
                    "map" => (|| {
                        arity(call, 2)?;
                        let (params, ret) = self
                            .check_closure_arg(&call.args[1], &[elem.clone()])?;
                        Self::closure_gate(&params, 1, Some(&elem))?;
                        Ok(match arr_len {
                            Some(len) => Type::Array(
                                sla_ast::types::ArrayType {
                                    elem: Box::new(ret),
                                    len,
                                },
                            ),
                            None => user_ty("Vec", vec![ret]),
                        })
                    })(),
                    "filter" => (|| {
                        arity(call, 2)?;
                        let (params, ret) = self
                            .check_closure_arg(&call.args[1], &[elem.clone()])?;
                        Self::closure_gate(&params, 1, Some(&elem))?;
                        if !matches!(ret, Type::Primitive(Primitive::Boolean))
                        {
                            return Err(type_mismatch());
                        }
                        Ok(recv_ty.clone())
                    })(),
                    _ => (|| {
                        arity(call, 3)?;
                        let init_ty = self.check_expr(&call.args[1])?;
                        let (params, ret) = self.check_closure_arg(
                            &call.args[2],
                            &[init_ty.clone(), elem.clone()],
                        )?;
                        if params.len() != 2 {
                            return Err(crate::check_leaves::invalid_args());
                        }
                        if !types_equal(&params[0], &init_ty)
                            || !types_equal(&params[1], &elem)
                            || !types_equal(&ret, &params[0])
                        {
                            return Err(type_mismatch());
                        }
                        Ok(init_ty)
                    })(),
                })
            }
            _ => None,
        }
    }
}

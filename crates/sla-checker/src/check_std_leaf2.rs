//! Std leaf cascade, second half — the atomic arms, the
//! VecDeque arms, the string-like query arms, the map/set arms
//! and the name-keyed container arms (`push` / `pop` / `remove`
//! / iterator combinators) of the Zig call arm
//! (type_checker.zig, venue :9606-10460). Split from
//! check_std_leaf.rs for the 700-line file cap; the two halves
//! dispatch on disjoint (shape, name) pairs except where noted,
//! and run in sequence as the last stage of
//! `check_receiver_call`. See check_std_leaf.rs for the port's
//! scope notes and registered residuals.

use crate::check_std_ctor::{option_ty, result_ty, string_ty, user_ty};
use crate::checker::{make_borrow_type, Checker};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    array_type_of, bool_ty, btree_map_types, btree_set_element_type,
    grow_vec_element_type, hash_map_types, hash_set_element_type,
    i32_ty, is_atomic_i32_type, is_atomic_usize_type,
    is_numeric_type, is_ordering_type, is_string_like_type,
    is_string_type, iterable_element_type, slice_element_type,
    type_desc, types_equal, vec_deque_element_type,
    vec_element_type, vec_like_element_type, void_ty,
};
use sla_ast::exprs::CallExpr;
use sla_ast::types::{Primitive, Type};

fn arity(name: &str, want: usize, got: usize) -> Diagnostic {
    Diagnostic::new(
        ErrorKind::InvalidArgsCount,
        format!("InvalidArgsCount: `{name}` expects {want} arguments, got {got}"),
    )
}

fn mismatch(msg: String) -> Diagnostic {
    Diagnostic::new(ErrorKind::TypeMismatch, msg)
}

/// Zig `btreeSetKeySupported`: only raw pointers and slices are
/// supported BTree keys in this snapshot.
fn btree_key_supported(ty: &Type) -> bool {
    if matches!(ty, Type::Primitive(Primitive::RawPtr)) {
        return true;
    }
    slice_element_type(ty).is_some()
}

impl Checker {
    /// Second half of the std receiver leaf cascade. Returns
    /// `None` when no leaf matches.
    pub fn check_std_receiver_leaf2(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        let argc = call.args.len();
        let recv = &call.args[0];

        // -- AtomicI32 / AtomicUsize ------------------------------
        if is_atomic_i32_type(recv_ty) || is_atomic_usize_type(recv_ty) {
            let scalar = if is_atomic_i32_type(recv_ty) {
                i32_ty()
            } else {
                Type::Primitive(Primitive::Usize)
            };
            match name {
                "load" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let ordering_ty = self.check_expr(&call.args[1])?;
                        if !is_ordering_type(&ordering_ty) {
                            return Err(mismatch(
                                "TypeMismatch: atomic load expects an Ordering".into(),
                            ));
                        }
                        Ok(scalar)
                    })());
                }
                "store" => {
                    return Some((|| {
                        if argc != 3 {
                            return Err(arity(name, 3, argc));
                        }
                        let value_ty = self.check_expr(&call.args[1])?;
                        let ordering_ty = self.check_expr(&call.args[2])?;
                        if !is_numeric_type(&value_ty) || !is_ordering_type(&ordering_ty) {
                            return Err(mismatch(
                                "TypeMismatch: atomic store expects a numeric value and an Ordering".into(),
                            ));
                        }
                        Ok(void_ty())
                    })());
                }
                "fetch_add" => {
                    return Some((|| {
                        if argc != 3 {
                            return Err(arity(name, 3, argc));
                        }
                        let value_ty = self.check_expr(&call.args[1])?;
                        let ordering_ty = self.check_expr(&call.args[2])?;
                        if !is_numeric_type(&value_ty) || !is_ordering_type(&ordering_ty) {
                            return Err(mismatch(
                                "TypeMismatch: atomic fetch_add expects a numeric value and an Ordering".into(),
                            ));
                        }
                        Ok(scalar)
                    })());
                }
                "compare_exchange" if is_atomic_i32_type(recv_ty) => {
                    return Some((|| {
                        if argc != 5 {
                            return Err(arity(name, 5, argc));
                        }
                        let expected_ty = self.check_expr(&call.args[1])?;
                        let new_ty = self.check_expr(&call.args[2])?;
                        let success_ty = self.check_expr(&call.args[3])?;
                        let failure_ty = self.check_expr(&call.args[4])?;
                        if !is_numeric_type(&expected_ty)
                            || !is_numeric_type(&new_ty)
                            || !is_ordering_type(&success_ty)
                            || !is_ordering_type(&failure_ty)
                        {
                            return Err(mismatch(
                                "TypeMismatch: compare_exchange expects numeric values and Orderings".into(),
                            ));
                        }
                        Ok(result_ty(i32_ty(), i32_ty()))
                    })());
                }
                _ => {}
            }
        }

        // -- VecDeque ------------------------------------------------
        if let Some(elem) = vec_deque_element_type(recv_ty) {
            match name {
                "rotate_left" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let count_ty = self.check_expr(&call.args[1])?;
                        if !is_numeric_type(&count_ty) {
                            return Err(mismatch(
                                "TypeMismatch: rotate_left expects a numeric count".into(),
                            ));
                        }
                        Ok(void_ty())
                    })());
                }
                "push_back" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let value_ty = self.check_expr(&call.args[1])?;
                        if !types_equal(&elem, &value_ty) {
                            return Err(mismatch(format!(
                                "TypeMismatch: push_back expects {}, actual {}",
                                type_desc(&elem),
                                type_desc(&value_ty)
                            )));
                        }
                        self.consume_push_arg(
                            recv,
                            &call.args[1],
                            &value_ty,
                            "push_back element argument",
                        )?;
                        if matches!(elem, Type::Infer) {
                            self.refine_receiver_binding(
                                recv,
                                &user_ty("VecDeque", vec![value_ty]),
                            );
                        }
                        Ok(void_ty())
                    })());
                }
                "pop_front" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        if matches!(elem, Type::Infer) {
                            return Err(mismatch(
                                "TypeMismatch: pop_front on a VecDeque of unknown element type".into(),
                            ));
                        }
                        Ok(option_ty(elem))
                    })());
                }
                _ => {}
            }
        }

        // -- string-like queries --------------------------------------
        if is_string_like_type(recv_ty) {
            match name {
                "as_bytes" | "bytes" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(user_ty(
                            "Slice",
                            vec![Type::Primitive(Primitive::U8)],
                        ))
                    })());
                }
                "try_exists" | "is_file" | "is_dir" | "is_symlink" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(bool_ty())
                    })());
                }
                _ => {}
            }
        }

        // -- map/set arms (check_std_leaf3.rs) --------------------
        if let Some(r) = self.check_std_map_leaf(call, recv_ty) {
            return Some(r);
        }

        // -- name-keyed: push / pop / remove ----------------------------
        // These arms are keyed on the NAME in Zig and hard-fail
        // with TypeMismatch when the receiver shape does not fit.
        if name == "push" {
            return Some((|| {
                if argc != 2 {
                    return Err(arity(name, 2, argc));
                }
                let elem = vec_like_element_type(recv_ty).ok_or_else(|| {
                    mismatch(format!(
                        "TypeMismatch: push receiver is not a Vec-like, actual {}",
                        type_desc(recv_ty)
                    ))
                })?;
                let arg_ty = self.check_expr(&call.args[1])?;
                if !types_equal(&elem, &arg_ty) {
                    return Err(mismatch(format!(
                        "TypeMismatch: push expects {}, actual {}",
                        type_desc(&elem),
                        type_desc(&arg_ty)
                    )));
                }
                self.consume_push_arg(
                    recv,
                    &call.args[1],
                    &arg_ty,
                    "push element argument",
                )?;
                if matches!(elem, Type::Infer) {
                    let concrete = if grow_vec_element_type(recv_ty).is_some() {
                        user_ty("GrowVec", vec![arg_ty])
                    } else {
                        user_ty("Vec", vec![arg_ty])
                    };
                    self.refine_receiver_binding(recv, &concrete);
                }
                Ok(void_ty())
            })());
        }
        if name == "pop" {
            return Some((|| {
                if argc != 1 {
                    return Err(arity(name, 1, argc));
                }
                let elem = vec_element_type(recv_ty).ok_or_else(|| {
                    mismatch(format!(
                        "TypeMismatch: pop receiver is not a Vec, actual {}",
                        type_desc(recv_ty)
                    ))
                })?;
                if matches!(elem, Type::Infer) {
                    return Err(mismatch(
                        "TypeMismatch: pop on a Vec of unknown element type".into(),
                    ));
                }
                Ok(option_ty(elem))
            })());
        }
        if name == "remove" {
            return Some((|| {
                if argc != 2 {
                    return Err(arity(name, 2, argc));
                }
                let elem = vec_element_type(recv_ty).ok_or_else(|| {
                    mismatch(format!(
                        "TypeMismatch: remove receiver is not a Vec, actual {}",
                        type_desc(recv_ty)
                    ))
                })?;
                let index_ty = self.check_expr(&call.args[1])?;
                if !is_numeric_type(&index_ty) {
                    return Err(mismatch(
                        "TypeMismatch: remove expects a numeric index".into(),
                    ));
                }
                if matches!(elem, Type::Infer) {
                    return Err(mismatch(
                        "TypeMismatch: remove on a Vec of unknown element type".into(),
                    ));
                }
                Ok(elem)
            })());
        }

        // -- name-keyed: iterator surface --------------------------------
        if name == "iter" || name == "into_iter" || name == "copied" {
            let fits = array_type_of(recv_ty).is_some()
                || vec_element_type(recv_ty).is_some()
                || grow_vec_element_type(recv_ty).is_some()
                || slice_element_type(recv_ty).is_some();
            if fits {
                return Some((|| {
                    if argc != 1 {
                        return Err(arity(name, 1, argc));
                    }
                    Ok(recv_ty.clone())
                })());
            }
            // Zig hard-fails these names on non-iterable receivers
            // only after its own receiver resolution missed; the
            // shape gate above is the arm's condition, so a miss
            // here falls through to the remaining name arms.
        }
        if name == "collect" {
            return Some((|| {
                if argc != 1 || call.generics.len() != 1 {
                    return Err(arity(name, 1, argc));
                }
                if !is_string_type(&call.generics[0]) {
                    return Err(mismatch(
                        "TypeMismatch: collect target must be String".into(),
                    ));
                }
                let elem = iterable_element_type(recv_ty).ok_or_else(|| {
                    mismatch(format!(
                        "TypeMismatch: collect receiver is not iterable, actual {}",
                        type_desc(recv_ty)
                    ))
                })?;
                if !matches!(elem, Type::Primitive(Primitive::U8)) {
                    return Err(mismatch(
                        "TypeMismatch: collect to String expects u8 elements".into(),
                    ));
                }
                Ok(string_ty())
            })());
        }
        if name == "join" {
            return Some((|| {
                if argc != 2 {
                    return Err(arity(name, 2, argc));
                }
                let elem = iterable_element_type(recv_ty).ok_or_else(|| {
                    mismatch(format!(
                        "TypeMismatch: join receiver is not iterable, actual {}",
                        type_desc(recv_ty)
                    ))
                })?;
                if !is_string_like_type(&elem) && !is_string_type(&elem) {
                    return Err(mismatch(
                        "TypeMismatch: join expects string-like elements".into(),
                    ));
                }
                let sep_ty = self.check_expr(&call.args[1])?;
                if !is_string_like_type(&sep_ty) && !is_string_type(&sep_ty) {
                    return Err(mismatch(
                        "TypeMismatch: join separator must be string-like".into(),
                    ));
                }
                Ok(string_ty())
            })());
        }
        if name == "map" {
            return Some((|| {
                if argc != 2 {
                    return Err(arity(name, 2, argc));
                }
                let elem = match array_type_of(recv_ty) {
                    Some((elem, _)) => elem,
                    None => vec_like_element_type(recv_ty).ok_or_else(|| {
                        mismatch(format!(
                            "TypeMismatch: map receiver is not iterable, actual {}",
                            type_desc(recv_ty)
                        ))
                    })?,
                };
                let (params, ret) = self.check_std_closure_arg(&call.args[1])?;
                if params.len() != 1 {
                    return Err(arity(name, 1, params.len()));
                }
                if !types_equal(&params[0], &elem) {
                    return Err(mismatch(format!(
                        "TypeMismatch: map closure parameter expects {}, actual {}",
                        type_desc(&elem),
                        type_desc(&params[0])
                    )));
                }
                if let Some((_, len)) = array_type_of(recv_ty) {
                    return Ok(Type::Array(sla_ast::types::ArrayType {
                        elem: Box::new(ret),
                        len,
                    }));
                }
                Ok(user_ty("Vec", vec![ret]))
            })());
        }
        if name == "filter" {
            return Some((|| {
                if argc != 2 {
                    return Err(arity(name, 2, argc));
                }
                let elem = match array_type_of(recv_ty) {
                    Some((elem, _)) => elem,
                    None => vec_like_element_type(recv_ty).ok_or_else(|| {
                        mismatch(format!(
                            "TypeMismatch: filter receiver is not iterable, actual {}",
                            type_desc(recv_ty)
                        ))
                    })?,
                };
                let (params, ret) = self.check_std_closure_arg(&call.args[1])?;
                if params.len() != 1 {
                    return Err(arity(name, 1, params.len()));
                }
                if !types_equal(&params[0], &elem) {
                    return Err(mismatch(format!(
                        "TypeMismatch: filter closure parameter expects {}, actual {}",
                        type_desc(&elem),
                        type_desc(&params[0])
                    )));
                }
                if !matches!(ret, Type::Primitive(Primitive::Boolean)) {
                    return Err(mismatch(
                        "TypeMismatch: filter closure must return bool".into(),
                    ));
                }
                Ok(recv_ty.clone())
            })());
        }
        if name == "fold" {
            return Some((|| {
                if argc != 3 {
                    return Err(arity(name, 3, argc));
                }
                let elem = match array_type_of(recv_ty) {
                    Some((elem, _)) => elem,
                    None => vec_like_element_type(recv_ty).ok_or_else(|| {
                        mismatch(format!(
                            "TypeMismatch: fold receiver is not iterable, actual {}",
                            type_desc(recv_ty)
                        ))
                    })?,
                };
                let init_ty = self.check_expr(&call.args[1])?;
                let (params, ret) = self.check_std_closure_arg(&call.args[2])?;
                if params.len() != 2 {
                    return Err(arity(name, 2, params.len()));
                }
                if !types_equal(&params[0], &init_ty) {
                    return Err(mismatch(format!(
                        "TypeMismatch: fold accumulator expects {}, actual {}",
                        type_desc(&init_ty),
                        type_desc(&params[0])
                    )));
                }
                if !types_equal(&params[1], &elem) {
                    return Err(mismatch(format!(
                        "TypeMismatch: fold closure element expects {}, actual {}",
                        type_desc(&elem),
                        type_desc(&params[1])
                    )));
                }
                if !types_equal(&ret, &params[0]) {
                    return Err(mismatch(
                        "TypeMismatch: fold closure must return the accumulator type".into(),
                    ));
                }
                Ok(params[0].clone())
            })());
        }

        None
    }
}

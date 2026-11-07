//! Std receiver-shape leaf cascade — the receiver-keyed arms
//! of the Zig call arm (type_checker.zig, venue :9512-10350)
//! plus its name-keyed container arms (`push` / `pop` /
//! `remove` / iterator combinators, venue :10161-10460).
//!
//! Until this port (phase-6 census root class R-2/R-1), the Rust
//! checker had NO arm for these: `v.push(x)`, `m.insert(k, v)`,
//! `opt.unwrap()` and kin all fell through to
//! `Undefined call: <name>` in whichever function bodies the
//! frontend actually parsed. The cascade is wired as the LAST
//! stage of `check_receiver_call` (after inherent / trait / dyn
//! / `using` resolution misses), so it can only turn previously
//! undefined calls into typed ones; user-defined methods keep
//! precedence, exactly the fallback character the arms have in
//! Zig (they fire on std-shaped receivers).
//!
//! Deliberately NOT in this batch (registered in
//! docs/divergences/checker.md): the `UncloneableClone` /
//! `UncloneableCopy` closure diagnostics on the clone/push
//! arms, `checkArenaMoveArg` escape validation, and the
//! enum-payload Copy exemption inside insert-argument
//! consumption (the checker's enum table carries variant names
//! only, so payload Copy-ness cannot be evaluated here). The
//! shallow-copy half of the push discipline
//! (`type_is_shallow_copy_call_arg_value`) IS ported, in
//! check_std_ctor.rs.

use crate::check_std_ctor::{option_ty, result_ty, user_ty};
use crate::checker::{make_borrow_type, Checker};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use crate::types_util::{
    arc_inner_type, array_type_of, atomic_ptr_inner_type, bool_ty,
    box_inner_type, cell_inner_type, grow_vec_element_type, i32_ty,
    i64_ty, is_file_type, is_metadata_type, is_numeric_type,
    is_ordering_type, is_string_like_type, is_string_type,
    join_handle_inner_type, mutex_inner_type, node_key_of,
    option_inner_type, peel_borrow_pointer, receiver_inner_type,
    ref_cell_inner_type, result_err_type, result_ok_type, rc_inner_type,
    rw_lock_inner_type, sender_inner_type, slice_element_type,
    type_desc, types_equal, u64_ty, vec_element_type, void_ty,
};
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
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
    /// Refine a receiver binding whose shape was `Infer` to the
    /// concrete container type (Zig: `sym.ty = concrete_ty` plus
    /// the `expr_types` rewrite of the receiver node).
    pub(crate) fn refine_receiver_binding(&mut self, recv: &Node, ty: &Type) {
        if let Node::Identifier(name) = recv {
            if let Some(b) = self.scopes.lookup_mut(name) {
                b.ty = ty.clone();
            }
            self.expr_types.insert(node_key_of(recv), ty.clone());
        }
    }

    /// Zig `consumeInsertArgIfMoved` (type_checker.zig:5031),
    /// minus the arena-move validation and the enum-payload Copy
    /// exemption (see the file header). Consumes a moved
    /// identifier argument of a map/set insert.
    pub(crate) fn consume_std_insert_arg(
        &mut self,
        recv: &Node,
        arg: &Node,
        arg_ty: &Type,
    ) -> CheckResult<()> {
        let Node::Identifier(name) = arg else {
            return Ok(());
        };
        if matches!(arg_ty, Type::Infer) {
            return Ok(());
        }
        // Zig's transfer fact (insertArgTransfersOwnership over
        // typeIsCopyValueLeaf): only scalar copy leaves, pointers
        // and borrow-like handles stay live — insert has NO copy
        // materialization, so even Copy-derive structs transfer.
        let copy_leaf = match arg_ty {
            Type::Primitive(p) => {
                !matches!(p, Primitive::RawPtr | Primitive::VoidType)
            }
            Type::FnPtr(_) => true,
            _ => false,
        };
        if copy_leaf
            || matches!(arg_ty, Type::Pointer(_))
            || crate::types_util::is_borrow_like_type(arg_ty)
        {
            return Ok(());
        }
        if slice_element_type(arg_ty).is_some() {
            return Ok(());
        }
        if matches!(arg_ty, Type::UserDefined(u) if u.name == "Chan") {
            return Ok(());
        }
        if let Node::Identifier(recv_name) = recv {
            if recv_name == name {
                return Ok(());
            }
        }
        let (is_const, state) = match self.scopes.lookup(name) {
            Some(b) => (b.is_const, b.state),
            None => return Ok(()),
        };
        if is_const {
            return Ok(());
        }
        if state == ValueState::Consumed {
            return Err(Diagnostic::new(
                ErrorKind::UseAfterMove,
                format!(
                    "UseAfterMove: identifier `{name}` was already consumed"
                ),
            ));
        }
        self.consume_binding(name, "insert argument")
    }

    /// The push consumption discipline of the Zig `push` /
    /// `push_back` arms: a non-Copy identifier element is
    /// consumed (moved into the container); a Copy element stays
    /// live. Self-push and const/borrowed bindings are exempt.
    pub(crate) fn consume_push_arg(
        &mut self,
        recv: &Node,
        arg: &Node,
        arg_ty: &Type,
        operation: &str,
    ) -> CheckResult<()> {
        let Node::Identifier(name) = arg else {
            return Ok(());
        };
        if matches!(arg_ty, Type::Infer) {
            return Ok(());
        }
        // Copy-like elements (Copy derive OR the DEF-5
        // shallow-copy classification) stay live: the emitters
        // materialize a copy into the container. Everything else
        // transfers (vecElementPushTransfersOwnership: not
        // copy-like and not borrow-like).
        let copy_like = self.type_is_copy(arg_ty)
            || self.type_is_shallow_copy_call_arg_value(arg_ty, 0);
        if copy_like || crate::types_util::is_borrow_like_type(arg_ty) {
            return Ok(());
        }
        if let Node::Identifier(recv_name) = recv {
            if recv_name == name {
                return Ok(());
            }
        }
        let eligible = match self.scopes.lookup(name) {
            Some(b) => {
                !b.is_const
                    && b.state == ValueState::Active
                    && b.borrow_source.is_none()
            }
            None => false,
        };
        if eligible {
            self.consume_binding(name, operation)?;
        }
        Ok(())
    }

    /// Check a closure argument and return its closure type
    /// (Zig checks the literal with context; SLA source
    /// annotates closure parameters, so the plain check plus the
    /// parameter comparisons in each arm are equivalent).
    pub(crate) fn check_std_closure_arg(&mut self, arg: &Node) -> CheckResult<(Vec<Type>, Type)> {
        let ty = self.check_expr(arg)?;
        match ty {
            Type::Closure(c) => Ok((c.params.clone(), (*c.ret).clone())),
            other => Err(mismatch(format!(
                "TypeMismatch: expected a closure, actual {}",
                type_desc(&other)
            ))),
        }
    }

    /// The std receiver leaf cascade. `recv_ty` is the already
    /// checked type of `call.args[0]`. Returns `None` when no
    /// leaf matches, leaving the caller's fallback to fire.
    pub fn check_std_receiver_leaf(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        let argc = call.args.len();
        let recv = &call.args[0];

        // -- Option<T> ------------------------------------------
        if let Some(inner) = option_inner_type(recv_ty) {
            match name {
                "is_some" | "is_none" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(bool_ty())
                    })());
                }
                "copied" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        match inner {
                            Type::Borrow(b) => Ok(option_ty((*b).clone())),
                            other => Err(mismatch(format!(
                                "TypeMismatch: copied expects an Option of a borrow, actual Option<{}>",
                                type_desc(&other)
                            ))),
                        }
                    })());
                }
                "map" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let (params, ret) = self.check_std_closure_arg(&call.args[1])?;
                        if params.len() != 1 {
                            return Err(arity(name, 1, params.len()));
                        }
                        if !types_equal(&params[0], &inner) {
                            return Err(mismatch(format!(
                                "TypeMismatch: Option::map closure parameter expects {}, actual {}",
                                type_desc(&inner),
                                type_desc(&params[0])
                            )));
                        }
                        Ok(option_ty(ret))
                    })());
                }
                "and_then" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let (params, ret) = self.check_std_closure_arg(&call.args[1])?;
                        if params.len() != 1 {
                            return Err(arity(name, 1, params.len()));
                        }
                        if !types_equal(&params[0], &inner) {
                            return Err(mismatch(format!(
                                "TypeMismatch: Option::and_then closure parameter expects {}, actual {}",
                                type_desc(&inner),
                                type_desc(&params[0])
                            )));
                        }
                        if option_inner_type(&ret).is_none() {
                            return Err(mismatch(format!(
                                "TypeMismatch: Option::and_then closure must return an Option, actual {}",
                                type_desc(&ret)
                            )));
                        }
                        Ok(ret)
                    })());
                }
                "unwrap" | "unwrap_or_default" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(inner)
                    })());
                }
                "unwrap_or" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let default_ty = self.check_expr(&call.args[1])?;
                        if !types_equal(&inner, &default_ty) {
                            return Err(mismatch(format!(
                                "TypeMismatch: unwrap_or default expects {}, actual {}",
                                type_desc(&inner),
                                type_desc(&default_ty)
                            )));
                        }
                        Ok(inner)
                    })());
                }
                "unwrap_or_else" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let (params, ret) = self.check_std_closure_arg(&call.args[1])?;
                        if !params.is_empty() {
                            return Err(arity(name, 0, params.len()));
                        }
                        if !types_equal(&inner, &ret) {
                            return Err(mismatch(format!(
                                "TypeMismatch: unwrap_or_else closure returns {}, expected {}",
                                type_desc(&ret),
                                type_desc(&inner)
                            )));
                        }
                        Ok(inner)
                    })());
                }
                _ => {}
            }
        }

        // -- Result<T, E> ----------------------------------------
        if let Some(ok) = result_ok_type(recv_ty) {
            match name {
                "is_ok" | "is_err" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(bool_ty())
                    })());
                }
                "map" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let err = result_err_type(recv_ty).ok_or_else(|| {
                            mismatch("TypeMismatch: Result has no error type".into())
                        })?;
                        let (params, ret) = self.check_std_closure_arg(&call.args[1])?;
                        if params.len() != 1 {
                            return Err(arity(name, 1, params.len()));
                        }
                        if !types_equal(&params[0], &ok) {
                            return Err(mismatch(format!(
                                "TypeMismatch: Result::map closure parameter expects {}, actual {}",
                                type_desc(&ok),
                                type_desc(&params[0])
                            )));
                        }
                        Ok(result_ty(ret, err))
                    })());
                }
                "unwrap" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(ok)
                    })());
                }
                "unwrap_or" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let default_ty = self.check_expr(&call.args[1])?;
                        if !types_equal(&ok, &default_ty) {
                            return Err(mismatch(format!(
                                "TypeMismatch: unwrap_or default expects {}, actual {}",
                                type_desc(&ok),
                                type_desc(&default_ty)
                            )));
                        }
                        Ok(ok)
                    })());
                }
                _ => {}
            }
        }

        // -- Cell / RefCell / Mutex / RwLock ----------------------
        if let Some(inner) = cell_inner_type(recv_ty) {
            match name {
                "get" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(inner)
                    })());
                }
                "set" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let value_ty = self.check_expr(&call.args[1])?;
                        if !types_equal(&inner, &value_ty) {
                            return Err(mismatch(format!(
                                "Cell::set type mismatch: cell tag={}, value tag={}",
                                type_desc(&inner),
                                type_desc(&value_ty)
                            )));
                        }
                        Ok(void_ty())
                    })());
                }
                _ => {}
            }
        }
        if let Some(inner) = ref_cell_inner_type(recv_ty) {
            if name == "borrow" || name == "borrow_mut" {
                return Some((|| {
                    if argc != 1 {
                        return Err(arity(name, 1, argc));
                    }
                    Ok(make_borrow_type(&inner))
                })());
            }
        }
        if let Some(inner) = mutex_inner_type(recv_ty) {
            if name == "lock" {
                return Some((|| {
                    if argc != 1 {
                        return Err(arity(name, 1, argc));
                    }
                    Ok(result_ty(user_ty("MutexGuard", vec![inner]), i32_ty()))
                })());
            }
        }
        if let Some(inner) = rw_lock_inner_type(recv_ty) {
            match name {
                "read" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(result_ty(
                            user_ty("RwLockReadGuard", vec![inner]),
                            i32_ty(),
                        ))
                    })());
                }
                "write" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(result_ty(
                            user_ty("RwLockWriteGuard", vec![inner]),
                            i32_ty(),
                        ))
                    })());
                }
                _ => {}
            }
        }

        // -- File / Metadata ---------------------------------------
        if is_file_type(recv_ty) && name == "as_raw_fd" {
            return Some((|| {
                if argc != 1 {
                    return Err(arity(name, 1, argc));
                }
                Ok(i32_ty())
            })());
        }
        if is_metadata_type(recv_ty) {
            match name {
                "is_file" | "is_dir" | "is_symlink" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(bool_ty())
                    })());
                }
                "modified_ms" | "created_ms" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(i64_ty())
                    })());
                }
                _ => {}
            }
        }
        if name == "metadata" && is_string_like_type(recv_ty) {
            return Some((|| {
                if argc != 1 {
                    return Err(arity(name, 1, argc));
                }
                Ok(result_ty(user_ty("Metadata", vec![]), i32_ty()))
            })());
        }

        // -- JoinHandle / Sender / Receiver ------------------------
        if let Some(inner) = join_handle_inner_type(recv_ty) {
            if name == "join" {
                return Some((|| {
                    if argc != 1 {
                        return Err(arity(name, 1, argc));
                    }
                    Ok(result_ty(inner, i32_ty()))
                })());
            }
        }
        if let Some(inner) = sender_inner_type(recv_ty) {
            match name {
                "clone" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(recv_ty.clone())
                    })());
                }
                "send" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let value_ty = self.check_expr(&call.args[1])?;
                        if !types_equal(&inner, &value_ty) {
                            return Err(mismatch(format!(
                                "TypeMismatch: send expects {}, actual {}",
                                type_desc(&inner),
                                type_desc(&value_ty)
                            )));
                        }
                        Ok(result_ty(void_ty(), i32_ty()))
                    })());
                }
                _ => {}
            }
        }
        if let Some(inner) = receiver_inner_type(recv_ty) {
            if name == "recv" {
                return Some((|| {
                    if argc != 1 {
                        return Err(arity(name, 1, argc));
                    }
                    Ok(result_ty(inner, i32_ty()))
                })());
            }
        }

        // -- clone (Rc / Arc / String / Vec / GrowVec / struct) ----
        // A user-defined `clone` method already won in
        // check_receiver_call, matching Zig's method-first gate.
        if name == "clone" {
            let is_clone_shape = rc_inner_type(recv_ty).is_some()
                || arc_inner_type(recv_ty).is_some()
                || is_string_type(recv_ty)
                || vec_element_type(recv_ty).is_some()
                || grow_vec_element_type(recv_ty).is_some()
                || self.struct_decl_for_type(recv_ty).is_some();
            if is_clone_shape {
                return Some((|| {
                    if argc != 1 {
                        return Err(arity(name, 1, argc));
                    }
                    Ok((*peel_borrow_pointer(recv_ty)).clone())
                })());
            }
        }

        // -- Rc / Arc reference counts ------------------------------
        if (rc_inner_type(recv_ty).is_some() || arc_inner_type(recv_ty).is_some())
            && (name == "strong_count" || name == "weak_count")
        {
            return Some((|| {
                if argc != 1 {
                    return Err(arity(name, 1, argc));
                }
                Ok(u64_ty())
            })());
        }

        // -- as_ptr / AtomicPtr::load / pointer add ------------------
        if name == "as_ptr" {
            let ptr_to = box_inner_type(recv_ty)
                .or_else(|| rc_inner_type(recv_ty))
                .or_else(|| arc_inner_type(recv_ty))
                .or_else(|| array_type_of(recv_ty).map(|(elem, _)| elem))
                .or_else(|| {
                    if is_string_like_type(recv_ty) {
                        Some(Type::Primitive(Primitive::U8))
                    } else {
                        None
                    }
                })
                .or_else(|| slice_element_type(recv_ty));
            if let Some(inner) = ptr_to {
                return Some((|| {
                    if argc != 1 {
                        return Err(arity(name, 1, argc));
                    }
                    Ok(Type::Pointer(Box::new(inner)))
                })());
            }
        }
        if let Some(inner) = atomic_ptr_inner_type(recv_ty) {
            if name == "load" {
                return Some((|| {
                    if argc != 2 {
                        return Err(arity(name, 2, argc));
                    }
                    let ordering_ty = self.check_expr(&call.args[1])?;
                    if !is_ordering_type(&ordering_ty) {
                        return Err(mismatch(
                            "TypeMismatch: AtomicPtr load expects an Ordering".into(),
                        ));
                    }
                    Ok(Type::Pointer(Box::new(inner)))
                })());
            }
        }
        if matches!(recv_ty, Type::Pointer(_)) && name == "add" {
            return Some((|| {
                if argc != 2 {
                    return Err(arity(name, 2, argc));
                }
                let index_ty = self.check_expr(&call.args[1])?;
                if !crate::types_util::is_integer_type(&index_ty) {
                    return Err(mismatch(
                        "TypeMismatch: pointer add expects an integer index".into(),
                    ));
                }
                Ok(recv_ty.clone())
            })());
        }
        if let Some((elem, _)) = array_type_of(recv_ty) {
            if name == "fill" {
                return Some((|| {
                    if argc != 2 {
                        return Err(arity(name, 2, argc));
                    }
                    let value_ty = self.check_expr(&call.args[1])?;
                    if !types_equal(&elem, &value_ty) {
                        return Err(mismatch(format!(
                            "TypeMismatch: fill expects {}, actual {}",
                            type_desc(&elem),
                            type_desc(&value_ty)
                        )));
                    }
                    Ok(void_ty())
                })());
            }
        }

        None
    }
}

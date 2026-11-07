//! Std leaf cascade, map/set arms — HashMap / HashSet /
//! BTreeMap / BTreeSet insert/get/contains/clear of the Zig
//! call arm (type_checker.zig, venue :9606-10460), split from
//! check_std_leaf2.rs for the 700-line file cap. Runs inside
//! `check_std_receiver_leaf2`'s dispatch sequence; see
//! check_std_leaf.rs for the port's scope notes.

use crate::check_std_ctor::{option_ty, user_ty};
use crate::checker::{make_borrow_type, Checker};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    bool_ty, btree_map_types, btree_set_element_type, hash_map_types,
    hash_set_element_type, i32_ty, slice_element_type, type_desc,
    types_equal, void_ty,
};
use sla_ast::exprs::CallExpr;
use sla_ast::types::{Primitive, Type};

pub(crate) fn arity(name: &str, want: usize, got: usize) -> Diagnostic {
    Diagnostic::new(
        ErrorKind::InvalidArgsCount,
        format!("InvalidArgsCount: `{name}` expects {want} arguments, got {got}"),
    )
}

pub(crate) fn mismatch(msg: String) -> Diagnostic {
    Diagnostic::new(ErrorKind::TypeMismatch, msg)
}

/// Zig `btreeSetKeySupported`: only raw pointers and slices are
/// supported BTree keys in this snapshot.
pub(crate) fn btree_key_supported(ty: &Type) -> bool {
    if matches!(ty, Type::Primitive(Primitive::RawPtr)) {
        return true;
    }
    slice_element_type(ty).is_some()
}

impl Checker {
    /// The map/set leaf arms. Returns `None` when no leaf
    /// matches.
    pub fn check_std_map_leaf(
        &mut self,
        call: &CallExpr,
        recv_ty: &Type,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        let argc = call.args.len();
        let recv = &call.args[0];
        let _ = recv;
        // -- HashMap ---------------------------------------------------
        if let Some((key_ty, value_ty)) = hash_map_types(recv_ty) {
            match name {
                "insert" => {
                    return Some((|| {
                        if argc != 3 {
                            return Err(arity(name, 3, argc));
                        }
                        let k = self.check_expr(&call.args[1])?;
                        let v = self.check_expr(&call.args[2])?;
                        if !types_equal(&key_ty, &k) || !types_equal(&value_ty, &v) {
                            return Err(mismatch(format!(
                                "TypeMismatch: HashMap insert expects ({}, {}), actual ({}, {})",
                                type_desc(&key_ty),
                                type_desc(&value_ty),
                                type_desc(&k),
                                type_desc(&v)
                            )));
                        }
                        if matches!(key_ty, Type::Infer)
                            || matches!(value_ty, Type::Infer)
                        {
                            self.refine_receiver_binding(
                                recv,
                                &user_ty("HashMap", vec![k.clone(), v.clone()]),
                            );
                        }
                        self.consume_std_insert_arg(recv, &call.args[1], &k)?;
                        self.consume_std_insert_arg(recv, &call.args[2], &v)?;
                        Ok(option_ty(v))
                    })());
                }
                "get" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let k = self.check_expr(&call.args[1])?;
                        if !types_equal(&key_ty, &k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: HashMap get expects key {}, actual {}",
                                type_desc(&key_ty),
                                type_desc(&k)
                            )));
                        }
                        Ok(option_ty(make_borrow_type(&value_ty)))
                    })());
                }
                "clear" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(void_ty())
                    })());
                }
                _ => {}
            }
        }

        // -- HashSet ---------------------------------------------------
        if let Some(key_ty) = hash_set_element_type(recv_ty) {
            match name {
                "insert" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let k = self.check_expr(&call.args[1])?;
                        if !types_equal(&key_ty, &k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: HashSet insert expects {}, actual {}",
                                type_desc(&key_ty),
                                type_desc(&k)
                            )));
                        }
                        if matches!(key_ty, Type::Infer) {
                            self.refine_receiver_binding(
                                recv,
                                &user_ty("HashSet", vec![k.clone()]),
                            );
                        }
                        self.consume_std_insert_arg(recv, &call.args[1], &k)?;
                        Ok(bool_ty())
                    })());
                }
                "contains" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let k = self.check_expr(&call.args[1])?;
                        if !types_equal(&key_ty, &k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: HashSet contains expects {}, actual {}",
                                type_desc(&key_ty),
                                type_desc(&k)
                            )));
                        }
                        Ok(bool_ty())
                    })());
                }
                _ => {}
            }
        }

        // -- BTreeMap --------------------------------------------------
        if let Some((key_ty, value_ty)) = btree_map_types(recv_ty) {
            match name {
                "insert" => {
                    return Some((|| {
                        if argc != 3 {
                            return Err(arity(name, 3, argc));
                        }
                        let k = self.check_expr(&call.args[1])?;
                        let v = self.check_expr(&call.args[2])?;
                        if !types_equal(&key_ty, &k) || !types_equal(&value_ty, &v) {
                            return Err(mismatch(format!(
                                "TypeMismatch: BTreeMap insert expects ({}, {}), actual ({}, {})",
                                type_desc(&key_ty),
                                type_desc(&value_ty),
                                type_desc(&k),
                                type_desc(&v)
                            )));
                        }
                        if !btree_key_supported(&k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: unsupported BTreeMap key type {}",
                                type_desc(&k)
                            )));
                        }
                        if matches!(key_ty, Type::Infer)
                            || matches!(value_ty, Type::Infer)
                        {
                            self.refine_receiver_binding(
                                recv,
                                &user_ty("BTreeMap", vec![k.clone(), v.clone()]),
                            );
                        }
                        self.consume_std_insert_arg(recv, &call.args[1], &k)?;
                        self.consume_std_insert_arg(recv, &call.args[2], &v)?;
                        Ok(option_ty(v))
                    })());
                }
                "get" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let k = self.check_expr(&call.args[1])?;
                        if !types_equal(&key_ty, &k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: BTreeMap get expects key {}, actual {}",
                                type_desc(&key_ty),
                                type_desc(&k)
                            )));
                        }
                        if !btree_key_supported(&k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: unsupported BTreeMap key type {}",
                                type_desc(&k)
                            )));
                        }
                        Ok(option_ty(make_borrow_type(&value_ty)))
                    })());
                }
                "clear" => {
                    return Some((|| {
                        if argc != 1 {
                            return Err(arity(name, 1, argc));
                        }
                        Ok(void_ty())
                    })());
                }
                _ => {}
            }
        }

        // -- BTreeSet --------------------------------------------------
        if let Some(key_ty) = btree_set_element_type(recv_ty) {
            match name {
                "insert" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let k = self.check_expr(&call.args[1])?;
                        if !types_equal(&key_ty, &k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: BTreeSet insert expects {}, actual {}",
                                type_desc(&key_ty),
                                type_desc(&k)
                            )));
                        }
                        if !btree_key_supported(&k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: unsupported BTreeSet key type {}",
                                type_desc(&k)
                            )));
                        }
                        if matches!(key_ty, Type::Infer) {
                            self.refine_receiver_binding(
                                recv,
                                &user_ty("BTreeSet", vec![k.clone()]),
                            );
                        }
                        self.consume_std_insert_arg(recv, &call.args[1], &k)?;
                        Ok(bool_ty())
                    })());
                }
                "contains" => {
                    return Some((|| {
                        if argc != 2 {
                            return Err(arity(name, 2, argc));
                        }
                        let k = self.check_expr(&call.args[1])?;
                        if !types_equal(&key_ty, &k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: BTreeSet contains expects {}, actual {}",
                                type_desc(&key_ty),
                                type_desc(&k)
                            )));
                        }
                        if !btree_key_supported(&k) {
                            return Err(mismatch(format!(
                                "TypeMismatch: unsupported BTreeSet key type {}",
                                type_desc(&k)
                            )));
                        }
                        Ok(bool_ty())
                    })());
                }
                _ => {}
            }
        }

        None
    }
}

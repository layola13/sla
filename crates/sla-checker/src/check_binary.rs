//! Binary operator typing, mirroring the `binary_expr` arm of
//! `type_checker.zig` (:3215-3560) including the parts that were
//! out of slice in phase 1: operator-overload resolution
//! (`overloadSymbol`), the numeric-struct special cases, and the
//! derive-gated struct paths for `<=>` and the comparisons
//! (`typeIsEq` / `typeIsOrd`).
//!
//! The logical / bitwise / shift arms keep the phase-1 slice
//! behavior (Zig routes them through the same generic tail: equal
//! operand types).

use crate::checker::{Checker, StructInfo};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    concrete_type_name, is_numeric_type, literal_zero, type_desc, types_equal,
};
use sla_ast::exprs::{BinaryExpr, BinaryOp};
use sla_ast::types::{Primitive, Type};
use std::collections::HashSet;

fn ordering_ty() -> Type {
    Type::UserDefined(sla_ast::types::UserDefined {
        name: "Ordering".to_string(),
        generics: Vec::new(),
    })
}

fn mismatch(msg: String) -> Diagnostic {
    Diagnostic::new(ErrorKind::TypeMismatch, msg)
}

impl Checker {
    pub fn check_binary_expr(&mut self, b: &BinaryExpr) -> CheckResult<Type> {
        let left = self.check_expr(&b.left)?;
        let right = self.check_expr(&b.right)?;
        match b.op {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod => self.check_arithmetic(b, &left, &right),
            BinaryOp::Spaceship => self.check_spaceship(&left, &right),
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => self.check_comparison(b.op, &left, &right),
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                if matches!(left, Type::Primitive(Primitive::Boolean))
                    && matches!(right, Type::Primitive(Primitive::Boolean))
                {
                    Ok(crate::types_util::bool_ty())
                } else {
                    Err(mismatch(format!(
                        "TypeMismatch: logical operator requires boolean operands on {} and {}",
                        type_desc(&left),
                        type_desc(&right)
                    )))
                }
            }
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                if crate::types_util::is_integer_type(&left)
                    && types_equal(&left, &right)
                {
                    Ok(left.clone())
                } else {
                    Err(mismatch(format!(
                        "TypeMismatch: bitwise operator requires integer operands on {} and {}",
                        type_desc(&left),
                        type_desc(&right)
                    )))
                }
            }
            BinaryOp::Shl | BinaryOp::Shr => {
                if crate::types_util::is_integer_type(&left)
                    && crate::types_util::is_integer_type(&right)
                {
                    Ok(left.clone())
                } else {
                    Err(mismatch(format!(
                        "TypeMismatch: shift requires integer operands on {} and {}",
                        type_desc(&left),
                        type_desc(&right)
                    )))
                }
            }
        }
    }

    /// Zig `structDeclForType`: peel borrow/pointer, then look the
    /// concrete name up in the struct table.
    pub fn struct_decl_for_type(&self, ty: &Type) -> Option<(String, StructInfo)> {
        let name = concrete_type_name(ty)?;
        self.structs.get(name).map(|info| (name.to_string(), info.clone()))
    }

    /// Zig `overloadSymbol` + `operatorToOverloadName`: find the
    /// unique registered overload whose two parameter types match
    /// the operands exactly; two distinct matching symbols are an
    /// AmbiguousCall. `Mod` has no overload name in Zig.
    fn overload_symbol(
        &self,
        op_name: Option<&str>,
        left: &Type,
        right: &Type,
    ) -> CheckResult<Option<Type>> {
        let Some(op_name) = op_name else {
            return Ok(None);
        };
        let Some(left_name) = concrete_type_name(left) else {
            return Ok(None);
        };
        let prefix = format!("{left_name}|");
        let mut found: Option<(String, Type)> = None;
        for (key, symbols) in &self.overloads {
            let Some((_, key_op)) = key.split_once('|') else {
                continue;
            };
            if !key.starts_with(&prefix) || key_op != op_name {
                continue;
            }
            for symbol in symbols {
                let Some(sig) = self.funcs.get(symbol) else {
                    continue;
                };
                if sig.params.len() == 2
                    && types_equal(&sig.params[0].ty, left)
                    && types_equal(&sig.params[1].ty, right)
                {
                    match &found {
                        Some((prev, _)) if prev != symbol => {
                            return Err(Diagnostic::new(
                                ErrorKind::AmbiguousCall,
                                format!(
                                    "AmbiguousCall: operator {op_name} for {}",
                                    type_desc(left)
                                ),
                            ));
                        }
                        _ => found = Some((symbol.clone(), sig.ret_ty.clone())),
                    }
                }
            }
        }
        Ok(found.map(|(_, ret)| ret))
    }

    fn check_arithmetic(
        &mut self,
        b: &BinaryExpr,
        left: &Type,
        right: &Type,
    ) -> CheckResult<Type> {
        let op_name = match b.op {
            BinaryOp::Add => Some("op_add"),
            BinaryOp::Sub => Some("op_sub"),
            BinaryOp::Mul => Some("op_mul"),
            BinaryOp::Div => Some("op_div"),
            _ => None,
        };
        if let Some(ret) = self.overload_symbol(op_name, left, right)? {
            return Ok(ret);
        }
        let left_decl = self.struct_decl_for_type(left);
        let right_decl = self.struct_decl_for_type(right);
        if let (Some((lname, linfo)), Some((rname, rinfo))) =
            (&left_decl, &right_decl)
        {
            if !types_equal(left, right) {
                return Err(mismatch(format!(
                    "TypeMismatch: operator requires matching struct types, actual {} and {}",
                    type_desc(left),
                    type_desc(right)
                )));
            }
            return match b.op {
                BinaryOp::Add | BinaryOp::Sub => {
                    if lname == rname
                        && struct_fields_all_numeric(linfo)
                        && struct_fields_all_numeric(rinfo)
                    {
                        Ok(left.clone())
                    } else {
                        Err(mismatch(format!(
                            "TypeMismatch: operator not supported for struct {}",
                            lname
                        )))
                    }
                }
                _ => Err(mismatch(format!(
                    "TypeMismatch: operator not supported for struct {}",
                    lname
                ))),
            };
        }
        if let Some((lname, _)) = &left_decl {
            return match b.op {
                BinaryOp::Mul if is_numeric_type(right) => Ok(left.clone()),
                _ => Err(mismatch(format!(
                    "TypeMismatch: operator not supported for struct {lname}"
                ))),
            };
        }
        if let Some((rname, _)) = &right_decl {
            return match b.op {
                BinaryOp::Sub if literal_zero(&b.left) => Ok(right.clone()),
                BinaryOp::Mul if is_numeric_type(left) => Ok(right.clone()),
                _ => Err(mismatch(format!(
                    "TypeMismatch: operator not supported for struct {rname}"
                ))),
            };
        }
        if b.op == BinaryOp::Sub
            && literal_zero(&b.left)
            && is_numeric_type(right)
        {
            return Ok(right.clone());
        }
        if !types_equal(left, right) {
            return Err(mismatch(format!(
                "TypeMismatch: operator requires matching operand types, actual {} and {}",
                type_desc(left),
                type_desc(right)
            )));
        }
        if matches!(left, Type::Infer) {
            return Ok(left.clone());
        }
        if !is_numeric_type(left) {
            return Err(mismatch(format!(
                "TypeMismatch: operator requires numeric operands, actual {}",
                type_desc(left)
            )));
        }
        Ok(left.clone())
    }

    fn check_spaceship(&mut self, left: &Type, right: &Type) -> CheckResult<Type> {
        if let Some(ret) = self.overload_symbol(Some("op_cmp"), left, right)? {
            return Ok(ret);
        }
        let left_decl = self.struct_decl_for_type(left);
        let right_decl = self.struct_decl_for_type(right);
        if left_decl.is_some() || right_decl.is_some() {
            if !self.type_is_ord(left) || !self.type_is_ord(right) {
                return Err(mismatch(
                    "TypeMismatch: spaceship requires Ord types".to_string(),
                ));
            }
            let lname = left_decl.as_ref().map(|(n, _)| n.clone());
            let rname = right_decl.as_ref().map(|(n, _)| n.clone());
            if lname != rname {
                return Err(mismatch(
                    "TypeMismatch: spaceship requires matching struct types"
                        .to_string(),
                ));
            }
            if !types_equal(left, right) {
                return Err(mismatch(
                    "TypeMismatch: spaceship requires matching operand types"
                        .to_string(),
                ));
            }
            return Ok(ordering_ty());
        }
        if !types_equal(left, right) {
            return Err(mismatch(
                "TypeMismatch: spaceship requires matching operand types"
                    .to_string(),
            ));
        }
        if !is_numeric_type(left) {
            return Err(mismatch(
                "TypeMismatch: spaceship requires numeric operands".to_string(),
            ));
        }
        Ok(ordering_ty())
    }

    fn check_comparison(
        &mut self,
        op: BinaryOp,
        left: &Type,
        right: &Type,
    ) -> CheckResult<Type> {
        let left_decl = self.struct_decl_for_type(left);
        let right_decl = self.struct_decl_for_type(right);
        if left_decl.is_some() || right_decl.is_some() {
            let gated = match op {
                BinaryOp::Eq | BinaryOp::Ne => {
                    self.type_is_eq(left) && self.type_is_eq(right)
                }
                _ => self.type_is_ord(left) && self.type_is_ord(right),
            };
            if !gated {
                return Err(mismatch(format!(
                    "TypeMismatch: comparison requires {} types",
                    if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                        "Eq"
                    } else {
                        "Ord"
                    }
                )));
            }
            let lname = left_decl.as_ref().map(|(n, _)| n.clone());
            let rname = right_decl.as_ref().map(|(n, _)| n.clone());
            if lname != rname {
                return Err(mismatch(
                    "TypeMismatch: comparison requires matching struct types"
                        .to_string(),
                ));
            }
            if !types_equal(left, right) {
                return Err(mismatch(
                    "TypeMismatch: comparison requires matching operand types"
                        .to_string(),
                ));
            }
            return Ok(crate::types_util::bool_ty());
        }
        if !types_equal(left, right) {
            return Err(mismatch(format!(
                "TypeMismatch: comparison requires matching operand types, actual {} and {}",
                type_desc(left),
                type_desc(right)
            )));
        }
        Ok(crate::types_util::bool_ty())
    }

    /// Zig `typeIsEq`: primitives use the Copy-value base set;
    /// user structs need the Eq derive gate, non-opaque,
    /// non-union, and all fields recursively Eq. (A visiting set
    /// keeps recursive structs from looping where Zig's recursion
    /// is unbounded.)
    pub fn type_is_eq(&self, ty: &Type) -> bool {
        self.type_is_eq_ord(ty, false, &mut HashSet::new())
    }

    /// Zig `typeIsOrd`: same shape with the hashable base set
    /// (floats excluded) and the Ord derive gate.
    pub fn type_is_ord(&self, ty: &Type) -> bool {
        self.type_is_eq_ord(ty, true, &mut HashSet::new())
    }

    fn type_is_eq_ord(
        &self,
        ty: &Type,
        ord: bool,
        visiting: &mut HashSet<String>,
    ) -> bool {
        match ty {
            Type::Primitive(p) => {
                if ord {
                    !matches!(
                        p,
                        Primitive::RawPtr
                            | Primitive::VoidType
                            | Primitive::F32
                            | Primitive::F64
                            | Primitive::Float
                    )
                } else {
                    !matches!(p, Primitive::RawPtr | Primitive::VoidType)
                }
            }
            Type::UserDefined(_) => {
                let Some((name, info)) = self.struct_decl_for_type(ty) else {
                    return false;
                };
                let gated = if ord {
                    info.derives.iter().any(|d| {
                        d.eq_ignore_ascii_case("ord")
                            || d.eq_ignore_ascii_case("partialord")
                    })
                } else {
                    info.derives.iter().any(|d| {
                        d.eq_ignore_ascii_case("eq")
                            || d.eq_ignore_ascii_case("partialeq")
                    })
                };
                if !gated || info.is_opaque || info.is_union {
                    return false;
                }
                if !visiting.insert(name) {
                    return false;
                }
                let ok = info
                    .fields
                    .iter()
                    .all(|(_, fty)| self.type_is_eq_ord(fty, ord, visiting));
                visiting.remove(&self.struct_decl_name(ty));
                ok
            }
            _ => false,
        }
    }

    fn struct_decl_name(&self, ty: &Type) -> String {
        concrete_type_name(ty).unwrap_or("").to_string()
    }
}

/// Zig `structFieldsAllNumeric`.
fn struct_fields_all_numeric(info: &StructInfo) -> bool {
    info.fields.iter().all(|(_, ty)| is_numeric_type(ty))
}

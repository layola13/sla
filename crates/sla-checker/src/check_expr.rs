//! Expression typing, mirroring `checkExpr` / `checkExprImpl` of
//! `type_checker.zig` (wrapper 3143, arms from 3169).
//!
//! Reading an identifier NEVER consumes it; consumption happens only
//! at the enumerated sites (see the consume-site census in
//! docs/divergences/checker.md, D-C7): explicit `^move`, let/assign
//! implicit moves, `release`, macro arguments, extern move contracts,
//! await, and try propagation (the last two live in this file).

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use crate::types_util::{
    bool_ty, i64_ty, is_borrow_like_type, is_integer_type, is_numeric_type, node_key_of,
    type_desc, types_equal, u64_ty, value_assignable_to, void_ty,
};
use sla_ast::exprs::{BinaryOp, Literal};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

impl Checker {
    /// Mirrors the Zig `checkExpr` wrapper: every successfully typed
    /// expression is recorded in the `expr_types` side table.
    pub fn check_expr(&mut self, node: &Node) -> CheckResult<Type> {
        let ty = self.check_expr_impl(node)?;
        self.expr_types.insert(node_key_of(node), ty.clone());
        Ok(ty)
    }

    fn check_expr_impl(&mut self, node: &Node) -> CheckResult<Type> {
        match node {
            Node::Literal(lit) => Ok(match lit {
                Literal::IntVal(_) => i64_ty(),
                Literal::FloatVal(_) => crate::types_util::f64_ty(),
                Literal::BoolVal(_) => bool_ty(),
                // String literals type as raw pointers (the {ptr,len}
                // handle is a codegen concern).
                Literal::StringVal(_) => crate::types_util::raw_ptr_ty(),
            }),
            Node::Identifier(name) => self.check_identifier(name),
            Node::BinaryExpr(b) => {
                let left = self.check_expr(&b.left)?;
                let right = self.check_expr(&b.right)?;
                check_binary(b.op, &left, &right)
            }
            Node::BorrowExpr(b) => {
                if let Node::Identifier(name) = &*b.expr {
                    match self.lookup_state(name) {
                        Some(ValueState::Consumed) => {
                            return Err(Diagnostic::new(
                                ErrorKind::UseAfterMove,
                                format!(
                                    "UseAfterMove: identifier `{name}` was already consumed"
                                ),
                            ))
                        }
                        Some(ValueState::Uninitialized) => {
                            return Err(Diagnostic::new(
                                ErrorKind::UseBeforeInit,
                                format!("UseBeforeInit: `{name}` is not initialized"),
                            ))
                        }
                        _ => {}
                    }
                }
                let inner = self.check_expr(&b.expr)?;
                Ok(Type::Borrow(Box::new(inner)))
            }
            Node::MoveExpr(m) => {
                if let Node::Identifier(name) = &*m.expr {
                    let ty = self.check_identifier(name)?;
                    self.consume_binding(name, "explicit move")?;
                    return Ok(ty);
                }
                self.check_expr(&m.expr)
            }
            Node::DerefExpr(d) => {
                let inner = self.check_expr(&d.expr)?;
                peel_pointer(&inner).ok_or_else(|| {
                    Diagnostic::new(
                        ErrorKind::DereferenceNonPointer,
                        format!(
                            "DereferenceNonPointer: cannot dereference {}",
                            type_desc(&inner)
                        ),
                    )
                })
            }
            Node::CastExpr(c) => {
                let inner = self.check_expr(&c.expr)?;
                let target = (*c.ty).clone();
                if cast_allowed(&inner, &target) {
                    Ok(target)
                } else {
                    Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: cannot cast {} to {}",
                            type_desc(&inner),
                            type_desc(&target)
                        ),
                    ))
                }
            }
            Node::FieldExpr(f) => self.check_field(&f.expr, &f.field_name),
            Node::StructLiteral(s) => self.check_struct_literal(s),
            Node::TupleLiteral(t) => {
                let mut elems = Vec::new();
                for e in &t.elements {
                    elems.push(self.check_expr(e)?);
                }
                Ok(Type::Tuple(sla_ast::types::TupleType { elems }))
            }
            Node::ArrayLiteral(a) => {
                if a.elements.is_empty() {
                    return Ok(Type::Array(sla_ast::types::ArrayType {
                        elem: Box::new(Type::Infer),
                        len: 0,
                    }));
                }
                let first = self.check_expr(&a.elements[0])?;
                for e in &a.elements[1..] {
                    let ty = self.check_expr(e)?;
                    if !types_equal(&first, &ty) {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            "array literal elements have mismatched types",
                        ));
                    }
                }
                Ok(Type::Array(sla_ast::types::ArrayType {
                    elem: Box::new(first),
                    len: a.elements.len(),
                }))
            }
            Node::RepeatArrayLiteral(r) => {
                let elem = self.check_expr(&r.value)?;
                Ok(Type::Array(sla_ast::types::ArrayType {
                    elem: Box::new(elem),
                    len: r.len,
                }))
            }
            Node::IndexExpr(i) => {
                let target = self.check_expr(&i.target)?;
                let index = self.check_expr(&i.index)?;
                if !is_integer_type(&index) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        "index must be an integer",
                    ));
                }
                index_result_type(&target).ok_or_else(|| {
                    Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!("TypeMismatch: {} is not indexable", type_desc(&target)),
                    )
                })
            }
            Node::TryExpr(t) => {
                let inner = self.check_expr(&t.expr)?;
                if let Node::Identifier(name) = &*t.expr {
                    if !self.type_is_copy(&inner) && !is_borrow_like_type(&inner) {
                        self.consume_binding(name, "try propagation")?;
                    }
                }
                Ok(try_payload_type(&inner))
            }
            Node::AwaitExpr(a) => {
                let inner = self.check_expr(&a.expr)?;
                if let Node::Identifier(name) = &*a.expr {
                    if !self.type_is_copy(&inner) && !is_borrow_like_type(&inner) {
                        self.consume_binding(name, "await")?;
                    }
                }
                Ok(match inner {
                    Type::Future(payload) => (*payload).clone(),
                    other => other,
                })
            }
            Node::IfExpr(i) => self.check_if_expr(i),
            Node::CallExpr(c) => self.check_call(node, c),
            Node::UnsafeExpr(u) => Ok(self.check_block_value(&u.body)?.unwrap_or_else(void_ty)),
            Node::GenericFuncRef(g) => match self.funcs.get(&g.func_name) {
                Some(sig) => Ok(fn_ptr_of(sig)),
                None => Err(Diagnostic::new(
                    ErrorKind::UndefinedVariable,
                    format!("UndefinedVariable: `{}`", g.func_name),
                )),
            },
            _ => Err(Diagnostic::out_of_slice(crate::types_util::node_tag(node))),
        }
    }

    fn check_identifier(&mut self, name: &str) -> CheckResult<Type> {
        // `None` literal of the Option family.
        if name == "None" {
            return Ok(Type::UserDefined(sla_ast::types::UserDefined {
                name: "Option".to_string(),
                generics: vec![Type::Infer],
            }));
        }
        if matches!(name, "Less" | "Equal" | "Greater") {
            return Ok(Type::UserDefined(sla_ast::types::UserDefined {
                name: "Ordering".to_string(),
                generics: Vec::new(),
            }));
        }
        if let Some(binding) = self.scopes.lookup(name) {
            let (state, ty) = (binding.state, binding.ty.clone());
            return match state {
                ValueState::Consumed => Err(Diagnostic::new(
                    ErrorKind::UseAfterMove,
                    format!("UseAfterMove: identifier `{name}` was already consumed"),
                )),
                ValueState::Uninitialized => Err(Diagnostic::new(
                    ErrorKind::UseBeforeInit,
                    format!("UseBeforeInit: `{name}` is not initialized"),
                )),
                ValueState::Active => Ok(ty),
            };
        }
        if let Some(sig) = self.funcs.get(name) {
            return Ok(fn_ptr_of(sig));
        }
        if let Some(sig) = self.imported_funcs.get(name) {
            return Ok(fn_ptr_of(sig));
        }
        if let Some(ext) = self.extern_funcs.get(name) {
            let ret = crate::checker::extern_ret_type(&ext.ret_abi);
            return Ok(Type::FnPtr(sla_ast::types::FnPtrType {
                abi: None,
                params: vec![Type::Infer; ext.move_params.len()],
                ret: Box::new(ret),
            }));
        }
        Err(Diagnostic::new(
            ErrorKind::UndefinedVariable,
            format!("UndefinedVariable: `{name}`"),
        ))
    }

    fn check_field(&mut self, base: &Node, field_name: &str) -> CheckResult<Type> {
        let base_ty = self.check_expr(base)?;
        // `.len` on string-like values is special-cased BEFORE struct
        // lookup in Zig.
        if field_name == "len" {
            let is_string_like = matches!(base_ty, Type::Primitive(Primitive::RawPtr))
                || matches!(&base_ty, Type::UserDefined(u) if u.name == "String" || u.name == "str");
            if is_string_like {
                return Ok(u64_ty());
            }
        }
        let peeled = peel_borrow_pointer(&base_ty);
        match peeled {
            Type::UserDefined(u) => match self.structs.get(&u.name) {
                Some(info) => info.field_ty(field_name).cloned().ok_or_else(|| {
                    Diagnostic::new(
                        ErrorKind::FieldNotFound,
                        format!("FieldNotFound: `{}` has no field `{field_name}`", u.name),
                    )
                }),
                None => Err(Diagnostic::new(
                    ErrorKind::NotAStruct,
                    format!("NotAStruct: {} has no fields", u.name),
                )),
            },
            Type::Tuple(t) => {
                if let Ok(idx) = field_name.parse::<usize>() {
                    if idx < t.elems.len() {
                        return Ok(t.elems[idx].clone());
                    }
                }
                Err(Diagnostic::new(
                    ErrorKind::FieldNotFound,
                    format!("FieldNotFound: tuple has no field `{field_name}`"),
                ))
            }
            other => Err(Diagnostic::new(
                ErrorKind::NotAStruct,
                format!("NotAStruct: field access on {}", type_desc(other)),
            )),
        }
    }

    /// Mirrors the `struct_literal` arm (type_checker.zig:3428):
    /// the type must name a registered struct (else NotAStruct,
    /// opaque included), the update expression must have the same
    /// type, duplicate fields are a CompileError, unknown fields
    /// FieldNotFound, mistyped fields TypeMismatch, unions take
    /// exactly one field, and without an update expression every
    /// field is required (missing -> FieldNotFound). Field VALUES
    /// are not consumed by the checker.
    fn check_struct_literal(
        &mut self,
        lit: &sla_ast::exprs::StructLiteral,
    ) -> CheckResult<Type> {
        let ty = (*lit.ty).clone();
        let name = match &ty {
            Type::UserDefined(u) => u.name.clone(),
            _ => {
                return Err(Diagnostic::new(
                    ErrorKind::NotAStruct,
                    "NotAStruct: struct literal over a non-struct type",
                ))
            }
        };
        let info = match self.structs.get(&name) {
            Some(info) if !info.is_opaque => info.clone(),
            _ => {
                return Err(Diagnostic::new(
                    ErrorKind::NotAStruct,
                    format!("NotAStruct: cannot construct `{name}`"),
                ))
            }
        };
        if let Some(update) = &lit.update_expr {
            let update_ty = self.check_expr(update)?;
            if !types_equal(&update_ty, &ty) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    "struct update expression type mismatch",
                ));
            }
        }
        if info.is_union && lit.fields.len() != 1 {
            return Err(Diagnostic::new(
                ErrorKind::CompileError,
                format!("Union literal must initialize exactly one field: {name}"),
            ));
        }
        let mut seen: Vec<&str> = Vec::new();
        for f in &lit.fields {
            if seen.contains(&f.name.as_str()) {
                return Err(Diagnostic::new(
                    ErrorKind::CompileError,
                    format!("Duplicate field in struct literal: {name}.{}", f.name),
                ));
            }
            seen.push(f.name.as_str());
            let value_ty = self.check_expr(&f.value)?;
            match info.fields.iter().find(|(n, _)| n == &f.name) {
                Some((_, expected)) => {
                    if !value_assignable_to(expected, &value_ty) {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            format!(
                                "TypeMismatch in struct literal field {name}.{}: expected {}, actual {}",
                                f.name,
                                type_desc(expected),
                                type_desc(&value_ty)
                            ),
                        ));
                    }
                }
                None => {
                    return Err(Diagnostic::new(
                        ErrorKind::FieldNotFound,
                        format!(
                            "Field not found in struct literal: {name}.{}",
                            f.name
                        ),
                    ))
                }
            }
        }
        if lit.update_expr.is_none() && !info.is_union {
            for (field_name, _) in &info.fields {
                if !seen.contains(&field_name.as_str()) {
                    return Err(Diagnostic::new(
                        ErrorKind::FieldNotFound,
                        format!("Missing field in struct literal: {name}.{field_name}"),
                    ));
                }
            }
        }
        Ok(ty)
    }
}

/// The fn-ptr type of a signature (identifier arm for function names).
pub fn fn_ptr_of(sig: &crate::checker::FuncSig) -> Type {
    Type::FnPtr(sla_ast::types::FnPtrType {
        abi: None,
        params: sig.params.iter().map(|p| p.ty.clone()).collect(),
        ret: Box::new(sig.ret_ty.clone()),
    })
}

/// Deref peeling: pointers and borrows, plus the smart-pointer /
/// guard user types Zig peels by name (Rc / Arc / Box / guards).
fn peel_pointer(ty: &Type) -> Option<Type> {
    match ty {
        Type::Pointer(inner) | Type::Borrow(inner) => Some((**inner).clone()),
        Type::UserDefined(u) => {
            if matches!(
                u.name.as_str(),
                "Rc" | "Arc" | "Box" | "MutexGuard" | "RwLockReadGuard" | "RwLockWriteGuard"
            ) && !u.generics.is_empty()
            {
                Some(u.generics[0].clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn peel_borrow_pointer(ty: &Type) -> &Type {
    match ty {
        Type::Pointer(inner) | Type::Borrow(inner) => inner,
        other => other,
    }
}

fn cast_allowed(inner: &Type, target: &Type) -> bool {
    if matches!(inner, Type::Infer) || matches!(target, Type::Infer) {
        return true;
    }
    if types_equal(inner, target) {
        return true;
    }
    if is_numeric_type(inner) && is_numeric_type(target) {
        return true;
    }
    let carrier = |t: &Type| {
        matches!(t, Type::Pointer(_) | Type::Borrow(_))
            || matches!(t, Type::Primitive(Primitive::RawPtr))
    };
    carrier(inner) && carrier(target)
}

fn index_result_type(target: &Type) -> Option<Type> {
    match target {
        Type::Array(a) => Some((*a.elem).clone()),
        Type::Pointer(inner) => Some((**inner).clone()),
        Type::UserDefined(u) => {
            if u.name == "String" || u.name == "str" {
                Some(Type::Primitive(Primitive::U8))
            } else if !u.generics.is_empty() {
                Some(u.generics[0].clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Payload of a `?`-propagated value: Result<T, E> / Option<T> -> T;
/// anything else passes through unchanged (as in the Zig arm, which
/// falls back to the inner type for non-wrapper shapes).
fn try_payload_type(inner: &Type) -> Type {
    if let Type::UserDefined(u) = inner {
        if (u.name == "Result" || u.name == "Option") && !u.generics.is_empty() {
            return u.generics[0].clone();
        }
    }
    inner.clone()
}

/// Binary operator typing, mirroring the `binary_expr` arm
/// (type_checker.zig:3215) minus its overload / numeric-struct
/// special cases (out of slice, listed as not-covered).
fn check_binary(op: BinaryOp, left: &Type, right: &Type) -> CheckResult<Type> {
    let mismatch = |what: &str| {
        Diagnostic::new(
            ErrorKind::TypeMismatch,
            format!(
                "TypeMismatch: {what} on {} and {}",
                type_desc(left),
                type_desc(right)
            ),
        )
    };
    match op {
        BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
            if matches!(left, Type::Primitive(Primitive::Boolean))
                && matches!(right, Type::Primitive(Primitive::Boolean))
            {
                Ok(bool_ty())
            } else {
                Err(mismatch("logical operator requires boolean operands"))
            }
        }
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt
        | BinaryOp::Ge => {
            if types_equal(left, right) {
                Ok(bool_ty())
            } else {
                Err(mismatch("comparison requires equal types"))
            }
        }
        BinaryOp::Spaceship => {
            if types_equal(left, right) {
                Ok(Type::UserDefined(sla_ast::types::UserDefined {
                    name: "Ordering".to_string(),
                    generics: Vec::new(),
                }))
            } else {
                Err(mismatch("spaceship requires equal types"))
            }
        }
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
            if is_integer_type(left) && types_equal(left, right) {
                Ok(left.clone())
            } else {
                Err(mismatch("bitwise operator requires integer operands"))
            }
        }
        BinaryOp::Shl | BinaryOp::Shr => {
            if is_integer_type(left) && is_integer_type(right) {
                Ok(left.clone())
            } else {
                Err(mismatch("shift requires integer operands"))
            }
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            if is_numeric_type(left) && types_equal(left, right) {
                Ok(left.clone())
            } else {
                Err(mismatch("arithmetic requires equal numeric types"))
            }
        }
    }
}

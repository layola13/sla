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
    option_inner_type, result_ok_type, root_identifier_deep,
    type_desc, types_equal, u64_ty, value_assignable_to, void_ty,
};
use sla_ast::exprs::Literal;
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
            Node::BinaryExpr(b) => self.check_binary_expr(b),
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
                // Zig try arm (:3562): non-Copy non-borrow identifier
                // sources are consumed FIRST ("try propagation");
                // then Option/Result unwrap with a caller-side
                // cleanup recorded into the shared `cleanups` table;
                // then a struct `value`-field fallback; anything
                // else is NotAStruct.
                let operand_ty = self.check_expr(&t.expr)?;
                if let Node::Identifier(name) = &*t.expr {
                    if !self.type_is_copy(&operand_ty)
                        && !is_borrow_like_type(&operand_ty)
                    {
                        self.consume_binding(name, "try propagation")?;
                    }
                }
                if matches!(operand_ty, Type::Infer) {
                    return Ok(operand_ty);
                }
                if let Some(unwrapped) = option_inner_type(&operand_ty) {
                    self.record_try_cleanup(node, &t.expr);
                    return Ok(unwrapped);
                }
                if let Some(unwrapped) = result_ok_type(&operand_ty) {
                    self.record_try_cleanup(node, &t.expr);
                    return Ok(unwrapped);
                }
                let peeled = peel_borrow_pointer(&operand_ty).clone();
                match &peeled {
                    Type::UserDefined(u) => {
                        let info = match self.structs.get(&u.name) {
                            Some(info) => info,
                            None => {
                                return Err(Diagnostic::new(
                                    ErrorKind::NotAStruct,
                                    format!(
                                        "TypeMismatch: try requires Option, Result, or struct value, actual {}",
                                        type_desc(&operand_ty)
                                    ),
                                ))
                            }
                        };
                        match info.field_ty("value") {
                            Some(value_ty) => {
                                let value_ty = value_ty.clone();
                                self.record_try_cleanup(node, &t.expr);
                                Ok(value_ty)
                            }
                            None => Err(Diagnostic::new(
                                ErrorKind::FieldNotFound,
                                format!(
                                    "TypeMismatch: try requires Option, Result, or struct value, actual {}",
                                    type_desc(&operand_ty)
                                ),
                            )),
                        }
                    }
                    _ => Err(Diagnostic::new(
                        ErrorKind::NotAStruct,
                        format!(
                            "TypeMismatch: try requires Option, Result, or struct value, actual {}",
                            type_desc(&operand_ty)
                        ),
                    )),
                }
            }
            Node::AwaitExpr(a) => {
                // Zig await arm (:3668): the operand must be a
                // future; recordAwaitCleanup runs BEFORE the root
                // consumption, so the awaited binding is still
                // active in the snapshot and is excluded by the
                // escape rule instead.
                let operand_ty = self.check_expr(&a.expr)?;
                let inner = match &operand_ty {
                    Type::Future(payload) => (**payload).clone(),
                    other => {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            format!(
                                "await requires future<T>, actual {}",
                                type_desc(other)
                            ),
                        ))
                    }
                };
                self.record_await_cleanup(node, &a.expr);
                if let Some(root) = root_identifier_deep(&a.expr) {
                    let root = root.to_string();
                    match self.lookup_state(&root) {
                        None => {
                            return Err(Diagnostic::new(
                                ErrorKind::UndefinedVariable,
                                format!("UndefinedVariable: `{root}`"),
                            ))
                        }
                        Some(crate::rules::ValueState::Consumed) => {
                            return Err(Diagnostic::new(
                                ErrorKind::UseAfterMove,
                                format!(
                                    "UseAfterMove: `{root}` was already consumed"
                                ),
                            ))
                        }
                        _ => {}
                    }
                    self.consume_binding(&root, "await")?;
                }
                Ok(inner)
            }
            Node::EnumLiteral(e) => self.check_enum_literal(e),
            Node::SliceExpr(sl) => self.check_slice_expr(sl),
            Node::ClosureLiteral(c) => {
                // Zig closure_literal arm (:4700): parameter types
                // must be annotated — an `infer` parameter is a
                // TypeMismatch with this exact diagnostic (venue
                // probe p_closure_infer, rc=1). Parameters are
                // defined as CONST bindings in a child scope; the
                // body is a single expression whose type is the
                // closure's return type.
                for p in &c.params {
                    if matches!(*p.ty, Type::Infer) {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            "closure parameter types must be inferred from context; standalone inference is not available",
                        ));
                    }
                }
                self.scopes.push_frame();
                for p in &c.params {
                    self.define_symbol_with_state(
                        &p.name,
                        (*p.ty).clone(),
                        true,
                        crate::rules::ValueState::Active,
                        None,
                    )?;
                }
                let body_result = self.check_expr(&c.body);
                self.scopes.pop_frame();
                let ret = body_result?;
                Ok(Type::Closure(sla_ast::types::ClosureType {
                    params: c.params.iter().map(|p| (*p.ty).clone()).collect(),
                    ret: Box::new(ret),
                }))
            }
            Node::IfExpr(i) => self.check_if_expr(i),
            Node::SwitchExpr(s) => self.check_switch(s),
            Node::MatchExpr(m) => self.check_match(m),
            Node::CallExpr(c) => self.check_call(node, c),
            Node::UnsafeExpr(u) => Ok(self.check_block_value(&u.body)?.unwrap_or_else(void_ty)),
            Node::GenericFuncRef(_) => {
                // The Zig arm is a bare `return TypeError.CompileError`
                // (type_checker.zig:3209) with NO setError: a generic
                // function reference that survives monomorphization
                // into the checker is simply rejected, and the
                // caller's wrapper synthesizes the fallback text.
                // Generic INSTANTIATION is the monomorphizer's job,
                // upstream of the checker entirely (D-C11 / FB-012).
                Err(Diagnostic::new(
                    ErrorKind::CompileError,
                    "checkExpr failed at node tag generic_func_ref",
                ))
            }
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

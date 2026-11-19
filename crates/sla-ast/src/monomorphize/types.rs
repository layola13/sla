//! Type-level half of the monomorphizer: specialization,
//! substitution, mangled names, and the shallow call-argument
//! inference — mirrors the corresponding sections of
//! `monomorphizer.zig` (`specializeType`, `substituteType`,
//! `getMangledFuncName` / `getMangledStructName` /
//! `appendTypeName`, `inferGenericArgsForCall`,
//! `inferNodeTypeShallow`, `collectGenericBindings`).
//!
//! Structural type equality is the AST's derived `PartialEq`,
//! which is exactly the Zig `typesStructurallyEqual` recursion
//! (the Rust `FnPtrType` carries no `is_variadic` field, the one
//! field the Zig comparison also ignores).

use super::{MonomorphizeError, Monomorphizer};
use crate::decls::FuncDecl;
use crate::node::Node;
use crate::types::{ArrayType, ClosureType, FnPtrType, Primitive, TupleType, Type, UserDefined};

impl Monomorphizer {
    /// Mirrors Zig `specializeType`: generic user-defined types
    /// that name a struct/enum template are replaced by their
    /// mangled concrete instance (materializing it on first
    /// use); every other shape is rebuilt with specialized
    /// children. Unknown generic names pass through as opaque
    /// types with specialized arguments.
    pub(crate) fn specialize_type(&mut self, ty: Type) -> Result<Type, MonomorphizeError> {
        Ok(match ty {
            Type::Infer | Type::Primitive(_) => ty,
            Type::Pointer(inner) => Type::Pointer(Box::new(self.specialize_type(*inner)?)),
            Type::Borrow(inner) => Type::Borrow(Box::new(self.specialize_type(*inner)?)),
            Type::Array(arr) => Type::Array(ArrayType {
                elem: Box::new(self.specialize_type(*arr.elem)?),
                len: arr.len,
            }),
            Type::Tuple(tuple) => Type::Tuple(TupleType {
                elems: self.specialize_types(tuple.elems)?,
            }),
            Type::Future(inner) => Type::Future(Box::new(self.specialize_type(*inner)?)),
            Type::Closure(closure) => Type::Closure(ClosureType {
                params: self.specialize_types(closure.params)?,
                ret: Box::new(self.specialize_type(*closure.ret)?),
            }),
            Type::FnPtr(fn_ptr) => Type::FnPtr(FnPtrType {
                abi: fn_ptr.abi,
                params: self.specialize_types(fn_ptr.params)?,
                ret: Box::new(self.specialize_type(*fn_ptr.ret)?),
                is_variadic: fn_ptr.is_variadic,
            }),
            Type::UserDefined(ud) => {
                if ud.generics.is_empty() {
                    Type::UserDefined(ud)
                } else {
                    let mut spec_args = Vec::with_capacity(ud.generics.len());
                    for g in &ud.generics {
                        spec_args.push(self.specialize_type(g.clone())?);
                    }
                    if !self.struct_templates.contains_key(&ud.name)
                        && !self.enum_templates.contains_key(&ud.name)
                    {
                        return Ok(Type::UserDefined(UserDefined {
                            name: ud.name,
                            generics: spec_args,
                        }));
                    }
                    // The Zig original mangles and instantiates
                    // from the UNSPECIALIZED argument list (at
                    // every reachable call site the arguments are
                    // already concrete: source types are, and
                    // template bodies are substituted before they
                    // are specialized).
                    let mangled = mangled_name(&ud.name, &ud.generics);
                    if let Some(template) = self.struct_templates.get(&ud.name).cloned() {
                        if !self.specialized_structs.contains(&mangled) {
                            self.instantiate_struct(&mangled, &template, &ud.generics)?;
                        }
                    } else if let Some(template) = self.enum_templates.get(&ud.name).cloned() {
                        if !self.specialized_enums.contains(&mangled) {
                            self.instantiate_enum(&mangled, &template, &ud.generics)?;
                        }
                    } else {
                        return Err(self.template_not_found(&ud.name));
                    }
                    Type::UserDefined(UserDefined {
                        name: mangled,
                        generics: Vec::new(),
                    })
                }
            }
        })
    }

    fn specialize_types(&mut self, tys: Vec<Type>) -> Result<Vec<Type>, MonomorphizeError> {
        tys.into_iter().map(|t| self.specialize_type(t)).collect()
    }

    /// Mirrors Zig `substituteType`: a user-defined type whose
    /// name matches a generic parameter is replaced by the
    /// corresponding argument wholesale; every other shape is
    /// rebuilt with substituted children. Infallible (the Zig
    /// original only fails on allocation).
    pub(crate) fn substitute_type(&self, ty: &Type, params: &[String], args: &[Type]) -> Type {
        match ty {
            Type::UserDefined(ud) => {
                for (param, arg) in params.iter().zip(args.iter()) {
                    if ud.name == *param {
                        return arg.clone();
                    }
                }
                Type::UserDefined(UserDefined {
                    name: ud.name.clone(),
                    generics: ud
                        .generics
                        .iter()
                        .map(|g| self.substitute_type(g, params, args))
                        .collect(),
                })
            }
            Type::Pointer(inner) => {
                Type::Pointer(Box::new(self.substitute_type(inner, params, args)))
            }
            Type::Borrow(inner) => {
                Type::Borrow(Box::new(self.substitute_type(inner, params, args)))
            }
            Type::Array(arr) => Type::Array(ArrayType {
                elem: Box::new(self.substitute_type(&arr.elem, params, args)),
                len: arr.len,
            }),
            Type::Tuple(tuple) => Type::Tuple(TupleType {
                elems: tuple
                    .elems
                    .iter()
                    .map(|e| self.substitute_type(e, params, args))
                    .collect(),
            }),
            Type::Future(inner) => {
                Type::Future(Box::new(self.substitute_type(inner, params, args)))
            }
            Type::Closure(closure) => Type::Closure(ClosureType {
                params: closure
                    .params
                    .iter()
                    .map(|p| self.substitute_type(p, params, args))
                    .collect(),
                ret: Box::new(self.substitute_type(&closure.ret, params, args)),
            }),
            Type::FnPtr(fn_ptr) => Type::FnPtr(FnPtrType {
                abi: fn_ptr.abi.clone(),
                params: fn_ptr
                    .params
                    .iter()
                    .map(|p| self.substitute_type(p, params, args))
                    .collect(),
                is_variadic: fn_ptr.is_variadic,
                ret: Box::new(self.substitute_type(&fn_ptr.ret, params, args)),
            }),
            Type::Infer | Type::Primitive(_) => ty.clone(),
        }
    }

    /// Mirrors Zig `inferGenericArgsForCall`: binds the
    /// template's generic parameters from the shallowly-inferred
    /// argument types; `None` when any parameter cannot be bound
    /// (the call then stays an ordinary call, as in Zig).
    pub(crate) fn infer_generic_args_for_call(
        &self,
        template: &FuncDecl,
        args: &[Node],
    ) -> Option<Vec<Type>> {
        if template.generics.is_empty() || template.params.len() != args.len() {
            return None;
        }
        let mut inferred: Vec<Option<Type>> = vec![None; template.generics.len()];
        for (param, arg) in template.params.iter().zip(args.iter()) {
            let actual_ty = infer_node_type_shallow(arg)?;
            if !collect_generic_bindings(&template.generics, &param.ty, &actual_ty, &mut inferred) {
                return None;
            }
        }
        inferred.into_iter().collect()
    }
}

/// Mirrors Zig `getMangledFuncName` / `getMangledStructName`
/// (the two are identical): `base` + `_` + type-name per type
/// argument.
pub(crate) fn mangled_name(base: &str, generics: &[Type]) -> String {
    let mut name = String::from(base);
    for g in generics {
        name.push('_');
        append_type_name(&mut name, g);
    }
    name
}

/// Mirrors Zig `appendTypeName`.
fn append_type_name(buf: &mut String, ty: &Type) {
    match ty {
        Type::Infer => buf.push_str("infer"),
        Type::Primitive(p) => buf.push_str(match p {
            Primitive::I8 => "i8",
            Primitive::I16 => "i16",
            Primitive::I32 => "i32",
            Primitive::I64 => "i64",
            Primitive::Isize => "isize",
            Primitive::U8 => "u8",
            Primitive::U16 => "u16",
            Primitive::U32 => "u32",
            Primitive::U64 => "u64",
            Primitive::Usize => "usize",
            Primitive::F32 => "f32",
            Primitive::F64 => "f64",
            Primitive::Integer => "int",
            Primitive::Float => "float",
            Primitive::Boolean => "bool",
            Primitive::RawPtr => "ptr",
            Primitive::VaList => "valist",
            Primitive::VoidType => "void",
        }),
        Type::Pointer(inner) => {
            buf.push_str("ptr_");
            append_type_name(buf, inner);
        }
        Type::Borrow(inner) => {
            buf.push_str("ref_");
            append_type_name(buf, inner);
        }
        Type::Array(arr) => {
            buf.push_str("arr");
            buf.push_str(&arr.len.to_string());
            buf.push('_');
            append_type_name(buf, &arr.elem);
        }
        Type::Tuple(tuple) => {
            buf.push_str("tuple_");
            for elem in &tuple.elems {
                append_type_name(buf, elem);
                buf.push('_');
            }
        }
        Type::Future(inner) => {
            buf.push_str("future_");
            append_type_name(buf, inner);
        }
        Type::Closure(closure) => {
            buf.push_str("closure_");
            for p in &closure.params {
                append_type_name(buf, p);
                buf.push('_');
            }
            append_type_name(buf, &closure.ret);
        }
        Type::FnPtr(fn_ptr) => {
            buf.push_str("fnptr_");
            if let Some(abi) = &fn_ptr.abi {
                buf.push_str(abi);
                buf.push('_');
            }
            for p in &fn_ptr.params {
                append_type_name(buf, p);
                buf.push('_');
            }
            append_type_name(buf, &fn_ptr.ret);
        }
        Type::UserDefined(ud) => {
            buf.push_str(&ud.name);
            for g in &ud.generics {
                buf.push('_');
                append_type_name(buf, g);
            }
        }
    }
}

/// Mirrors Zig `inferNodeTypeShallow`: the type of the handful
/// of expression shapes whose type is knowable without the
/// checker (literals, tuple/array literals, struct literals,
/// borrows, moves, casts); `None` for everything else.
fn infer_node_type_shallow(node: &Node) -> Option<Type> {
    match node {
        Node::Literal(lit) => Some(match lit {
            crate::exprs::Literal::IntVal(_) => Type::Primitive(Primitive::I32),
            crate::exprs::Literal::FloatVal(_) => Type::Primitive(Primitive::F64),
            crate::exprs::Literal::BoolVal(_) => Type::Primitive(Primitive::Boolean),
            crate::exprs::Literal::StringVal(_) => Type::UserDefined(UserDefined {
                name: "Slice".to_string(),
                generics: vec![Type::Primitive(Primitive::U8)],
            }),
        }),
        Node::TupleLiteral(tuple) => {
            let mut elems = Vec::with_capacity(tuple.elements.len());
            for elem in &tuple.elements {
                elems.push(infer_node_type_shallow(elem)?);
            }
            Some(Type::Tuple(TupleType { elems }))
        }
        Node::ArrayLiteral(array) => {
            if array.elements.is_empty() {
                return None;
            }
            let elem_ty = infer_node_type_shallow(&array.elements[0])?;
            for elem in &array.elements[1..] {
                if infer_node_type_shallow(elem)? != elem_ty {
                    return None;
                }
            }
            Some(Type::Array(ArrayType {
                elem: Box::new(elem_ty),
                len: array.elements.len(),
            }))
        }
        Node::RepeatArrayLiteral(array) => {
            let elem_ty = infer_node_type_shallow(&array.value)?;
            Some(Type::Array(ArrayType {
                elem: Box::new(elem_ty),
                len: array.len,
            }))
        }
        Node::StructLiteral(lit) => Some((*lit.ty).clone()),
        Node::BorrowExpr(borrow) => {
            Some(Type::Borrow(Box::new(infer_node_type_shallow(&borrow.expr)?)))
        }
        Node::MoveExpr(mv) => infer_node_type_shallow(&mv.expr),
        Node::CastExpr(cast) => Some((*cast.ty).clone()),
        _ => None,
    }
}

/// Mirrors Zig `collectGenericBindings`: matches a template
/// parameter type against an actual argument type, recording
/// generic-parameter bindings; `false` on any structural
/// mismatch.
fn collect_generic_bindings(
    generic_names: &[String],
    template_ty: &Type,
    actual_ty: &Type,
    inferred: &mut [Option<Type>],
) -> bool {
    match template_ty {
        Type::UserDefined(ud) => {
            for (idx, name) in generic_names.iter().enumerate() {
                if ud.name == *name && ud.generics.is_empty() {
                    if let Some(existing) = &inferred[idx] {
                        return existing == actual_ty;
                    }
                    inferred[idx] = Some(actual_ty.clone());
                    return true;
                }
            }
            let Type::UserDefined(actual_ud) = actual_ty else {
                return false;
            };
            if ud.name != actual_ud.name || ud.generics.len() != actual_ud.generics.len() {
                return false;
            }
            ud.generics.iter().zip(actual_ud.generics.iter()).all(|(tg, ag)| {
                collect_generic_bindings(generic_names, tg, ag, inferred)
            })
        }
        Type::Pointer(inner) => {
            let Type::Pointer(actual_inner) = actual_ty else {
                return false;
            };
            collect_generic_bindings(generic_names, inner, actual_inner, inferred)
        }
        Type::Borrow(inner) => {
            let Type::Borrow(actual_inner) = actual_ty else {
                return false;
            };
            collect_generic_bindings(generic_names, inner, actual_inner, inferred)
        }
        Type::Array(arr) => {
            let Type::Array(actual_arr) = actual_ty else {
                return false;
            };
            arr.len == actual_arr.len
                && collect_generic_bindings(generic_names, &arr.elem, &actual_arr.elem, inferred)
        }
        Type::Tuple(tuple) => {
            let Type::Tuple(actual_tuple) = actual_ty else {
                return false;
            };
            tuple.elems.len() == actual_tuple.elems.len()
                && tuple
                    .elems
                    .iter()
                    .zip(actual_tuple.elems.iter())
                    .all(|(te, ae)| collect_generic_bindings(generic_names, te, ae, inferred))
        }
        Type::Primitive(_) => template_ty == actual_ty,
        Type::Future(inner) => {
            let Type::Future(actual_inner) = actual_ty else {
                return false;
            };
            collect_generic_bindings(generic_names, inner, actual_inner, inferred)
        }
        Type::Closure(closure) => {
            let Type::Closure(actual_closure) = actual_ty else {
                return false;
            };
            closure.params.len() == actual_closure.params.len()
                && closure
                    .params
                    .iter()
                    .zip(actual_closure.params.iter())
                    .all(|(tp, ap)| collect_generic_bindings(generic_names, tp, ap, inferred))
                && collect_generic_bindings(generic_names, &closure.ret, &actual_closure.ret, inferred)
        }
        Type::FnPtr(fn_ptr) => {
            let Type::FnPtr(actual_fn_ptr) = actual_ty else {
                return false;
            };
            fn_ptr.abi == actual_fn_ptr.abi
                && fn_ptr.params.len() == actual_fn_ptr.params.len()
                && fn_ptr
                    .params
                    .iter()
                    .zip(actual_fn_ptr.params.iter())
                    .all(|(tp, ap)| collect_generic_bindings(generic_names, tp, ap, inferred))
                && collect_generic_bindings(generic_names, &fn_ptr.ret, &actual_fn_ptr.ret, inferred)
        }
        Type::Infer => true,
    }
}

//! Enum literals and slice expressions, mirroring the
//! `enum_literal` (:4780) and `slice_expr` (:4734) arms of
//! `type_checker.zig`. Split out of check_expr.rs for the
//! 700-line file rule.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{array_type_of, type_desc, types_equal};
use sla_ast::exprs::{EnumLiteral, Literal, SliceExpr};
use sla_ast::node::Node;
use sla_ast::types::Type;

impl Checker {
    pub fn check_enum_literal(&mut self, e: &EnumLiteral) -> CheckResult<Type> {
            // Zig enum_literal arm (:4780). Quirk: an unknown
            // enum name reports NotAStruct; `Option::None` with
            // no fields synthesizes Option<infer> before the
            // declaration lookup.
            if e.enum_name == "Option"
                && e.variant_name == "None"
                && e.fields.is_empty()
            {
                return Ok(Type::UserDefined(sla_ast::types::UserDefined {
                    name: "Option".to_string(),
                    generics: vec![Type::Infer],
                }));
            }
            let variants = match self.enum_variants.get(&e.enum_name) {
                Some(v) => v.clone(),
                None => {
                    return Err(Diagnostic::new(
                        ErrorKind::NotAStruct,
                        format!(
                            "NotAStruct: enum `{}`",
                            e.enum_name
                        ),
                    ))
                }
            };
            let variant = match variants
                .iter()
                .find(|v| v.name == e.variant_name)
            {
                Some(v) => v.clone(),
                None => {
                    return Err(Diagnostic::new(
                        ErrorKind::FieldNotFound,
                        format!(
                            "FieldNotFound: enum {} has no variant {}",
                            e.enum_name, e.variant_name
                        ),
                    ))
                }
            };
            for field in &e.fields {
                let expected = match variant
                    .fields
                    .iter()
                    .find(|(fname, _)| fname == &field.name)
                {
                    Some((_, fty)) => fty.clone(),
                    None => {
                        return Err(Diagnostic::new(
                            ErrorKind::FieldNotFound,
                            format!(
                                "FieldNotFound: enum variant {}::{} has no field {}",
                                e.enum_name, e.variant_name, field.name
                            ),
                        ))
                    }
                };
                let actual = self.check_expr(&field.value)?;
                if !types_equal(&expected, &actual) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: enum field {} expects {}, actual {}",
                            field.name,
                            type_desc(&expected),
                            type_desc(&actual)
                        ),
                    ));
                }
            }
            for (i, field) in e.fields.iter().enumerate() {
                if e.fields[..i].iter().any(|prev| prev.name == field.name) {
                    return Err(Diagnostic::new(
                        ErrorKind::CompileError,
                        format!(
                            "duplicate enum literal field `{}`",
                            field.name
                        ),
                    ));
                }
            }
            if e.fields.len() != variant.fields.len() {
                return Err(Diagnostic::new(
                    ErrorKind::FieldNotFound,
                    format!(
                        "FieldNotFound: enum {}::{} expects {} fields",
                        e.enum_name,
                        e.variant_name,
                        variant.fields.len()
                    ),
                ));
            }
            Ok(Type::UserDefined(sla_ast::types::UserDefined {
                name: e.enum_name.clone(),
                generics: Vec::new(),
            }))
    }

    pub fn check_slice_expr(&mut self, s: &SliceExpr) -> CheckResult<Type> {
            // Zig slice_expr arm (:4734): bounds must be
            // integer LITERALS (expressions are rejected),
            // checked against the array length statically.
            let target_ty = self.check_expr(&s.target)?;
            self.check_expr(&s.start)?;
            self.check_expr(&s.end)?;
            let numeric = |n: &Node| matches!(
                n,
                Node::Literal(Literal::IntVal(_))
                    | Node::Literal(Literal::FloatVal(_))
            );
            if !numeric(&s.start) || !numeric(&s.end) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    "slice bounds must be numeric".to_string(),
                ));
            }
            let (elem, len) = match crate::types_util::array_type_of(
                &target_ty,
            ) {
                Some(pair) => pair,
                None => {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "slice target must be an array, actual {}",
                            type_desc(&target_ty)
                        ),
                    ))
                }
            };
            let lit_int = |n: &Node| -> Option<i64> {
                match n {
                    Node::Literal(Literal::IntVal(v)) => Some(*v),
                    _ => None,
                }
            };
            let (start_i, end_i) = match (lit_int(&s.start), lit_int(&s.end)) {
                (Some(a), Some(b)) => (a, b),
                _ => {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        "slice bounds must be integer literals".to_string(),
                    ))
                }
            };
            if start_i < 0 || end_i < start_i || (end_i as usize) > len {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "slice range out of bounds: {start_i}..{end_i} for len {len}"
                    ),
                ));
            }
            Ok(Type::Array(sla_ast::types::ArrayType {
                elem: Box::new(elem),
                len: (end_i - start_i) as usize,
            }))
        }
}

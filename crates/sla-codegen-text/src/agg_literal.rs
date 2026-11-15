//! Array / repeat-array / tuple literal materialisation
//! (codegen.zig:16245 `genArrayLiteralInto`, :16314
//! `genRepeatArrayLiteralInto`, :16402 `genTupleLiteralInto`),
//! split out of into.rs at the 700-line cap. The typed entry
//! points take the element types from the consuming context
//! (a struct field's declared type): the checker records no
//! expr type for aggregate literals in field position, and
//! the untyped fallback reads integer elements as i64,
//! laying the aggregate out at the wrong stride.

use sla_ast::{ArrayLiteral, Node, RepeatArrayLiteral, TupleLiteral, Type};
use sla_lowering::{call_arg_needs_release, root_identifier};

use crate::codegen::{type_size, type_string, Codegen};
use crate::into::is_heap_handle_type;
use crate::layout;
use crate::{unsupported, Result, TextError};

/// Zig `bulk_array_threshold` (codegen.zig:17).
const BULK_ARRAY_THRESHOLD: usize = 32;

impl<'c> Codegen<'c> {
    /// codegen.zig:16245 `genArrayLiteralInto`.
    pub(crate) fn gen_array_literal_into(
        &mut self,
        target: &str,
        node: &Node,
        lit: &ArrayLiteral,
    ) -> Result<()> {
        if lit.elements.is_empty() {
            return unsupported(
                "empty array literal",
                "codegen.zig:16245 (genArrayLiteralInto)",
            );
        }
        // Element layout comes from the literal's own array type,
        // never from an element expression's type (Zig comment:
        // untyped integer elements default to i64 and would store
        // an annotated [u8; 2] at the wrong stride).
        let elem_ty = match self.expr_type(node) {
            Some(Type::Array(arr)) => (*arr.elem).clone(),
            _ => self
                .expr_type(&lit.elements[0])
                .ok_or_else(|| TextError::MissingType {
                    context: "array literal element type".to_string(),
                })?,
        };
        self.gen_array_literal_into_typed(target, lit, &elem_ty)
    }

    /// The array literal with its element type supplied by the
    /// context (a struct field's declared type): the checker
    /// records no expr type for a field-value literal, and the
    /// element fallback would read untyped integer elements as
    /// i64 — storing an [i32; 2] field at the wrong stride.
    pub(crate) fn gen_array_literal_into_typed(
        &mut self,
        target: &str,
        lit: &ArrayLiteral,
        elem_ty: &Type,
    ) -> Result<()> {
        let elem_ty = elem_ty.clone();
        let elem_size = type_size(&elem_ty);
        let total_size = (elem_size * lit.elements.len()).max(1);
        self.ins(&format!("{target} = alloc {total_size}"));
        for (i, elem) in lit.elements.iter().enumerate() {
            let val_reg = self.gen_expr(elem)?;
            self.store_aggregate_element(target, elem_size * i, elem, &val_reg, &elem_ty)?;
        }
        Ok(())
    }

    /// codegen.zig:16314 `genRepeatArrayLiteralInto`. Scalar
    /// elements evaluate the value once (u8 uses the fill macro;
    /// long fills use the doubling bulk copy); composite elements
    /// evaluate per slot, as Rust semantics require.
    pub(crate) fn gen_repeat_array_literal_into(
        &mut self,
        target: &str,
        node: &Node,
        lit: &RepeatArrayLiteral,
    ) -> Result<()> {
        let arr = match self.expr_type(node) {
            Some(Type::Array(arr)) => arr,
            _ => {
                return Err(TextError::MissingType {
                    context: "repeat array literal type".to_string(),
                })
            }
        };
        let elem_ty = (*arr.elem).clone();
        let len = arr.len;
        let elem_size = type_size(&elem_ty);
        let total_size = (elem_size * len).max(1);
        self.ins(&format!("{target} = alloc {total_size}"));

        let elem_is_scalar = matches!(
            elem_ty,
            Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_)
        );
        if matches!(elem_ty, Type::Primitive(sla_ast::Primitive::U8)) {
            let value_reg = self.gen_expr(&lit.value)?;
            self.ins(&format!(
                "EXPAND SLA_ARRAY_FILL_U8 {target}, {value_reg}, {len}"
            ));
            if call_arg_needs_release(&lit.value) {
                self.emit_release(&value_reg)?;
            }
            return Ok(());
        }
        if elem_is_scalar {
            let value_reg = self.gen_expr(&lit.value)?;
            let elem_ty_str = type_string(&elem_ty);
            if len > BULK_ARRAY_THRESHOLD {
                self.ins(&format!(
                    "store {target}+0, {value_reg} as {elem_ty_str}"
                ));
                let mut filled = 1usize;
                while filled < len {
                    let chunk = filled.min(len - filled);
                    let ptr_reg = self.new_tmp();
                    self.ins(&format!(
                        "{ptr_reg} = ptr_add {target}, {}",
                        filled * elem_size
                    ));
                    self.ins(&format!(
                        "call @sa_mem_copy_nonoverlapping(&{ptr_reg}, &{target}, {})",
                        chunk * elem_size
                    ));
                    self.emit_release(&ptr_reg)?;
                    filled += chunk;
                }
            } else {
                for i in 0..len {
                    self.ins(&format!(
                        "store {target}+{}, {value_reg} as {elem_ty_str}",
                        elem_size * i
                    ));
                }
            }
            if call_arg_needs_release(&lit.value) {
                self.emit_release(&value_reg)?;
            }
            return Ok(());
        }
        for i in 0..len {
            let slot_reg = self.gen_expr(&lit.value)?;
            self.store_aggregate_element(target, elem_size * i, &lit.value, &slot_reg, &elem_ty)?;
        }
        Ok(())
    }

    /// codegen.zig:16402 `genTupleLiteralInto`.
    pub(crate) fn gen_tuple_literal_into(
        &mut self,
        target: &str,
        lit: &TupleLiteral,
    ) -> Result<()> {
        let mut elem_tys: Vec<Type> = Vec::with_capacity(lit.elements.len());
        for elem in &lit.elements {
            elem_tys.push(self.expr_type(elem).ok_or_else(|| TextError::MissingType {
                context: "tuple literal element type".to_string(),
            })?);
        }
        self.gen_tuple_literal_into_typed(target, lit, &elem_tys)
    }

    /// The tuple literal with element types supplied by the
    /// context (a struct field's declared type): in field
    /// position the elements' recorded types default untyped
    /// integers to i64, which lays the tuple out at word stride
    /// while every reader uses the declared packed layout.
    pub(crate) fn gen_tuple_literal_into_typed(
        &mut self,
        target: &str,
        lit: &TupleLiteral,
        elem_tys: &[Type],
    ) -> Result<()> {
        let elem_tys: Vec<Type> = elem_tys.to_vec();
        let tuple = sla_ast::TupleType { elems: elem_tys.clone() };
        self.ins(&format!(
            "{target} = alloc {}",
            sla_lowering::tuple_abi_size(&tuple)
        ));
        for (i, elem) in lit.elements.iter().enumerate() {
            let (offset, elem_ty) =
                layout::tuple_field_layout(&elem_tys, i).ok_or_else(|| TextError::Unsupported {
                    feature: format!("tuple field layout {i}"),
                    zig_ref: "lowering_rules.zig (tupleFieldLayout)",
                })?;
            let val_reg = self.gen_expr(elem)?;
            self.ins(&format!(
                "store {target}+{offset}, {val_reg} as {}",
                type_string(&elem_ty)
            ));
            // MOVEVEC-1 / DEF-TUPLE-COPY-EMIT: a fresh owning
            // element temp transfers to the aggregate (forget);
            // identifier elements keep their binding bookkeeping
            // to the checker cleanup lists, as in the Zig port's
            // markMovedExprBinding discipline.
            if root_identifier(elem).is_none() && is_heap_handle_type(&elem_ty) {
                self.emit_forget_moved_value(&val_reg)?;
            }
        }
        Ok(())
    }
}

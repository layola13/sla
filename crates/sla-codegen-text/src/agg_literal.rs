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
            // R2b (stage 6): a nested literal element takes its
            // layout from THIS literal's element type (the outer
            // annotation), not from its own recorded type.
            let val_reg = self.gen_expr_in_context(elem, Some(&elem_ty))?;
            self.store_aggregate_element(target, elem_size * i, elem, &val_reg, &elem_ty)?;
        }
        Ok(())
    }

    /// R2b (stage 6): emit `expr` where the consuming context
    /// expects `ctx_ty`. An array / repeat literal directly in
    /// this position takes its element layout from the context
    /// (Zig takes the element type from the annotation recorded
    /// for the whole node, codegen.zig:16365-16371); the literal's
    /// own recorded type defaults bare integer elements to i64
    /// and would lay a `[u8; N]` / `[i32; N]` consumer out at the
    /// wrong stride. Every other shape falls through to
    /// `gen_expr` unchanged. A borrow context peels to its inner
    /// array type.
    pub(crate) fn gen_expr_in_context(
        &mut self,
        expr: &Node,
        ctx_ty: Option<&Type>,
    ) -> Result<String> {
        let arr = match ctx_ty {
            Some(Type::Array(a)) => Some(a.clone()),
            Some(Type::Borrow(inner)) => match &**inner {
                Type::Array(a) => Some(a.clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(arr) = arr {
            match expr {
                Node::ArrayLiteral(lit) => {
                    let reg = self.new_tmp();
                    self.gen_array_literal_into_typed(&reg, lit, &arr.elem)?;
                    return Ok(reg);
                }
                Node::RepeatArrayLiteral(lit) => {
                    let reg = self.new_tmp();
                    self.gen_repeat_array_literal_into_typed(
                        &reg, lit, &arr.elem, arr.len,
                    )?;
                    return Ok(reg);
                }
                _ => {}
            }
        }
        self.gen_expr(expr)
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
        self.gen_repeat_array_literal_into_typed(target, lit, &elem_ty, len)
    }

    /// The repeat literal with its element type and length
    /// supplied by the consuming context (R2b stage 6: an
    /// annotated let's type, a function's return type, a struct
    /// field's declared type). The literal's own recorded type
    /// defaults a bare integer value to i64 — and in field
    /// position may not be recorded at all — so context-free
    /// emission lays the fill out at the wrong stride or fails
    /// with MissingType.
    pub(crate) fn gen_repeat_array_literal_into_typed(
        &mut self,
        target: &str,
        lit: &RepeatArrayLiteral,
        elem_ty: &Type,
        len: usize,
    ) -> Result<()> {
        let elem_ty = elem_ty.clone();
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
            // A nested literal value takes its layout from the
            // element type, as in the plain literal's typed path.
            let slot_reg = self.gen_expr_in_context(&lit.value, Some(&elem_ty))?;
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
        // SPRINT-100 L2: sibling frame over the elements, as in
        // gen_struct_literal_into (Zig `current_expr_later_nodes`).
        self.push_expr_later_frame(
            lit.elements.iter().map(|e| e as *const Node).collect(),
        );
        let mut elem_result: Result<()> = Ok(());
        for (i, elem) in lit.elements.iter().enumerate() {
            self.set_expr_later_frame_index(i);
            let (offset, elem_ty) =
                layout::tuple_field_layout(&elem_tys, i).ok_or_else(|| TextError::Unsupported {
                    feature: format!("tuple field layout {i}"),
                    zig_ref: "lowering_rules.zig (tupleFieldLayout)",
                })?;
            let val_reg = match self.gen_expr(elem) {
                Ok(r) => r,
                Err(e) => {
                    elem_result = Err(e);
                    break;
                }
            };
            self.ins(&format!(
                "store {target}+{offset}, {val_reg} as {}",
                type_string(&elem_ty)
            ));
            // MOVEVEC-1 / DEF-TUPLE-COPY-EMIT: a fresh owning
            // element temp transfers to the aggregate (forget);
            // identifier elements keep their binding bookkeeping
            // to the checker cleanup lists, as in the Zig port's
            // markMovedExprBinding discipline.
            // L4R2 Root T: the forget gate was the fixed std-name
            // list is_heap_handle_type, so a fresh temp of a plain
            // user struct (or a nested tuple) stored into the
            // aggregate was never retired and leaked at the
            // owner's return. The array path's store discipline
            // (store_aggregate_element) consumes every non-Copy
            // non-identifier temp; the tuple gate is the same
            // predicate in forget form: non-identifier root,
            // non-Copy, pointer-ABI (owning), non-borrow.
            if root_identifier(elem).is_none()
                && !self.type_is_copy_value(&elem_ty)
                && sla_lowering::abi_passes_as_pointer(&elem_ty)
                && !sla_lowering::is_borrow_like_type(&elem_ty)
            {
                self.emit_forget_moved_value(&val_reg)?;
            }
            // LINE-CENSUS-MEMLEAK: the forget gate above covers
            // only non-Copy temps; a Copy element temp (a POD
            // struct literal, a scalar literal/call result) is
            // settled with the struct-field Copy arm's predicate
            // once its value lives in the tuple record
            // (acl_mk_tuple stranded both shapes).
            if self.type_is_copy_value(&elem_ty)
                && self.struct_field_temp_needs_release(elem, &val_reg)
            {
                self.emit_release(&val_reg)?;
            }
            // A bare/Move identifier element of a non-Copy type
            // was consumed into the tuple by the checker; record
            // the whole-binding move so a later cleanup retires
            // only the husk (slot_pool's `return (q, idx)` freed
            // the returned pool's fields without it).
            self.mark_binding_moved_into_aggregate(elem, &elem_ty);
        }
        self.pop_expr_later_frame();
        elem_result?;
        Ok(())
    }
}

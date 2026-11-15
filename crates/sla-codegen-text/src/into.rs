//! Aggregate literal materialisation (the `gen*LiteralInto`
//! family, codegen.zig:16051-16520) plus the field-access and
//! borrow expression arms of `genExpr` (codegen.zig:18676,
//! 18987). Mirrors the Zig emission shapes: an Into target is a
//! fresh `alloc` whose fields/elements are stored at layout
//! offsets. Transfer rules follow the Zig branches ported here:
//! Copy values store plainly and release their temp, non-Copy
//! temps and identifier values move in with a `^` store, heap
//! handles retained by an aggregate are consumed, not released.
//! Sub-shapes whose Zig arms depend on unported machinery
//! (Copy-struct deep copy, inline-POD element images, struct
//! update expressions, field/index projections in borrows) stay
//! loud.

use sla_ast::{
    ArrayLiteral, EnumLiteral, Literal, Node, RepeatArrayLiteral, StructDecl, StructLiteral,
    TupleLiteral, Type,
};
use sla_lowering::{call_arg_needs_release, peel_borrow_pointer_type, root_identifier};

use crate::codegen::{type_size, type_string, Codegen};
use crate::layout;
use crate::{unsupported, Result, TextError};

/// Zig `bulk_array_threshold` (codegen.zig:17).
const BULK_ARRAY_THRESHOLD: usize = 32;

/// The collection/handle family whose values are heap handles
/// retained by an aggregate (Zig `isHeapHandleType` approximation
/// over the names this emitter's surface/release layers already
/// special-case).
fn is_heap_handle_type(ty: &Type) -> bool {
    match ty {
        Type::UserDefined(u) => matches!(
            u.name.as_str(),
            "Vec"
                | "GrowVec"
                | "String"
                | "HashMap"
                | "HashSet"
                | "BTreeMap"
                | "BTreeSet"
                | "VecDeque"
                | "Box"
                | "Rc"
                | "Arc"
        ),
        _ => false,
    }
}

/// Strip a trailing `ns.` / `ns::` qualifier (Zig
/// `userDefinedLocalName`).
fn local_name(name: &str) -> &str {
    let dot = name.rfind('.').map(|i| i + 1).unwrap_or(0);
    let colon = name.rfind("::").map(|i| i + 2).unwrap_or(0);
    let start = dot.max(colon);
    if start == 0 || start >= name.len() {
        name
    } else {
        &name[start..]
    }
}

impl<'c> Codegen<'c> {
    fn struct_decl_for_name(&self, name: &str) -> Option<StructDecl> {
        if let Some(decl) = self.check.struct_decl(name) {
            return Some(decl.clone());
        }
        let local = local_name(name);
        if local != name {
            return self.check.struct_decl(local).cloned();
        }
        None
    }

    /// One aggregate element store, shared by the array literal
    /// and repeat-array emitters (Zig's per-element branch in
    /// `genArrayLiteralInto`): an owned non-Copy temp moves in
    /// with a consume store; anything else stores plainly, with
    /// heap-handle temps consumed and Copy temps released.
    fn store_aggregate_element(
        &mut self,
        target: &str,
        offset: usize,
        elem_node: &Node,
        val_reg: &str,
        elem_ty: &Type,
    ) -> Result<()> {
        let ty_str = type_string(elem_ty);
        let is_ident = matches!(elem_node, Node::Identifier(_));
        let move_elem = !is_ident
            && !val_reg.starts_with('&')
            && !self.type_is_copy_value(elem_ty);
        if move_elem {
            let mv = if val_reg.starts_with('^') {
                val_reg.to_string()
            } else {
                format!("^{val_reg}")
            };
            self.ins(&format!("store {target}+{offset}, {mv} as {ty_str}"));
            return Ok(());
        }
        self.ins(&format!("store {target}+{offset}, {val_reg} as {ty_str}"));
        if is_heap_handle_type(elem_ty) && !is_ident {
            self.mark_consumed_binding(val_reg);
        } else if call_arg_needs_release(elem_node) {
            self.emit_release(val_reg)?;
        }
        Ok(())
    }

    /// codegen.zig:16051 `genStructLiteralInto` (direct + move
    /// transfer arms; the deep-copy arm stays loud with
    /// `gen_copy_value_into`).
    pub(crate) fn gen_struct_literal_into(
        &mut self,
        target: &str,
        lit: &StructLiteral,
    ) -> Result<()> {
        if lit.update_expr.is_some() {
            return unsupported(
                "struct literal update expression",
                "codegen.zig:16051 (genStructLiteralInto update arm)",
            );
        }
        let type_name = match &*lit.ty {
            Type::UserDefined(u) => u.name.clone(),
            _ => {
                return unsupported(
                    "struct literal of non-user-defined type",
                    "codegen.zig:16051 (genStructLiteralInto)",
                )
            }
        };
        let decl = self.struct_decl_for_name(&type_name).ok_or_else(|| {
            TextError::Unsupported {
                feature: format!("struct literal: no struct decl for {type_name}"),
                zig_ref: "codegen.zig:16051 (genStructLiteralInto)",
            }
        })?;
        self.ins(&format!("{target} = alloc {}", layout::struct_size(&decl)));
        for decl_field in &decl.fields {
            let value = lit
                .fields
                .iter()
                .find(|f| f.name == decl_field.name)
                .map(|f| &*f.value)
                .ok_or_else(|| TextError::Unsupported {
                    feature: format!(
                        "struct literal missing field value `{}`",
                        decl_field.name
                    ),
                    zig_ref: "codegen.zig:16051 (genStructLiteralInto)",
                })?;
            let (offset, field_ty) =
                layout::struct_field_layout(&decl, &decl_field.name).ok_or_else(|| {
                    TextError::Unsupported {
                        feature: format!("struct field layout `{}`", decl_field.name),
                        zig_ref: "lowering_rules.zig (structFieldLayout)",
                    }
                })?;
            if self.check.type_is_copy_struct(&field_ty) {
                return unsupported(
                    "struct literal Copy-struct field (deep copy)",
                    "codegen.zig:16051 (genStructLiteralInto deep-copy arm)",
                );
            }
            let val_reg = self.gen_expr(value)?;
            let ty_str = type_string(&field_ty);
            if self.type_is_copy_value(&field_ty) {
                self.ins(&format!(
                    "store {target}+{offset}, {val_reg} as {ty_str}"
                ));
                if call_arg_needs_release(value) {
                    self.emit_release(&val_reg)?;
                }
            } else if val_reg.starts_with('&') || val_reg.starts_with('^') {
                self.ins(&format!(
                    "store {target}+{offset}, {val_reg} as {ty_str}"
                ));
            } else {
                // Identifier bindings and fresh owned temps alike
                // move into the aggregate.
                self.ins(&format!(
                    "store {target}+{offset}, ^{val_reg} as {ty_str}"
                ));
            }
        }
        Ok(())
    }

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

    /// codegen.zig:16447 `genEnumLiteralInto`: discriminant at
    /// offset 0, payload fields at their layout offsets, plain
    /// stores (the Zig arm has no per-field transfer handling).
    pub(crate) fn gen_enum_literal_into(
        &mut self,
        target: &str,
        lit: &EnumLiteral,
    ) -> Result<()> {
        if lit.enum_name == "Option" && lit.variant_name == "None" && lit.fields.is_empty() {
            self.ins(&format!("EXPAND OPTION_NEW_NONE {target}"));
            return Ok(());
        }
        let decl = self
            .enum_decl(&lit.enum_name)
            .cloned()
            .ok_or_else(|| TextError::Unsupported {
                feature: format!("enum literal: no enum decl for {}", lit.enum_name),
                zig_ref: "codegen.zig:16447 (genEnumLiteralInto)",
            })?;
        let tag = layout::enum_variant_value(&decl, &lit.variant_name).ok_or_else(|| {
            TextError::Unsupported {
                feature: format!("enum variant {}::{}", lit.enum_name, lit.variant_name),
                zig_ref: "lowering_rules.zig (enumVariantValue)",
            }
        })?;
        let variant = decl
            .variants
            .iter()
            .find(|v| v.name == lit.variant_name)
            .ok_or_else(|| TextError::Unsupported {
                feature: format!("enum variant {}::{}", lit.enum_name, lit.variant_name),
                zig_ref: "lowering_rules.zig (enumVariant)",
            })?;
        self.ins(&format!("{target} = alloc {}", layout::enum_abi_size(&decl)));
        self.ins(&format!("store {target}+0, {tag} as i64"));
        for field in &variant.fields {
            let value = lit
                .fields
                .iter()
                .find(|f| f.name == field.name)
                .map(|f| &*f.value)
                .ok_or_else(|| TextError::Unsupported {
                    feature: format!("enum literal missing field value `{}`", field.name),
                    zig_ref: "codegen.zig:16447 (genEnumLiteralInto)",
                })?;
            let (offset, field_ty) =
                layout::enum_field_layout(variant, &field.name).ok_or_else(|| {
                    TextError::Unsupported {
                        feature: format!("enum field layout `{}`", field.name),
                        zig_ref: "lowering_rules.zig (enumFieldLayout)",
                    }
                })?;
            let val_reg = self.gen_expr(value)?;
            self.ins(&format!(
                "store {target}+{offset}, {val_reg} as {}",
                type_string(&field_ty)
            ));
        }
        Ok(())
    }

    /// codegen.zig:18987 `field_expr` arm of genExpr: load the
    /// field at its layout offset (a ManuallyDrop field yields
    /// its slot address instead), with the DEF-CHAIN load-through
    /// for a raw-pointer base.
    pub(crate) fn gen_field_expr(&mut self, expr: &Node) -> Result<String> {
        let field = match expr {
            Node::FieldExpr(f) => f,
            _ => unreachable!(),
        };
        if let Node::Literal(Literal::StringVal(_)) = &*field.expr {
            if field.field_name == "len" {
                let inner = self.gen_expr(&field.expr)?;
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND STRING_LEN {reg}, {inner}"));
                return Ok(reg);
            }
        }
        let inner = self.gen_expr(&field.expr)?;
        let base_ty = self.require_expr_type(&field.expr, "field base type")?;
        let (base, through) = if matches!(base_ty, Type::Pointer(_)) {
            let obj = self.new_tmp();
            self.ins(&format!("{obj} = load {inner}+0 as ptr"));
            (obj.clone(), Some(obj))
        } else {
            (inner.clone(), None)
        };
        let peeled = peel_borrow_pointer_type(&base_ty).clone();
        let reg = self.new_tmp();
        match &peeled {
            Type::Tuple(tuple) => {
                let index: usize = field.field_name.parse().map_err(|_| {
                    TextError::Unsupported {
                        feature: format!("tuple field name `{}`", field.field_name),
                        zig_ref: "codegen.zig:18987 (field_expr tuple arm)",
                    }
                })?;
                let (offset, elem_ty) = layout::tuple_field_layout(&tuple.elems, index)
                    .ok_or_else(|| TextError::Unsupported {
                        feature: format!("tuple field layout {index}"),
                        zig_ref: "lowering_rules.zig (tupleFieldLayout)",
                    })?;
                self.ins(&format!(
                    "{reg} = load {base}+{offset} as {}",
                    type_string(&elem_ty)
                ));
            }
            Type::UserDefined(u) => {
                let decl = self.struct_decl_for_name(&u.name).ok_or_else(|| {
                    TextError::Unsupported {
                        feature: format!("field access: no struct decl for {}", u.name),
                        zig_ref: "codegen.zig:18987 (field_expr)",
                    }
                })?;
                let (offset, field_ty) = layout::struct_field_layout(&decl, &field.field_name)
                    .ok_or_else(|| TextError::Unsupported {
                        feature: format!("struct field layout `{}`", field.field_name),
                        zig_ref: "lowering_rules.zig (structFieldLayout)",
                    })?;
                if matches!(&field_ty, Type::UserDefined(fu) if local_name(&fu.name) == "ManuallyDrop") {
                    self.ins(&format!("{reg} = ptr_add {base}, {offset}"));
                } else {
                    self.ins(&format!(
                        "{reg} = load {base}+{offset} as {}",
                        type_string(&field_ty)
                    ));
                }
            }
            _ => {
                return unsupported(
                    "field access on non-aggregate base",
                    "codegen.zig:18987 (field_expr)",
                )
            }
        }
        if self.expr_result_reg_needs_release(&field.expr) {
            self.emit_release(&inner)?;
        }
        if let Some(obj) = through {
            self.emit_release(&obj)?;
        }
        Ok(reg)
    }

    /// codegen.zig:18676 `borrow_expr` arm of genExpr, identifier
    /// and deref shapes: an assigned-value slot loads its pointer
    /// back out, an addressable binding materialises its address
    /// with `ptr_add +0`, any other binding borrows as `&name`;
    /// `&*p` is `p` itself. Field/index projections stay loud.
    pub(crate) fn gen_borrow_expr(&mut self, expr: &Node) -> Result<String> {
        let borrow = match expr {
            Node::BorrowExpr(b) => b,
            _ => unreachable!(),
        };
        match &*borrow.expr {
            Node::Identifier(name) => {
                let resolved = self.resolve_binding_name(name);
                if self.assigned_value_slots.contains(&resolved) {
                    let addr = self.new_tmp();
                    self.ins(&format!("{addr} = load {resolved}+0 as ptr"));
                    return Ok(addr);
                }
                if self.addressable_bindings.contains(name)
                    || self.addressable_bindings.contains(&resolved)
                {
                    let addr = self.new_tmp();
                    self.ins(&format!("{addr} = ptr_add {resolved}, 0"));
                    return Ok(addr);
                }
                Ok(format!("&{resolved}"))
            }
            Node::DerefExpr(d) => self.gen_expr(&d.expr),
            _ => unsupported(
                "borrow of field/index projection",
                "codegen.zig:18676 (genExpr borrow_expr projections)",
            ),
        }
    }
}

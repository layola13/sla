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

/// The collection/handle family whose values are heap handles
/// retained by an aggregate (Zig `isHeapHandleType` approximation
/// over the names this emitter's surface/release layers already
/// special-case).
pub(crate) fn is_heap_handle_type(ty: &Type) -> bool {
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
    pub(crate) fn store_aggregate_element(
        &mut self,
        target: &str,
        offset: usize,
        elem_node: &Node,
        val_reg: &str,
        elem_ty: &Type,
    ) -> Result<()> {
        let ty_str = type_string(elem_ty);
        let is_ident = matches!(elem_node, Node::Identifier(_));
        if crate::place::inline_pod_element(self.check, elem_ty) {
            // CLASSB segment 3 (Zig genArrayLiteralInto inlinePod
            // arm, codegen.zig:16281): an inline-POD element's slot
            // holds the packed record image, not a record pointer
            // (the read side addresses the slot as the record).
            // Storing the pointer here made every field read of a
            // struct array element consume pointer bits as record
            // data. An identifier element's record is only read
            // for its word; the binding stays live.
            let word = self.new_tmp();
            self.ins(&format!("{word} = load {val_reg}+0 as u64"));
            self.ins(&format!("store {target}+{offset}, {word} as u64"));
            self.emit_forget_moved_value(&word)?;
            if !is_ident {
                self.emit_forget_moved_value(val_reg)?;
            }
            return Ok(());
        }
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
        // Update arm (Zig genStructLiteralInto :15978): fields
        // the literal does not name are filled from the update
        // base — plain fields by direct word transfer,
        // Copy-struct fields by deep copy, other pointer-backed
        // fields by move out of the base record.
        if let Some(update_expr) = &lit.update_expr {
            let update_reg = self.gen_expr(update_expr)?;
            for decl_field in &decl.fields {
                if lit.fields.iter().any(|f| f.name == decl_field.name) {
                    continue;
                }
                let (offset, field_ty) = layout::struct_field_layout(&decl, &decl_field.name)
                    .ok_or_else(|| TextError::Unsupported {
                        feature: format!("struct field layout `{}`", decl_field.name),
                        zig_ref: "lowering_rules.zig (structFieldLayout)",
                    })?;
                let loaded = self.new_tmp();
                self.ins(&format!(
                    "{loaded} = load {update_reg}+{offset} as {}",
                    type_string(&field_ty)
                ));
                let pointer_backed = !matches!(
                    field_ty,
                    Type::Infer
                        | Type::Primitive(_)
                        | Type::Pointer(_)
                        | Type::Borrow(_)
                        | Type::FnPtr(_)
                );
                if !pointer_backed {
                    self.ins(&format!(
                        "store {target}+{offset}, {loaded} as {}",
                        type_string(&field_ty)
                    ));
                    if call_arg_needs_release(update_expr) {
                        self.emit_release(&loaded)?;
                    }
                } else if self.check.type_is_copy_struct(&field_ty) {
                    let copied = self.new_tmp();
                    self.gen_copy_value_into(&copied, &loaded, &field_ty)?;
                    self.ins(&format!(
                        "store {target}+{offset}, ^{copied} as {}",
                        type_string(&field_ty)
                    ));
                    if call_arg_needs_release(update_expr) {
                        self.emit_release(&loaded)?;
                    }
                } else {
                    self.ins(&format!(
                        "store {target}+{offset}, ^{loaded} as {}",
                        type_string(&field_ty)
                    ));
                }
            }
            if call_arg_needs_release(update_expr) {
                self.emit_release(&update_reg)?;
            }
        }
        for decl_field in &decl.fields {
            let value = lit
                .fields
                .iter()
                .find(|f| f.name == decl_field.name)
                .map(|f| &*f.value);
            let value = match value {
                Some(v) => v,
                None if lit.update_expr.is_some() => continue,
                None => {
                    return Err(TextError::Unsupported {
                        feature: format!(
                            "struct literal missing field value `{}`",
                            decl_field.name
                        ),
                        zig_ref: "codegen.zig:16051 (genStructLiteralInto)",
                    })
                }
            };
            let (offset, field_ty) =
                layout::struct_field_layout(&decl, &decl_field.name).ok_or_else(|| {
                    TextError::Unsupported {
                        feature: format!("struct field layout `{}`", decl_field.name),
                        zig_ref: "lowering_rules.zig (structFieldLayout)",
                    }
                })?;
            if self.check.type_is_copy_struct(&field_ty) {
                // Deep-copy arm: the field value is copied into
                // a fresh record and the copy's pointer is stored;
                // the source value keeps its own ownership.
                let val_reg = self.gen_expr(value)?;
                let copied = self.new_tmp();
                self.gen_copy_value_into(&copied, &val_reg, &field_ty)?;
                self.ins(&format!(
                    "store {target}+{offset}, {copied} as {}",
                    type_string(&field_ty)
                ));
                if call_arg_needs_release(value) {
                    self.emit_release(&val_reg)?;
                }
                continue;
            }
            // An array-literal field value takes its element
            // type from the FIELD's declared type (the checker
            // records no expr type for the literal in this
            // position; the untyped fallback stores i64-wide).
            if let (Node::ArrayLiteral(alit), Type::Array(arr)) = (value, &field_ty) {
                let arr = arr.clone();
                let temp = self.new_tmp();
                self.gen_array_literal_into_typed(&temp, alit, &arr.elem)?;
                self.ins(&format!(
                    "store {target}+{offset}, ^{temp} as {}",
                    type_string(&field_ty)
                ));
                continue;
            }
            // R2b (stage 6): a repeat-literal field value takes
            // its element type and length from the FIELD's
            // declared type, like the plain literal above (the
            // context-free path reads the literal's own recorded
            // type — i64-wide for a bare integer value — or fails
            // with MissingType when none was recorded).
            if let (Node::RepeatArrayLiteral(rlit), Type::Array(arr)) = (value, &field_ty) {
                let arr = arr.clone();
                let temp = self.new_tmp();
                self.gen_repeat_array_literal_into_typed(
                    &temp, rlit, &arr.elem, arr.len,
                )?;
                self.ins(&format!(
                    "store {target}+{offset}, ^{temp} as {}",
                    type_string(&field_ty)
                ));
                continue;
            }
            if let (Node::TupleLiteral(tlit), Type::Tuple(tt)) = (value, &field_ty) {
                let tt = tt.clone();
                let temp = self.new_tmp();
                self.gen_tuple_literal_into_typed(&temp, tlit, &tt.elems)?;
                self.ins(&format!(
                    "store {target}+{offset}, ^{temp} as {}",
                    type_string(&field_ty)
                ));
                continue;
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
            // R2b (stage 6): a literal payload takes its layout
            // from the variant field's declared type.
            let val_reg = self.gen_expr_in_context(value, Some(&field_ty))?;
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
    /// `&*p` is `p` itself. Field/index projections go through
    /// the place-address machinery (place.rs).
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
            Node::FieldExpr(f) => {
                // codegen.zig:18463 borrow arm, `.field` shape: a
                // smart-pointer field's borrow is the field value
                // itself; a pointer-backed field (lowering_rules
                // structFieldIsPointerBacked: user-defined, tuple
                // and array types — the field word IS the value's
                // pointer, Vec/GrowVec handles included) borrows
                // the loaded word; every other field borrows its
                // slot address.
                let field_ty = self.require_expr_type(&borrow.expr, "borrow field type")?;
                if matches!(
                    sla_lowering::peel_borrow_pointer_type(&field_ty),
                    Type::UserDefined(u) if matches!(u.name.as_str(), "Box" | "Rc" | "Arc")
                ) {
                    return self.gen_expr(&borrow.expr);
                }
                let projection = self.gen_field_address(f)?;
                // structFieldIsPointerBacked: false only for
                // Infer/Primitive/Pointer/Borrow/FnPtr.
                if !matches!(
                    field_ty,
                    Type::Infer
                        | Type::Primitive(_)
                        | Type::Pointer(_)
                        | Type::Borrow(_)
                        | Type::FnPtr(_)
                ) {
                    let loaded = self.new_tmp();
                    self.ins(&format!("{loaded} = load {}+0 as ptr", projection.ptr));
                    self.emit_release(&projection.ptr)?;
                    if let Some(source_temp) = &projection.source_temp {
                        self.borrow_source_temps
                            .insert(loaded.clone(), source_temp.clone());
                    }
                    return Ok(loaded);
                }
                self.remember_address_projection_source(&projection);
                Ok(projection.ptr)
            }
            Node::IndexExpr(idx) => {
                // codegen.zig:18463 borrow arm, `.index` shape
                // (CLASSB-DEF2): an element's borrow is its slot
                // address through genIndexAddress, for Vec/GrowVec,
                // slice and array targets alike — a heap-handle
                // element borrows the slot holding the handle word
                // (the consumer loads the word itself; returning
                // the loaded handle instead breaks the call-site
                // capability shape, CapabilityMismatch on
                // test_unit_index_borrow_handle_elem). The slot
                // base is NOT released here: the address escapes
                // with the source container's lifetime, and
                // releasing the base first makes the verifier
                // reject the result as UseAfterMove (the
                // DEF-SLAREDIS-BUILD-MAIN ordering hazard;
                // test_unit_diag_trio_param_borrows). Instead the
                // slot base is chained into the borrow-source
                // table under the address register (and the
                // container source under the slot base), so
                // releasing the borrow result settles the whole
                // chain in finishIndexAddress order — which is
                // also what keeps loop back-edges in agreement
                // (PhiStateConflict on the unreleased base).
                let address = self.gen_index_address(idx)?;
                if let Some(slot_base) = &address.slot_base_reg {
                    self.borrow_source_temps
                        .insert(address.ptr.clone(), slot_base.clone());
                    let container_source = address.base_tmp.clone().or_else(|| {
                        if address.release_base_reg {
                            Some(address.base_reg.clone())
                        } else {
                            None
                        }
                    });
                    if let Some(source) = container_source {
                        self.borrow_source_temps.insert(slot_base.clone(), source);
                    }
                } else {
                    self.remember_index_address_source(&address);
                }
                // A heap-handle element's borrow is the handle
                // word stored in the slot (the DEF-SLAREDIS shape):
                // a &Vec parameter reads the handle's fields
                // directly, so the borrow must be the loaded
                // handle, not the slot address. The loaded
                // register chains to the slot address in the
                // borrow-source table, so releasing it settles
                // the chain. An inline-POD element lives in the
                // slot and keeps the slot address.
                let target_ty = self.require_expr_type(&idx.target, "borrow index target type")?;
                if let Some((_, elem_ty)) = crate::place::vec_like_element_type(&target_ty) {
                    if is_heap_handle_type(&elem_ty)
                        && !crate::place::inline_pod_element(self.check, &elem_ty)
                    {
                        let rec = self.new_tmp();
                        self.ins(&format!("{rec} = load {}+0 as ptr", address.ptr));
                        self.borrow_source_temps
                            .insert(rec.clone(), address.ptr.clone());
                        return Ok(rec);
                    }
                }
                Ok(address.ptr)
            }
            _ => unsupported(
                "borrow of field/index projection",
                "codegen.zig:18676 (genExpr borrow_expr projections)",
            ),
        }
    }
}

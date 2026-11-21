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
use sla_lowering::{
    call_arg_needs_release, is_temporary_register_name, peel_borrow_pointer_type, root_identifier,
};

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
        self.mark_binding_moved_into_aggregate(elem_node, elem_ty);
        if is_heap_handle_type(elem_ty) && !is_ident {
            self.mark_consumed_binding(val_reg);
        } else if call_arg_needs_release(elem_node) {
            self.emit_release(val_reg)?;
        }
        Ok(())
    }

    /// codegen.zig:17334 `structFieldTempNeedsRelease`: after a
    /// plain field store, the produced register is retired when
    /// the value expression's result needs release, or when the
    /// register is a fresh temporary that is not the value's own
    /// binding register — the case the let-slot gate
    /// (`stored_value_needs_release`) misses: an identifier that
    /// materialised a new temp (a function name in fn-ptr
    /// position lowers to a fresh `&vtable` temp) leaves that
    /// temp live at function exit (MemoryLeak, trap 1012;
    /// LINE-FNPTR-REMAINDER).
    pub(crate) fn struct_field_temp_needs_release(
        &self,
        value: &Node,
        generated_reg: &str,
    ) -> bool {
        if self.expr_result_reg_needs_release(value) {
            return true;
        }
        if !is_temporary_register_name(generated_reg) {
            return false;
        }
        if let Node::Identifier(name) = value {
            if generated_reg == self.resolve_binding_name(name) {
                return false;
            }
        }
        true
    }

    /// codegen.zig:16051 `genStructLiteralInto` (direct + move
    /// transfer arms; the deep-copy arm stays loud with
    /// `gen_copy_value_into`).
    pub(crate) fn gen_struct_literal_into(
        &mut self,
        target: &str,
        lit: &StructLiteral,
        node: &Node,
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
        // A `Self { ... }` literal inside an impl method: the
        // parser keeps the spelled name `Self` in the AST and
        // the checker resolves it against the impl target,
        // recording the concrete type on the literal node (Zig's
        // checker rewrites the type before codegen; the Rust
        // checker records it in expr_types instead). Resolve
        // through the recorded type so the decl lookup below
        // sees the impl target's name.
        let type_name = if type_name == "Self" {
            match self.expr_type(node) {
                Some(ty) => match sla_lowering::concrete_type_name(&ty) {
                    Some(name) => name.to_string(),
                    None => type_name,
                },
                None => type_name,
            }
        } else {
            type_name
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
            // The update source's root binding, for drop taint:
            // fields this update moves out of the source place
            // must not be dropped again by the source binding's
            // own plan release (the emitter-side mirror of the
            // checker's drop taint, which the Rust checker does
            // not surface).
            let update_root = root_identifier(update_expr).map(str::to_string);
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
                    // The copy is word-wise (no new ownership of
                    // the field's payload): taint the field so the
                    // source's plan release does not drop the
                    // value the target now shares.
                    if self.plan_value_drop(&field_ty) != crate::drop::DropPlan::None {
                        if let Some(root) = &update_root {
                            self.update_moved_fields
                                .insert((root.clone(), decl_field.name.clone()));
                        }
                    }
                    if call_arg_needs_release(update_expr) {
                        self.emit_release(&loaded)?;
                    }
                } else {
                    // Raw move: ownership of the field transfers
                    // to the target, so the source's plan release
                    // must skip the field (taint) — with one
                    // exception: an Arc/Rc field of a PARAM source
                    // was clone-bumped by the call-argument copy
                    // that materialised the param, and the
                    // source's plan drop of the field is exactly
                    // that bump's compensation, so it stays
                    // untainted and fires.
                    let field_plan = self.plan_value_drop(&field_ty);
                    let smart = matches!(
                        field_plan,
                        crate::drop::DropPlan::ArcPtr(_) | crate::drop::DropPlan::RcPtr(_)
                    );
                    let taint = field_plan != crate::drop::DropPlan::None
                        && !(smart
                            && update_root
                                .as_ref()
                                .is_some_and(|r| self.param_bindings.contains(r)));
                    if taint {
                        if let Some(root) = &update_root {
                            self.update_moved_fields
                                .insert((root.clone(), decl_field.name.clone()));
                        }
                    }
                    self.ins(&format!(
                        "store {target}+{offset}, ^{loaded} as {}",
                        type_string(&field_ty)
                    ));
                }
            }
            if call_arg_needs_release(update_expr) {
                // Settle the source register with the plan
                // dispatch suppressed: its fields were raw-moved
                // into the target (or word-copied by
                // gen_copy_value_into, which creates no new
                // ownership), so a plan-driven field drop here
                // would free values the target now owns (the
                // struct-update half of
                // RUST-S9-RUNTIME-DOUBLE-FREE).
                let prev_suppressed = self.plan_drop_suppressed;
                self.plan_drop_suppressed = true;
                let settle = self.emit_release(&update_reg);
                self.plan_drop_suppressed = prev_suppressed;
                settle?;
            }
        }
        // SPRINT-100 L2: sibling frame (Zig
        // `current_expr_later_nodes`) over the field values in
        // generation (decl) order — a field value whose call
        // consumes a binding that a LATER field still reads
        // (e.g. `Ref { value: get_values(world), tick:
        // world.tick }`) must clone for the consuming position
        // instead of moving the binding out from under the
        // later fields.
        let mut frame_index_of: Vec<Option<usize>> = Vec::new();
        {
            let mut children: Vec<*const Node> = Vec::new();
            for decl_field in &decl.fields {
                if let Some(f) = lit.fields.iter().find(|f| f.name == decl_field.name) {
                    frame_index_of.push(Some(children.len()));
                    children.push(&*f.value as *const Node);
                } else {
                    frame_index_of.push(None);
                }
            }
            self.push_expr_later_frame(children);
        }
        let mut field_loop_result: Result<()> = Ok(());
        for (decl_idx, decl_field) in decl.fields.iter().enumerate() {
            let value = lit
                .fields
                .iter()
                .find(|f| f.name == decl_field.name)
                .map(|f| &*f.value);
            let value = match value {
                Some(v) => v,
                None if lit.update_expr.is_some() => continue,
                None => {
                    field_loop_result = Err(TextError::Unsupported {
                        feature: format!(
                            "struct literal missing field value `{}`",
                            decl_field.name
                        ),
                        zig_ref: "codegen.zig:16051 (genStructLiteralInto)",
                    });
                    break;
                }
            };
            if let Some(fi) = frame_index_of[decl_idx] {
                self.set_expr_later_frame_index(fi);
            }
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
                // REG2-D3 (codegen.zig genStructLiteralInto deep-copy
                // arm): settle the copy register with the consume
                // marker in the store itself, or the nested
                // allocation stays Active at function exit
                // (MemoryLeak, trap 1012 — the
                // @sla__ip_mask_from_prefix_len shape: a nested
                // copy-struct literal field). The update arm above
                // already stores `^{copied}`; this arm was missing
                // the marker.
                let val_reg = self.gen_expr(value)?;
                let copied = self.new_tmp();
                self.gen_copy_value_into(&copied, &val_reg, &field_ty)?;
                self.ins(&format!(
                    "store {target}+{offset}, ^{copied} as {}",
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
                if self.struct_field_temp_needs_release(value, &val_reg) {
                    self.emit_release(&val_reg)?;
                }
            } else if val_reg.starts_with('&') || val_reg.starts_with('^') {
                self.ins(&format!(
                    "store {target}+{offset}, {val_reg} as {ty_str}"
                ));
                self.mark_binding_moved_into_aggregate(value, &field_ty);
            } else {
                // RUST-STAGE10-CLONE-EXTRACT (shape D): a place-read
                // field value (field/index out of a live parent)
                // aliases the parent's buffers; moving the loaded
                // word into the literal hands the literal a buffer
                // the parent's exit cleanup will also free (the
                // callee-tail double free in reg3_redis_clear_multi).
                // Clone the extracted value — a transfer-point clone
                // in Zig's sense, so the literal owns a fresh copy
                // and the source binding keeps its field. Bare
                // identifiers are moves (never cloned); a place
                // read with no identifier root (call-result temp)
                // is already freshly owned, so it moves as-is;
                // map/set types keep the historical raw pass.
                if root_identifier(value).is_some()
                    && matches!(value, Node::FieldExpr(_) | Node::IndexExpr(_))
                    && self.deep_clone_needed(&field_ty, 0)
                    && self.uncloneable_type_name(&field_ty, 0).is_none()
                {
                    if let Some(cloned) = self.gen_deep_clone_value(&val_reg, &field_ty)? {
                        // The source temp holds the aliased word:
                        // retire it (a temp has no binding type, so
                        // this is a plain `!reg`, never a field
                        // drop — the REG3-DEF-2 shape).
                        self.emit_release(&val_reg)?;
                        self.ins(&format!(
                            "store {target}+{offset}, ^{cloned} as {ty_str}"
                        ));
                        continue;
                    }
                }
                // Identifier bindings and fresh owned temps alike
                // move into the aggregate.
                self.ins(&format!(
                    "store {target}+{offset}, ^{val_reg} as {ty_str}"
                ));
                self.mark_binding_moved_into_aggregate(value, &field_ty);
            }
        }
        self.pop_expr_later_frame();
        field_loop_result?;
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
            self.mark_binding_moved_into_aggregate(value, &field_ty);
            // LINE-CENSUS-MEMLEAK: settle a fresh payload temp
            // after the store, exactly like the struct-field
            // Copy arm (struct_field_temp_needs_release): the
            // payload value now lives in the variant record, and
            // without this a nested literal payload temp stayed
            // Active at function exit (MemoryLeak 1012,
            // @sla__acl_mk_wrap in
            // test_unit_def_acl_aggregate_payload).
            if self.struct_field_temp_needs_release(value, &val_reg) {
                self.emit_release(&val_reg)?;
            }
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
            Node::DerefExpr(d) => {
                // codegen.zig:20073 `planAddressOf` deref shapes: a
                // borrow or plain-pointer source already holds the
                // place address, so `&*p` clones the pointer
                // register with `ptr_add +0` — returning the source
                // register itself would let the consumer move it
                // out from under its own exit cleanup
                // (UnknownRegister at the source's release, e.g.
                // `&*value` for a `&value: i32` parameter). A
                // pointer source whose pointee is a pointer-backed
                // aggregate addresses the cell's content instead.
                // Smart-pointer sources (Box/Rc/Arc/RefCell after
                // peeling) and all other shapes keep the historical
                // fallthrough below.
                let source_ty = self.expr_type(&d.expr);
                let plain_source = match &source_ty {
                    Some(ty) => {
                        let peeled = peel_borrow_pointer_type(ty);
                        let smart = matches!(peeled, Type::UserDefined(u) if matches!(u.name.as_str(), "Box" | "Rc" | "Arc" | "RefCell"));
                        let typed = matches!(ty, Type::Borrow(_) | Type::Pointer(_));
                        let borrow_param = match &*d.expr {
                            Node::Identifier(name) => self
                                .borrow_param_bindings
                                .contains(&self.resolve_binding_name(name)),
                            _ => false,
                        };
                        (typed || borrow_param) && !smart
                    }
                    None => false,
                };
                if plain_source {
                    let source = self.gen_expr(&d.expr)?;
                    let addr = self.new_tmp();
                    let pointer_aggregate = matches!(source_ty, Some(Type::Pointer(_)))
                        && match self.expr_type(&borrow.expr) {
                            Some(target_ty) => !matches!(
                                target_ty,
                                Type::Infer
                                    | Type::Primitive(_)
                                    | Type::Pointer(_)
                                    | Type::Borrow(_)
                                    | Type::FnPtr(_)
                            ),
                            None => false,
                        };
                    if pointer_aggregate {
                        self.ins(&format!("{addr} = load {source}+0 as ptr"));
                    } else {
                        self.ins(&format!("{addr} = ptr_add {source}, 0"));
                    }
                    if sla_lowering::expr_result_needs_release(&d.expr) {
                        self.borrow_source_temps.insert(addr.clone(), source);
                    }
                    return Ok(addr);
                }
                // codegen.zig:20202 `deref_smart_pointer` shape
                // (non-macro-inline fallthrough to the generic
                // tail at 20218): generate the deref itself, then
                // take the address of its result temp — `&*item`
                // for a Box/Rc/Arc source is the address of the
                // loaded pointee handle, never the source register
                // (returning the source register lets the consumer
                // move it, and the pointee load is lost).
                // codegen.zig:20228, borrow arm `deref_smart_pointer`
                // shape: inside a macro inline, `&*box` is a ptr_add
                // alias of the box register itself — the handle IS
                // the pointee address, and the generic `&deref_temp`
                // tail below would address a value temp with no
                // storage (host segfault on the callee's load).
                // Box only, and only in inlines, exactly as in Zig.
                if self.macro_inline_depth > 0 {
                    let box_source = match &source_ty {
                        Some(ty) => matches!(
                            peel_borrow_pointer_type(ty),
                            Type::UserDefined(u)
                                if u.name == "Box" && u.generics.len() == 1
                        ),
                        None => false,
                    };
                    if box_source {
                        let source = self.gen_expr(&d.expr)?;
                        let addr = self.new_tmp();
                        self.ins(&format!("{addr} = ptr_add {source}, 0"));
                        if sla_lowering::expr_result_needs_release(&d.expr) {
                            self.borrow_source_temps.insert(addr.clone(), source);
                        }
                        return Ok(addr);
                    }
                }
                let smart_source = match &source_ty {
                    Some(ty) => matches!(
                        peel_borrow_pointer_type(ty),
                        Type::UserDefined(u)
                            if matches!(u.name.as_str(), "Box" | "Rc" | "Arc")
                    ),
                    None => false,
                };
                if smart_source {
                    let inner = self.gen_expr(&borrow.expr)?;
                    // F-DYN L3: for a dyn pointee the deref result
                    // IS the trait-object fat cell address (the
                    // RC_GET result / the box payload), so `&*obj`
                    // is that register itself — the Zig emission
                    // is `borrowed = <RC_GET result>`. Taking
                    // `&inner` instead addresses the temp slot
                    // holding the cell pointer, and the dispatch
                    // then loads Dyn_vtable out of unrelated
                    // stack (signal 11).
                    let dyn_pointee = self
                        .expr_type(&borrow.expr)
                        .map(|ty| {
                            sla_lowering::dyn_trait_name(&ty).is_some()
                        })
                        .unwrap_or(false);
                    if dyn_pointee {
                        return Ok(inner);
                    }
                    let reg = self.new_tmp();
                    self.ins(&format!("{reg} = &{inner}"));
                    if sla_lowering::expr_result_needs_release(&borrow.expr) {
                        self.borrow_source_temps.insert(reg.clone(), inner);
                    }
                    return Ok(reg);
                }
                self.gen_expr(&d.expr)
            }
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
                // the chain.
                // SPRINT-100 L2R2: the same holds for EVERY
                // pointer-represented aggregate element, not just
                // the named heap handles — a Vec slot never holds
                // a struct inline (the index READ path loads the
                // stored record pointer for every non-primitive
                // element), so `&v[i]` over a plain struct
                // element must be the loaded record pointer;
                // returning the slot address makes field reads
                // through the borrow reinterpret the pointer word
                // as the first field
                // (test_unit_def_sab_borrow_write_null GUARD:
                // BnRec{key, val} read back as pointer garbage).
                // DEF-RUST-TEXT-VEC-ELEMENT-BORROW-ARG: an
                // inline-POD element is no exception. Inline POD
                // describes the element's single-word packed
                // LAYOUT, not its Vec storage: VEC_PUSH stores
                // the record pointer the value materialises as
                // (struct literals included) and the index READ
                // path loads that pointer back, so the slot
                // holds a pointer for a PhiAtom{z, charge} just
                // as it does for a wider struct. Keeping the
                // slot address for inline-POD elements handed
                // the callee the address OF the pointer word;
                // its field reads consumed pointer bits as
                // record data — a silent wrong result, no crash
                // (test_unit_vec_elem_borrow_argmin: the argmin
                // itself came out wrong). The slot-address form
                // stays correct for ARRAY and SLICE targets,
                // whose slots genuinely hold the packed image;
                // this branch is Vec-gated, so they never reach
                // the load.
                let target_ty = self.require_expr_type(&idx.target, "borrow index target type")?;
                if let Some((_, elem_ty)) = crate::place::vec_like_element_type(&target_ty) {
                    if is_heap_handle_type(&elem_ty)
                        || matches!(
                            elem_ty,
                            Type::UserDefined(_)
                                | Type::Tuple(_)
                                | Type::Array(_)
                        )
                    {
                        let rec = self.new_tmp();
                        self.ins(&format!("{rec} = load {}+0 as ptr", address.ptr));
                        // DEF-RUST-VECELEM-BORROW-LOAD-LEAK (task 47):
                        // the slot address is consumed by this load,
                        // so retire it (and its chained slot base)
                        // here, exactly as the field arm above
                        // retires its projection ptr after loading.
                        // Chaining rec -> address.ptr instead left
                        // both temps live whenever the borrow value
                        // was MOVED rather than released (a Vec push
                        // forgets rec with ^rec, which never settles
                        // the chain), and the verifier rejects the
                        // live ptr_add address at function exit
                        // (MemoryLeak trap 1012,
                        // test_unit_s100_vec_borrow_push).
                        self.emit_release(&address.ptr)?;
                        return Ok(rec);
                    }
                }
                Ok(address.ptr)
            }
            _ => {
                // codegen.zig:13029 `value_temp` fallthrough
                // (planAddressOf `else => .value_temp`): every
                // other borrow shape — a call result, a cast, a
                // literal — evaluates into a temp and takes the
                // temp's address; the temp chains into the
                // borrow-source table when its result needs
                // release, so the borrow's release settles it.
                let inner = self.gen_expr(&borrow.expr)?;
                let reg = self.new_tmp();
                self.ins(&format!("{reg} = &{inner}"));
                if sla_lowering::expr_result_needs_release(&borrow.expr) {
                    self.borrow_source_temps.insert(reg.clone(), inner);
                }
                Ok(reg)
            }
        }
    }
}

//! Statement emission, mirrored from codegen.zig's `genStmt`
//! (10650-11580). The Zig switch ends with `else => {}` — unknown
//! statement kinds are silently ignored there; this mirror fails
//! loudly instead (intentional divergence, logged in
//! docs/divergences/lowering-text.md).

use sla_ast::{CallExpr, Literal, Node, Type};
use sla_lowering::{
    call_arg_needs_release, is_borrow_like_type, is_discard_name, is_panic_builtin_name,
    is_println_call, is_stack_alloc_node, is_void_type, node_key, root_identifier,
    stmt_terminates, stored_value_moves_identifier,
};

use crate::codegen::{type_size, type_string, Codegen};
use crate::{unsupported, Result, TextError};

impl<'c> Codegen<'c> {
    /// codegen.zig:10650 `genStmt` (dispatch + lexical-cleanup epilogue).
    pub(crate) fn gen_stmt(&mut self, stmt: &Node) -> Result<()> {
        match stmt {
            Node::LetStmt(_) => self.gen_let_stmt(stmt)?,
            Node::VarStmt(v) => {
                self.remember_local_binding_type(&v.name, &v.ty);
                let slot = self.resolve_binding_name(&v.name);
                self.stack_alloc_bindings.insert(slot.clone());
                self.addressable_bindings.insert(v.name.clone());
                self.ins(&format!("{slot} = stack_alloc {}", type_size(&v.ty)));
            }
            Node::ConstStmt(c) => {
                let val = self.gen_expr(&c.value)?;
                let value_ty = self.expr_type(&c.value);
                if let Some(moved) = stored_value_moves_identifier(
                    &c.value,
                    value_ty.as_ref().unwrap_or(&Type::Infer),
                    value_ty
                        .as_ref()
                        .map(|t| self.type_is_copy_value(t))
                        .unwrap_or(false),
                ) {
                    self.mark_consumed_binding(moved);
                }
                self.ins(&format!("{} = {val}", c.name));
            }
            Node::AssignStmt(_) => self.gen_assign_stmt(stmt)?,
            Node::BlockStmt(blk) => self.gen_scoped_block(&blk.body)?,
            Node::ArenaStmt(blk) => self.gen_scoped_block(&blk.body)?,
            // R-3 goroutine batch: spawn/defer/select lower in
            // the runtime batch; the check-level slice passes the
            // statements through scope-only so unrelated codegen
            // fixtures keep compiling (full lowering is R-4
            // jurisdiction).
            Node::GoStmt(_) | Node::DeferStmt(_) | Node::SelectStmt(_) => {},
            // A `using` declaration inside a function body only
            // scopes the checker's extension resolution (already
            // done); it emits nothing, exactly as the SAB
            // emitter treats it and as the top-level UsingDecl
            // is treated in generate_program.
            Node::UsingDecl(_) => {},
            Node::ExprStmt(expr) => self.gen_expr_stmt(expr)?,
            Node::ReturnStmt(_) => self.gen_return_stmt(stmt)?,
            Node::WhileStmt(_) => self.gen_while_stmt(stmt)?,
            Node::BreakStmt(_) => self.gen_break_or_continue(stmt, true)?,
            Node::ContinueStmt(_) => self.gen_break_or_continue(stmt, false)?,
            Node::ReleaseStmt(rel) => {
                let resolved = self.resolve_binding_name(&rel.var_name);
                self.emit_release(&resolved)?;
            }
            Node::ForStmt(_) => self.gen_for_stmt(stmt)?,
            Node::LetElseStmt(_) => {
                return unsupported(
                    "let-else statement (pattern family deferred)",
                    "codegen.zig:10700 (let_else_stmt emission)",
                )
            }
            Node::LetDestructureStmt(_) => {
                return unsupported(
                    "let-destructure statement (pattern family deferred)",
                    "codegen.zig:11000 (let_destructure_stmt emission)",
                )
            }
            other => {
                return Err(TextError::Unsupported {
                    feature: format!("statement kind {:?}", std::mem::discriminant(other)),
                    zig_ref: "codegen.zig:10650 (genStmt else => {} silently ignores)",
                })
            }
        }
        if !stmt_terminates(stmt) {
            if let Some(list) = self.check.cleanups(stmt) {
                let names: Vec<String> = list.to_vec();
                for name in names {
                    self.emit_lexical_cleanup_release(&name)?;
                }
            }
        }
        Ok(())
    }

    /// codegen.zig:10652 `genLetStmt` (via the genStmt let arm).
    fn gen_let_stmt(&mut self, stmt: &Node) -> Result<()> {
        let let_stmt = match stmt {
            Node::LetStmt(l) => l,
            _ => unreachable!(),
        };
        let stmt_key = node_key(stmt);
        let stmt_name: String = match self.let_binding_aliases.get(&stmt_key) {
            Some(alias) => alias.clone(),
            None => let_stmt.name.clone(),
        };
        let resolved_name = self.resolve_binding_name(&stmt_name);
        let value_ty = self.expr_type(&let_stmt.value);
        let let_ty: Option<Type> = match &let_stmt.ty {
            Some(t) => Some((**t).clone()),
            None => value_ty.clone(),
        };
        if let Some(ty) = &let_ty {
            self.remember_local_binding_type(&resolved_name, ty);
        }

        if self.hoisted_allocs.iter().any(|h| h == &resolved_name) {
            self.ins(&format!("// stack-alloc hoisted for {resolved_name}"));
            self.stack_alloc_bindings.insert(resolved_name.clone());
            return Ok(());
        }

        if is_discard_name(&let_stmt.name) {
            if let Node::Identifier(_) = &*let_stmt.value {
                let copy = let_ty
                    .as_ref()
                    .map(|t| self.type_is_copy_value(t))
                    .unwrap_or(false);
                let borrow_like = let_ty.as_ref().map(is_borrow_like_type).unwrap_or(false);
                if !copy && !borrow_like {
                    let val = self.gen_expr(&let_stmt.value)?;
                    self.emit_forget_moved_value(&val)?;
                    return Ok(());
                }
            }
            let val = self.gen_expr(&let_stmt.value)?;
            self.release_after_call_arg_if_needed(&let_stmt.value, &val)?;
            return Ok(());
        }

        // Atomic/Cell/ManuallyDrop associated-constructor lets
        // materialize directly into the binding's own storage
        // (codegen.zig:13846-13897): heap cells for the atomics
        // and Cell (their addresses escape into owners), a stack
        // slot for ManuallyDrop.
        if let Node::CallExpr(call) = &*let_stmt.value {
            if call.func_name == "new" && call.args.len() == 1 {
                let materialize: Option<(&str, &str, bool)> =
                    match call.associated_target.as_deref() {
                        Some("AtomicI32") => {
                            Some(("alloc AtomicI32_SIZE", "ATOMIC_I32_INIT", false))
                        }
                        Some("AtomicUsize") => {
                            Some(("alloc AtomicUsize_SIZE", "ATOMIC_USIZE_INIT", false))
                        }
                        Some("AtomicPtr") => {
                            Some(("alloc AtomicPtr_SIZE", "ATOMIC_PTR_INIT", false))
                        }
                        Some("Cell") => Some(("alloc Cell_SIZE", "CELL_SET", false)),
                        Some("ManuallyDrop") => Some((
                            "stack_alloc ManuallyDropU64_SIZE",
                            "MANUALLY_DROP_U64_NEW",
                            true,
                        )),
                        _ => None,
                    };
                if let Some((alloc_op, init_macro, is_stack)) = materialize {
                    let value_reg = self.gen_expr(&call.args[0])?;
                    if is_stack {
                        self.stack_alloc_bindings.insert(resolved_name.clone());
                    }
                    self.ins(&format!("{resolved_name} = {alloc_op}"));
                    self.ins(&format!("EXPAND {init_macro} {resolved_name}, {value_reg}"));
                    self.release_after_call_arg_if_needed(&call.args[0], &value_reg)?;
                    return Ok(());
                }
            }
        }

        // Borrow-of-slice initializer.
        if let Node::BorrowExpr(b) = &*let_stmt.value {
            if matches!(&*b.expr, Node::Identifier(_))
                && matches!(
                    self.expr_type(&let_stmt.value),
                    Some(Type::Borrow(_))
                )
            {
                // Only the slice-shaped borrow goes down the slice
                // path in the Zig; plain borrows fall through.
                if let Some(ty) = &let_ty {
                    if matches!(ty, Type::Borrow(inner) if matches!(&**inner, Type::Array(_))) {
                        return unsupported(
                            "borrowed slice let materialization",
                            "codegen.zig:10760 (slice let path)",
                        );
                    }
                }
            }
        }

        let ty_or_err = match &let_ty {
            Some(t) => t.clone(),
            None => {
                return Err(TextError::MissingType {
                    context: "let statement value type".to_string(),
                })
            }
        };

        if self.binding_needs_addressable_storage(&resolved_name, &ty_or_err)
            && !is_stack_alloc_node(&let_stmt.value)
        {
            let val = match self.gen_let_string_literal_value(&let_stmt.value, &ty_or_err)? {
                Some(owned) => owned,
                None => self.gen_expr(&let_stmt.value)?,
            };
            self.stack_alloc_bindings.insert(resolved_name.clone());
            self.ins(&format!(
                "{resolved_name} = stack_alloc {}",
                type_size(&ty_or_err)
            ));
            self.ins(&format!(
                "store {resolved_name}+0, {val} as {}",
                type_string(&ty_or_err)
            ));
            if call_arg_needs_release(&let_stmt.value) {
                self.emit_release(&val)?;
            }
            return Ok(());
        }

        if self.binding_needs_assigned_value_slot(&resolved_name, &ty_or_err)
            && !is_stack_alloc_node(&let_stmt.value)
        {
            let val = match self.gen_let_string_literal_value(&let_stmt.value, &ty_or_err)? {
                Some(owned) => owned,
                None => self.gen_expr(&let_stmt.value)?,
            };
            self.stack_alloc_bindings.insert(resolved_name.clone());
            self.assigned_value_slots.insert(resolved_name.clone());
            self.ins(&format!(
                "{resolved_name} = stack_alloc {}",
                type_size(&ty_or_err)
            ));
            self.ins(&format!(
                "store {resolved_name}+0, {val} as {}",
                type_string(&ty_or_err)
            ));
            if call_arg_needs_release(&let_stmt.value) {
                self.emit_release(&val)?;
            }
            return Ok(());
        }

        if is_stack_alloc_node(&let_stmt.value) {
            self.stack_alloc_bindings.insert(resolved_name.clone());
            let size = self.stack_alloc_size(&let_stmt.value);
            self.ins(&format!("{resolved_name} = stack_alloc {size}"));
            return Ok(());
        }

        match &*let_stmt.value {
            Node::StructLiteral(lit) => {
                self.gen_struct_literal_into(&resolved_name, lit)?;
                return Ok(());
            }
            Node::EnumLiteral(lit) => {
                self.gen_enum_literal_into(&resolved_name, lit)?;
                return Ok(());
            }
            Node::TupleLiteral(lit) => {
                self.gen_tuple_literal_into(&resolved_name, lit)?;
                return Ok(());
            }
            Node::ArrayLiteral(lit) => {
                // R2b (stage-5a resume): an annotated array let of
                // bare integer elements (`let suffix: [u8; 13] =
                // [46, 105, ...]`) must lay out at the ANNOTATION's
                // stride. The checker's literal type is [i64; 13]
                // (untyped ints default to i64), so the untyped
                // fallback stored each byte at i64 stride and every
                // read at index >= 1 came back 0 (Zig
                // genArrayLiteralInto :16365-16371 takes the element
                // type from the annotation recorded for the whole
                // node). Take the element type from the let's type
                // when it is an array; without an annotation let_ty
                // IS the literal's own type, so the un-annotated
                // path is unchanged.
                match &let_ty {
                    Some(Type::Array(arr)) => {
                        self.gen_array_literal_into_typed(
                            &resolved_name,
                            lit,
                            &arr.elem,
                        )?;
                    }
                    _ => {
                        self.gen_array_literal_into(
                            &resolved_name,
                            &let_stmt.value,
                            lit,
                        )?;
                    }
                }
                return Ok(());
            }
            Node::RepeatArrayLiteral(lit) => {
                // R2b (stage 6): the annotated-let discipline of
                // the plain literal arm above applies to repeats
                // too — the annotation supplies the element type
                // and length; without one let_ty IS the literal's
                // own type, so the un-annotated path is unchanged.
                match &let_ty {
                    Some(Type::Array(arr)) => {
                        self.gen_repeat_array_literal_into_typed(
                            &resolved_name,
                            lit,
                            &arr.elem,
                            arr.len,
                        )?;
                    }
                    _ => {
                        self.gen_repeat_array_literal_into(
                            &resolved_name,
                            &let_stmt.value,
                            lit,
                        )?;
                    }
                }
                return Ok(());
            }
            Node::ClosureLiteral(_) => {
                return unsupported(
                    "closure literal let materialization",
                    "codegen.zig:10830 (gen*LiteralInto family)",
                );
            }
            _ => {}
        }

        // Identifier of copy-struct type: deep-copy into the binding.
        if matches!(&*let_stmt.value, Node::Identifier(_))
            && value_ty
                .as_ref()
                .map(|t| self.check.type_is_copy_struct(t))
                .unwrap_or(false)
        {
            let val = self.gen_expr(&let_stmt.value)?;
            return self.gen_copy_value_into(&resolved_name, &val, value_ty.as_ref().unwrap());
        }

        // String-typed let of a string literal (Zig genLetStmt
        // :15009/:15054): the literal denotes an owned String
        // handle, not the Slice view the generic path would rename
        // into the binding (StackEscape + missing cap slot).
        if let Some(owned) = self.gen_let_string_literal_value(&let_stmt.value, &ty_or_err)? {
            self.ins(&format!("{resolved_name} = {owned}"));
            return Ok(());
        }

        // Generic path.
        let val = self.gen_expr(&let_stmt.value)?;
        // The binding takes over the value register; a recorded
        // borrow source moves with it (codegen.zig:14942).
        self.transfer_borrow_source_to_binding(&val, &resolved_name);
        // Async state maps move with the value too
        // (codegen.zig:11232 transferResultSlotValueState).
        self.transfer_future_state(&val, &resolved_name);
        if let Some(moved) = stored_value_moves_identifier(
            &let_stmt.value,
            &ty_or_err,
            self.type_is_copy_value(&ty_or_err),
        ) {
            self.mark_consumed_binding(moved);
        }
        if matches!(ty_or_err, Type::Primitive(_)) {
            self.emit_primitive_copy(&resolved_name, &val, &ty_or_err)?;
            if call_arg_needs_release(&let_stmt.value) {
                self.emit_release(&val)?;
            }
        } else {
            self.ins(&format!("{resolved_name} = {val}"));
        }
        Ok(())
    }

    /// R1 (stage-5a resume): Zig `genLetStmt` materializes a string
    /// literal bound into a String-typed let as an owned String
    /// handle (`genOwnedStringLiteral`, codegen.zig:14295, called
    /// from the addressable :14940, assigned-slot :14964, and
    /// literal :15009/:15054 arms). The literal's default form is
    /// a stack-allocated Slice view; renaming it into a String
    /// binding trips the host StackEscape, and the view lacks the
    /// cap slot String consumers read. Returns `Some(owned_reg)`
    /// only for exactly `String` + string literal (no borrow peel:
    /// a `&String` let must not take the owned form); `None`
    /// leaves the caller's existing path untouched (fail-safe: a
    /// missed fire keeps the old loud emission).
    fn gen_let_string_literal_value(
        &mut self,
        value: &Node,
        ty: &Type,
    ) -> Result<Option<String>> {
        let Node::Literal(Literal::StringVal(text)) = value else {
            return Ok(None);
        };
        if !matches!(ty, Type::UserDefined(u) if u.name == "String") {
            return Ok(None);
        }
        Ok(Some(self.gen_owned_string_literal(text)?))
    }

    /// codegen.zig:11044 `genConstStmt` shares the generic path above
    /// (kept inline in gen_stmt); this helper covers `stackAllocSize`.
    fn stack_alloc_size(&self, expr: &Node) -> i64 {
        if let Node::CallExpr(call) = expr {
            if let Some(first) = call.args.first() {
                let lookup = |name: &str| match self.global_scalar_const(name) {
                    Some(sla_ast::Literal::IntVal(v)) => Some(v),
                    _ => None,
                };
                if let Some(v) = sla_lowering::int_constant_expr_value(first, &lookup) {
                    return v;
                }
            }
        }
        8
    }

    /// codegen.zig:11105 assign statement, identifier-target path.
    /// codegen.zig:14284 `finishStoredValueAfterSlotStore`: after
    /// a store into a place, a stored identifier temp is forgotten
    /// (the slot owns it now); any other release-needing value is
    /// forgotten when it is a raw pointer or a non-Copy
    /// non-borrow-like value, and released otherwise.
    pub(crate) fn finish_stored_value_after_slot_store(
        &mut self,
        val_reg: &str,
        value: &Node,
    ) -> Result<()> {
        if stored_identifier_needs_release(value, val_reg) {
            self.emit_forget_moved_value(val_reg)?;
            return Ok(());
        }
        if sla_lowering::call_arg_needs_release(value) {
            let val_ty = self.expr_type(value);
            let is_ptr = val_ty
                .as_ref()
                .map(|t| crate::codegen::type_string(t) == "ptr")
                .unwrap_or(false);
            let forget = is_ptr
                || val_ty
                    .as_ref()
                    .map(|t| !self.type_is_copy_value(t) && !sla_lowering::is_borrow_like_type(t))
                    .unwrap_or(false);
            if forget {
                self.emit_forget_moved_value(val_reg)?;
            } else {
                self.emit_release(val_reg)?;
            }
        }
        Ok(())
    }

    /// The SAB crate's `mark_assignment_moved_source`
    /// (gen_assign.rs): an identifier value stored into a place
    /// has moved — mark the binding consumed so its scope cleanup
    /// does not release it a second time.
    pub(crate) fn mark_assignment_moved_source(&mut self, target: &Node, value: &Node) {
        let Some(val_ty) = self.expr_type(value) else {
            return;
        };
        let is_copy = self.type_is_copy_value(&val_ty);
        if let Some(name) =
            sla_lowering::release_facts::assignment_moves_identifier(target, value, &val_ty, is_copy)
        {
            self.mark_consumed_binding(name);
        }
    }

    /// codegen.zig `genIndexAssign` vec arm for owned (non-scalar)
    /// elements: same bounds/ptr/store shape as the scalar arm,
    /// but the element word is stored as u64 (a handle/record
    /// pointer's bits) and the stored value goes through
    /// finishStoredValueAfterSlotStore. No overwritten-element
    /// drop is emitted — the SAB crate's index_assign surface
    /// rules likewise store without one, and these shapes pass
    /// the host verifier on that route.
    fn gen_vec_index_assign_owned(
        &mut self,
        idx: &sla_ast::IndexExpr,
        tname: &str,
        elem: &sla_ast::Type,
        assign: &sla_ast::AssignStmt,
    ) -> Result<()> {
        let (len_def, ptr_def) = if tname == "GrowVec" {
            ("GrowVec_len", "GrowVec_ptr")
        } else {
            ("Vec_len", "Vec_ptr")
        };
        let vec_reg = self.gen_expr(&idx.target)?;
        let index_reg = self.gen_expr(&idx.index)?;
        let val_reg = self.gen_expr(&assign.value)?;
        let len_reg = self.new_tmp();
        let in_bounds_reg = self.new_tmp();
        let hit_label = self.new_label("L_VEC_INDEX_ASSIGN_OK");
        let miss_label = self.new_label("L_VEC_INDEX_ASSIGN_OOB");
        self.ins(&format!("{len_reg} = load {vec_reg}+{len_def} as u64"));
        self.ins(&format!("{in_bounds_reg} = ult {index_reg}, {len_reg}"));
        self.ins(&format!("br {in_bounds_reg} -> {hit_label}, {miss_label}"));
        self.ins(&format!("{miss_label}:"));
        self.ins("panic(87)");
        self.ins(&format!("{hit_label}:"));
        let data_reg = self.new_tmp();
        let offset_reg = self.new_tmp();
        let ptr_reg = self.new_tmp();
        self.ins(&format!("!{in_bounds_reg}"));
        self.ins(&format!("{data_reg} = load {vec_reg}+{ptr_def} as ptr"));
        self.ins(&format!(
            "{offset_reg} = mul {index_reg}, {}",
            type_size(elem).max(8)
        ));
        self.ins(&format!("{ptr_reg} = ptr_add {data_reg}, {offset_reg}"));
        self.ins(&format!("store {ptr_reg}+0, {val_reg} as u64"));
        self.finish_stored_value_after_slot_store(&val_reg, &assign.value)?;
        self.mark_assignment_moved_source(&assign.target, &assign.value);
        self.emit_release(&ptr_reg)?;
        self.emit_release(&offset_reg)?;
        self.emit_release(&data_reg)?;
        self.emit_release(&len_reg)?;
        if sla_lowering::call_arg_needs_release(&idx.index) {
            self.emit_release(&index_reg)?;
        }
        if self.expr_result_reg_needs_release(&idx.target) {
            self.emit_release(&vec_reg)?;
        }
        Ok(())
    }

    fn gen_assign_stmt(&mut self, stmt: &Node) -> Result<()> {
        let assign = match stmt {
            Node::AssignStmt(a) => a,
            _ => unreachable!(),
        };
        // Vec/GrowVec scalar index assignment (codegen.zig
        // `genIndexAssign` vec arm): bounds check with panic(87),
        // then ptr arithmetic + store. The overwritten-value drop
        // is a no-op for scalar elements; owned-element drops and
        // the other target shapes stay on the loud path.
        if let Node::IndexExpr(idx) = &*assign.target {
            let target_ty = self.expr_type(&idx.target);
            let vec_shape = match &target_ty {
                Some(sla_ast::Type::UserDefined(u))
                    if u.name == "Vec" || u.name == "GrowVec" =>
                {
                    Some((u.name.clone(), u.generics.first().cloned()))
                }
                _ => None,
            };
            if let Some((tname, Some(elem))) = vec_shape {
                if !matches!(elem, sla_ast::Type::Primitive(_)) {
                    return self.gen_vec_index_assign_owned(idx, &tname, &elem, assign);
                }
                let (len_def, ptr_def) = if tname == "GrowVec" {
                    ("GrowVec_len", "GrowVec_ptr")
                } else {
                    ("Vec_len", "Vec_ptr")
                };
                let vec_reg = self.gen_expr(&idx.target)?;
                let index_reg = self.gen_expr(&idx.index)?;
                let val_reg = self.gen_expr(&assign.value)?;
                let len_reg = self.new_tmp();
                let in_bounds_reg = self.new_tmp();
                let hit_label = self.new_label("L_VEC_INDEX_ASSIGN_OK");
                let miss_label = self.new_label("L_VEC_INDEX_ASSIGN_OOB");
                self.ins(&format!("{len_reg} = load {vec_reg}+{len_def} as u64"));
                self.ins(&format!("{in_bounds_reg} = ult {index_reg}, {len_reg}"));
                self.ins(&format!(
                    "br {in_bounds_reg} -> {hit_label}, {miss_label}"
                ));
                self.ins(&format!("{miss_label}:"));
                self.ins("panic(87)");
                self.ins(&format!("{hit_label}:"));
                let data_reg = self.new_tmp();
                let offset_reg = self.new_tmp();
                let ptr_reg = self.new_tmp();
                self.ins(&format!("!{in_bounds_reg}"));
                self.ins(&format!("{data_reg} = load {vec_reg}+{ptr_def} as ptr"));
                self.ins(&format!(
                    "{offset_reg} = mul {index_reg}, {}",
                    type_size(&elem).max(8)
                ));
                self.ins(&format!("{ptr_reg} = ptr_add {data_reg}, {offset_reg}"));
                self.ins(&format!(
                    "store {ptr_reg}+0, {val_reg} as {}",
                    type_string(&elem)
                ));
                self.emit_release(&ptr_reg)?;
                self.emit_release(&offset_reg)?;
                self.emit_release(&data_reg)?;
                self.emit_release(&len_reg)?;
                if sla_lowering::call_arg_needs_release(&idx.index) {
                    self.emit_release(&index_reg)?;
                }
                return Ok(());
            }
        }
        // Non-identifier targets (codegen.zig genAssignStmt place
        // arms; release accounting mirrors the SAB crate's
        // gen_assign, which passes the host verifier on these
        // shapes without an overwritten-place drop).
        match &*assign.target {
            Node::DerefExpr(d) => {
                let ptr_reg = self.gen_expr(&d.expr)?;
                let val_reg = self.gen_expr(&assign.value)?;
                let val_ty = self.require_expr_type(&assign.value, "assign value type")?;
                self.ins(&format!(
                    "store {ptr_reg}+0, {val_reg} as {}",
                    type_string(&val_ty)
                ));
                self.finish_stored_value_after_slot_store(&val_reg, &assign.value)?;
                self.mark_assignment_moved_source(&assign.target, &assign.value);
                if self.expr_result_reg_needs_release(&d.expr) {
                    self.emit_release(&ptr_reg)?;
                }
                return Ok(());
            }
            Node::FieldExpr(f) => {
                let addr = self.gen_field_address(f)?;
                self.remember_address_projection_source(&addr);
                let (_, field_ty) = self.field_address_layout(
                    &self.require_expr_type(&f.expr, "assign field base type")?,
                    &f.field_name,
                )?;
                let val_reg =
                    self.gen_expr_in_context(&assign.value, Some(&field_ty))?;
                self.ins(&format!(
                    "store {}+0, {val_reg} as {}",
                    addr.ptr,
                    type_string(&field_ty)
                ));
                self.finish_stored_value_after_slot_store(&val_reg, &assign.value)?;
                self.mark_assignment_moved_source(&assign.target, &assign.value);
                self.emit_release(&addr.ptr)?;
                return Ok(());
            }
            Node::IndexExpr(idx) => {
                let addr = self.gen_index_address(idx)?;
                self.remember_index_address_source(&addr);
                let val_reg =
                    self.gen_expr_in_context(&assign.value, Some(&addr.elem_ty))?;
                self.ins(&format!(
                    "store {}+0, {val_reg} as {}",
                    addr.ptr,
                    type_string(&addr.elem_ty)
                ));
                self.finish_stored_value_after_slot_store(&val_reg, &assign.value)?;
                self.mark_assignment_moved_source(&assign.target, &assign.value);
                self.finish_index_address(&addr)?;
                return Ok(());
            }
            _ => {}
        }
        let target_name = match &*assign.target {
            Node::Identifier(name) => name.clone(),
            _ => {
                return unsupported(
                    "assign target shape",
                    "codegen.zig:11105 (assign_stmt)",
                )
            }
        };
        let resolved_name = self.resolve_binding_name(&target_name);
        // The target is a place, not a value expr: the checker
        // records no expr type for it, so fall back to the
        // binding's remembered type (without it a primitive
        // `m = cap` degrades to a plain register move, consuming
        // the source in one branch only — PhiStateConflict at
        // the join).
        let target_ty = self
            .expr_type(&assign.target)
            .or_else(|| self.local_binding_type_for_name(&resolved_name));

        // Copy-struct target: release + deep copy into (deferred).
        if let Some(ty) = &target_ty {
            if self.check.type_is_copy_struct(ty) {
                return unsupported(
                    "assign into copy-struct target (deep-copy family)",
                    "codegen.zig:11110 (copy-struct assign)",
                );
            }
        }

        // R2b (stage 6): an assigned literal takes its layout
        // from the target binding's type.
        let val = self.gen_expr_in_context(&assign.value, target_ty.as_ref())?;
        let stored_val: String = match val.strip_prefix('^') {
            Some(stripped) => stripped.to_string(),
            None => val.clone(),
        };

        let mut renamed_from_source = false;
        // Zig REG-BR-6 move-out (codegen.zig ~15558): the
        // right-hand tail call claimed this target (it consumed the
        // binding itself), so the slot's old value no longer exists.
        // Storing the result through the consumed slot register
        // trips the SA referee (UseAfterMove); rebind the register
        // instead, which is the form the referee accepts and branch
        // merging expects for a live target. (The generalized
        // rebind form is not ported; only the self-assign claim.)
        let move_out_rebind = self.assign_has_self_move_out_claim(stmt, &target_name)
            && (self.assigned_value_slots.contains(&resolved_name)
                || self.addressable_bindings.contains(&target_name)
                || self.stack_alloc_bindings.contains(&resolved_name));
        if move_out_rebind {
            if matches!(target_ty, Some(Type::Primitive(_)))
                && !matches!(&*assign.value, Node::MoveExpr(_))
            {
                let ty = target_ty.as_ref().unwrap();
                self.emit_primitive_copy(&resolved_name, &stored_val, ty)?;
            } else {
                self.ins(&format!("{resolved_name} = {stored_val}"));
            }
        } else if self.assigned_value_slots.contains(&resolved_name) {
            let ty = target_ty
                .as_ref()
                .ok_or_else(|| TextError::MissingType {
                    context: "assign target type".to_string(),
                })?;
            self.ins(&format!(
                "store {resolved_name}+0, {stored_val} as {}",
                type_string(ty)
            ));
        } else if self.addressable_bindings.contains(&target_name)
            || self.stack_alloc_bindings.contains(&resolved_name)
        {
            let ty = target_ty
                .as_ref()
                .ok_or_else(|| TextError::MissingType {
                    context: "assign target type".to_string(),
                })?;
            self.ins(&format!(
                "store {resolved_name}+0, {stored_val} as {}",
                type_string(ty)
            ));
        } else {
            self.emit_release(&target_name)?;
            if matches!(&*assign.value, Node::Identifier(_))
                && matches!(target_ty, Some(Type::Primitive(_)))
            {
                let ty = target_ty.as_ref().unwrap();
                self.emit_primitive_copy(&resolved_name, &stored_val, ty)?;
            } else {
                self.ins(&format!("{resolved_name} = {stored_val}"));
                renamed_from_source = true;
            }
        }

        if renamed_from_source {
            self.finish_assign_rename_source(&assign.value, &stored_val, &target_ty);
        }
        if stored_identifier_needs_release(&assign.value, &stored_val) {
            if let Some(src) = root_identifier(&assign.value) {
                let src_resolved = self.resolve_binding_name(src);
                self.mark_consumed_binding(&src_resolved);
            }
        }
        self.consumed_bindings.remove(&resolved_name);
        Ok(())
    }

    /// codegen.zig:11246 return statement.
    fn gen_return_stmt(&mut self, stmt: &Node) -> Result<()> {
        let ret = match stmt {
            Node::ReturnStmt(r) => r,
            _ => unreachable!(),
        };
        let mut value_reg: Option<String> = None;
        if let Some(value) = &ret.value {
            // R2b (stage 6): a returned literal takes its layout
            // from the function's declared return type.
            let ret_ty = self.current_ret_ty.clone();
            value_reg = Some(self.gen_expr_in_context(value, ret_ty.as_ref())?);
            // codegen.zig:11250: a pending await inside the value
            // already returned from this async function.
            if self.async_pending_return_emitted {
                return Ok(());
            }
        }
        // codegen.zig:11252: inside an async function, a return
        // value wraps in a ready future (a bare `return` wraps 0).
        if self.current_async {
            let base = match value_reg {
                Some(reg) => reg,
                None => {
                    let zero = self.new_tmp();
                    self.ins(&format!("{zero} = 0"));
                    zero
                }
            };
            value_reg = Some(self.gen_ready_future_i64(&base)?);
        }
        if let Some(list) = self.check.cleanups(stmt) {
            let names: Vec<String> = list.to_vec();
            for name in names {
                let should_release = match &ret.value {
                    Some(value) => self
                        .plan_function_result_cleanup(&name, value)?
                        .releases_value(),
                    None => true,
                };
                if should_release {
                    self.emit_release(&name)?;
                }
            }
        }
        match &value_reg {
            Some(reg) => self.ins(&format!("return {reg}")),
            None => self.ins("return"),
        }
        Ok(())
    }

    /// codegen.zig:11546/11562 break/continue statements.
    pub(crate) fn gen_break_or_continue(&mut self, stmt: &Node, is_break: bool) -> Result<()> {
        if let Some(list) = self.check.cleanups(stmt) {
            let names: Vec<String> = list.to_vec();
            for name in names {
                if self.active_loop_body_local_contains_name(&name) {
                    self.emit_loop_body_local_cleanup(&name, true)?;
                } else {
                    self.emit_lexical_cleanup_release(&name)?;
                }
            }
        }
        let skip: Vec<String> = self
            .check
            .cleanups(stmt)
            .map(|l| l.to_vec())
            .unwrap_or_default();
        self.emit_active_loop_body_local_cleanups(Some(&skip), true)?;
        let label = if is_break {
            self.loop_break_labels.last().cloned()
        } else {
            self.loop_continue_labels.last().cloned()
        };
        match label {
            Some(label) => {
                self.w(&format!("    jmp {label}\n"));
                Ok(())
            }
            None => unsupported(
                "break/continue outside a loop",
                "codegen.zig:11546 (loop label stack empty)",
            ),
        }
    }

    /// codegen.zig:11581 expression statement.
    fn gen_expr_stmt(&mut self, expr: &Node) -> Result<()> {
        match expr {
            Node::CallExpr(call) => {
                if self.is_void_call(expr) {
                    self.gen_call_stmt(expr)
                } else if is_panic_builtin_name(&call.func_name) {
                    let _ = self.gen_expr(expr)?;
                    Ok(())
                } else {
                    let reg = self.gen_expr(expr)?;
                    self.emit_release(&reg)
                }
            }
            Node::IfExpr(_) | Node::SwitchExpr(_) | Node::MatchExpr(_) => {
                let _ = self.gen_expr(expr)?;
                Ok(())
            }
            _ => {
                let reg = self.gen_expr(expr)?;
                self.emit_release(&reg)
            }
        }
    }

    /// codegen.zig:1868 `isVoidCall`.
    pub(crate) fn is_void_call(&self, expr: &Node) -> bool {
        let call = match expr {
            Node::CallExpr(c) => c,
            _ => return false,
        };
        if is_println_call(call) {
            return true;
        }
        if self.check.is_macro(&call.func_name) {
            return false;
        }
        if call.callee.is_some() {
            return false;
        }
        if call.associated_target.is_some() {
            // An associated call to a known void function is a
            // void call too (thread::sleep in an async fn body,
            // stage 6): resolve the callee declaration the same
            // ways gen_expr_call does.
            if let Some((_, decl)) = self.resolve_associated_decl(call) {
                return is_void_type(&decl.ret_ty);
            }
            if let Some((_, decl)) = self.resolve_receiver_method_decl(call) {
                return is_void_type(&decl.ret_ty);
            }
            if let Some(decl) = self.func_decl(&call.func_name) {
                return is_void_type(&decl.ret_ty);
            }
            if let Some((_, decl)) = self.func_decl_tail_match(&call.func_name) {
                return is_void_type(&decl.ret_ty);
            }
            return false;
        }
        match self.func_decl(&call.func_name) {
            Some(f) => is_void_type(&f.ret_ty),
            // No user declaration: std surface methods (Vec::push,
            // Cell::set and kin) are void by their checker-recorded
            // call type. Without this arm they took the value route
            // in gen_expr_stmt, whose dummy result register was
            // released undeclared (host UnknownRegister) — the
            // phase-6 census R-2 codegen residual.
            None => {
                !is_panic_builtin_name(&call.func_name)
                    && matches!(
                        self.expr_type(expr),
                        Some(sla_ast::Type::Primitive(sla_ast::Primitive::VoidType))
                    )
            }
        }
    }

    /// codegen.zig `releaseAfterCallArgIfNeeded` (let/const discard path).
    pub(crate) fn release_after_call_arg_if_needed(
        &mut self,
        expr: &Node,
        reg: &str,
    ) -> Result<()> {
        if call_arg_needs_release(expr) {
            self.emit_release(reg)?;
        }
        Ok(())
    }
}

/// codegen.zig `storedIdentifierNeedsRelease`: an identifier value
/// whose register is a fresh temporary distinct from the identifier.
fn stored_identifier_needs_release(value: &Node, val_reg: &str) -> bool {
    match value {
        Node::Identifier(name) => {
            val_reg != name && sla_lowering::is_temporary_register_name(val_reg)
        }
        _ => false,
    }
}

/// Re-export for call.rs: the CallExpr view used by is_void_call users.
pub(crate) fn as_call(expr: &Node) -> Option<&CallExpr> {
    match expr {
        Node::CallExpr(c) => Some(c),
        _ => None,
    }
}

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
            // the runtime batch. F-G2: a supervised spawn lowers
            // for real (go_spawn.rs); plain `go` statement form
            // lowers for real too (detached spawn, DEF-R1).
            // F-G4 exception: a select lowers for real
            // (select.rs); `defer` keeps the pass-through
            // (F-G5 jurisdiction).
            Node::GoStmt(go) => {
                if go.supervised {
                    self.gen_supervised_go_stmt(go)?;
                } else {
                    self.gen_plain_go_stmt(go)?;
                }
            }
            Node::DeferStmt(_) => {
                return unsupported(
                    "defer statement survived the desugar pass",
                    "syntax_lowering.zig (internal: defer statement survived the desugar pass)",
                )
            }
            Node::SelectStmt(sel) => self.gen_select_stmt(stmt, sel)?,
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
            Node::LetDestructureStmt(_) => self.gen_let_destructure_stmt(stmt)?,
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
            if self.async_pending_return_emitted {
                return Ok(());
            }
            self.stack_alloc_bindings.insert(resolved_name.clone());
            self.ins(&format!(
                "{resolved_name} = stack_alloc {}",
                type_size(&ty_or_err)
            ));
            self.ins(&format!(
                "store {resolved_name}+0, {val} as {}",
                type_string(&ty_or_err)
            ));
            if self.store_refcell_handle_companion_for_slot(&resolved_name, &val) {
                self.emit_forget_moved_value(&val)?;
            } else if call_arg_needs_release(&let_stmt.value) {
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
            if self.async_pending_return_emitted {
                return Ok(());
            }
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
            if self.store_refcell_handle_companion_for_slot(&resolved_name, &val) {
                self.emit_forget_moved_value(&val)?;
            } else if call_arg_needs_release(&let_stmt.value) {
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

        // DEF-TUPLE-COPY-EMIT: Identifier of all-Copy tuple type:
        // element-wise materializing copy into the binding (Zig
        // genLetStmt tuple arm). The generic path below would
        // alias the source register — an SA-level move — while
        // the checker (FB-010 caliber) keeps the source binding
        // live, so any later read of the source traps UseAfterMove.
        if matches!(&*let_stmt.value, Node::Identifier(_))
            && value_ty
                .as_ref()
                .map(|t| matches!(t, Type::Tuple(_)) && self.check.type_is_copy_value(t))
                .unwrap_or(false)
        {
            let val = self.gen_expr(&let_stmt.value)?;
            return self.gen_tuple_copy_value_into(
                &resolved_name,
                &val,
                value_ty.as_ref().unwrap(),
            );
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
        // codegen.zig:7988: a pending await inside the value
        // already returned from this async function — the binding
        // is never materialized.
        if self.async_pending_return_emitted {
            return Ok(());
        }
        // The binding takes over the value register; a recorded
        // borrow source moves with it (codegen.zig:14942).
        self.transfer_borrow_source_to_binding(&val, &resolved_name);
        // A RefCell borrow handle moves with it too (codegen.zig
        // genLetStmt handle cascade): the binding's own cleanup
        // settles the dynamic borrow.
        self.transfer_refcell_borrow_handle(&val, &resolved_name);
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
            if matches!(&*let_stmt.value, Node::CallExpr(_)) {
                // A call result register is dead after the let:
                // the binding takes it over by rename instead of
                // copy + release. The copy + release pair frees
                // the block when the "primitive" is really a heap
                // handle produced by the call (an imported macro's
                // Poll struct in block_on_direct, stage 6), while
                // copy-without-release leaks the source register
                // (extern_raw_param's PTR_BYTE_ADD). The rename
                // transfers ownership to the binding, whose own
                // cleanup disposes it.
                self.ins(&format!("{resolved_name} = {val}"));
            } else {
                self.emit_primitive_copy(&resolved_name, &val, &ty_or_err)?;
                if self.stored_value_needs_release(&let_stmt.value) {
                    self.emit_release(&val)?;
                }
            }
        } else {
            self.ins(&format!("{resolved_name} = {val}"));
        }
        // RUST-STAGE10-DROP-TAINT (B1): record borrow/taint for
        // the fresh binding (M1 move-out / M2 borrow-alias /
        // M2-call). No-op for plan-None values.
        self.record_value_taint(&resolved_name, &let_stmt.value, &ty_or_err);
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
        if self.stored_value_needs_release(value) {
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

    /// RUST-STAGE10-SHAPE-E (E3, Zig-style source compensation).
    ///
    /// DEF-deep-drop crash-A (replace semantics): the general
    /// source-nulling path, mirroring the Zig checker's
    /// `replaceSourceForAssign` demarcation (type_checker.zig:1862)
    /// and codegen's `emitReplaceSourceNullText` (codegen.zig:1834).
    /// Path 1 above only sees identifier-rooted field chains; this
    /// path covers the remaining transferable place reads -- a
    /// field/index/deref place (move wrappers stripped) holding a
    /// handle value whose drop plan the source tail frees, rooted at
    /// an owned binding, stored into a provably different place.
    /// After the store, a fresh empty handle is written back into
    /// the source place so the transferred value has exactly one
    /// owner. Shapes that cannot be proven different, borrow-rooted
    /// sources (clone-on-extract territory, binding points only),
    /// and non-handle plans keep the plain store, exactly as the
    /// Zig annotation declines them.
    fn emit_replace_source_null_general(
        &mut self,
        target: &Node,
        value: &Node,
        fallback_ty: &Type,
    ) -> Result<()> {
        let mut rhs = value;
        while let Node::MoveExpr(m) = rhs {
            rhs = &m.expr;
        }
        if !matches!(
            rhs,
            Node::FieldExpr(_) | Node::IndexExpr(_) | Node::DerefExpr(_)
        ) {
            return Ok(());
        }
        let value_ty = self
            .expr_type(rhs)
            .unwrap_or_else(|| fallback_ty.clone());
        if !matches!(
            self.plan_value_drop(&value_ty),
            crate::drop::DropPlan::Buffer
                | crate::drop::DropPlan::BufferGrowVec
                | crate::drop::DropPlan::DeepVec(_)
                | crate::drop::DropPlan::DeepGrowVec(_)
                | crate::drop::DropPlan::ArcPtr(_)
                | crate::drop::DropPlan::RcPtr(_)
                | crate::drop::DropPlan::BoxPtr(_)
        ) {
            return Ok(());
        }
        let Some(src_key) = Self::place_key(rhs) else {
            return Ok(());
        };
        let Some(dst_key) = Self::place_key(target) else {
            return Ok(());
        };
        if !Self::place_keys_diverge(&src_key, &dst_key) {
            return Ok(());
        }
        if !self.place_root_owned(&src_key.0) {
            return Ok(());
        }
        let mut active: Vec<Type> = Vec::new();
        let fresh = self.gen_fresh_empty_value(&value_ty, &mut active)?;
        match rhs {
            Node::FieldExpr(f) => {
                let addr = self.gen_field_address(f)?;
                self.ins(&format!(
                    "store {}+0, {fresh} as {}",
                    addr.ptr,
                    type_string(&value_ty)
                ));
                self.emit_forget_moved_value(&fresh)?;
                self.remember_address_projection_source(&addr);
                self.emit_release(&addr.ptr)?;
            }
            Node::IndexExpr(ix) => {
                let addr = self.gen_index_address(ix)?;
                self.ins(&format!(
                    "store {}+0, {fresh} as {}",
                    addr.ptr,
                    type_string(&addr.elem_ty)
                ));
                self.emit_forget_moved_value(&fresh)?;
                self.remember_index_address_source(&addr);
                self.finish_index_address(&addr)?;
            }
            Node::DerefExpr(d) => {
                let ptr_reg = self.gen_expr(&d.expr)?;
                self.ins(&format!(
                    "store {ptr_reg}+0, {fresh} as {}",
                    type_string(&value_ty)
                ));
                self.emit_forget_moved_value(&fresh)?;
                if self.expr_result_reg_needs_release(&d.expr) {
                    self.emit_release(&ptr_reg)?;
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    /// A place spine as (root binding, root-to-leaf projections) --
    /// the codegen-side analogue of the Zig checker's
    /// `taintKeyOfPlace` / codegen's `dropPlaceKeyText`. Slice and
    /// borrow projections and non-identifier roots have no key.
    fn place_key(expr: &Node) -> Option<(String, Vec<PlaceSeg>)> {
        let mut segs: Vec<PlaceSeg> = Vec::new();
        let mut cur = expr;
        loop {
            match cur {
                Node::FieldExpr(f) => {
                    segs.push(PlaceSeg::Field(f.field_name.clone()));
                    cur = &f.expr;
                }
                Node::IndexExpr(ix) => {
                    match &*ix.index {
                        Node::Literal(Literal::IntVal(v)) => {
                            segs.push(PlaceSeg::IndexLit(*v));
                        }
                        _ => segs.push(PlaceSeg::IndexDyn),
                    }
                    cur = &ix.target;
                }
                Node::DerefExpr(d) => {
                    segs.push(PlaceSeg::Deref);
                    cur = &d.expr;
                }
                Node::MoveExpr(m) => {
                    cur = &m.expr;
                }
                Node::Identifier(name) => {
                    segs.reverse();
                    return Some((name.clone(), segs));
                }
                _ => return None,
            }
        }
    }

    /// Two place keys provably denote different places: different
    /// roots, or a definitely-differing projection before either
    /// spine ends. Equal keys, prefix keys, and uncertain pairs
    /// (a dynamic index against a literal one) do not diverge --
    /// the nulling store must never clobber the target place or an
    /// intermediate the source derivation loads.
    fn place_keys_diverge(
        a: &(String, Vec<PlaceSeg>),
        b: &(String, Vec<PlaceSeg>),
    ) -> bool {
        if a.0 != b.0 {
            return true;
        }
        let shared = a.1.len().min(b.1.len());
        for i in 0..shared {
            let definite = match (&a.1[i], &b.1[i]) {
                (PlaceSeg::Field(x), PlaceSeg::Field(y)) => x != y,
                (PlaceSeg::IndexLit(x), PlaceSeg::IndexLit(y)) => x != y,
                (PlaceSeg::Field(_), _)
                | (_, PlaceSeg::Field(_))
                | (PlaceSeg::Deref, PlaceSeg::IndexLit(_))
                | (PlaceSeg::IndexLit(_), PlaceSeg::Deref)
                | (PlaceSeg::Deref, PlaceSeg::IndexDyn)
                | (PlaceSeg::IndexDyn, PlaceSeg::Deref) => true,
                _ => false,
            };
            if definite {
                return true;
            }
        }
        false
    }

    /// Zig `placeSpineRootOwned`: the spine root is a binding whose
    /// type is not borrow-like. Unknown bindings are not owned.
    fn place_root_owned(&self, root: &str) -> bool {
        let resolved = self.resolve_binding_name(root);
        match self.local_binding_type_for_name(&resolved) {
            Some(ty) => !matches!(ty, Type::Borrow(_)),
            None => false,
        }
    }

    /// DEF-5 / DEF-AOF-EXIT69 clone-on-extract for identifier
    /// assignment targets (codegen.zig `cloneBorrowedHandleFieldValue`,
    /// assign arm): when the RHS is a field read whose type carries
    /// owning handles (String / Vec / GrowVec / Box / Rc / Arc, or a
    /// struct with deep fields), the stored value is a deep clone of
    /// the field and the loaded view temp is retired. Other shapes
    /// keep the plain load (identifier moves and plain copies are
    /// unaffected).
    fn clone_on_extract_value(
        &mut self,
        value: &Node,
        raw_reg: String,
    ) -> Result<String> {
        if raw_reg.starts_with('^') || raw_reg.starts_with('&') {
            return Ok(raw_reg);
        }
        if !matches!(value, Node::FieldExpr(_)) {
            return Ok(raw_reg);
        }
        let Some(ty) = self.expr_type(value) else {
            return Ok(raw_reg);
        };
        let cloneable = match &ty {
            Type::UserDefined(u) => {
                matches!(
                    u.name.as_str(),
                    "String" | "Vec" | "GrowVec" | "Box" | "Rc" | "Arc"
                ) || (crate::place::struct_decl_lookup(self.check, &u.name)
                    .is_some()
                    && self.deep_clone_needed(&ty, 0))
            }
            _ => false,
        };
        if !cloneable {
            return Ok(raw_reg);
        }
        match self.gen_deep_clone_value(&raw_reg, &ty)? {
            Some(cloned) => {
                self.emit_release(&raw_reg)?;
                Ok(cloned)
            }
            None => Ok(raw_reg),
        }
    }

    /// After a field-assign move-out (`dst.f = src.g`), the source field's
    /// storage is now owned by the destination, but nothing records the
    /// move — the source binding's tail still frees the field, so the same
    /// buffer is freed twice (signal 11 / double free). Store a fresh
    /// empty value back into the source field so the tail frees the fresh
    /// value instead.
    ///
    /// Fires only when all three hold: (1) the value is a FieldExpr chain
    /// over an identifier root (the source address re-derivation is pure
    /// address arithmetic there; a call-result root would re-emit side
    /// effects); (2) the moved value's drop plan is one whose storage the
    /// source tail actually frees (Buffer / BufferGrowVec / DeepVec /
    /// DeepGrowVec / ArcPtr / RcPtr / BoxPtr — DeepStruct is deliberately
    /// excluded: probes show the tail never frees struct fields in these
    /// shapes, so there is no double-free to fix); (3) the target place
    /// provably differs from the source place, and neither is a prefix of
    /// the other (a prefix target would clobber the source address
    /// derivation — the store overwrites an intermediate pointer the
    /// derivation loads).
    ///
    /// A fresh-empty value that cannot be synthesized (recursive struct,
    /// direct `Box<Self>` field, tuple / array / enum payload) is a loud
    /// `TextError::Unsupported`, never a silent skip — a skipped
    /// compensation is a live double-free.
    pub(crate) fn emit_field_assign_move_out_compensation(
        &mut self,
        target: &Node,
        value: &Node,
        fallback_ty: &Type,
    ) -> Result<()> {
        if self.compensation_field_chain(target, value, fallback_ty)? {
            return Ok(());
        }
        self.emit_replace_source_null_general(target, value, fallback_ty)
    }

    /// Path 1 of the move-out compensation: identifier-rooted field
    /// chains (the stage-10 shapes). Returns true when it nulled the
    /// source place.
    fn compensation_field_chain(
        &mut self,
        target: &Node,
        value: &Node,
        fallback_ty: &Type,
    ) -> Result<bool> {
        let Some(value_place) = Self::field_place_path(value) else {
            return Ok(false);
        };
        let value_ty = self
            .expr_type(value)
            .unwrap_or_else(|| fallback_ty.clone());
        if !matches!(
            self.plan_value_drop(&value_ty),
            crate::drop::DropPlan::Buffer
                | crate::drop::DropPlan::BufferGrowVec
                | crate::drop::DropPlan::DeepVec(_)
                | crate::drop::DropPlan::DeepGrowVec(_)
                | crate::drop::DropPlan::ArcPtr(_)
                | crate::drop::DropPlan::RcPtr(_)
                | crate::drop::DropPlan::BoxPtr(_)
        ) {
            return Ok(false);
        }
        let distinct = match target {
            Node::FieldExpr(_) => {
                let Some(target_place) = Self::field_place_path(target) else {
                    return Ok(false);
                };
                Self::place_paths_diverge(&value_place, &target_place)
            }
            Node::IndexExpr(_) => {
                // Index equality is not provable cheaply: fire only when
                // the roots differ outright. DerefExpr targets may alias
                // the source, so they never fire.
                match root_identifier(target) {
                    Some(t) => t != value_place.0,
                    None => false,
                }
            }
            _ => false,
        };
        if !distinct {
            return Ok(false);
        }
        let field = match value {
            Node::FieldExpr(f) => f,
            _ => return Ok(false),
        };
        let src_addr = self.gen_field_address(field)?;
        let mut active: Vec<Type> = Vec::new();
        let fresh = self.gen_fresh_empty_value(&value_ty, &mut active)?;
        self.ins(&format!(
            "store {}+0, {fresh} as {}",
            src_addr.ptr,
            type_string(&value_ty)
        ));
        self.emit_forget_moved_value(&fresh)?;
        self.remember_address_projection_source(&src_addr);
        self.emit_release(&src_addr.ptr)?;
        Ok(true)
    }

    /// RUST-STAGE10-SHAPE-E (E3): synthesize a fresh empty value of type
    /// `ty` — a value the source binding's tail may free exactly once.
    /// `active` is the stack of in-progress types, compared by `Type`
    /// identity (NOT by `type_string`: that is the ABI spelling and it
    /// collapses every composite type to "ptr" — abi.rs `abi_type_string`
    /// — so it false-positives on e.g. `Box<RedisValue>` → `RedisValue`,
    /// a perfectly constructible payload whose recursion runs through
    /// `Vec`, which needs no element construction). Re-entry of the SAME
    /// type means a recursive shape (direct `Box<Self>` field, recursive
    /// struct) and fails loudly. Types with no fresh-empty form (tuple /
    /// array / enum / fn-ptr / borrow / opaque struct) fail loudly as
    /// well — never silently skipped.
    fn gen_fresh_empty_value(
        &mut self,
        ty: &Type,
        active: &mut Vec<Type>,
    ) -> Result<String> {
        use sla_ast::Primitive;
        match ty {
            Type::Primitive(p) => {
                let reg = self.new_tmp();
                match p {
                    Primitive::F32 | Primitive::F64 | Primitive::Float => {
                        self.emit_float_const(&reg, 0.0);
                    }
                    Primitive::VoidType => {
                        return unsupported(
                            "fresh-empty value for void",
                            "RUST-STAGE10-SHAPE-E (E3)",
                        );
                    }
                    _ => {
                        self.emit_int_const(&reg, 0);
                    }
                }
                Ok(reg)
            }
            Type::Pointer(_) => {
                let reg = self.new_tmp();
                self.emit_int_const(&reg, 0);
                Ok(reg)
            }
            Type::UserDefined(u) => match u.name.as_str() {
                "String" => self.gen_owned_string_literal(""),
                "Vec" => {
                    let reg = self.new_tmp();
                    self.ins(&format!("EXPAND VEC_NEW {reg}"));
                    Ok(reg)
                }
                "GrowVec" => {
                    let elem = u.generics.first().cloned().ok_or_else(|| {
                        TextError::Unsupported {
                            feature: "fresh-empty GrowVec without element type".to_string(),
                            zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
                        }
                    })?;
                    let reg = self.new_tmp();
                    self.ins(&format!(
                        "EXPAND GROWVEC_NEW {reg}, {}, 0",
                        type_size(&elem).max(8)
                    ));
                    Ok(reg)
                }
                "Box" | "Rc" | "Arc" => {
                    let inner = u.generics.first().cloned().ok_or_else(|| {
                        TextError::Unsupported {
                            feature: format!("fresh-empty {} without payload type", u.name),
                            zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
                        }
                    })?;
                    if active.contains(ty) {
                        return Err(TextError::Unsupported {
                            feature: format!("fresh-empty: recursive {} payload", u.name),
                            zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
                        });
                    }
                    active.push(ty.clone());
                    let inner_res = self.gen_fresh_empty_value(&inner, active);
                    active.pop();
                    let inner_reg = inner_res?;
                    let reg = self.new_tmp();
                    let macro_name = match u.name.as_str() {
                        "Box" => "BOX_NEW",
                        "Rc" => "RC_NEW",
                        _ => "ARC_NEW",
                    };
                    self.ins(&format!("EXPAND {macro_name} {reg}, {inner_reg}"));
                    self.emit_forget_moved_value(&inner_reg)?;
                    Ok(reg)
                }
                _ => {
                    // Struct payload (DeepStruct constructor — E3 never
                    // FIRES for a struct-typed value, but Box/Rc/Arc
                    // payloads need one). Generic structs are out of
                    // scope: field types would need substitution.
                    if !u.generics.is_empty() {
                        return Err(TextError::Unsupported {
                            feature: format!(
                                "fresh-empty: generic struct payload {}",
                                u.name
                            ),
                            zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
                        });
                    }
                    if active.contains(ty) {
                        return Err(TextError::Unsupported {
                            feature: format!("fresh-empty: recursive struct {}", u.name),
                            zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
                        });
                    }
                    let decl = crate::place::struct_decl_lookup(self.check, &u.name)
                        .ok_or_else(|| TextError::Unsupported {
                            feature: format!("fresh-empty: no struct decl for {}", u.name),
                            zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
                        })?;
                    if decl.is_opaque || decl.is_union {
                        return Err(TextError::Unsupported {
                            feature: format!("fresh-empty: opaque/union struct {}", u.name),
                            zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
                        });
                    }
                    active.push(ty.clone());
                    let reg = self.new_tmp();
                    self.ins(&format!(
                        "{reg} = alloc {}",
                        crate::layout::struct_size(&decl)
                    ));
                    let mut result = Ok(());
                    for field in &decl.fields {
                        let Some((offset, field_ty)) =
                            crate::layout::struct_field_layout(&decl, &field.name)
                        else {
                            result = Err(TextError::Unsupported {
                                feature: format!(
                                    "fresh-empty: no layout for {}",
                                    field.name
                                ),
                                zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
                            });
                            break;
                        };
                        match self.gen_fresh_empty_value(&field_ty, active) {
                            Ok(fresh) => {
                                self.ins(&format!(
                                    "store {reg}+{offset}, {fresh} as {}",
                                    type_string(&field_ty)
                                ));
                                if let Err(e) = self.emit_forget_moved_value(&fresh) {
                                    result = Err(e);
                                    break;
                                }
                            }
                            Err(e) => {
                                result = Err(e);
                                break;
                            }
                        }
                    }
                    active.pop();
                    result?;
                    Ok(reg)
                }
            },
            _ => Err(TextError::Unsupported {
                feature: format!("fresh-empty value for {}", type_string(ty)),
                zig_ref: "RUST-STAGE10-SHAPE-E (E3)",
            }),
        }
    }

    /// A FieldExpr chain over an identifier root, as (root, fields
    /// root-to-leaf). Anything else (call / index / deref root, non-field
    /// chain) is None.
    fn field_place_path(expr: &Node) -> Option<(String, Vec<String>)> {
        let mut path: Vec<String> = Vec::new();
        let mut cur = expr;
        loop {
            match cur {
                Node::FieldExpr(f) => {
                    path.push(f.field_name.clone());
                    cur = &f.expr;
                }
                Node::Identifier(name) => {
                    path.reverse();
                    return Some((name.clone(), path));
                }
                _ => return None,
            }
        }
    }

    /// Two field places diverge when neither is a prefix of the other:
    /// different roots, or a first differing field before either path
    /// ends. Equal paths (self-assign) and prefix paths (the store would
    /// clobber the source address derivation) do not diverge.
    fn place_paths_diverge(a: &(String, Vec<String>), b: &(String, Vec<String>)) -> bool {
        if a.0 != b.0 {
            return true;
        }
        let mut i = 0;
        while i < a.1.len() && i < b.1.len() {
            if a.1[i] != b.1[i] {
                return true;
            }
            i += 1;
        }
        false
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
                self.emit_field_assign_move_out_compensation(
                    &assign.target,
                    &assign.value,
                    &field_ty,
                )?;
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
                self.emit_field_assign_move_out_compensation(
                    &assign.target,
                    &assign.value,
                    &addr.elem_ty,
                )?;
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
        // DEF-5 clone-on-extract (codegen.zig:16679): a handle-typed
        // field read stored into an identifier target is a deep
        // clone, never the aliasing load -- the parent place keeps
        // (and drops) the field, so an aliasing binding would drop
        // the same buffers a second time.
        let val = self.clone_on_extract_value(&assign.value, val)?;
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
        // DEF-6 (codegen.zig assign cascade): only genuinely
        // materialized stack slots may be stored through. An
        // addressable-only binding (e.g. a struct literal, whose
        // register holds the heap cell's address and whose `&x` is
        // taken elsewhere) keeps its value IN the register; storing
        // through it writes the new value's pointer into the old
        // cell's first word. Such bindings fall through to the
        // release-and-rebind branch below, which is the form the
        // Zig cascade uses for them. (Every binding for which the
        // store is correct -- addressable-storage primitives,
        // assigned-value slots, `var` stmts, stack_alloc calls --
        // is already in stack_alloc_bindings.)
        } else if self.stack_alloc_bindings.contains(&resolved_name)
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
        // RUST-STAGE10-DROP-TAINT (B1): rebind — the old value
        // was released above (seeing OLD taint); record the new
        // value's taint (the decision reads the pre-rebind state,
        // so `x = x.field` judges the OLD x).
        let assign_value_ty = self.expr_type(&assign.value).or(target_ty);
        if let Some(value_ty) = assign_value_ty {
            self.record_value_taint(&resolved_name, &assign.value, &value_ty);
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
                    if self.async_pending_return_emitted {
                        return Ok(());
                    }
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
            // codegen.zig isVoidCall: mem::forget is void.
            if crate::call::is_mem_forget_call(call) {
                return true;
            }
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

/// One projection step of a place spine (see
/// `Codegen::place_key`).
#[derive(Clone, PartialEq)]
enum PlaceSeg {
    Field(String),
    IndexLit(i64),
    IndexDyn,
    Deref,
}

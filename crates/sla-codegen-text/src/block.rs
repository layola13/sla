//! Block emission, mirrored from codegen.zig's `genBlock` (7939),
//! `genScopedBlock` (8035), `genBlockTailValueStore` (8069) and the
//! loop-body local cleanup helpers (1059-1110, 8009-8033).
//!
//! Loop-body local tracking: `genBlock` remembers top-level `let`
//! names of the innermost loop body (via `rememberLoopBodyTopLevelLocal`,
//! depth-guarded) so the loop back-edge can release them per iteration.

use sla_ast::{Node, Type};
use sla_lowering::{is_discard_name, stmt_terminates};

use crate::codegen::{type_string, Codegen};
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// codegen.zig:7939 `genBlock`.
    pub(crate) fn gen_block(&mut self, block: &[Node]) -> Result<()> {
        self.enter_block_for_loop_local_tracking();
        let saved_tracking = self.enter_block_tracking(block);
        let result = self.gen_block_inner(block);
        self.exit_block_tracking(saved_tracking);
        self.leave_block_for_loop_local_tracking();
        result
    }

    fn gen_block_inner(&mut self, block: &[Node]) -> Result<()> {
        let mut scoped_aliases: Vec<String> = Vec::new();
        for (index, stmt) in block.iter().enumerate() {
            self.current_stmt_index = index;
            if let Node::LetStmt(let_stmt) = stmt {
                if !is_discard_name(&let_stmt.name) {
                    let source_name = let_stmt.name.clone();
                    let resolved_name = self.resolve_binding_name(&source_name);
                    if self.repeated_let_bindings.contains(&source_name)
                        || self.stack_alloc_bindings.contains(&resolved_name)
                    {
                        let alias = self.push_binding_alias(&source_name);
                        scoped_aliases.push(source_name.clone());
                        if self.addressable_bindings.contains(&source_name) {
                            self.addressable_bindings.insert(alias.clone());
                        }
                        if self.assigned_bindings.contains(&source_name) {
                            self.assigned_bindings.insert(alias.clone());
                        }
                        self.let_binding_aliases
                            .insert(sla_lowering::node_key(stmt), alias.clone());
                        // Generate the let under its alias name —
                        // gen_let_stmt resolves the name through
                        // let_binding_aliases. The ORIGINAL node
                        // must be generated, never a clone: checker
                        // facts (expr types, cleanups) are keyed by
                        // node address, so a cloned let's value
                        // misses every lookup (MissingType on the
                        // second same-name let in sibling blocks).
                        self.gen_stmt(stmt)?;
                        self.remember_loop_body_top_level_local(&alias);
                    } else {
                        self.gen_stmt(stmt)?;
                        self.remember_loop_body_top_level_local(&source_name);
                    }
                    if self.async_pending_return_emitted {
                        break;
                    }
                    continue;
                }
            }
            self.gen_stmt(stmt)?;
            // codegen.zig:468: a pending-await return ends the
            // block (stage 6 async cluster).
            if self.async_pending_return_emitted {
                break;
            }
        }
        for name in scoped_aliases.iter().rev() {
            self.pop_binding_alias(name);
        }
        Ok(())
    }

    /// codegen.zig:8035 `genScopedBlock`: `var` declarations get a
    /// scoped alias for the block's duration. Repeated `let`
    /// declarations get the same alias treatment as `gen_block`
    /// (DEF-RUST-CODEGEN-SCOPED-BLOCK-LET-ALIAS): without it a
    /// shadowing `let` inside a bare block resolves by name to the
    /// outer binding (e.g. an outer held iterator-chain plan) and
    /// the inner use is emitted as the builtin terminal (panic
    /// 31016) instead of the user fn.
    pub(crate) fn gen_scoped_block(&mut self, block: &[Node]) -> Result<()> {
        let mut scoped_var_aliases: Vec<String> = Vec::new();
        let mut scoped_let_aliases: Vec<String> = Vec::new();
        self.enter_block_for_loop_local_tracking();
        let saved_tracking = self.enter_block_tracking(block);
        let result = self.gen_scoped_block_inner(
            block,
            &mut scoped_var_aliases,
            &mut scoped_let_aliases,
        );
        self.exit_block_tracking(saved_tracking);
        self.leave_block_for_loop_local_tracking();
        for name in scoped_let_aliases.iter().rev() {
            self.pop_binding_alias(name);
        }
        for name in scoped_var_aliases.iter().rev() {
            self.pop_binding_alias(name);
        }
        result
    }

    fn gen_scoped_block_inner(
        &mut self,
        block: &[Node],
        scoped_var_aliases: &mut Vec<String>,
        scoped_let_aliases: &mut Vec<String>,
    ) -> Result<()> {
        for (index, stmt) in block.iter().enumerate() {
            self.current_stmt_index = index;
            if let Node::VarStmt(v) = stmt {
                self.push_binding_alias(&v.name);
                scoped_var_aliases.push(v.name.clone());
            }
            // Mirror of `gen_block_inner`'s repeated-`let` branch:
            // a shadowing `let` gets a fresh alias for the block's
            // duration so later uses resolve to the inner binding.
            if let Node::LetStmt(let_stmt) = stmt {
                if !is_discard_name(&let_stmt.name) {
                    let source_name = let_stmt.name.clone();
                    let resolved_name = self.resolve_binding_name(&source_name);
                    if self.repeated_let_bindings.contains(&source_name)
                        || self.stack_alloc_bindings.contains(&resolved_name)
                    {
                        let alias = self.push_binding_alias(&source_name);
                        scoped_let_aliases.push(source_name.clone());
                        if self.addressable_bindings.contains(&source_name) {
                            self.addressable_bindings.insert(alias.clone());
                        }
                        if self.assigned_bindings.contains(&source_name) {
                            self.assigned_bindings.insert(alias.clone());
                        }
                        self.let_binding_aliases
                            .insert(sla_lowering::node_key(stmt), alias.clone());
                        // Generate the let under its alias name —
                        // gen_let_stmt resolves the name through
                        // let_binding_aliases. The ORIGINAL node
                        // must be generated, never a clone: checker
                        // facts (expr types, cleanups) are keyed by
                        // node address, so a cloned let's value
                        // misses every lookup (MissingType on the
                        // second same-name let in sibling blocks).
                        self.gen_stmt(stmt)?;
                        self.remember_loop_body_top_level_local(&alias);
                    } else {
                        self.gen_stmt(stmt)?;
                        self.remember_loop_body_top_level_local(&source_name);
                    }
                    if self.async_pending_return_emitted {
                        break;
                    }
                    continue;
                }
            }
            self.gen_stmt(stmt)?;
        }
        Ok(())
    }

    /// codegen.zig:8069 `genBlockTailValueStore`: evaluate the block's
    /// tail expression and store it into the result slot. Identifier
    /// primitives are copied first; everything else stores directly.
    pub(crate) fn gen_block_tail_value_store(
        &mut self,
        block: &[Node],
        target_ptr: &str,
        target_ty: &Type,
    ) -> Result<()> {
        let saved_tracking = self.enter_block_tracking(block);
        let result = self.gen_block_tail_value_store_inner(block, target_ptr, target_ty);
        self.exit_block_tracking(saved_tracking);
        result
    }

    fn gen_block_tail_value_store_inner(
        &mut self,
        block: &[Node],
        target_ptr: &str,
        target_ty: &Type,
    ) -> Result<()> {
        if block.is_empty() {
            return unsupported("empty block tail value store", "codegen.zig:8069");
        }
        let split = block.len() - 1;
        for (index, stmt) in block[..split].iter().enumerate() {
            self.current_stmt_index = index;
            self.gen_stmt(stmt)?;
        }
        let last = &block[split];
        let value_expr = match last {
            Node::ExprStmt(expr) => expr,
            _ => return unsupported("block tail is not an expr statement", "codegen.zig:8069"),
        };
        self.current_stmt_index = split;
        // R2b (stage 6): a tail literal takes its layout from
        // the slot's target type.
        let value_reg = self.gen_expr_in_context(value_expr, Some(target_ty))?;
        let value_ty = self.require_expr_type(value_expr, "block tail value")?;

        if matches!(&**value_expr, Node::Identifier(_)) && matches!(value_ty, Type::Primitive(_)) {
            let copied = self.new_tmp();
            self.emit_primitive_copy(&copied, &value_reg, &value_ty)?;
            self.ins(&format!(
                "store {target_ptr}+0, {copied} as {}",
                type_string(target_ty)
            ));
            self.emit_release(&copied)?;
        } else {
            self.ins(&format!(
                "store {target_ptr}+0, {value_reg} as {}",
                type_string(target_ty)
            ));
        }
        // The RefCell companion branch of
        // `storeResultSlotTransferredValueState` (codegen.zig
        // :1234): when the target slot pre-allocated a companion
        // (borrow-typed if/match result slots do, before the
        // branch), a handle-carrying stored value moves its
        // handle into the companion slot, so the merged load can
        // restore it onto a path-valid register instead of
        // leaving branch-local guard registers in the merged
        // handle table. Slots without a pre-allocated companion
        // (let / assign targets, if-let chains) keep their own
        // bookkeeping paths and are untouched here.
        if self.result_slot_refcell_slots.contains_key(target_ptr) {
            self.store_refcell_handle_companion_for_slot(target_ptr, &value_reg);
        }
        // Zig `storeResultSlotTransferredValueState` (codegen.zig
        // :2279) store lifecycle:
        // planResultSlotStoreLifecycle over
        // planResultSlotTransfer(target_ty), whose transfers_value
        // is exactly abiPassesAsPointer(target_ty). A transferred
        // value leaves a temporary source register Active unless it
        // is retired with `^src` (the value now lives in the slot);
        // named bindings stay live. A non-transferred source is
        // released when the value expression's result needs it.
        // Without this, an EXPAND-produced tail value (e.g.
        // OPTION_NEW_SOME in an if branch) leaked at function exit
        // (MemoryLeak 1012).
        if sla_lowering::abi_passes_as_pointer(target_ty) {
            if sla_lowering::is_temporary_register_name(&value_reg)
                && !self.consumed_bindings.contains(&value_reg)
            {
                self.emit_forget_moved_value(&value_reg)?;
            }
        } else if sla_lowering::call_arg_needs_release(value_expr) {
            self.emit_release(&value_reg)?;
        }

        if !stmt_terminates(last) {
            if let Some(list) = self.check.cleanups(last) {
                let names: Vec<String> = list.to_vec();
                for name in names {
                    self.emit_release(&name)?;
                }
            }
        }
        Ok(())
    }

    /// codegen.zig `genBlockTailValueInto` (value-into-register form).
    /// Only the shapes the slice can produce are ported; struct copies
    /// route through the (deferred) deep-copy emitter.
    pub(crate) fn gen_block_tail_value_into(
        &mut self,
        block: &[Node],
        target: &str,
    ) -> Result<()> {
        let saved_tracking = self.enter_block_tracking(block);
        let result = self.gen_block_tail_value_into_inner(block, target);
        self.exit_block_tracking(saved_tracking);
        result
    }

    fn gen_block_tail_value_into_inner(
        &mut self,
        block: &[Node],
        target: &str,
    ) -> Result<()> {
        if block.is_empty() {
            return unsupported("empty block tail value into", "codegen.zig:8039");
        }
        let split = block.len() - 1;
        for (index, stmt) in block[..split].iter().enumerate() {
            self.current_stmt_index = index;
            self.gen_stmt(stmt)?;
        }
        let last = &block[split];
        let value_expr = match last {
            Node::ExprStmt(expr) => expr,
            _ => return unsupported("block tail is not an expr statement", "codegen.zig:8039"),
        };
        self.current_stmt_index = split;
        let value_reg = self.gen_expr(value_expr)?;
        let value_ty = self.require_expr_type(value_expr, "block tail value")?;
        if matches!(&**value_expr, Node::Identifier(_)) && matches!(value_ty, Type::Primitive(_)) {
            self.emit_primitive_copy(target, &value_reg, &value_ty)?;
        } else {
            self.ins(&format!("{target} = {value_reg}"));
        }
        if !stmt_terminates(last) {
            if let Some(list) = self.check.cleanups(last) {
                let names: Vec<String> = list.to_vec();
                for name in names {
                    self.emit_release(&name)?;
                }
            }
        }
        Ok(())
    }

    // -- loop-body local tracking (codegen.zig:1059-1110) -------------

    pub(crate) fn push_loop_body_local_scope(&mut self) {
        self.loop_body_local_scopes.push(Vec::new());
        self.loop_body_block_depths.push(0);
    }

    pub(crate) fn pop_loop_body_local_scope(&mut self) {
        self.loop_body_local_scopes.pop();
        self.loop_body_block_depths.pop();
    }

    pub(crate) fn enter_block_for_loop_local_tracking(&mut self) {
        for depth in &mut self.loop_body_block_depths {
            *depth += 1;
        }
    }

    pub(crate) fn leave_block_for_loop_local_tracking(&mut self) {
        for depth in &mut self.loop_body_block_depths {
            *depth -= 1;
        }
    }

    /// codegen.zig:1072 `rememberLoopBodyTopLevelLocal`.
    pub(crate) fn remember_loop_body_top_level_local(&mut self, name: &str) {
        if self.loop_body_local_scopes.is_empty() {
            return;
        }
        let scope_index = self.loop_body_local_scopes.len() - 1;
        if self.loop_body_block_depths[scope_index] > 1 {
            return;
        }
        self.loop_body_local_scopes[scope_index].push(name.to_string());
    }

    pub(crate) fn active_loop_body_local_contains_name(&self, name: &str) -> bool {
        let scope = match self.loop_body_local_scopes.last() {
            None => return false,
            Some(s) => s,
        };
        let resolved = self.resolve_binding_name(name);
        scope
            .iter()
            .any(|item| self.resolve_binding_name(item) == resolved)
    }

    fn cleanup_list_contains_name(&self, list: Option<&[String]>, name: &str) -> bool {
        let list = match list {
            None => return false,
            Some(l) => l,
        };
        let resolved = self.resolve_binding_name(name);
        list.iter()
            .any(|item| self.resolve_binding_name(item) == resolved)
    }

    /// codegen.zig:1090 `emitLoopBodyLocalCleanup`.
    pub(crate) fn emit_loop_body_local_cleanup(
        &mut self,
        name: &str,
        force_consumed_primitive: bool,
    ) -> Result<()> {
        let resolved = self.resolve_binding_name(name);
        if force_consumed_primitive {
            if let Some(ty) = self.local_binding_type_for_name(&resolved) {
                if matches!(ty, Type::Primitive(_)) && self.consumed_bindings.contains(&resolved) {
                    self.ins(&format!("!{resolved}"));
                    return Ok(());
                }
            }
        }
        self.emit_release(&resolved)
    }

    /// codegen.zig:1100 `emitActiveLoopBodyLocalCleanups`.
    pub(crate) fn emit_active_loop_body_local_cleanups(
        &mut self,
        skip_list: Option<&[String]>,
        force_consumed_primitive: bool,
    ) -> Result<()> {
        let scope = match self.loop_body_local_scopes.last() {
            None => return Ok(()),
            Some(s) => s.clone(),
        };
        for name in scope.iter().rev() {
            if !self.cleanup_list_contains_name(skip_list, name) {
                self.emit_loop_body_local_cleanup(name, force_consumed_primitive)?;
            }
        }
        Ok(())
    }

    /// codegen.zig:8009 `emitLoopBodyTopLevelLocalCleanups`: reverse
    /// order over the block; only const and destructure bindings are
    /// released here (plain lets are the active-scope list's job).
    pub(crate) fn emit_loop_body_top_level_local_cleanups(&mut self, block: &[Node]) -> Result<()> {
        for stmt in block.iter().rev() {
            match stmt {
                Node::ConstStmt(c) => {
                    if !is_discard_name(&c.name) {
                        self.emit_release(&c.name)?;
                    }
                }
                Node::LetDestructureStmt(let_stmt) => {
                    if let Some(rest_alias) = &let_stmt.rest_alias {
                        if !is_discard_name(rest_alias) {
                            self.emit_release(rest_alias)?;
                        }
                    }
                    if let Some(rest_name) = &let_stmt.rest_name {
                        if !is_discard_name(rest_name) {
                            self.emit_release(rest_name)?;
                        }
                    }
                    for name in let_stmt.names.iter().rev() {
                        if !is_discard_name(name) {
                            self.emit_release(name)?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

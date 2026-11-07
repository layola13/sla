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
        let result = self.gen_block_inner(block);
        self.leave_block_for_loop_local_tracking();
        result
    }

    fn gen_block_inner(&mut self, block: &[Node]) -> Result<()> {
        let mut scoped_aliases: Vec<String> = Vec::new();
        for stmt in block {
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
                        // Generate the let under its alias name.
                        let mut aliased = let_stmt.clone();
                        aliased.name = alias.clone();
                        self.gen_stmt(&Node::LetStmt(aliased))?;
                        self.remember_loop_body_top_level_local(&alias);
                    } else {
                        self.gen_stmt(stmt)?;
                        self.remember_loop_body_top_level_local(&source_name);
                    }
                    continue;
                }
            }
            self.gen_stmt(stmt)?;
        }
        for name in scoped_aliases.iter().rev() {
            self.pop_binding_alias(name);
        }
        Ok(())
    }

    /// codegen.zig:8035 `genScopedBlock`: `var` declarations get a
    /// scoped alias for the block's duration.
    pub(crate) fn gen_scoped_block(&mut self, block: &[Node]) -> Result<()> {
        let mut scoped_var_aliases: Vec<String> = Vec::new();
        for stmt in block {
            if let Node::VarStmt(v) = stmt {
                self.push_binding_alias(&v.name);
                scoped_var_aliases.push(v.name.clone());
            }
            self.gen_stmt(stmt)?;
        }
        for name in scoped_var_aliases.iter().rev() {
            self.pop_binding_alias(name);
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
        if block.is_empty() {
            return unsupported("empty block tail value store", "codegen.zig:8069");
        }
        let split = block.len() - 1;
        for stmt in &block[..split] {
            self.gen_stmt(stmt)?;
        }
        let last = &block[split];
        let value_expr = match last {
            Node::ExprStmt(expr) => expr,
            _ => return unsupported("block tail is not an expr statement", "codegen.zig:8069"),
        };
        let value_reg = self.gen_expr(value_expr)?;
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
        // (Zig: storeResultSlotTransferredValueState — RefCell state
        // transfer bookkeeping, untracked in the slice.)

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
        if block.is_empty() {
            return unsupported("empty block tail value into", "codegen.zig:8039");
        }
        let split = block.len() - 1;
        for stmt in &block[..split] {
            self.gen_stmt(stmt)?;
        }
        let last = &block[split];
        let value_expr = match last {
            Node::ExprStmt(expr) => expr,
            _ => return unsupported("block tail is not an expr statement", "codegen.zig:8039"),
        };
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

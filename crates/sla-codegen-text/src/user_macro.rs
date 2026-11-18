//! User-macro call expansion (codegen.zig `genUserMacroCallInline`
//! family, :648-1000): a user macro call is expanded inline at the
//! call site — arguments are evaluated once, parameters are bound
//! to the argument registers through the binding-alias stack, body
//! locals are hygiene-renamed, and temp arguments the body did not
//! consume are retired afterwards (DEF-TEXT31-USER-MACRO-EVAL-ARG-LEAK).
//!
//! The Rust lane never clones AST nodes (checker facts are keyed by
//! node address), so where Zig renames a body `let` by generating a
//! renamed stack copy, this port registers the hygiene alias in
//! `let_binding_aliases` and generates the ORIGINAL node: `gen_stmt`'s
//! own epilogue then emits the statement-exit cleanups Zig has to
//! emit explicitly against the original node
//! (DEF-TEXT31-USER-MACRO-AGG-LOCAL-LEAK is the Zig-side workaround
//! for the copy's pointer miss; it has no counterpart here).

use sla_ast::{CallExpr, MacroDecl, Node, Type};
use sla_lowering::{
    call_arg_needs_release, is_borrow_like_type, is_discard_name,
    is_temporary_register_name, node_key, stmt_terminates,
};

use crate::codegen::{type_size, type_string, Codegen};
use crate::{Result, TextError};

/// A pre-evaluated first argument for the inline: the register is
/// used as-is instead of running gen_expr on the argument node
/// (J-4 binds its synthetic output slot this way — the slot has no
/// source-level binding gen_expr could resolve).
struct PreboundMacroArg {
    reg: String,
    ty: Option<Type>,
}

struct SavedArg {
    name: String,
    old_expr: Option<usize>,
    old_ty: Option<Type>,
}

impl<'c> Codegen<'c> {
    /// Dispatch for a user-macro call in expression position (Zig
    /// genExpr call arm): the J-4 expression form when the leading
    /// output argument is omitted, else the plain inline plus the
    /// zero sentinel register.
    pub(crate) fn gen_user_macro_call(
        &mut self,
        call_node: &Node,
        call: &CallExpr,
    ) -> Result<String> {
        let macro_decl = self.macro_decl_or_err(&call.func_name)?;
        if call.args.len() + 1 == macro_decl.params.len() {
            return self.gen_user_macro_call_expr_inline(&macro_decl, call_node, call);
        }
        let args: Vec<&Node> = call.args.iter().collect();
        self.gen_user_macro_call_inline_prebound(&macro_decl, &args, call_node, None)?;
        let sentinel = self.new_tmp();
        self.ins(&format!("{sentinel} = 0"));
        Ok(sentinel)
    }

    /// Dispatch for a user-macro call in statement position (Zig
    /// genCallStmt macro arm): the J-4 form runs and its value is
    /// discarded; the plain form is the bare inline.
    pub(crate) fn gen_user_macro_call_stmt(
        &mut self,
        call_node: &Node,
        call: &CallExpr,
    ) -> Result<()> {
        let macro_decl = self.macro_decl_or_err(&call.func_name)?;
        if call.args.len() + 1 == macro_decl.params.len() {
            let _ = self.gen_user_macro_call_expr_inline(&macro_decl, call_node, call)?;
            return Ok(());
        }
        let args: Vec<&Node> = call.args.iter().collect();
        self.gen_user_macro_call_inline_prebound(&macro_decl, &args, call_node, None)
    }

    fn macro_decl_or_err(&self, name: &str) -> Result<MacroDecl> {
        self.check.macro_decl(name).cloned().ok_or_else(|| {
            TextError::MissingType {
                context: format!("user macro declaration for `{name}`"),
            }
        })
    }

    /// J-4 (leading_output_convention): expression form of a user
    /// macro call — the leading output argument is omitted at the
    /// call site. A fresh stack slot stands in for it: the ordinary
    /// statement-form inline runs with a synthetic identifier
    /// argument bound to the slot, and the slot is loaded back as
    /// the expression value (codegen.zig:964 `genUserMacroCallExprInline`).
    fn gen_user_macro_call_expr_inline(
        &mut self,
        macro_decl: &MacroDecl,
        call_node: &Node,
        call: &CallExpr,
    ) -> Result<String> {
        let result_ty = macro_decl
            .param_tys
            .first()
            .cloned()
            .flatten()
            .ok_or_else(|| TextError::MissingType {
                context: "J-4 macro leading output declared type".to_string(),
            })?;
        let slot = self.new_tmp();
        self.stack_alloc_bindings.insert(slot.clone());
        self.assigned_value_slots.insert(slot.clone());
        self.ins(&format!("{slot} = stack_alloc {}", type_size(&result_ty)));
        // Zig `prepareResultSlotRefCellCompanion` is plan-gated on
        // RefCell-carrier result types; the J-4 shapes in the corpus
        // are scalar outputs, for which the plan needs no companion.
        let slot_node = Node::Identifier(slot.clone());
        let mut args: Vec<&Node> = vec![&slot_node];
        args.extend(call.args.iter());
        self.gen_user_macro_call_inline_prebound(
            macro_decl,
            &args,
            call_node,
            Some(PreboundMacroArg {
                reg: slot.clone(),
                ty: Some(result_ty.clone()),
            }),
        )?;
        let result_reg = self.new_tmp();
        self.ins(&format!(
            "{result_reg} = load {slot}+0 as {}",
            type_string(&result_ty)
        ));
        // Zig `loadResultSlotTransferredValueState` is likewise
        // plan-gated; scalar outputs carry no transferred state.
        Ok(result_reg)
    }

    /// codegen.zig:843 `genUserMacroCallInlinePrebound`.
    fn gen_user_macro_call_inline_prebound(
        &mut self,
        macro_decl: &MacroDecl,
        args: &[&Node],
        cleanup_node: &Node,
        prebound_first: Option<PreboundMacroArg>,
    ) -> Result<()> {
        if macro_decl.params.len() != args.len() {
            return Err(TextError::MissingType {
                context: format!(
                    "user macro `{}` arity (checker should have rejected)",
                    macro_decl.name
                ),
            });
        }

        // Evaluate each argument once, in order.
        let mut arg_regs: Vec<String> = Vec::with_capacity(args.len());
        let mut arg_types: Vec<Option<Type>> = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            if index == 0 {
                if let Some(pre) = &prebound_first {
                    arg_regs.push(pre.reg.clone());
                    arg_types.push(pre.ty.clone());
                    continue;
                }
            }
            let arg_ty = self.resolved_type_for_expr(arg);
            let reg = self.gen_expr(arg)?;
            arg_regs.push(reg);
            arg_types.push(arg_ty);
        }

        // Bind parameters to the argument registers, saving the
        // outer macro-arg state for restoration (nested inlines).
        let mut saved_args: Vec<SavedArg> = Vec::with_capacity(args.len());
        let mut aliased_params: Vec<String> = Vec::with_capacity(args.len());
        for (index, param) in macro_decl.params.iter().enumerate() {
            saved_args.push(SavedArg {
                name: param.clone(),
                old_expr: self.macro_arg_exprs.get(param).copied(),
                old_ty: self.macro_arg_types.get(param).cloned(),
            });
            self.push_binding_alias_to(param, &arg_regs[index]);
            aliased_params.push(param.clone());
            self.macro_arg_exprs
                .insert(param.clone(), node_key(args[index]));
            match &arg_types[index] {
                Some(ty) => {
                    self.macro_arg_types.insert(param.clone(), ty.clone());
                }
                None => {
                    self.macro_arg_types.remove(param);
                }
            }
        }

        self.macro_inline_depth += 1;
        let previous_inline_macro = self.active_inline_macro.clone();
        self.active_inline_macro = Some(macro_decl.clone());
        let previous_try_cleanup = self.active_macro_try_cleanup.clone();
        if let Some(list) = self.check.macro_call_try_cleanups(cleanup_node) {
            self.active_macro_try_cleanup = Some(list.to_vec());
        }

        let body_result =
            self.gen_user_macro_block_inline(macro_decl, &macro_decl.body);

        // DEF-TEXT31-USER-MACRO-EVAL-ARG-LEAK: retire each
        // temp-materialising argument the body did not itself
        // consume (see the Zig comment at the retirement loop).
        // The store-moved half of Zig's consumed/store-moved pair
        // has no separate set in this lane; the lane-faithful form
        // is the consumed probe plus the body-consumption scan
        // (release.rs records the same precedent for tail cleanups).
        if body_result.is_ok() && !self.async_pending_return_emitted {
            for (index, arg) in args.iter().enumerate() {
                if index == 0 && prebound_first.is_some() {
                    continue;
                }
                let arg_reg = &arg_regs[index];
                if !call_arg_needs_release(arg)
                    && !(matches!(arg, Node::CastExpr(_))
                        && self.expr_result_reg_needs_release(arg))
                {
                    continue;
                }
                if !is_temporary_register_name(arg_reg) {
                    continue;
                }
                if self.stack_alloc_bindings.contains(arg_reg) {
                    continue;
                }
                if let Some(ty) = &arg_types[index] {
                    if is_borrow_like_type(ty) {
                        continue;
                    }
                }
                if self.consumed_bindings.contains(arg_reg) {
                    continue;
                }
                if self
                    .check
                    .macro_body_consumes_param(&macro_decl.name, &macro_decl.params[index])
                {
                    continue;
                }
                self.emit_release(arg_reg)?;
            }
        }

        // Restore the saved state (Zig's deferred restores).
        self.macro_inline_depth -= 1;
        self.active_inline_macro = previous_inline_macro;
        self.active_macro_try_cleanup = previous_try_cleanup;
        for saved in saved_args.iter().rev() {
            match saved.old_expr {
                Some(key) => {
                    self.macro_arg_exprs.insert(saved.name.clone(), key);
                }
                None => {
                    self.macro_arg_exprs.remove(&saved.name);
                }
            }
            match &saved.old_ty {
                Some(ty) => {
                    self.macro_arg_types.insert(saved.name.clone(), ty.clone());
                }
                None => {
                    self.macro_arg_types.remove(&saved.name);
                }
            }
        }
        for param in aliased_params.iter().rev() {
            self.pop_binding_alias(param);
        }
        body_result
    }

    /// codegen.zig:630 `genUserMacroBlockInline`.
    fn gen_user_macro_block_inline(
        &mut self,
        macro_decl: &MacroDecl,
        body: &[Node],
    ) -> Result<()> {
        let mut scoped_aliases: Vec<String> = Vec::new();
        let mut scoped_types: Vec<(String, Option<Type>)> = Vec::new();
        let mut result: Result<()> = Ok(());
        for stmt in body {
            let step = self.gen_user_macro_block_stmt(
                macro_decl,
                stmt,
                &mut scoped_aliases,
                &mut scoped_types,
            );
            if let Err(e) = step {
                result = Err(e);
                break;
            }
            if self.async_pending_return_emitted {
                break;
            }
        }
        // Block-exit cleanups: the Zig checker records them on the
        // block's last statement (checkMacro) and genStmt's
        // epilogue emits them; this lane's checker records no
        // cleanups for macro body statements, so the block's own
        // hygiene-renamed lets are released here, in reverse
        // declaration order, skipping any the body consumed.
        if result.is_ok() && !self.async_pending_return_emitted {
            let aliases: Vec<String> = scoped_aliases
                .iter()
                .map(|name| self.resolve_binding_name(name))
                .collect();
            for alias in aliases.iter().rev() {
                if !self.consumed_bindings.contains(alias) {
                    if let Err(e) = self.emit_release(alias) {
                        result = Err(e);
                        break;
                    }
                }
            }
        }
        for (name, old_ty) in scoped_types.iter().rev() {
            match old_ty {
                Some(ty) => {
                    self.macro_arg_types.insert(name.clone(), ty.clone());
                }
                None => {
                    self.macro_arg_types.remove(name);
                }
            }
        }
        for name in scoped_aliases.iter().rev() {
            self.pop_binding_alias(name);
        }
        result
    }

    fn gen_user_macro_block_stmt(
        &mut self,
        macro_decl: &MacroDecl,
        stmt: &Node,
        scoped_aliases: &mut Vec<String>,
        scoped_types: &mut Vec<(String, Option<Type>)>,
    ) -> Result<()> {
        match stmt {
            Node::LetStmt(let_stmt) if !is_discard_name(&let_stmt.name) => {
                let alias = self.new_inline_macro_local(&macro_decl.name, &let_stmt.name);
                // Generate the ORIGINAL node under the hygiene
                // alias (let_binding_aliases is how gen_let_stmt
                // resolves the let's register name); gen_stmt's
                // epilogue emits this statement's exit cleanups,
                // which Zig emits explicitly after its renamed copy.
                self.let_binding_aliases
                    .insert(node_key(stmt), alias.clone());
                let step = self.gen_stmt(stmt);
                self.let_binding_aliases.remove(&node_key(stmt));
                step?;
                self.push_binding_alias_to(&let_stmt.name, &alias);
                scoped_aliases.push(let_stmt.name.clone());
                scoped_types.push((
                    let_stmt.name.clone(),
                    self.macro_arg_types.get(&let_stmt.name).cloned(),
                ));
                if let Some(explicit) = &let_stmt.ty {
                    self.macro_arg_types
                        .insert(let_stmt.name.clone(), (**explicit).clone());
                } else if let Some(inferred) =
                    self.resolved_type_for_expr(&let_stmt.value)
                {
                    self.macro_arg_types
                        .insert(let_stmt.name.clone(), inferred);
                } else {
                    self.macro_arg_types.remove(&let_stmt.name);
                }
                Ok(())
            }
            Node::BlockStmt(block) => {
                self.gen_user_macro_block_inline(macro_decl, &block.body)?;
                // The recursion bypasses gen_stmt for the block
                // statement itself, so emit its exit cleanups here,
                // exactly as gen_stmt's epilogue would.
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
            Node::ForStmt(for_stmt) => {
                let alias =
                    self.new_inline_macro_local(&macro_decl.name, &for_stmt.var_name);
                let counter_slot = format!("{alias}_slot");
                self.ins(&format!("{counter_slot} = stack_alloc 8"));
                self.stack_alloc_bindings.insert(counter_slot.clone());
                // The loop variable's type is a checker fact in
                // ordinary code; record it in the macro-arg table
                // for the loop's duration so body reads resolve it.
                let var_ty = if for_stmt.end.is_some() {
                    self.resolved_type_for_expr(&for_stmt.start)
                } else {
                    self.resolved_type_for_expr(&for_stmt.start)
                        .and_then(|ty| {
                            match sla_lowering::peel_borrow_pointer_type(&ty) {
                                Type::Array(arr) => Some((*arr.elem).clone()),
                                Type::UserDefined(u) => {
                                    u.generics.first().cloned()
                                }
                                _ => None,
                            }
                        })
                };
                let old_var_ty =
                    self.macro_arg_types.get(&for_stmt.var_name).cloned();
                if let Some(ty) = var_ty {
                    self.macro_arg_types
                        .insert(for_stmt.var_name.clone(), ty);
                }
                self.for_slot_override = Some((node_key(stmt), counter_slot));
                let step = self.gen_stmt(stmt);
                self.for_slot_override = None;
                match old_var_ty {
                    Some(ty) => {
                        self.macro_arg_types
                            .insert(for_stmt.var_name.clone(), ty);
                    }
                    None => {
                        self.macro_arg_types.remove(&for_stmt.var_name);
                    }
                }
                step
            }
            _ => self.gen_stmt(stmt),
        }
    }

    /// codegen.zig:725 `genUserMacroUnsafeValueInline`: an unsafe
    /// block inside an active inline macro is a value form — its
    /// leading statements run under hygiene renaming and its final
    /// expression statement is the value.
    pub(crate) fn gen_user_macro_unsafe_value_inline(
        &mut self,
        macro_decl: &MacroDecl,
        body: &[Node],
    ) -> Result<String> {
        let (last, head) = body.split_last().ok_or_else(|| TextError::MissingType {
            context: "unsafe macro value block is empty".to_string(),
        })?;
        let value_expr: &Node = match last {
            Node::ExprStmt(expr) => expr,
            _ => {
                return Err(TextError::MissingType {
                    context: "unsafe macro value block does not end in an expression"
                        .to_string(),
                })
            }
        };
        let mut scoped_aliases: Vec<String> = Vec::new();
        let mut scoped_types: Vec<(String, Option<Type>)> = Vec::new();
        let mut run = |this: &mut Self| -> Result<String> {
            for stmt in head {
                match stmt {
                    Node::LetStmt(let_stmt)
                        if !is_discard_name(&let_stmt.name) =>
                    {
                        let alias = this
                            .new_inline_macro_local(&macro_decl.name, &let_stmt.name);
                        this.let_binding_aliases
                            .insert(node_key(stmt), alias.clone());
                        let step = this.gen_stmt(stmt);
                        this.let_binding_aliases.remove(&node_key(stmt));
                        step?;
                        this.push_binding_alias_to(&let_stmt.name, &alias);
                        scoped_aliases.push(let_stmt.name.clone());
                        scoped_types.push((
                            let_stmt.name.clone(),
                            this.macro_arg_types.get(&let_stmt.name).cloned(),
                        ));
                        if let Some(explicit) = &let_stmt.ty {
                            this.macro_arg_types
                                .insert(let_stmt.name.clone(), (**explicit).clone());
                        } else if let Some(inferred) =
                            this.resolved_type_for_expr(&let_stmt.value)
                        {
                            this.macro_arg_types
                                .insert(let_stmt.name.clone(), inferred);
                        }
                    }
                    _ => this.gen_stmt(stmt)?,
                }
            }
            let value_reg = this.gen_expr(value_expr)?;
            let value_ty = this
                .resolved_type_for_expr(value_expr)
                .ok_or_else(|| TextError::MissingType {
                    context: "unsafe macro value type".to_string(),
                })?;
            let result = this.new_tmp();
            if matches!(value_expr, Node::Identifier(_))
                && matches!(value_ty, Type::Primitive(_))
            {
                this.emit_primitive_copy(&result, &value_reg, &value_ty)?;
            } else {
                this.ins(&format!("{result} = {value_reg}"));
            }
            if let Some(list) = this.check.cleanups(last) {
                let names: Vec<String> = list.to_vec();
                for name in names {
                    this.emit_release(&name)?;
                }
            }
            Ok(result)
        };
        let result = run(self);
        for (name, old_ty) in scoped_types.iter().rev() {
            match old_ty {
                Some(ty) => {
                    self.macro_arg_types.insert(name.clone(), ty.clone());
                }
                None => {
                    self.macro_arg_types.remove(name);
                }
            }
        }
        for name in scoped_aliases.iter().rev() {
            self.pop_binding_alias(name);
        }
        result
    }
}

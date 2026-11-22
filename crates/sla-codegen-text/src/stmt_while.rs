//! While statement emission, mirrored from codegen.zig's
//! `genWhileStmt` plain path (11431-11545). The while-let / pattern
//! arms route through the pattern-matching family and are deferred:
//! they fail loudly here. Range `for` statements lower onto the
//! same loop skeleton with the hoisted counter slot
//! (`{var}_slot`, emitted at function entry by
//! emit_hoisted_and_counter_slots) as the loop variable's home.
//! Collection `for` statements (codegen.zig `genForStmt`, the
//! no-`end` arm) count an i64 index in the same slot and bind the
//! loop variable per iteration — from the array image, or through
//! the iterable protocol (`iter_len` once, `iter_at` per index).

use sla_ast::{EnumPattern, Node, Type, WhileStmt};
use sla_lowering::{block_terminates, node_key};

use crate::codegen::{type_size, type_string, Codegen};
use crate::{unsupported, Result, TextError};

impl<'c> Codegen<'c> {
    /// codegen.zig:11431 `genWhileStmt` (plain condition path).
    pub(crate) fn gen_while_stmt(&mut self, stmt: &Node) -> Result<()> {
        let while_stmt = match stmt {
            Node::WhileStmt(w) => w,
            _ => unreachable!(),
        };
        if let Some(pattern) = &while_stmt.let_pattern {
            return self.gen_while_let_stmt(while_stmt, pattern);
        }
        // Task 114: Vec-header local lift. The analysis runs before
        // any emission for this loop; the setup loads run once,
        // before the head jump; the lift stays on the stack while
        // the body is emitted so the inline push/pop arms find it.
        let lifts = self.plan_vec_lift(while_stmt);
        if !lifts.is_empty() {
            self.emit_vec_lift_setup(&lifts);
            self.vec_header_lifts.extend(lifts.iter().cloned());
        }
        let head_label = self.new_label("L_WHILE_HEAD");
        let body_label = self.new_label("L_WHILE_BODY");
        let cond_false_label = self.new_label("L_WHILE_COND_FALSE");
        let exit_label = self.new_label("L_WHILE_EXIT");

        self.w(&format!("    jmp {head_label}\n\n"));
        self.wl(&format!("{head_label}:"));
        let cond_reg = self.gen_branch_condition_reg(&while_stmt.cond)?;
        self.w(&format!(
            "    br {cond_reg} -> {body_label}, {cond_false_label}\n\n"
        ));
        self.wl(&format!("{body_label}:"));
        // Zig prints the condition release raw (not via emitRelease).
        self.w(&format!("    !{cond_reg}\n"));

        self.loop_continue_labels.push(head_label.clone());
        self.loop_break_labels.push(exit_label.clone());
        // Zig: `defer self.popLoopBodyLocalScope()` — the scope
        // stays live across the back-edge cleanups below and is
        // popped on EVERY exit path, including a gen_block error.
        // The previous shape popped immediately after gen_block
        // (so the cleanups read the OUTER scope or none) and
        // skipped the pop entirely on the error path, leaking a
        // zero depth entry that the enclosing gen_block's
        // leave-tracking then decremented — a subtract-overflow
        // panic masking the real error (14 corpus files).
        self.push_loop_body_local_scope();
        let mut flow = self.gen_block(&while_stmt.body);
        self.loop_continue_labels.pop();
        self.loop_break_labels.pop();
        // Task 114: retire the lift as soon as the body is emitted
        // (mirroring the loop-label pops above, so an error path
        // cannot leak it into a later loop). The write-back below
        // uses the local `lifts`, not the stack.
        if !lifts.is_empty() {
            let n = lifts.len();
            self.vec_header_lifts
                .truncate(self.vec_header_lifts.len() - n);
        }

        if flow.is_ok() && !block_terminates(&while_stmt.body) {
            flow = self.emit_active_loop_body_local_cleanups(None, false);
            if flow.is_ok() {
                flow = self.emit_loop_body_top_level_local_cleanups(&while_stmt.body);
            }
            if flow.is_ok() {
                self.w(&format!("    jmp {head_label}\n\n"));
            }
        }
        self.pop_loop_body_local_scope();
        flow?;
        self.wl(&format!("{cond_false_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        self.w(&format!("    jmp {exit_label}\n\n"));
        self.wl(&format!("{exit_label}:"));
        // Task 114: write the lifted header fields back. This runs
        // at the exit label, so `break` (which jumps straight here)
        // hits it too; a `return` in the body voids the lift in the
        // analysis, so no return can bypass it.
        if !lifts.is_empty() {
            self.emit_vec_lift_writeback(&lifts);
        }
        Ok(())
    }

    /// codegen.zig:11440 `genWhileStmt` let_pattern path, the
    /// Option/Result macro forms: the head evaluates the
    /// condition value, tests it with OPTION_IS_SOME /
    /// RESULT_IS_OK into a branch flag, and the body edge
    /// extracts the single payload binding with OPTION_GET /
    /// RESULT_GET_OK / RESULT_GET_ERR before retiring the
    /// condition register. User-enum patterns (Zig's
    /// genEnumPatternCheck arm) stay loudly unsupported.
    pub(crate) fn gen_while_let_stmt(
        &mut self,
        while_stmt: &WhileStmt,
        pattern: &EnumPattern,
    ) -> Result<()> {
        let variant = pattern.variant_name.as_str();
        let (check_macro, get_macro, success_on_true, payload_index) = match variant {
            "Some" => ("OPTION_IS_SOME", "OPTION_GET", true, 0usize),
            "None" => ("OPTION_IS_SOME", "", false, 0usize),
            "Ok" => ("RESULT_IS_OK", "RESULT_GET_OK", true, 0usize),
            "Err" => ("RESULT_IS_OK", "RESULT_GET_ERR", false, 1usize),
            _ => {
                return unsupported(
                    "while-let statement over a user-enum pattern",
                    "codegen.zig:11440 (while let_pattern enum arm)",
                )
            }
        };
        if pattern.bindings.len() > 1 || (variant == "None" && !pattern.bindings.is_empty()) {
            return unsupported(
                "while-let statement with multiple pattern bindings",
                "codegen.zig:11440 (while let_pattern bindings)",
            );
        }
        let head_label = self.new_label("L_WHILE_HEAD");
        let body_label = self.new_label("L_WHILE_BODY");
        let cond_false_label = self.new_label("L_WHILE_COND_FALSE");
        let exit_label = self.new_label("L_WHILE_EXIT");

        self.w(&format!("    jmp {head_label}\n\n"));
        self.wl(&format!("{head_label}:"));
        let cond_reg = self.gen_branch_condition_reg(&while_stmt.cond)?;
        let cond_ty = self.expr_type(&while_stmt.cond);
        let branch_flag = self.new_tmp();
        self.ins(&format!("EXPAND {check_macro} {branch_flag}, {cond_reg}"));
        if success_on_true {
            self.w(&format!(
                "    br {branch_flag} -> {body_label}, {cond_false_label}\n\n"
            ));
        } else {
            self.w(&format!(
                "    br {branch_flag} -> {cond_false_label}, {body_label}\n\n"
            ));
        }
        self.wl(&format!("{body_label}:"));
        self.w(&format!("    !{branch_flag}\n"));
        // Payload binding (Zig: pushBindingAlias + the GET
        // macro), then the condition register retires.
        let mut pushed: Vec<String> = Vec::new();
        if let Some(binding) = pattern.bindings.first() {
            let alias = self.push_binding_alias(binding);
            self.ins(&format!("EXPAND {get_macro} {alias}, {cond_reg}"));
            if let Some(Type::UserDefined(u)) = &cond_ty {
                if let Some(payload_ty) = u.generics.get(payload_index) {
                    let payload_ty = payload_ty.clone();
                    self.remember_local_binding_type(&alias, &payload_ty);
                }
            }
            pushed.push(binding.clone());
        }
        self.emit_release(&cond_reg)?;

        self.loop_continue_labels.push(head_label.clone());
        self.loop_break_labels.push(exit_label.clone());
        self.push_loop_body_local_scope();
        let pre_loop_borrow_sources = self.borrow_source_temps.clone();
        let pre_loop_refcell_handles = self.refcell_borrow_handles.clone();
        let mut flow = self.gen_block(&while_stmt.body);
        self.loop_continue_labels.pop();
        self.loop_break_labels.pop();
        if flow.is_ok()
            && crate::codegen::plan_refcell_loop_state_merge_restores_pre_loop()
        {
            flow = self.restore_refcell_branch_state(
                &pre_loop_refcell_handles,
                &pre_loop_borrow_sources,
            );
        }
        if flow.is_ok() && !block_terminates(&while_stmt.body) {
            // Zig releases each pattern binding the body did
            // not consume; emit_release no-ops for consumed /
            // stack / borrow registers, so printing it for
            // every binding is the same set.
            for binding in &pattern.bindings {
                let alias = self.resolve_binding_name(binding);
                if flow.is_ok() {
                    flow = self.emit_release(&alias);
                }
            }
            if flow.is_ok() {
                flow = self.emit_active_loop_body_local_cleanups(None, false);
            }
            if flow.is_ok() {
                flow = self.emit_loop_body_top_level_local_cleanups(&while_stmt.body);
            }
            if flow.is_ok() {
                self.w(&format!("    jmp {head_label}\n\n"));
            }
        }
        self.pop_loop_body_local_scope();
        for binding in pushed.iter().rev() {
            self.pop_binding_alias(binding);
        }
        flow?;
        self.wl(&format!("{cond_false_label}:"));
        self.w(&format!("    !{branch_flag}\n"));
        self.emit_release(&cond_reg)?;
        self.w(&format!("    jmp {exit_label}\n\n"));
        self.wl(&format!("{exit_label}:"));
        Ok(())
    }

    /// Range `for var in start..end` over the hoisted counter
    /// slot. The collection form (no end) routes to
    /// `gen_for_collection_stmt`. The slot is memory, so the
    /// loop has no register-phi surface; the body runs under
    /// the same loop-local scope discipline as `while`, with
    /// `continue` landing on the increment.
    pub(crate) fn gen_for_stmt(&mut self, stmt: &Node) -> Result<()> {
        let for_stmt = match stmt {
            Node::ForStmt(f) => f,
            _ => unreachable!(),
        };
        if for_stmt.end.is_none() {
            return self.gen_for_collection_stmt(stmt);
        }
        let end_expr: &Node = for_stmt.end.as_deref().expect("end present");
        let slot = match &self.for_slot_override {
            Some((key, slot)) if *key == node_key(stmt) => slot.clone(),
            _ => format!("{}_slot", for_stmt.var_name),
        };
        let counter_ty = self
            .expr_type(&for_stmt.start)
            .unwrap_or(Type::Primitive(sla_ast::Primitive::I64));
        let ty_str = type_string(&counter_ty);

        let start_reg = self.gen_expr(&for_stmt.start)?;
        self.ins(&format!("store {slot}+0, {start_reg} as {ty_str}"));
        if self.expr_result_reg_needs_release(&for_stmt.start) {
            self.emit_release(&start_reg)?;
        }
        let end_reg = self.gen_expr(end_expr)?;
        let end_is_temp = self.expr_result_reg_needs_release(end_expr);

        let head_label = self.new_label("L_FOR_HEAD");
        let body_label = self.new_label("L_FOR_BODY");
        let incr_label = self.new_label("L_FOR_INCR");
        let cond_false_label = self.new_label("L_FOR_COND_FALSE");
        let exit_label = self.new_label("L_FOR_EXIT");

        self.w(&format!("    jmp {head_label}\n\n"));
        self.wl(&format!("{head_label}:"));
        let cur_reg = self.new_tmp();
        self.ins(&format!("{cur_reg} = load {slot}+0 as {ty_str}"));
        let cond_reg = self.new_tmp();
        let cmp = if matches!(
            counter_ty,
            Type::Primitive(
                sla_ast::Primitive::U8
                    | sla_ast::Primitive::U16
                    | sla_ast::Primitive::U32
                    | sla_ast::Primitive::U64
                    | sla_ast::Primitive::Usize
            )
        ) {
            "ult"
        } else {
            "slt"
        };
        self.ins(&format!("{cond_reg} = {cmp} {cur_reg}, {end_reg}"));
        self.emit_release(&cur_reg)?;
        self.w(&format!(
            "    br {cond_reg} -> {body_label}, {cond_false_label}\n\n"
        ));
        self.wl(&format!("{body_label}:"));
        self.w(&format!("    !{cond_reg}\n"));

        self.loop_continue_labels.push(incr_label.clone());
        self.loop_break_labels.push(exit_label.clone());
        self.push_loop_body_local_scope();
        self.push_binding_alias_to(&for_stmt.var_name, &slot);
        let was_absent = self.addressable_bindings.insert(for_stmt.var_name.clone());
        let mut flow = self.gen_block(&for_stmt.body);
        self.pop_binding_alias(&for_stmt.var_name);
        if was_absent {
            self.addressable_bindings.remove(&for_stmt.var_name);
        }
        self.loop_continue_labels.pop();
        self.loop_break_labels.pop();

        if flow.is_ok() && !block_terminates(&for_stmt.body) {
            flow = self.emit_active_loop_body_local_cleanups(None, false);
            if flow.is_ok() {
                flow = self.emit_loop_body_top_level_local_cleanups(&for_stmt.body);
            }
        }
        if flow.is_ok() {
            // The increment block is the `continue` target: it
            // must exist (and jump back to the head) even when
            // the body always terminates, since a `continue`
            // inside still lands here.
            self.wl(&format!("{incr_label}:"));
            let cur2 = self.new_tmp();
            self.ins(&format!("{cur2} = load {slot}+0 as {ty_str}"));
            let one = self.new_tmp();
            self.ins(&format!("{one} = 1"));
            let next = self.new_tmp();
            self.ins(&format!("{next} = add {cur2}, {one}"));
            self.ins(&format!("store {slot}+0, {next} as {ty_str}"));
            self.emit_release(&cur2)?;
            self.emit_release(&one)?;
            self.emit_release(&next)?;
            self.w(&format!("    jmp {head_label}\n\n"));
        }
        self.pop_loop_body_local_scope();
        flow?;
        self.wl(&format!("{cond_false_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        self.w(&format!("    jmp {exit_label}\n\n"));
        self.wl(&format!("{exit_label}:"));
        if end_is_temp {
            self.emit_release(&end_reg)?;
        }
        Ok(())
    }

    /// Associated-method declaration by mangled key: the assoc
    /// index first (where inherent/trait impl methods live),
    /// then the plain function lookup.
    fn protocol_method_decl(&self, key: &str) -> Option<sla_ast::FuncDecl> {
        for (k, d) in &self.assoc_index {
            if k == key {
                return Some(d.clone());
            }
        }
        self.func_decl(key)
    }

    /// Collection `for var in coll` (codegen.zig `genForStmt`,
    /// the no-`end` arm): the hoisted `{var}_slot` counts an
    /// i64 index from 0; the head compares it against the
    /// collection length (the array's static length, or the
    /// value of one `iter_len` call made before the loop). The
    /// body binds the loop variable per iteration: arrays read
    /// the element out of the array image — aggregate elements
    /// bind the element address instead (CLOSING-F, the image
    /// address is the value register) — and protocol values
    /// take the element from an `iter_at` call on the collection
    /// register, its receiver borrowed exactly as the method
    /// declaration states. The body runs under the same
    /// loop-local scope discipline as `while`; the element
    /// register is released at the back edge (Zig prints a raw
    /// `!{var}` at L_LOOP_CONTINUE — `emit_release` reduces to
    /// that for plain values and runs the drop plan for owned
    /// elements), and break/continue reach the exit/increment
    /// labels through the shared label stacks, their own
    /// checker-recorded cleanups having settled the variable.
    pub(crate) fn gen_for_collection_stmt(&mut self, stmt: &Node) -> Result<()> {
        let for_stmt = match stmt {
            Node::ForStmt(f) => f,
            _ => unreachable!(),
        };
        let start_reg = self.gen_expr(&for_stmt.start)?;
        let start_is_temp = self.expr_result_reg_needs_release(&for_stmt.start);
        let source_ty = self.require_expr_type(&for_stmt.start, "for collection source")?;
        let source_arr = match sla_lowering::peel_borrow_pointer_type(&source_ty) {
            Type::Array(a) => Some(a.clone()),
            _ => None,
        };

        // Protocol setup (Zig genForStmt:16973): resolve both
        // methods before the loop; `iter_len` runs once here.
        let mut protocol_len_reg: Option<String> = None;
        let mut protocol_at: Option<(String, bool)> = None;
        if source_arr.is_none() {
            let type_name = sla_lowering::concrete_type_name(&source_ty)
                .ok_or(TextError::MissingType {
                    context: "for collection source type name".to_string(),
                })?
                .to_string();
            let len_key = sla_lowering::mangle_method_name(&type_name, "iter_len");
            let at_key = sla_lowering::mangle_method_name(&type_name, "iter_at");
            // Method declarations live in the emitter's assoc
            // index under the same mangled key (program.rs indexes
            // impl methods there, not in `func_index`); the plain
            // function lookup is the fallback — the candidate
            // order of `resolve_associated_decl`. (`check.func`
            // itself is None on the CLI adapter: its CheckInfo
            // impl leaves the trait default in place.)
            let len_func = self
                .protocol_method_decl(&len_key)
                .ok_or(TextError::MissingType {
                    context: "for iterable protocol iter_len".to_string(),
                })?;
            let at_func = self
                .protocol_method_decl(&at_key)
                .ok_or(TextError::MissingType {
                    context: "for iterable protocol iter_at".to_string(),
                })?;
            let len_reg = self.new_tmp();
            let lowered_len = self.lowered_func_symbol(&len_key);
            if len_func.params.first().map(|p| p.is_borrow).unwrap_or(false) {
                self.ins(&format!("{len_reg} = call @{lowered_len}(&{start_reg})"));
            } else {
                self.ins(&format!("{len_reg} = call @{lowered_len}({start_reg})"));
            }
            protocol_len_reg = Some(len_reg);
            protocol_at = Some((
                self.lowered_func_symbol(&at_key),
                at_func.params.first().map(|p| p.is_borrow).unwrap_or(false),
            ));
        }

        let slot = match &self.for_slot_override {
            Some((key, slot)) if *key == node_key(stmt) => slot.clone(),
            _ => format!("{}_slot", for_stmt.var_name),
        };
        self.ins(&format!("store {slot}+0, 0 as i64"));

        let head_label = self.new_label("L_FOR_HEAD");
        let body_label = self.new_label("L_FOR_BODY");
        let incr_label = self.new_label("L_FOR_INCR");
        let cond_false_label = self.new_label("L_FOR_COND_FALSE");
        let exit_label = self.new_label("L_FOR_EXIT");

        self.w(&format!("    jmp {head_label}\n\n"));
        self.wl(&format!("{head_label}:"));
        let index_reg = self.new_tmp();
        self.ins(&format!("{index_reg} = load {slot}+0 as i64"));
        let cond_reg = self.new_tmp();
        if let Some(arr) = &source_arr {
            self.ins(&format!("{cond_reg} = slt {index_reg}, {}", arr.len));
        } else {
            let len_reg = protocol_len_reg.as_ref().expect("protocol len register");
            self.ins(&format!("{cond_reg} = slt {index_reg}, {len_reg}"));
        }
        self.w(&format!(
            "    br {cond_reg} -> {body_label}, {cond_false_label}\n\n"
        ));
        self.wl(&format!("{body_label}:"));
        self.w(&format!("    !{cond_reg}\n"));

        if let Some(arr) = &source_arr {
            let elem_size = type_size(&arr.elem);
            let byte_offset = self.new_tmp();
            if elem_size == 1 {
                self.ins(&format!("{byte_offset} = {index_reg}"));
            } else {
                self.ins(&format!(
                    "{byte_offset} = mul {index_reg}, {elem_size}"
                ));
            }
            let elem_ptr = self.new_tmp();
            self.ins(&format!(
                "{elem_ptr} = ptr_add {start_reg}, {byte_offset}"
            ));
            if sla_lowering::for_in_array_elem_binds_address(&arr.elem) {
                self.ins(&format!("{} = add {elem_ptr}, 0", for_stmt.var_name));
                self.emit_release(&byte_offset)?;
            } else {
                self.ins(&format!(
                    "{} = load {elem_ptr}+0 as {}",
                    for_stmt.var_name,
                    type_string(&arr.elem)
                ));
                self.emit_release(&byte_offset)?;
                self.emit_release(&elem_ptr)?;
            }
        } else {
            let (lowered_at, at_borrow) =
                protocol_at.as_ref().expect("protocol iter_at symbol");
            if *at_borrow {
                self.ins(&format!(
                    "{} = call @{lowered_at}(&{start_reg}, {index_reg})",
                    for_stmt.var_name
                ));
            } else {
                self.ins(&format!(
                    "{} = call @{lowered_at}({start_reg}, {index_reg})",
                    for_stmt.var_name
                ));
            }
        }
        // The index is dead once the variable is bound; retiring
        // it here (instead of at the back edge, as Zig does) keeps
        // no register live across the body, so the break edge —
        // which lands directly on the exit label — retires it
        // exactly once on every path. The increment reloads the
        // counter from its slot, like the range form.
        self.emit_release(&index_reg)?;

        self.loop_continue_labels.push(incr_label.clone());
        self.loop_break_labels.push(exit_label.clone());
        self.push_loop_body_local_scope();
        let mut flow = self.gen_block(&for_stmt.body);
        self.loop_continue_labels.pop();
        self.loop_break_labels.pop();

        if flow.is_ok() && !block_terminates(&for_stmt.body) {
            flow = self.emit_active_loop_body_local_cleanups(None, false);
            if flow.is_ok() {
                flow = self.emit_loop_body_top_level_local_cleanups(&for_stmt.body);
            }
            if flow.is_ok() {
                flow = self.emit_release(&for_stmt.var_name);
            }
        }
        if flow.is_ok() {
            self.wl(&format!("{incr_label}:"));
            let cur = self.new_tmp();
            self.ins(&format!("{cur} = load {slot}+0 as i64"));
            let next = self.new_tmp();
            self.ins(&format!("{next} = add {cur}, 1"));
            self.ins(&format!("store {slot}+0, {next} as i64"));
            self.emit_release(&cur)?;
            self.emit_release(&next)?;
            self.w(&format!("    jmp {head_label}\n\n"));
        }
        self.pop_loop_body_local_scope();
        flow?;
        self.wl(&format!("{cond_false_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        self.w(&format!("    !{index_reg}\n"));
        self.w(&format!("    jmp {exit_label}\n\n"));
        self.wl(&format!("{exit_label}:"));
        if let Some(len_reg) = &protocol_len_reg {
            self.emit_release(len_reg)?;
        }
        if start_is_temp {
            self.emit_release(&start_reg)?;
        }
        Ok(())
    }
}

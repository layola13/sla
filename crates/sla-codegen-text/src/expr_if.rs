//! If-expression emission, plain path, mirrored from codegen.zig
//! (15697-15880). Two counter behaviors matter for byte parity and
//! are preserved exactly: the `L_MERGE` label is allocated even when
//! no merge block is emitted, and a value-less if returns a freshly
//! allocated but discarded temporary register (the invisible tmp_7
//! in fixture 07 — coordinator item D-C5, resolved in
//! docs/divergences/lowering-text.md). The if-let chain path
//! (codegen.zig's inline if-let arm of the expression cascade)
//! chains one pattern test per condition: each test branches to
//! the next test (or the then block) on success and to a fail
//! block — which releases the bindings acquired so far and jumps
//! to the else block — on failure. The `?.` optional-chain
//! operator desugars to exactly this shape in the parser.

use sla_ast::{EnumDecl, EnumPattern, IfExpr, IfLetCond, Node, Type};
use sla_lowering::{block_terminates, call_arg_needs_release, is_discard_name, is_void_type};

use crate::codegen::{type_size, type_string, Codegen};
use crate::layout;
use crate::Result;

impl<'c> Codegen<'c> {
    /// codegen.zig:15697 plain `genIfExpr` path.
    pub(crate) fn gen_if_expr(&mut self, expr: &Node) -> Result<String> {
        let ife = match expr {
            Node::IfExpr(i) => i,
            _ => unreachable!(),
        };
        if let Some(chain) = &ife.let_chain {
            return self.gen_if_let_chain(expr, ife, chain);
        }
        let cond_reg = self.gen_branch_condition_reg(&ife.cond)?;
        let expr_ty: Type = self.require_expr_type(expr, "if expression result")?;
        let has_value = !is_void_type(&expr_ty);

        let result_slot: Option<String> = if has_value {
            let slot = self.new_tmp();
            self.ins(&format!("{slot} = alloc {}", type_size(&expr_ty)));
            // codegen.zig `prepareResultSlotRefCellCompanion`:
            // a borrow-typed result slot's companion cell slot is
            // allocated here, before the branch, so both branch
            // paths can store the borrowed cell into it.
            self.prepare_result_slot_refcell_companion(&slot, &expr_ty);
            Some(slot)
        } else {
            None
        };
        let then_label = self.new_label("L_THEN");
        let else_label = self.new_label("L_ELSE");
        // Allocated unconditionally in the Zig; emitted only when a
        // merge is needed.
        let merge_label = self.new_label("L_MERGE");

        let then_terminates = block_terminates(&ife.then_block);
        let else_terminates = match &ife.else_block {
            Some(block) => block_terminates(block),
            None => false,
        };
        let needs_merge = !then_terminates || !else_terminates;

        let consumed_snapshot = self.snapshot_consumed();
        // The RefCell handle table is branch state too (the
        // loop emitters snapshot/restore it the same way): a
        // handle released by a terminating side's return cleanup
        // must not vanish from the surviving side's table, or
        // that side's later cleanup degrades to a plain release
        // and the shared borrow count leaks.
        let handles_snapshot = self.refcell_borrow_handles.clone();
        // Branch-compensation candidates (codegen.zig
        // `emitBranchConsumptionCompensation`, the β fix): the
        // pre-branch live owned bindings — named locals/params
        // known at branch entry, minus discard names, minus
        // global consts, minus live RefCell borrow handles.
        let live_owned: Vec<String> = self
            .local_binding_types
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| {
                !is_discard_name(n)
                    && !self.global_const_bindings.contains(n)
                    && !self.refcell_borrow_handles.contains_key(n)
            })
            .collect();
        self.w(&format!(
            "    br {cond_reg} -> {then_label}, {else_label}\n\n"
        ));
        self.wl(&format!("{then_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        match (&result_slot, then_terminates) {
            (Some(slot), false) => {
                self.gen_block_tail_value_store(&ife.then_block, slot, &expr_ty)?;
            }
            _ => {
                self.gen_block(&ife.then_block)?;
            }
        }
        let then_phi = self.emit_phi_cleanups(ife.then_block.last())?;
        // The then side's jump (and any compensation releases)
        // are deferred to the compensation step below, which
        // inserts them at this offset.
        let then_text_end = self.out.len();
        let then_end_snapshot = self.snapshot_consumed();
        let then_end_handles = self.refcell_borrow_handles.clone();
        // The Zig restores the pre-branch state before generating
        // the else side unconditionally (consumed / store-moved /
        // borrow / RefCell books).
        self.restore_consumed(&consumed_snapshot);
        self.refcell_borrow_handles = handles_snapshot;

        self.wl(&format!("{else_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        let else_body: &[Node] = ife.else_block.as_deref().unwrap_or(&[]);
        match (&result_slot, else_terminates) {
            (Some(slot), false) => {
                self.gen_block_tail_value_store(else_body, slot, &expr_ty)?;
            }
            _ => {
                self.gen_block(else_body)?;
            }
        }
        let else_phi = self.emit_phi_cleanups(else_body.last())?;

        if needs_merge {
            self.emit_branch_compensation(
                &live_owned,
                &consumed_snapshot.0,
                &then_end_snapshot.0,
                then_text_end,
                &merge_label,
                then_terminates,
                else_terminates,
                &then_phi,
                &else_phi,
            )?;
            // Post-merge settle (codegen.zig, the call-site tail):
            // both sides reach the merge -> consumed is the union
            // of the two end states; the else side terminates ->
            // the then end state survives; the then side
            // terminates -> the else end state (already live).
            if !then_terminates && !else_terminates {
                let union: std::collections::HashSet<String> = then_end_snapshot
                    .0
                    .union(&self.consumed_bindings)
                    .cloned()
                    .collect();
                self.consumed_bindings = union;
            } else if else_terminates {
                self.restore_consumed(&then_end_snapshot);
                self.refcell_borrow_handles = then_end_handles.clone();
            }
            self.wl(&format!("{merge_label}:"));
        } else {
            // Both sides terminate: the Zig inserts the deferred
            // blank line at the end of the then segment and prints
            // one after the else segment.
            self.out.insert_str(then_text_end, "\n");
            self.w("\n");
            if else_terminates {
                self.restore_consumed(&then_end_snapshot);
                self.refcell_borrow_handles = then_end_handles.clone();
            }
        }
        match result_slot {
            Some(slot) => {
                let result = self.new_tmp();
                self.ins(&format!(
                    "{result} = load {slot}+0 as {}",
                    type_string(&expr_ty)
                ));
                // codegen.zig `loadResultSlotTransferredValueState`
                // (the companion restore): a RefCell handle a
                // branch stored into the slot's companion becomes
                // a fresh handle on the load result, keyed on
                // registers declared here at the merge — never on
                // the branch-local guard registers, which do not
                // exist on the sibling path (UnknownRegister,
                // corpus test_unit_refcell_struct_payload). An
                // untouched companion slot is simply retired.
                if let Some(entry) = self.result_slot_refcell_handles.remove(&slot) {
                    self.result_slot_refcell_slots.remove(&slot);
                    let cell_reg = self.new_tmp();
                    self.ins(&format!(
                        "{cell_reg} = load {}+0 as ptr",
                        entry.cell_slot
                    ));
                    self.refcell_borrow_handles.insert(
                        result.clone(),
                        crate::codegen::RefCellBorrowHandle {
                            cell_reg: cell_reg.clone(),
                            kind: entry.kind,
                            cell_release_temp: Some(cell_reg),
                            // DEF-RUST-REFCELL-BRANCH-HANDLE-MERGE
                            // (task 21): the handle is a fresh
                            // result temp; the cell is a separate
                            // register loaded from the companion
                            // slot, so the handle is not the cell.
                            handle_is_cell: false,
                        },
                    );
                    self.emit_release(&entry.cell_slot)?;
                } else if let Some(cell_slot) =
                    self.result_slot_refcell_slots.remove(&slot)
                {
                    self.emit_release(&cell_slot)?;
                }
                self.emit_release(&slot)?;
                Ok(result)
            }
            // Void if: the Zig returns `try self.newTmp()` — a counter
            // allocation with no emission (D-C5).
            None => Ok(self.new_tmp()),
        }
    }

    /// codegen.zig:2440 `emitBranchConsumptionCompensation` (the
    /// β fix): a binding consumed on exactly one branch path is
    /// released at the end of the OTHER path, before that path's
    /// jump to the merge — otherwise the merge point sees the
    /// register Active on one predecessor and Consumed on the
    /// other (PhiStateConflict, trap 1015; the reg3 redis_client
    /// / redis_session shape). The Zig store-moved differential
    /// (`emitBranchSettle`) and load-class settle have no state
    /// in this lane (no store-moved set, no load-class tracking);
    /// the family shapes consume through plain calls, which this
    /// core differential path covers.
    fn emit_branch_compensation(
        &mut self,
        live_owned: &[String],
        pre_consumed: &std::collections::HashSet<String>,
        then_consumed: &std::collections::HashSet<String>,
        then_text_end: usize,
        merge_label: &str,
        then_terminates: bool,
        else_terminates: bool,
        then_phi: &[String],
        else_phi: &[String],
    ) -> Result<()> {
        // The checker's phi cleanups (merge_branch_states) are
        // the same β-fix release recorded from the checker's side:
        // a binding the phi list already released on a path must
        // not be released again here — the double `!name` is a
        // UseAfterMove at the SA verifier (corpus reg2_proxy /
        // reg3_regression_branch_compensation).
        let else_consumed = self.consumed_bindings.clone();
        if !then_terminates && !else_terminates {
            // Consumed only on the then path: release at the end
            // of the else path (appended live, before its jump).
            for name in live_owned {
                if then_consumed.contains(name)
                    && !pre_consumed.contains(name)
                    && !else_consumed.contains(name)
                    && !else_phi.contains(name)
                {
                    self.emit_release(name)?;
                }
            }
        }
        if !else_terminates {
            self.w(&format!("    jmp {merge_label}\n"));
        }
        self.w("\n");
        if !then_terminates {
            // Consumed only on the else path: release at the end
            // of the then path — rendered into a scratch buffer
            // and inserted at then_text_end, followed by the
            // then jump and the segment's blank line.
            let live_out = std::mem::take(&mut self.out);
            if !else_terminates {
                for name in live_owned {
                    if else_consumed.contains(name)
                        && !pre_consumed.contains(name)
                        && !then_consumed.contains(name)
                        && !then_phi.contains(name)
                    {
                        self.consumed_bindings.remove(name);
                        self.emit_release(name)?;
                    }
                }
            }
            self.w(&format!("    jmp {merge_label}\n"));
            self.w("\n");
            let scratch = std::mem::replace(&mut self.out, live_out);
            self.out.insert_str(then_text_end, &scratch);
        }
        Ok(())
    }

    /// Phi cleanup registers attached to a block's last statement are
    /// printed raw (not via emitRelease), mirroring the Zig.
    /// Returns the names printed, so branch compensation can skip
    /// the ones already released here.
    fn emit_phi_cleanups(&mut self, last_stmt: Option<&Node>) -> Result<Vec<String>> {
        let mut printed = Vec::new();
        if let Some(stmt) = last_stmt {
            if let Some(list) = self.check.phi_cleanups(stmt) {
                let names: Vec<String> = list.to_vec();
                for name in names {
                    self.w(&format!("    !{name}\n"));
                    printed.push(name);
                }
            }
        }
        Ok(printed)
    }

    /// codegen.zig's if-let arm of the expression cascade: one
    /// pattern test per chain condition. Each test's fail block
    /// releases the bindings acquired by earlier conditions and
    /// jumps to the else block; the success path binds the
    /// pattern payload and falls through to the next test (the
    /// last one falls into the then block). Pattern bindings the
    /// then block does not consume are released at its end.
    fn gen_if_let_chain(
        &mut self,
        expr: &Node,
        ife: &IfExpr,
        chain: &[IfLetCond],
    ) -> Result<String> {
        let expr_ty: Type = self.require_expr_type(expr, "if-let expression result")?;
        let value_if = !is_void_type(&expr_ty);
        let result_slot: Option<String> = if value_if {
            let slot = self.new_tmp();
            self.ins(&format!("{slot} = alloc {}", type_size(&expr_ty)));
            Some(slot)
        } else {
            None
        };
        let then_label = self.new_label("L_IF_LET_THEN");
        let else_label = self.new_label("L_IF_LET_ELSE");
        let merge_label = self.new_label("L_IF_LET_MERGE");
        let then_terminates = block_terminates(&ife.then_block);
        let else_body: &[Node] = ife.else_block.as_deref().unwrap_or(&[]);
        let else_terminates = block_terminates(else_body);
        let needs_merge = !then_terminates || !else_terminates;
        // Pattern binding names acquired along the success path,
        // in acquisition order (Zig `acquired`).
        let mut acquired: Vec<String> = Vec::new();

        for (i, cond) in chain.iter().enumerate() {
            let is_last = i + 1 == chain.len();
            let success_label = if is_last {
                then_label.clone()
            } else {
                self.new_label("L_IF_LET_NEXT")
            };
            let fail_label = self.new_label("L_IF_LET_FAIL");
            let value_reg = self.gen_expr(&cond.value)?;
            let branch_flag = self.new_tmp();
            let pattern = &cond.pattern;
            let enum_decl = self.enum_decl_for_pattern_value(&cond.value, pattern)?;
            let success_on_true = enum_decl.is_some()
                || pattern.variant_name == "Some"
                || pattern.variant_name == "Ok";
            if let Some(decl) = &enum_decl {
                self.gen_enum_pattern_check(
                    &value_reg,
                    decl,
                    &pattern.variant_name,
                    &branch_flag,
                )?;
            } else if sla_lowering::pattern_uses_result_macros(
                &pattern.enum_name,
                &pattern.variant_name,
            ) {
                self.ins(&format!(
                    "EXPAND RESULT_IS_OK {branch_flag}, {value_reg}"
                ));
            } else {
                self.ins(&format!(
                    "EXPAND OPTION_IS_SOME {branch_flag}, {value_reg}"
                ));
            }
            if success_on_true {
                self.w(&format!(
                    "    br {branch_flag} -> {success_label}, {fail_label}\n\n"
                ));
            } else {
                self.w(&format!(
                    "    br {branch_flag} -> {fail_label}, {success_label}\n\n"
                ));
            }
            // Fail block: the consumed-state snapshot is taken
            // before the label prints (the fail path must see the
            // pre-test state) and restored after the jump, so the
            // success path continues from the same state.
            let snapshot = self.snapshot_consumed();
            self.wl(&format!("{fail_label}:"));
            self.w(&format!("    !{branch_flag}\n"));
            if call_arg_needs_release(&cond.value) {
                self.emit_release(&value_reg)?;
            }
            for name in &acquired {
                self.emit_release(name)?;
            }
            self.w(&format!("    jmp {else_label}\n\n"));
            self.restore_consumed(&snapshot);
            // Success block: bind the pattern payload.
            self.wl(&format!("{success_label}:"));
            self.w(&format!("    !{branch_flag}\n"));
            if let Some(decl) = &enum_decl {
                self.gen_enum_pattern_bindings(&value_reg, decl, pattern)?;
                acquired.extend(pattern.bindings.iter().cloned());
            } else if pattern.bindings.len() == 1 {
                let macro_name = match pattern.variant_name.as_str() {
                    "Some" => Some("OPTION_GET"),
                    "Ok" => Some("RESULT_GET_OK"),
                    "Err" => Some("RESULT_GET_ERR"),
                    _ => None,
                };
                if let Some(macro_name) = macro_name {
                    let target = self.push_binding_alias(&pattern.bindings[0]);
                    self.ins(&format!(
                        "EXPAND {macro_name} {target}, {value_reg}"
                    ));
                    self.transfer_guard_metadata(&target, &value_reg);
                    acquired.push(pattern.bindings[0].clone());
                }
            }
            if call_arg_needs_release(&cond.value) {
                self.emit_release(&value_reg)?;
            }
        }

        // Then block.
        match (&result_slot, then_terminates) {
            (Some(slot), false) => {
                self.gen_block_tail_value_store(&ife.then_block, slot, &expr_ty)?;
            }
            _ => {
                self.gen_block(&ife.then_block)?;
            }
        }
        for name in &acquired {
            if !block_consumes_identifier(&ife.then_block, name) {
                self.emit_release(name)?;
            }
        }
        for name in &acquired {
            self.pop_binding_alias(name);
        }
        self.emit_phi_cleanups(ife.then_block.last())?;
        if needs_merge && !then_terminates {
            self.w(&format!("    jmp {merge_label}\n"));
        }
        self.w("\n");

        // Else block.
        self.wl(&format!("{else_label}:"));
        match (&result_slot, else_terminates) {
            (Some(slot), false) => {
                self.gen_block_tail_value_store(else_body, slot, &expr_ty)?;
            }
            _ => {
                self.gen_block(else_body)?;
            }
        }
        self.emit_phi_cleanups(else_body.last())?;
        if needs_merge && !else_terminates {
            self.w(&format!("    jmp {merge_label}\n"));
        }
        self.w("\n");

        if needs_merge {
            self.wl(&format!("{merge_label}:"));
        }
        match result_slot {
            Some(slot) => {
                let result = self.new_tmp();
                self.ins(&format!(
                    "{result} = load {slot}+0 as {}",
                    type_string(&expr_ty)
                ));
                self.emit_release(&slot)?;
                Ok(result)
            }
            None => Ok(self.new_tmp()),
        }
    }

    /// codegen.zig `genLetElseStmt` (the let_else_stmt arm of
    /// genStmt): evaluate the value, test the pattern; on match
    /// bind the payload and fall through to the continuation, on
    /// mismatch run the else block (which must diverge — the
    /// checker enforces that; a non-terminating else still gets
    /// its jump to the continuation, as in the Zig). The else
    /// side is generated under a snapshot of the consumed /
    /// borrow-source / RefCell-handle state, restored before the
    /// success side, exactly as the Zig clones and restores the
    /// three tables.
    pub(crate) fn gen_let_else_stmt(&mut self, stmt: &Node) -> Result<()> {
        let l = match stmt {
            Node::LetElseStmt(l) => l,
            _ => unreachable!(),
        };
        let value_reg = self.gen_expr(&l.value)?;
        let branch_flag = self.new_tmp();
        let success_label = self.new_label("L_LET_ELSE_OK");
        let else_label = self.new_label("L_LET_ELSE_ELSE");
        let cont_label = self.new_label("L_LET_ELSE_CONT");
        let enum_decl = self.enum_decl_for_pattern_value(&l.value, &l.pattern)?;
        let success_on_true = enum_decl.is_some()
            || l.pattern.variant_name == "Some"
            || l.pattern.variant_name == "Ok";
        if let Some(decl) = &enum_decl {
            self.gen_enum_pattern_check(
                &value_reg,
                decl,
                &l.pattern.variant_name,
                &branch_flag,
            )?;
        } else if sla_lowering::pattern_uses_result_macros(
            &l.pattern.enum_name,
            &l.pattern.variant_name,
        ) {
            self.ins(&format!(
                "EXPAND RESULT_IS_OK {branch_flag}, {value_reg}"
            ));
        } else {
            self.ins(&format!(
                "EXPAND OPTION_IS_SOME {branch_flag}, {value_reg}"
            ));
        }
        if success_on_true {
            self.w(&format!(
                "    br {branch_flag} -> {success_label}, {else_label}\n\n"
            ));
        } else {
            self.w(&format!(
                "    br {branch_flag} -> {else_label}, {success_label}\n\n"
            ));
        }

        let snapshot = self.snapshot_consumed();
        let saved_handles = self.refcell_borrow_handles.clone();

        self.wl(&format!("{else_label}:"));
        self.w(&format!("    !{branch_flag}\n"));
        if sla_lowering::call_arg_needs_release(&l.value) {
            self.emit_release(&value_reg)?;
        }
        self.gen_block(&l.else_block)?;
        if !sla_lowering::block_terminates(&l.else_block) {
            self.w(&format!("    jmp {cont_label}\n"));
        }
        self.w("\n");
        self.restore_consumed(&snapshot);
        self.refcell_borrow_handles = saved_handles;

        self.wl(&format!("{success_label}:"));
        self.w(&format!("    !{branch_flag}\n"));
        if let Some(decl) = &enum_decl {
            self.gen_enum_pattern_bindings(&value_reg, decl, &l.pattern)?;
            if let Some(variant) = decl
                .variants
                .iter()
                .find(|v| v.name == l.pattern.variant_name)
            {
                for (binding, field) in
                    l.pattern.bindings.iter().zip(variant.fields.iter())
                {
                    if let Some((_, field_ty)) =
                        layout::enum_field_layout(variant, &field.name)
                    {
                        self.remember_local_binding_type(binding, &field_ty);
                    }
                }
            }
        } else if l.pattern.bindings.len() == 1 {
            let macro_name = match l.pattern.variant_name.as_str() {
                "Some" => Some("OPTION_GET"),
                "Ok" => Some("RESULT_GET_OK"),
                "Err" => Some("RESULT_GET_ERR"),
                _ => None,
            };
            if let Some(macro_name) = macro_name {
                let target = self.push_binding_alias(&l.pattern.bindings[0]);
                self.ins(&format!(
                    "EXPAND {macro_name} {target}, {value_reg}"
                ));
                self.transfer_guard_metadata(&target, &value_reg);
                if let Some(value_ty) = self.expr_type(&l.value) {
                    let payload_ty = match &value_ty {
                        Type::UserDefined(u) if u.generics.len() == 2 => {
                            Some(u.generics[if l.pattern.variant_name == "Err" {
                                1
                            } else {
                                0
                            }]
                            .clone())
                        }
                        Type::UserDefined(u) if u.generics.len() == 1 => {
                            Some(u.generics[0].clone())
                        }
                        _ => None,
                    };
                    if let Some(payload_ty) = payload_ty {
                        self.remember_local_binding_type(
                            &l.pattern.bindings[0],
                            &payload_ty,
                        );
                    }
                }
            }
        }
        if sla_lowering::call_arg_needs_release(&l.value) {
            self.emit_release(&value_reg)?;
        }
        self.w(&format!("    jmp {cont_label}\n\n"));
        self.wl(&format!("{cont_label}:"));
        Ok(())
    }

    /// codegen.zig:7169 `enumDeclForPatternValue`: the enum
    /// declaration a pattern tests, resolved from the pattern
    /// value's checked type; Option/Result are macro-tested and
    /// yield None, as does any non-enum type. A pattern naming a
    /// different enum than the value's type is a loud error.
    fn enum_decl_for_pattern_value(
        &self,
        value: &Node,
        pattern: &EnumPattern,
    ) -> Result<Option<EnumDecl>> {
        let value_ty = self
            .expr_type(value)
            .ok_or_else(|| crate::TextError::MissingType {
                context: "if-let pattern value type".to_string(),
            })?;
        let type_name = match sla_lowering::concrete_type_name(&value_ty) {
            Some(n) => n.to_string(),
            None => return Ok(None),
        };
        if type_name == "Option" || type_name == "Result" {
            return Ok(None);
        }
        let decl = match self.enum_decl(&type_name) {
            Some(d) => d.clone(),
            None => return Ok(None),
        };
        if !sla_lowering::enum_name_matches_decl(&pattern.enum_name, &decl.name) {
            return Err(crate::TextError::Unsupported {
                feature: format!(
                    "if-let pattern enum `{}` does not match value enum `{}`",
                    pattern.enum_name, decl.name
                ),
                zig_ref: "codegen.zig:7169 (enumDeclForPatternValue)",
            });
        }
        Ok(Some(decl))
    }

    /// codegen.zig:6384 `genEnumPatternCheck`: compare the
    /// value's tag word against the variant's discriminant into
    /// `flag_reg`.
    fn gen_enum_pattern_check(
        &mut self,
        value_reg: &str,
        decl: &EnumDecl,
        variant_name: &str,
        flag_reg: &str,
    ) -> Result<()> {
        let tag = layout::enum_variant_value(decl, variant_name).ok_or_else(|| {
            crate::TextError::Unsupported {
                feature: format!("enum pattern variant {variant_name}"),
                zig_ref: "codegen.zig:6384 (genEnumPatternCheck)",
            }
        })?;
        let tag_reg = self.new_tmp();
        self.ins(&format!("{tag_reg} = load {value_reg}+0 as i64"));
        let tag_const = self.new_tmp();
        self.emit_int_const(&tag_const, tag);
        self.ins(&format!("{flag_reg} = eq {tag_reg}, {tag_const}"));
        self.w(&format!("    !{tag_reg}\n"));
        self.w(&format!("    !{tag_const}\n"));
        Ok(())
    }

    /// codegen.zig:6395 `genEnumPatternBindings`: load each
    /// variant field into its pattern binding's fresh alias, in
    /// declaration order; the binding count must equal the
    /// variant's field count. Guard metadata transfers only for
    /// single-field variants (the Zig's own condition).
    fn gen_enum_pattern_bindings(
        &mut self,
        value_reg: &str,
        decl: &EnumDecl,
        pattern: &EnumPattern,
    ) -> Result<()> {
        let variant = decl
            .variants
            .iter()
            .find(|v| v.name == pattern.variant_name)
            .ok_or_else(|| crate::TextError::Unsupported {
                feature: format!("enum pattern variant {}", pattern.variant_name),
                zig_ref: "codegen.zig:6395 (genEnumPatternBindings)",
            })?;
        if pattern.bindings.len() != variant.fields.len() {
            return Err(crate::TextError::Unsupported {
                feature: format!(
                    "enum pattern {}::{} binds {} names for {} fields",
                    decl.name,
                    pattern.variant_name,
                    pattern.bindings.len(),
                    variant.fields.len()
                ),
                zig_ref: "codegen.zig:6395 (genEnumPatternBindings)",
            });
        }
        for (binding, field) in pattern.bindings.iter().zip(variant.fields.iter()) {
            let target = self.push_binding_alias(binding);
            let (offset, field_ty) =
                layout::enum_field_layout(variant, &field.name).ok_or_else(|| {
                    crate::TextError::Unsupported {
                        feature: format!("enum pattern field layout {}", field.name),
                        zig_ref: "codegen.zig:6395 (genEnumPatternBindings)",
                    }
                })?;
            self.ins(&format!(
                "{target} = load {value_reg}+{offset} as {}",
                type_string(&field_ty)
            ));
            if variant.fields.len() == 1 {
                self.transfer_guard_metadata(&target, value_reg);
            }
        }
        Ok(())
    }

    /// codegen.zig:1978 `transferGuardMetadata`: a payload load
    /// inherits the source register's guard bookkeeping (RefCell
    /// borrow handles, mutex guard handles, mutex lock results)
    /// so later releases settle the right handles.
    fn transfer_guard_metadata(&mut self, dst: &str, src: &str) {
        if dst == src {
            return;
        }
        if let Some(handle) = self.refcell_borrow_handles.remove(src) {
            self.refcell_borrow_handles.insert(dst.to_string(), handle);
        }
        if let Some(handle) = self.mutex_guard_handles.remove(src) {
            self.mutex_guard_handles.insert(dst.to_string(), handle);
        }
        if let Some(result) = self.mutex_lock_results.remove(src) {
            self.mutex_lock_results.insert(dst.to_string(), result);
        }
    }
}

/// codegen.zig:15557 `blockConsumesIdentifier`: whether any
/// statement in the block consumes the named binding (a move of
/// it, per `exprConsumesIdentifier`).
fn block_consumes_identifier(block: &[Node], name: &str) -> bool {
    block.iter().any(|stmt| stmt_consumes_identifier(stmt, name))
}

/// codegen.zig `stmtConsumesIdentifier`.
fn stmt_consumes_identifier(stmt: &Node, name: &str) -> bool {
    match stmt {
        Node::LetStmt(s) => expr_consumes_identifier(&s.value, name),
        Node::LetDestructureStmt(s) => expr_consumes_identifier(&s.value, name),
        Node::ConstStmt(s) => expr_consumes_identifier(&s.value, name),
        Node::LetElseStmt(s) => {
            expr_consumes_identifier(&s.value, name)
                || block_consumes_identifier(&s.else_block, name)
        }
        Node::AssignStmt(s) => {
            expr_consumes_identifier(&s.target, name)
                || expr_consumes_identifier(&s.value, name)
        }
        Node::ExprStmt(e) => expr_consumes_identifier(e, name),
        Node::ReturnStmt(s) => s
            .value
            .as_ref()
            .map(|v| expr_consumes_identifier(v, name))
            .unwrap_or(false),
        Node::ForStmt(s) => {
            expr_consumes_identifier(&s.start, name)
                || s.end
                    .as_ref()
                    .map(|e| expr_consumes_identifier(e, name))
                    .unwrap_or(false)
                || block_consumes_identifier(&s.body, name)
        }
        Node::WhileStmt(s) => {
            expr_consumes_identifier(&s.cond, name)
                || block_consumes_identifier(&s.body, name)
        }
        Node::BlockStmt(s) => block_consumes_identifier(&s.body, name),
        _ => false,
    }
}

/// codegen.zig `exprConsumesIdentifier`: only a move (`^name`,
/// possibly nested inside composite expressions) consumes; a
/// plain identifier read does not.
fn expr_consumes_identifier(expr: &Node, name: &str) -> bool {
    match expr {
        Node::MoveExpr(m) => match &*m.expr {
            Node::Identifier(id) => id == name,
            _ => expr_consumes_identifier(&m.expr, name),
        },
        Node::AwaitExpr(a) => match &*a.expr {
            Node::Identifier(id) => id == name,
            _ => expr_consumes_identifier(&a.expr, name),
        },
        Node::BorrowExpr(b) => expr_consumes_identifier(&b.expr, name),
        Node::DerefExpr(d) => expr_consumes_identifier(&d.expr, name),
        Node::CastExpr(c) => expr_consumes_identifier(&c.expr, name),
        Node::FieldExpr(f) => expr_consumes_identifier(&f.expr, name),
        Node::IndexExpr(x) => {
            expr_consumes_identifier(&x.target, name)
                || expr_consumes_identifier(&x.index, name)
        }
        Node::SliceExpr(s) => {
            expr_consumes_identifier(&s.target, name)
                || expr_consumes_identifier(&s.start, name)
                || expr_consumes_identifier(&s.end, name)
        }
        Node::BinaryExpr(b) => {
            expr_consumes_identifier(&b.left, name)
                || expr_consumes_identifier(&b.right, name)
        }
        Node::CallExpr(c) => c
            .args
            .iter()
            .any(|a| expr_consumes_identifier(a, name)),
        Node::ClosureLiteral(c) => expr_consumes_identifier(&c.body, name),
        Node::TryExpr(t) => expr_consumes_identifier(&t.expr, name),
        Node::IfExpr(i) => {
            expr_consumes_identifier(&i.cond, name)
                || i.let_chain
                    .as_ref()
                    .map(|chain| {
                        chain
                            .iter()
                            .any(|c| expr_consumes_identifier(&c.value, name))
                    })
                    .unwrap_or(false)
                || block_consumes_identifier(&i.then_block, name)
                || i.else_block
                    .as_ref()
                    .map(|b| block_consumes_identifier(b, name))
                    .unwrap_or(false)
        }
        Node::SwitchExpr(s) => {
            expr_consumes_identifier(&s.val, name)
                || s.cases.iter().any(|case| {
                    expr_consumes_identifier(&case.pattern, name)
                        || block_consumes_identifier(&case.body, name)
                })
        }
        Node::MatchExpr(m) => {
            expr_consumes_identifier(&m.val, name)
                || m.cases.iter().any(|case| {
                    case.pattern.bindings.iter().any(|b| b == name)
                        || case
                            .guard
                            .as_ref()
                            .map(|g| expr_consumes_identifier(g, name))
                            .unwrap_or(false)
                        || block_consumes_identifier(&case.body, name)
                })
        }
        Node::StructLiteral(s) => {
            s.fields
                .iter()
                .any(|f| expr_consumes_identifier(&f.value, name))
                || s.update_expr
                    .as_ref()
                    .map(|u| expr_consumes_identifier(u, name))
                    .unwrap_or(false)
        }
        Node::EnumLiteral(e) => e
            .fields
            .iter()
            .any(|f| expr_consumes_identifier(&f.value, name)),
        Node::TupleLiteral(t) => t
            .elements
            .iter()
            .any(|e| expr_consumes_identifier(e, name)),
        Node::ArrayLiteral(a) => a
            .elements
            .iter()
            .any(|e| expr_consumes_identifier(e, name)),
        Node::RepeatArrayLiteral(r) => expr_consumes_identifier(&r.value, name),
        _ => false,
    }
}

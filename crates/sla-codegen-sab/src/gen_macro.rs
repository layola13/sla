//! G wiring — imported SA-macro calls, string materialization and
//! the bare-name builtin arms (`STR_PTR` / `STR_LEN` literal forms,
//! `stack_alloc`) for the direct SAB driver.
//!
//! Zig anchors: `planImportedMacroCall`
//! (lowering_rules.zig:3268), `genImportedMacroCall` /
//! `emitDirectImportedMacroCall` (sab_codegen.zig:15072 / 18866),
//! `genStringLiteral` (13215), `genStrPtrCall` / `genStrLenCall`
//! (13238 / 13252) and the `stack_alloc` arm of `genCall` with
//! `stackAllocSize` (2693). The plan classification and the
//! expansion itself were already ported (`macro_plan.rs`,
//! `macro_expand.rs`, `callsite_args.rs`); what was missing is
//! exactly this file: the plan *construction* from the checker's
//! imported-macro registry (exposed through
//! `CheckInfo::imported_macro`), the `Driver` host implementation,
//! and the `gen_call` dispatch. The registry is generated from
//! each imported `.sa` file's own `[MACRO]` declarations and
//! merged per package — fixes land here at the declaration /
//! registry side, never as per-callsite patches.

use sla_ast::{CallExpr, Literal, Node, Primitive, Type};
use sla_lowering::ImportedMacroView;

use crate::callsite_args::MacroCallHost;
use crate::driver::{unsupported, Driver};
use crate::emit::ModuleBuilder;
use crate::error::{Error, Result};
use crate::fragment::FragmentMachine;
use crate::ir::{InstKind, Instruction, Operand, PrimType};
use crate::macro_expand::{DirectImportedMacroResult, MacroExpandHost};
use crate::macro_plan::ImportedMacroCallPlan;
use crate::state::RegState;
use crate::types::{AddressSource, Local, MacroExpansionContext};

/// Zig `importedMacroUsesExpressionOutput`
/// (lowering_rules.zig:3121): an imported macro used as an
/// expression omits exactly its single leading output parameter.
fn imported_macro_uses_expression_output(view: &ImportedMacroView, arg_count: usize) -> bool {
    view.leading_outputs == 1 && arg_count + 1 == view.arity
}

impl<'c> Driver<'c> {
    /// Zig `planImportedMacroCall` (lowering_rules.zig:3268) over
    /// the `CheckInfo` imported-macro registry.
    pub(crate) fn plan_imported_macro_call(
        &self,
        call: &CallExpr,
    ) -> Option<ImportedMacroCallPlan> {
        if call.associated_target.is_some() {
            return None;
        }
        let view = self.check.imported_macro(&call.func_name)?;
        let expression_output =
            imported_macro_uses_expression_output(&view, call.args.len());
        if call.args.len() != view.arity && !expression_output {
            return None;
        }
        Some(ImportedMacroCallPlan {
            macro_name: call.func_name.clone(),
            import_path: view.import_path.clone(),
            arity: view.arity,
            leading_outputs: view.leading_outputs,
            borrowed_arg_mask: view.borrowed_arg_mask,
            address_slot_arg_mask: view.address_slot_arg_mask,
            expression_output,
        })
    }

    /// The `gen_call` dispatch for an imported-macro call (Zig
    /// genCall's plan arm): expand through the ported G machinery,
    /// then apply Zig's result bookkeeping — a pointer-scalar
    /// result is non-owning. Returns `None` when no plan applies
    /// so the caller falls through to its remaining arms.
    pub(crate) fn gen_imported_macro_call(
        &mut self,
        expr: &Node,
        call: &CallExpr,
    ) -> Option<Result<u32>> {
        let plan = self.plan_imported_macro_call(call)?;
        Some(self.expand_planned_imported_macro(expr, call, plan))
    }

    fn expand_planned_imported_macro(
        &mut self,
        expr: &Node,
        call: &CallExpr,
        plan: ImportedMacroCallPlan,
    ) -> Result<u32> {
        // The result type is computed before expansion: the
        // expansion's W1 re-materialisation gate needs it, and the
        // checker query is independent of codegen state.
        let result_ty = self
            .check
            .expr_type(expr)
            .cloned()
            .or_else(|| literal_fallback_type(expr));
        let reg = crate::macro_expand::expand_imported_macro_call(
            self,
            &plan,
            &call.args,
            None,
            result_ty.as_ref(),
        )?;
        // Zig genCall: `if (typeIsPointerScalarValue(ty))
        // markNonOwningReg(reg)` over the call's result type.
        if let Some(ty) = &result_ty {
            if crate::release_emit::type_is_pointer_scalar_value(ty) {
                crate::release_meta::mark_non_owning_reg(&mut self.state, reg);
            }
        }
        Ok(reg)
    }

    /// Zig `genStringLiteral` (13215): intern the decoded bytes as
    /// a utf8 const, then build the `{ptr, len}` slice value on a
    /// stack slot (non-owning; SliceAbi offsets 0 / 8).
    pub(crate) fn gen_string_literal(&mut self, value: &str) -> Result<u32> {
        let label = self.b().string_literal_const_label(value)?;
        let bytes = crate::literals::decode_string_literal_bytes(value.as_bytes())?;
        let len_reg = self.b().new_tmp();
        self.b().emit_assign_imm(len_reg, bytes.len() as i64);
        let slice_reg = self.b().new_tmp();
        crate::release_prim::emit_stack_alloc(&mut self.state, slice_reg, 16);
        crate::release_meta::mark_non_owning_reg(&mut self.state, slice_reg);
        let ptr_reg = self.b().new_tmp();
        self.emit_borrow_symbol(ptr_reg, &label);
        self.emit_store(slice_reg, 0, ptr_reg, PrimType::Ptr);
        self.emit_store(slice_reg, 8, len_reg, PrimType::U64);
        self.emit_release(ptr_reg)?;
        self.emit_release(len_reg)?;
        Ok(slice_reg)
    }

    /// Zig `emitBorrowSymbol` (6753): a read borrow of an
    /// interned symbol (a const label) into `dst`.
    fn emit_borrow_symbol(&mut self, dst: u32, symbol_name: &str) {
        let symbol_id = self.b().intern(symbol_name);
        self.b().record_reg(symbol_id);
        self.emit_borrow_reg(dst, symbol_id, "read");
    }

    /// Zig `genStrPtrCall` (13238): the literal form only — a raw
    /// pointer to the string's interned const bytes. Non-literal
    /// arguments return `None` and reach the imported-macro
    /// expansion of `STR_PTR` itself.
    pub(crate) fn gen_str_ptr_call(&mut self, call: &CallExpr) -> Option<Result<u32>> {
        if call.func_name != "STR_PTR" || call.args.len() != 1 {
            return None;
        }
        match &call.args[0] {
            Node::Literal(Literal::StringVal(text)) => {
                let text = text.clone();
                Some(self.gen_raw_pointer_string_literal_arg(&text))
            }
            _ => None,
        }
    }

    /// Zig `genRawPointerStringLiteralArg` (13231).
    fn gen_raw_pointer_string_literal_arg(&mut self, value: &str) -> Result<u32> {
        let label = self.b().string_literal_const_label(value)?;
        let ptr_reg = self.b().new_tmp();
        self.emit_borrow_symbol(ptr_reg, &label);
        Ok(ptr_reg)
    }

    /// Zig `genStrLenCall` (13252): the literal form only — the
    /// decoded byte length as an immediate.
    pub(crate) fn gen_str_len_call(&mut self, call: &CallExpr) -> Option<Result<u32>> {
        if call.func_name != "STR_LEN" || call.args.len() != 1 {
            return None;
        }
        match &call.args[0] {
            Node::Literal(Literal::StringVal(text)) => {
                let text = text.clone();
                Some(self.gen_str_len_literal(&text))
            }
            _ => None,
        }
    }

    fn gen_str_len_literal(&mut self, value: &str) -> Result<u32> {
        let bytes = crate::literals::decode_string_literal_bytes(value.as_bytes())?;
        let dst = self.b().new_tmp();
        self.b().emit_assign_imm(dst, bytes.len() as i64);
        Ok(dst)
    }

    /// The `stack_alloc` arm of Zig `genCall` (with
    /// `stackAllocSize`, 2693): the size is the first argument's
    /// integer constant value (an int literal or a global scalar
    /// const), defaulting to 16; the slot register is pushed as a
    /// stack-alloc local under its own symbol name.
    pub(crate) fn gen_stack_alloc_call(&mut self, call: &CallExpr) -> Result<u32> {
        let size = self.stack_alloc_size(call)?;
        let dst = self.b().new_tmp();
        crate::release_prim::emit_stack_alloc(&mut self.state, dst, size);
        // Zig `pushStackAllocLocal(self.symbols.items[dst], dst)`.
        let name = self.b().symbol_name(dst).to_string();
        self.state.frame.push_local(Local {
            name,
            reg: dst,
            is_param: false,
            stack_ty: None,
            ty: None,
            param_cap: None,
            is_stack_alloc: true,
        });
        Ok(dst)
    }

    /// Zig `stackAllocSize` (2693) over the port's global scalar
    /// const table (const values are stored as their literal
    /// nodes by the C1 pre-scan).
    fn stack_alloc_size(&self, call: &CallExpr) -> Result<usize> {
        let Some(arg) = call.args.first() else {
            return Ok(16);
        };
        let value = match arg {
            Node::Literal(Literal::IntVal(v)) => Some(*v),
            Node::Identifier(name) => match self.state.program.global_scalar_consts.get(name) {
                Some(Node::Literal(Literal::IntVal(v))) => Some(*v),
                _ => None,
            },
            _ => None,
        };
        match value {
            Some(v) if v < 0 => Err(unsupported(
                "negative stack_alloc size",
                "G gen_call (stack_alloc)",
                "2696",
            )),
            Some(v) => Ok(v as usize),
            None => Ok(16),
        }
    }

    /// Zig `emitPtrAdd` with a register offset (agg_emit's
    /// `emit_ptr_add_imm` is the immediate-operand sibling).
    fn emit_ptr_add_reg(&mut self, dst: u32, base: u32, offset: u32) {
        self.b().record_reg(dst);
        let mut inst = Instruction::new(InstKind::PtrAdd, 0, self.b().instructions_len() as u32);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::Reg(base);
        inst.operands[2] = Operand::Reg(offset);
        self.b().append_instruction(inst);
    }

    /// The PTR_BYTE_ADD / SLA_PTR_ADD shared dst bookkeeping (Zig
    /// emitDirectImportedMacroCall): a dst derived from a borrow /
    /// non-owning / local / borrow-result base is consumed;
    /// otherwise it is non-owning.
    fn direct_ptr_add_dst(
        &mut self,
        dst: u32,
        base: u32,
    ) -> DirectImportedMacroResult {
        let base_is_view = self.is_local_reg(base)
            || self.state.frame.borrow_address_temps.contains_key(&base)
            || matches!(
                self.state.frame.reg_states.get(&base),
                Some(RegState::NonOwning | RegState::BorrowResult)
            );
        if base_is_view {
            crate::release_emit::mark_consumed(&mut self.state, dst);
            DirectImportedMacroResult {
                emitted: true,
                consumed_dst: Some(dst),
            }
        } else {
            crate::release_meta::mark_non_owning_reg(&mut self.state, dst);
            DirectImportedMacroResult {
                emitted: true,
                consumed_dst: None,
            }
        }
    }
}

/// The SAB-D12 literal fallback (mirrors `driver_gen.rs`'s copy):
/// a literal operand's own shape supplies its type when the
/// checker recorded none.
fn literal_fallback_type(node: &Node) -> Option<Type> {
    match node {
        Node::Literal(Literal::IntVal(_)) => Some(Type::Primitive(Primitive::I64)),
        Node::Literal(Literal::FloatVal(_)) => Some(Type::Primitive(Primitive::F64)),
        Node::Literal(Literal::BoolVal(_)) => Some(Type::Primitive(Primitive::Boolean)),
        _ => None,
    }
}

impl<'c> MacroCallHost for Driver<'c> {
    fn builder(&mut self) -> &mut ModuleBuilder {
        self.b()
    }

    fn gen_expr(&mut self, arg: &Node, ctx: Option<&MacroExpansionContext>) -> Result<u32> {
        match ctx {
            None => Driver::gen_expr(self, arg),
            Some(_) => Err(unsupported(
                "imported macro argument under a macro expansion context",
                "G macros (macro context)",
                "15072",
            )),
        }
    }

    fn gen_address_of(
        &mut self,
        arg: &Node,
        ctx: Option<&MacroExpansionContext>,
    ) -> Result<AddressSource> {
        match ctx {
            None => Driver::gen_address_of(self, arg),
            Some(_) => Err(unsupported(
                "imported macro address argument under a macro expansion context",
                "G macros (macro context)",
                "15072",
            )),
        }
    }

    fn arg_type(
        &mut self,
        arg: &Node,
        _ctx: Option<&MacroExpansionContext>,
    ) -> Result<Option<Type>> {
        Ok(self
            .check
            .expr_type(arg)
            .cloned()
            .or_else(|| literal_fallback_type(arg)))
    }

    fn gen_string_literal(&mut self, value: &str) -> Result<u32> {
        Driver::gen_string_literal(self, value)
    }

    fn binding_reg(&mut self, name: &str) -> Result<u32> {
        Ok(Driver::binding_reg(self, name))
    }

    fn macro_assign_target_name(
        &mut self,
        _arg: &Node,
        _ctx: &MacroExpansionContext,
    ) -> Option<String> {
        None
    }

    fn stack_local_reg(&self, name: &str) -> Option<u32> {
        crate::captures::stack_local(&self.state, name).map(|local| local.reg)
    }

    fn stack_alloc_emitted(&self, reg: u32) -> bool {
        self.state.frame.stack_alloc_emitted.contains(&reg)
    }

    fn note_stack_alloc(&mut self, reg: u32) {
        self.state.frame.reg_states.insert(reg, RegState::StackAlloc);
        self.state.frame.stack_alloc_emitted.insert(reg);
    }

    fn is_local_reg(&self, reg: u32) -> bool {
        Driver::is_local_reg(self, reg)
    }

    fn has_global_scalar_const(&self, name: &str) -> bool {
        self.state.program.global_scalar_consts.contains_key(name)
    }

    fn expr_has_fnptr_type(&self, arg: &Node) -> bool {
        matches!(self.check.expr_type(arg), Some(Type::FnPtr(_)))
    }

    fn funcs_contains(&self, name: &str) -> bool {
        self.funcs.contains_key(name)
    }

    fn expr_type(&self, arg: &Node) -> Option<Type> {
        self.check.expr_type(arg).cloned()
    }

    fn has_constructor_surface_rule(&self, type_name: &str, name: &str) -> bool {
        crate::surface_query::find_std_surface_rule(
            &self.state.program.std_surface_rules,
            crate::types::StdSurfaceRuleKind::Constructor,
            type_name,
            Some(name),
        )
        .is_some()
    }

    fn type_is_pointer_scalar_value(&self, ty: &Type) -> bool {
        crate::release_emit::type_is_pointer_scalar_value(ty)
    }

    fn emit_release(&mut self, reg: u32) -> Result<()> {
        Driver::emit_release(self, reg)
    }

    fn emit_move(&mut self, reg: u32) -> Result<()> {
        crate::release_emit::emit_move(&mut self.state, reg);
        Ok(())
    }

    fn mark_consumed(&mut self, reg: u32) -> Result<()> {
        crate::release_emit::mark_consumed(&mut self.state, reg);
        Ok(())
    }
}

impl<'c> MacroExpandHost for Driver<'c> {
    /// Zig `plannedCallArgReleaseReg` (20698): a candidate is
    /// released unless it carries a RefCell borrow handle
    /// (`planRefCellCallArgLifecycle` withholds those).
    fn planned_call_arg_release_reg(&self, candidate: Option<u32>) -> Option<u32> {
        match candidate {
            Some(reg) if !self.state.frame.refcell_borrow_values.contains_key(&reg) => {
                Some(reg)
            }
            _ => None,
        }
    }

    /// Zig `emitDirectImportedMacroCall` (18866), the arms the
    /// corpus' direct-lowered macros need: byte/pointer helpers
    /// whose SA bodies the SAB driver lowers natively. Anything
    /// else reports `emitted: false` and the caller splices the
    /// macro's own body through the fragment machine.
    fn emit_direct_imported_macro_call(
        &mut self,
        macro_name: &str,
        arg_names: &[String],
        _known_init_func: Option<&str>,
    ) -> Result<DirectImportedMacroResult> {
        let not_emitted = DirectImportedMacroResult {
            emitted: false,
            consumed_dst: None,
        };
        match macro_name {
            "PTR_BYTE_ADD" | "SLA_PTR_ADD" => {
                if arg_names.len() != 3 {
                    return Ok(not_emitted);
                }
                let (dst, base, offset) = (
                    self.b().intern(&arg_names[0]),
                    self.b().intern(&arg_names[1]),
                    self.b().intern(&arg_names[2]),
                );
                self.emit_ptr_add_reg(dst, base, offset);
                Ok(self.direct_ptr_add_dst(dst, base))
            }
            "PTR_READ_U8" => {
                if arg_names.len() != 2 {
                    return Ok(not_emitted);
                }
                let (dst, ptr) = (
                    self.b().intern(&arg_names[0]),
                    self.b().intern(&arg_names[1]),
                );
                self.emit_load(dst, ptr, 0, PrimType::U8);
                Ok(DirectImportedMacroResult {
                    emitted: true,
                    consumed_dst: None,
                })
            }
            "PTR_READ_U64" | "PTR_READ" => {
                if arg_names.len() != 2 {
                    return Ok(not_emitted);
                }
                let (dst, ptr) = (
                    self.b().intern(&arg_names[0]),
                    self.b().intern(&arg_names[1]),
                );
                self.emit_load(dst, ptr, 0, PrimType::U64);
                Ok(DirectImportedMacroResult {
                    emitted: true,
                    consumed_dst: None,
                })
            }
            "SLA_BYTE_AT" => {
                if arg_names.len() != 3 {
                    return Ok(not_emitted);
                }
                let (dst, ptr, offset) = (
                    self.b().intern(&arg_names[0]),
                    self.b().intern(&arg_names[1]),
                    self.b().intern(&arg_names[2]),
                );
                let addr = self.b().new_tmp();
                self.emit_ptr_add_reg(addr, ptr, offset);
                self.emit_load(dst, addr, 0, PrimType::U8);
                // DEF-SAB-PHISTATE (Zig comment): the SA macro
                // body ends with `!__sla_ba_addr`; retire the
                // address temp the same way, or a borrow-derived
                // base leaves it Active|InteriorPtr at a loop
                // back-edge (PhiStateConflict 1015).
                self.emit_release(addr)?;
                Ok(DirectImportedMacroResult {
                    emitted: true,
                    consumed_dst: None,
                })
            }
            "SLA_BYTE_PUT" => {
                if arg_names.len() != 3 {
                    return Ok(not_emitted);
                }
                let (ptr, offset, value) = (
                    self.b().intern(&arg_names[0]),
                    self.b().intern(&arg_names[1]),
                    self.b().intern(&arg_names[2]),
                );
                let addr = self.b().new_tmp();
                self.emit_ptr_add_reg(addr, ptr, offset);
                self.emit_store(addr, 0, value, PrimType::U8);
                // DEF-SAB-PHISTATE: same dropped release as
                // SLA_BYTE_AT (the body ends with `!__sla_bp_addr`).
                self.emit_release(addr)?;
                Ok(DirectImportedMacroResult {
                    emitted: true,
                    consumed_dst: None,
                })
            }
            _ => Ok(not_emitted),
        }
    }

    /// Zig `emitStdMacroFragment` over the F machine.
    fn emit_std_macro_fragment(
        &mut self,
        import_path: &str,
        macro_name: &str,
        arg_names: &[String],
    ) -> Result<()> {
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
            &mut *self.flattener,
            import_path,
            macro_name,
            arg_names,
        )
    }

    /// Zig `pushTypedLocal(name, reg, false, ty)`.
    fn push_typed_local(&mut self, name: &str, reg: u32, ty: &Type) -> Result<()> {
        self.state.frame.push_local(Local {
            name: name.to_string(),
            reg,
            is_param: false,
            stack_ty: None,
            ty: Some(ty.clone()),
            param_cap: None,
            is_stack_alloc: false,
        });
        Ok(())
    }

    /// Zig `releaseNonLocalTemps` — the port's production
    /// release paths run with the loud-default fragment emitter
    /// (E7 discipline), and this host does the same.
    fn release_non_local_temps(&mut self, regs: &[u32]) -> Result<()> {
        let mut frags = crate::release_emit::NoMacroFragments;
        crate::release_emit::release_non_local_temps(&mut self.state, &mut frags, regs)
    }
}

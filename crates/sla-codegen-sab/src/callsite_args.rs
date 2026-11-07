//! G group (materializeCallsiteArgs): the call-site argument
//! materialization family for imported-macro expansion.
//!
//! Faithful port of the `sab_codegen.zig` family (reference tree
//! @3de91932):
//! - `SabLoweredCallArg` (15571)
//! - `genImportedMacroValueArg` (14504),
//!   `importedMacroValueArgNeedsRelease` (14560),
//!   `importedMacroOutputTargetName` (14510),
//!   `genImportedMacroLeadingOutputArg` (14518),
//!   `importedMacroDirectCallConsumesValueArg` (14556)
//! - `genImportedMacroAddressExpressionSource` (14580),
//!   `genImportedMacroMaterializedSlotArg` (14585),
//!   `cleanupImportedMacroMaterializedSlotValue` (14604),
//!   `genImportedMacroAddressExpressionMaterializedSlotArg`
//!   (14619), `genImportedMacroAddressExpressionArg` (14644)
//! - `genImportedMacroArg` (14656) — the ArgRole dispatch
//! - `importedMacroExistingAddressableSymbol` (14452),
//!   `importedMacroArgType` (14470),
//!   `importedMacroArgAddressShape` (14475)
//! - `materializeFnPtrValueArgSlot` (15960)
//!
//! Everything that belongs to other groups reaches this family
//! through [`MacroCallHost`]: expression generation (driver /
//! gen-core), the full release engine (E group), fragment
//! emission (F group). The stack-alloc / load / store instruction
//! shapes are emitted here over `ModuleBuilder` because the B5
//! trunk does not yet carry `emitStackAlloc` / `emitLoad` /
//! `emitStore` helpers; their operand layouts are Zig's
//! (sab_codegen.zig:3949/3967/3987) exactly. See the G-group
//! report's wiring requirements.
//!
//! Ordering discipline: no hash-map iteration is used anywhere in
//! this file; argument processing is strictly in call order.

use sla_ast::{Literal, Node, Type};

use crate::emit::ModuleBuilder;
use crate::error::{Error, Result};
use crate::ir::{InstKind, Instruction, Operand, PrimType};
use crate::layout::{addressable_slot_prim_type, type_base_name, type_size};
use crate::macro_ctx;
use crate::macro_plan::{
    imported_macro_direct_call_consumes_value_arg, plan_address_of_shape, AddressOfShape,
    ImportedMacroCallPlan,
};
use crate::types::{AddressSource, MacroExpansionContext};

/// Zig `SabLoweredCallArg` (sab_codegen.zig:15571).
#[derive(Debug, Clone, PartialEq)]
pub struct SabLoweredCallArg {
    pub operand: String,
    pub release_reg: Option<u32>,
    pub release_regs: Vec<u32>,
    pub consume_reg: Option<u32>,
    pub forget_reg: Option<u32>,
    pub restore_slot: Option<u32>,
    pub restore_value: Option<u32>,
    pub output_bind_name: Option<String>,
    pub output_bind_ty: Option<Type>,
    pub output_store_slot: Option<u32>,
    pub output_store_ty: Option<PrimType>,
}

impl SabLoweredCallArg {
    /// A lowered arg carrying only an operand (the common shape:
    /// Zig `.{ .operand = ..., .release_reg = null }`).
    pub fn operand_only(operand: String) -> Self {
        SabLoweredCallArg {
            operand,
            release_reg: None,
            release_regs: Vec::new(),
            consume_reg: None,
            forget_reg: None,
            restore_slot: None,
            restore_value: None,
            output_bind_name: None,
            output_bind_ty: None,
            output_store_slot: None,
            output_store_ty: None,
        }
    }
}

/// Host services the G family consumes from the surrounding
/// codegen (driver state + gen-core + E-group release engine).
/// Implementations live with the driver wiring; this trait is the
/// mechanism seam, mirroring the C/D/F provider pattern.
pub trait MacroCallHost {
    /// The emission trunk.
    fn builder(&mut self) -> &mut ModuleBuilder;

    /// Zig `genMacroExpr` when `ctx` is present, else `genExpr`.
    fn gen_expr(&mut self, arg: &Node, ctx: Option<&MacroExpansionContext>) -> Result<u32>;

    /// Zig `genMacroAddressOf` / `genAddressOf` (via
    /// `genImportedMacroAddressExpressionSource`).
    fn gen_address_of(
        &mut self,
        arg: &Node,
        ctx: Option<&MacroExpansionContext>,
    ) -> Result<AddressSource>;

    /// Zig `importedMacroArgType`: `macroExprType` under a
    /// context, else `exprTypeOrFallback`.
    fn arg_type(
        &mut self,
        arg: &Node,
        ctx: Option<&MacroExpansionContext>,
    ) -> Result<Option<Type>>;

    /// Zig `genStringLiteral`.
    fn gen_string_literal(&mut self, value: &str) -> Result<u32>;

    /// Zig `bindingReg`.
    fn binding_reg(&mut self, name: &str) -> Result<u32>;

    /// Zig `macroAssignTargetName` (context path of
    /// `importedMacroOutputTargetName`).
    fn macro_assign_target_name(
        &mut self,
        arg: &Node,
        ctx: &MacroExpansionContext,
    ) -> Option<String>;

    /// Zig `stackLocal(name) != null` mapped to its slot register.
    fn stack_local_reg(&self, name: &str) -> Option<u32>;

    /// Zig `stack_alloc_emitted.contains(reg)`.
    fn stack_alloc_emitted(&self, reg: u32) -> bool;

    /// Zig side effects of `emitStackAlloc` on the codegen state
    /// (`stack_alloc_regs` + `stack_alloc_emitted` insertion).
    fn note_stack_alloc(&mut self, reg: u32);

    /// Zig `isLocalReg`.
    fn is_local_reg(&self, reg: u32) -> bool;

    /// Zig `global_scalar_consts.contains(name)`.
    fn has_global_scalar_const(&self, name: &str) -> bool;

    /// Zig `exprHasFnPtrType(arg)`.
    fn expr_has_fnptr_type(&self, arg: &Node) -> bool;

    /// Zig `tc.funcs.contains(name)`.
    fn funcs_contains(&self, name: &str) -> bool;

    /// Zig `tc.expr_types.get(arg)`.
    fn expr_type(&self, arg: &Node) -> Option<Type>;

    /// Zig `findStdSurfaceRule(.constructor, type_name, name)`.
    fn has_constructor_surface_rule(&self, type_name: &str, name: &str) -> bool;

    /// Zig `typeIsPointerScalarValue` (lowering_rules).
    fn type_is_pointer_scalar_value(&self, ty: &Type) -> bool;

    /// Zig `emitRelease` — the E-group release engine, with its
    /// full RefCell/guard/stack-slot semantics.
    fn emit_release(&mut self, reg: u32) -> Result<()>;

    /// Zig `emitMove` — the E-group move emission.
    fn emit_move(&mut self, reg: u32) -> Result<()>;

    /// Zig `markConsumed`.
    fn mark_consumed(&mut self, reg: u32) -> Result<()>;
}

/// Zig `emitStackAlloc` (sab_codegen.zig:3949) instruction shape,
/// including the re-emit no-op for an already-emitted slot (the
/// name-keyed register identity makes sibling-scope slots share
/// an id; a second `stack_alloc` would trip RegisterRedefinition).
pub fn emit_stack_alloc(host: &mut dyn MacroCallHost, dst: u32, size: usize) {
    let builder = host.builder();
    builder.record_reg(dst);
    if host.stack_alloc_emitted(dst) {
        return;
    }
    host.note_stack_alloc(dst);
    let builder = host.builder();
    let mut inst = Instruction::new(InstKind::StackAlloc, 0, builder_instructions_len(builder));
    inst.operands[0] = Operand::Reg(dst);
    inst.operands[1] = Operand::ImmU64(size as u64);
    builder.append_instruction(inst);
}

/// Zig `emitLoad` (sab_codegen.zig:3967) instruction shape.
pub fn emit_load(host: &mut dyn MacroCallHost, dst: u32, base: u32, offset: usize, ty: PrimType) {
    let builder = host.builder();
    builder.record_reg(dst);
    let mut inst = Instruction::new(InstKind::Load, 0, builder_instructions_len(builder));
    inst.operands[0] = Operand::Reg(dst);
    inst.operands[1] = Operand::Reg(base);
    inst.operands[2] = Operand::ImmU64(offset as u64);
    inst.operands[3] = Operand::Ty(ty as u32);
    builder.append_instruction(inst);
}

/// Zig `emitStore` (sab_codegen.zig:3987) instruction shape.
pub fn emit_store(host: &mut dyn MacroCallHost, base: u32, offset: usize, value: u32, ty: PrimType) {
    let builder = host.builder();
    let mut inst = Instruction::new(InstKind::Store, 0, builder_instructions_len(builder));
    inst.operands[0] = Operand::Reg(base);
    inst.operands[1] = Operand::ImmU64(offset as u64);
    inst.operands[2] = Operand::Reg(value);
    inst.operands[3] = Operand::Ty(ty as u32);
    builder.append_instruction(inst);
}

fn builder_instructions_len(builder: &ModuleBuilder) -> u32 {
    builder.instructions_len() as u32
}

fn symbol_of(host: &mut dyn MacroCallHost, reg: u32) -> String {
    host.builder().symbol_name(reg).to_string()
}

/// Zig `importedMacroValueArgNeedsRelease` (sab_codegen.zig:14560).
pub fn imported_macro_value_arg_needs_release(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    reg: u32,
    ctx: Option<&MacroExpansionContext>,
) -> bool {
    if matches!(arg, Node::CastExpr(_)) {
        return !host.is_local_reg(reg);
    }
    if sla_lowering::expr_result_needs_release(arg) {
        return !host.is_local_reg(reg);
    }
    let name = match arg {
        Node::Identifier(name) => name.clone(),
        _ => return false,
    };
    if let Some(macro_ctx) = ctx {
        if macro_ctx::identifier_name(macro_ctx, &name).is_some() {
            return !host.is_local_reg(reg);
        }
        if let Some(binding) = macro_ctx::arg_binding(macro_ctx, &name) {
            let inner_ctx = binding.ctx.as_ref().map(|c| c.borrow());
            return imported_macro_value_arg_needs_release(
                host,
                &binding.arg,
                reg,
                inner_ctx.as_deref(),
            );
        }
    }
    if host.stack_local_reg(&name).is_some() {
        return !host.is_local_reg(reg);
    }
    if host.has_global_scalar_const(&name) {
        return !host.is_local_reg(reg);
    }
    if host.expr_has_fnptr_type(arg) && host.funcs_contains(&name) {
        return !host.is_local_reg(reg);
    }
    if let Some(expr_ty) = host.expr_type(arg) {
        if let Some(type_name) = type_base_name(&expr_ty) {
            if host.has_constructor_surface_rule(&type_name, &name) {
                return !host.is_local_reg(reg);
            }
        }
    }
    false
}

/// Zig `genImportedMacroValueArg` (sab_codegen.zig:14504).
pub fn gen_imported_macro_value_arg(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
    release_value: bool,
) -> Result<SabLoweredCallArg> {
    let arg_reg = host.gen_expr(arg, ctx)?;
    let operand = symbol_of(host, arg_reg);
    let release_reg = if release_value
        && imported_macro_value_arg_needs_release(host, arg, arg_reg, ctx)
    {
        Some(arg_reg)
    } else {
        None
    };
    Ok(SabLoweredCallArg {
        release_reg,
        ..SabLoweredCallArg::operand_only(operand)
    })
}

/// Zig `importedMacroOutputTargetName` (sab_codegen.zig:14510).
pub fn imported_macro_output_target_name(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
) -> Option<String> {
    if let Some(macro_ctx) = ctx {
        return host.macro_assign_target_name(arg, macro_ctx);
    }
    match arg {
        Node::Identifier(name) => Some(name.clone()),
        _ => None,
    }
}

/// Zig `genImportedMacroLeadingOutputArg` (sab_codegen.zig:14518).
pub fn gen_imported_macro_leading_output_arg(
    host: &mut dyn MacroCallHost,
    plan: &ImportedMacroCallPlan,
    call_arg_index: usize,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
    arg_ty: &Type,
) -> Result<Option<SabLoweredCallArg>> {
    if plan.expression_output || call_arg_index >= plan.leading_outputs {
        return Ok(None);
    }
    let name = match imported_macro_output_target_name(host, arg, ctx) {
        Some(name) => name,
        None => return Ok(None),
    };
    let dst = host.binding_reg(&name)?;
    if host.stack_alloc_emitted(dst) {
        // Address-slot output macros (for example PIN_AS_REF) write
        // into an existing stack allocation. Passing an
        // uninitialized temporary and copying it back produces an
        // undeclared SAB register. (Zig comment, 14530–14532.)
        if plan.call_arg_needs_direct_address_slot(call_arg_index) {
            let operand = symbol_of(host, dst);
            return Ok(Some(SabLoweredCallArg::operand_only(operand)));
        }
        let tmp = host.builder().new_tmp();
        let operand = symbol_of(host, tmp);
        return Ok(Some(SabLoweredCallArg {
            output_store_slot: Some(dst),
            output_store_ty: Some(addressable_slot_prim_type(arg_ty)?),
            ..SabLoweredCallArg::operand_only(operand)
        }));
    }
    let operand = symbol_of(host, dst);
    Ok(Some(SabLoweredCallArg {
        output_bind_name: Some(name),
        output_bind_ty: Some(arg_ty.clone()),
        ..SabLoweredCallArg::operand_only(operand)
    }))
}

/// Zig `cleanupImportedMacroMaterializedSlotValue`
/// (sab_codegen.zig:14604).
pub fn cleanup_imported_macro_materialized_slot_value(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    arg_ty: &Type,
    value_reg: u32,
    ctx: Option<&MacroExpansionContext>,
) -> Result<()> {
    if imported_macro_value_arg_needs_release(host, arg, value_reg, ctx) {
        host.emit_release(value_reg)?;
        return Ok(());
    }
    if !host.is_local_reg(value_reg) && host.type_is_pointer_scalar_value(arg_ty) {
        if host.stack_alloc_emitted(value_reg) {
            host.mark_consumed(value_reg)?;
        } else {
            host.emit_release(value_reg)?;
        }
    }
    Ok(())
}

/// Zig `genImportedMacroMaterializedSlotArg` (sab_codegen.zig:14585).
pub fn gen_imported_macro_materialized_slot_arg(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
) -> Result<SabLoweredCallArg> {
    let value_reg = host.gen_expr(arg, ctx)?;
    let arg_ty = host
        .arg_type(arg, ctx)?
        .ok_or_else(|| Error::MissingType("imported macro addressable type".to_string()))?;
    let slot = host.builder().new_tmp();
    emit_stack_alloc(host, slot, type_size(&arg_ty));
    let store_ty = addressable_slot_prim_type(&arg_ty)?;
    emit_store(host, slot, 0, value_reg, store_ty);
    cleanup_imported_macro_materialized_slot_value(host, arg, &arg_ty, value_reg, ctx)?;
    let operand = symbol_of(host, slot);
    Ok(SabLoweredCallArg::operand_only(operand))
}

/// Zig `genImportedMacroAddressExpressionMaterializedSlotArg`
/// (sab_codegen.zig:14619).
pub fn gen_imported_macro_address_expression_materialized_slot_arg(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
) -> Result<SabLoweredCallArg> {
    let source = host.gen_address_of(arg, ctx)?;
    let arg_ty = host
        .arg_type(arg, ctx)?
        .ok_or_else(|| Error::MissingType("imported macro address-expression type".to_string()))?;
    let slot = host.builder().new_tmp();
    emit_stack_alloc(host, slot, type_size(&arg_ty));
    let store_ty = addressable_slot_prim_type(&arg_ty)?;
    let loaded = host.builder().new_tmp();
    emit_load(host, loaded, source.reg, 0, store_ty);
    emit_store(host, slot, 0, loaded, store_ty);
    cleanup_imported_macro_materialized_slot_value(host, arg, &arg_ty, loaded, ctx)?;
    if !host.is_local_reg(source.reg) {
        host.emit_release(source.reg)?;
    }
    for release_reg in &source.release_regs {
        host.emit_release(*release_reg)?;
    }
    let operand = symbol_of(host, slot);
    Ok(SabLoweredCallArg::operand_only(operand))
}

/// Zig `genImportedMacroAddressExpressionArg` (sab_codegen.zig:14644).
pub fn gen_imported_macro_address_expression_arg(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
) -> Result<SabLoweredCallArg> {
    let source = host.gen_address_of(arg, ctx)?;
    let operand = symbol_of(host, source.reg);
    Ok(SabLoweredCallArg {
        release_reg: if !host.is_local_reg(source.reg) {
            Some(source.reg)
        } else {
            None
        },
        release_regs: source.release_regs.clone(),
        ..SabLoweredCallArg::operand_only(operand)
    })
}

/// Zig `importedMacroExistingAddressableSymbol`
/// (sab_codegen.zig:14452).
pub fn imported_macro_existing_addressable_symbol(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
) -> Option<String> {
    let name = match arg {
        Node::Identifier(name) => name.clone(),
        _ => return None,
    };
    if let Some(macro_ctx) = ctx {
        if let Some(mapped) = macro_ctx::identifier_name(macro_ctx, &name) {
            return host.stack_local_reg(&mapped).map(|reg| {
                let builder = host.builder();
                builder.symbol_name(reg).to_string()
            });
        }
        if let Some(binding) = macro_ctx::arg_binding(macro_ctx, &name) {
            let inner_ctx = binding.ctx.as_ref().map(|c| c.borrow());
            return imported_macro_existing_addressable_symbol(
                host,
                &binding.arg,
                inner_ctx.as_deref(),
            );
        }
    }
    host.stack_local_reg(&name).map(|reg| {
        let builder = host.builder();
        builder.symbol_name(reg).to_string()
    })
}

/// Zig `importedMacroArgAddressShape` (sab_codegen.zig:14475).
pub fn imported_macro_arg_address_shape(
    host: &mut dyn MacroCallHost,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
) -> Result<AddressOfShape> {
    if let Some(macro_ctx) = ctx {
        if let Node::Identifier(name) = arg {
            if macro_ctx::identifier_name(macro_ctx, name).is_some() {
                return Ok(AddressOfShape::Identifier);
            }
            if let Some(binding) = macro_ctx::arg_binding(macro_ctx, name) {
                let inner_ctx = binding.ctx.as_ref().map(|c| c.borrow());
                return imported_macro_arg_address_shape(host, &binding.arg, inner_ctx.as_deref());
            }
        }
    }
    let mut deref_source_ty = None;
    let mut index_target_ty = None;
    match arg {
        Node::DerefExpr(deref) => {
            deref_source_ty = Some(
                host.arg_type(&deref.expr, ctx)?
                    .ok_or_else(|| Error::MissingType("deref source type".to_string()))?,
            );
        }
        Node::IndexExpr(idx) => {
            index_target_ty = Some(
                host.arg_type(&idx.target, ctx)?
                    .ok_or_else(|| Error::MissingType("index target type".to_string()))?,
            );
        }
        _ => {}
    }
    Ok(plan_address_of_shape(
        arg,
        deref_source_ty.as_ref(),
        index_target_ty.as_ref(),
    ))
}

/// Zig `genImportedMacroArg` (sab_codegen.zig:14656): the ArgRole
/// dispatch for one call-site argument.
pub fn gen_imported_macro_arg(
    host: &mut dyn MacroCallHost,
    plan: &ImportedMacroCallPlan,
    call_arg_index: usize,
    arg: &Node,
    ctx: Option<&MacroExpansionContext>,
) -> Result<SabLoweredCallArg> {
    let arg_ty = host
        .arg_type(arg, ctx)?
        .ok_or_else(|| Error::MissingType("imported macro arg type".to_string()))?;
    if let Some(output_arg) =
        gen_imported_macro_leading_output_arg(host, plan, call_arg_index, arg, ctx, &arg_ty)?
    {
        return Ok(output_arg);
    }
    if let Node::Literal(Literal::StringVal(value)) = arg {
        if plan.call_arg_needs_addressable_slot(call_arg_index)
            && plan.macro_name.starts_with("BTREE_")
        {
            let slice_reg = host.gen_string_literal(value)?;
            let operand = symbol_of(host, slice_reg);
            return Ok(SabLoweredCallArg::operand_only(operand));
        }
    }
    let release_value =
        !imported_macro_direct_call_consumes_value_arg(&plan.macro_name, call_arg_index);
    if let Some(action) = plan.plan_arg_value_bypass_action(call_arg_index, arg, &arg_ty) {
        if action.passes_value() || action.passes_raw_pointer_value() {
            return gen_imported_macro_value_arg(host, arg, ctx, release_value);
        }
        // Zig falls through to `unreachable` here; the bypass
        // planner only ever yields pass-value / pass-raw-pointer /
        // none, so this arm is dead in Zig too. Ported as a loud
        // error per SAB-D06 discipline (see report, SAB-D122).
        return Err(Error::UnsupportedSabDirectFeature(format!(
            "imported macro bypass action {action:?} for {} arg {call_arg_index}",
            plan.macro_name
        )));
    }
    let existing_symbol = imported_macro_existing_addressable_symbol(host, arg, ctx);
    let address_shape = imported_macro_arg_address_shape(host, arg, ctx)?;
    let action = plan.plan_addressable_arg_lowering_action(
        call_arg_index,
        address_shape,
        existing_symbol.is_some(),
        &arg_ty,
    );
    if action.passes_value() || action.passes_pointer_backed_projection() {
        return gen_imported_macro_value_arg(host, arg, ctx, release_value);
    }
    if action.passes_address_expression() {
        return gen_imported_macro_address_expression_arg(host, arg, ctx);
    }
    if action.reuses_existing_addressable() {
        let operand = existing_symbol.expect("action implies existing symbol");
        return Ok(SabLoweredCallArg::operand_only(operand));
    }
    if action.materializes_stack_slot() {
        return gen_imported_macro_materialized_slot_arg(host, arg, ctx);
    }
    if action.materializes_address_expression_stack_slot() {
        return gen_imported_macro_address_expression_materialized_slot_arg(host, arg, ctx);
    }
    // Zig `unreachable` at the end of the dispatch (14689): every
    // action variant is handled above. Loud error, as above.
    Err(Error::UnsupportedSabDirectFeature(format!(
        "imported macro arg action {action:?} for {} arg {call_arg_index}",
        plan.macro_name
    )))
}

/// Zig `materializeFnPtrValueArgSlot` (sab_codegen.zig:15960):
/// load the fn-ptr out of its source slot into a fresh stack slot
/// for the call.
pub fn materialize_fnptr_value_arg_slot(
    host: &mut dyn MacroCallHost,
    source_reg: u32,
    release_source_after_call: bool,
) -> Result<SabLoweredCallArg> {
    let target = host.builder().new_tmp();
    emit_load(host, target, source_reg, 0, PrimType::Ptr);
    let slot = host.builder().new_tmp();
    emit_stack_alloc(host, slot, 8);
    emit_store(host, slot, 0, target, PrimType::Ptr);
    host.emit_move(target)?;
    let operand = symbol_of(host, slot);
    Ok(SabLoweredCallArg {
        release_reg: if release_source_after_call {
            Some(source_reg)
        } else {
            None
        },
        ..SabLoweredCallArg::operand_only(operand)
    })
}

#[cfg(test)]
mod tests;

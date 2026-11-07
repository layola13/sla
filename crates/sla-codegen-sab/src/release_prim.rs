//! E7 primitives — the raw instruction emitters the whole E
//! group builds on, mirroring the tail of the Zig Codegen
//! impl (emitLoad/emitStore/emitAlloc/emitStackAlloc/
//! emitPtrAdd/emitOp/emitConsumedMarker, sab_codegen.zig
//! lines 4700-4800 in the reference tree). Split from
//! release_emit.rs (E7) by the 700-line rule.

use crate::ir::{InstKind, Instruction, OpKind, Operand, PrimType};
use crate::locals::record_reg;
use crate::state::{CodegenState, RegState};


/// Zig `makeInst`: an instruction whose expanded line is the
/// current instruction count (the builder's own `make_inst`
/// does the same; it is private, so E7 rebuilds it here over
/// the builder's public length).
pub(crate) fn make_inst(state: &CodegenState, kind: InstKind) -> Instruction {
    Instruction::new(kind, 0, state.builder.instructions_len() as u32)
}

pub(crate) fn append_inst(state: &mut CodegenState, inst: Instruction) {
    state.builder.append_instruction(inst);
}

pub(crate) fn reg_name(state: &CodegenState, reg: u32) -> String {
    state.builder.symbol_name(reg).to_string()
}

/// Whether the frame currently classifies `reg` as released
/// (Zig `released_regs.contains`).
pub fn is_released(state: &CodegenState, reg: u32) -> bool {
    state.frame.reg_state(reg) == RegState::Released
}

/// Zig's `released_regs.put(reg)`: mark a register released.
pub fn mark_released(state: &mut CodegenState, reg: u32) {
    state.frame.reg_states.insert(reg, RegState::Released);
}

/// Zig `emitAlloc`.
pub fn emit_alloc(state: &mut CodegenState, dst: u32, size: usize) {
    record_reg(state, dst);
    let mut inst = make_inst(state, InstKind::Alloc);
    inst.operands[0] = Operand::Reg(dst);
    inst.operands[1] = Operand::ImmU64(size as u64);
    append_inst(state, inst);
}

/// Zig `emitStackAlloc`: records the slot in the stack-alloc
/// classification and emits at most once per register id —
/// register identity is name-keyed and function-global, so two
/// sibling-scope bindings of one name share the slot (Zig
/// comment, verbatim rationale).
pub fn emit_stack_alloc(state: &mut CodegenState, dst: u32, size: usize) {
    record_reg(state, dst);
    state.frame.reg_states.insert(dst, RegState::StackAlloc);
    if state.frame.stack_alloc_emitted.contains(&dst) {
        return;
    }
    state.frame.stack_alloc_emitted.insert(dst);
    let mut inst = make_inst(state, InstKind::StackAlloc);
    inst.operands[0] = Operand::Reg(dst);
    inst.operands[1] = Operand::ImmU64(size as u64);
    append_inst(state, inst);
}

/// Zig `emitLoad`.
pub fn emit_load(state: &mut CodegenState, dst: u32, base: u32, offset: usize, ty: PrimType) {
    record_reg(state, dst);
    let mut inst = make_inst(state, InstKind::Load);
    inst.operands[0] = Operand::Reg(dst);
    inst.operands[1] = Operand::Reg(base);
    inst.operands[2] = Operand::ImmU64(offset as u64);
    inst.operands[3] = Operand::Ty(ty as u32);
    append_inst(state, inst);
}

/// Zig `emitStore`.
pub fn emit_store(state: &mut CodegenState, base: u32, offset: usize, value: u32, ty: PrimType) {
    let mut inst = make_inst(state, InstKind::Store);
    inst.operands[0] = Operand::Reg(base);
    inst.operands[1] = Operand::ImmU64(offset as u64);
    inst.operands[2] = Operand::Reg(value);
    inst.operands[3] = Operand::Ty(ty as u32);
    append_inst(state, inst);
}

/// Zig `emitPtrAdd`.
pub fn emit_ptr_add(state: &mut CodegenState, dst: u32, base: u32, offset: Operand) {
    record_reg(state, dst);
    let mut inst = make_inst(state, InstKind::PtrAdd);
    inst.operands[0] = Operand::Reg(dst);
    inst.operands[1] = Operand::Reg(base);
    inst.operands[2] = offset;
    append_inst(state, inst);
}

/// Zig `emitOp`.
pub fn emit_op(state: &mut CodegenState, dst: u32, op: OpKind, lhs: Operand, rhs: Operand) {
    record_reg(state, dst);
    let mut inst = make_inst(state, InstKind::Op);
    inst.op_kind = Some(op);
    inst.operands[0] = Operand::Reg(dst);
    inst.operands[1] = lhs;
    inst.operands[2] = rhs;
    append_inst(state, inst);
}

/// Zig `emitConsumedMarker`: a fence whose text operand is the
/// `^name` marker (the DEF-ACL ownership-transfer idiom).
pub fn emit_consumed_marker(state: &mut CodegenState, value: u32) {
    let mut inst = make_inst(state, InstKind::Fence);
    inst.operands[0] = Operand::Text(format!("^{}", reg_name(state, value)));
    append_inst(state, inst);
}

//! H group — structured statements and expressions: the
//! `gen*` forms of `sab_codegen.zig` that are neither locals /
//! branch-state machinery (E), macro and call-site materialization
//! (G), nor aggregates / closures / literals (I):
//!
//! - `gen_for_range` — `genFor` numeric-range form (current-head
//!   Zig lines 19506–19603; the collection forms `genForOverArray`
//!   / `genForOverProtocol` stay loud at the H boundary: element
//!   materialization is I/D territory);
//! - `gen_block_tail_value_store` — `genBlockTailValueStore`
//!   (Zig 22377–22397), the shared tail-value mechanism of the
//!   value-shaped structured expressions;
//! - `gen_if_value` — `genIfValue` (Zig 22595–22703), the
//!   result-slot form of `if`;
//! - `gen_unsafe_expr` — `genUnsafeExpr` (Zig 13477–13509);
//! - `gen_cast` — `genCast` (Zig 13414–13476) with the
//!   `opKindForCast` mapping (Zig 2428–2457; that mapping is E1's
//!   canonical home — it is reproduced here at the H use site and
//!   registered as a hand-off, see the phase-4 H report);
//! - `gen_try` — `genTry` (Zig 12374–12452), user-defined
//!   result-struct arm; the Option/Result arms ride F-group std
//!   macro fragments and stay loud with that attribution;
//! - `gen_inline_asm` — the `genExpr` inline-asm arm (Zig
//!   12362–12369): a no-op yielding a zero sentinel, mirroring
//!   SA-text.
//!
//! Release instructions and RefCell / branch-state merge machinery
//! are E-group state work and are omitted exactly as in the B4
//! subset's `gen_if` / `gen_while` ports (SAB-D19 items 1 and 5):
//! the instruction *sequence* — label allocation order, branch
//! shape (including the SAB-D13 duplicated then-label via the
//! trunk's `emit_branch`), alloc / store / load forms — is Zig's.
//! Nothing here falls back to the text backend.

use sla_ast::{CastExpr, ForStmt, IfExpr, Node, TryExpr, Type, UnsafeExpr};
use sla_lowering::{block_terminates, is_numeric_type, plan_loop_control};

use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::ir::{InstKind, Instruction, OpKind, Operand, PrimType};
use crate::layout;
use crate::types::Local;

// -- emission helpers ------------------------------------------------
// The trunk (`emit.rs`, F-group wiring) exposes no alloc / load /
// store emitters in this baseline; these reproduce the Zig
// emitters' instruction shapes (sab_codegen.zig 4716–4788) through
// the trunk's verbatim append, recording registers exactly where
// the Zig emitters record them. Registered as a wiring need in
// the H report: when the trunk grows these emitters, these
// helpers should be deleted in favour of them.

impl<'c> Driver<'c> {
    fn h_push(&mut self, inst: Instruction) {
        self.b().append_instruction(inst);
    }

    fn h_inst(&mut self, kind: InstKind) -> Instruction {
        Instruction::new(kind, 0, self.b().instructions_len() as u32)
    }

    /// Zig `emitAlloc` (4716–4722): `dst = alloc <size imm_u64>`.
    pub(crate) fn h_emit_alloc(&mut self, dst: u32, size: usize) {
        self.b().record_reg(dst);
        let mut inst = self.h_inst(InstKind::Alloc);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::ImmU64(size as u64);
        self.h_push(inst);
    }

    /// Zig `emitStackAlloc` (3949): records the register in the
    /// stack-alloc classification and emits at most once per
    /// register id — register identity is name-keyed and
    /// function-global, so two sibling-scope bindings of one name
    /// (e.g. `let str_end` in two `if` blocks, which resolve to
    /// one interned register) share the slot; a second
    /// `stack_alloc` for the same id would trip the SAB
    /// verifier's RegisterRedefinition. Mirrors
    /// release_prim::emit_stack_alloc.
    pub(crate) fn h_emit_stack_alloc(&mut self, dst: u32, size: usize) {
        self.b().record_reg(dst);
        self.state.frame.reg_states.insert(dst, crate::state::RegState::StackAlloc);
        if self.state.frame.stack_alloc_emitted.contains(&dst) {
            return;
        }
        self.state.frame.stack_alloc_emitted.insert(dst);
        let mut inst = self.h_inst(InstKind::StackAlloc);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::ImmU64(size as u64);
        self.h_push(inst);
    }

    /// Zig `emitLoad` (4750–4760).
    pub(crate) fn h_emit_load(&mut self, dst: u32, base: u32, offset: usize, ty: PrimType) {
        self.b().record_reg(dst);
        let mut inst = self.h_inst(InstKind::Load);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::Reg(base);
        inst.operands[2] = Operand::ImmU64(offset as u64);
        inst.operands[3] = Operand::Ty(ty as u32);
        self.h_push(inst);
    }

    /// Zig `emitStore` (4770–4777): no register recording.
    pub(crate) fn h_emit_store(&mut self, base: u32, offset: usize, value: u32, ty: PrimType) {
        let mut inst = self.h_inst(InstKind::Store);
        inst.operands[0] = Operand::Reg(base);
        inst.operands[1] = Operand::ImmU64(offset as u64);
        inst.operands[2] = Operand::Reg(value);
        inst.operands[3] = Operand::Ty(ty as u32);
        self.h_push(inst);
    }

    /// The cast op shape (Zig `genCast`): an `op` instruction with
    /// operands `[Reg(dst), Reg(src), Ty(dst_ty)]`.
    fn h_emit_cast_op(&mut self, dst: u32, op: OpKind, src: u32, dst_ty: PrimType) {
        self.b().record_reg(dst);
        let mut inst = self.h_inst(InstKind::Op);
        inst.op_kind = Some(op);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::Reg(src);
        inst.operands[2] = Operand::Ty(dst_ty as u32);
        self.h_push(inst);
    }
}

// -- for (numeric range) --------------------------------------------

impl<'c> Driver<'c> {
    /// Zig `genFor` (19506–19603), numeric-range form. The
    /// collection forms dispatch before the range path in Zig and
    /// stay loud here (element / protocol materialization).
    pub(crate) fn gen_for(&mut self, f: &ForStmt) -> Result<()> {
        let end_expr = match &f.end {
            Some(end) => end,
            None => {
                return Err(unsupported(
                    "for over a collection (genForOverArray / genForOverProtocol forms)",
                    "H gen_for collection forms (I/D element materialization)",
                    "19509-19517",
                ))
            }
        };
        let saved_locals = self.state.frame.locals.len();
        let loop_control = plan_loop_control(&f.body);

        let counter_slot = self.b().new_tmp();
        self.h_emit_stack_alloc(counter_slot, 8);
        let start_reg = self.gen_expr(&f.start)?;
        let end_reg = self.gen_expr(end_expr)?;
        self.h_emit_store(counter_slot, 0, start_reg, PrimType::I64);

        // Zig allocates all six labels here, in this order, the
        // break-cleanup label included whether or not it is used.
        let head_label = self.b().new_label("L_FOR_HEAD");
        let body_label = self.b().new_label("L_FOR_BODY");
        let cont_label = self.b().new_label("L_FOR_CONTINUE");
        let cond_false_label = self.b().new_label("L_FOR_COND_FALSE");
        let break_cleanup_label = self.b().new_label("L_FOR_BREAK_CLEANUP");
        let exit_label = self.b().new_label("L_FOR_EXIT");

        self.b().emit_jmp(&head_label);
        self.b().emit_label(&head_label);
        let index_reg = self.b().new_tmp();
        self.h_emit_load(index_reg, counter_slot, 0, PrimType::I64);
        let cond = self.b().new_tmp();
        self.b()
            .emit_op(cond, OpKind::Slt, Operand::Reg(index_reg), Operand::Reg(end_reg));
        self.b().emit_branch(cond, &body_label, &cond_false_label);

        let body_locals_len = self.state.frame.locals.len();
        self.b().emit_label(&body_label);
        let loop_value = self.b().new_tmp();
        self.b()
            .emit_op(loop_value, OpKind::Add, Operand::Reg(index_reg), Operand::ImmI64(0));
        self.state
            .frame
            .push_local(Local::plain(&f.var_name, loop_value, false));
        self.state.frame.loop_continue_labels.push(cont_label.clone());
        self.state.frame.loop_break_labels.push(if loop_control.has_break {
            break_cleanup_label.clone()
        } else {
            exit_label.clone()
        });
        let result = self.gen_block(&f.body);
        self.state.frame.loop_continue_labels.pop();
        self.state.frame.loop_break_labels.pop();
        result?;
        if !self.b().last_is_terminator() {
            self.b().emit_jmp(&cont_label);
        }
        self.state.frame.pop_locals_to(body_locals_len);

        self.b().emit_label(&cont_label);
        let next = self.b().new_tmp();
        self.b()
            .emit_op(next, OpKind::Add, Operand::Reg(index_reg), Operand::ImmI64(1));
        self.h_emit_store(counter_slot, 0, next, PrimType::I64);
        self.b().emit_jmp(&head_label);

        if loop_control.has_break {
            self.b().emit_label(&break_cleanup_label);
            self.b().emit_jmp(&exit_label);
        }

        self.b().emit_label(&cond_false_label);
        self.b().emit_jmp(&exit_label);
        self.b().emit_label(&exit_label);
        self.state.frame.pop_locals_to(saved_locals);
        Ok(())
    }
}

// -- block tail value / if-value / unsafe ----------------------------

impl<'c> Driver<'c> {
    /// Zig `blockTailExpr` (2459–2464): the block's value tail is
    /// its last statement iff that statement is an expression
    /// statement.
    fn block_tail_expr<'n>(block: &'n [Node]) -> Option<&'n Node> {
        match block.last() {
            Some(Node::ExprStmt(inner)) => Some(inner),
            _ => None,
        }
    }

    /// Zig `genBlockTailValueStore` (22377–22397). Returns whether
    /// the block terminated (no value was stored on this path).
    /// The `storeResultSlotTransferredValue` ownership bookkeeping
    /// is E-group state work (SAB-D19 class) and is omitted.
    pub(crate) fn gen_block_tail_value_store(
        &mut self,
        block: &[Node],
        target: u32,
        target_ty: &Type,
    ) -> Result<bool> {
        let tail = match Self::block_tail_expr(block) {
            Some(tail) => tail,
            None => {
                for stmt in block {
                    self.gen_stmt(stmt)?;
                    if self.b().last_is_terminator() {
                        return Ok(true);
                    }
                }
                return Err(unsupported(
                    "block with no value tail in value position",
                    "H gen_block_tail_value_store",
                    "22377-22386",
                ));
            }
        };
        for stmt in &block[..block.len() - 1] {
            self.gen_stmt(stmt)?;
            if self.b().last_is_terminator() {
                return Ok(true);
            }
        }
        let value = self.gen_expr(tail)?;
        let prim = layout::prim_type(target_ty)?;
        self.h_emit_store(target, 0, value, prim);
        Ok(false)
    }

    /// Zig `genIfValue` (22595–22703), subset form: cond, result
    /// slot alloc, THEN/ELSE/MERGE labels allocated in that order,
    /// tail-value stores per branch, merge load. The RefCell
    /// branch-handle merge slots and the released-set merge are
    /// E-group state work (SAB-D19 class) and are omitted, as in
    /// the B4 `gen_if` statement port. `branch_needs_merge` in Zig
    /// gates only that E machinery; the merge label / load shape
    /// below follows the same !terminated conditions Zig uses.
    pub(crate) fn gen_if_value(
        &mut self,
        ife: &IfExpr,
        else_block: &[Node],
        result_ty: &Type,
    ) -> Result<u32> {
        let _ = block_terminates(&ife.then_block) || block_terminates(else_block);
        let cond = self.gen_expr(&ife.cond)?;
        let result_slot = self.b().new_tmp();
        self.h_emit_alloc(result_slot, layout::type_size(result_ty));
        let then_label = self.b().new_label("L_THEN");
        let else_label = self.b().new_label("L_ELSE");
        let merge_label = self.b().new_label("L_MERGE");
        self.b().emit_branch(cond, &then_label, &else_label);

        let saved_locals = self.state.frame.locals.len();
        self.b().emit_label(&then_label);
        let then_terminated =
            self.gen_block_tail_value_store(&ife.then_block, result_slot, result_ty)?;
        if !then_terminated {
            self.b().emit_jmp(&merge_label);
        }
        self.state.frame.pop_locals_to(saved_locals);

        self.b().emit_label(&else_label);
        let else_terminated =
            self.gen_block_tail_value_store(else_block, result_slot, result_ty)?;
        if !else_terminated {
            self.b().emit_jmp(&merge_label);
        }
        self.state.frame.pop_locals_to(saved_locals);

        if !then_terminated || !else_terminated {
            self.b().emit_label(&merge_label);
            let result = self.b().new_tmp();
            let prim = layout::prim_type(result_ty)?;
            self.h_emit_load(result, result_slot, 0, prim);
            return Ok(result);
        }
        // Both branches terminated: Zig returns a fresh recorded
        // bookkeeping tmp with no defining instruction (the slot
        // is never loaded) — mirrored, not "fixed".
        let result = self.b().new_tmp();
        self.b().record_reg(result);
        Ok(result)
    }

    /// Zig `genUnsafeExpr` (13477–13509), subset form. The
    /// RefCell result-slot companion is I/E state work and is
    /// omitted (SAB-D19 class).
    pub(crate) fn gen_unsafe_expr(&mut self, expr: &Node, unsafe_expr: &UnsafeExpr) -> Result<u32> {
        let result_ty = self
            .check
            .expr_type(expr)
            .cloned()
            .ok_or_else(|| Error::MissingType("unsafe expression type".to_string()))?;
        if sla_lowering::is_void_type(&result_ty) {
            // Void unsafe: run the block for effects, yield a zero
            // sentinel — assigned only on fall-through, exactly as
            // in Zig.
            self.gen_block(&unsafe_expr.body)?;
            let sentinel = self.b().new_tmp();
            self.b().record_reg(sentinel);
            if !self.b().last_is_terminator() {
                self.b().emit_assign_imm(sentinel, 0);
            }
            return Ok(sentinel);
        }
        let result_slot = self.b().new_tmp();
        self.h_emit_alloc(result_slot, layout::type_size(&result_ty));
        let terminated =
            self.gen_block_tail_value_store(&unsafe_expr.body, result_slot, &result_ty)?;
        if terminated {
            let result = self.b().new_tmp();
            self.b().record_reg(result);
            return Ok(result);
        }
        let result = self.b().new_tmp();
        let prim = layout::prim_type(&result_ty)?;
        self.h_emit_load(result, result_slot, 0, prim);
        Ok(result)
    }
}

// -- cast -------------------------------------------------------------

/// Zig `primTypeBits` (sci signature.zig) for the cast mapping.
fn prim_type_bits(ty: PrimType) -> u32 {
    match ty {
        PrimType::Void => 0,
        PrimType::I1 => 1,
        PrimType::I8 | PrimType::U8 => 8,
        PrimType::I16 | PrimType::U16 => 16,
        PrimType::I32 | PrimType::U32 | PrimType::F32 => 32,
        PrimType::I64 | PrimType::U64 | PrimType::F64 | PrimType::Ptr => 64,
        PrimType::BlobHandle => 64,
        PrimType::V128 => 128,
    }
}

/// Zig `isIntegerPrimType` (2390–2395).
fn is_integer_prim(ty: PrimType) -> bool {
    matches!(
        ty,
        PrimType::I1
            | PrimType::I8
            | PrimType::I16
            | PrimType::I32
            | PrimType::I64
            | PrimType::U8
            | PrimType::U16
            | PrimType::U32
            | PrimType::U64
    )
}

/// Zig `isSignedPrimType` (2397–2402).
fn is_signed_prim(ty: PrimType) -> bool {
    matches!(ty, PrimType::I8 | PrimType::I16 | PrimType::I32 | PrimType::I64)
}

/// Zig `isFloatPrimType` (2404–2409).
fn is_float_prim(ty: PrimType) -> bool {
    matches!(ty, PrimType::F32 | PrimType::F64)
}

/// Zig `opKindForCast` (2428–2457). This mapping's canonical home
/// in the decomposition is E1 (`opkinds.rs`); it is reproduced at
/// this H use site pending E1's landing and registered as a
/// hand-off in the H report — E1 must adopt it verbatim or the
/// cast bytes will diverge.
pub fn op_kind_for_cast(src_ty: PrimType, dst_ty: PrimType) -> Result<OpKind> {
    if src_ty == dst_ty {
        return Ok(OpKind::Bitcast);
    }
    let src_bits = prim_type_bits(src_ty);
    let dst_bits = prim_type_bits(dst_ty);
    let src_int = is_integer_prim(src_ty);
    let dst_int = is_integer_prim(dst_ty);
    let src_float = is_float_prim(src_ty);
    let dst_float = is_float_prim(dst_ty);
    if src_int && dst_int {
        if dst_bits < src_bits {
            return Ok(OpKind::Trunc);
        }
        if dst_bits > src_bits {
            return Ok(if is_signed_prim(src_ty) {
                OpKind::Sext
            } else {
                OpKind::Zext
            });
        }
        return Ok(OpKind::Bitcast);
    }
    if src_int && dst_float {
        return Ok(if is_signed_prim(src_ty) {
            OpKind::Sitofp
        } else {
            OpKind::Uitofp
        });
    }
    if src_float && dst_int {
        return Ok(OpKind::Fptosi);
    }
    if src_float && dst_float {
        return Ok(if dst_bits < src_bits {
            OpKind::Fptrunc
        } else {
            OpKind::Fpext
        });
    }
    if src_ty == PrimType::Ptr && dst_ty == PrimType::Ptr {
        return Ok(OpKind::Bitcast);
    }
    if (src_ty == PrimType::Ptr && dst_int && dst_bits == 64)
        || (dst_ty == PrimType::Ptr && src_int && src_bits == 64)
    {
        return Ok(OpKind::Bitcast);
    }
    Err(unsupported(
        "cast between these primitive kinds",
        "H gen_cast / E1 opkinds",
        "2428-2457",
    ))
}

impl<'c> Driver<'c> {
    /// Zig `genCast` (13414–13476): fn-pointer load arm,
    /// pointer/integer bitcast arm, numeric arm via
    /// `op_kind_for_cast`. Source-release bookkeeping is E-group
    /// state work and is omitted (SAB-D19 class).
    pub(crate) fn gen_cast(&mut self, cast: &CastExpr) -> Result<u32> {
        let src_ast_ty = self
            .check
            .expr_type(&cast.expr)
            .cloned()
            .ok_or_else(|| Error::MissingType("cast source type".to_string()))?;
        if matches!(src_ast_ty, Type::FnPtr(_)) {
            let src = self.gen_expr(&cast.expr)?;
            let dst_ty = layout::prim_type(&cast.ty)?;
            let loaded = self.b().new_tmp();
            self.h_emit_load(loaded, src, 0, dst_ty);
            if dst_ty != PrimType::Ptr {
                return Ok(loaded);
            }
            let dst = self.b().new_tmp();
            self.h_emit_cast_op(dst, OpKind::Bitcast, loaded, dst_ty);
            return Ok(dst);
        }
        let src_ty = layout::prim_type(&src_ast_ty)?;
        let dst_ty = layout::prim_type(&cast.ty)?;
        let src = self.gen_expr(&cast.expr)?;
        if !is_numeric_type(&src_ast_ty) || !is_numeric_type(&cast.ty) {
            // Pointer <-> integer reinterpretation is a register
            // bitcast (Zig's ZOZI-2 arm).
            if (src_ty == PrimType::Ptr && is_integer_prim(dst_ty))
                || (is_integer_prim(src_ty) && dst_ty == PrimType::Ptr)
                || (src_ty == PrimType::Ptr && dst_ty == PrimType::Ptr)
            {
                let dst = self.b().new_tmp();
                self.h_emit_cast_op(dst, OpKind::Bitcast, src, dst_ty);
                return Ok(dst);
            }
            return Err(unsupported(
                "cast between non-numeric, non-pointer kinds",
                "H gen_cast",
                "13441-13462",
            ));
        }
        let dst = self.b().new_tmp();
        let op = op_kind_for_cast(src_ty, dst_ty)?;
        self.h_emit_cast_op(dst, op, src, dst_ty);
        Ok(dst)
    }
}

// -- try --------------------------------------------------------------

impl<'c> Driver<'c> {
    /// Zig `genTry` (12374–12452), user-defined result-struct
    /// arm: load `is_err`, branch, error path returns the whole
    /// inner value, success path loads `value`. The Option /
    /// Result arms emit std macro fragments (F-group machinery)
    /// and stay loud with that attribution; the branch cleanup
    /// (`emitBranchCleanupForNode`) is E-group work, omitted as
    /// in the other H ports (SAB-D19 class).
    pub(crate) fn gen_try(&mut self, _expr: &Node, try_expr: &TryExpr) -> Result<u32> {
        let inner_ty = self
            .check
            .expr_type(&try_expr.expr)
            .cloned()
            .ok_or_else(|| Error::MissingType("try operand type".to_string()))?;
        let inner_reg = self.gen_expr(&try_expr.expr)?;
        let success_label = self.b().new_label("L_TRY_SUCCESS");
        let error_label = self.b().new_label("L_TRY_ERROR");

        // Option / Result detection follows Zig's order: those
        // arms come first and ride std fragments.
        let base_name = layout::type_base_name(&inner_ty);
        if matches!(base_name.as_deref(), Some("Option") | Some("Result")) {
            return Err(unsupported(
                "try over Option/Result (std macro fragment expansion)",
                "F fragments",
                "12382-12430",
            ));
        }
        let peeled = sla_lowering::peel_borrow_pointer_type(&inner_ty);
        let type_name = match peeled {
            Type::UserDefined(u) => u.name.clone(),
            _ => {
                return Err(unsupported(
                    "try over a non-result-struct type",
                    "H gen_try",
                    "12432-12436",
                ))
            }
        };
        let decl = self.check.struct_decl(&type_name).ok_or_else(|| {
            unsupported(
                "try result struct is not declared",
                "H gen_try",
                "12434",
            )
        })?;
        let is_err_layout = layout::struct_field_layout_detailed(decl, "is_err")
            .collapsed()
            .ok_or_else(|| {
                unsupported("try result struct has no is_err field", "H gen_try", "12435")
            })?;
        let value_layout = layout::struct_field_layout_detailed(decl, "value")
            .collapsed()
            .ok_or_else(|| {
                unsupported("try result struct has no value field", "H gen_try", "12436")
            })?;
        let is_err = self.b().new_tmp();
        // D2's FieldLayout already carries the storage PrimType.
        let is_err_prim = is_err_layout.ty;
        self.h_emit_load(is_err, inner_reg, is_err_layout.offset, is_err_prim);
        self.b().emit_branch(is_err, &error_label, &success_label);

        self.b().emit_label(&error_label);
        self.b().emit_return(Some(inner_reg));

        self.b().emit_label(&success_label);
        let value = self.b().new_tmp();
        let value_prim = value_layout.ty;
        self.h_emit_load(value, inner_reg, value_layout.offset, value_prim);
        Ok(value)
    }

    /// The `genExpr` inline-asm arm (Zig 12362–12369): inline
    /// assembly is a no-op yielding a zero sentinel, mirroring
    /// SA-text.
    pub(crate) fn gen_inline_asm(&mut self) -> Result<u32> {
        let sentinel = self.b().new_tmp();
        self.b().record_reg(sentinel);
        self.b().emit_assign_imm(sentinel, 0);
        Ok(sentinel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{Field, Primitive, StructDecl, UserDefined};
    use sla_lowering::{node_key, MapCheckInfo};

    #[test]
    fn cast_op_table_matches_zig() {
        use OpKind::*;
        use PrimType::*;
        assert_eq!(op_kind_for_cast(I32, I64).unwrap(), Sext);
        assert_eq!(op_kind_for_cast(U32, U64).unwrap(), Zext);
        assert_eq!(op_kind_for_cast(I64, I32).unwrap(), Trunc);
        assert_eq!(op_kind_for_cast(I32, I32).unwrap(), Bitcast);
        assert_eq!(op_kind_for_cast(I32, F64).unwrap(), Sitofp);
        assert_eq!(op_kind_for_cast(U64, F32).unwrap(), Uitofp);
        assert_eq!(op_kind_for_cast(F64, I32).unwrap(), Fptosi);
        assert_eq!(op_kind_for_cast(F64, F32).unwrap(), Fptrunc);
        assert_eq!(op_kind_for_cast(F32, F64).unwrap(), Fpext);
        assert_eq!(op_kind_for_cast(Ptr, U64).unwrap(), Bitcast);
        assert_eq!(op_kind_for_cast(I64, Ptr).unwrap(), Bitcast);
        assert!(op_kind_for_cast(Ptr, I32).is_err());
        assert!(op_kind_for_cast(Void, I32).is_err());
    }

    #[test]
    fn try_struct_arm_instruction_shape() {
        // struct R { is_err: bool, value: i32 }; a local `r: R`,
        // expression `r?` — load is_err, branch (error first),
        // error path returns the whole inner value, success path
        // loads `value`.
        let decl = StructDecl {
            name: "R".to_string(),
            derives: Vec::new(),
            generics: Vec::new(),
            fields: vec![
                Field {
                    name: "is_err".to_string(),
                    ty: Box::new(Type::Primitive(Primitive::Boolean)),
                },
                Field {
                    name: "value".to_string(),
                    ty: Box::new(Type::Primitive(Primitive::I32)),
                },
            ],
            is_union: false,
            is_opaque: false,
        };
        let r_ty = Type::UserDefined(UserDefined {
            name: "R".to_string(),
            generics: Vec::new(),
        });
        let inner = Node::Identifier("r".to_string());
        let try_node = Node::TryExpr(TryExpr {
            expr: Box::new(inner),
        });
        let mut info = MapCheckInfo::new();
        // node_key is address-based: register the type on the
        // inner node as it lives inside `try_node`.
        let inner_key = match &try_node {
            Node::TryExpr(t) => node_key(&t.expr),
            _ => unreachable!(),
        };
        info.expr_types.insert(inner_key, r_ty.clone());
        info.structs.insert("R".to_string(), decl);
        let mut driver = Driver::new(&info);
        let reg = driver.b().intern("r");
        driver
            .state
            .frame
            .push_local(Local::typed("r", reg, false, r_ty));
        let value = driver.gen_try(&try_node, match &try_node {
            Node::TryExpr(t) => t,
            _ => unreachable!(),
        });
        let value = value.expect("try struct arm must generate");
        let module = std::mem::take(&mut driver.state.builder).finish();
        let kinds: Vec<InstKind> = module.instructions.iter().map(|i| i.kind).collect();
        assert_eq!(
            kinds,
            vec![
                InstKind::Load,
                InstKind::Br,
                InstKind::Label,
                InstKind::Return,
                InstKind::Label,
                InstKind::Load,
            ]
        );
        // The success load's destination is the returned register.
        assert_eq!(module.instructions[5].operands[0], Operand::Reg(value));
    }
}

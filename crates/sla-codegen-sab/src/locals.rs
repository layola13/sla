//! E2 — locals and function-frame management, mirroring
//! `sab_codegen.zig` lines 2509–2619 in the reference tree
//! (`/content/sa_all`, HEAD 2cc12ea8): the `pushLocal` family,
//! `markObjectPtrReg`/`regIsObjectPtr`, `beginFunction` and
//! `finishFunctionBody`. (Decomposition doc lines 2391–2511 in
//! the older tree.)
//!
//! Split notes (B3/B5 ownership split, SAB-D09/D16):
//! - Zig's `recordReg` touches two places: the builder's
//!   recorded-reg ledger (`current_reg_ids`/`current_reg_seen`,
//!   B5's `ModuleBuilder`) and `released_regs` (the frame's
//!   per-register classification, B3). [`record_reg`] here does
//!   both halves.
//! - Zig's `beginFunction` also clears the builder ledger; in
//!   this port the driver clears it via
//!   `ModuleBuilder::begin_function(sig)` (it needs the sig
//!   index Zig's version does not), so [`begin_function`] here
//!   covers the frame half plus the program-side per-function
//!   sets Zig clears (multi-let names, string-slice inference
//!   sets, closure bindings/params, borrowed bindings,
//!   hashmap key slots).
//! - Zig's `finishFunctionBody` writes `finishFunctionRegs()`
//!   into the sig and flushes pending std deps. The sig write is
//!   `ModuleBuilder::finish_function` in this split (the driver
//!   calls it), and the flush is the F group's fragment machine
//!   consuming `pending_std_deps` — [`finish_function_body`]
//!   therefore returns the finished reg list and clears
//!   `in_function_body`; the flush stays a registered wiring
//!   need (see the E-group REPORT).
//! - `object_ptr_regs` (current-head addition, the
//!   DEF-SAB-BORROW-WRITE-NULL provenance set) is a payload set
//!   on the frame, added by this group to `state.rs` alongside
//!   `field_handle_view_regs` (DEF-KB-1, consumed by E7).

use sla_ast::Type;

use crate::error::Result;
use crate::ir::CapPrefix;
use crate::layout::user_defined_generic_inner;
use crate::release_meta::mark_non_owning_reg;
use crate::state::{CodegenState, RegState};
use crate::types::Local;

/// Zig `recordReg` (line 3190): a (re-)recorded register leaves
/// the released set, and enters the builder's reg ledger once.
pub fn record_reg(state: &mut CodegenState, reg: u32) {
    if state.frame.reg_state(reg) == RegState::Released {
        state.frame.reg_states.remove(&reg);
    }
    state.builder.record_reg(reg);
}

/// Zig `finishFunctionRegs` (line 3197): the recorded regs, in
/// record order (the builder's ledger already dedups, as Zig's
/// `current_reg_seen` does).
pub fn finish_function_regs(state: &CodegenState) -> Vec<u32> {
    state.builder.finish_function_regs()
}

/// Zig `pushLocal`.
pub fn push_local(state: &mut CodegenState, name: &str, reg: u32, is_param: bool) -> Result<()> {
    record_reg(state, reg);
    state.frame.push_local(Local::plain(name, reg, is_param));
    Ok(())
}

/// Zig `pushTypedLocal`.
pub fn push_typed_local(
    state: &mut CodegenState,
    name: &str,
    reg: u32,
    is_param: bool,
    ty: &Type,
) -> Result<()> {
    record_reg(state, reg);
    state.frame.push_local(Local::typed(name, reg, is_param, ty.clone()));
    Ok(())
}

/// Zig `markObjectPtrReg`: the register holds an object pointer
/// delivered directly (borrow-param provenance), so field/index
/// access through it skips the through-load.
pub fn mark_object_ptr_reg(state: &mut CodegenState, reg: u32) {
    state.frame.object_ptr_regs.insert(reg);
}

/// Zig `regIsObjectPtr`.
pub fn reg_is_object_ptr(state: &CodegenState, reg: u32) -> bool {
    state.frame.object_ptr_regs.contains(&reg)
}

/// Zig `senderInnerType` (lowering_rules.zig:4872): the `Sender`
/// generic's inner type, if this is a `Sender<T>`.
fn sender_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Sender")
}

/// Zig `pushParamLocal`: borrow-typed params are marked as
/// object-pointer regs (their ABI value *is* the pointee
/// address); raw-pointer params are deliberately NOT marked
/// (Zig comment, verbatim rationale). `Sender` handles are
/// shared ring-buffer aliases: retired via `move_`, never
/// freed — marked non-owning, matching the SA-text backend.
pub fn push_param_local(
    state: &mut CodegenState,
    name: &str,
    reg: u32,
    ty: &Type,
    cap: CapPrefix,
) -> Result<()> {
    record_reg(state, reg);
    if matches!(ty, Type::Borrow(_)) {
        mark_object_ptr_reg(state, reg);
    }
    let mut local = Local::typed(name, reg, true, ty.clone());
    local.param_cap = Some(cap);
    state.frame.push_local(local);
    if sender_inner_type(ty).is_some() {
        mark_non_owning_reg(state, reg);
    }
    Ok(())
}

/// Zig `pushRawParamLocal`: synthesized helpers (async poll
/// continuations) receive their state pointer directly; a
/// borrow-cap param reg is an object pointer.
pub fn push_raw_param_local(
    state: &mut CodegenState,
    name: &str,
    reg: u32,
    cap: CapPrefix,
) -> Result<()> {
    record_reg(state, reg);
    if cap == CapPrefix::Borrow {
        mark_object_ptr_reg(state, reg);
    }
    let mut local = Local::plain(name, reg, true);
    local.param_cap = Some(cap);
    state.frame.push_local(local);
    Ok(())
}

/// Zig `pushStackLocal`.
pub fn push_stack_local(state: &mut CodegenState, name: &str, reg: u32, ty: &Type) -> Result<()> {
    record_reg(state, reg);
    state.frame.push_local(Local::stack(name, reg, ty.clone()));
    Ok(())
}

/// Zig `pushStackAllocLocal`.
pub fn push_stack_alloc_local(state: &mut CodegenState, name: &str, reg: u32) -> Result<()> {
    record_reg(state, reg);
    let mut local = Local::plain(name, reg, false);
    local.is_stack_alloc = true;
    state.frame.push_local(local);
    Ok(())
}

/// Zig `pushStackAllocTypedLocal`.
pub fn push_stack_alloc_typed_local(
    state: &mut CodegenState,
    name: &str,
    reg: u32,
    ty: &Type,
) -> Result<()> {
    record_reg(state, reg);
    let mut local = Local::typed(name, reg, false, ty.clone());
    local.is_stack_alloc = true;
    state.frame.push_local(local);
    Ok(())
}

/// Zig `beginFunction`: per-function reset. The frame half is
/// `FrameState::begin_function` (B3); Zig's list additionally
/// clears these program-side per-function sets, done here.
pub fn begin_function(state: &mut CodegenState) {
    state.frame.begin_function();
    state.program.multi_let_bindings.clear();
    state.program.inferred_string_slice_locals.clear();
    state.program.explicit_ptr_string_locals.clear();
    state.program.closure_bindings.clear();
    state.program.closure_param_regs.clear();
    state.program.borrowed_bindings.clear();
    state.program.hashmap_key_slots.clear();
}

/// Zig `finishFunctionBody` (minus the F-group pending-dep
/// flush and the builder-side sig write — see module docs):
/// returns the finished reg list and leaves the function body.
pub fn finish_function_body(state: &mut CodegenState) -> Vec<u32> {
    let regs = finish_function_regs(state);
    state.frame.in_function_body = false;
    regs
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{Primitive, UserDefined};

    fn ud(name: &str, generics: Vec<Type>) -> Type {
        Type::UserDefined(UserDefined {
            name: name.to_string(),
            generics,
        })
    }

    #[test]
    fn push_family_records_regs_and_shapes() {
        let mut st = CodegenState::new();
        push_local(&mut st, "a", 10, false).unwrap();
        push_typed_local(&mut st, "b", 11, false, &Type::Primitive(Primitive::I32)).unwrap();
        push_stack_alloc_local(&mut st, "c", 12).unwrap();
        assert_eq!(st.frame.locals.len(), 3);
        assert_eq!(st.frame.local_reg("b"), Some(11));
        assert!(st.frame.locals[2].is_stack_alloc);
        assert_eq!(finish_function_regs(&st), vec![10, 11, 12]);
    }

    #[test]
    fn record_reg_unreleases() {
        let mut st = CodegenState::new();
        st.frame.reg_states.insert(7, RegState::Released);
        record_reg(&mut st, 7);
        assert_eq!(st.frame.reg_state(7), RegState::Owned);
        // Recording twice keeps one ledger entry (Zig seen-set).
        record_reg(&mut st, 7);
        assert_eq!(finish_function_regs(&st), vec![7]);
    }

    #[test]
    fn borrow_param_marks_object_ptr_sender_marks_non_owning() {
        let mut st = CodegenState::new();
        let borrow_ty = Type::Borrow(Box::new(Type::Primitive(Primitive::I32)));
        push_param_local(&mut st, "p", 20, &borrow_ty, CapPrefix::Borrow).unwrap();
        assert!(reg_is_object_ptr(&st, 20));
        let sender_ty = ud("Sender", vec![Type::Primitive(Primitive::I32)]);
        push_param_local(&mut st, "s", 21, &sender_ty, CapPrefix::ByValue).unwrap();
        assert!(!reg_is_object_ptr(&st, 21));
        assert_eq!(st.frame.reg_state(21), RegState::NonOwning);
    }

    #[test]
    fn raw_param_borrow_cap_marks_object_ptr() {
        let mut st = CodegenState::new();
        push_raw_param_local(&mut st, "st", 30, CapPrefix::Borrow).unwrap();
        assert!(reg_is_object_ptr(&st, 30));
        push_raw_param_local(&mut st, "mv", 31, CapPrefix::Move).unwrap();
        assert!(!reg_is_object_ptr(&st, 31));
    }

    #[test]
    fn begin_function_clears_frame_and_program_sets() {
        let mut st = CodegenState::new();
        push_local(&mut st, "a", 1, false).unwrap();
        st.program.multi_let_bindings.insert("a".to_string());
        st.program.borrowed_bindings.insert("a".to_string());
        begin_function(&mut st);
        assert!(st.frame.locals.is_empty());
        assert!(st.program.multi_let_bindings.is_empty());
        assert!(st.program.borrowed_bindings.is_empty());
        assert!(st.frame.in_function_body);
    }
}

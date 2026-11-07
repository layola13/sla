//! E5 — local release walkers and the deep-drop engine,
//! mirroring `sab_codegen.zig` lines 3219–4030 in the
//! reference tree (`/content/sa_all`, HEAD 2cc12ea8):
//! `releaseLocalsFrom` / `emitBranchReleaseLocalsFrom`,
//! `emitCleanupLocalDrop` / `emitBalanceReleaseLocal` /
//! `balanceBranchReleasedLocals`, the DEF deep-drop family
//! (`emitDeepValueDrop`, `emitSmartPointerDrop`,
//! `emitVecElementDrops`, `emitContainerElementDrops`,
//! `emitHashMapValueCellsFree`, `emitStructFieldDrops`,
//! `emitPlannedLocalDrop`), `releaseStackLocalValue`, the
//! checker-cleanup consumers (`releaseCleanupForStmt`,
//! `emitBranchCleanupForNode`, `genLoopJump`,
//! `genReleaseStmt`), and `releaseOpenLocals`.
//! (Decomposition doc lines 3057–3377 in the older tree.)
//!
//! Boundaries and registered notes:
//! - The drop *plan* itself is computed by the checker
//!   (`tc.planValueDrop` / `tc.dropTaintCoversAt`). The Rust
//!   CheckInfo surface has no such facts yet, so they arrive
//!   through [`DropFacts`]; [`NoDropFacts`] is loud on the
//!   plan query. `emit_planned_local_drop` maps a plan error
//!   to `false`, exactly Zig's `catch return false` — with
//!   the loud default that means "no deep plan available"
//!   surfaces as the shallow path only when the caller chose
//!   the unwired default, never silently inside a wired
//!   checker. [`DropPlan`] mirrors `lowering_rules.zig`'s
//!   union (lines 10231–10260) locally, as E4 does for its
//!   plan family (re-home when sla-lowering picks it up).
//! - `emitStructFieldDrops` reproduces Zig's `fieldLayout
//!   catch continue`: a field whose layout cannot be
//!   resolved is skipped, not errored. That is a Zig-side
//!   silent skip inside a drop path (SAB-D14 family);
//!   reproduced for byte fidelity and registered as
//!   SAB-D103 in the E-group REPORT — not "fixed" here.
//! - `genScopedBlock` is not here: block *generation* is the
//!   driver's (H group territory); its release half is
//!   [`DropEngine::release_locals_from`], which the driver
//!   already approximates by locals truncation (SAB-D19⑤).

use std::collections::HashSet;

use sla_ast::{Node, ReleaseStmt, Type};
use sla_lowering::{is_borrow_like_type, node_key, CheckInfo};

use crate::branch_state::restore_released;
use crate::copy_facts::CopyFacts;
use crate::error::{Error, Result};
use crate::ir::PrimType;
use crate::layout::{prim_type, storage_prim_type, LayoutAdapter};
use crate::release_drop::DropFacts;
use crate::release_emit::{
    emit_branch_move, emit_branch_release_with_metadata, emit_move,
    emit_release, mark_consumed, MacroFragmentEmitter,
};
use crate::release_prim::{emit_load, is_released};
use crate::release_meta::param_cleanup_action;
use crate::surface_query::{PendingStdDeps, StdDepEnsurer};
use crate::state::CodegenState;
use crate::types::{Local, LoopJumpKind};

/// Zig `ownedBufferDropType`: the declared type is exactly
/// the owned buffer type (`String`, `Vec`, `GrowVec`).
/// Borrow spellings never reach this predicate as
/// user-defined, so `&String`/`&Vec<T>` never qualify.
pub fn owned_buffer_drop_type(ty: Option<&Type>) -> bool {
    let ty = match ty {
        Some(ty) => ty,
        None => return false,
    };
    match ty {
        Type::UserDefined(ud) => matches!(ud.name.as_str(), "String" | "Vec" | "GrowVec"),
        _ => false,
    }
}

pub(crate) fn ensure_deps(state: &mut CodegenState, import_path: &str, deps: &[&str]) -> Result<()> {
    let owned: Vec<String> = deps.iter().map(|d| d.to_string()).collect();
    let mut ensurer = PendingStdDeps::new(
        &mut state.program.included_imports,
        &mut state.program.pending_std_deps,
    );
    ensurer.ensure_std_deps(import_path, &owned)
}

/// Zig `emitOwnedBufferFree`: `@sa_vec_free` (or
/// `@sa_growvec_free` for GrowVec, whose reservation is
/// released by munmap of the whole VA range) takes the
/// buffer out of the value struct and releases both the
/// buffer and the struct. A bare release/move on the struct
/// pointer would free only the struct allocation.
pub fn emit_owned_buffer_free(
    state: &mut CodegenState,
    reg: u32,
    ty: Option<&Type>,
) -> Result<()> {
    if is_released(state, reg) {
        return Ok(());
    }
    let name = state.builder.symbol_name(reg).to_string();
    if let Some(t) = ty {
        if matches!(t, Type::UserDefined(ud) if ud.name == "GrowVec") {
            ensure_deps(state, "sa_std/growvec.sa", &["sa_growvec_free"])?;
            state
                .builder
                .emit_call(None, &format!("@sa_growvec_free(^{name})"));
            crate::release_prim::mark_released(state, reg);
            return Ok(());
        }
    }
    ensure_deps(state, "sa_std/vec.sa", &["sa_vec_free"])?;
    state.builder.emit_call(None, &format!("@sa_vec_free(^{name})"));
    crate::release_prim::mark_released(state, reg);
    Ok(())
}

/// The E5 walker context: checker + drop facts + the layout
/// and copy-fact adapters the walkers consult.
pub struct DropEngine<'c> {
    pub check: &'c dyn CheckInfo,
    pub drops: &'c dyn DropFacts,
    pub(crate) layout: LayoutAdapter<'c>,
    pub(crate) copy: CopyFacts<'c>,
}

impl<'c> DropEngine<'c> {
    pub fn new(check: &'c dyn CheckInfo, drops: &'c dyn DropFacts) -> Self {
        DropEngine {
            check,
            drops,
            layout: LayoutAdapter::new(check),
            copy: CopyFacts::new(check),
        }
    }

    pub(crate) fn taint_covers(&self, state: &CodegenState, key: &str) -> bool {
        self.drops
            .drop_taint_covers_at(state.frame.current_drop_node, key)
    }

    /// Zig `releaseLocalsFrom`: cleanup-drop locals from the
    /// top of the stack down to `start`, newest first.
    pub fn release_locals_from(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        start: usize,
        except: Option<u32>,
    ) -> Result<()> {
        let mut i = state.frame.locals.len();
        while i > start {
            i -= 1;
            let local = state.frame.locals[i].clone();
            if except == Some(local.reg) {
                continue;
            }
            self.emit_cleanup_local_drop(state, frags, &local)?;
        }
        Ok(())
    }

    /// Zig `releaseOpenLocals`.
    pub fn release_open_locals(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        except: Option<u32>,
    ) -> Result<()> {
        self.release_locals_from(state, frags, 0, except)
    }

    /// Zig `emitBranchReleaseLocalsFrom`: the branch form —
    /// released regs are skipped, params follow their cleanup
    /// action, stack values and stack allocs take their
    /// branch paths, everything else goes through the
    /// metadata-aware branch release under a fresh `seen`.
    pub fn emit_branch_release_locals_from(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        start: usize,
        except: Option<u32>,
    ) -> Result<()> {
        let mut seen: HashSet<u32> = HashSet::new();
        let mut i = state.frame.locals.len();
        while i > start {
            i -= 1;
            let local = state.frame.locals[i].clone();
            if except == Some(local.reg) {
                continue;
            }
            if is_released(state, local.reg) {
                continue;
            }
            if local.is_param {
                let action = param_cleanup_action(self.check, &local)?;
                if action.skips() {
                    continue;
                }
                if action.marks_consumed() {
                    mark_consumed(state, local.reg);
                    continue;
                }
                if action.consumes() {
                    emit_branch_move(state, local.reg);
                    continue;
                }
            }
            if local.stack_ty.is_some() {
                self.release_stack_local_value(state, frags, &local)?;
                continue;
            }
            if local.is_stack_alloc {
                if state.frame.stack_alloc_emitted.contains(&local.reg) {
                    emit_branch_release_with_metadata(state, frags, local.reg, &mut seen)?;
                }
                continue;
            }
            emit_branch_release_with_metadata(state, frags, local.reg, &mut seen)?;
        }
        Ok(())
    }

    /// Zig `emitCleanupLocalDrop`: the sequential (non-branch)
    /// cleanup drop for one local.
    pub fn emit_cleanup_local_drop(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        local: &Local,
    ) -> Result<()> {
        if is_released(state, local.reg) {
            return Ok(());
        }
        if local.is_param {
            let action = param_cleanup_action(self.check, local)?;
            if action.skips() {
                return Ok(());
            }
            if action.marks_consumed() {
                mark_consumed(state, local.reg);
                return Ok(());
            }
            if action.consumes() {
                if self.emit_planned_local_drop(state, frags, local)? {
                    return Ok(());
                }
                if owned_buffer_drop_type(local.ty.as_ref()) {
                    return emit_owned_buffer_free(state, local.reg, local.ty.as_ref());
                }
                emit_move(state, local.reg);
                return Ok(());
            }
            if action.releases()
                && local.param_cap == Some(crate::ir::CapPrefix::Borrow)
            {
                // FORIN-GEN: a borrow parameter owns nothing it
                // points at; retire only the register.
                return emit_release(state, frags, local.reg);
            }
        }
        if self.emit_planned_local_drop(state, frags, local)? {
            return Ok(());
        }
        if local.stack_ty.is_some() {
            return self.release_stack_local_value(state, frags, local);
        }
        if local.is_stack_alloc {
            if state.frame.stack_alloc_emitted.contains(&local.reg) {
                emit_release(state, frags, local.reg)?;
            }
            return Ok(());
        }
        if owned_buffer_drop_type(local.ty.as_ref()) {
            return emit_owned_buffer_free(state, local.reg, local.ty.as_ref());
        }
        // DEF-HASHMAP-XTALK (A-form): a dropped HashMap's
        // entry value cells belong to this Map layer; free
        // them before the handle itself is released.
        if let Some(ty) = &local.ty {
            if matches!(ty, Type::UserDefined(ud) if ud.name == "HashMap") {
                self.emit_hashmap_value_cells_free(state, frags, local.reg)?;
            }
        }
        emit_release(state, frags, local.reg)
    }

    /// Zig `emitBalanceReleaseLocal`: branch-merge
    /// compensation for one local.
    pub fn emit_balance_release_local(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        local: &Local,
    ) -> Result<()> {
        if is_released(state, local.reg) {
            return Ok(());
        }
        let deep_balance = owned_buffer_drop_type(local.ty.as_ref())
            && local.stack_ty.is_none()
            && !local.is_stack_alloc;
        if local.is_param {
            let action = param_cleanup_action(self.check, local)?;
            if action.skips() {
                return Ok(());
            }
            if action.marks_consumed() {
                mark_consumed(state, local.reg);
                return Ok(());
            }
            if action.consumes() {
                if self.emit_planned_local_drop(state, frags, local)? {
                    return Ok(());
                }
                if deep_balance {
                    return emit_owned_buffer_free(state, local.reg, local.ty.as_ref());
                }
                emit_move(state, local.reg);
                return Ok(());
            }
            return Ok(());
        }
        if self.emit_planned_local_drop(state, frags, local)? {
            return Ok(());
        }
        if let Some(ty) = &local.ty {
            // Zig: `primType(ty) catch return` — a local whose
            // type has no prim mapping is silently skipped
            // here. SAB-D04's registered site is this exact
            // shape in the branch-balance path; reproduced
            // and cross-referenced in the E-group REPORT
            // (SAB-D102) — the port does not silently "fix" it.
            let abi_ty = match prim_type(ty) {
                Ok(t) => t,
                Err(_) => return Ok(()),
            };
            if abi_ty == PrimType::Ptr {
                let is_copy = self.copy.type_is_copy_value(ty);
                let is_borrow = is_borrow_like_type(ty);
                if is_copy || is_borrow {
                    return Ok(());
                }
                if deep_balance {
                    return emit_owned_buffer_free(state, local.reg, local.ty.as_ref());
                }
                emit_move(state, local.reg);
                crate::release_prim::mark_released(state, local.reg);
                return Ok(());
            }
        }
        if local.stack_ty.is_some() {
            return self.release_stack_local_value(state, frags, local);
        }
        if local.is_stack_alloc {
            if state.frame.stack_alloc_emitted.contains(&local.reg) {
                emit_release(state, frags, local.reg)?;
            }
            return Ok(());
        }
        if deep_balance {
            return emit_owned_buffer_free(state, local.reg, local.ty.as_ref());
        }
        emit_release(state, frags, local.reg)
    }

    /// Zig `balanceBranchReleasedLocals`: for each pre-branch
    /// local the target path released but this branch did
    /// not, emit the balance release.
    pub fn balance_branch_released_locals(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        branch_locals_len: usize,
        branch_released: &mut HashSet<u32>,
        target_released: &HashSet<u32>,
    ) -> Result<()> {
        let locals: Vec<Local> = state.frame.locals[..branch_locals_len].to_vec();
        for local in &locals {
            if !target_released.contains(&local.reg) || branch_released.contains(&local.reg) {
                continue;
            }
            let local_ty = match &local.ty {
                Some(ty) => ty,
                None => continue,
            };
            if is_borrow_like_type(local_ty) {
                continue;
            }
            self.emit_balance_release_local(state, frags, local)?;
            branch_released.insert(local.reg);
        }
        Ok(())
    }

    /// Zig `releaseStackLocalValue`.
    pub fn release_stack_local_value(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        local: &Local,
    ) -> Result<()> {
        let ty = match &local.stack_ty {
            Some(ty) => ty.clone(),
            None => return Ok(()),
        };
        if is_released(state, local.reg) {
            return Ok(());
        }
        if crate::release_emit::type_is_pointer_scalar_value(&ty) {
            mark_consumed(state, local.reg);
            return Ok(());
        }
        if state.frame.stack_alloc_emitted.contains(&local.reg) {
            if !self.copy.type_is_copy_value(&ty) {
                let value = state.builder.new_tmp();
                emit_load(state, value, local.reg, 0, storage_prim_type(&ty)?);
                if owned_buffer_drop_type(Some(&ty)) {
                    emit_owned_buffer_free(state, value, Some(&ty))?;
                } else {
                    emit_release(state, frags, value)?;
                }
            }
            mark_consumed(state, local.reg);
            return Ok(());
        }
        if self.copy.type_is_copy_value(&ty) {
            return emit_release(state, frags, local.reg);
        }
        let value = state.builder.new_tmp();
        emit_load(state, value, local.reg, 0, storage_prim_type(&ty)?);
        if owned_buffer_drop_type(Some(&ty)) {
            emit_owned_buffer_free(state, value, Some(&ty))?;
        } else {
            emit_release(state, frags, value)?;
        }
        emit_release(state, frags, local.reg)
    }

    /// Zig `releaseCleanupName`: cleanup-drop the newest
    /// local with this name; an unknown name interns and
    /// releases the register directly.
    pub fn release_cleanup_name(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        name: &str,
    ) -> Result<()> {
        for local in state.frame.locals.clone().iter().rev() {
            if local.name == name {
                let local = local.clone();
                return self.emit_cleanup_local_drop(state, frags, &local);
            }
        }
        let reg = state.builder.intern(name);
        if !is_released(state, reg) {
            emit_release(state, frags, reg)?;
        }
        Ok(())
    }

    /// Zig `releaseCleanupForStmt`: run the checker's
    /// per-statement cleanup list under the statement as
    /// the current drop node.
    pub fn release_cleanup_for_stmt(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        stmt: &Node,
    ) -> Result<()> {
        if let Some(list) = self.check.cleanups(stmt) {
            let names: Vec<String> = list.to_vec();
            let prev = state.frame.current_drop_node;
            state.frame.current_drop_node = Some(node_key(stmt));
            let mut result = Ok(());
            for name in &names {
                if let Err(e) = self.release_cleanup_name(state, frags, name) {
                    result = Err(e);
                    break;
                }
            }
            state.frame.current_drop_node = prev;
            return result;
        }
        Ok(())
    }

    /// Zig `emitBranchCleanupForNode`: the branch form of
    /// the cleanup consumer (macro-try cleanup names first,
    /// else the checker's list for the node).
    pub fn emit_branch_cleanup_for_node(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        node: &Node,
    ) -> Result<()> {
        let mut seen: HashSet<u32> = HashSet::new();
        if let Some(names) = state.frame.active_macro_try_cleanup.clone() {
            for name in &names {
                for local in state.frame.locals.iter().rev() {
                    if local.name == *name {
                        if local.stack_ty.is_none() && !local.is_stack_alloc {
                            let reg = local.reg;
                            emit_branch_release_with_metadata(state, frags, reg, &mut seen)?;
                        }
                        break;
                    }
                }
            }
            return Ok(());
        }
        if let Some(list) = self.check.cleanups(node) {
            let names: Vec<String> = list.to_vec();
            for name in &names {
                let mut reg: Option<u32> = None;
                for local in state.frame.locals.iter().rev() {
                    if local.name == *name {
                        if local.stack_ty.is_none() && !local.is_stack_alloc {
                            reg = Some(local.reg);
                        }
                        break;
                    }
                }
                if let Some(value_reg) = reg {
                    emit_branch_release_with_metadata(state, frags, value_reg, &mut seen)?;
                }
            }
        }
        Ok(())
    }

    /// Zig `genLoopJump`: statement cleanups, then jump to
    /// the innermost loop label of the requested kind; an
    /// empty label stack is loud `Unsupported`.
    pub fn gen_loop_jump(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        stmt: &Node,
        kind: LoopJumpKind,
    ) -> Result<()> {
        self.release_cleanup_for_stmt(state, frags, stmt)?;
        let labels = match kind {
            LoopJumpKind::Break => &state.frame.loop_break_labels,
            LoopJumpKind::Continue => &state.frame.loop_continue_labels,
        };
        let label = labels.last().ok_or_else(|| {
            Error::UnsupportedSabDirectFeature(
                "genLoopJump outside a loop [group E5 release_locals]".to_string(),
            )
        })?;
        let label = label.clone();
        state.builder.emit_jmp(&label);
        Ok(())
    }

    /// Zig `genReleaseStmt`: an explicit `release` statement.
    pub fn gen_release_stmt(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        rel: &ReleaseStmt,
    ) -> Result<()> {
        if let Some(local) = crate::captures::stack_local(state, &rel.var_name) {
            return self.release_stack_local_value(state, frags, &local);
        }
        let reg = match state.frame.local_reg(&rel.var_name) {
            Some(r) => r,
            None => state.builder.intern(&rel.var_name),
        };
        if let Some(ty) = crate::captures::local_type(state, &rel.var_name) {
            if self.copy.type_is_copy_value(&ty) {
                mark_consumed(state, reg);
                return Ok(());
            }
        }
        emit_release(state, frags, reg)
    }

    /// Zig `restoreReleased` re-export for driver call
    /// sites that pair it with the E5 walkers (the function
    /// itself lives in E3 `branch_state`).
    pub fn restore_released_snapshot(state: &mut CodegenState, snapshot: &HashSet<u32>) {
        restore_released(state, snapshot);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::InstKind;
    use crate::release_drop::{DropPlan, NoDropFacts};
    use crate::release_emit::NoMacroFragments;
    use sla_ast::{Primitive, UserDefined};
    use sla_lowering::MapCheckInfo;

    fn ud(name: &str, generics: Vec<Type>) -> Type {
        Type::UserDefined(UserDefined {
            name: name.to_string(),
            generics,
        })
    }

    struct FixedDrops(DropPlan);
    impl DropFacts for FixedDrops {
        fn plan_value_drop(&self, _ty: &Type) -> Result<DropPlan> {
            Ok(self.0.clone())
        }
        fn drop_taint_covers_at(&self, _n: Option<usize>, _k: &str) -> bool {
            false
        }
    }

    #[test]
    fn owned_buffer_type_names() {
        assert!(owned_buffer_drop_type(Some(&ud("Vec", vec![]))));
        assert!(owned_buffer_drop_type(Some(&ud("String", vec![]))));
        assert!(owned_buffer_drop_type(Some(&ud("GrowVec", vec![]))));
        assert!(!owned_buffer_drop_type(Some(&ud("HashMap", vec![]))));
        assert!(!owned_buffer_drop_type(Some(&Type::Primitive(Primitive::I32))));
        assert!(!owned_buffer_drop_type(None));
    }

    #[test]
    fn owned_buffer_free_emits_vec_free_call_and_marks() {
        let mut st = CodegenState::new();
        let reg = st.builder.intern("buf");
        emit_owned_buffer_free(&mut st, reg, Some(&ud("Vec", vec![]))).unwrap();
        assert!(is_released(&st, reg));
        // The dep was queued in request order (SAB-D26 form).
        assert_eq!(st.program.pending_std_deps.len(), 1);
        assert_eq!(st.program.pending_std_deps[0].dep, "sa_vec_free");
        let module = st.builder.finish();
        assert_eq!(module.instructions.len(), 1);
        assert_eq!(module.instructions[0].kind, InstKind::Call);
    }

    #[test]
    fn cleanup_drop_plain_local_releases() {
        let check = MapCheckInfo::new();
        let drops = NoDropFacts;
        let mut engine = DropEngine::new(&check, &drops);
        let mut st = CodegenState::new();
        let reg = st.builder.intern("x");
        crate::locals::push_typed_local(
            &mut st,
            "x",
            reg,
            false,
            &Type::Primitive(Primitive::I32),
        )
        .unwrap();
        let mut frags = NoMacroFragments;
        engine.release_open_locals(&mut st, &mut frags, None).unwrap();
        assert!(is_released(&st, reg));
    }

    #[test]
    fn deep_vec_drop_walks_elements_then_frees() {
        let check = MapCheckInfo::new();
        let drops = FixedDrops(DropPlan::DeepVec(Box::new(DropPlan::Buffer)));
        let mut engine = DropEngine::new(&check, &drops);
        let mut st = CodegenState::new();
        let vec_ty = ud("Vec", vec![ud("String", vec![])]);
        let reg = st.builder.intern("v");
        let mut frags = NoMacroFragments;
        engine
            .emit_deep_value_drop(&mut st, &mut frags, reg, &vec_ty, &drops.0, "v", false, true)
            .unwrap();
        assert!(is_released(&st, reg));
        let kinds: Vec<InstKind> = st
            .builder
            .finish()
            .instructions
            .iter()
            .map(|i| i.kind)
            .collect();
        // Walker prologue loads ptr+len, and the sequence ends
        // with the @sa_vec_free call for the container.
        assert!(kinds.contains(&InstKind::Load));
        assert_eq!(*kinds.last().unwrap(), InstKind::Call);
    }

    #[test]
    fn planned_drop_with_no_facts_falls_back() {
        let check = MapCheckInfo::new();
        let drops = NoDropFacts;
        let mut engine = DropEngine::new(&check, &drops);
        let mut st = CodegenState::new();
        let local = Local::typed("s", 3, false, ud("String", vec![]));
        let mut frags = NoMacroFragments;
        // NoDropFacts errors -> Zig's `catch return false`.
        assert!(!engine.emit_planned_local_drop(&mut st, &mut frags, &local).unwrap());
    }
}

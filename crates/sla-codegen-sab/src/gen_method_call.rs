//! D surface (surface_call) — the bare-call *method* probes of
//! Zig `genCallInner` (sab_codegen.zig, current head), ported
//! for the S2' method-wall slice: `genVecLenCall` (15801),
//! `genGrowVecLenCall` (15782), `genVecPushCall` (17094),
//! `genGrowVecPushCall` (17163), `genVecRemoveCall` (17228),
//! `genVecDequePushBackCall` (22639),
//! `genResultOptionQueryCall` (17564), `genOptionUnwrapCall`
//! (22576), `genUnwrapOrCall` (22721),
//! `genOptionUnwrapOrDefaultCall` (22760),
//! `genAtomicMethodCall` (17353; load/store/fetch_add arms),
//! and the bare-call branches of `genStdSurfaceCall` (21345:
//! function / fallible_method / method rules over the D1/D4
//! std-surface machinery, behind the DEF-SAB-SURFACE-GET-SHADOW
//! known-callee guard).
//!
//! Probe order mirrors Zig's dispatch chain restricted to the
//! ported probes; wiring sits in `gen_call` after the bound-
//! closure probe and before the plain static-call tail, the
//! same relative position the chain occupies in Zig. Every
//! probe self-guards on the call shape and receiver type;
//! `Ok(None)` keeps the caller's loud stop. Arms whose support
//! machinery is not yet ported stay loud, never approximated:
//! Vec pop, map clear/remove/keys/values,
//! VecDeque pop_front / rotate, atomic compare_exchange,
//! RefCell borrow, smart-pointer clone/count, and the raw_ptr
//! `len` arm (string materialization plan) are NOT ported here.

use sla_ast::{CallExpr, Node, Primitive, Type};

use crate::copy_facts::CopyFacts;
use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::fragment::FragmentMachine;
use crate::fragment_imports::{MachineStdDeps, MachineSurfaceEmitter};
use crate::ir::{CapPrefix, InstKind, Instruction, Operand, PrimType};
use crate::layout::{
    element_slot_size, prim_type, struct_size, type_base_name, user_defined_generic_inner,
    vec_element_slot_size, vec_element_type, LayoutAdapter,
};
use crate::release_drop::{GROWVEC_ABI_LEN_OFFSET, VEC_ABI_LEN_OFFSET};
use crate::state::RegState;
use crate::surface_query::{
    emit_std_surface_rule, find_std_surface_rule, std_surface_rule_has_arg, PendingStdDeps,
    StdDepEnsurer, StdSurfaceValues,
};
use crate::types::{StdSurfaceArgKind, StdSurfaceRule, StdSurfaceRuleKind};

/// Zig `SliceAbi.len_offset` (lowering_rules.zig:3372).
const SLICE_ABI_LEN_OFFSET: usize = 8;

impl<'c> Driver<'c> {
    /// The ported bare-call method probe chain, in Zig
    /// `genCallInner` order restricted to the ported probes.
    /// `Ok(None)` = no ported probe claimed the call (the
    /// caller keeps the loud stop / plain static call).
    pub(crate) fn gen_method_call(
        &mut self,
        expr: &Node,
        call: &CallExpr,
    ) -> Result<Option<u32>> {
        if let Some(reg) = self.gen_vec_len_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_grow_vec_len_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_vec_push_call(call, false)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_vec_push_call(call, true)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_vec_remove_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_vec_deque_push_back_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_result_option_query_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_option_unwrap_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_unwrap_or_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_option_unwrap_or_default_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_atomic_method_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_map_method_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_std_surface_bare_call(expr, call)? {
            return Ok(Some(reg));
        }
        Ok(None)
    }

    // -- shared emission helpers --------------------------------
    // (Per-module copies of the D-surface wiring, as in
    // gen_surface_call.rs / gen_print.rs: the inherent helpers
    // are module-private by design.)

    /// Zig `ensureStdDeps` through the F machine.
    pub(crate) fn ensure_method_std_deps(&mut self, import_path: &str, deps: &[&str]) -> Result<()> {
        let owned: Vec<String> = deps.iter().map(|dep| dep.to_string()).collect();
        let mut ensurer = MachineStdDeps {
            machine: FragmentMachine::new(&mut self.state),
            flattener: &mut *self.flattener,
        };
        ensurer.ensure_std_deps(import_path, &owned)
    }

    /// Zig `emitCallBody` over the fragment machine.
    pub(crate) fn emit_method_call_body(&mut self, dst: Option<u32>, body: String) -> Result<()> {
        FragmentMachine::new(&mut self.state).emit_call_body(dst, body)
    }

    /// Zig `emitStdMacroFragment` (plain form).
    pub(crate) fn emit_method_macro_fragment(
        &mut self,
        import_path: &str,
        macro_name: &str,
        args: &[String],
    ) -> Result<()> {
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
            &mut *self.flattener,
            import_path,
            macro_name,
            args,
        )
    }

    /// Zig `emitStdMacroFragmentWithLiteralArgs`.
    pub(crate) fn emit_method_macro_fragment_literal(
        &mut self,
        import_path: &str,
        macro_name: &str,
        args: &[String],
        literal_args: &[bool],
    ) -> Result<()> {
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment_with_literal_args(
            &mut *self.flattener,
            import_path,
            macro_name,
            args,
            literal_args,
        )
    }

    /// Zig `emitStdSurfaceRule` (D4) with the production
    /// emitter; same queue/symbol plumbing as
    /// gen_surface_call's `emit_surface_rule`.
    pub(crate) fn emit_method_rule(
        &mut self,
        rule: &StdSurfaceRule,
        values: &StdSurfaceValues,
    ) -> Result<()> {
        let symbols: Vec<String> = self.b().symbols().to_vec();
        let mut included = std::mem::take(&mut self.state.program.included_imports);
        let mut pending = std::mem::take(&mut self.state.program.pending_std_deps);
        let result = {
            let mut emitter = MachineSurfaceEmitter {
                machine: FragmentMachine::new(&mut self.state),
                flattener: &mut *self.flattener,
            };
            let mut ensurer = PendingStdDeps::new(&mut included, &mut pending);
            emit_std_surface_rule(&mut emitter, &mut ensurer, rule, values, &symbols)
        };
        self.state.program.included_imports = included;
        self.state.program.pending_std_deps = pending;
        result
    }

    /// Zig `releaseNonLocalTemps` for one register / the
    /// `if (!isLocalReg(reg)) emitRelease(reg)` idiom.
    pub(crate) fn release_non_local(&mut self, reg: u32) -> Result<()> {
        if !self.is_local_reg(reg) {
            self.emit_release(reg)?;
        }
        Ok(())
    }

    /// Zig `emitBorrowReg` (14108): a borrow instruction with
    /// a text mode operand and the borrow cap prefix; the
    /// result register joins `borrow_result_regs` (the port's
    /// `RegState::BorrowResult`).
    pub(crate) fn emit_borrow_reg(&mut self, dst: u32, source: u32, mode: &str) {
        self.b().record_reg(dst);
        self.state
            .frame
            .reg_states
            .insert(dst, RegState::BorrowResult);
        let mut inst = Instruction::new(
            InstKind::Borrow,
            0,
            self.b().instructions_len() as u32,
        );
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::Reg(source);
        inst.operands[2] = Operand::Text(mode.to_string());
        inst.operands[3] = Operand::CapPrefix(CapPrefix::Borrow);
        self.b().append_instruction(inst);
    }

    // -- len probes ----------------------------------------------

    /// Zig `genVecLenCall` (15801): array / Slice / HashMap /
    /// BTreeMap arms plus the Vec/String fall-through arm
    /// (borrow-scoped length load at VecAbi.len_offset). The
    /// raw_ptr arm (CSTR_LEN / slice-field plan over the
    /// string materialization table) is not ported: raw_ptr
    /// receivers return `Ok(None)` and stay loud downstream.
    /// The field-projection owner receiver (`genVecOwnerReceiver`)
    /// is plain `gen_expr` here — the Rust driver's `gen_expr`
    /// has no FieldExpr arm yet, so field receivers stop loudly
    /// inside `gen_expr`, exactly the reachable-shape parity.
    fn gen_vec_len_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if !is_bare_len_unary_call(call) {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        if let Type::Array(arr) = &receiver_ty {
            let dst = self.b().new_tmp();
            self.b().emit_assign_imm(dst, arr.len as i64);
            return Ok(Some(dst));
        }
        if user_defined_generic_inner(&receiver_ty, "Slice").is_some() {
            let base_reg = self.gen_expr(&call.args[0])?;
            let dst = self.b().new_tmp();
            self.emit_load(dst, base_reg, SLICE_ABI_LEN_OFFSET, PrimType::U64);
            self.release_non_local(base_reg)?;
            return Ok(Some(dst));
        }
        if matches!(receiver_ty, Type::Primitive(Primitive::RawPtr)) {
            return Ok(None);
        }
        if vec_element_type(&receiver_ty).is_none() && !is_string_type(&receiver_ty) {
            // HashMap length mirrors the SA-text emitter
            // (`EXPAND MAP_LEN`).
            if map_types(&receiver_ty, "HashMap").is_some() {
                self.ensure_method_std_deps("sa_std/hashmap.sa", &["sa_map_len"])?;
                let map_reg = self.gen_expr(&call.args[0])?;
                let dst = self.b().new_tmp();
                self.b().record_reg(dst);
                let dst_name = self.b().symbol_name(dst).to_string();
                let map_name = self.b().symbol_name(map_reg).to_string();
                self.emit_method_macro_fragment(
                    "sa_std/hashmap.sa",
                    "MAP_LEN",
                    &[dst_name, map_name],
                )?;
                self.release_non_local(map_reg)?;
                return Ok(Some(dst));
            }
            // BTreeMap length mirrors the SA-text emitter
            // (`EXPAND BTREE_MAP_LEN`).
            if map_types(&receiver_ty, "BTreeMap").is_some() {
                self.ensure_method_std_deps("sa_std/btree_map.sa", &["sa_btree_map_len"])?;
                let map_reg = self.gen_expr(&call.args[0])?;
                let dst = self.b().new_tmp();
                self.b().record_reg(dst);
                let dst_name = self.b().symbol_name(dst).to_string();
                let map_name = self.b().symbol_name(map_reg).to_string();
                self.emit_method_macro_fragment(
                    "sa_std/btree_map.sa",
                    "BTREE_MAP_LEN",
                    &[dst_name, map_name],
                )?;
                self.release_non_local(map_reg)?;
                return Ok(Some(dst));
            }
            return Ok(None);
        }
        self.ensure_method_std_deps("sa_std/vec.sa", &["sa_vec_len"])?;
        let receiver_reg = self.gen_expr(&call.args[0])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        // SA-text parity: scope the len load through an
        // explicit borrow of the Vec value (see the Zig
        // comment at 15801: loop-head merge trap 1015).
        let len_view = self.b().new_tmp();
        self.emit_borrow_reg(len_view, receiver_reg, "read");
        self.emit_load(dst, len_view, VEC_ABI_LEN_OFFSET, PrimType::U64);
        self.emit_release(len_view)?;
        self.release_non_local(receiver_reg)?;
        Ok(Some(dst))
    }

    /// Zig `genGrowVecLenCall` (15782): borrow-scoped length
    /// load at GrowVecAbi.len_offset (+8; Vec's is +16).
    fn gen_grow_vec_len_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if !is_bare_len_unary_call(call) {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        if user_defined_generic_inner(&receiver_ty, "GrowVec").is_none() {
            return Ok(None);
        }
        self.ensure_method_std_deps("sa_std/growvec.sa", &["sa_growvec_len"])?;
        let receiver_reg = self.gen_expr(&call.args[0])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let len_view = self.b().new_tmp();
        self.emit_borrow_reg(len_view, receiver_reg, "read");
        self.emit_load(dst, len_view, GROWVEC_ABI_LEN_OFFSET, PrimType::U64);
        self.emit_release(len_view)?;
        self.release_non_local(receiver_reg)?;
        Ok(Some(dst))
    }

    // -- push probes ---------------------------------------------

    /// Zig `genVecPushCall` (17094) / `genGrowVecPushCall`
    /// (17163), one body parameterized by the container (the
    /// two Zig probes are exact mirrors). Guards: bare binary
    /// `push`, element type present, element slot size 8.
    /// Inline-POD elements push their packed word; a local
    /// shallow-copyable heap-handle value is materialized
    /// through `pushArgCopyMaterialization`; heap-handle
    /// elements are consumed by the store (DEF-ZRANGE +
    /// DEF-SAB-BORROW-RESIDUAL); other non-local values are
    /// released. Returns a 0 sentinel.
    fn gen_vec_push_call(&mut self, call: &CallExpr, grow: bool) -> Result<Option<u32>> {
        if !is_bare_push_binary_call(call) {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let elem_ty = if grow {
            user_defined_generic_inner(&receiver_ty, "GrowVec")
        } else {
            vec_element_type(&receiver_ty)
        };
        let elem_ty = match elem_ty {
            Some(ty) => ty,
            None => return Ok(None),
        };
        if vec_element_slot_size(&elem_ty) != 8 {
            return Ok(None);
        }
        let (import_path, fn_name) = if grow {
            ("sa_std/growvec.sa", "sa_growvec_push")
        } else {
            ("sa_std/vec.sa", "sa_vec_push")
        };
        self.ensure_method_std_deps(import_path, &[fn_name, "sa_mem_copy"])?;
        let receiver_reg = self.gen_expr(&call.args[0])?;
        let value_reg = self.gen_expr(&call.args[1])?;
        // CLASSB segment 3: inline-POD elements push their
        // packed record image, not the record pointer.
        if self.inline_pod_element(&elem_ty) {
            let word_reg = self.b().new_tmp();
            self.b().record_reg(word_reg);
            self.emit_load(word_reg, value_reg, 0, PrimType::U64);
            let receiver_name = self.b().symbol_name(receiver_reg).to_string();
            let word_name = self.b().symbol_name(word_reg).to_string();
            let body = format!("@{fn_name}(^{receiver_name}, {word_name}, 8)");
            self.emit_method_call_body(Some(receiver_reg), body)?;
            self.release_non_local(receiver_reg)?;
            self.emit_release(word_reg)?;
            self.release_non_local(value_reg)?;
            let sentinel = self.b().new_tmp();
            self.b().record_reg(sentinel);
            self.b().emit_assign_imm(sentinel, 0);
            return Ok(Some(sentinel));
        }
        let copy_reg = self.push_arg_copy_materialization(value_reg, &elem_ty)?;
        let stored_reg = copy_reg.unwrap_or(value_reg);
        let receiver_name = self.b().symbol_name(receiver_reg).to_string();
        let stored_name = self.b().symbol_name(stored_reg).to_string();
        let body = format!("@{fn_name}(^{receiver_name}, {stored_name}, 8)");
        self.emit_method_call_body(Some(receiver_reg), body)?;
        self.release_non_local(receiver_reg)?;
        if let Some(cr) = copy_reg {
            self.emit_consumed_marker(cr);
            self.mark_consumed(cr);
        } else if crate::agg_emit::is_heap_handle_type(&elem_ty) {
            // DEF-ZRANGE: the push stores the element's handle
            // word itself, so the container owns the cell from
            // here on; a non-local source gets the consumed
            // marker, and a local source is marked consumed so
            // its exit drop skips it (DEF-SAB-BORROW-RESIDUAL).
            if !self.is_local_reg(value_reg) {
                self.emit_consumed_marker(value_reg);
            }
            self.mark_consumed(value_reg);
        } else {
            self.release_non_local(value_reg)?;
        }
        let sentinel = self.b().new_tmp();
        self.b().record_reg(sentinel);
        self.b().emit_assign_imm(sentinel, 0);
        Ok(Some(sentinel))
    }

    /// Zig `pushArgCopyMaterialization` (17083): only a local
    /// heap-handle value whose element type is shallow-copyable
    /// (and does not transfer ownership outright) is copied.
    fn push_arg_copy_materialization(
        &mut self,
        value_reg: u32,
        elem_ty: &Type,
    ) -> Result<Option<u32>> {
        if !self.is_local_reg(value_reg) {
            return Ok(None);
        }
        if !crate::agg_emit::is_heap_handle_type(elem_ty) {
            return Ok(None);
        }
        let (copy_like, shallow) = {
            let mut facts = CopyFacts::new(self.check);
            let copy_like = facts.type_is_copy_value(elem_ty)
                || facts.type_is_shallow_copy_call_arg_value(elem_ty, 0);
            let shallow = facts.type_is_shallow_copy_call_arg_value(elem_ty, 0);
            (copy_like, shallow)
        };
        if vec_element_push_transfers_ownership(elem_ty, copy_like) {
            return Ok(None);
        }
        if !shallow {
            return Ok(None);
        }
        let decl = {
            let mut layouts = LayoutAdapter::new(self.check);
            layouts.struct_decl_for_type(elem_ty)
        };
        let decl = match decl {
            Some(decl) => decl,
            None => return Ok(None),
        };
        if decl.is_opaque || decl.is_union {
            return Ok(None);
        }
        Ok(Some(self.gen_shallow_copy_call_arg_value(value_reg, elem_ty)?))
    }

    /// Zig `inlinePodElement` (2176) over
    /// `structDeclIsInlinePod` (lowering_rules.zig:5582).
    pub(crate) fn inline_pod_element(&mut self, elem_ty: &Type) -> bool {
        let Type::UserDefined(ud) = elem_ty else {
            return false;
        };
        let decl = match self.check.struct_decl(&ud.name) {
            Some(decl) => decl,
            None => return false,
        };
        if decl.is_opaque || decl.is_union {
            return false;
        }
        if !decl.generics.is_empty() || decl.fields.is_empty() {
            return false;
        }
        if decl.fields.iter().any(|field| !matches!(*field.ty, Type::Primitive(_))) {
            return false;
        }
        struct_size(decl) == 8
    }

    /// Zig `materializeInlinePodRecord` (2185): a fresh heap
    /// record holding the packed slot word.
    pub(crate) fn materialize_inline_pod_record(&mut self, word_reg: u32, elem_ty: &Type) -> Result<u32> {
        let Type::UserDefined(ud) = elem_ty else {
            return Err(unsupported(
                "inline-POD record materialization of a non-struct element",
                "D surface (surface_call)",
                "2185",
            ));
        };
        let decl = self
            .check
            .struct_decl(&ud.name)
            .cloned()
            .ok_or_else(|| Error::MissingType("inline-POD element struct decl".to_string()))?;
        let rec = self.b().new_tmp();
        self.emit_alloc(rec, struct_size(&decl));
        self.emit_store(rec, 0, word_reg, PrimType::U64);
        Ok(rec)
    }

    // -- remove probe ----------------------------------------------

    /// Zig `genVecRemoveCall` (17228): `VEC_REMOVE` fragment,
    /// miss branch panics 86, hit branch casts the raw u64
    /// back to the element prim (inline-POD elements
    /// materialize a record instead). Other receivers/widths
    /// stay on the previous path (`Ok(None)`); the meta
    /// fallible-method rule for `Vec remove` sits behind this
    /// probe in the chain, as in Zig.
    fn gen_vec_remove_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.func_name != "remove" || call.associated_target.is_some() || call.args.len() != 2 {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let elem_ty = match vec_element_type(&receiver_ty) {
            Some(ty) => ty,
            None => return Ok(None),
        };
        if vec_element_slot_size(&elem_ty) != 8 {
            return Ok(None);
        }
        let pod_inline = self.inline_pod_element(&elem_ty);
        let elem_prim = if pod_inline {
            PrimType::U64
        } else {
            match prim_type(&elem_ty) {
                Ok(prim) => prim,
                Err(_) => return Ok(None),
            }
        };
        self.ensure_method_std_deps("sa_std/vec.sa", &["sa_vec_try_remove"])?;
        let receiver_reg = self.gen_expr(&call.args[0])?;
        let index_reg = self.gen_expr(&call.args[1])?;
        let ok_reg = self.b().new_tmp();
        let raw_reg = self.b().new_tmp();
        self.b().record_reg(ok_reg);
        self.b().record_reg(raw_reg);
        let ok_name = self.b().symbol_name(ok_reg).to_string();
        let raw_name = self.b().symbol_name(raw_reg).to_string();
        let receiver_name = self.b().symbol_name(receiver_reg).to_string();
        let index_name = self.b().symbol_name(index_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/vec.sa",
            "VEC_REMOVE",
            &[
                ok_name,
                raw_name,
                receiver_name,
                index_name,
                vec_element_slot_size(&elem_ty).to_string(),
            ],
        )?;
        let hit_label = self.b().new_label("L_VEC_REMOVE_HIT");
        let miss_label = self.b().new_label("L_VEC_REMOVE_MISS");
        self.b().emit_branch(ok_reg, &hit_label, &miss_label);

        self.b().emit_label(&miss_label);
        crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
        if !self.is_local_reg(raw_reg) {
            crate::release_emit::emit_branch_release(&mut self.state, raw_reg);
        }
        if !self.is_local_reg(index_reg) {
            crate::release_emit::emit_branch_release(&mut self.state, index_reg);
        }
        self.b().emit_panic_code(86);

        self.b().emit_label(&hit_label);
        crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
        self.release_non_local(index_reg)?;
        if pod_inline {
            let rec = self.materialize_inline_pod_record(raw_reg, &elem_ty)?;
            self.release_non_local(raw_reg)?;
            return Ok(Some(rec));
        }
        if elem_prim == PrimType::U64 {
            return Ok(Some(raw_reg));
        }
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let op = crate::gen_struct::op_kind_for_cast(PrimType::U64, elem_prim)?;
        self.b().emit_op(
            dst,
            op,
            Operand::Reg(raw_reg),
            Operand::Ty(elem_prim as u32),
        );
        self.release_non_local(raw_reg)?;
        Ok(Some(dst))
    }

}

// -- pure call-shape / type facts (lowering_rules) -------------

/// Zig `isBareLenUnaryCall` (lowering_rules.zig): a bare
/// `len(receiver)` call.
pub(crate) fn is_bare_len_unary_call(call: &CallExpr) -> bool {
    call.associated_target.is_none() && call.func_name == "len" && call.args.len() == 1
}

/// Zig `isBarePushBinaryCall` (lowering_rules.zig:4384): a
/// bare `push(receiver, value)` call.
pub(crate) fn is_bare_push_binary_call(call: &CallExpr) -> bool {
    call.associated_target.is_none() && call.func_name == "push" && call.args.len() == 2
}

/// Zig `isUserDefinedNamed` (lowering_rules.zig): peel
/// borrow/pointer layers, then an exact name match on a
/// non-generic user-defined type.
pub(crate) fn is_user_defined_named(ty: &Type, name: &str) -> bool {
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::UserDefined(ud) => ud.name == name && ud.generics.is_empty(),
        _ => false,
    }
}

/// Zig `isStringType` (lowering_rules.zig:4935).
pub(crate) fn is_string_type(ty: &Type) -> bool {
    is_user_defined_named(ty, "String")
}

/// The key/value types of a two-generic map type
/// (`hashMapTypes` / `btreeMapTypes`, lowering_rules.zig:4768).
pub(crate) fn map_types(ty: &Type, name: &str) -> Option<(Type, Type)> {
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::UserDefined(ud) if ud.name == name && ud.generics.len() == 2 => {
            Some((ud.generics[0].clone(), ud.generics[1].clone()))
        }
        _ => None,
    }
}

/// Zig `vecElementPushTransfersOwnership`
/// (lowering_rules.zig:6830).
pub(crate) fn vec_element_push_transfers_ownership(elem_ty: &Type, elem_is_copy: bool) -> bool {
    !elem_is_copy && !sla_lowering::is_borrow_like_type(elem_ty)
}

/// Zig `atomicIntMacroPrefix` (lowering_rules.zig:4658).
pub(crate) fn atomic_int_macro_prefix(ty: &Type) -> Option<&'static str> {
    if is_user_defined_named(ty, "AtomicI32") {
        return Some("ATOMIC_I32");
    }
    if is_user_defined_named(ty, "AtomicUsize") {
        return Some("ATOMIC_USIZE");
    }
    None
}

/// Zig `atomicOrderingArgToken` over `atomicOrderingToken`
/// (lowering_rules.zig:4971): the argument must be an
/// `Ordering::X` identifier; anything else is the loud stop.
pub(crate) fn atomic_ordering_arg_token(arg: &Node) -> Result<&'static str> {
    let Node::Identifier(name) = arg else {
        return Err(unsupported(
            "atomic ordering argument is not an identifier",
            "D surface (surface_call)",
            "17353-17360",
        ));
    };
    match name.as_str() {
        "Ordering::SeqCst" => Ok("seq_cst"),
        "Ordering::Acquire" => Ok("acquire"),
        "Ordering::Release" => Ok("release"),
        "Ordering::Relaxed" => Ok("relaxed"),
        "Ordering::AcqRel" => Ok("acq_rel"),
        _ => Err(unsupported(
            &format!("unknown atomic ordering `{name}`"),
            "D surface (surface_call)",
            "17353-17360",
        )),
    }
}


#[cfg(test)]
mod tests;

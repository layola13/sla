//! B3 — the `Codegen` state, mirroring `sab_codegen.zig` lines
//! 294–534 (field block + `init`/`deinit`).
//!
//! The Zig original is one struct with ~70 fields, a dozen of them
//! parallel hash sets over register ids whose legal combinations
//! exist only by convention (registered as SAB-D09). This port
//! splits the fields into ownership-clear sub-states:
//!
//! - [`ProgramState`] — whole-program registries (C/D/F groups):
//!   surface rules, imported std modules, macro templates, global
//!   consts, escaped closures.
//! - [`FrameState`] — per-function state (E group): locals, loop
//!   label stacks, and the per-register ownership classification.
//! - [`CodegenState`] — the two above plus the emission trunk
//!   ([`ModuleBuilder`], which owns the B5 fields: symbols /
//!   symbol_ids / const_decls / instructions / function_sigs and
//!   the tmp/label/string counters) and the scalar bookkeeping
//!   fields that belong to neither.
//!
//! SAB-D09 mapping — every Zig register set lands in exactly one
//! place here:
//! - `non_owning_regs` / `borrow_result_regs` / `stack_alloc_regs`
//!   / `released_regs` -> [`RegState`] variants on the frame (a
//!   register has exactly one classification at a time);
//! - `refcell_borrow_values` / `mutex_guard_values` /
//!   `rwlock_guard_values` / `borrow_address_temps` /
//!   `result_slot_refcell_handles` / `result_slot_refcell_slots` /
//!   `future_state_vtables` / `future_readiness` -> payload maps on
//!   the frame (they carry data, not just membership);
//! - `stack_alloc_emitted` -> a plain set on the frame (it records
//!   an emission fact, not an ownership class);
//! - `current_reg_ids` / `current_reg_seen` -> `recorded_regs`
//!   bookkeeping inside `ModuleBuilder` (B5, phase 1).
//!
//! Hash-order discipline: every map/set here is lookup-only. The
//! collections that decide emission order are `Vec`s
//! (`std_surface_rules`, `pending_std_deps`, `std_import_modules`,
//! `std_macro_templates`, `escaped_closure_entries`, `locals`,
//! loop label stacks), in insertion order, as in Zig.

use std::collections::{HashMap, HashSet};

use sla_ast::{ClosureLiteral, Node, Type};

use crate::emit::ModuleBuilder;
use crate::ir::FunctionSig;
use crate::types::*;

/// Per-register ownership classification (SAB-D09): the mutually
/// exclusive union of the Zig frame's register-membership sets.
/// `Owned` is the default for a freshly recorded register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegState {
    Owned,
    /// Zig `non_owning_regs`: a view that must never be released.
    NonOwning,
    /// Zig `borrow_result_regs`.
    BorrowResult,
    /// Zig `stack_alloc_regs`: dies with its scope, never released
    /// or moved explicitly.
    StackAlloc,
    /// Zig membership in `released_regs`.
    Released,
    /// Consumed by an assign/move (Zig `markConsumed` bookkeeping).
    Consumed,
}

/// Whole-program registries (Zig field block, program-scope half).
#[derive(Debug, Default)]
pub struct ProgramState {
    /// Zig `fn_ptr_vtables`: vtable names already emitted.
    pub fn_ptr_vtables: HashSet<String>,
    /// Zig `closure_bindings`: let-bound closure literals by name.
    pub closure_bindings: HashMap<String, ClosureLiteral>,
    /// Zig `closure_param_regs`: closure param name -> caller reg.
    pub closure_param_regs: HashMap<String, u32>,
    /// Zig `borrowed_bindings` (C2 prescan result).
    pub borrowed_bindings: HashSet<String>,
    /// Zig `assigned_bindings` (C2 prescan result).
    pub assigned_bindings: HashSet<String>,
    /// Zig `multi_let_bindings`: names let-bound two or more times
    /// in one function; each occurrence gets a fresh register
    /// (Zig field comment, lines 306–309; see SAB-D08).
    pub multi_let_bindings: HashSet<String>,
    /// Zig `inferred_string_slice_locals` (lines 310–314).
    pub inferred_string_slice_locals: HashSet<String>,
    /// Zig `explicit_ptr_string_locals` (lines 315–319).
    pub explicit_ptr_string_locals: HashSet<String>,
    /// Zig `std_surface_rules` (D group; loaded by D1, queried by D4).
    pub std_surface_rules: Vec<StdSurfaceRule>,
    /// Zig `included_imports`.
    pub included_imports: HashSet<String>,
    /// Zig `pending_std_deps` (insertion order = preload order).
    pub pending_std_deps: Vec<PendingStdDep>,
    /// Zig `std_import_modules` + `std_import_module_ids` (F group).
    pub std_import_modules: Vec<StdImportModule>,
    pub std_import_module_ids: HashMap<String, usize>,
    /// Zig `std_macro_templates` + `std_macro_template_ids` (F group).
    pub std_macro_templates: Vec<StdMacroTemplate>,
    pub std_macro_template_ids: HashMap<String, usize>,
    /// Zig `escaped_closure_entries`, keyed there by `*const Node`.
    /// Here: (node key, entry) pairs in insertion order — the Zig
    /// map is iterated at emission time (`emitEscapedClosureEntries`
    /// walks `valueIterator()`), which is exactly the hash-order
    /// hazard the house discipline bans, so the port keeps the
    /// insertion sequence explicitly. Registered as SAB-D18.
    pub escaped_closure_entries: Vec<(usize, EscapedClosureEntry)>,
    /// Zig `global_scalar_consts`: top-level scalar const name ->
    /// the literal node it folds to (C1 prescan result).
    pub global_scalar_consts: HashMap<String, Node>,
    /// Zig `global_array_consts`: name -> const label (C1).
    pub global_array_consts: HashMap<String, String>,
    /// Zig `copy_value_cache` (D3; see SAB-D05).
    pub copy_value_cache: HashMap<String, bool>,
    /// Zig `hashmap_key_slots`.
    pub hashmap_key_slots: HashMap<String, u32>,
    /// Zig `sa_std_root` (D1 resolution result; see SAB-D02).
    pub sa_std_root: Option<String>,
    /// Zig `test_sigs`: genTestDecl appends every test sig here in
    /// addition to the main sig list (the main list drives the
    /// encoder; this one is a registry duplicate).
    pub test_sigs: Vec<FunctionSig>,
}

/// Per-function frame state (Zig field block, function-scope half).
#[derive(Debug, Default)]
pub struct FrameState {
    /// Zig `locals` (E2). Searched newest-first by `local_reg`.
    pub locals: Vec<Local>,
    /// Zig `loop_continue_labels` / `loop_break_labels` (stacks).
    pub loop_continue_labels: Vec<String>,
    pub loop_break_labels: Vec<String>,
    /// The SAB-D09 classification, one entry per live register.
    pub reg_states: HashMap<u32, RegState>,
    /// Zig `stack_alloc_emitted`: regs whose stack_alloc was
    /// already emitted in this function (emission fact, E2/E5).
    pub stack_alloc_emitted: HashSet<u32>,
    /// Zig `refcell_borrow_values` (payload map).
    pub refcell_borrow_values: HashMap<u32, RefCellBorrowValue>,
    /// Zig `mutex_guard_values` (payload map).
    pub mutex_guard_values: HashMap<u32, MutexGuardValue>,
    /// Zig `rwlock_guard_values` (payload map).
    pub rwlock_guard_values: HashMap<u32, RwLockGuardValue>,
    /// Zig `result_slot_refcell_handles` / `result_slot_refcell_slots`.
    pub result_slot_refcell_handles: HashMap<u32, ResultSlotRefCellHandle>,
    pub result_slot_refcell_slots: HashMap<u32, u32>,
    /// Zig `borrow_address_temps` (payload map).
    pub borrow_address_temps: HashMap<u32, BorrowAddressTempState>,
    /// Zig `future_state_vtables` / `future_readiness` /
    /// `future_readiness_by_name` (H group payloads).
    pub future_state_vtables: HashMap<u32, String>,
    pub future_readiness: HashMap<u32, FutureReadiness>,
    pub future_readiness_by_name: HashMap<String, FutureReadiness>,
    /// Zig `in_function_body`.
    pub in_function_body: bool,
    /// Zig `current_async_return` / `current_async_return_ty`.
    pub current_async_return: bool,
    pub current_async_return_ty: Option<Type>,
    /// Zig `current_expr_result_escapes`.
    pub current_expr_result_escapes: bool,
    /// Zig `active_macro_try_cleanup`.
    pub active_macro_try_cleanup: Option<Vec<String>>,
}

impl FrameState {
    /// Classification of a register, defaulting to `Owned`
    /// (a register absent from the map was never reclassified).
    pub fn reg_state(&self, reg: u32) -> RegState {
        self.reg_states.get(&reg).copied().unwrap_or(RegState::Owned)
    }

    /// Zig `beginFunction`'s per-function reset, frame half:
    /// locals, loop stacks and every per-register map start empty
    /// for each function. (The builder half — recorded regs — is
    /// `ModuleBuilder::begin_function`.)
    pub fn begin_function(&mut self) {
        self.locals.clear();
        self.loop_continue_labels.clear();
        self.loop_break_labels.clear();
        self.reg_states.clear();
        self.stack_alloc_emitted.clear();
        self.refcell_borrow_values.clear();
        self.mutex_guard_values.clear();
        self.rwlock_guard_values.clear();
        self.result_slot_refcell_handles.clear();
        self.result_slot_refcell_slots.clear();
        self.borrow_address_temps.clear();
        self.future_state_vtables.clear();
        self.future_readiness.clear();
        self.future_readiness_by_name.clear();
        self.in_function_body = true;
        self.current_async_return = false;
        self.current_async_return_ty = None;
        self.current_expr_result_escapes = false;
        self.active_macro_try_cleanup = None;
    }

    /// Zig `localReg` (line 3526): newest binding wins.
    pub fn local_reg(&self, name: &str) -> Option<u32> {
        self.locals
            .iter()
            .rev()
            .find(|local| local.name == name)
            .map(|local| local.reg)
    }

    /// Zig `pushLocal` bookkeeping half (the builder records the
    /// register itself at the call sites that emit).
    pub fn push_local(&mut self, local: Local) {
        self.locals.push(local);
    }

    /// Zig `popLocalsTo`: truncate the locals stack.
    pub fn pop_locals_to(&mut self, len: usize) {
        self.locals.truncate(len);
    }
}

/// The full codegen state: emission trunk + program registries +
/// current frame + the counters/flags that belong to neither
/// (Zig `macro_fragment_idx`, `macro_call_idx`,
/// `escaped_closure_idx`, `future_task_helpers_emitted`,
/// `fragment_rename*`, `decoded_module_rename_idx`).
#[derive(Debug, Default)]
pub struct CodegenState {
    pub builder: ModuleBuilder,
    pub program: ProgramState,
    pub frame: FrameState,
    /// Zig `macro_fragment_idx` (F group).
    pub macro_fragment_idx: usize,
    /// Zig `macro_call_idx` (G group).
    pub macro_call_idx: usize,
    /// Zig `escaped_closure_idx` (I group).
    pub escaped_closure_idx: usize,
    /// Zig `future_task_helpers_emitted` (H group).
    pub future_task_helpers_emitted: bool,
    /// Zig `fragment_rename` / `fragment_rename_args` /
    /// `fragment_rename_idx` (F group; see the Zig field comment on
    /// fragment-internal register renaming).
    pub fragment_rename: Option<HashMap<String, String>>,
    pub fragment_rename_args: Option<Vec<String>>,
    pub fragment_rename_idx: usize,
    /// Zig `decoded_module_rename_idx` (F group).
    pub decoded_module_rename_idx: usize,
}

impl CodegenState {
    /// Zig `Codegen.init`: every collection empty, every counter
    /// zero. (Zig `deinit` has no Rust counterpart — ownership drops
    /// the state; the explicit freeing of rule/module payloads in
    /// Zig `deinit` is RAII here.)
    pub fn new() -> Self {
        Self::default()
    }
}

//! The `Codegen` state and its small shared emitters, mirrored from
//! the state block and helper methods of codegen.zig's `Codegen`
//! struct (codegen.zig:96, 331-398, 620-703, 786-841, 9724-9765,
//! 10437-10460).
//!
//! State that the Zig original threads as parameters or keeps in
//! handle-family tables the slice never populates (file/metadata
//! handle maps) is not carried here; the release cascade documents
//! where those checks sat. RefCell borrow handles and their
//! result-slot companions ARE carried (refcell.rs plans in
//! sla-lowering). `hoisted_allocs` is per-function state instead of
//! a threaded parameter (restructuring note, divergences (b)).

use std::collections::{HashMap, HashSet};

use sla_ast::{FuncDecl, Literal, Node, Primitive, Type};
use sla_lowering::{
    abi_return_type_string, abi_type_size, abi_type_string, call_arg_needs_release,
    expr_result_needs_release, is_main_name, CheckInfo, FutureReadiness,
};

use crate::{unsupported, Result, TextError};

/// codegen.zig:113 `RefCellBorrowHandle`: the cell register a
/// borrow was taken from, the borrow kind, and an optional
/// owner temp that also needs release when the handle settles.
#[derive(Debug, Clone)]
pub(crate) struct RefCellBorrowHandle {
    pub(crate) cell_reg: String,
    pub(crate) kind: sla_lowering::refcell::RefCellBorrowKind,
    pub(crate) cell_release_temp: Option<String>,
}

/// codegen.zig:119 `ResultSlotRefCellHandle`: the companion
/// slot holding a stored value's cell register, plus the
/// borrow kind to release with.
#[derive(Debug, Clone)]
pub(crate) struct ResultSlotRefCellHandle {
    pub(crate) cell_slot: String,
    pub(crate) kind: sla_lowering::refcell::RefCellBorrowKind,
}

pub struct Codegen<'c> {
    pub(crate) check: &'c dyn CheckInfo,
    pub(crate) out: String,

    // Global counters: never reset between functions (goldens show
    // tmp/label/string numbering continuing across the whole file).
    pub(crate) tmp_idx: usize,
    pub(crate) label_idx: usize,
    pub(crate) string_idx: usize,
    #[allow(dead_code)]
    pub(crate) macro_local_idx: usize,

    // Per-function binding state (cleared at each func/test decl).
    pub(crate) consumed_bindings: HashSet<String>,
    pub(crate) stack_alloc_bindings: HashSet<String>,
    pub(crate) assigned_value_slots: HashSet<String>,
    pub(crate) addressable_bindings: HashSet<String>,
    pub(crate) assigned_bindings: HashSet<String>,
    pub(crate) repeated_let_bindings: HashSet<String>,
    pub(crate) binding_aliases: Vec<(String, Vec<String>)>,
    /// codegen.zig `borrow_source_temps`: address temp -> the
    /// source temp its value derives from, recorded by the
    /// place-address consumers (remember*Source) and consumed by
    /// emitRelease's borrow-address-temp cascade step.
    pub(crate) borrow_source_temps: HashMap<String, String>,
    /// codegen.zig `hashmap_key_slots`: string literal -> the
    /// cached slice-slot register materialised for it in this
    /// function (identity-keyed map/set probes for the same
    /// literal must hit the same slot).
    pub(crate) hashmap_key_slots: HashMap<String, String>,
    pub(crate) let_binding_aliases: HashMap<usize, String>,
    pub(crate) local_binding_types: Vec<(String, Type)>,
    pub(crate) hoisted_allocs: Vec<String>,
    /// Names of the current function's borrow parameters
    /// (codegen.zig `borrow_param_bindings`): a borrowed binding
    /// never owns its value, so emit_release's drop-plan dispatch
    /// must not fire for it.
    pub(crate) borrow_param_bindings: HashSet<String>,
    /// Set by the struct-update site (into.rs) while it settles
    /// the update source register: that register's fields were
    /// raw-moved or explicitly compensated field by field, so the
    /// plan dispatch in emit_release must stay out of the way.
    pub(crate) plan_drop_suppressed: bool,
    /// Emitter-side drop taint for struct updates (the Rust
    /// checker exposes no taint surface): `(root binding, field)`
    /// pairs whose ownership a struct update moved out of the
    /// source place. The source binding's later plan release must
    /// not drop those fields a second time. Arc/Rc fields of a
    /// PARAM source are never tainted: the call-argument copy
    /// clone-bumped them, and the source's plan drop is exactly
    /// the compensation for that bump.
    pub(crate) update_moved_fields: HashSet<(String, String)>,
    /// Bindings whose whole value was moved into an aggregate
    /// literal (a tuple / struct / array / enum-payload element
    /// that is a bare identifier or an explicit move of one, of
    /// a non-Copy, non-borrow-like type). Their fields belong to
    /// the aggregate now: a later release of the binding retires
    /// only the husk register, never the field drop cascade
    /// (the Zig cell model's release of a moved-out local is
    /// trivial for the same reason). Cleared per function with
    /// `update_moved_fields`.
    pub(crate) whole_moved_bindings: HashSet<String>,
    /// RUST-STAGE10-DROP-TAINT (B1): emitter-side borrow/taint
    /// (the Rust checker exposes no taint surface, and the
    /// emitter has no clone-on-extract, so a shallow copy of a
    /// borrowed/moved-out deep value aliases the source's
    /// buffers). Binding name -> the form of its taint: a
    /// FreshHusk binding owns a fresh block whose FIELDS alias
    /// (a call result the callee constructed); an AliasedHusk
    /// binding's own block aliases the source's storage (a place
    /// read, whose register holds a pointer into the source).
    /// Cleared per function with `update_moved_fields`.
    pub(crate) tainted_bindings: HashMap<String, crate::drop::TaintForm>,
    /// Names of the current function's parameters (all kinds);
    /// the struct-update taint rules distinguish a param source
    /// (arrived through the call-argument copy) from a local one.
    pub(crate) param_bindings: HashSet<String>,

    /// User-macro inline expansion state (codegen.zig
    /// `macro_arg_exprs` / `macro_arg_types` / `macro_inline_depth`
    /// / `active_inline_macro` / `active_macro_try_cleanup`).
    /// Arg nodes are stored by node key — this lane's equivalent
    /// of Zig's pointer-keyed map — and their types read back
    /// through CheckInfo::expr_type_by_key.
    pub(crate) macro_arg_exprs: HashMap<String, usize>,
    pub(crate) macro_arg_types: HashMap<String, Type>,
    pub(crate) macro_inline_depth: usize,
    pub(crate) active_inline_macro: Option<sla_ast::MacroDecl>,
    pub(crate) active_macro_try_cleanup: Option<Vec<String>>,
    /// One-shot loop-counter slot override for a `for` statement
    /// generated inside a user-macro inline: the hygiene slot is
    /// allocated at the inline site, not hoisted at function entry.
    pub(crate) for_slot_override: Option<(usize, String)>,

    // Program-wide state.
    pub(crate) global_const_bindings: HashSet<String>,
    pub(crate) global_scalar_consts: Vec<(String, Literal)>,
    pub(crate) func_index: Vec<(String, FuncDecl)>,
    /// Impl / overload methods by mangled name (`Type_method`,
    /// `Type__Trait_method`), collected in `generate_program`
    /// alongside the emission of their bodies. Kept separate from
    /// `func_index` so bare-name and tail-match resolution keep
    /// their existing ambiguity behavior; associated calls and
    /// receiver-typed method calls consult this index.
    pub(crate) assoc_index: Vec<(String, FuncDecl)>,
    /// Enum declarations by name, collected from the program's
    /// decls in `generate_program` (the SAB driver keeps the same
    /// index; the checker's own enum table keeps only variant
    /// names, not fields/discriminants).
    pub(crate) enum_index: Vec<(String, sla_ast::EnumDecl)>,
    /// Trait declarations by name, collected from the program's
    /// decls in `generate_program`; the fallback source for trait
    /// shapes when the checker adapter supplies none (the
    /// production path reads `CheckInfo::trait_decl`, the
    /// projection of the checker's trait table).
    pub(crate) trait_index: Vec<(String, sla_ast::TraitDecl)>,
    /// thread::spawn helper table (thread_spawn.rs), keyed by
    /// the spawn call node's key; filled by the collection
    /// pre-pass in `generate_program` and never cleared per
    /// function (codegen.zig `thread_spawn_helpers`).
    pub(crate) thread_spawn_helpers: Vec<(usize, crate::thread_spawn::ThreadSpawnHelper)>,
    pub(crate) thread_helper_idx: usize,
    /// RUST-STAGE10-CLONE-EXTRACT: per-type `@sla_clone_<key>`
    /// helpers queued by `gen_recursive_clone_call` (clone.rs) and
    /// emitted at the end of `generate_program` (Zig
    /// `clone_helper_types` / `clone_helper_keys`,
    /// codegen.zig:12354-12408). Program-wide: never cleared by
    /// `reset_function_state`, so a helper queued while generating
    /// one function is emitted after all of them.
    pub(crate) clone_helpers: Vec<(String, Type)>,
    /// RUST-STAGE10-CLONE-EXTRACT (M2 refinement): names of
    /// functions whose return value is produced by a cloning
    /// extraction (a SLICE_GET writeback that deep-cloned,
    /// extern_macro.rs). Such a callee returns a fresh-owned
    /// value, so the caller-side M2-call taint (drop.rs) must not
    /// suppress the result's field drops — the clone replaces the
    /// taint for this shape. Program-wide; populated during the
    /// callee's compilation, consulted when the caller is compiled
    /// (sa_std functions precede tests in `generate_program`).
    pub(crate) clone_extract_funcs: HashSet<String>,
    /// Slots of inline-joined spawns (codegen.zig
    /// `inline_thread_handles`): a join on one reads the
    /// already-computed slot value instead of waiting.
    pub(crate) inline_thread_handles: HashSet<String>,
    /// codegen.zig `mutex_lock_results` (mutex.rs lock arm):
    /// a `Mutex::lock` call's Result register -> the mutex
    /// register it locked. `Result::unwrap` transfers the entry
    /// to `mutex_guard_handles`; `emitRelease` unlocks from
    /// either map. Stage 9 macro-family (B_mirror_zig).
    pub(crate) mutex_lock_results: HashMap<String, String>,
    /// codegen.zig `mutex_guard_handles`: a live `MutexGuard`
    /// register -> the mutex register it guards. `emitRelease`
    /// emits `EXPAND MUTEX_UNLOCK` for the mutex and retires
    /// the guard register (codegen.zig:1186).
    pub(crate) mutex_guard_handles: HashMap<String, String>,
    /// codegen.zig `refcell_borrow_handles`: a live RefCell
    /// borrow register -> the handle recorded when its
    /// `borrow` / `borrow_mut` expansion ran. `emitRelease`
    /// settles the dynamic borrow from this table (the
    /// REFCELL_U64_RELEASE_* expansion against the cell
    /// register) before the plain register retire.
    pub(crate) refcell_borrow_handles: HashMap<String, RefCellBorrowHandle>,
    /// codegen.zig `result_slot_refcell_handles` /
    /// `result_slot_refcell_slots`: when a handle-carrying
    /// value is stored into a stack slot, the cell register is
    /// kept in a companion slot (an `alloc 8` whose +0 word
    /// holds the cell register) so the slot's own release can
    /// settle the borrow after the value register is gone.
    pub(crate) result_slot_refcell_handles: HashMap<String, ResultSlotRefCellHandle>,
    pub(crate) result_slot_refcell_slots: HashMap<String, String>,
    /// Worker-body capture bindings (codegen.zig
    /// `thread_capture_regs`): identifier reads inside a
    /// generated worker resolve to the slot loads.
    pub(crate) thread_capture_regs: HashMap<String, String>,

    // Loop state.
    pub(crate) loop_break_labels: Vec<String>,
    pub(crate) loop_continue_labels: Vec<String>,
    pub(crate) loop_body_local_scopes: Vec<Vec<String>>,
    pub(crate) loop_body_block_depths: Vec<usize>,

    // Statement tracking (liveness.rs): the innermost block being
    // generated, as a raw pointer + len (the AST outlives codegen;
    // reconstructed only inside the liveness accessors), the index
    // of the statement being generated, and the REG-BR-6 move-out
    // claim (assign-statement node key + source name).
    pub(crate) current_block: Option<(*const Node, usize)>,
    pub(crate) current_stmt_index: usize,
    pub(crate) move_out_stmt: Option<usize>,
    pub(crate) move_out_name: Option<String>,
    /// SPRINT-100 L2 (Zig `current_expr_later_nodes`): the stack
    /// of sibling frames for the compound expressions currently
    /// being generated — call arguments, struct-literal field
    /// values, tuple elements. Each frame holds the child node
    /// pointers in generation order plus the index of the child
    /// being generated now; a call argument whose identifier is
    /// read by a LATER sibling must stay live (the value is
    /// cloned for the consuming position instead of moved). Raw
    /// pointers like `current_block`: the AST outlives codegen.
    pub(crate) expr_later_frames: Vec<(Vec<*const Node>, usize)>,
    /// The enclosing function's declared return type (R2b stage
    /// 6): the consuming context for a return statement's value,
    /// consulted by `gen_expr_in_context`. Set by
    /// `gen_func_decl_named`; cleared on every reset (test decls
    /// and nested emission see None).
    pub(crate) current_ret_ty: Option<Type>,
    /// RUST-STAGE10-CLONE-EXTRACT: the enclosing function's
    /// declared name (`FuncDecl.name`, not the lowered/mangled
    /// emission name), for the M2 refinement set
    /// (`clone_extract_funcs`): the SLICE_GET writeback records
    /// the function it is generating. Set by
    /// `gen_func_decl_named` / `gen_test_decl`; NOT cleared by
    /// `reset_function_state` (it is assigned per function, and
    /// the clone helpers generated after the program's functions
    /// contain no writebacks).
    pub(crate) current_func_name: Option<String>,

    // Async family state (stage 6, codegen.zig: task_future_objects,
    // future_state_vtables, future_readiness, executor_task_counts,
    // current_async, current_async_return_ty,
    // async_pending_return_emitted). Cleared per function like the
    // Zig clear list in genFuncDeclNamed.
    pub(crate) future_state_vtables: HashMap<String, String>,
    pub(crate) future_readiness: HashMap<String, FutureReadiness>,
    pub(crate) task_future_objects: HashMap<String, String>,
    pub(crate) executor_task_counts: HashMap<String, usize>,
    pub(crate) current_async: bool,
    pub(crate) current_async_return_ty: Option<Type>,
    pub(crate) async_pending_return_emitted: bool,
}

impl<'c> Codegen<'c> {
    pub fn new(check: &'c dyn CheckInfo) -> Self {
        Codegen {
            check,
            out: String::new(),
            tmp_idx: 0,
            label_idx: 0,
            string_idx: 0,
            macro_local_idx: 0,
            consumed_bindings: HashSet::new(),
            stack_alloc_bindings: HashSet::new(),
            assigned_value_slots: HashSet::new(),
            addressable_bindings: HashSet::new(),
            assigned_bindings: HashSet::new(),
            repeated_let_bindings: HashSet::new(),
            binding_aliases: Vec::new(),
            borrow_source_temps: HashMap::new(),
            hashmap_key_slots: HashMap::new(),
            let_binding_aliases: HashMap::new(),
            local_binding_types: Vec::new(),
            hoisted_allocs: Vec::new(),
            borrow_param_bindings: HashSet::new(),
            plan_drop_suppressed: false,
            update_moved_fields: HashSet::new(),
            whole_moved_bindings: HashSet::new(),
            tainted_bindings: HashMap::new(),
            param_bindings: HashSet::new(),
            macro_arg_exprs: HashMap::new(),
            macro_arg_types: HashMap::new(),
            macro_inline_depth: 0,
            active_inline_macro: None,
            active_macro_try_cleanup: None,
            for_slot_override: None,
            global_const_bindings: HashSet::new(),
            global_scalar_consts: Vec::new(),
            func_index: Vec::new(),
            assoc_index: Vec::new(),
            enum_index: Vec::new(),
            trait_index: Vec::new(),
            thread_spawn_helpers: Vec::new(),
            thread_helper_idx: 0,
            clone_helpers: Vec::new(),
            clone_extract_funcs: HashSet::new(),
            inline_thread_handles: HashSet::new(),
            mutex_lock_results: HashMap::new(),
            mutex_guard_handles: HashMap::new(),
            refcell_borrow_handles: HashMap::new(),
            result_slot_refcell_handles: HashMap::new(),
            result_slot_refcell_slots: HashMap::new(),
            thread_capture_regs: HashMap::new(),
            loop_break_labels: Vec::new(),
            loop_continue_labels: Vec::new(),
            loop_body_local_scopes: Vec::new(),
            loop_body_block_depths: Vec::new(),
            current_block: None,
            current_stmt_index: 0,
            move_out_stmt: None,
            move_out_name: None,
            expr_later_frames: Vec::new(),
            current_ret_ty: None,
            current_func_name: None,
            future_state_vtables: HashMap::new(),
            future_readiness: HashMap::new(),
            task_future_objects: HashMap::new(),
            executor_task_counts: HashMap::new(),
            current_async: false,
            current_async_return_ty: None,
            async_pending_return_emitted: false,
        }
    }

    // -- output ------------------------------------------------------

    /// Raw append (Zig `outPrint` with the format's own newlines).
    pub(crate) fn w(&mut self, s: &str) {
        self.out.push_str(s);
    }

    /// Append one line (content + `\n`).
    pub(crate) fn wl(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    /// Append an indented instruction line (4 spaces, as every Zig
    /// instruction format string carries).
    pub(crate) fn ins(&mut self, s: &str) {
        self.out.push_str("    ");
        self.out.push_str(s);
        self.out.push('\n');
    }

    // -- counters (codegen.zig:367-398) ------------------------------

    pub(crate) fn new_tmp(&mut self) -> String {
        let name = format!("tmp_{}", self.tmp_idx);
        self.tmp_idx += 1;
        name
    }

    pub(crate) fn new_label(&mut self, prefix: &str) -> String {
        let name = format!("{prefix}_{}", self.label_idx);
        self.label_idx += 1;
        name
    }

    pub(crate) fn new_string_const(&mut self) -> String {
        let name = format!("SLA_STR_{}", self.string_idx);
        self.string_idx += 1;
        name
    }

    // -- binding aliases (codegen.zig:331-359) ------------------------

    pub(crate) fn resolve_binding_name(&self, name: &str) -> String {
        for (n, aliases) in &self.binding_aliases {
            if n == name {
                if let Some(top) = aliases.last() {
                    return top.clone();
                }
            }
        }
        name.to_string()
    }

    pub(crate) fn push_binding_alias(&mut self, name: &str) -> String {
        let alias = self.new_tmp();
        self.push_binding_alias_to(name, &alias);
        alias
    }

    pub(crate) fn push_binding_alias_to(&mut self, name: &str, alias: &str) {
        for (n, aliases) in &mut self.binding_aliases {
            if n == name {
                aliases.push(alias.to_string());
                return;
            }
        }
        self.binding_aliases
            .push((name.to_string(), vec![alias.to_string()]));
    }

    pub(crate) fn pop_binding_alias(&mut self, name: &str) {
        for (n, aliases) in &mut self.binding_aliases {
            if n == name {
                aliases.pop();
                return;
            }
        }
    }

    pub(crate) fn clear_binding_aliases(&mut self) {
        self.binding_aliases.clear();
    }

    // -- per-function state reset (genFuncDeclNamed/genTestDecl) ------

    pub(crate) fn reset_function_state(&mut self) {
        self.addressable_bindings.clear();
        self.assigned_bindings.clear();
        self.assigned_value_slots.clear();
        self.repeated_let_bindings.clear();
        self.stack_alloc_bindings.clear();
        self.consumed_bindings.clear();
        self.borrow_source_temps.clear();
        self.hashmap_key_slots.clear();
        self.local_binding_types.clear();
        self.borrow_param_bindings.clear();
        self.plan_drop_suppressed = false;
        self.update_moved_fields.clear();
        self.whole_moved_bindings.clear();
        self.tainted_bindings.clear();
        self.param_bindings.clear();
        self.macro_arg_exprs.clear();
        self.macro_arg_types.clear();
        self.macro_inline_depth = 0;
        self.active_inline_macro = None;
        self.active_macro_try_cleanup = None;
        self.for_slot_override = None;
        self.clear_binding_aliases();
        self.let_binding_aliases.clear();
        self.hoisted_allocs.clear();
        self.current_block = None;
        self.current_stmt_index = 0;
        self.move_out_stmt = None;
        self.move_out_name = None;
        self.expr_later_frames.clear();
        self.current_ret_ty = None;
        self.future_state_vtables.clear();
        self.future_readiness.clear();
        self.task_future_objects.clear();
        self.executor_task_counts.clear();
        self.mutex_lock_results.clear();
        self.mutex_guard_handles.clear();
        self.refcell_borrow_handles.clear();
        self.result_slot_refcell_handles.clear();
        self.result_slot_refcell_slots.clear();
        self.async_pending_return_emitted = false;
    }

    // -- lookups --------------------------------------------------------

    /// `tc.funcs` first (checker-registered, incl. imported), then the
    /// program's own declarations.
    pub(crate) fn func_decl(&self, name: &str) -> Option<FuncDecl> {
        if let Some(f) = self.check.func(name) {
            return Some(f.clone());
        }
        for (n, f) in &self.func_index {
            if n == name {
                return Some(f.clone());
            }
        }
        None
    }

    /// `using` extension callee lookup (mirrors the SAB driver's
    /// plain-call tail): the call site keeps the bare method name
    /// while the import expansion lifted the callee under a
    /// module-prefixed key (`math3__double`); a UNIQUE tail-segment
    /// match over the program's function index is the callee.
    /// Ambiguity or absence returns None and the caller keeps its
    /// existing fallback.
    pub(crate) fn func_decl_tail_match(&self, name: &str) -> Option<(String, FuncDecl)> {
        let mut found: Option<&(String, FuncDecl)> = None;
        for entry in &self.func_index {
            let key = &entry.0;
            let tail_hit = key.rsplit("::").next() == Some(name)
                || key.rsplit("__").next() == Some(name);
            if tail_hit {
                if found.is_some() {
                    return None;
                }
                found = Some(entry);
            }
        }
        found.map(|(n, f)| (n.clone(), f.clone()))
    }

    /// Enum declaration from the program index (collected in
    /// `generate_program`, including imported modules' decls).
    pub(crate) fn enum_decl(&self, name: &str) -> Option<&sla_ast::EnumDecl> {
        self.enum_index
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d)
    }

    pub(crate) fn expr_type(&self, node: &Node) -> Option<Type> {
        let fact = self.check.expr_type(node).cloned();
        match fact {
            Some(ty) if ty != Type::Infer => Some(ty),
            _ if self.macro_inline_depth > 0 => {
                // Inside a user-macro inline, body nodes carry no
                // checker facts (untyped macro bodies are never
                // type-checked); Zig's single resolver
                // (resolvedTypeForExpr) derives their types
                // structurally, so this lane does the same — scoped
                // to the inline so non-macro emission is unchanged.
                self.resolved_type_for_expr(node).or(fact)
            }
            other => other,
        }
    }

    pub(crate) fn require_expr_type(&self, node: &Node, context: &'static str) -> Result<Type> {
        self.expr_type(node).ok_or(TextError::MissingType {
            context: context.to_string(),
        })
    }

    pub(crate) fn type_is_copy_value(&self, ty: &Type) -> bool {
        // F-G10: `Chan` handles are copy values -- the checker's
        // leaf predicate (check_leaves.rs `is_copy_value`, mirroring
        // Zig `typeIsCopyValue` on this arm) says so, but the
        // CheckInfo fact answers with the checker's `typeIsCopy`,
        // under which a Chan has no struct declaration and reads as
        // non-Copy. Planning with the non-Copy answer marked Chan
        // parameters with the move capability (`^ch` in the
        // signature and at call sites), so a second use of the same
        // channel trapped UseAfterMove 1009 in the host. Answer the
        // leaf arm here so every planning consumer in this crate
        // agrees with the checker; all other shapes defer to the
        // CheckInfo fact.
        if matches!(ty, Type::UserDefined(u) if u.name == "Chan") {
            return true;
        }
        // RUST-TEXT-FNPTR-PARAM-EXIT-RELEASE: fn-ptrs are copy
        // values too -- the same leaf predicate carries the
        // dedicated arm `Type::FnPtr(_) => true` (mirroring Zig
        // lowering_rules `typeIsCopyValueLeaf`), while the
        // CheckInfo fact answers with the checker `type_is_copy`,
        // under which fn_ptr stays deliberately non-Copy.
        // Planning with the non-Copy answer marked fn-ptr
        // parameters with the move capability (`^entry: ptr` in
        // the signature, `^arg` at call sites), so the exit
        // cleanup `!entry` freed the register value -- a
        // caller-supplied `@const` vtable address, not a heap
        // allocation -- and the host aborted with signal 6
        // (`free(): invalid pointer`) on a function that merely
        // received a `fn(u64)` parameter and returned. Answer
        // the leaf arm here, as for Chan above, so signature,
        // call-argument, capture and payload planning all agree
        // with the checker; the checker `type_is_copy`
        // adjudication itself is untouched.
        if matches!(ty, Type::FnPtr(_)) {
            return true;
        }
        // An array of copy values is a copy value (Zig codegen
        // `typeIsCopyValue` `.array` arm; the checker's leaf
        // predicate `is_copy_value` carries the same arm, but the
        // CheckInfo fact answers with `type_is_copy`, under which
        // an array is never Copy — the same caliber split as Chan
        // and FnPtr above). Planning a by-value array argument
        // with the non-Copy answer transfers ownership out of the
        // source binding, so a later borrow of the same array
        // (`read_index_borrow(values, 1)` then
        // `read_borrowed_index_borrow(&values, 2)`) trapped
        // UseAfterMove 1009 in the host.
        if let Type::Array(arr) = ty {
            return self.type_is_copy_value(&arr.elem);
        }
        self.check.type_is_copy_value(ty)
    }

    pub(crate) fn type_is_copy_struct(&self, ty: &Type) -> bool {
        self.check.type_is_copy_struct(ty)
    }

    pub(crate) fn remember_local_binding_type(&mut self, name: &str, ty: &Type) {
        for (n, t) in &mut self.local_binding_types {
            if n == name {
                *t = ty.clone();
                return;
            }
        }
        self.local_binding_types.push((name.to_string(), ty.clone()));
    }

    pub(crate) fn local_binding_type_for_name(&self, name: &str) -> Option<Type> {
        self.local_binding_types
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
    }

    /// codegen.zig `macroArgTypeForName`: the recorded macro-arg
    /// type for a name (skipping Infer), else the recorded arg
    /// expression's checker type (also skipping Infer).
    pub(crate) fn macro_arg_type_for_name(&self, name: &str) -> Option<Type> {
        if let Some(ty) = self.macro_arg_types.get(name) {
            if *ty != Type::Infer {
                return Some(ty.clone());
            }
        }
        if let Some(key) = self.macro_arg_exprs.get(name) {
            if let Some(ty) = self.check.expr_type_by_key(*key) {
                if *ty != Type::Infer {
                    return Some(ty.clone());
                }
            }
        }
        None
    }

    /// codegen.zig `newInlineMacroLocal`: the hygiene-renamed
    /// register for a macro-body local.
    pub(crate) fn new_inline_macro_local(&mut self, macro_name: &str, local: &str) -> String {
        let idx = self.macro_local_idx;
        self.macro_local_idx += 1;
        format!("__sla_macro_{macro_name}_{local}_{idx}")
    }

    /// codegen.zig `fieldTypeForType` (structs via the CheckInfo
    /// projection; tuples by numeric field name).
    pub(crate) fn field_type_for_type(&self, ty: &Type, name: &str) -> Option<Type> {
        let curr = sla_lowering::peel_borrow_pointer_type(ty);
        if let Type::Tuple(t) = curr {
            let index: usize = name.parse().ok()?;
            return t.elems.get(index).cloned();
        }
        if let Type::UserDefined(u) = curr {
            let decl = self.check.struct_decl(&u.name)?;
            for field in &decl.fields {
                if field.name == name {
                    return Some((*field.ty).clone());
                }
            }
        }
        None
    }

    /// codegen.zig:5322 `resolvedTypeForExpr`: the checker fact
    /// first (skipping Infer), then the structural fallback chain.
    /// Macro bodies of untyped macros are never type-checked, so
    /// body nodes reach codegen with no facts at all; this chain
    /// is what the Zig emitter resolves them with.
    pub(crate) fn resolved_type_for_expr(&self, expr: &Node) -> Option<Type> {
        if let Some(ty) = self.check.expr_type(expr) {
            if *ty != Type::Infer {
                return Some(ty.clone());
            }
        }
        match expr {
            Node::Literal(lit) => Some(Type::Primitive(match lit {
                sla_ast::Literal::IntVal(_) => sla_ast::Primitive::I64,
                sla_ast::Literal::FloatVal(_) => sla_ast::Primitive::F64,
                sla_ast::Literal::BoolVal(_) => sla_ast::Primitive::Boolean,
                sla_ast::Literal::StringVal(_) => sla_ast::Primitive::RawPtr,
            })),
            Node::Identifier(name) => self
                .macro_arg_type_for_name(name)
                .or_else(|| self.local_binding_type_for_name(name))
                // Zig `localBindingTypeForName`'s second step: the
                // type recorded under the resolved (aliased)
                // register name — match payload bindings and other
                // alias-introduced bindings are remembered there.
                .or_else(|| {
                    let resolved = self.resolve_binding_name(name);
                    if resolved != *name {
                        self.local_binding_type_for_name(&resolved)
                    } else {
                        None
                    }
                }),
            Node::CallExpr(call) => {
                if let Some(func) = self.func_decl(&call.func_name) {
                    return Some((*func.ret_ty).clone());
                }
                None
            }
            Node::BinaryExpr(bin) => {
                use sla_ast::BinaryOp as B;
                if matches!(
                    bin.op,
                    B::Eq | B::Ne | B::Lt | B::Le | B::Gt | B::Ge
                        | B::LogicalAnd | B::LogicalOr
                ) {
                    return Some(Type::Primitive(sla_ast::Primitive::Boolean));
                }
                self.resolved_type_for_expr(&bin.left)
                    .or_else(|| self.resolved_type_for_expr(&bin.right))
            }
            Node::BorrowExpr(b) => self
                .resolved_type_for_expr(&b.expr)
                .map(|inner| Type::Borrow(Box::new(inner))),
            Node::DerefExpr(d) => {
                let source_ty = self.resolved_type_for_expr(&d.expr)?;
                match source_ty {
                    Type::Borrow(inner) | Type::Pointer(inner) => Some((*inner).clone()),
                    Type::UserDefined(u) if u.generics.len() == 1
                        && matches!(u.name.as_str(), "Rc" | "Arc" | "Box") =>
                    {
                        Some(u.generics[0].clone())
                    }
                    _ => None,
                }
            }
            Node::CastExpr(cast) => Some((*cast.ty).clone()),
            Node::FieldExpr(f) => {
                let target_ty = self.resolved_type_for_expr(&f.expr)?;
                self.field_type_for_type(&target_ty, &f.field_name)
            }
            Node::IndexExpr(idx) => {
                let target_ty = self.resolved_type_for_expr(&idx.target)?;
                match sla_lowering::peel_borrow_pointer_type(&target_ty) {
                    Type::Array(arr) => Some((*arr.elem).clone()),
                    Type::UserDefined(u)
                        if matches!(
                            u.name.as_str(),
                            "Slice" | "Vec" | "GrowVec" | "VecDeque"
                        ) =>
                    {
                        u.generics.first().cloned()
                    }
                    _ => None,
                }
            }
            Node::StructLiteral(lit) => Some((*lit.ty).clone()),
            Node::TupleLiteral(lit) => {
                if lit.elements.is_empty() {
                    return None;
                }
                let mut elems = Vec::with_capacity(lit.elements.len());
                for elem in &lit.elements {
                    elems.push(self.resolved_type_for_expr(elem)?);
                }
                Some(Type::Tuple(sla_ast::TupleType { elems }))
            }
            Node::ArrayLiteral(lit) => {
                let first = lit.elements.first()?;
                let elem_ty = self.resolved_type_for_expr(first)?;
                Some(Type::Array(sla_ast::ArrayType {
                    elem: Box::new(elem_ty),
                    len: lit.elements.len(),
                }))
            }
            Node::RepeatArrayLiteral(lit) => {
                let elem_ty = self.resolved_type_for_expr(&lit.value)?;
                Some(Type::Array(sla_ast::ArrayType {
                    elem: Box::new(elem_ty),
                    len: lit.len,
                }))
            }
            Node::MoveExpr(m) => self.resolved_type_for_expr(&m.expr),
            // Rust-lane compensation arms: the Zig checker records
            // body-node types at macro declaration (checkMacro),
            // so Zig's chain needs no if/match arms; this lane's
            // checker records none for untyped macros, so the
            // result type is derived from the branch tails.
            Node::IfExpr(ife) => {
                let tail_ty = |block: &[Node]| -> Option<Type> {
                    match block.last() {
                        Some(Node::ExprStmt(e)) => self.resolved_type_for_expr(e),
                        _ => None,
                    }
                };
                tail_ty(&ife.then_block)
                    .or_else(|| ife.else_block.as_ref().and_then(|b| tail_ty(b)))
                    // A statement-position if has no value tail in
                    // either branch; its type is void (the fact the
                    // Zig checker's checkMacro would have recorded).
                    .or_else(|| {
                        Some(Type::Primitive(sla_ast::Primitive::VoidType))
                    })
            }
            Node::MatchExpr(m) => m
                .cases
                .iter()
                .find_map(|case| match case.body.last() {
                    Some(Node::ExprStmt(e)) => self.resolved_type_for_expr(e),
                    _ => None,
                })
                .or_else(|| Some(Type::Primitive(sla_ast::Primitive::VoidType))),
            _ => None,
        }
    }

    pub(crate) fn global_scalar_const(&self, name: &str) -> Option<Literal> {
        self.global_scalar_consts
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, lit)| lit.clone())
    }

    pub(crate) fn mark_consumed_binding(&mut self, name: &str) {
        self.consumed_bindings.insert(name.to_string());
    }

    // -- symbol / type spellings ---------------------------------------

    /// codegen.zig:628 `loweredFuncSymbol`.
    pub(crate) fn lowered_func_symbol(&self, name: &str) -> String {
        if is_main_name(name) {
            return name.to_string();
        }
        if let Some(func) = self.func_decl(name) {
            if func.is_extern || func.no_mangle {
                return name.to_string();
            }
        }
        if self.check.is_extern_func(name) || name.starts_with("sa_") {
            return name.to_string();
        }
        format!("sla__{name}")
    }

    pub(crate) fn fn_ptr_vtable_name(&self, func_name: &str) -> String {
        format!("SLA_FNPTR_VT_{func_name}")
    }

    /// codegen.zig:671 `emitFunctionPointerVTableDecl`.
    pub(crate) fn emit_function_pointer_vtable_decl(&mut self, func_name: &str) {
        let vt_name = self.fn_ptr_vtable_name(func_name);
        let lowered = self.lowered_func_symbol(func_name);
        self.wl(&format!("@const {vt_name} = vtable {{ call = @{lowered} }}"));
    }

    /// The SA symbol a function DEFINITION is emitted under.
    /// Members of a plain-function overload set (2+ declarations
    /// sharing a name in this program) are emitted under their
    /// mangled member symbol (`sla_ast::overload_member_symbol`),
    /// matching the symbol the checker recorded for each resolved
    /// call; every other function keeps `lowered_func_symbol`.
    pub(crate) fn decl_symbol(&self, f: &FuncDecl) -> String {
        let base = self.lowered_func_symbol(&f.name);
        if f.is_extern || f.no_mangle || is_main_name(&f.name) {
            return base;
        }
        if self.func_index.iter().filter(|(n, _)| n == &f.name).count() <= 1 {
            return base;
        }
        let tys: Vec<Type> = f.params.iter().map(|p| (*p.ty).clone()).collect();
        sla_ast::overload_member_symbol(&f.name, &tys)
    }

    /// Decl-aware variant of `emit_function_pointer_vtable_decl`:
    /// overload members get a per-member vtable keyed by the
    /// member symbol (a bare-name reference to an overload set is
    /// checker-rejected, so no use site looks these up by name).
    pub(crate) fn emit_function_pointer_vtable_decl_for_decl(&mut self, f: &FuncDecl) {
        let symbol = self.decl_symbol(f);
        if symbol == self.lowered_func_symbol(&f.name) {
            self.emit_function_pointer_vtable_decl(&f.name);
        } else {
            let vt_name = format!("SLA_FNPTR_VT_{symbol}");
            self.wl(&format!("@const {vt_name} = vtable {{ call = @{symbol} }}"));
        }
    }
}

/// codegen.zig:2455 `typeSize` (thins to lowering `abiTypeSize`).
pub(crate) fn type_size(ty: &Type) -> usize {
    abi_type_size(ty)
}

/// codegen.zig:2467 `typeString` (thins to lowering `abiTypeString`).
pub(crate) fn type_string(ty: &Type) -> &'static str {
    abi_type_string(ty)
}

/// codegen.zig:2491 `abiReturnTypeString` re-export path.
pub(crate) fn ret_type_string(ty: &Type) -> &'static str {
    abi_return_type_string(ty)
}

impl<'c> Codegen<'c> {
    // -- const emitters (codegen.zig:792-841) --------------------------

    /// codegen.zig:792 `emitIntConst`.
    pub(crate) fn emit_int_const(&mut self, target: &str, value: i64) {
        self.ins(&format!("{target} = {value}"));
    }

    /// codegen.zig:796 `emitFloatConst`. Zig `{d}` on f64 prints the
    /// decimal form; integral finite values get an explicit `.0`.
    /// Rust's Display for f64 is shortest-round-trip, which can differ
    /// from Zig's decimal expansion for non-integral values — no float
    /// literal appears in the slice; flagged in the divergences log.
    pub(crate) fn emit_float_const(&mut self, target: &str, value: f64) {
        if value.is_finite() && value.floor() == value {
            self.ins(&format!("{target} = {}.0", value as i64));
        } else {
            self.ins(&format!("{target} = {value}"));
        }
    }

    /// codegen.zig:804 `genLiteralValue`.
    pub(crate) fn gen_literal_value(&mut self, lit: &Literal) -> Result<String> {
        let reg = self.new_tmp();
        match lit {
            Literal::IntVal(v) => self.emit_int_const(&reg, *v),
            Literal::FloatVal(v) => self.emit_float_const(&reg, *v),
            Literal::BoolVal(v) => self.emit_int_const(&reg, if *v { 1 } else { 0 }),
            Literal::StringVal(v) => {
                let label = self.new_string_const();
                // One trailing NUL so the constant can double as a C
                // string; escapedStringByteLen excludes it.
                self.ins(&format!("@const {label} = utf8:\"{v}\\0\""));
                let len_reg = self.new_tmp();
                self.emit_int_const(&len_reg, escaped_string_byte_len(v) as i64);
                self.stack_alloc_bindings.insert(reg.clone());
                self.ins(&format!("{reg} = stack_alloc Slice_SIZE"));
                self.ins(&format!("EXPAND SLICE_NEW {reg}, &{label}, {len_reg}"));
                self.emit_release(&len_reg)?;
            }
        }
        Ok(reg)
    }

    // -- copies and conditions -----------------------------------------

    /// codegen.zig:9724 `emitPrimitiveCopy`.
    pub(crate) fn emit_primitive_copy(
        &mut self,
        target: &str,
        source: &str,
        ty: &Type,
    ) -> Result<()> {
        match ty {
            Type::Primitive(sla_ast::Primitive::Boolean) => {
                self.ins(&format!("{target} = or {source}, 0"));
            }
            Type::Primitive(
                sla_ast::Primitive::F32 | sla_ast::Primitive::F64 | sla_ast::Primitive::Float,
            ) => {
                // fadd against a materialized float zero (an integer
                // `add` copy would be folded onto the source register).
                let zero = self.new_tmp();
                self.emit_float_const(&zero, 0.0);
                self.ins(&format!("{target} = fadd {source}, {zero}"));
                self.emit_release(&zero)?;
            }
            Type::Primitive(_) => {
                self.ins(&format!("{target} = add {source}, 0"));
            }
            _ => return Err(TextError::MissingType { context: "emitPrimitiveCopy non-primitive".to_string() }),
        }
        Ok(())
    }

    /// codegen.zig:9744 `genBranchConditionReg`: a bare primitive
    /// identifier condition is copied into a fresh temp so both branch
    /// successors can release it; anything else is used as generated.
    pub(crate) fn gen_branch_condition_reg(&mut self, cond: &Node) -> Result<String> {
        let cond_reg = self.gen_expr(cond)?;
        let cond_ty = self.require_expr_type(cond, "branch condition")?;
        if let Node::Identifier(name) = cond {
            if matches!(cond_ty, Type::Primitive(_)) {
                let resolved = self.resolve_binding_name(name);
                if cond_reg == resolved
                    || cond_reg == *name
                    || self.global_const_bindings.contains(name)
                {
                    let copied = self.new_tmp();
                    self.emit_primitive_copy(&copied, &cond_reg, &cond_ty)?;
                    return Ok(copied);
                }
            }
        }
        Ok(cond_reg)
    }

    /// codegen.zig:10441 `exprResultRegNeedsRelease`: the plain
    /// lowering fact plus the cast materialization rule.
    pub(crate) fn expr_result_reg_needs_release(&self, expr: &Node) -> bool {
        if let Node::CastExpr(cast) = expr {
            let src_ty = self.expr_type(&cast.expr);
            return match src_ty {
                Some(src) => {
                    sla_lowering::cast_result_materializes_temp(&src, &cast.ty)
                }
                None => expr_result_needs_release(expr),
            };
        }
        expr_result_needs_release(expr)
    }

    /// The stored-value retire gate (Zig `finishStoredValueAfterSlotStore`,
    /// SHARD-S5): `callArgNeedsRelease` is false for every cast_expr,
    /// but a materialising cast (e.g. u64 -> i64) produces a fresh
    /// result temp that nothing else retires after the store/copy.
    /// Let materialising casts through to the caller's retire logic;
    /// non-materialising casts keep the plain fact.
    pub(crate) fn stored_value_needs_release(&self, value: &Node) -> bool {
        call_arg_needs_release(value)
            || (matches!(value, Node::CastExpr(_)) && self.expr_result_reg_needs_release(value))
    }

    /// Deep-copy emission for Copy structs (codegen.zig:3913
    /// `genCopyValueInto`), plain-field subset in copy.rs: a
    /// record whose fields are all plain words or nested plain
    /// structs copies field by field into a fresh record. A
    /// heap-handle field needs the deep clone dispatcher and
    /// stays loud.
    pub(crate) fn gen_copy_value_into(
        &mut self,
        target: &str,
        source: &str,
        ty: &Type,
    ) -> Result<()> {
        if self.gen_copy_value_into_inner(target, source, ty)? {
            Ok(())
        } else {
            unsupported(
                "genCopyValueInto (Copy-struct deep copy of handle fields)",
                "codegen.zig:3913",
            )
        }
    }

    /// DEF-TUPLE-COPY-EMIT (Zig `genTupleCopyValueInto`,
    /// codegen.zig:6177): the tuple analog of
    /// [`Self::gen_copy_value_into`], element-wise in copy.rs.
    pub(crate) fn gen_tuple_copy_value_into(
        &mut self,
        target: &str,
        source: &str,
        ty: &Type,
    ) -> Result<()> {
        if self.gen_tuple_copy_value_into_inner(target, source, ty)? {
            Ok(())
        } else {
            unsupported(
                "genTupleCopyValueInto (Copy-tuple copy with a non-plain nested element)",
                "codegen.zig:6177",
            )
        }
    }
}

/// codegen.zig:2420 `sanitizeTestName`.
pub(crate) fn sanitize_test_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '(' | ')' => out.push('-'),
            '"' | '\\' | '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// codegen.zig:3063 `escapedStringByteLen`: byte length of the value
/// as SA will store it — a backslash escape pair counts once. The AST
/// string value retains source escape text (two bytes), which is why
/// this differs from `value.len()`.
pub(crate) fn escaped_string_byte_len(value: &str) -> usize {
    let bytes = value.as_bytes();
    let mut len = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
        }
        len += 1;
        i += 1;
    }
    len
}

/// codegen.zig:3055/3059 shape facts used by the println and
/// format emitters, mirroring lowering_rules.isFormatStringType /
/// isStringLikeType exactly (borrow/pointer peeled first).
pub(crate) fn is_format_string_type(ty: &Type) -> bool {
    // lowering_rules.isFormatStringType: peel borrow/pointer,
    // then the user-defined `String`.
    matches!(
        sla_lowering::peel_borrow_pointer_type(ty),
        Type::UserDefined(ud) if ud.name == "String"
    )
}

pub(crate) fn is_string_like_type(ty: &Type) -> bool {
    // lowering_rules.isStringLikeType (5280): peel borrow/pointer,
    // then raw pointers, arrays and `String` are string-like. The
    // earlier reduced form (Borrow(Array) / `str` only) missed
    // raw-pointer-typed string literals in format/println args.
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::Primitive(Primitive::RawPtr) => true,
        Type::Array(_) => true,
        Type::UserDefined(ud) => ud.name == "String",
        _ => false,
    }
}

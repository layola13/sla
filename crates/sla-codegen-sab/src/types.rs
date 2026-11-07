//! B2 — auxiliary structures of `sab_codegen.zig` (Zig lines 66–293):
//! locals, borrow-guard values, branch snapshot slots, macro
//! captures, std-surface rules, decoded-module remaps.
//!
//! Porting conventions (structural, registered as SAB-D16/D09 kin):
//! - Zig `[]const u8` names become owned `String`s; `*const ast.Type`
//!   and `*const ast.Node` become owned `Type` / `Node` values (the
//!   Rust AST is owned, so the port clones instead of borrowing).
//! - Zig `undefined` payloads that are only meaningful under a flag
//!   (`MacroLocalChange.old` under `had_old`) become `Option<T>` —
//!   the uninitialized state is not expressible here.
//! - Lookup-only Zig hash maps stay `HashMap`/`HashSet`; nothing in
//!   this module is iterated for emission. The one ordered
//!   collection, `EscapedCaptureCollector.ordered`, is a `Vec`,
//!   exactly as in Zig (`array_list.Managed`).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::cell::RefCell;

use sla_ast::{ClosureLiteral, Node, Type};

use crate::ir::{CapPrefix, ConstDecl, FunctionSig, Instruction, Module, PrimType};

/// Zig `Local` (lines 66–73): one local binding's register and the
/// type/capability facts the release engine consults.
#[derive(Debug, Clone, PartialEq)]
pub struct Local {
    pub name: String,
    pub reg: u32,
    pub is_param: bool,
    /// Type of the stack slot for stack-homed bindings
    /// (Zig `stack_ty`, set by `pushStackLocal`).
    pub stack_ty: Option<Type>,
    /// Declared/value type when known (Zig `ty`).
    pub ty: Option<Type>,
    /// ABI capability for parameters (Zig `param_cap`).
    pub param_cap: Option<CapPrefix>,
    pub is_stack_alloc: bool,
}

impl Local {
    /// Zig `pushLocal` shape: plain binding, no type facts.
    pub fn plain(name: &str, reg: u32, is_param: bool) -> Self {
        Local {
            name: name.to_string(),
            reg,
            is_param,
            stack_ty: None,
            ty: None,
            param_cap: None,
            is_stack_alloc: false,
        }
    }

    /// Zig `pushTypedLocal` shape.
    pub fn typed(name: &str, reg: u32, is_param: bool, ty: Type) -> Self {
        Local {
            ty: Some(ty),
            ..Local::plain(name, reg, is_param)
        }
    }

    /// Zig `pushStackLocal` shape: `stack_ty` set, `ty` unset.
    pub fn stack(name: &str, reg: u32, ty: Type) -> Self {
        Local {
            stack_ty: Some(ty),
            ..Local::plain(name, reg, false)
        }
    }
}

/// Zig `lowering_rules.RefCellBorrowKind` (lowering_rules.zig:3299).
/// Defined locally because sla-lowering has not ported that enum yet;
/// when it does, this type is replaced by a re-export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefCellBorrowKind {
    Shared,
    Mutable,
}

/// Zig `lowering_rules.FutureReadiness` (lowering_rules.zig:320).
/// Same local-definition rationale as `RefCellBorrowKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FutureReadiness {
    Unknown,
    Ready,
    Pending,
}

/// Zig `RefCellBorrowValue` (lines 75–79): a live RefCell borrow and
/// the registers releasing it frees.
#[derive(Debug, Clone, PartialEq)]
pub struct RefCellBorrowValue {
    pub cell_reg: u32,
    pub kind: RefCellBorrowKind,
    pub release_regs: Vec<u32>,
}

/// Zig `MutexGuardValue` (lines 84–86): a live `MutexGuard`
/// data-pointer register back to the mutex allocation it locks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MutexGuardValue {
    pub mutex_reg: u32,
}

/// Zig `RwLockGuardValue` (lines 91–94).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RwLockGuardValue {
    pub lock_reg: u32,
    pub is_write: bool,
}

/// Zig `BorrowAddressTempState` (lines 96–100).
#[derive(Debug, Clone, PartialEq)]
pub struct BorrowAddressTempState {
    pub release_regs: Vec<u32>,
    pub restore_slot: Option<u32>,
    pub restore_value: Option<u32>,
}

/// Zig `BranchEmitterStateSnapshot` (lines 102–106): the branch
/// emitter state cloned before a branch and restored/merged after
/// it (E3 territory; the shape is B2).
#[derive(Debug, Clone, Default)]
pub struct BranchEmitterStateSnapshot {
    pub released: HashSet<u32>,
    pub refcell_values: HashMap<u32, RefCellBorrowValue>,
    pub borrow_temps: HashMap<u32, BorrowAddressTempState>,
}

/// Zig `ResultSlotRefCellHandle` (lines 108–111).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultSlotRefCellHandle {
    pub cell_slot: u32,
    pub kind: RefCellBorrowKind,
}

/// Zig `BranchRefCellHandleOwnerMergeSlot` (lines 113–116).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BranchRefCellHandleOwnerMergeSlot {
    pub cell_slot: u32,
    pub kind: RefCellBorrowKind,
}

/// Zig `LoopJumpKind` (lines 118–121).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopJumpKind {
    Break,
    Continue,
}

/// Zig `MacroArgBinding` (lines 123–129). The Zig `ctx` back-pointer
/// to the owning expansion context becomes a shared `Rc` handle.
#[derive(Debug, Clone)]
pub struct MacroArgBinding {
    pub name: String,
    pub arg: Node,
    pub ctx: Option<Rc<RefCell<MacroExpansionContext>>>,
    pub evaluated_reg: Option<u32>,
    pub release_evaluated_reg: bool,
}

/// Zig `MacroLocalBinding` (lines 131–134).
#[derive(Debug, Clone, PartialEq)]
pub struct MacroLocalBinding {
    pub mapped: String,
    pub ty: Option<Type>,
}

/// Zig `MacroLocalChange` (lines 136–140). Zig leaves `old`
/// uninitialized when `had_old` is false; here it is `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct MacroLocalChange {
    pub name: String,
    pub had_old: bool,
    pub old: Option<MacroLocalBinding>,
}

/// Zig `MacroExpansionContext` (lines 142–150).
#[derive(Debug, Clone)]
pub struct MacroExpansionContext {
    pub macro_name: String,
    pub invocation: usize,
    pub local_idx: usize,
    pub args: Vec<MacroArgBinding>,
    pub locals: HashMap<String, MacroLocalBinding>,
    pub local_changes: Vec<MacroLocalChange>,
    pub allocated_names: Vec<String>,
}

/// Zig `EscapedCapture` (lines 152–156).
#[derive(Debug, Clone, PartialEq)]
pub struct EscapedCapture {
    pub name: String,
    pub offset: usize,
    pub ty: Type,
}

/// Zig `EscapedClosureEntry` (lines 158–168).
#[derive(Debug, Clone, PartialEq)]
pub struct EscapedClosureEntry {
    pub worker_name: String,
    pub spawn_name: String,
    pub vtable_name: String,
    pub closure: ClosureLiteral,
    pub ret_ty: Type,
    pub captures: Vec<EscapedCapture>,
    pub slot_size: usize,
    pub inline_join: bool,
}

/// Zig `EscapedCaptureCollector` (lines 170–173): insertion-ordered
/// captures with a name set for dedup. Emission consumes `ordered`
/// (the Vec), never the set — the hash-order discipline holds.
#[derive(Debug, Clone, Default)]
pub struct EscapedCaptureCollector {
    pub ordered: Vec<EscapedCapture>,
    pub seen: HashSet<String>,
}

impl EscapedCaptureCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a capture unless its name was already collected
    /// (the Zig collector's `seen` guard).
    pub fn push(&mut self, capture: EscapedCapture) {
        if self.seen.insert(capture.name.clone()) {
            self.ordered.push(capture);
        }
    }
}

/// Zig `SavedClosureParam` (lines 175–178).
#[derive(Debug, Clone, PartialEq)]
pub struct SavedClosureParam {
    pub name: String,
    pub old: Option<u32>,
}

/// Zig `FieldLayout` (lines 180–183).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldLayout {
    pub offset: usize,
    pub ty: PrimType,
}

/// Zig `AddressSource` (lines 185–189).
#[derive(Debug, Clone, PartialEq)]
pub struct AddressSource {
    pub reg: u32,
    pub release_regs: Vec<u32>,
    pub restore_slot: Option<u32>,
}

/// Zig `StdSurfaceRuleKind` (lines 191–200).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StdSurfaceRuleKind {
    Associated,
    Constructor,
    Function,
    Method,
    FallibleMethod,
    Index,
    IndexAddress,
    IndexAssign,
}

/// Zig `StdSurfaceArgKind` (lines 202–210).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StdSurfaceArgKind {
    Out,
    Ok,
    Receiver,
    Value,
    Index,
    ElemSize,
    ElemTy,
}

/// Zig `StdSurfaceRule` (lines 212–223): one row of the std surface
/// registry (D-group data; the shape is B2).
#[derive(Debug, Clone, PartialEq)]
pub struct StdSurfaceRule {
    pub kind: StdSurfaceRuleKind,
    pub type_name: String,
    pub member_name: Option<String>,
    pub import_path: String,
    pub macro_name: String,
    pub args: Vec<StdSurfaceArgKind>,
    pub deps: Vec<String>,
    pub panic_code: Option<i64>,
    pub consume_value: bool,
}

/// Zig `StdSurfaceRuleOptions` (lines 225–229).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StdSurfaceRuleOptions {
    pub deps: Vec<String>,
    pub panic_code: Option<i64>,
    pub consume_value: bool,
}

/// Zig `StdImportModule` (lines 231–245): a flattened std module
/// kept for fragment splicing (F group). Zig stores the
/// `FlattenResult` plus separately-built sigs; this port stores the
/// four module sections directly — `module()` reassembles exactly
/// the `sab.Module` view the Zig accessor returns.
#[derive(Debug, Clone, PartialEq)]
pub struct StdImportModule {
    pub import_path: String,
    pub symbols: Vec<String>,
    pub function_sigs: Vec<FunctionSig>,
    pub const_decls: Vec<ConstDecl>,
    pub instructions: Vec<Instruction>,
}

impl StdImportModule {
    /// Zig `StdImportModule.module`.
    pub fn module(&self) -> Module {
        Module {
            symbols: self.symbols.clone(),
            function_sigs: self.function_sigs.clone(),
            const_decls: self.const_decls.clone(),
            instructions: self.instructions.clone(),
        }
    }
}

/// Zig `StdMacroTemplate` (lines 247–252).
#[derive(Debug, Clone, PartialEq)]
pub struct StdMacroTemplate {
    pub key: String,
    pub func_name: String,
    pub arg_count: usize,
    pub module: Module,
}

/// Zig `PendingStdDep` (lines 254–257).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingStdDep {
    pub import_path: String,
    pub dep: String,
}

/// Zig `DecodedModuleLocalRemap` (lines 259–293): the register /
/// label remapping built while splicing a decoded std module (F
/// group). All maps are lookup-only; the two order-bearing fields
/// (`reg_order`, `extra_reg_ids`) are `Vec`s as in Zig.
#[derive(Debug, Clone, Default)]
pub struct DecodedModuleLocalRemap {
    pub reg_ids: HashMap<u32, u32>,
    pub reg_order: Vec<u32>,
    pub extra_reg_ids: Vec<u32>,
    pub stable_reg_ids: HashMap<u32, u32>,
    pub source_reg_symbol_ids: HashMap<u32, u32>,
    pub label_ids: HashMap<u32, u32>,
    pub reg_names: HashMap<String, String>,
    pub reg_name_ids: HashMap<String, u32>,
}

impl DecodedModuleLocalRemap {
    /// Zig `DecodedModuleLocalRemap.init` (all collections empty).
    pub fn new() -> Self {
        Self::default()
    }
}

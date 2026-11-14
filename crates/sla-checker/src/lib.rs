//! sla-checker: type / ownership checker subset mirrored from the Zig
//! plugin's `src/type_checker.zig` (6,098 lines) plus the pure decision
//! rules of `src/control_flow_rules.zig` (276 lines).
//!
//! Slice scope (fixtures first): declaration registration, function /
//! test body checking, expression typing, and the ownership state
//! machine's core transitions (implicit let/assignment moves, explicit
//! `^move`, `release`, macro and extern move contracts, await / try
//! consumption). Diagnostics align by KIND with Zig's `TypeError`;
//! named sub-diagnostics that Zig encodes only as message prefixes
//! (LoopConditionalConsume, MacroConditionalConsume) are carried in
//! `Diagnostic::marker`.
//!
//! Divergences from the Zig implementation are NOT silently smoothed
//! over; they are registered in `docs/divergences/checker.md`.
//! The headline one: Zig snapshots branch/loop binding states in a
//! flat map keyed by NAME, conflating shadowed bindings; this crate
//! keys snapshots by binding identity (see scope.rs and D-C1).
//!
//! Wiring: this crate consumes `sla_ast` (path dependency). The
//! checker borrows the AST (`&Node`); side tables key nodes by
//! address, mirroring Zig's `*const ast.Node` keys.

pub mod checker;
pub mod diag;
pub mod rules;
pub mod scope;
pub mod types_util;

pub mod check_arena;
mod check_assoc_helpers;
mod check_binary;
mod check_builtin;
mod check_call_consume;
mod check_borrow_origin;
mod check_go;
mod check_hook;
mod check_shader;
mod check_field_borrow;
mod check_leaves;
mod check_preassoc;
mod check_receiver_leaves;
mod check_receiver_leaves2;
mod check_call;
pub mod check_overload2;
pub mod check_overload3;
pub mod check_extern;
mod check_method;
mod check_literal;
mod check_overload;
mod check_expr;
mod check_expr_util;
mod check_stmt_util;
mod check_stmt_release;
mod checker_util;
mod checker_alias;
mod check_arena_tail;
mod check_call_assoc;
mod check_call_sig;
mod check_overload_extern;
mod check_macro;
mod check_macro_typed;
mod check_flow;
mod check_match;
mod check_pattern;
mod check_program;
mod check_stmt;
mod check_std_ctor;
mod check_std_leaf;
mod check_std_leaf2;
mod check_std_leaf3;
mod check_trait;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_r1;
#[cfg(test)]
mod tests_std_leaf;
#[cfg(test)]
mod tests_e2e;
#[cfg(test)]
mod tests_extern_def;
#[cfg(test)]
mod tests_phase2;
#[cfg(test)]
mod tests_phase2b;
#[cfg(test)]
mod tests_phase2c;
#[cfg(test)]
mod tests_phase2d;
#[cfg(test)]
mod tests_phase3;
#[cfg(test)]
mod tests_phase3b;
#[cfg(test)]
mod tests_phase3c;
#[cfg(test)]
mod tests_phase3d;
#[cfg(test)]
mod tests_phase3e;
#[cfg(test)]
mod tests_r6;
#[cfg(test)]
mod tests_r6b;
#[cfg(test)]
mod tests_r3;

#[cfg(test)]
mod tests_r4;

pub use checker::{
    Checker, EnumVariantInfo, ExternParam, ExternSig, FuncSig, ImportedMacroInfo,
    ParamSig, StructInfo, TraitInfo, TraitMethodInfo,
};
pub use diag::{CheckResult, Diagnostic, ErrorKind};
pub use rules::ValueState;
pub use scope::BindingId;

//! sla-lowering: lowering rules mirrored from `lowering_rules.zig`
//! (9,705 lines) plus the `control_flow_rules.zig` facts that
//! `lowering_rules.zig` re-exports as its own (e.g.
//! `planFunctionTailCleanup = control_flow_rules.planFunctionExitCleanup`,
//! lowering_rules.zig:6120-6121).
//!
//! Lane D scope (vertical slice): the rule families the fixture programs
//! exercise are ported faithfully; every other family is registered in
//! [`deferred`] and its entry points fail loudly with
//! [`LoweringError::Unsupported`] instead of silently approximating the
//! Zig behavior. See `docs/divergences/lowering-text.md` for the
//! divergence log and the covered/deferred inventory.
//!
//! The reachable-set contract stub placed by the coordinator lives in
//! [`reachability`] and is not part of the lowering_rules.zig mirror.

pub mod abi;
pub mod call_args;
pub mod check_info;
pub mod collect;
pub mod control_flow;
pub mod deferred;
pub mod names;
pub mod reachability;
pub mod release_facts;
pub mod scalar_ops;

pub use abi::*;
pub use call_args::*;
pub use check_info::{CheckInfo, ExternParamView, ExternSigView, ImportedMacroView, MapCheckInfo};
pub use collect::collect_repeated_let_bindings;
pub use control_flow::*;
pub use names::*;
pub use release_facts::*;
pub use scalar_ops::*;

use sla_ast::Node;

/// Node identity key shared with the checker: the node's address while
/// the checked tree is alive. The checker lane defines the identical
/// function (`node_key_of`, sla-checker/src/types_util.rs:263-265); it is
/// re-defined here because backends consume upstream data only through
/// sla-lowering (workspace verdict), and the two definitions must stay
/// byte-identical: `node as *const Node as usize`.
pub fn node_key(node: &Node) -> usize {
    node as *const Node as usize
}

/// Loud failure for a lowering rule this lane has not ported. Carries the
/// rule-family name and the Zig source reference so a failure can be
/// classified without guessing (no silent fallback, per house rules).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    Unsupported {
        feature: &'static str,
        zig_ref: &'static str,
    },
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoweringError::Unsupported { feature, zig_ref } => write!(
                f,
                "sla-lowering: unsupported rule `{feature}` (Zig reference: {zig_ref})"
            ),
        }
    }
}

impl std::error::Error for LoweringError {}

/// Convenience constructor for deferred rule families.
pub fn unsupported<T>(feature: &'static str, zig_ref: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported { feature, zig_ref })
}

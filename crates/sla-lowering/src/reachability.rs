//! reachability: placement of sa_plugin_sla/src/plugin_reachability.zig
//! (six-house verdict 2026-11-07, mandatory item 2).
//!
//! The reachable set is computed ONCE here, in sla-lowering, and the SAME set
//! is consumed by both backends (sla-codegen-text and sla-codegen-sab).
//! Copying the computation into a backend, or skipping it, is forbidden:
//! divergent reachable sets were a known text/SAB divergence source.
//! Post-phase-1 this module may be extracted into a sla-reachability crate.
//!
//! Implementation belongs to the lowering lane; this file is the coordinator's
//! placement + contract stub. Ordering contract: iteration order of the set
//! is insertion order (first-reached first), never hash order — the set feeds
//! emission paths in both backends.

use std::collections::BTreeSet;

/// Insertion-ordered set of reachable item names (function / const / global
/// names as they appear in the lowered program).
#[derive(Debug, Default)]
pub struct ReachableSet {
    order: Vec<String>,
    seen: BTreeSet<String>,
}

impl ReachableSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a name; returns true if it was newly reached.
    pub fn insert(&mut self, name: &str) -> bool {
        if self.seen.insert(name.to_string()) {
            self.order.push(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.seen.contains(name)
    }

    /// Reachable names in first-reached order (the only emission-safe view).
    pub fn in_order(&self) -> &[String] {
        &self.order
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

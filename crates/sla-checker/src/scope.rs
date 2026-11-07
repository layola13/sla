//! Lexical scopes and binding state — the ownership ledger.
//!
//! Zig model (type_checker.zig:39-110): `Symbol { name, ty, is_const,
//! state, borrow_source }` lives in a `Scope` = StringHashMap + parent
//! pointer. Branch/loop state snapshots (`saveScopeStates`, line 2716)
//! are ONE FLAT `StringHashMap(ValueState)` keyed by NAME STRING across
//! the whole scope chain: inner scopes are written first, outer scopes
//! then OVERWRITE the same key, and `restoreScopeStates` writes the
//! single saved state back to EVERY same-named binding in EVERY scope.
//! Shadowed bindings are therefore conflated by construction (D-C1).
//!
//! Rust model: every binding gets a stable [`BindingId`] at definition
//! time. Frames keep bindings in declaration order (which also fixes
//! block-exit cleanup order — Zig iterates a hash map, D-C6). Snapshots
//! are keyed by `BindingId`, so a shadowed name snapshots and restores
//! each binding independently.

use crate::rules::ValueState;
use sla_ast::types::Type;
use std::collections::HashMap;

/// Stable identity of one binding for the whole check of one function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindingId(pub u32);

/// Mirrors Zig `Symbol` (type_checker.zig:39), plus the `id` Rust needs
/// to tell shadowed same-name bindings apart.
#[derive(Debug, Clone)]
pub struct Binding {
    pub id: BindingId,
    pub name: String,
    pub ty: Type,
    pub is_const: bool,
    pub state: ValueState,
    /// Name of the binding this value borrows from, when the defining
    /// expression was borrow-like (Zig `borrow_source`).
    pub borrow_source: Option<String>,
    /// R-6: Zig `Symbol.consumed_by_call` — the name of the call that
    /// consumed this binding through a by-value argument, so a later
    /// read can name the consuming call in its UseAfterMove text
    /// (REG-BR-6). Cleared when an assignment reactivates the slot.
    pub consumed_by_call: Option<String>,
    /// R-6: Zig `Symbol.consumed_by_literal` — a description of the
    /// struct-literal field fill that consumed this binding, for the
    /// literal form of the UseAfterMove text (DEF-deep-copy).
    /// Cleared when an assignment reactivates the slot.
    pub consumed_by_literal: Option<String>,
}

/// One lexical frame. Bindings stay in declaration order.
#[derive(Debug, Clone)]
pub struct Frame {
    pub bindings: Vec<Binding>,
    /// `using` module paths visible from this frame (Zig
    /// `Scope.using_modules`), stored NORMALIZED (`::` already
    /// rewritten to `__`, as Zig's checkProgram / checkStmt arms
    /// normalize before appending). Consumed only by the `using`
    /// static-extension resolution in check_method.rs.
    pub using_modules: Vec<String>,
}

impl Frame {
    fn new() -> Frame {
        Frame {
            bindings: Vec::new(),
            using_modules: Vec::new(),
        }
    }

    fn get(&self, name: &str) -> Option<&Binding> {
        // Innermost definition wins within a frame; Zig's per-scope map
        // holds at most one entry per name because `define` rejects
        // same-frame redeclaration before inserting.
        self.bindings.iter().rev().find(|b| b.name == name)
    }

    fn get_mut(&mut self, name: &str) -> Option<&mut Binding> {
        self.bindings.iter_mut().rev().find(|b| b.name == name)
    }
}

/// Snapshot of binding states, keyed by identity — never by name.
/// (Zig: `StringHashMap(ValueState)` keyed by name; see D-C1.)
pub type StateSnapshot = HashMap<BindingId, ValueState>;

/// The scope stack of one in-progress function check (plus the global
/// frame at index 0). Zig threads `*Scope` parent pointers through
/// every call; Rust keeps the chain in one owner so frame indices are
/// stable identities usable for loop-locality tests.
#[derive(Debug)]
pub struct ScopeStack {
    pub frames: Vec<Frame>,
    next_id: u32,
}

impl ScopeStack {
    pub fn new() -> ScopeStack {
        ScopeStack {
            frames: vec![Frame::new()],
            next_id: 0,
        }
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    pub fn push_frame(&mut self) -> usize {
        self.frames.push(Frame::new());
        self.frames.len() - 1
    }

    pub fn pop_frame(&mut self) -> Frame {
        self.frames.pop().expect("scope stack underflow")
    }

    /// Frame index of the innermost binding of `name`, if any.
    pub fn frame_of(&self, name: &str) -> Option<usize> {
        (0..self.frames.len())
            .rev()
            .find(|&i| self.frames[i].get(name).is_some())
    }

    /// Innermost visible binding of `name` (Zig `Scope.lookup`).
    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        (0..self.frames.len())
            .rev()
            .find_map(|i| self.frames[i].get(name))
    }

    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut Binding> {
        let idx = self.frame_of(name)?;
        self.frames[idx].get_mut(name)
    }

    pub fn lookup_id(&self, id: BindingId) -> Option<&Binding> {
        self.frames
            .iter()
            .flat_map(|f| f.bindings.iter())
            .find(|b| b.id == id)
    }

    /// Define in the CURRENT frame. Returns `false` when the name is
    /// already defined in this frame (Zig `Scope.define` checks only
    /// the local frame; shadowing an outer frame is legal).
    pub fn define(
        &mut self,
        name: &str,
        ty: Type,
        is_const: bool,
        state: ValueState,
        borrow_source: Option<String>,
    ) -> Option<BindingId> {
        let frame = self.frames.last_mut().expect("scope stack empty");
        if frame.get(name).is_some() {
            return None;
        }
        let id = BindingId(self.next_id);
        self.next_id += 1;
        frame.bindings.push(Binding {
            id,
            name: name.to_string(),
            ty,
            is_const,
            state,
            borrow_source,
            consumed_by_call: None,
            consumed_by_literal: None,
        });
        Some(id)
    }

    /// Snapshot the states of ALL visible bindings (Zig
    /// `saveScopeStates`). Keyed by `BindingId`: shadowed bindings keep
    /// separate entries — the exact point where Zig's name-keyed flat
    /// map loses one of them (D-C1).
    pub fn save_states(&self) -> StateSnapshot {
        let mut snap = StateSnapshot::new();
        for frame in &self.frames {
            for b in &frame.bindings {
                snap.insert(b.id, b.state);
            }
        }
        snap
    }

    /// Restore states from a snapshot (Zig `restoreScopeStates`).
    /// Bindings defined after the snapshot was taken are untouched.
    pub fn restore_states(&mut self, snap: &StateSnapshot) {
        for frame in &mut self.frames {
            for b in &mut frame.bindings {
                if let Some(state) = snap.get(&b.id) {
                    b.state = *state;
                }
            }
        }
    }

    /// Zig `restoreUninitializedFromSaved` (type_checker.zig:2777):
    /// only bindings whose saved state was `uninitialized` roll back;
    /// every other state keeps its post-body value.
    pub fn restore_uninitialized_from_saved(&mut self, snap: &StateSnapshot) {
        for frame in &mut self.frames {
            for b in &mut frame.bindings {
                if let Some(saved) = snap.get(&b.id) {
                    if *saved == ValueState::Uninitialized {
                        b.state = ValueState::Uninitialized;
                    }
                }
            }
        }
    }
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

//! Stage-1 field-borrow ledger (R-6 family F3), mirroring the
//! `FieldPlace` / `FieldBorrow` machinery of the Zig checker
//! (def-deepcopy staging type_checker.zig:518-600, 2004-2075).
//!
//! A field borrow (`let x = &s.b`) is registered against the PLACE
//! it borrows (root binding + field/index/deref path). While the
//! borrower binding stays active, the checker rejects:
//!   * a second borrow/move of an overlapping place (borrow_expr /
//!     move_expr arms, check_expr.rs);
//!   * a direct write to an overlapping place (assign arm);
//!   * a reassignment or move of the borrowed ROOT (assign / let
//!     arms).
//! Places overlap when they share a root and one path is a prefix
//! of the other; a dynamic-index segment (`*`) matches any single
//! segment. Disjoint field borrows are legal.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use sla_ast::exprs::Literal;
use sla_ast::node::Node;
use sla_ast::types::Type;

/// A place: a root binding plus a path of segments from root to
/// leaf. Segments are field names, `"@"` for a deref step, `"*"`
/// for a dynamic index, or decimal digits for a literal index.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldPlace {
    pub root: String,
    pub segs: Vec<String>,
}

/// One registered field borrow (Zig `FieldBorrow`): the borrowed
/// place, the borrower binding's name, and the frame index that
/// defined the borrower (the def-scope identity; liveness requires
/// that exact frame's binding to still be active).
#[derive(Debug, Clone)]
pub struct FieldBorrow {
    pub place: FieldPlace,
    pub borrower: String,
    pub def_frame: usize,
}

/// Compute the place an expression denotes, if any (Zig
/// `placeOfExpr`): walks field/index/deref chains, skipping
/// borrow/move wrappers, down to an identifier root.
pub fn place_of_expr(node: &Node) -> Option<FieldPlace> {
    fn walk(node: &Node, segs: &mut Vec<String>) -> Option<String> {
        match node {
            Node::Identifier(name) => Some(name.clone()),
            Node::FieldExpr(f) => {
                let root = walk(&f.expr, segs)?;
                segs.push(f.field_name.clone());
                Some(root)
            }
            Node::IndexExpr(i) => {
                let root = walk(&i.target, segs)?;
                let seg = match &*i.index {
                    Node::Literal(Literal::IntVal(v)) => v.to_string(),
                    _ => "*".to_string(),
                };
                segs.push(seg);
                Some(root)
            }
            Node::DerefExpr(d) => {
                let root = walk(&d.expr, segs)?;
                segs.push("@".to_string());
                Some(root)
            }
            Node::BorrowExpr(b) => walk(&b.expr, segs),
            Node::MoveExpr(m) => walk(&m.expr, segs),
            _ => None,
        }
    }
    let mut segs = Vec::new();
    let root = walk(node, &mut segs)?;
    segs.reverse();
    Some(FieldPlace { root, segs })
}

fn segs_match(a: &str, b: &str) -> bool {
    a == b || a == "*" || b == "*"
}

fn is_prefix(short: &[String], long: &[String]) -> bool {
    short.len() <= long.len()
        && short
            .iter()
            .zip(long.iter())
            .all(|(a, b)| segs_match(a, b))
}

/// Zig `fieldPlacesOverlap`: same root, and one path is a prefix
/// of the other (`*` matches any single segment).
pub fn places_overlap(a: &FieldPlace, b: &FieldPlace) -> bool {
    a.root == b.root && (is_prefix(&a.segs, &b.segs) || is_prefix(&b.segs, &a.segs))
}

/// Zig `fieldPlaceDesc`: dotted rendering (`@` -> `.*`,
/// `*` -> `[_]`).
pub fn place_desc(place: &FieldPlace) -> String {
    let mut out = place.root.clone();
    for seg in &place.segs {
        match seg.as_str() {
            "@" => out.push_str(".*"),
            "*" => out.push_str("[_]"),
            other => {
                out.push('.');
                out.push_str(other);
            }
        }
    }
    out
}

impl Checker {
    /// Zig `fieldBorrowIsLive`: the borrower's defining frame still
    /// holds that binding, active.
    fn field_borrow_is_live(&self, entry: &FieldBorrow) -> bool {
        self.scopes.frame_of(&entry.borrower) == Some(entry.def_frame)
            && self
                .scopes
                .lookup(&entry.borrower)
                .map(|b| b.state == ValueState::Active)
                .unwrap_or(false)
    }

    /// Zig `liveFieldBorrowConflict`: the first live registered
    /// borrow whose place overlaps `place`.
    pub fn live_field_borrow_conflict(&self, place: &FieldPlace) -> Option<FieldBorrow> {
        self.field_borrows
            .iter()
            .find(|e| self.field_borrow_is_live(e) && places_overlap(&e.place, place))
            .cloned()
    }

    /// Zig `rootHasLiveFieldBorrow`: the first live registered
    /// borrow rooted at `root`.
    pub fn root_has_live_field_borrow(&self, root: &str) -> Option<FieldBorrow> {
        self.field_borrows
            .iter()
            .find(|e| e.place.root == root && self.field_borrow_is_live(e))
            .cloned()
    }

    /// Zig `dropFieldBorrowsOf`: forget every borrow registered to
    /// the binding that currently defines `borrower`.
    pub fn drop_field_borrows_of(&mut self, borrower: &str) {
        let Some(def_frame) = self.scopes.frame_of(borrower) else {
            return;
        };
        self.field_borrows
            .retain(|e| !(e.borrower == borrower && e.def_frame == def_frame));
    }

    /// Zig `registerFieldBorrow`: a fresh registration replaces the
    /// borrower's previous one.
    pub fn register_field_borrow(&mut self, place: FieldPlace, borrower: &str) {
        self.drop_field_borrows_of(borrower);
        if let Some(def_frame) = self.scopes.frame_of(borrower) {
            self.field_borrows.push(FieldBorrow {
                place,
                borrower: borrower.to_string(),
                def_frame,
            });
        }
    }

    /// Zig `borrowPlaceOfValue`: the place a value expression
    /// borrows from, if it denotes one — a borrow expression's inner
    /// place, an identifier's registered borrow (or its recorded
    /// borrow source as a root-only place), or a field/index place.
    pub fn borrow_place_of_value(&self, value: &Node) -> Option<FieldPlace> {
        match value {
            Node::BorrowExpr(b) => place_of_expr(&b.expr),
            Node::Identifier(name) => {
                if let Some(entry) = self
                    .field_borrows
                    .iter()
                    .find(|e| e.borrower == *name && self.field_borrow_is_live(e))
                {
                    return Some(entry.place.clone());
                }
                let binding = self.scopes.lookup(name)?;
                binding.borrow_source.as_ref().map(|src| FieldPlace {
                    root: src.clone(),
                    segs: Vec::new(),
                })
            }
            Node::FieldExpr(_) | Node::IndexExpr(_) => place_of_expr(value),
            _ => None,
        }
    }

    // -- statement/expression gates (Zig call sites) ----------------

    fn borrow_conflict(message: String) -> Diagnostic {
        Diagnostic::marked(ErrorKind::InvalidBorrow, "BorrowConflict", message)
    }

    /// borrow_expr arm (staging :7587): a fresh borrow of a place
    /// that is already borrowed live is rejected.
    pub fn check_borrow_place_free(&self, place: &FieldPlace) -> CheckResult<()> {
        if let Some(conflict) = self.live_field_borrow_conflict(place) {
            return Err(Self::borrow_conflict(format!(
                "BorrowConflict: cannot borrow `{}` while `{}` is still borrowed by `{}`; disjoint field borrows are allowed, overlapping field borrows are not",
                place_desc(place),
                place_desc(&conflict.place),
                conflict.borrower
            )));
        }
        Ok(())
    }

    /// move_expr arm (staging :7606): moving a place that is
    /// borrowed live is rejected.
    pub fn check_move_place_free(&self, place: &FieldPlace) -> CheckResult<()> {
        if let Some(conflict) = self.live_field_borrow_conflict(place) {
            return Err(Self::borrow_conflict(format!(
                "BorrowConflict: cannot move `{}` while `{}` is still borrowed by `{}`",
                place_desc(place),
                place_desc(&conflict.place),
                conflict.borrower
            )));
        }
        Ok(())
    }

    /// let arm (staging :6943): moving a root binding into a new
    /// let while one of its fields is borrowed live is rejected.
    pub fn check_let_move_root_free(&self, src: &str, name: &str) -> CheckResult<()> {
        if let Some(conflict) = self.root_has_live_field_borrow(src) {
            return Err(Self::borrow_conflict(format!(
                "BorrowConflict: cannot move `{src}` into `{name}` while `{}` is still borrowed by `{}`",
                place_desc(&conflict.place),
                conflict.borrower
            )));
        }
        Ok(())
    }

    /// assign arm, identifier target (staging :7053): reassigning
    /// a root with a live field borrow is rejected; otherwise the
    /// assignment drops the target's own registered borrows.
    pub fn check_assign_root_free(&mut self, name: &str) -> CheckResult<()> {
        if let Some(conflict) = self.root_has_live_field_borrow(name) {
            return Err(Self::borrow_conflict(format!(
                "BorrowConflict: cannot reassign `{name}` while `{}` is still borrowed by `{}`",
                place_desc(&conflict.place),
                conflict.borrower
            )));
        }
        self.drop_field_borrows_of(name);
        Ok(())
    }

    /// assign arm, place target (staging :7067): a direct write to
    /// a place borrowed live is rejected. Deref targets never
    /// reach here (writing through a borrow is its own use).
    pub fn check_assign_place_free(&self, target: &Node) -> CheckResult<()> {
        if let Some(place) = place_of_expr(target) {
            if let Some(conflict) = self.live_field_borrow_conflict(&place) {
                return Err(Self::borrow_conflict(format!(
                    "BorrowConflict: cannot assign to `{}` while `{}` is still borrowed by `{}`",
                    place_desc(&place),
                    place_desc(&conflict.place),
                    conflict.borrower
                )));
            }
        }
        Ok(())
    }

    /// Registration shared by the let arm (staging :6952) and the
    /// assign tail (:7170): when the defined value denotes a
    /// borrowed place, register it against the new borrower.
    /// Zig gates this on `isBorrowLikeType(declared_ty)`: a plain
    /// struct-field or element READ (`let t = r[i]`, i64) denotes a
    /// place in `place_of_expr` but is a VALUE copy, not a borrow —
    /// registering it made later writes to that place false-positive
    /// BorrowConflicts (corpus test_unit_bend2_parity_array /
    /// _list / vec_push_pod / field_compare: `let t = r[i]` then
    /// `r[i] = ...` rejected). Only a genuinely borrow-like declared
    /// type registers.
    pub fn register_borrow_from_value(
        &mut self,
        name: &str,
        value: &Node,
        declared_ty: &Type,
    ) {
        if !crate::types_util::is_borrow_like_type(declared_ty) {
            return;
        }
        if let Some(place) = self.borrow_place_of_value(value) {
            self.register_field_borrow(place, name);
        }
    }
}

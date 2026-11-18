//! View-types stage 1: pairwise place-overlap validation for one
//! call's arguments — port of the Zig checker's
//! `checkCallArgPlaceOverlaps` (sla-integ-85b2
//! type_checker.zig:2289), which the Rust checker previously lacked
//! entirely: the stage-1 field-borrow ledger
//! (`check_field_borrow`) only registers borrows bound by let/assign,
//! so two inline call-argument borrows of overlapping places never
//! saw each other and `sla check` accepted them (corpus
//! test_error_view_args_same_field / _prefix_overlap /
//! _field_and_whole).
//!
//! Borrowed places come from explicit borrow expressions and from
//! plain places passed to borrow parameters. Overlapping borrows of
//! prefix-related (but not identical) places are always rejected;
//! identical places are rejected unless both corresponding
//! parameters are provably read-only in the callee body
//! (`paramMayBeWritten`, type_checker.zig:2083-2240); a borrowed
//! place may not overlap any plain place argument either — except
//! that a plain argument which is a by-value read of a Copy-typed
//! place is not a move and never conflicts (`plainArgIsCopyRead`,
//! type_checker.zig:2272, decided by `placeExprType`,
//! type_checker.zig:2243); and a by-value move of a root may not
//! happen while a field of it is borrowed by a live local.

use crate::check_field_borrow::{
    place_desc, place_of_expr, places_overlap, FieldPlace,
};
use crate::checker::{Checker, ParamSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    array_type_of, btree_map_types, grow_vec_element_type, hash_map_types,
    is_borrow_like_type, is_string_like_type, peel_borrow_pointer,
    slice_element_type, vec_deque_element_type, vec_element_type,
};
use sla_ast::decls::FuncDecl;
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

fn borrow_conflict(message: String) -> Diagnostic {
    Diagnostic::marked(ErrorKind::BorrowConflict, "BorrowConflict", message)
}

/// Zig-ordered place for one expression. The pre-existing
/// `place_of_expr` (check_field_borrow) reverses the segment list
/// once more than Zig `placeOfExpr` (type_checker.zig:511-547:
/// leaf-first walk + one reverse = root-to-leaf order), so ledger
/// places carry leaf-to-root segments and prefix relations at
/// depth >= 1 never test as overlapping there. That representation
/// is the ledger's own status quo and is deliberately NOT changed
/// by this line; the call gate needs true Zig place semantics for
/// its within-call comparisons, so it un-reverses. (Arm 1 below
/// compares against the ledger in the ledger's own representation,
/// keeping its verdicts identical to the borrow_expr arm's.)
fn place_of_expr_ordered(node: &Node) -> Option<FieldPlace> {
    let mut p = place_of_expr(node)?;
    p.segs.reverse();
    Some(p)
}

/// Zig `fieldPlacesIdentical`: exact root + segment equality (the
/// `*` dynamic-index segment does NOT wildcard here).
fn places_identical(a: &FieldPlace, b: &FieldPlace) -> bool {
    a.root == b.root && a.segs == b.segs
}

/// Zig `placeRootName`: the root binding of a place expression,
/// walking field / index / deref / borrow / move wrappers.
fn place_root_name(node: &Node) -> Option<&str> {
    match node {
        Node::Identifier(name) => Some(name),
        Node::FieldExpr(f) => place_root_name(&f.expr),
        Node::IndexExpr(i) => place_root_name(&i.target),
        Node::DerefExpr(d) => place_root_name(&d.expr),
        Node::BorrowExpr(b) => place_root_name(&b.expr),
        Node::MoveExpr(m) => place_root_name(&m.expr),
        _ => None,
    }
}

/// Zig `rootIdentifier`: the root binding of an identifier /
/// field / index chain (no deref or wrapper steps).
fn root_identifier(node: &Node) -> Option<&str> {
    match node {
        Node::Identifier(name) => Some(name),
        Node::IndexExpr(i) => root_identifier(&i.target),
        Node::FieldExpr(f) => root_identifier(&f.expr),
        _ => None,
    }
}

/// Zig `nodeMayWriteThroughParam`: conservative syntactic scan —
/// true when the node may write through the named parameter.
/// Node kinds without an arm fall through to `true`, exactly like
/// the Zig `else => return true`.
fn node_may_write_through_param(node: &Node, name: &str) -> bool {
    match node {
        Node::Identifier(_) | Node::Literal(_) => false,
        Node::AssignStmt(a) => {
            if place_root_name(&a.target) == Some(name) {
                return true;
            }
            node_may_write_through_param(&a.target, name)
                || node_may_write_through_param(&a.value, name)
        }
        Node::CallExpr(c) => {
            if let Some(callee) = &c.callee {
                if node_may_write_through_param(callee, name) {
                    return true;
                }
            }
            for arg in &c.args {
                if root_identifier(arg) == Some(name) {
                    return true;
                }
                if node_may_write_through_param(arg, name) {
                    return true;
                }
            }
            false
        }
        Node::BorrowExpr(b) => {
            place_root_name(&b.expr) == Some(name)
                || node_may_write_through_param(&b.expr, name)
        }
        Node::MoveExpr(m) => {
            place_root_name(&m.expr) == Some(name)
                || node_may_write_through_param(&m.expr, name)
        }
        Node::LetStmt(l) => node_may_write_through_param(&l.value, name),
        Node::ConstStmt(c) => node_may_write_through_param(&c.value, name),
        Node::LetElseStmt(l) => {
            node_may_write_through_param(&l.value, name)
                || l.else_block
                    .iter()
                    .any(|s| node_may_write_through_param(s, name))
        }
        Node::ExprStmt(e) => node_may_write_through_param(e, name),
        Node::ReturnStmt(r) => r
            .value
            .as_ref()
            .map_or(false, |v| node_may_write_through_param(v, name)),
        Node::BlockStmt(b) => b
            .body
            .iter()
            .any(|s| node_may_write_through_param(s, name)),
        Node::IfExpr(e) => {
            node_may_write_through_param(&e.cond, name)
                || e.then_block
                    .iter()
                    .any(|s| node_may_write_through_param(s, name))
                || e.else_block.as_ref().map_or(false, |eb| {
                    eb.iter().any(|s| node_may_write_through_param(s, name))
                })
        }
        Node::WhileStmt(w) => {
            node_may_write_through_param(&w.cond, name)
                || w.body
                    .iter()
                    .any(|s| node_may_write_through_param(s, name))
        }
        Node::ForStmt(f) => {
            node_may_write_through_param(&f.start, name)
                || f.end
                    .as_ref()
                    .map_or(false, |e| node_may_write_through_param(e, name))
                || f.body
                    .iter()
                    .any(|s| node_may_write_through_param(s, name))
        }
        Node::BinaryExpr(b) => {
            node_may_write_through_param(&b.left, name)
                || node_may_write_through_param(&b.right, name)
        }
        Node::FieldExpr(f) => node_may_write_through_param(&f.expr, name),
        Node::IndexExpr(i) => {
            node_may_write_through_param(&i.target, name)
                || node_may_write_through_param(&i.index, name)
        }
        Node::DerefExpr(d) => node_may_write_through_param(&d.expr, name),
        Node::CastExpr(c) => node_may_write_through_param(&c.expr, name),
        Node::TryExpr(t) => node_may_write_through_param(&t.expr, name),
        Node::StructLiteral(lit) => {
            lit.update_expr
                .as_ref()
                .map_or(false, |u| node_may_write_through_param(u, name))
                || lit.fields
                    .iter()
                    .any(|fld| node_may_write_through_param(&fld.value, name))
        }
        Node::TupleLiteral(t) => t
            .elements
            .iter()
            .any(|e| node_may_write_through_param(e, name)),
        Node::ArrayLiteral(a) => a
            .elements
            .iter()
            .any(|e| node_may_write_through_param(e, name)),
        Node::ClosureLiteral(c) => node_may_write_through_param(&c.body, name),
        Node::DeferStmt(d) => node_may_write_through_param(&d.call, name),
        _ => true,
    }
}

/// Zig `paramMayBeWritten`, per parameter of one declaration: a
/// decl-only (bodyless) function may write through every parameter.
/// Computed at signature-registration time and stored on
/// `FuncSig::param_may_write`; an empty vector means "unknown"
/// (signature built without a declaration) and is treated as
/// may-write, matching the Zig gate's callee-unknown behaviour.
pub fn param_may_write_flags(func: &FuncDecl) -> Vec<bool> {
    if func.is_decl_only {
        return vec![true; func.params.len()];
    }
    func.params
        .iter()
        .map(|p| {
            func.body
                .iter()
                .any(|s| node_may_write_through_param(s, &p.name))
        })
        .collect()
}

impl Checker {
    /// Zig `placeExprType`: pure (side-effect-free) type of a place
    /// expression, resolved from scope bindings and struct
    /// declarations only. `None` means "unknown", never a concrete
    /// type.
    fn place_expr_type(&self, node: &Node) -> Option<Type> {
        match node {
            Node::Identifier(name) => self.lookup_ty(name),
            Node::MoveExpr(m) => self.place_expr_type(&m.expr),
            Node::FieldExpr(f) => {
                let target_ty = self.place_expr_type(&f.expr)?;
                if f.field_name == "len" && is_string_like_type(&target_ty) {
                    return Some(Type::Primitive(Primitive::U64));
                }
                match peel_borrow_pointer(&target_ty).clone() {
                    Type::UserDefined(u) => {
                        let info = self.structs.get(&u.name)?;
                        if info.is_opaque {
                            return None;
                        }
                        info.fields
                            .iter()
                            .find(|(n, _)| *n == f.field_name)
                            .map(|(_, t)| t.clone())
                    }
                    Type::Tuple(t) => {
                        let index: usize = f.field_name.parse().ok()?;
                        t.elems.get(index).cloned()
                    }
                    _ => None,
                }
            }
            Node::IndexExpr(i) => {
                let target_ty = self.place_expr_type(&i.target)?;
                if let Some((_, v)) = hash_map_types(&target_ty) {
                    return Some(v);
                }
                if let Some((_, v)) = btree_map_types(&target_ty) {
                    return Some(v);
                }
                if let Some(e) = vec_element_type(&target_ty) {
                    return Some(e);
                }
                if let Some(e) = grow_vec_element_type(&target_ty) {
                    return Some(e);
                }
                if let Some(e) = vec_deque_element_type(&target_ty) {
                    return Some(e);
                }
                if let Some(e) = slice_element_type(&target_ty) {
                    return Some(e);
                }
                if let Some((e, _)) = array_type_of(&target_ty) {
                    return Some(e);
                }
                None
            }
            Node::DerefExpr(d) => {
                let inner_ty = self.place_expr_type(&d.expr)?;
                match inner_ty {
                    Type::Pointer(p) => Some((*p).clone()),
                    Type::Borrow(b) => Some((*b).clone()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Zig `plainArgIsCopyRead`: a plain (by-value) argument whose
    /// place has a Copy type is a pure read — evaluating it produces
    /// an independent value copy, so it cannot alias or invalidate a
    /// borrow of an overlapping place taken by the same call. Only
    /// non-Copy places are true by-value moves.
    fn plain_arg_is_copy_read(&self, arg: &Node) -> bool {
        match self.place_expr_type(arg) {
            Some(ty) => self.type_is_copy(&ty),
            None => false,
        }
    }

    /// Zig `checkCallArgPlaceOverlaps` (borrowed-mask is always 0 on
    /// the signature / method paths ported here; the Zig macro call
    /// sites are not ported — macro expansions re-check their inner
    /// calls through these gated paths). `params` are the callee's
    /// declared parameters; `may_write` is the callee's
    /// `FuncSig::param_may_write` (empty = unknown = may-write).
    pub fn check_call_arg_place_overlaps(
        &self,
        name: &str,
        params: &[ParamSig],
        may_write: &[bool],
        args: &[Node],
    ) -> CheckResult<()> {
        #[derive(Default, Clone)]
        struct ArgPlace {
            borrowed: Option<FieldPlace>,
            plain: Option<FieldPlace>,
        }
        let mut places: Vec<ArgPlace> = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            let mut ap = ArgPlace::default();
            if let Node::BorrowExpr(b) = arg {
                ap.borrowed = place_of_expr_ordered(&b.expr);
            } else if let Some(p) = place_of_expr_ordered(arg) {
                let is_borrowed = i < params.len()
                    && (matches!(params[i].ty, Type::Borrow(_))
                        || params[i].is_borrow);
                if is_borrowed {
                    ap.borrowed = Some(p);
                } else if !self.plain_arg_is_copy_read(arg) {
                    ap.plain = Some(p);
                }
            }
            places.push(ap);
        }
        // Borrowed places against live local borrows (compared in
        // the ledger's own segment representation; the rendered
        // conflict place is un-reversed back for the message).
        for ap in &places {
            if let Some(bp) = &ap.borrowed {
                let mut ledger_place = bp.clone();
                ledger_place.segs.reverse();
                if let Some(conflict) =
                    self.live_field_borrow_conflict(&ledger_place)
                {
                    let mut conflict_ordered = conflict.place.clone();
                    conflict_ordered.segs.reverse();
                    return Err(borrow_conflict(format!(
                        "BorrowConflict: call to {name} borrows `{}` while `{}` is still borrowed by `{}`",
                        place_desc(bp),
                        place_desc(&conflict_ordered),
                        conflict.borrower
                    )));
                }
            }
        }
        // Borrowed against borrowed / plain within the call.
        for (i, a) in places.iter().enumerate() {
            let Some(ab) = &a.borrowed else { continue };
            for (j, b) in places.iter().enumerate() {
                if i == j {
                    continue;
                }
                if let Some(bb) = &b.borrowed {
                    if j < i {
                        continue;
                    }
                    if !places_overlap(ab, bb) {
                        continue;
                    }
                    if places_identical(ab, bb) {
                        let read_only = i < params.len()
                            && j < params.len()
                            && i < may_write.len()
                            && j < may_write.len()
                            && !may_write[i]
                            && !may_write[j];
                        if read_only {
                            continue;
                        }
                    }
                    return Err(borrow_conflict(format!(
                        "BorrowConflict: call to {name} borrows overlapping places `{}` and `{}` in the same call",
                        place_desc(ab),
                        place_desc(bb)
                    )));
                }
                if let Some(bp) = &b.plain {
                    if places_overlap(ab, bp) {
                        return Err(borrow_conflict(format!(
                            "BorrowConflict: call to {name} borrows `{}` while also passing overlapping place `{}` in the same call",
                            place_desc(ab),
                            place_desc(bp)
                        )));
                    }
                }
            }
        }
        // By-value move of a root while a field of it is borrowed.
        for (arg, ap) in args.iter().zip(places.iter()) {
            if ap.plain.is_none() {
                continue;
            }
            let Node::Identifier(root) = arg else { continue };
            let Some(ty) = self.lookup_ty(root) else { continue };
            if self.type_is_copy(&ty) || is_borrow_like_type(&ty) {
                continue;
            }
            if let Some(conflict) = self.root_has_live_field_borrow(root) {
                return Err(borrow_conflict(format!(
                    "BorrowConflict: call to {name} moves `{root}` while `{}` is still borrowed by `{}`",
                    place_desc(&conflict.place),
                    conflict.borrower
                )));
            }
        }
        Ok(())
    }
}

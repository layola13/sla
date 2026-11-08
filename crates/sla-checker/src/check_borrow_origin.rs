//! R-3: borrow-origin annotations (E_res corpus) — the trailing
//! `-> &[T][name]` return annotation and its five gates. The
//! surface postdates the mirrored Zig tree; the corpus files
//! (`neg_borrow_*` / `test_unit_borrow_return_views`) and their
//! `// expect:` lines are the authority:
//!
//! * **BorrowAnnotationInvalid** — the annotation names
//!   something that is not one of the function's own parameters.
//! * **BorrowEscape** — a view is returned from a function whose
//!   return carries no (matching) origin annotation, or a view is
//!   stored into a struct field.
//! * **BorrowOriginNotABinding** — the argument fed to the
//!   annotated origin parameter is not a place (binding).
//! * **BorrowOriginMerge** — one binding merges views of two
//!   different owners (the if/else rebind shape).
//! * **BorrowUseAfterInvalidation** — a view is read after a
//!   may-invalidate event on its owner: moving the owner, freeing
//!   it through a consuming (`^`) call, or passing the owner to
//!   ANY other call (conservative default for unknown effects;
//!   the producing call itself and further view-producing calls
//!   on the same owner are the exceptions the positive corpus
//!   exercises).
//!
//! Tracking is deliberately flow-light: views are a flat
//! function-local ledger (binding -> owner), invalidations are a
//! name set, and branch joins detect the merge shape by
//! snapshotting the ledger around `if` expressions. Views whose
//! owner is a function PARAMETER are caller-owned: returning them
//! is governed by the callee's own annotation, and invalidation
//! events do not apply to them inside this function.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use sla_ast::decls::FuncDecl;
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::Type;

/// Is this type a borrow (`&T`)? Views are exactly the values
/// whose type is a borrow produced by an annotated call.
fn is_borrow_ty(ty: &Type) -> bool {
    matches!(ty, Type::Borrow(_))
}

/// The root binding of a place expression (identifier, field and
/// index chains), or `None` for temporaries and computed values.
pub fn place_root(node: &Node) -> Option<String> {
    match node {
        Node::Identifier(n) => Some(n.clone()),
        Node::BorrowExpr(b) => place_root(&b.expr),
        // An explicit move (`^x`) still names its place — the
        // consuming-call invalidation gate needs the root (the
        // origin-parameter binding gate treats it via the move
        // arm of `place_root_strict` instead).
        Node::MoveExpr(m) => place_root(&m.expr),
        Node::FieldExpr(f) => place_root(&f.expr),
        Node::IndexExpr(i) => place_root(&i.target),
        _ => None,
    }
}

/// The strict place root for the origin-parameter binding gate:
/// an explicit move is a value, not a borrowable place, so it
/// does NOT satisfy the annotated parameter (corpus:
/// `data(^owner)`-shaped arguments are not the borrow form).
pub fn place_root_strict(node: &Node) -> Option<String> {
    match node {
        Node::Identifier(n) => Some(n.clone()),
        Node::BorrowExpr(b) => place_root_strict(&b.expr),
        Node::FieldExpr(f) => place_root_strict(&f.expr),
        Node::IndexExpr(i) => place_root_strict(&i.target),
        _ => None,
    }
}

/// Does this expression mention binding `name` anywhere (the
/// coarse read test for invalidated views)?
fn expr_mentions(node: &Node, name: &str) -> bool {
    match node {
        Node::Identifier(n) => n == name,
        Node::BorrowExpr(b) => expr_mentions(&b.expr, name),
        Node::DerefExpr(d) => expr_mentions(&d.expr, name),
        Node::MoveExpr(m) => expr_mentions(&m.expr, name),
        Node::FieldExpr(f) => expr_mentions(&f.expr, name),
        Node::IndexExpr(i) => {
            expr_mentions(&i.target, name) || expr_mentions(&i.index, name)
        }
        Node::BinaryExpr(b) => {
            expr_mentions(&b.left, name) || expr_mentions(&b.right, name)
        }
        Node::CallExpr(c) => c.args.iter().any(|a| expr_mentions(a, name)),
        _ => false,
    }
}

impl Checker {
    /// Reset the function-local origin ledger at function entry.
    pub fn origin_reset(&mut self) {
        self.origin_views.clear();
        self.origin_invalidated.clear();
    }

    /// Look up a binding's view owner.
    pub fn origin_owner_of(&self, name: &str) -> Option<String> {
        self.origin_views
            .iter()
            .rev()
            .find(|(v, _)| v == name)
            .map(|(_, o)| o.clone())
    }

    fn origin_register(&mut self, view: &str, owner: &str) {
        if let Some(slot) = self
            .origin_views
            .iter_mut()
            .find(|(v, _)| v == view)
        {
            slot.1 = owner.to_string();
        } else {
            self.origin_views.push((view.to_string(), owner.to_string()));
        }
        self.origin_invalidated.remove(view);
    }

    fn origin_invalidate_owner(&mut self, owner: &str) {
        for (view, o) in &self.origin_views {
            if o == owner {
                self.origin_invalidated.insert(view.clone());
            }
        }
    }

    /// Declaration gate: the annotation must name one of the
    /// function's own parameters.
    pub fn origin_check_decl(&self, func: &FuncDecl) -> CheckResult<()> {
        if let Some(origin) = &func.ret_borrow_origin {
            if !func.params.iter().any(|p| &p.name == origin) {
                return Err(Diagnostic::new(
                    ErrorKind::BorrowAnnotationInvalid,
                    format!(
                        "BorrowAnnotationInvalid: origin `{origin}` of `{}` is not one of its parameters",
                        func.name
                    ),
                ));
            }
        }
        // A_min scope limit: `T!` parameters are rejected loudly.
        for param in &func.params {
            if param.fallible_sugar {
                return Err(Diagnostic::new(
                    ErrorKind::FallibleSugarParam,
                    format!(
                        "FallibleSugarParam: parameter `{}` of `{}` uses the `!` sugar, which is return-position only",
                        param.name, func.name
                    ),
                ));
            }
        }
        Ok(())
    }

    /// The origin (owner binding name) a call result carries, if
    /// the callee's return is annotated: the argument bound to the
    /// annotated parameter determines the owner. A non-place
    /// argument is BorrowOriginNotABinding.
    pub fn origin_call_owner(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Option<String>> {
        let sig = self
            .funcs
            .get(&call.func_name)
            .or_else(|| self.imported_funcs.get(&call.func_name))
            .cloned();
        let Some(sig) = sig else { return Ok(None) };
        let Some(origin_param) = sig.ret_borrow_origin.clone() else {
            return Ok(None);
        };
        let Some(idx) = sig.params.iter().position(|p| p.name == origin_param)
        else {
            return Ok(None);
        };
        let Some(arg) = call.args.get(idx) else {
            return Ok(None);
        };
        match place_root_strict(arg) {
            Some(root) => Ok(Some(root)),
            None => Err(Diagnostic::new(
                ErrorKind::BorrowOriginNotABinding,
                format!(
                    "BorrowOriginNotABinding: argument for origin parameter `{origin_param}` of `{}` must be a binding",
                    call.func_name
                ),
            )),
        }
    }

    /// After a call has been checked: (1) register its result view
    /// against the destination binding happens in the let/assign
    /// hooks — this hook applies the may-invalidate rule to the
    /// call's ARGUMENTS: passing an owner (by borrow or by move)
    /// to a call that does not itself produce a view of that owner
    /// invalidates outstanding views of it; a consuming (`^`)
    /// argument invalidates unconditionally.
    pub fn origin_after_call(&mut self, call: &CallExpr) {
        // Owners this call produces views of are exempt.
        let produced: Option<String> = self
            .funcs
            .get(&call.func_name)
            .or_else(|| self.imported_funcs.get(&call.func_name))
            .and_then(|sig| sig.ret_borrow_origin.clone())
            .and_then(|origin_param| {
                let sig = self
                    .funcs
                    .get(&call.func_name)
                    .or_else(|| self.imported_funcs.get(&call.func_name))?;
                let idx = sig
                    .params
                    .iter()
                    .position(|p| p.name == origin_param)?;
                call.args.get(idx).and_then(place_root)
            });
        // Consuming parameters of the callee.
        let move_params: Vec<bool> = self
            .funcs
            .get(&call.func_name)
            .or_else(|| self.imported_funcs.get(&call.func_name))
            .map(|sig| sig.params.iter().map(|p| p.is_move).collect())
            .unwrap_or_default();
        for (i, arg) in call.args.iter().enumerate() {
            let Some(root) = place_root(arg) else { continue };
            let consumed = move_params.get(i).copied().unwrap_or(false)
                || matches!(arg, Node::MoveExpr(_));
            let has_views = self.origin_views.iter().any(|(_, o)| o == &root);
            if !has_views {
                continue;
            }
            if consumed {
                self.origin_invalidate_owner(&root);
            } else if produced.as_deref() != Some(root.as_str()) {
                self.origin_invalidate_owner(&root);
            }
        }
    }

    /// A call used as (part of) a binding's value: if it produces
    /// a view, register the destination as a view of the owner.
    pub fn origin_on_bind(
        &mut self,
        name: &str,
        value: &Node,
        val_ty: &Type,
    ) -> CheckResult<()> {
        if let Node::CallExpr(call) = value {
            if let Some(owner) = self.origin_call_owner(call)? {
                self.origin_register(name, &owner);
                return Ok(());
            }
        }
        // Binding initialized from another view aliases its owner.
        if is_borrow_ty(val_ty) {
            if let Some(root) = place_root(value) {
                if let Some(owner) = self.origin_owner_of(&root) {
                    self.origin_register(name, &owner);
                }
            }
        }
        Ok(())
    }

    /// Rebinding an existing view binding to a view of a
    /// DIFFERENT owner is the merge gate (checked by the caller
    /// against the pre-branch snapshot for the branch shape, and
    /// directly for a straight-line rebind to another owner).
    pub fn origin_on_assign(
        &mut self,
        name: &str,
        value: &Node,
        val_ty: &Type,
    ) -> CheckResult<()> {
        if let Node::CallExpr(call) = value {
            if let Some(owner) = self.origin_call_owner(call)? {
                if let Some(prev) = self.origin_owner_of(name) {
                    if prev != owner {
                        return Err(Diagnostic::new(
                            ErrorKind::BorrowOriginMerge,
                            format!(
                                "BorrowOriginMerge: `{name}` merges views of `{prev}` and `{owner}`"
                            ),
                        ));
                    }
                }
                self.origin_register(name, &owner);
                return Ok(());
            }
        }
        if is_borrow_ty(val_ty) {
            if let Some(root) = place_root(value) {
                if let Some(owner) = self.origin_owner_of(&root) {
                    if let Some(prev) = self.origin_owner_of(name) {
                        if prev != owner {
                            return Err(Diagnostic::new(
                                ErrorKind::BorrowOriginMerge,
                                format!(
                                    "BorrowOriginMerge: `{name}` merges views of `{prev}` and `{owner}`"
                                ),
                            ));
                        }
                    }
                    self.origin_register(name, &owner);
                }
            }
        }
        Ok(())
    }

    /// Moving a binding (`let moved = owner;`, by-value pass)
    /// invalidates its outstanding views.
    pub fn origin_on_move(&mut self, root: &str) {
        self.origin_invalidate_owner(root);
    }

    /// A read of an invalidated view is BorrowUseAfterInvalidation.
    pub fn origin_check_read(&self, name: &str) -> CheckResult<()> {
        if self.origin_invalidated.contains(name) {
            return Err(Diagnostic::new(
                ErrorKind::BorrowUseAfterInvalidation,
                format!(
                    "BorrowUseAfterInvalidation: view `{name}` is read after its owner was invalidated"
                ),
            ));
        }
        Ok(())
    }

    /// Return gate: returning a view requires the current
    /// function's return to carry the matching origin annotation
    /// (owner must be the annotated parameter).
    pub fn origin_check_return(
        &self,
        func: &FuncDecl,
        value: Option<&Node>,
    ) -> CheckResult<()> {
        let Some(v) = value else { return Ok(()) };
        let Some(root) = place_root(v) else { return Ok(()) };
        let Some(owner) = self.origin_owner_of(&root) else {
            return Ok(());
        };
        match &func.ret_borrow_origin {
            Some(annotated) if annotated == &owner => Ok(()),
            _ => Err(Diagnostic::new(
                ErrorKind::BorrowEscape,
                format!(
                    "BorrowEscape: view of `{owner}` returned from `{}` without the matching origin annotation",
                    func.name
                ),
            )),
        }
    }

    /// Store gate: storing a view into a struct literal field is
    /// an escape. Consulted by the struct-literal check: `fields`
    /// are the literal's field values, `value_is_view` resolves
    /// each. Kept as a free-standing predicate pair below.
    pub fn origin_value_is_view(&self, value: &Node) -> bool {
        if let Node::CallExpr(call) = value {
            let sig = self
                .funcs
                .get(&call.func_name)
                .or_else(|| self.imported_funcs.get(&call.func_name));
            if let Some(sig) = sig {
                if sig.ret_borrow_origin.is_some() {
                    return true;
                }
            }
        }
        if let Some(root) = place_root(value) {
            return self.origin_owner_of(&root).is_some();
        }
        false
    }

    /// Mention scan for a statement-level expression: reading an
    /// invalidated view anywhere inside is the invalidation gate
    /// (the identifier hook covers the common root positions;
    /// this covers method receivers like `view.len()`).
    pub fn origin_check_expr_mentions(&self, expr: &Node) -> CheckResult<()> {
        for name in &self.origin_invalidated {
            if expr_mentions(expr, name) {
                return Err(Diagnostic::new(
                    ErrorKind::BorrowUseAfterInvalidation,
                    format!(
                        "BorrowUseAfterInvalidation: view `{name}` is read after its owner was invalidated"
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Snapshot the ledger (branch-merge detection).
    pub fn origin_snapshot(&self) -> Vec<(String, String)> {
        self.origin_views.clone()
    }

    /// After an if/else: a binding whose owner differs between
    /// the pre-branch snapshot and now — and which the branch
    /// rebound — has merged two origins.
    pub fn origin_check_merge(
        &self,
        before: &[(String, String)],
        rebound: &[String],
    ) -> CheckResult<()> {
        for name in rebound {
            let prev = before
                .iter()
                .rev()
                .find(|(v, _)| v == name)
                .map(|(_, o)| o.clone());
            let now = self.origin_owner_of(name);
            if let (Some(p), Some(n)) = (prev, now) {
                if p != n {
                    return Err(Diagnostic::new(
                        ErrorKind::BorrowOriginMerge,
                        format!(
                            "BorrowOriginMerge: `{name}` merges views of `{p}` and `{n}`"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

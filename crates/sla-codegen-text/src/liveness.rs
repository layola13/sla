//! Liveness and move-out predicates for call arguments, mirrored
//! from codegen.zig (`identifierMustStayLiveForLaterUse`,
//! `isSelfAssignMoveOutArg`, `identifierUsedLaterInCurrentBlock`,
//! `identifierDefinedInCurrentBlock`) and lowering_rules.zig
//! (`sharedNodeUsesIdentifier`, `sharedNodeBindsIdentifier`,
//! `tailChainContainsArg`, `countIdentifierUsesExcept`).
//!
//! The `Codegen` side keeps Zig's `current_block` /
//! `current_stmt_index` / move-out claim state (raw pointer + len
//! for the block: the AST outlives codegen and is never mutated,
//! so the slice is reconstructed only inside the accessors).
//! `gen_block` / `gen_scoped_block` save and restore the tracking
//! around nested blocks, exactly like the Zig `genBlock` nesting.
//!
//! Fail-safe direction: every predicate here failing to fire
//! leaves the previous emission in place (a loud host trap, never
//! a silent miscompile); firing spuriously only adds a clone or a
//! register rebind.

use sla_ast::Node;
use sla_lowering::node_key;

use crate::codegen::Codegen;

/// lowering_rules.zig:1399 `sharedNodeUsesIdentifier`: whether the
/// node reads identifier `name` anywhere (binding positions and
/// shadowed names do not count: a `for` var or closure param of the
/// same name shadows; a call's callee is not walked, matching the
/// Zig arm).
pub(crate) fn node_uses_identifier(node: &Node, name: &str) -> bool {
    match node {
        Node::Program(p) => block_uses_identifier(&p.decls, name),
        Node::FuncDecl(f) => block_uses_identifier(&f.body, name),
        Node::MacroDecl(m) => block_uses_identifier(&m.body, name),
        Node::TestDecl(t) => block_uses_identifier(&t.body, name),
        Node::ImplDecl(i) => block_uses_identifier(&i.methods, name),
        Node::LetStmt(l) => node_uses_identifier(&l.value, name),
        Node::LetElseStmt(l) => {
            node_uses_identifier(&l.value, name)
                || block_uses_identifier(&l.else_block, name)
        }
        Node::LetDestructureStmt(l) => node_uses_identifier(&l.value, name),
        Node::ConstStmt(c) => node_uses_identifier(&c.value, name),
        Node::AssignStmt(a) => {
            node_uses_identifier(&a.target, name) || node_uses_identifier(&a.value, name)
        }
        Node::BlockStmt(b) => block_uses_identifier(&b.body, name),
        Node::ArenaStmt(b) => block_uses_identifier(&b.body, name),
        Node::ExprStmt(e) => node_uses_identifier(e, name),
        Node::ReturnStmt(r) => match &r.value {
            Some(v) => node_uses_identifier(v, name),
            None => false,
        },
        Node::ForStmt(f) => {
            if node_uses_identifier(&f.start, name) {
                return true;
            }
            if let Some(end) = &f.end {
                if node_uses_identifier(end, name) {
                    return true;
                }
            }
            if f.var_name == name {
                return false;
            }
            block_uses_identifier(&f.body, name)
        }
        Node::WhileStmt(w) => {
            node_uses_identifier(&w.cond, name) || block_uses_identifier(&w.body, name)
        }
        Node::GoStmt(g) => node_uses_identifier(&g.call, name),
        Node::DeferStmt(d) => node_uses_identifier(&d.call, name),
        Node::Identifier(n) => n == name,
        Node::IfExpr(i) => {
            if node_uses_identifier(&i.cond, name) {
                return true;
            }
            if let Some(chain) = &i.let_chain {
                for item in chain {
                    if node_uses_identifier(&item.value, name) {
                        return true;
                    }
                }
            }
            if block_uses_identifier(&i.then_block, name) {
                return true;
            }
            match &i.else_block {
                Some(b) => block_uses_identifier(b, name),
                None => false,
            }
        }
        Node::SwitchExpr(s) => {
            if node_uses_identifier(&s.val, name) {
                return true;
            }
            s.cases
                .iter()
                .any(|c| block_uses_identifier(&c.body, name))
        }
        Node::MatchExpr(m) => {
            if node_uses_identifier(&m.val, name) {
                return true;
            }
            m.cases.iter().any(|c| {
                if let Some(g) = &c.guard {
                    if node_uses_identifier(g, name) {
                        return true;
                    }
                }
                block_uses_identifier(&c.body, name)
            })
        }
        Node::UnsafeExpr(u) => block_uses_identifier(&u.body, name),
        Node::AwaitExpr(a) => node_uses_identifier(&a.expr, name),
        Node::BinaryExpr(b) => {
            node_uses_identifier(&b.left, name) || node_uses_identifier(&b.right, name)
        }
        Node::CallExpr(c) => c.args.iter().any(|a| node_uses_identifier(a, name)),
        Node::ClosureLiteral(cl) => {
            if cl.params.iter().any(|p| p.name == name) {
                return false;
            }
            node_uses_identifier(&cl.body, name)
        }
        Node::BorrowExpr(b) => node_uses_identifier(&b.expr, name),
        Node::MoveExpr(m) => node_uses_identifier(&m.expr, name),
        Node::DerefExpr(d) => node_uses_identifier(&d.expr, name),
        Node::CastExpr(c) => node_uses_identifier(&c.expr, name),
        Node::FieldExpr(f) => node_uses_identifier(&f.expr, name),
        Node::StructLiteral(l) => {
            if let Some(u) = &l.update_expr {
                if node_uses_identifier(u, name) {
                    return true;
                }
            }
            l.fields.iter().any(|f| node_uses_identifier(&f.value, name))
        }
        Node::EnumLiteral(l) => l
            .fields
            .iter()
            .any(|f| node_uses_identifier(&f.value, name)),
        Node::TupleLiteral(t) => t.elements.iter().any(|e| node_uses_identifier(e, name)),
        Node::ArrayLiteral(a) => a.elements.iter().any(|e| node_uses_identifier(e, name)),
        Node::RepeatArrayLiteral(r) => node_uses_identifier(&r.value, name),
        Node::IndexExpr(i) => {
            node_uses_identifier(&i.target, name) || node_uses_identifier(&i.index, name)
        }
        Node::SliceExpr(s) => {
            node_uses_identifier(&s.target, name)
                || node_uses_identifier(&s.start, name)
                || node_uses_identifier(&s.end, name)
        }
        Node::TryExpr(t) => node_uses_identifier(&t.expr, name),
        _ => false,
    }
}

fn block_uses_identifier(block: &[Node], name: &str) -> bool {
    block.iter().any(|stmt| node_uses_identifier(stmt, name))
}

/// lowering_rules.zig:1376 `sharedNodeBindsIdentifier`: whether
/// the statement binds identifier `name` (a fresh binding shadows
/// the outer one for later statements).
pub(crate) fn node_binds_identifier(node: &Node, name: &str) -> bool {
    match node {
        Node::LetStmt(l) => l.name == name,
        Node::ConstStmt(c) => c.name == name,
        Node::VarStmt(v) => v.name == name,
        Node::LetDestructureStmt(l) => {
            l.names.iter().any(|n| n == name)
                || l.rest_name.as_deref() == Some(name)
                || l.rest_alias.as_deref() == Some(name)
        }
        _ => false,
    }
}

/// lowering_rules.zig:7044 `tailChainContainsArg`: whether the
/// argument node (by identity) sits in the tail call chain of the
/// assigned value — the value is a call containing it, or a
/// move/try/await wrapper around such a call, recursively.
pub(crate) fn tail_chain_contains_arg(value: &Node, arg: *const Node) -> bool {
    match value {
        Node::CallExpr(c) => c.args.iter().any(|a| {
            std::ptr::eq(a, unsafe { &*arg }) || tail_chain_contains_arg(a, arg)
        }),
        Node::MoveExpr(m) => tail_chain_contains_arg(&m.expr, arg),
        Node::TryExpr(t) => tail_chain_contains_arg(&t.expr, arg),
        Node::AwaitExpr(a) => tail_chain_contains_arg(&a.expr, arg),
        _ => false,
    }
}

/// lowering_rules.zig:7066 `countIdentifierUsesExcept`: uses of
/// identifier `name` inside `node`, skipping the subtree rooted at
/// `except` (the move-out argument itself). Node kinds that cannot
/// be walked conservatively return `usize::MAX`, which disqualifies
/// the move-out: any other use of the name in the assigned value
/// means the old value must stay live.
pub(crate) fn count_identifier_uses_except(
    node: &Node,
    name: &str,
    except: *const Node,
) -> usize {
    if std::ptr::eq(node, unsafe { &*except }) {
        return 0;
    }
    match node {
        Node::Identifier(n) => {
            if n == name {
                1
            } else {
                0
            }
        }
        Node::Literal(_) => 0,
        Node::CallExpr(c) => {
            let mut count = 0usize;
            if let Some(callee) = &c.callee {
                count += count_identifier_uses_except(callee, name, except);
            }
            for a in &c.args {
                count += count_identifier_uses_except(a, name, except);
            }
            count
        }
        Node::MoveExpr(m) => count_identifier_uses_except(&m.expr, name, except),
        Node::TryExpr(t) => count_identifier_uses_except(&t.expr, name, except),
        Node::AwaitExpr(a) => count_identifier_uses_except(&a.expr, name, except),
        Node::BorrowExpr(b) => count_identifier_uses_except(&b.expr, name, except),
        Node::DerefExpr(d) => count_identifier_uses_except(&d.expr, name, except),
        Node::CastExpr(c) => count_identifier_uses_except(&c.expr, name, except),
        Node::BinaryExpr(b) => {
            count_identifier_uses_except(&b.left, name, except)
                + count_identifier_uses_except(&b.right, name, except)
        }
        Node::FieldExpr(f) => count_identifier_uses_except(&f.expr, name, except),
        Node::IndexExpr(i) => {
            count_identifier_uses_except(&i.target, name, except)
                + count_identifier_uses_except(&i.index, name, except)
        }
        _ => usize::MAX,
    }
}

/// Find the identifier argument named `name` in the value's tail
/// call chain (same walk as `tail_chain_contains_arg`), for the
/// move-out single-use check. Returns the first match.
pub(crate) fn find_tail_chain_identifier_arg<'a>(value: &'a Node, name: &str) -> Option<&'a Node> {
    match value {
        Node::CallExpr(c) => {
            for a in &c.args {
                if let Node::Identifier(n) = a {
                    if n == name {
                        return Some(a);
                    }
                }
                if let Some(found) = find_tail_chain_identifier_arg(a, name) {
                    return Some(found);
                }
            }
            None
        }
        Node::MoveExpr(m) => find_tail_chain_identifier_arg(&m.expr, name),
        Node::TryExpr(t) => find_tail_chain_identifier_arg(&t.expr, name),
        Node::AwaitExpr(a) => find_tail_chain_identifier_arg(&a.expr, name),
        _ => None,
    }
}

impl<'c> Codegen<'c> {
    /// Save the current block tracking and enter `block` as the
    /// current block (index 0). Returns the saved state for
    /// `exit_block_tracking`.
    pub(crate) fn enter_block_tracking(
        &mut self,
        block: &[Node],
    ) -> (Option<(*const Node, usize)>, usize) {
        let saved = (self.current_block, self.current_stmt_index);
        self.current_block = Some((block.as_ptr(), block.len()));
        self.current_stmt_index = 0;
        saved
    }

    /// Restore block tracking saved by `enter_block_tracking`.
    pub(crate) fn exit_block_tracking(
        &mut self,
        saved: (Option<(*const Node, usize)>, usize),
    ) {
        self.current_block = saved.0;
        self.current_stmt_index = saved.1;
    }

    /// Reconstruct the current block slice from the stored raw
    /// pointer. The AST outlives codegen and is never mutated, so
    /// this is sound for the duration of generation.
    fn current_block_slice(&self) -> Option<&[Node]> {
        self.current_block
            .map(|(ptr, len)| unsafe { std::slice::from_raw_parts(ptr, len) })
    }

    /// Task 137 (P1): the statements after `current_stmt_index` in
    /// the current block — the remainder of the enclosing block in
    /// which a `let` being emitted lives — or None when block
    /// tracking is not active for this statement. Validated: the
    /// statement at `current_stmt_index` must be `stmt` itself (by
    /// node key); otherwise the tracking is stale (e.g. an async
    /// desugar path emitting outside a block iterator) and the
    /// caller must fail closed to a heap header. `gen_block_inner`
    /// / `gen_scoped_block_inner` / the tail-value stores all set
    /// `current_stmt_index` before each `gen_stmt`, so the check
    /// passes on every ordinary block path.
    pub(crate) fn current_block_remainder_for(&self, stmt: &Node) -> Option<&[Node]> {
        let block = self.current_block_slice()?;
        let idx = self.current_stmt_index;
        let cur = block.get(idx)?;
        if node_key(cur) != node_key(stmt) {
            return None;
        }
        block.get(idx + 1..)
    }

    /// codegen.zig:12003 `identifierUsedLaterInCurrentBlock`: a
    /// later statement in the current block reads the identifier
    /// (nested blocks included, via the recursive use scan). A
    /// statement that rebinds the name first stops the scan: the
    /// old value is dead at that point.
    pub(crate) fn identifier_used_later_in_current_block(&self, name: &str) -> bool {
        let Some(block) = self.current_block_slice() else {
            return false;
        };
        let mut idx = self.current_stmt_index + 1;
        while idx < block.len() {
            let stmt = &block[idx];
            if node_uses_identifier(stmt, name) {
                return true;
            }
            if node_binds_identifier(stmt, name) {
                return false;
            }
            idx += 1;
        }
        false
    }

    /// codegen.zig:12014 `identifierDefinedInCurrentBlock`:
    /// whether the name is bound by a statement in the current
    /// block at or before the current statement.
    pub(crate) fn identifier_defined_in_current_block(&self, name: &str) -> bool {
        let Some(block) = self.current_block_slice() else {
            return false;
        };
        let mut idx = 0;
        while idx <= self.current_stmt_index && idx < block.len() {
            if node_binds_identifier(&block[idx], name) {
                return true;
            }
            idx += 1;
        }
        false
    }

    /// codegen.zig:12023 `identifierMustStayLiveForLaterUse`:
    /// used later in the current block, or — the loop back-edge
    /// rule — an outer/param binding read inside a loop body, which
    /// must remain live across iterations even when the current
    /// statement is the only textual use in this pass. Loop-local
    /// bindings are owned by the iteration and may be consumed.
    /// (Zig also consults `current_expr_later_nodes` — sibling
    /// arguments later in the enclosing call; that half lives in
    /// `identifier_used_in_later_sibling` below, SPRINT-100 L2.)
    pub(crate) fn identifier_must_stay_live_for_later_use(&self, name: &str) -> bool {
        if self.identifier_used_later_in_current_block(name) {
            return true;
        }
        if self.loop_continue_labels.is_empty() {
            return false;
        }
        if self.identifier_defined_in_current_block(name) {
            return false;
        }
        true
    }

    /// SPRINT-100 L2: push a sibling frame for a compound
    /// expression whose children are generated in order (call
    /// arguments, struct-literal field values, tuple elements) —
    /// the Zig `current_expr_later_nodes` state this lane did not
    /// track. The generator advances the frame index as each
    /// child is produced and pops the frame when the compound is
    /// done.
    pub(crate) fn push_expr_later_frame(&mut self, children: Vec<*const Node>) {
        self.expr_later_frames.push((children, 0));
    }

    pub(crate) fn set_expr_later_frame_index(&mut self, index: usize) {
        if let Some(frame) = self.expr_later_frames.last_mut() {
            frame.1 = index;
        }
    }

    pub(crate) fn pop_expr_later_frame(&mut self) {
        self.expr_later_frames.pop();
    }

    /// Whether identifier `name` is read by a sibling node that
    /// will be generated AFTER the child currently being
    /// generated, in any enclosing compound frame. A consuming
    /// position (a by-value call argument) must not move a
    /// binding a later sibling still reads: the caller clones
    /// the value for the consuming position instead.
    pub(crate) fn identifier_used_in_later_sibling(&self, name: &str) -> bool {
        for (children, current) in &self.expr_later_frames {
            let Some(later) = children.get(current.saturating_add(1)..) else {
                continue;
            };
            for child in later {
                let node = unsafe { &**child };
                if node_uses_identifier(node, name) {
                    return true;
                }
            }
        }
        false
    }

    /// codegen.zig:12036 `isSelfAssignMoveOutArg` (REG-BR-6):
    /// `arg` is an identifier argument in the tail call chain of
    /// the current statement, the statement assigns the result back
    /// to that same identifier, the identifier has no other use in
    /// the assigned value, and no earlier argument of this
    /// statement already claimed the move-out. When true, the call
    /// may consume the binding directly (move-out) instead of
    /// deep-cloning it: the assignment rebinds the slot to the call
    /// result. (The generalized rebind form,
    /// `isRebindMoveOutArg` / `rebindMoveOutProven`, is not ported:
    /// it needs a later-statement overwrite proof and is outside
    /// the 5a file set.)
    pub(crate) fn is_self_assign_move_out_arg(&self, arg: &Node) -> bool {
        let Node::Identifier(name) = arg else {
            return false;
        };
        let Some(block) = self.current_block_slice() else {
            return false;
        };
        if self.current_stmt_index >= block.len() {
            return false;
        }
        let stmt = &block[self.current_stmt_index];
        let Node::AssignStmt(assign) = stmt else {
            return false;
        };
        let Node::Identifier(target) = &*assign.target else {
            return false;
        };
        if self.resolve_binding_name(target) != self.resolve_binding_name(name) {
            return false;
        }
        if self.move_out_stmt == Some(node_key(stmt)) {
            if let Some(claimed) = &self.move_out_name {
                if self.resolve_binding_name(claimed) == self.resolve_binding_name(name) {
                    return false;
                }
            }
        }
        if !tail_chain_contains_arg(&assign.value, arg as *const Node) {
            return false;
        }
        if count_identifier_uses_except(&assign.value, name, arg as *const Node) != 0 {
            return false;
        }
        true
    }

    /// codegen.zig:12060 `claimSelfAssignMoveOut`: record the
    /// move-out claim for the current statement so the assignment
    /// emission rebinds the slot instead of storing through it.
    pub(crate) fn claim_self_assign_move_out(&mut self, arg: &Node) {
        let stmt_key = match self.current_block_slice() {
            Some(block) if self.current_stmt_index < block.len() => {
                node_key(&block[self.current_stmt_index])
            }
            _ => return,
        };
        self.move_out_stmt = Some(stmt_key);
        if let Node::Identifier(name) = arg {
            self.move_out_name = Some(name.clone());
        }
    }

    /// Whether the assignment statement `stmt` may rebind its
    /// target instead of storing through the slot: the value's tail
    /// call claimed this exact target as a move-out (REG-BR-6,
    /// codegen.zig ~15558).
    pub(crate) fn assign_has_self_move_out_claim(&self, stmt: &Node, target_name: &str) -> bool {
        let Some(claimed_stmt) = self.move_out_stmt else {
            return false;
        };
        if claimed_stmt != node_key(stmt) {
            return false;
        }
        let Some(claimed) = &self.move_out_name else {
            return false;
        };
        self.resolve_binding_name(claimed) == self.resolve_binding_name(target_name)
    }
}

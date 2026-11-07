//! C2 — binding pre-scans and the per-function register ledger,
//! mirroring `sab_codegen.zig` lines 855–934 and 2954–3056:
//! - `collectAssignedBindings` / `...InNode` (837–925): every
//!   assignment target's root identifier, program-wide;
//! - `collectBorrowedBindingsInNode` (2958–3043): `&name` shapes
//!   plus imported-macro addressable args. This is a
//!   *per-function* scan, not part of `generate`'s pre-scan: it is
//!   called at every function/test entry path (Zig 6060, 6117,
//!   6152, 6212, 6477, 7690, 7909, 7950), right after
//!   `prepareMultiLetBindings`, on the set `beginFunction` just
//!   cleared. The driver wires it at the same points
//!   (`gen_func_decl` / `gen_test_decl`). Registered as SAB-D23.
//! - `prepareMultiLetBindings` (847–849): repeated-`let` names per
//!   function body, delegated to sla-lowering's
//!   `collect_repeated_let_bindings` (which clears the set first,
//!   as the Zig lowering helper does);
//! - `bindingNeedsScalarReassignSlot` /
//!   `bindingNeedsCopyScalarReuseSlot` (927–934);
//! - `recordReg` / `finishFunctionRegs` (3045–3056): the
//!   per-function register ledger ([`RegLedger`]).
//!
//! All result sets here are membership-only: nothing in this
//! module is ever iterated into an emission path (house
//! hash-order discipline).

use std::collections::HashSet;

use sla_ast::{CallExpr, Node, Type};

/// Zig `collectAssignedBindings` (837–845): clears the set, then
/// walks every top-level declaration. Returns a fresh set.
pub fn collect_assigned_bindings(decls: &[Node]) -> HashSet<String> {
    let mut out = HashSet::new();
    for decl in decls {
        collect_assigned_in_node(decl, &mut out);
    }
    out
}

fn collect_assigned_in_block(body: &[Node], out: &mut HashSet<String>) {
    for node in body {
        collect_assigned_in_node(node, out);
    }
}

/// Zig `collectAssignedBindingsInNode` (855–925). Note the Zig
/// walk's exact reach: `var_stmt` has no arm at all (falls into
/// `else => {}`), enum literals and slices are NOT walked here
/// (unlike the borrowed walker below), and assignment targets are
/// recorded by *root* identifier (field/index/borrow/move peeled,
/// lowering `rootIdentifier`).
fn collect_assigned_in_node(node: &Node, out: &mut HashSet<String>) {
    match node {
        Node::FuncDecl(f) => collect_assigned_in_block(&f.body, out),
        Node::TestDecl(t) => collect_assigned_in_block(&t.body, out),
        Node::ImplDecl(i) => {
            for m in &i.methods {
                collect_assigned_in_node(m, out);
            }
        }
        Node::OverloadDecl(o) => {
            for m in &o.methods {
                collect_assigned_in_node(m, out);
            }
        }
        Node::LetStmt(l) => collect_assigned_in_node(&l.value, out),
        Node::LetDestructureStmt(l) => collect_assigned_in_node(&l.value, out),
        Node::LetElseStmt(l) => {
            collect_assigned_in_node(&l.value, out);
            collect_assigned_in_block(&l.else_block, out);
        }
        Node::ConstStmt(c) => collect_assigned_in_node(&c.value, out),
        Node::AssignStmt(a) => {
            if let Some(name) = sla_lowering::root_identifier(&a.target) {
                out.insert(name.to_string());
            }
            collect_assigned_in_node(&a.target, out);
            collect_assigned_in_node(&a.value, out);
        }
        Node::ExprStmt(e) => collect_assigned_in_node(e, out),
        Node::ReturnStmt(r) => {
            if let Some(v) = &r.value {
                collect_assigned_in_node(v, out);
            }
        }
        Node::BlockStmt(b) => collect_assigned_in_block(&b.body, out),
        Node::BinaryExpr(b) => {
            collect_assigned_in_node(&b.left, out);
            collect_assigned_in_node(&b.right, out);
        }
        Node::CallExpr(call) => {
            if let Some(callee) = &call.callee {
                collect_assigned_in_node(callee, out);
            }
            for arg in &call.args {
                collect_assigned_in_node(arg, out);
            }
        }
        Node::FieldExpr(f) => collect_assigned_in_node(&f.expr, out),
        Node::StructLiteral(lit) => {
            for field in &lit.fields {
                collect_assigned_in_node(&field.value, out);
            }
            if let Some(update) = &lit.update_expr {
                collect_assigned_in_node(update, out);
            }
        }
        Node::TupleLiteral(lit) => {
            for e in &lit.elements {
                collect_assigned_in_node(e, out);
            }
        }
        Node::ArrayLiteral(lit) => {
            for e in &lit.elements {
                collect_assigned_in_node(e, out);
            }
        }
        Node::RepeatArrayLiteral(lit) => collect_assigned_in_node(&lit.value, out),
        Node::IndexExpr(idx) => {
            collect_assigned_in_node(&idx.target, out);
            collect_assigned_in_node(&idx.index, out);
        }
        Node::IfExpr(ife) => {
            collect_assigned_in_node(&ife.cond, out);
            if let Some(chain) = &ife.let_chain {
                for cond in chain {
                    collect_assigned_in_node(&cond.value, out);
                }
            }
            collect_assigned_in_block(&ife.then_block, out);
            if let Some(else_block) = &ife.else_block {
                collect_assigned_in_block(else_block, out);
            }
        }
        Node::WhileStmt(w) => {
            collect_assigned_in_node(&w.cond, out);
            collect_assigned_in_block(&w.body, out);
        }
        Node::ForStmt(f) => {
            collect_assigned_in_node(&f.start, out);
            if let Some(end) = &f.end {
                collect_assigned_in_node(end, out);
            }
            collect_assigned_in_block(&f.body, out);
        }
        Node::MatchExpr(m) => {
            collect_assigned_in_node(&m.val, out);
            for case in &m.cases {
                collect_assigned_in_block(&case.body, out);
            }
        }
        Node::BorrowExpr(b) => collect_assigned_in_node(&b.expr, out),
        Node::MoveExpr(m) => collect_assigned_in_node(&m.expr, out),
        Node::DerefExpr(d) => collect_assigned_in_node(&d.expr, out),
        Node::CastExpr(c) => collect_assigned_in_node(&c.expr, out),
        Node::UnsafeExpr(u) => collect_assigned_in_block(&u.body, out),
        Node::AwaitExpr(a) => collect_assigned_in_node(&a.expr, out),
        Node::ClosureLiteral(c) => collect_assigned_in_node(&c.body, out),
        _ => {}
    }
}

/// Zig `prepareMultiLetBindings` (847–849): names let-bound two or
/// more times within this one function body. The Zig lowering
/// helper clears the target set first; sla-lowering's port returns
/// a fresh set with the same membership.
pub fn prepare_multi_let_bindings(body: &[Node]) -> HashSet<String> {
    sla_lowering::collect_repeated_let_bindings(body)
}

/// Zig `bindingNeedsScalarReassignSlot` (927–929):
/// `typeCanUseScalarReassignSlot(ty) and assigned_bindings.contains(name)`,
/// where the lowering fact is exactly "type is a primitive".
pub fn binding_needs_scalar_reassign_slot(
    name: &str,
    ty: &Type,
    assigned_bindings: &HashSet<String>,
) -> bool {
    matches!(ty, Type::Primitive(_)) && assigned_bindings.contains(name)
}

/// Zig `bindingNeedsCopyScalarReuseSlot` (931–934): the scalar
/// fact plus `identifierUsedLaterInCurrentBlock(name)`. That
/// lookahead belongs to the E-group block walker (its Zig home is
/// outside the C2 ranges), so the caller supplies its verdict.
pub fn binding_needs_copy_scalar_reuse_slot(ty: &Type, used_later_in_current_block: bool) -> bool {
    matches!(ty, Type::Primitive(_)) && used_later_in_current_block
}

/// The imported-macro addressability fact C2's borrowed walker
/// needs from lowering `planImportedMacroCall` (lowering_rules
/// 3109–3123 + `addressableIdentifierArgName` 596–601): for call
/// argument `i`, the identifier name to mark borrowed, if the
/// macro plan makes that argument addressable. sla-lowering has
/// not ported the imported-macro plan yet, so the driver-side
/// provider returns `None` for every call: within the driver's
/// subset imported-macro calls stop loudly in `gen_call` anyway,
/// but a program whose *only* imported-macro use sits inside a
/// borrowed-binding scan would under-collect versus Zig until
/// the plan lands in sla-lowering (SAB-D23).
pub trait ImportedMacroPlans {
    fn addressable_identifier_arg_name(&self, call: &CallExpr, arg_index: usize) -> Option<&str>;
}

/// A plan provider with no imported macros: every query declines.
pub struct NoImportedMacroPlans;

impl ImportedMacroPlans for NoImportedMacroPlans {
    fn addressable_identifier_arg_name(&self, _call: &CallExpr, _arg_index: usize) -> Option<&str> {
        None
    }
}

/// Zig `collectBorrowedBindingsInNode` (2958–3043), ported
/// faithfully — including its wider reach than the assigned
/// walker (enum literals, slices, try expressions, switch, match
/// guards and if-let chains are all walked here). Wired into the
/// driver per function/test body, as in Zig (SAB-D23).
pub fn collect_borrowed_bindings_in_block(
    body: &[Node],
    plans: &dyn ImportedMacroPlans,
    out: &mut HashSet<String>,
) {
    for node in body {
        collect_borrowed_in_node(node, plans, out);
    }
}

fn collect_borrowed_in_node(node: &Node, plans: &dyn ImportedMacroPlans, out: &mut HashSet<String>) {
    match node {
        Node::BorrowExpr(b) => {
            if let Some(name) = sla_lowering::borrowed_identifier_name(node) {
                out.insert(name.to_string());
            }
            collect_borrowed_in_node(&b.expr, plans, out);
        }
        Node::MoveExpr(m) => collect_borrowed_in_node(&m.expr, plans, out),
        Node::DerefExpr(d) => collect_borrowed_in_node(&d.expr, plans, out),
        Node::CastExpr(c) => collect_borrowed_in_node(&c.expr, plans, out),
        Node::BinaryExpr(b) => {
            collect_borrowed_in_node(&b.left, plans, out);
            collect_borrowed_in_node(&b.right, plans, out);
        }
        Node::CallExpr(call) => {
            for i in 0..call.args.len() {
                if let Some(name) = plans.addressable_identifier_arg_name(call, i) {
                    out.insert(name.to_string());
                }
            }
            for arg in &call.args {
                collect_borrowed_in_node(arg, plans, out);
            }
        }
        Node::FieldExpr(f) => collect_borrowed_in_node(&f.expr, plans, out),
        Node::StructLiteral(lit) => {
            for field in &lit.fields {
                collect_borrowed_in_node(&field.value, plans, out);
            }
            if let Some(update) = &lit.update_expr {
                collect_borrowed_in_node(update, plans, out);
            }
        }
        Node::EnumLiteral(lit) => {
            for field in &lit.fields {
                collect_borrowed_in_node(&field.value, plans, out);
            }
        }
        Node::TupleLiteral(lit) => {
            for e in &lit.elements {
                collect_borrowed_in_node(e, plans, out);
            }
        }
        Node::ArrayLiteral(lit) => {
            for e in &lit.elements {
                collect_borrowed_in_node(e, plans, out);
            }
        }
        Node::RepeatArrayLiteral(lit) => collect_borrowed_in_node(&lit.value, plans, out),
        Node::IndexExpr(idx) => {
            collect_borrowed_in_node(&idx.target, plans, out);
            collect_borrowed_in_node(&idx.index, plans, out);
        }
        Node::SliceExpr(s) => {
            collect_borrowed_in_node(&s.target, plans, out);
            collect_borrowed_in_node(&s.start, plans, out);
            collect_borrowed_in_node(&s.end, plans, out);
        }
        Node::ClosureLiteral(c) => collect_borrowed_in_node(&c.body, plans, out),
        Node::TryExpr(t) => collect_borrowed_in_node(&t.expr, plans, out),
        Node::UnsafeExpr(u) => collect_borrowed_bindings_in_block(&u.body, plans, out),
        Node::AwaitExpr(a) => collect_borrowed_in_node(&a.expr, plans, out),
        Node::SwitchExpr(s) => {
            collect_borrowed_in_node(&s.val, plans, out);
            for case in &s.cases {
                collect_borrowed_bindings_in_block(&case.body, plans, out);
            }
        }
        Node::MatchExpr(m) => {
            collect_borrowed_in_node(&m.val, plans, out);
            for case in &m.cases {
                if let Some(guard) = &case.guard {
                    collect_borrowed_in_node(guard, plans, out);
                }
                collect_borrowed_bindings_in_block(&case.body, plans, out);
            }
        }
        Node::IfExpr(ife) => {
            collect_borrowed_in_node(&ife.cond, plans, out);
            collect_borrowed_bindings_in_block(&ife.then_block, plans, out);
            if let Some(else_block) = &ife.else_block {
                collect_borrowed_bindings_in_block(else_block, plans, out);
            }
            if let Some(chain) = &ife.let_chain {
                for cond in chain {
                    collect_borrowed_in_node(&cond.value, plans, out);
                }
            }
        }
        Node::ForStmt(f) => {
            collect_borrowed_in_node(&f.start, plans, out);
            if let Some(end) = &f.end {
                collect_borrowed_in_node(end, plans, out);
            }
            collect_borrowed_bindings_in_block(&f.body, plans, out);
        }
        Node::WhileStmt(w) => {
            collect_borrowed_in_node(&w.cond, plans, out);
            collect_borrowed_bindings_in_block(&w.body, plans, out);
        }
        Node::LetStmt(l) => collect_borrowed_in_node(&l.value, plans, out),
        Node::LetElseStmt(l) => {
            collect_borrowed_in_node(&l.value, plans, out);
            collect_borrowed_bindings_in_block(&l.else_block, plans, out);
        }
        Node::ConstStmt(c) => collect_borrowed_in_node(&c.value, plans, out),
        Node::LetDestructureStmt(l) => collect_borrowed_in_node(&l.value, plans, out),
        Node::AssignStmt(a) => {
            collect_borrowed_in_node(&a.target, plans, out);
            collect_borrowed_in_node(&a.value, plans, out);
        }
        Node::ExprStmt(e) => collect_borrowed_in_node(e, plans, out),
        Node::ReturnStmt(r) => {
            if let Some(v) = &r.value {
                collect_borrowed_in_node(v, plans, out);
            }
        }
        Node::BlockStmt(b) => collect_borrowed_bindings_in_block(&b.body, plans, out),
        _ => {}
    }
}

/// Zig `recordReg` / `finishFunctionRegs` (3045–3056) as an
/// explicit ledger: `record` first *removes* the register from the
/// released set (a re-recorded register is live again), then
/// appends it once, in first-record order; `finish` returns a copy
/// of the ids (Zig `allocator.dupe`) without clearing — clearing
/// is `beginFunction`'s job (line 2432), mirrored by
/// [`RegLedger::begin_function`]. The emission trunk's
/// `ModuleBuilder` keeps its own simplified ledger for the
/// already-wired paths (its `record_reg`/`finish_function`); this
/// type is the full C2 semantics, including the released-set
/// interplay the trunk's frame state tracks separately
/// (`state::RegState::Released`).
#[derive(Debug, Default)]
pub struct RegLedger {
    ids: Vec<u32>,
    seen: HashSet<u32>,
    released: HashSet<u32>,
}

impl RegLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Zig `beginFunction`'s ledger reset (2432–2434).
    pub fn begin_function(&mut self) {
        self.ids.clear();
        self.seen.clear();
        self.released.clear();
    }

    /// Zig `recordReg` (3045–3050).
    pub fn record_reg(&mut self, reg: u32) {
        self.released.remove(&reg);
        if self.seen.insert(reg) {
            self.ids.push(reg);
        }
    }

    /// Mark a register released (Zig `released_regs.put` at the
    /// release sites); a later `record_reg` revives it.
    pub fn mark_released(&mut self, reg: u32) {
        self.released.insert(reg);
    }

    pub fn is_released(&self, reg: u32) -> bool {
        self.released.contains(&reg)
    }

    /// Zig `finishFunctionRegs` (3052–3056): a copy of the ids in
    /// record order (empty slice when nothing was recorded).
    pub fn finish_function_regs(&self) -> Vec<u32> {
        self.ids.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{
        AssignStmt, BinaryExpr, BinaryOp, BorrowExpr, FieldExpr, FuncDecl, LetStmt, Literal,
        Primitive, ReturnStmt,
    };

    fn ident(name: &str) -> Node {
        Node::Identifier(name.to_string())
    }

    fn int(v: i64) -> Node {
        Node::Literal(Literal::IntVal(v))
    }

    fn assign(target: Node, value: Node) -> Node {
        Node::AssignStmt(AssignStmt {
            target: Box::new(target),
            value: Box::new(value),
        })
    }

    fn func(name: &str, body: Vec<Node>) -> Node {
        Node::FuncDecl(FuncDecl {
            name: name.to_string(),
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: Vec::new(),
            params: Vec::new(),
            ret_ty: Box::new(Type::Primitive(Primitive::I32)),
            body,
            is_inline: false,
            is_async: false,
            operator: None,
        })
    }

    #[test]
    fn assigned_collects_root_identifiers() {
        // `a = 1`, `b.field = 2`, `c[i] = 3` all register their
        // root; a plain read of `d` does not.
        let field_target = Node::FieldExpr(FieldExpr {
            expr: Box::new(ident("b")),
            field_name: "x".to_string(),
        });
        let index_target = Node::IndexExpr(sla_ast::IndexExpr {
            target: Box::new(ident("c")),
            index: Box::new(int(0)),
        });
        let body = vec![
            assign(ident("a"), int(1)),
            assign(field_target, int(2)),
            assign(index_target, int(3)),
            Node::ReturnStmt(ReturnStmt {
                value: Some(Box::new(ident("d"))),
            }),
        ];
        let set = collect_assigned_bindings(&[func("f", body)]);
        assert!(set.contains("a"));
        assert!(set.contains("b"));
        assert!(set.contains("c"));
        assert!(!set.contains("d"));
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn assigned_walks_nested_blocks_and_values() {
        // Assignment inside an assignment's value and inside a
        // while body are both found (the walker is fully recursive
        // over the arms Zig lists).
        let inner = assign(ident("inner"), int(1));
        let outer = assign(ident("outer"), inner);
        let while_node = Node::WhileStmt(sla_ast::WhileStmt {
            cond: Box::new(Node::Literal(Literal::BoolVal(true))),
            let_pattern: None,
            body: vec![assign(ident("in_loop"), int(5))],
        });
        let set = collect_assigned_bindings(&[func("f", vec![outer, while_node])]);
        assert!(set.contains("outer"));
        assert!(set.contains("inner"));
        assert!(set.contains("in_loop"));
    }

    #[test]
    fn borrowed_collects_ampersand_identifiers_only() {
        // `&x` registers x; `&y.field` does not (borrowedIdentifierName
        // is the `&name` shape only); nested `&z` inside a binary
        // expression is found.
        let borrow_field = Node::BorrowExpr(BorrowExpr {
            expr: Box::new(Node::FieldExpr(FieldExpr {
                expr: Box::new(ident("y")),
                field_name: "f".to_string(),
            })),
        });
        let bin = Node::BinaryExpr(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(Node::BorrowExpr(BorrowExpr {
                expr: Box::new(ident("z")),
            })),
            right: Box::new(int(1)),
        });
        let body = vec![
            Node::LetStmt(LetStmt {
                name: "p".to_string(),
                ty: None,
                value: Box::new(Node::BorrowExpr(BorrowExpr {
                    expr: Box::new(ident("x")),
                })),
            }),
            Node::ExprStmt(Box::new(borrow_field)),
            Node::ExprStmt(Box::new(bin)),
        ];
        let mut out = HashSet::new();
        collect_borrowed_bindings_in_block(&body, &NoImportedMacroPlans, &mut out);
        assert!(out.contains("x"));
        assert!(out.contains("z"));
        assert!(!out.contains("y"));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn multi_let_is_per_function() {
        // Same name let-bound once in each of two functions is not
        // multi; twice in one body is.
        let let_a = |v: i64| {
            Node::LetStmt(LetStmt {
                name: "a".to_string(),
                ty: None,
                value: Box::new(int(v)),
            })
        };
        let single = prepare_multi_let_bindings(&[let_a(1)]);
        assert!(single.is_empty());
        let double = prepare_multi_let_bindings(&[let_a(1), let_a(2)]);
        assert!(double.contains("a"));
    }

    #[test]
    fn reassign_slot_facts() {
        let mut assigned = HashSet::new();
        assigned.insert("a".to_string());
        let i32_ty = Type::Primitive(Primitive::I32);
        let ptr_ty = Type::Pointer(Box::new(i32_ty.clone()));
        assert!(binding_needs_scalar_reassign_slot("a", &i32_ty, &assigned));
        assert!(!binding_needs_scalar_reassign_slot("b", &i32_ty, &assigned));
        assert!(!binding_needs_scalar_reassign_slot("a", &ptr_ty, &assigned));
        assert!(binding_needs_copy_scalar_reuse_slot(&i32_ty, true));
        assert!(!binding_needs_copy_scalar_reuse_slot(&i32_ty, false));
        assert!(!binding_needs_copy_scalar_reuse_slot(&ptr_ty, true));
    }

    #[test]
    fn reg_ledger_record_dedup_revival_and_finish() {
        let mut ledger = RegLedger::new();
        ledger.record_reg(5);
        ledger.record_reg(3);
        ledger.record_reg(5); // duplicate: not appended twice
        assert_eq!(ledger.finish_function_regs(), vec![5, 3]);
        // Release then re-record: the register is live again and
        // its position is its first-record position.
        ledger.mark_released(5);
        assert!(ledger.is_released(5));
        ledger.record_reg(5);
        assert!(!ledger.is_released(5));
        assert_eq!(ledger.finish_function_regs(), vec![5, 3]);
        // finish does not clear; begin_function does.
        ledger.begin_function();
        assert_eq!(ledger.finish_function_regs(), Vec::<u32>::new());
        assert!(!ledger.is_released(5));
    }
}

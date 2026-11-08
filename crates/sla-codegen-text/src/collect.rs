//! Per-function pre-pass collectors, mirrored from codegen.zig:
//! `collectAssignedBindings` (2563-2655), `collectAddressableBindings`
//! (2657-2790), `collectHoistedAllocs` (7689, 7802-7938) and
//! `collectLoopCounterSlots` (7660-7800). These fill the binding sets
//! the emitters consult; they never emit text.

use sla_ast::Node;
use sla_lowering::{borrowed_identifier_name, is_stack_alloc_node, root_identifier};

use crate::codegen::Codegen;

impl<'c> Codegen<'c> {
    // -- assigned bindings ---------------------------------------------

    pub(crate) fn collect_assigned_bindings(&mut self, block: &[Node]) {
        for stmt in block {
            self.collect_assigned_bindings_in_node(stmt);
        }
    }

    fn collect_assigned_bindings_in_node(&mut self, node: &Node) {
        match node {
            Node::FuncDecl(f) => self.collect_assigned_bindings(&f.body),
            Node::TestDecl(t) => self.collect_assigned_bindings(&t.body),
            Node::ImplDecl(i) => {
                for method in &i.methods {
                    self.collect_assigned_bindings_in_node(method);
                }
            }
            Node::OverloadDecl(o) => {
                for method in &o.methods {
                    self.collect_assigned_bindings_in_node(method);
                }
            }
            Node::LetStmt(l) => self.collect_assigned_bindings_in_node(&l.value),
            Node::LetElseStmt(l) => {
                self.collect_assigned_bindings_in_node(&l.value);
                self.collect_assigned_bindings(&l.else_block);
            }
            Node::LetDestructureStmt(l) => self.collect_assigned_bindings_in_node(&l.value),
            Node::ConstStmt(c) => self.collect_assigned_bindings_in_node(&c.value),
            Node::VarStmt(_) => {}
            Node::AssignStmt(assign) => {
                if let Some(name) = root_identifier(&assign.target) {
                    self.assigned_bindings.insert(name.to_string());
                }
                self.collect_assigned_bindings_in_node(&assign.target);
                self.collect_assigned_bindings_in_node(&assign.value);
            }
            Node::ExprStmt(expr) => self.collect_assigned_bindings_in_node(expr),
            Node::ReturnStmt(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_assigned_bindings_in_node(value);
                }
            }
            Node::BlockStmt(blk) => self.collect_assigned_bindings(&blk.body),
            Node::ArenaStmt(blk) => self.collect_assigned_bindings(&blk.body),
            Node::ForStmt(f) => {
                self.collect_assigned_bindings_in_node(&f.start);
                if let Some(end) = &f.end {
                    self.collect_assigned_bindings_in_node(end);
                }
                self.collect_assigned_bindings(&f.body);
            }
            Node::WhileStmt(w) => {
                self.collect_assigned_bindings_in_node(&w.cond);
                self.collect_assigned_bindings(&w.body);
            }
            Node::BinaryExpr(bin) => {
                self.collect_assigned_bindings_in_node(&bin.left);
                self.collect_assigned_bindings_in_node(&bin.right);
            }
            Node::CallExpr(call) => {
                if let Some(callee) = &call.callee {
                    self.collect_assigned_bindings_in_node(callee);
                }
                for arg in &call.args {
                    self.collect_assigned_bindings_in_node(arg);
                }
            }
            Node::FieldExpr(field) => self.collect_assigned_bindings_in_node(&field.expr),
            Node::IndexExpr(idx) => {
                self.collect_assigned_bindings_in_node(&idx.target);
                self.collect_assigned_bindings_in_node(&idx.index);
            }
            Node::SliceExpr(slc) => {
                self.collect_assigned_bindings_in_node(&slc.target);
                self.collect_assigned_bindings_in_node(&slc.start);
                self.collect_assigned_bindings_in_node(&slc.end);
            }
            Node::StructLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_assigned_bindings_in_node(&field.value);
                }
                if let Some(update) = &lit.update_expr {
                    self.collect_assigned_bindings_in_node(update);
                }
            }
            Node::EnumLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_assigned_bindings_in_node(&field.value);
                }
            }
            Node::TupleLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_assigned_bindings_in_node(elem);
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_assigned_bindings_in_node(elem);
                }
            }
            Node::RepeatArrayLiteral(lit) => {
                self.collect_assigned_bindings_in_node(&lit.value)
            }
            Node::BorrowExpr(b) => self.collect_assigned_bindings_in_node(&b.expr),
            Node::MoveExpr(m) => self.collect_assigned_bindings_in_node(&m.expr),
            Node::DerefExpr(d) => self.collect_assigned_bindings_in_node(&d.expr),
            Node::CastExpr(c) => self.collect_assigned_bindings_in_node(&c.expr),
            Node::AwaitExpr(a) => self.collect_assigned_bindings_in_node(&a.expr),
            Node::TryExpr(t) => self.collect_assigned_bindings_in_node(&t.expr),
            Node::UnsafeExpr(ue) => self.collect_assigned_bindings(&ue.body),
            Node::ClosureLiteral(cl) => self.collect_assigned_bindings_in_node(&cl.body),
            Node::IfExpr(ife) => {
                self.collect_assigned_bindings_in_node(&ife.cond);
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        self.collect_assigned_bindings_in_node(&cond.value);
                    }
                }
                self.collect_assigned_bindings(&ife.then_block);
                if let Some(else_block) = &ife.else_block {
                    self.collect_assigned_bindings(else_block);
                }
            }
            Node::SwitchExpr(swe) => {
                self.collect_assigned_bindings_in_node(&swe.val);
                for case in &swe.cases {
                    self.collect_assigned_bindings(&case.body);
                }
            }
            Node::MatchExpr(mat) => {
                self.collect_assigned_bindings_in_node(&mat.val);
                for case in &mat.cases {
                    if let Some(guard) = &case.guard {
                        self.collect_assigned_bindings_in_node(guard);
                    }
                    self.collect_assigned_bindings(&case.body);
                }
            }
            _ => {}
        }
    }

    // -- addressable bindings (names whose `&name` is taken) -----------

    pub(crate) fn collect_addressable_bindings(&mut self, block: &[Node]) {
        for stmt in block {
            self.collect_addressable_bindings_in_stmt(stmt);
        }
    }

    fn collect_addressable_bindings_in_stmt(&mut self, stmt: &Node) {
        match stmt {
            Node::LetStmt(l) => self.collect_addressable_bindings_in_expr(&l.value),
            Node::LetElseStmt(l) => {
                self.collect_addressable_bindings_in_expr(&l.value);
                self.collect_addressable_bindings(&l.else_block);
            }
            Node::LetDestructureStmt(l) => self.collect_addressable_bindings_in_expr(&l.value),
            Node::ConstStmt(c) => self.collect_addressable_bindings_in_expr(&c.value),
            Node::AssignStmt(assign) => {
                self.collect_addressable_bindings_in_expr(&assign.target);
                self.collect_addressable_bindings_in_expr(&assign.value);
            }
            Node::ExprStmt(expr) => self.collect_addressable_bindings_in_expr(expr),
            Node::ReturnStmt(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_addressable_bindings_in_expr(value);
                }
            }
            Node::ForStmt(f) => {
                self.collect_addressable_bindings_in_expr(&f.start);
                if let Some(end) = &f.end {
                    self.collect_addressable_bindings_in_expr(end);
                }
                self.collect_addressable_bindings(&f.body);
            }
            Node::WhileStmt(w) => {
                self.collect_addressable_bindings_in_expr(&w.cond);
                self.collect_addressable_bindings(&w.body);
            }
            Node::BlockStmt(blk) => self.collect_addressable_bindings(&blk.body),
            Node::ArenaStmt(blk) => self.collect_addressable_bindings(&blk.body),
            _ => {}
        }
    }

    fn collect_addressable_bindings_in_expr(&mut self, expr: &Node) {
        match expr {
            Node::BorrowExpr(_) => {
                if let Some(name) = borrowed_identifier_name(expr) {
                    self.addressable_bindings.insert(name.to_string());
                }
                if let Node::BorrowExpr(b) = expr {
                    self.collect_addressable_bindings_in_expr(&b.expr);
                }
            }
            Node::MoveExpr(m) => self.collect_addressable_bindings_in_expr(&m.expr),
            Node::DerefExpr(d) => self.collect_addressable_bindings_in_expr(&d.expr),
            Node::BinaryExpr(bin) => {
                self.collect_addressable_bindings_in_expr(&bin.left);
                self.collect_addressable_bindings_in_expr(&bin.right);
            }
            Node::CallExpr(call) => {
                if let Some(callee) = &call.callee {
                    self.collect_addressable_bindings_in_expr(callee);
                }
                for arg in &call.args {
                    self.collect_addressable_bindings_in_expr(arg);
                }
            }
            Node::FieldExpr(f) => self.collect_addressable_bindings_in_expr(&f.expr),
            Node::IndexExpr(idx) => {
                self.collect_addressable_bindings_in_expr(&idx.target);
                self.collect_addressable_bindings_in_expr(&idx.index);
            }
            Node::SliceExpr(slc) => {
                self.collect_addressable_bindings_in_expr(&slc.target);
                self.collect_addressable_bindings_in_expr(&slc.start);
                self.collect_addressable_bindings_in_expr(&slc.end);
            }
            Node::CastExpr(c) => self.collect_addressable_bindings_in_expr(&c.expr),
            Node::AwaitExpr(a) => self.collect_addressable_bindings_in_expr(&a.expr),
            Node::TryExpr(t) => self.collect_addressable_bindings_in_expr(&t.expr),
            Node::StructLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_addressable_bindings_in_expr(&field.value);
                }
                if let Some(update) = &lit.update_expr {
                    self.collect_addressable_bindings_in_expr(update);
                }
            }
            Node::EnumLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_addressable_bindings_in_expr(&field.value);
                }
            }
            Node::TupleLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_addressable_bindings_in_expr(elem);
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_addressable_bindings_in_expr(elem);
                }
            }
            Node::RepeatArrayLiteral(lit) => {
                self.collect_addressable_bindings_in_expr(&lit.value)
            }
            Node::IfExpr(ife) => {
                self.collect_addressable_bindings_in_expr(&ife.cond);
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        self.collect_addressable_bindings_in_expr(&cond.value);
                    }
                }
                self.collect_addressable_bindings(&ife.then_block);
                if let Some(else_block) = &ife.else_block {
                    self.collect_addressable_bindings(else_block);
                }
            }
            Node::SwitchExpr(swe) => {
                self.collect_addressable_bindings_in_expr(&swe.val);
                for case in &swe.cases {
                    self.collect_addressable_bindings(&case.body);
                }
            }
            Node::MatchExpr(mat) => {
                self.collect_addressable_bindings_in_expr(&mat.val);
                for case in &mat.cases {
                    self.collect_addressable_bindings(&case.body);
                }
            }
            Node::UnsafeExpr(ue) => self.collect_addressable_bindings(&ue.body),
            Node::ClosureLiteral(cl) => self.collect_addressable_bindings_in_expr(&cl.body),
            _ => {}
        }
    }

    // -- hoisted loop allocations ---------------------------------------

    pub(crate) fn collect_hoisted_allocs(&self, block: &[Node], list: &mut Vec<String>) {
        self.collect_hoisted_allocs_internal(block, list, false);
    }

    fn collect_hoisted_allocs_internal(&self, block: &[Node], list: &mut Vec<String>, in_loop: bool) {
        for stmt in block {
            match stmt {
                Node::LetStmt(l) => {
                    if in_loop && is_stack_alloc_node(&l.value) {
                        list.push(l.name.clone());
                    }
                    self.collect_hoisted_allocs_in_expr(&l.value, list, in_loop);
                }
                Node::LetElseStmt(l) => {
                    self.collect_hoisted_allocs_in_expr(&l.value, list, in_loop);
                    self.collect_hoisted_allocs_internal(&l.else_block, list, in_loop);
                }
                Node::LetDestructureStmt(l) => {
                    self.collect_hoisted_allocs_in_expr(&l.value, list, in_loop);
                }
                Node::ConstStmt(c) => {
                    if in_loop && is_stack_alloc_node(&c.value) {
                        list.push(c.name.clone());
                    }
                    self.collect_hoisted_allocs_in_expr(&c.value, list, in_loop);
                }
                Node::AssignStmt(assign) => {
                    self.collect_hoisted_allocs_in_expr(&assign.target, list, in_loop);
                    self.collect_hoisted_allocs_in_expr(&assign.value, list, in_loop);
                }
                Node::ExprStmt(expr) => {
                    self.collect_hoisted_allocs_in_expr(expr, list, in_loop);
                }
                Node::ReturnStmt(ret) => {
                    if let Some(value) = &ret.value {
                        self.collect_hoisted_allocs_in_expr(value, list, in_loop);
                    }
                }
                Node::ForStmt(f) => {
                    self.collect_hoisted_allocs_internal(&f.body, list, true);
                }
                Node::ReleaseStmt(_) => {}
                // Mirrored quirk: the Zig walker does not descend into
                // while bodies here (only `for` sets in_loop).
                _ => {}
            }
        }
    }

    fn collect_hoisted_allocs_in_expr(&self, expr: &Node, list: &mut Vec<String>, in_loop: bool) {
        match expr {
            Node::IfExpr(ife) => {
                self.collect_hoisted_allocs_in_expr(&ife.cond, list, in_loop);
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        self.collect_hoisted_allocs_in_expr(&cond.value, list, in_loop);
                    }
                }
                self.collect_hoisted_allocs_internal(&ife.then_block, list, in_loop);
                if let Some(else_block) = &ife.else_block {
                    self.collect_hoisted_allocs_internal(else_block, list, in_loop);
                }
            }
            Node::SwitchExpr(swe) => {
                for case in &swe.cases {
                    self.collect_hoisted_allocs_internal(&case.body, list, in_loop);
                }
            }
            Node::MatchExpr(mat) => {
                for case in &mat.cases {
                    self.collect_hoisted_allocs_internal(&case.body, list, in_loop);
                }
            }
            Node::BinaryExpr(bin) => {
                self.collect_hoisted_allocs_in_expr(&bin.left, list, in_loop);
                self.collect_hoisted_allocs_in_expr(&bin.right, list, in_loop);
            }
            Node::BorrowExpr(b) => self.collect_hoisted_allocs_in_expr(&b.expr, list, in_loop),
            Node::MoveExpr(m) => self.collect_hoisted_allocs_in_expr(&m.expr, list, in_loop),
            Node::DerefExpr(d) => self.collect_hoisted_allocs_in_expr(&d.expr, list, in_loop),
            Node::CastExpr(c) => self.collect_hoisted_allocs_in_expr(&c.expr, list, in_loop),
            Node::CallExpr(call) => {
                for arg in &call.args {
                    self.collect_hoisted_allocs_in_expr(arg, list, in_loop);
                }
            }
            Node::FieldExpr(f) => self.collect_hoisted_allocs_in_expr(&f.expr, list, in_loop),
            Node::IndexExpr(idx) => {
                self.collect_hoisted_allocs_in_expr(&idx.target, list, in_loop);
                self.collect_hoisted_allocs_in_expr(&idx.index, list, in_loop);
            }
            Node::SliceExpr(slc) => {
                self.collect_hoisted_allocs_in_expr(&slc.target, list, in_loop);
                self.collect_hoisted_allocs_in_expr(&slc.start, list, in_loop);
                self.collect_hoisted_allocs_in_expr(&slc.end, list, in_loop);
            }
            Node::TupleLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_hoisted_allocs_in_expr(elem, list, in_loop);
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_hoisted_allocs_in_expr(elem, list, in_loop);
                }
            }
            Node::StructLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_hoisted_allocs_in_expr(&field.value, list, in_loop);
                }
                if let Some(update) = &lit.update_expr {
                    self.collect_hoisted_allocs_in_expr(update, list, in_loop);
                }
            }
            Node::EnumLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_hoisted_allocs_in_expr(&field.value, list, in_loop);
                }
            }
            Node::RepeatArrayLiteral(lit) => {
                self.collect_hoisted_allocs_in_expr(&lit.value, list, in_loop)
            }
            Node::UnsafeExpr(ue) => {
                self.collect_hoisted_allocs_internal(&ue.body, list, in_loop)
            }
            Node::AwaitExpr(a) => self.collect_hoisted_allocs_in_expr(&a.expr, list, in_loop),
            Node::TryExpr(t) => self.collect_hoisted_allocs_in_expr(&t.expr, list, in_loop),
            _ => {}
        }
    }

    // -- for-loop counter slots ------------------------------------------

    pub(crate) fn collect_loop_counter_slots(&self, block: &[Node], list: &mut Vec<String>) {
        for stmt in block {
            match stmt {
                Node::ForStmt(f) => {
                    append_unique(list, format!("{}_slot", f.var_name));
                    self.collect_loop_counter_slots(&f.body, list);
                }
                Node::WhileStmt(w) => self.collect_loop_counter_slots(&w.body, list),
                Node::BlockStmt(blk) => self.collect_loop_counter_slots(&blk.body, list),
                Node::ArenaStmt(blk) => self.collect_loop_counter_slots(&blk.body, list),
                Node::LetElseStmt(l) => self.collect_loop_counter_slots(&l.else_block, list),
                Node::ExprStmt(expr) => self.collect_loop_counter_slots_in_expr(expr, list),
                Node::LetStmt(l) => self.collect_loop_counter_slots_in_expr(&l.value, list),
                Node::ConstStmt(c) => self.collect_loop_counter_slots_in_expr(&c.value, list),
                Node::AssignStmt(assign) => {
                    self.collect_loop_counter_slots_in_expr(&assign.target, list);
                    self.collect_loop_counter_slots_in_expr(&assign.value, list);
                }
                Node::ReturnStmt(ret) => {
                    if let Some(value) = &ret.value {
                        self.collect_loop_counter_slots_in_expr(value, list);
                    }
                }
                Node::LetDestructureStmt(l) => {
                    self.collect_loop_counter_slots_in_expr(&l.value, list)
                }
                _ => {}
            }
        }
    }

    fn collect_loop_counter_slots_in_expr(&self, expr: &Node, list: &mut Vec<String>) {
        match expr {
            Node::IfExpr(ife) => {
                self.collect_loop_counter_slots_in_expr(&ife.cond, list);
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        self.collect_loop_counter_slots_in_expr(&cond.value, list);
                    }
                }
                self.collect_loop_counter_slots(&ife.then_block, list);
                if let Some(else_block) = &ife.else_block {
                    self.collect_loop_counter_slots(else_block, list);
                }
            }
            Node::SwitchExpr(swe) => {
                for case in &swe.cases {
                    self.collect_loop_counter_slots(&case.body, list);
                }
            }
            Node::MatchExpr(mat) => {
                for case in &mat.cases {
                    self.collect_loop_counter_slots(&case.body, list);
                }
            }
            Node::UnsafeExpr(ue) => self.collect_loop_counter_slots(&ue.body, list),
            Node::CallExpr(call) => {
                for arg in &call.args {
                    self.collect_loop_counter_slots_in_expr(arg, list);
                }
            }
            Node::BinaryExpr(bin) => {
                self.collect_loop_counter_slots_in_expr(&bin.left, list);
                self.collect_loop_counter_slots_in_expr(&bin.right, list);
            }
            Node::BorrowExpr(b) => self.collect_loop_counter_slots_in_expr(&b.expr, list),
            Node::MoveExpr(m) => self.collect_loop_counter_slots_in_expr(&m.expr, list),
            Node::DerefExpr(d) => self.collect_loop_counter_slots_in_expr(&d.expr, list),
            Node::CastExpr(c) => self.collect_loop_counter_slots_in_expr(&c.expr, list),
            Node::FieldExpr(f) => self.collect_loop_counter_slots_in_expr(&f.expr, list),
            Node::IndexExpr(idx) => {
                self.collect_loop_counter_slots_in_expr(&idx.target, list);
                self.collect_loop_counter_slots_in_expr(&idx.index, list);
            }
            Node::SliceExpr(slc) => {
                self.collect_loop_counter_slots_in_expr(&slc.target, list);
                self.collect_loop_counter_slots_in_expr(&slc.start, list);
                self.collect_loop_counter_slots_in_expr(&slc.end, list);
            }
            Node::TupleLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_loop_counter_slots_in_expr(elem, list);
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_loop_counter_slots_in_expr(elem, list);
                }
            }
            Node::StructLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_loop_counter_slots_in_expr(&field.value, list);
                }
            }
            Node::EnumLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_loop_counter_slots_in_expr(&field.value, list);
                }
            }
            Node::RepeatArrayLiteral(lit) => {
                self.collect_loop_counter_slots_in_expr(&lit.value, list)
            }
            _ => {}
        }
    }
}

/// codegen.zig `appendUniqueName`.
fn append_unique(list: &mut Vec<String>, name: String) {
    if !list.contains(&name) {
        list.push(name);
    }
}

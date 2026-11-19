//! The AST walk for the minimal-import scan (`Scanner` methods):
//! drives the call-site and type rules over the whole program tree.
//! Part of [`super`] — see the parent module docs for the rule catalogue.

use super::*;

impl<'a> Scanner<'a> {
    pub(super) fn walk(&mut self, node: &Node) {
        // `@sa_mem_free` provider probe (see the parent module
        // docs): a node whose recorded type materializes a smart
        // pointer or a HashMap can reach the deep-drop / map-free
        // emission sites that call it, so `string` stays imported.
        if let Some(ty) = self.check.expr_type(node) {
            if ty_has_mem_free_value(ty) {
                self.flags.string = true;
            }
        }
        // Array->slice call-arg materialization (call.rs
        // ArrayToSliceBorrow arm) emits SLICE_NEW, whose home
        // (core/slice.sa) reaches the program through
        // string_format.sa — the same provider as the other
        // SLICE_NEW sites in the parent module docs. Macro-body
        // sites carry no checker fact; call_rules.rs covers them
        // structurally.
        if self.check.call_arg_array_to_slice_borrow(node) {
            self.flags.string_format = true;
        }
        match node {
            Node::Program(p) => {
                for d in &p.decls {
                    self.walk(d);
                }
            }
            Node::FuncDecl(f) => {
                // A by-value smart-pointer parameter (or return
                // type) is dropped with @sa_mem_free at the exit
                // sweep — no Box:: call need appear in the body.
                if f.params.iter().any(|p| ty_has_mem_free_value(p.ty.as_ref()))
                    || ty_has_mem_free_value(f.ret_ty.as_ref())
                {
                    self.flags.fmt = true;
                }
                for s in &f.body {
                    self.walk(s);
                }
            }
            Node::TestDecl(t) => {
                for s in &t.body {
                    self.walk(s);
                }
            }
            Node::ImplDecl(d) => {
                for m in &d.methods {
                    self.walk(m);
                }
            }
            Node::OverloadDecl(d) => {
                for m in &d.methods {
                    self.walk(m);
                }
            }
            Node::MacroDecl(m) => {
                for s in &m.body {
                    self.walk(s);
                }
            }
            Node::LetStmt(l) => {
                if let Some(ty) = &l.ty {
                    if ty_has_string(ty.as_ref())
                        && matches!(&*l.value, Node::Literal(Literal::StringVal(_)))
                    {
                        self.flags.string = true;
                    }
                    if ty_has_mem_free_value(ty.as_ref()) {
                        self.flags.fmt = true;
                    }
                }
                self.walk(&l.value);
            }
            Node::ConstStmt(c) => {
                if let Some(ty) = &c.ty {
                    if ty_has_string(ty.as_ref())
                        && matches!(&*c.value, Node::Literal(Literal::StringVal(_)))
                    {
                        self.flags.string = true;
                    }
                }
                self.walk(&c.value);
            }
            Node::VarStmt(v) => {
                if ty_has_string(v.ty.as_ref()) {
                    self.flags.string = true;
                }
            }
            Node::LetElseStmt(l) => {
                self.walk(&l.value);
                for s in &l.else_block {
                    self.walk(s);
                }
            }
            Node::LetDestructureStmt(l) => {
                if let Some(rest) = &l.rest_name {
                    if rest != "_" {
                        // Slice rest binding: fresh Slice cell via
                        // SLICE_NEW (destructure.rs).
                        self.flags.string_format = true;
                    }
                }
                self.walk(&l.value);
            }
            Node::AssignStmt(a) => {
                if matches!(&*a.value, Node::Literal(Literal::StringVal(_))) {
                    // `s = "lit"` materializes an owned handle
                    // when the target holds a String (probe q06).
                    let mut needs_string = false;
                    if let Some(ty) = self.check.expr_type(&a.target) {
                        if self.ty_mentions_string(ty, 0) {
                            needs_string = true;
                        }
                    }
                    if !needs_string {
                        if let Node::FieldExpr(f) = &*a.target {
                            if let Some(base_ty) = self.check.expr_type(&f.expr) {
                                if let Some(field_ty) =
                                    self.struct_field_ty(base_ty, &f.field_name)
                                {
                                    if self.ty_mentions_string(&field_ty, 0) {
                                        needs_string = true;
                                    }
                                }
                            }
                        }
                    }
                    if needs_string {
                        self.flags.string = true;
                    }
                }
                self.walk(&a.target);
                self.walk(&a.value);
            }
            Node::ExprStmt(e) => self.walk(e),
            Node::ReturnStmt(r) => {
                if let Some(v) = &r.value {
                    self.walk(v);
                }
            }
            Node::BlockStmt(b) => {
                for s in &b.body {
                    self.walk(s);
                }
            }
            Node::ArenaStmt(b) => {
                for s in &b.body {
                    self.walk(s);
                }
            }
            Node::WhileStmt(w) => {
                self.walk(&w.cond);
                for s in &w.body {
                    self.walk(s);
                }
            }
            Node::ForStmt(f) => {
                self.walk(&f.start);
                if let Some(e) = &f.end {
                    self.walk(e);
                }
                for s in &f.body {
                    self.walk(s);
                }
            }
            Node::IfExpr(i) => {
                self.walk(&i.cond);
                if let Some(chain) = &i.let_chain {
                    for cond in chain {
                        self.walk(&cond.value);
                    }
                }
                for s in &i.then_block {
                    self.walk(s);
                }
                if let Some(e) = &i.else_block {
                    for s in e {
                        self.walk(s);
                    }
                }
            }
            Node::BinaryExpr(b) => {
                self.walk(&b.left);
                self.walk(&b.right);
            }
            Node::CallExpr(call) => self.walk_call(call),
            Node::GenericFuncRef(g) => {
                match g.func_name.as_str() {
                    "format" => {
                        self.flags.string_format = true;
                        self.flags.fmt = true;
                    }
                    "println" => self.flags.print = true,
                    "panic" | "panic_msg" => self.flags.panic = true,
                    "str_eq" => self.flags.string = true,
                    _ => {}
                }
            }
            Node::Identifier(name) => {
                // A bare builtin name used as a value (fn item);
                // defensive — a same-named local only over-flags.
                match name.as_str() {
                    "format" => {
                        self.flags.string_format = true;
                        self.flags.fmt = true;
                    }
                    "println" => self.flags.print = true,
                    "panic" | "panic_msg" => self.flags.panic = true,
                    "str_eq" => self.flags.string = true,
                    _ => {}
                }
            }
            Node::Literal(Literal::StringVal(_)) => {
                // Evaluated as a value: SLICE_NEW (codegen.rs
                // `gen_literal_value`). Literals in the builtin
                // argument positions are handled inside
                // `walk_call` and never reach this arm.
                self.flags.string_format = true;
            }
            Node::BorrowExpr(b) => self.walk(&b.expr),
            Node::MoveExpr(m) => self.walk(&m.expr),
            Node::DerefExpr(d) => self.walk(&d.expr),
            Node::CastExpr(c) => self.walk(&c.expr),
            Node::FieldExpr(f) => {
                if f.field_name == "len" {
                    if matches!(&*f.expr, Node::Literal(Literal::StringVal(_))) {
                        self.flags.string = true;
                    }
                }
                self.walk(&f.expr);
            }
            Node::IndexExpr(i) => {
                self.walk(&i.target);
                self.walk(&i.index);
            }
            Node::SliceExpr(sl) => {
                self.walk(&sl.target);
                self.walk(&sl.start);
                self.walk(&sl.end);
            }
            Node::ClosureLiteral(c) => {
                self.walk(&c.body);
            }
            Node::AwaitExpr(a) => self.walk(&a.expr),
            Node::TryExpr(t) => self.walk(&t.expr),
            Node::StructLiteral(l) => {
                // into.rs shape D: a projection value stored into
                // a struct literal field is deep-cloned when the
                // field type needs it.
                for field in &l.fields {
                    // Field position is where a repeat literal's
                    // own recorded type may be absent (agg_literal
                    // context-typed emission); the declared field
                    // type still proves a u8 fill.
                    if matches!(&*field.value, Node::RepeatArrayLiteral(_)) {
                        if let Some(field_ty) =
                            self.struct_field_ty(l.ty.as_ref(), &field.name)
                        {
                            if let Type::Array(arr) = &field_ty {
                                if matches!(
                                    &*arr.elem,
                                    Type::Primitive(sla_ast::Primitive::U8)
                                ) {
                                    self.flags.string_format = true;
                                }
                                // The bulk-copy route keys on the
                                // declared length (agg_literal.rs's
                                // BULK_ARRAY_THRESHOLD = 32).
                                if arr.len > 32
                                    && Self::repeat_elem_is_bulk_scalar(&arr.elem)
                                {
                                    self.flags.mem = true;
                                }
                            }
                        }
                    }
                    if matches!(&*field.value, Node::FieldExpr(_) | Node::IndexExpr(_))
                        && Self::projection_root(&field.value).is_some()
                    {
                        let mut needs_string = false;
                        if let Some(ty) = self.check.expr_type(&field.value) {
                            if self.clone_mentions_string(ty, 0) {
                                needs_string = true;
                            }
                        }
                        if !needs_string {
                            if let Some(field_ty) =
                                self.struct_field_ty(l.ty.as_ref(), &field.name)
                            {
                                if self.clone_mentions_string(&field_ty, 0) {
                                    needs_string = true;
                                }
                            }
                        }
                        if needs_string {
                            self.flags.string = true;
                        }
                    }
                    self.walk(&field.value);
                }
                if let Some(u) = &l.update_expr {
                    self.walk(u);
                }
            }
            Node::EnumLiteral(l) => {
                for f in &l.fields {
                    self.walk(&f.value);
                }
            }
            Node::TupleLiteral(l) => {
                for e in &l.elements {
                    self.walk_projection_clone_element(e);
                    self.walk(e);
                }
            }
            Node::ArrayLiteral(l) => {
                for e in &l.elements {
                    self.walk_projection_clone_element(e);
                    self.walk(e);
                }
            }
            Node::RepeatArrayLiteral(r) => {
                if self.repeat_fill_needs_string_format(node, r) {
                    self.flags.string_format = true;
                }
                if self.repeat_fill_needs_mem(node, r) {
                    self.flags.mem = true;
                }
                self.walk(&r.value);
            }
            Node::SwitchExpr(sw) => {
                self.walk(&sw.val);
                for case in &sw.cases {
                    self.walk(&case.pattern);
                    for s in &case.body {
                        self.walk(s);
                    }
                }
            }
            Node::MatchExpr(m) => {
                self.walk(&m.val);
                for case in &m.cases {
                    if let Some(g) = &case.guard {
                        self.walk(g);
                    }
                    for s in &case.body {
                        self.walk(s);
                    }
                }
            }
            Node::GoStmt(g) => self.walk(&g.call),
            Node::DeferStmt(d) => self.walk(&d.call),
            Node::SelectStmt(s) => {
                for arm in &s.arms {
                    if let Some(recv) = &arm.recv {
                        self.walk(recv);
                    }
                    for stmt in &arm.body {
                        self.walk(stmt);
                    }
                }
                if let Some(timeout) = &s.timeout {
                    self.walk(&timeout.expr);
                    for stmt in &timeout.body {
                        self.walk(stmt);
                    }
                }
            }
            Node::UnsafeExpr(u) => {
                for s in &u.body {
                    self.walk(s);
                }
            }
            Node::InlineAsmExpr(_) => {}
            // Declarations and other nodes with no emittable body:
            // StructDecl / EnumDecl / TraitDecl / TypeAliasDecl /
            // ImportDecl / UsingDecl / CheckHookDecl /
            // SublanguageDecl / ShaderDecl / StructDecl field types
            // emit nothing on their own.
            _ => {}
        }
    }

    /// agg_literal.rs `gen_repeat_array_literal_into_typed`: a
    /// repeat literal with u8 elements emits EXPAND
    /// SLA_ARRAY_FILL_U8, whose (unconditionally emitted) macro
    /// body calls @sa_mem_set. @sa_mem_set is defined in
    /// sa_std/core/mem.sa and reaches the program only through
    /// the core/slice.sa importers among the five base imports
    /// (string.sa / string_format.sa) -- the same provider shape
    /// as SLICE_NEW -- so the cheaper closure (string_format)
    /// stays imported for such a site. The element type is read
    /// from the literal's recorded array type, or from the
    /// repeated value's recorded type (a suffixed `0u8`).
    /// agg_literal.rs's doubling bulk copy: a repeat literal with
    /// a scalar non-u8 element type and length above
    /// BULK_ARRAY_THRESHOLD (32) emits
    /// `call @sa_mem_copy_nonoverlapping`, whose definition lives
    /// in `sa_std/core/mem.sa`. The element type is read from the
    /// literal's recorded array type, else from the repeated
    /// value's recorded type; when neither is recorded the typed
    /// emission routes supply the type contextually, so the scan
    /// keeps the import (the standing conservative rule).
    fn repeat_fill_needs_mem(
        &self,
        node: &Node,
        r: &sla_ast::RepeatArrayLiteral,
    ) -> bool {
        if r.len <= 32 {
            return false;
        }
        if let Some(Type::Array(arr)) = self.check.expr_type(node) {
            return Self::repeat_elem_is_bulk_scalar(&arr.elem);
        }
        match self.check.expr_type(&r.value) {
            Some(t) => Self::repeat_elem_is_bulk_scalar(t),
            None => true,
        }
    }

    /// The bulk-copy element test, mirroring agg_literal.rs's
    /// `elem_is_scalar` minus the u8 macro route.
    fn repeat_elem_is_bulk_scalar(ty: &Type) -> bool {
        match ty {
            Type::Primitive(sla_ast::Primitive::U8) => false,
            Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_) => true,
            _ => false,
        }
    }

    fn repeat_fill_needs_string_format(
        &self,
        node: &Node,
        r: &sla_ast::RepeatArrayLiteral,
    ) -> bool {
        if let Some(Type::Array(arr)) = self.check.expr_type(node) {
            if matches!(&*arr.elem, Type::Primitive(sla_ast::Primitive::U8)) {
                return true;
            }
        }
        matches!(
            self.check.expr_type(&r.value),
            Some(Type::Primitive(sla_ast::Primitive::U8))
        )
    }

    /// Aggregate-literal elements (tuple / array): a projection
    /// element is cloned on transfer like a call argument when
    /// its closure holds a String (same shape-C discipline).
    fn walk_projection_clone_element(&mut self, elem: &Node) {
        if !matches!(elem, Node::FieldExpr(_) | Node::IndexExpr(_)) {
            return;
        }
        if Self::projection_root(elem).is_none() {
            return;
        }
        if let Some(ty) = self.check.expr_type(elem) {
            if self.clone_mentions_string(ty, 0) {
                self.flags.string = true;
            }
        }
    }
}

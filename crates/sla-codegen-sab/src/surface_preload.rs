//! D5 — surface dep preloading, mirroring `sab_codegen.zig`
//! lines 1998–2106 and 2255–2317: `preloadStdSurfaceDeps`,
//! `preloadBlockStdSurfaceDeps`, `preloadNodeStdSurfaceDeps`,
//! `isThreadSpawnCall`. The future/thread usage scans and
//! `stackAllocSize` (the other half of D5) live in
//! `surface_scan.rs`, split for the 700-line discipline.
//!
//! The preload walk is a pure *discovery* pass: it finds every
//! std surface a program touches and ensures the corresponding
//! deps through the D4 ensurer surface. Dep ensuring lands in
//! the pending queue (SAB-D26); the actual fragment append is
//! F-group. The walk's arm set is Zig's exactly — node kinds
//! Zig does not recurse into (switch/match/slice/try/await in
//! the preload walk) are not recursed into here either, however
//! surprising that is.

use sla_ast::{CallExpr, Node, Type};
use sla_lowering::{is_println_call, CheckInfo};

use crate::error::Result;
use crate::layout::{type_base_name, user_defined_generic_inner, vec_element_type};
use crate::surface_query::{find_std_surface_rule, StdDepEnsurer};
use crate::types::{StdSurfaceRule, StdSurfaceRuleKind};

/// Zig `isThreadSpawnCall` (lowering_rules.zig:4132):
/// `thread::spawn` with exactly one argument.
pub fn is_thread_spawn_call(call: &CallExpr) -> bool {
    call.associated_target.as_deref() == Some("thread")
        && call.func_name == "spawn"
        && call.args.len() == 1
}

/// Zig `isDebugCall` (lowering_rules.zig:4311, name-only).
fn is_debug_call(call: &CallExpr) -> bool {
    call.func_name == "debug"
}

/// Zig `isBareVecCall` (lowering_rules.zig:4321).
fn is_bare_vec_call(call: &CallExpr) -> bool {
    call.associated_target.is_none() && call.func_name == "vec"
}

/// Zig `isBarePopUnaryCall` (lowering_rules.zig:4356).
fn is_bare_pop_unary_call(call: &CallExpr) -> bool {
    call.associated_target.is_none() && call.func_name == "pop" && call.args.len() == 1
}

/// Zig `isJoinCall` (lowering_rules.zig:4361, name-only).
fn is_join_call(call: &CallExpr) -> bool {
    call.func_name == "join"
}

/// Zig `joinHandleInnerType` (sab_codegen.zig:4017, thinning to
/// lowering's generic-inner extraction for `JoinHandle`).
fn join_handle_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "JoinHandle")
}

/// The D5 preloader: checker facts + the loaded rule table,
/// ensuring deps through a D4 ensurer.
pub struct SurfacePreloader<'c> {
    pub check: &'c dyn CheckInfo,
    pub rules: &'c [StdSurfaceRule],
}

impl<'c> SurfacePreloader<'c> {
    pub fn new(check: &'c dyn CheckInfo, rules: &'c [StdSurfaceRule]) -> Self {
        SurfacePreloader { check, rules }
    }

    /// Zig `ensureRuleDeps` over this preloader's rule table.
    fn ensure_rule(
        &self,
        ensurer: &mut dyn StdDepEnsurer,
        kind: StdSurfaceRuleKind,
        type_name: &str,
        member: Option<&str>,
    ) -> Result<()> {
        if let Some(rule) = find_std_surface_rule(self.rules, kind, type_name, member) {
            ensurer.ensure_rule_deps(rule)?;
        }
        Ok(())
    }

    /// Zig `preloadStdSurfaceDeps` (lines 2103–2106).
    pub fn preload_std_surface_deps(
        &self,
        ensurer: &mut dyn StdDepEnsurer,
        program: &Node,
    ) -> Result<()> {
        if let Node::Program(p) = program {
            for decl in &p.decls {
                self.preload_node(ensurer, decl)?;
            }
        }
        Ok(())
    }

    /// Zig `preloadBlockStdSurfaceDeps`.
    pub fn preload_block(
        &self,
        ensurer: &mut dyn StdDepEnsurer,
        body: &[Node],
    ) -> Result<()> {
        for stmt in body {
            self.preload_node(ensurer, stmt)?;
        }
        Ok(())
    }

    /// Zig `preloadNodeStdSurfaceDeps` (lines 2255–2317).
    pub fn preload_node(&self, ensurer: &mut dyn StdDepEnsurer, node: &Node) -> Result<()> {
        match node {
            Node::FuncDecl(f) => self.preload_block(ensurer, &f.body),
            Node::TestDecl(t) => self.preload_block(ensurer, &t.body),
            Node::LetStmt(l) => self.preload_node(ensurer, &l.value),
            Node::LetDestructureStmt(l) => self.preload_node(ensurer, &l.value),
            Node::LetElseStmt(l) => {
                self.preload_node(ensurer, &l.value)?;
                self.preload_block(ensurer, &l.else_block)
            }
            Node::ConstStmt(c) => self.preload_node(ensurer, &c.value),
            Node::VarStmt(_) => Ok(()),
            Node::AssignStmt(a) => {
                if let Node::IndexExpr(idx) = &*a.target {
                    if let Some(target_ty) = self.check.expr_type(&idx.target) {
                        if let Some(name) = type_base_name(target_ty) {
                            self.ensure_rule(
                                ensurer,
                                StdSurfaceRuleKind::IndexAssign,
                                &name,
                                None,
                            )?;
                        }
                    }
                }
                self.preload_node(ensurer, &a.target)?;
                self.preload_node(ensurer, &a.value)
            }
            Node::ExprStmt(e) => self.preload_node(ensurer, e),
            Node::ReturnStmt(r) => match &r.value {
                Some(value) => self.preload_node(ensurer, value),
                None => Ok(()),
            },
            Node::BlockStmt(b) => self.preload_block(ensurer, &b.body),
            Node::Identifier(name) => {
                if let Some(expr_ty) = self.check.expr_type(node) {
                    if let Some(type_name) = type_base_name(expr_ty) {
                        self.ensure_rule(
                            ensurer,
                            StdSurfaceRuleKind::Constructor,
                            &type_name,
                            Some(name),
                        )?;
                    }
                }
                Ok(())
            }
            Node::BinaryExpr(b) => {
                self.preload_node(ensurer, &b.left)?;
                self.preload_node(ensurer, &b.right)
            }
            Node::BorrowExpr(b) => self.preload_node(ensurer, &b.expr),
            Node::MoveExpr(m) => self.preload_node(ensurer, &m.expr),
            Node::DerefExpr(d) => self.preload_node(ensurer, &d.expr),
            Node::CastExpr(c) => self.preload_node(ensurer, &c.expr),
            Node::FieldExpr(f) => self.preload_node(ensurer, &f.expr),
            Node::StructLiteral(l) => {
                for field in &l.fields {
                    self.preload_node(ensurer, &field.value)?;
                }
                if let Some(update) = &l.update_expr {
                    self.preload_node(ensurer, update)?;
                }
                Ok(())
            }
            Node::TupleLiteral(l) => {
                for elem in &l.elements {
                    self.preload_node(ensurer, elem)?;
                }
                Ok(())
            }
            Node::ArrayLiteral(l) => {
                for elem in &l.elements {
                    self.preload_node(ensurer, elem)?;
                }
                Ok(())
            }
            Node::RepeatArrayLiteral(l) => self.preload_node(ensurer, &l.value),
            Node::ClosureLiteral(l) => self.preload_node(ensurer, &l.body),
            Node::CallExpr(call) => self.preload_call(ensurer, node, call),
            Node::IndexExpr(idx) => {
                if let Some(target_ty) = self.check.expr_type(&idx.target) {
                    if let Some(name) = type_base_name(target_ty) {
                        self.ensure_rule(ensurer, StdSurfaceRuleKind::Index, &name, None)?;
                        self.ensure_rule(
                            ensurer,
                            StdSurfaceRuleKind::IndexAddress,
                            &name,
                            None,
                        )?;
                    }
                }
                self.preload_node(ensurer, &idx.target)?;
                self.preload_node(ensurer, &idx.index)
            }
            Node::IfExpr(i) => {
                self.preload_node(ensurer, &i.cond)?;
                if let Some(chain) = &i.let_chain {
                    for cond in chain {
                        self.preload_node(ensurer, &cond.value)?;
                    }
                }
                self.preload_block(ensurer, &i.then_block)?;
                if let Some(else_block) = &i.else_block {
                    self.preload_block(ensurer, else_block)?;
                }
                Ok(())
            }
            Node::WhileStmt(w) => {
                self.preload_node(ensurer, &w.cond)?;
                self.preload_block(ensurer, &w.body)
            }
            Node::ForStmt(f) => {
                self.preload_node(ensurer, &f.start)?;
                if let Some(end) = &f.end {
                    self.preload_node(ensurer, end)?;
                }
                self.preload_block(ensurer, &f.body)
            }
            _ => Ok(()),
        }
    }

    /// The `call_expr` arm of `preloadNodeStdSurfaceDeps`,
    /// in Zig's exact check order.
    fn preload_call(
        &self,
        ensurer: &mut dyn StdDepEnsurer,
        node: &Node,
        call: &CallExpr,
    ) -> Result<()> {
        if is_thread_spawn_call(call) {
            ensurer.ensure_std_deps(
                "sa_std/thread.sa",
                &["pthread_spawn".to_string()],
            )?;
        }
        if is_println_call(call) && call.associated_target.is_none() {
            ensurer.ensure_println_deps()?;
        }
        if is_debug_call(call) && call.args.len() == 1 {
            ensurer.ensure_debug_format_deps()?;
        }
        if is_bare_vec_call(call) {
            if let Some(expr_ty) = self.check.expr_type(node) {
                if vec_element_type(expr_ty).is_some() {
                    ensurer.ensure_std_deps(
                        "sa_std/vec.sa",
                        &[
                            "sa_vec_new".to_string(),
                            "sa_vec_push".to_string(),
                            "sa_mem_copy".to_string(),
                        ],
                    )?;
                }
            }
        }
        if is_bare_pop_unary_call(call) {
            if let Some(receiver_ty) = self.check.expr_type(&call.args[0]) {
                if vec_element_type(receiver_ty).is_some() {
                    ensurer.ensure_std_deps(
                        "sa_std/vec.sa",
                        &["sa_vec_try_pop".to_string()],
                    )?;
                }
            }
        }
        if let Some(target_name) = &call.associated_target {
            self.ensure_rule(
                ensurer,
                StdSurfaceRuleKind::Associated,
                target_name,
                Some(&call.func_name),
            )?;
        } else if !call.args.is_empty() {
            if is_join_call(call) {
                if let Some(receiver_ty) = self.check.expr_type(&call.args[0]) {
                    if join_handle_inner_type(receiver_ty).is_some() {
                        ensurer.ensure_std_deps(
                            "sa_std/thread.sa",
                            &["sa_pthread_join".to_string(), "pthread_drop".to_string()],
                        )?;
                    }
                }
            }
            if let Some(receiver_ty) = self.check.expr_type(&call.args[0]) {
                if let Some(receiver_name) = type_base_name(receiver_ty) {
                    self.ensure_rule(
                        ensurer,
                        StdSurfaceRuleKind::Function,
                        &receiver_name,
                        Some(&call.func_name),
                    )?;
                    self.ensure_rule(
                        ensurer,
                        StdSurfaceRuleKind::Method,
                        &receiver_name,
                        Some(&call.func_name),
                    )?;
                    self.ensure_rule(
                        ensurer,
                        StdSurfaceRuleKind::FallibleMethod,
                        &receiver_name,
                        Some(&call.func_name),
                    )?;
                }
            }
        }
        if let Some(expr_ty) = self.check.expr_type(node) {
            if let Some(type_name) = type_base_name(expr_ty) {
                self.ensure_rule(
                    ensurer,
                    StdSurfaceRuleKind::Constructor,
                    &type_name,
                    Some(&call.func_name),
                )?;
            }
        }
        for arg in &call.args {
            self.preload_node(ensurer, arg)?;
        }
        if let Some(callee) = &call.callee {
            self.preload_node(ensurer, callee)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface_load::parse_std_surface_rules;
    use crate::surface_query::PendingStdDeps;
    use sla_ast::{CallExpr, FuncDecl, Literal, Primitive, Program, Type};
    use sla_lowering::MapCheckInfo;
    use std::collections::HashSet;

    fn call(name: &str, target: Option<&str>, args: Vec<Node>) -> Node {
        Node::CallExpr(CallExpr {
            func_name: name.to_string(),
            associated_target: target.map(|t| t.to_string()),
            generics: vec![],
            args,
            callee: None,
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
            generics: vec![],
            params: vec![],
            ret_ty: Box::new(Type::Primitive(Primitive::VoidType)),
            body,
            is_inline: false,
            is_async: false,
            operator: None,
        })
    }

    fn run_preload(
        program: &Node,
        check: &dyn CheckInfo,
        rules: &[StdSurfaceRule],
    ) -> Vec<(String, String)> {
        let mut included = HashSet::new();
        let mut pending = Vec::new();
        {
            let mut ensurer = PendingStdDeps::new(&mut included, &mut pending);
            let preloader = SurfacePreloader::new(check, rules);
            preloader
                .preload_std_surface_deps(&mut ensurer, program)
                .unwrap();
        }
        pending
            .iter()
            .map(|p| (p.import_path.clone(), p.dep.clone()))
            .collect()
    }

    #[test]
    fn preload_finds_println_and_associated_deps() {
        let rules = parse_std_surface_rules(
            "associated Vec new sa_std/vec.sa VEC_NEW out deps=sa_vec_new\n",
        )
        .unwrap();
        let program = Node::Program(Program {
            decls: vec![func(
                "main",
                vec![
                    Node::ExprStmt(Box::new(call(
                        "println",
                        None,
                        vec![Node::Literal(Literal::StringVal("hi".into()))],
                    ))),
                    Node::ExprStmt(Box::new(call("new", Some("Vec"), vec![]))),
                ],
            )],
        });
        let check = MapCheckInfo::new();
        let deps = run_preload(&program, &check, &rules);
        assert_eq!(deps[0], ("sa_std/io/print.sai".into(), "sa_print_bytes".into()));
        assert!(deps.contains(&("sa_std/fmt.sai".into(), "sa_fmt_i64".into())));
        assert_eq!(
            deps.last().unwrap(),
            &("sa_std/vec.sa".to_string(), "sa_vec_new".to_string())
        );
    }

    #[test]
    fn preload_thread_spawn_and_join() {
        let rules: Vec<StdSurfaceRule> = vec![];
        let program = Node::Program(Program {
            decls: vec![func(
                "main",
                vec![Node::ExprStmt(Box::new(call(
                    "spawn",
                    Some("thread"),
                    vec![Node::Identifier("job".into())],
                )))],
            )],
        });
        let check = MapCheckInfo::new();
        let deps = run_preload(&program, &check, &rules);
        assert_eq!(
            deps,
            vec![("sa_std/thread.sa".to_string(), "pthread_spawn".to_string())]
        );
    }

}

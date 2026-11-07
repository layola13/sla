//! Program-level checking: registration passes, function and test
//! checking, block checking with exit cleanup — mirroring
//! `checkProgram` (type_checker.zig:2130), `checkFunc` (2573),
//! `checkBlock` (2618) and the termination predicates.
//!
//! Deliberate structural differences (see docs/divergences/checker.md):
//!   * The current return type is `Checker::current_ret_ty`, not a
//!     symbol named `return_ty_sentinel` in the function's scope (D-C5).
//!   * Block-exit cleanup is collected in DECLARATION order; Zig
//!     iterates a StringHashMap, i.e. hash order (D-C6).
//!   * Zig's void-function exit silently flips still-active root-scope
//!     symbols to `.consumed`; the flip is unobservable after the
//!     function scope is dropped, so it is not reproduced.

use crate::checker::{Checker, StructInfo};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use crate::types_util::{is_internal_symbol, node_tag, value_assignable_to};
use sla_ast::decls::{FuncDecl, Program, TestDecl, TypeAliasComponent};
use sla_ast::node::Node;
use sla_ast::types::Type;
use std::collections::HashSet;

impl Checker {
    /// Mirrors Zig `checkProgram`. Registration happens in passes so
    /// declarations may reference each other in any order.
    pub fn check_program(&mut self, program: &Program) -> CheckResult<()> {
        // Pass 1: type-level names (structs / enums / aliases /
        // traits), with cross-kind collision detection
        // (`ensureTopLevelNameUnused`).
        let mut type_names: HashSet<String> = HashSet::new();
        let mut trait_names: HashSet<String> = HashSet::new();
        for decl in &program.decls {
            match decl {
                Node::StructDecl(s) => {
                    if !type_names.insert(s.name.clone()) {
                        return Err(redecl(&s.name));
                    }
                    self.register_struct(
                        &s.name,
                        StructInfo {
                            fields: s
                                .fields
                                .iter()
                                .map(|f| (f.name.clone(), (*f.ty).clone()))
                                .collect(),
                            derives: s.derives.clone(),
                            is_union: s.is_union,
                            is_opaque: s.is_opaque,
                        },
                    );
                }
                Node::EnumDecl(e) => {
                    if !type_names.insert(e.name.clone()) {
                        return Err(redecl(&e.name));
                    }
                    self.register_enum(
                        &e.name,
                        e.variants.iter().map(|v| v.name.clone()).collect(),
                    );
                }
                Node::TypeAliasDecl(a) => {
                    if !type_names.insert(a.name.clone()) {
                        return Err(redecl(&a.name));
                    }
                    // Only single-type components form a resolvable
                    // alias in this slice; inline-struct components
                    // are registered by the coordinator's full port.
                    if a.components.len() == 1 {
                        if let TypeAliasComponent::Ty(t) = &a.components[0] {
                            self.register_alias(&a.name, (**t).clone());
                        }
                    }
                }
                Node::TraitDecl(t) => {
                    if !type_names.insert(t.name.clone()) {
                        return Err(redecl(&t.name));
                    }
                    trait_names.insert(t.name.clone());
                }
                _ => {}
            }
        }
        // Supertraits must exist (Zig checkProgram validates them).
        for decl in &program.decls {
            if let Node::TraitDecl(t) = decl {
                for sup in &t.supertraits {
                    if !trait_names.contains(sup) {
                        return Err(Diagnostic::new(
                            ErrorKind::CompileError,
                            format!("unknown supertrait `{sup}` of trait `{}`", t.name),
                        ));
                    }
                }
            }
        }
        // Pass 2: functions and macros.
        for decl in &program.decls {
            match decl {
                Node::FuncDecl(f) => self.register_func(f),
                Node::MacroDecl(m) => self.register_macro(m),
                Node::ImplDecl(imp) => {
                    for method in &imp.methods {
                        if let Node::FuncDecl(f) = method {
                            self.register_func(f);
                        }
                    }
                }
                _ => {}
            }
        }
        // Pass 3: check bodies.
        for decl in &program.decls {
            match decl {
                Node::FuncDecl(f) => {
                    if !f.is_decl_only && !f.is_extern {
                        self.check_func(f)?;
                    }
                }
                Node::TestDecl(t) => self.check_test(t)?,
                Node::ImplDecl(imp) => {
                    for method in &imp.methods {
                        if let Node::FuncDecl(f) = method {
                            if !f.is_decl_only && !f.is_extern {
                                self.check_func(f)?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Mirrors Zig `checkFunc` (type_checker.zig:2573).
    pub fn check_func(&mut self, func: &FuncDecl) -> CheckResult<()> {
        let ret_ty = (*func.ret_ty).clone();
        let frame = self.scopes.push_frame();
        let saved_root = self.fn_root_depth;
        let saved_ret = self.current_ret_ty.replace(ret_ty.clone());
        self.fn_root_depth = frame;

        let mut result = self.check_func_body(func, &ret_ty);
        if result.is_ok() {
            result = self.check_tail_type(func, &ret_ty);
        }

        self.scopes.pop_frame();
        self.fn_root_depth = saved_root;
        self.current_ret_ty = saved_ret;
        result
    }

    fn check_func_body(&mut self, func: &FuncDecl, _ret_ty: &Type) -> CheckResult<()> {
        for param in &func.params {
            let ty = if param.is_borrow {
                crate::checker::make_borrow_type(&param.ty)
            } else {
                (*param.ty).clone()
            };
            self.define_symbol(&param.name, ty, false)?;
        }
        self.check_block(&func.body)
    }

    /// Tail-expression typing: a non-void function must end in an
    /// expression statement whose recorded type is assignable to the
    /// declared return type (Zig reads `expr_types` the same way).
    fn check_tail_type(&mut self, func: &FuncDecl, ret_ty: &Type) -> CheckResult<()> {
        if matches!(ret_ty, Type::Primitive(sla_ast::types::Primitive::VoidType)) {
            return Ok(());
        }
        let tail_ty = block_tail_type(self, &func.body);
        match tail_ty {
            Some(ty) => {
                if value_assignable_to(ret_ty, &ty) {
                    Ok(())
                } else {
                    Err(Diagnostic::new(
                        ErrorKind::CompileError,
                        format!(
                            "return type mismatch in `{}`: expected {}, actual {}",
                            func.name,
                            crate::types_util::type_desc(ret_ty),
                            crate::types_util::type_desc(&ty)
                        ),
                    ))
                }
            }
            None => Err(Diagnostic::new(
                ErrorKind::CompileError,
                format!("missing return value in non-void function `{}`", func.name),
            )),
        }
    }

    /// Test bodies check like void functions with no parameters.
    pub fn check_test(&mut self, test: &TestDecl) -> CheckResult<()> {
        let frame = self.scopes.push_frame();
        let saved_root = self.fn_root_depth;
        let saved_ret = self
            .current_ret_ty
            .replace(crate::types_util::void_ty());
        self.fn_root_depth = frame;
        let result = self.check_block(&test.body);
        self.scopes.pop_frame();
        self.fn_root_depth = saved_root;
        self.current_ret_ty = saved_ret;
        result
    }

    /// Mirrors Zig `checkBlock` (type_checker.zig:2618): a fresh child
    /// scope per block; on exit, the still-active non-internal
    /// bindings of the block's own frame are recorded as cleanups on
    /// the block's last statement (unless it is a terminator).
    pub fn check_block(&mut self, body: &[Node]) -> CheckResult<()> {
        self.scopes.push_frame();
        let mut result = Ok(());
        for stmt in body {
            if let Err(e) = self.check_stmt(stmt) {
                result = Err(e);
                break;
            }
        }
        let frame = self.scopes.pop_frame();
        if result.is_ok() {
            if let Some(last) = body.last() {
                if !stmt_terminates(last) {
                    let names = block_exit_cleanups(&frame.bindings);
                    if !names.is_empty() {
                        self.cleanups
                            .insert(crate::types_util::node_key_of(last), names);
                    }
                }
            }
        }
        result
    }
}

/// Bindings released when a block scope exits: still active, not
/// internal, not a raw-pointer primitive, and not the borrow source
/// of another still-active local (Zig's issue013 guard). Collected in
/// declaration order (D-C6).
fn block_exit_cleanups(bindings: &[crate::scope::Binding]) -> Vec<String> {
    let mut names = Vec::new();
    for b in bindings {
        if b.state != ValueState::Active {
            continue;
        }
        if is_internal_symbol(&b.name) {
            continue;
        }
        if matches!(b.ty, Type::Primitive(sla_ast::types::Primitive::RawPtr)) {
            continue;
        }
        let borrowed_by_active = bindings.iter().any(|other| {
            other.state == ValueState::Active
                && other.borrow_source.as_deref() == Some(b.name.as_str())
        });
        if borrowed_by_active {
            continue;
        }
        names.push(b.name.clone());
    }
    names
}

fn redecl(name: &str) -> Diagnostic {
    Diagnostic::new(
        ErrorKind::Redeclaration,
        format!("Redeclaration: top-level name `{name}` is already used"),
    )
}

/// The recorded type of a block's tail expression, if the last
/// statement is an expression statement (Zig `blockTailExprType`).
pub fn block_tail_type(checker: &Checker, body: &[Node]) -> Option<Type> {
    let last = body.last()?;
    if let Node::ExprStmt(inner) = last {
        return checker
            .expr_types
            .get(&crate::types_util::node_key_of(inner))
            .cloned();
    }
    None
}

/// Whether a statement syntactically terminates its block (return /
/// break / continue / terminating expression statement).
pub fn stmt_terminates(node: &Node) -> bool {
    match node {
        Node::ReturnStmt(_) | Node::BreakStmt(_) | Node::ContinueStmt(_) => true,
        Node::ExprStmt(inner) => expr_terminates(inner),
        Node::BlockStmt(b) => b.body.last().map(stmt_terminates).unwrap_or(false),
        _ => false,
    }
}

/// Whether a block's tail terminates it.
pub fn block_terminates(body: &[Node]) -> bool {
    body.last().map(stmt_terminates).unwrap_or(false)
}

/// Whether an expression never falls through (panic builtins, if with
/// two terminating branches, unsafe block with terminating tail).
pub fn expr_terminates(node: &Node) -> bool {
    match node {
        Node::CallExpr(c) => {
            c.callee.is_none() && crate::checker::is_panic_builtin_name(&c.func_name)
        }
        Node::IfExpr(i) => match &i.else_block {
            Some(else_block) => {
                block_terminates(&i.then_block) && block_terminates(else_block)
            }
            None => false,
        },
        Node::UnsafeExpr(u) => block_terminates(&u.body),
        _ => false,
    }
}

/// Fallback-diagnostic helper mirroring Zig's `checkStmt` wrapper
/// (type_checker.zig:2791): kept for parity of wording where a caller
/// needs a message for a bare kind.
pub fn stmt_fallback_message(node: &Node) -> String {
    format!("checkStmt failed at node tag {}", node_tag(node))
}

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
use crate::types_util::{is_internal_symbol, node_key_of, value_assignable_to};
use sla_ast::decls::{FuncDecl, Program, TestDecl, TypeAliasComponent};
use sla_ast::node::Node;
use sla_ast::types::Type;
use std::collections::HashSet;

impl Checker {
    /// Mirrors Zig `checkProgram`. Registration happens in passes so
    /// declarations may reference each other in any order.
    pub fn check_program(&mut self, program: &Program) -> CheckResult<()> {
        // R-3 (J-5): check-hook validation + the macro_param_audit
        // run before any other pass names a fault.
        self.apply_check_hooks(program)?;
        // R-3 (J-1): sublanguage binding + shader body checks.
        self.apply_sublanguage_checks(program)?;
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
                            generics: s.generics.clone(),
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
                    self.register_enum_variants(
                        &e.name,
                        e.variants
                            .iter()
                            .map(|v| crate::checker::EnumVariantInfo {
                                generics: e.generics.clone(),
                                name: v.name.clone(),
                                fields: v
                                    .fields
                                    .iter()
                                    .map(|f| {
                                        (f.name.clone(), (*f.ty).clone())
                                    })
                                    .collect(),
                            })
                            .collect(),
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
                Node::UsingDecl(u) => {
                    // Zig checkProgram (:2142): a top-level `using`
                    // registers its NORMALIZED path on the global
                    // scope, where the `using` static-extension
                    // resolver later collects it.
                    let normalized =
                        crate::types_util::normalize_using_path(&u.path);
                    self.scopes.frames[0]
                        .using_modules
                        .push(normalized);
                }
                Node::TraitDecl(t) => {
                    if !type_names.insert(t.name.clone()) {
                        return Err(redecl(&t.name));
                    }
                    trait_names.insert(t.name.clone());
                    self.register_trait(
                        &t.name,
                        crate::checker::TraitInfo {
                            supertraits: t.supertraits.clone(),
                            methods: t
                                .methods
                                .iter()
                                .map(|m| crate::checker::TraitMethodInfo {
                                    name: m.name.clone(),
                                    params: m
                                        .params
                                        .iter()
                                        .map(crate::checker::ParamSig::from)
                                        .collect(),
                                    ret_ty: (*m.ret_ty).clone(),
                                })
                                .collect(),
                        },
                    );
                }
                _ => {}
            }
        }
        // Supertraits must exist (Zig checkProgram validates them
        // with a bare `return TypeError.UndefinedVariable`).
        for decl in &program.decls {
            if let Node::TraitDecl(t) = decl {
                for sup in &t.supertraits {
                    if !trait_names.contains(sup) {
                        return Err(Diagnostic::new(
                            ErrorKind::UndefinedVariable,
                            format!("unknown supertrait `{sup}` of trait `{}`", t.name),
                        ));
                    }
                }
            }
        }
        // Pass 2: functions, macros, impls, overloads — registered
        // under Zig's symbol keys (type_checker.zig checkProgram):
        // inherent methods as `{Type}_{method}`, trait methods as
        // `{Type}__{Trait}_{method}`, overloads as `{Type}__{op}`.
        //
        // R-6 (F2/F4): plain functions are grouped by name first
        // (declaration order preserved). At a group's first
        // declaration the walk decides: one member registers as
        // before; several members with an identical repeated
        // signature are a Redeclaration; several distinct
        // signatures form an overload set (check_overload.rs).
        // Function, macro and top-level const names share one
        // namespace here (Zig rejects a const reusing a function
        // name with `Redeclaration: const ... is already defined`).
        let mut fn_groups: std::collections::HashMap<String, Vec<&FuncDecl>> =
            std::collections::HashMap::new();
        for decl in &program.decls {
            if let Node::FuncDecl(f) = decl {
                // Source-level `extern "C"` declarations are NOT
                // plain functions: they pair with their prototypes
                // and promote into extern overload sets
                // (check_overload2.rs), exactly as contract
                // declarations do. They never enter fn_groups.
                if !(f.is_extern && (f.abi.is_some() || f.is_variadic)) {
                    fn_groups.entry(f.name.clone()).or_default().push(f);
                }
            }
        }
        let mut handled_fn_names: HashSet<String> = HashSet::new();
        let mut top_names: HashSet<String> = HashSet::new();
        for decl in &program.decls {
            match decl {
                Node::FuncDecl(f) => {
                    if f.is_extern && (f.abi.is_some() || f.is_variadic) {
                        // Source-level extern declaration
                        // (prototype or @no_mangle definition):
                        // register under the contract discipline.
                        // The top-level name is claimed so a plain
                        // function/macro/const cannot reuse it.
                        if !handled_fn_names.contains(&f.name) {
                            top_names.insert(f.name.clone());
                            handled_fn_names.insert(f.name.clone());
                        }
                        self.register_source_extern(f)?;
                    } else if handled_fn_names.insert(f.name.clone()) {
                        if !top_names.insert(f.name.clone()) {
                            return Err(Diagnostic::new(
                                ErrorKind::Redeclaration,
                                format!(
                                    "Redeclaration: function `{}` is already defined",
                                    f.name
                                ),
                            ));
                        }
                        let group = &fn_groups[&f.name];
                        if group.len() == 1 {
                            self.register_func(f);
                        } else {
                            let sigs: Vec<crate::checker::FuncSig> = group
                                .iter()
                                .map(|d| crate::check_overload::sig_of_decl(d))
                                .collect();
                            for i in 1..sigs.len() {
                                if sigs[..i].iter().any(|s| {
                                    crate::check_overload::sigs_identical(
                                        s, &sigs[i],
                                    )
                                }) {
                                    if let Some(diag) =
                                        Checker::check_return_only_redeclaration(
                                            &f.name, &sigs,
                                        )
                                    {
                                        return Err(diag);
                                    }
                                    return Err(Diagnostic::new(
                                        ErrorKind::Redeclaration,
                                        format!(
                                            "Redeclaration: function `{}` is already defined",
                                            f.name
                                        ),
                                    ));
                                }
                            }
                            if let Some(g) =
                                group.iter().find(|d| !d.generics.is_empty())
                            {
                                self.func_generics
                                    .insert(f.name.clone(), g.generics.clone());
                            }
                            self.func_overload_sets
                                .insert(f.name.clone(), sigs);
                        }
                    }
                }
                Node::MacroDecl(m) => {
                    // R-6 (F2): duplicate macro parameters and
                    // duplicate macro names are Redeclarations
                    // (Zig registerMacro / defineSymbol paths).
                    let mut seen_params: HashSet<String> = HashSet::new();
                    for p in &m.params {
                        if !seen_params.insert(p.clone()) {
                            return Err(Diagnostic::new(
                                ErrorKind::Redeclaration,
                                format!(
                                    "Redeclaration: symbol `{p}` is already defined"
                                ),
                            ));
                        }
                    }
                    if !top_names.insert(m.name.clone()) {
                        return Err(Diagnostic::new(
                            ErrorKind::Redeclaration,
                            format!(
                                "Redeclaration: macro `{}` is already defined",
                                m.name
                            ),
                        ));
                    }
                    self.register_macro(m);
                }
                Node::ConstStmt(c) => {
                    // R-6 (F2): a top-level const shares the
                    // declaration namespace; a collision with a
                    // function, macro or earlier const is a
                    // Redeclaration naming the const.
                    if !top_names.insert(c.name.clone()) {
                        return Err(Diagnostic::new(
                            ErrorKind::Redeclaration,
                            format!(
                                "Redeclaration: const `{}` is already defined",
                                c.name
                            ),
                        ));
                    }
                }
                Node::ImplDecl(imp) => {
                    let target_name = target_type_name(&imp.target_ty)?;
                    if let Some(trait_name) = &imp.trait_name {
                        let key = format!("{trait_name}|{target_name}");
                        if !self.trait_impls.insert(key) {
                            return Err(Diagnostic::new(
                                ErrorKind::Redeclaration,
                                format!(
                                    "Redeclaration: impl `{trait_name}` for `{target_name}` is already defined"
                                ),
                            ));
                        }
                    }
                    for method in &imp.methods {
                        let Node::FuncDecl(f) = method else {
                            return Err(Diagnostic::new(
                                ErrorKind::CompileError,
                                "impl block contains a non-function member",
                            ));
                        };
                        let method_key = match &imp.trait_name {
                            Some(trait_name) => format!(
                                "{target_name}__{trait_name}_{}",
                                f.name
                            ),
                            None => format!("{target_name}_{}", f.name),
                        };
                        if self.funcs.contains_key(&method_key) {
                            return Err(Diagnostic::new(
                                ErrorKind::Redeclaration,
                                format!(
                                    "Redeclaration: method `{}` for `{target_name}` is already defined",
                                    f.name
                                ),
                            ));
                        }
                        self.register_func_as(&method_key, f);
                    }
                }
                Node::OverloadDecl(o) => {
                    let target_name = target_type_name(&o.target_ty)?;
                    for method in &o.methods {
                        let Node::FuncDecl(f) = method else {
                            return Err(Diagnostic::new(
                                ErrorKind::CompileError,
                                format!(
                                    "overload `{target_name}` contains a non-function member"
                                ),
                            ));
                        };
                        let op = f.operator.ok_or_else(|| {
                            Diagnostic::new(
                                ErrorKind::CompileError,
                                format!(
                                    "overload `{target_name}` methods must use an operator function"
                                ),
                            )
                        })?;
                        let op_name = match op {
                            sla_ast::exprs::BinaryOp::Add => "op_add",
                            sla_ast::exprs::BinaryOp::Sub => "op_sub",
                            sla_ast::exprs::BinaryOp::Mul => "op_mul",
                            sla_ast::exprs::BinaryOp::Div => "op_div",
                            _ => {
                                return Err(Diagnostic::new(
                                    ErrorKind::CompileError,
                                    format!(
                                        "unsupported overload operator `{}` in overload `{target_name}`",
                                        f.name
                                    ),
                                ))
                            }
                        };
                        let symbol = format!("{target_name}__{op_name}");
                        self.overloads
                            .entry(format!("{target_name}|{op_name}"))
                            .or_default()
                            .push(symbol.clone());
                        self.register_func_as(&symbol, f);
                    }
                }
                _ => {}
            }
        }
        // Pass 2.5 (R-4): register top-level consts into the global
        // scope so functions/tests can use them — Zig checkProgram
        // (type_checker.zig:2271-2290) does this after the macro
        // bodies and before function checking: each const's value
        // expression is checked in the global scope, the declared
        // type (or the value's type) must accept the value type,
        // and the const is defined as an immutable global symbol.
        // Without this pass every top-level const use fell to
        // UndefinedVariable on the test-codegen form (the Check
        // form's decl-only parser skips bodies, which is why
        // `sla check` accepted these programs while `sla test`
        // rejected them). Name collision with a function, macro or
        // earlier const is the Pass-2 Redeclaration above.
        for decl in &program.decls {
            if let Node::ConstStmt(c) = decl {
                let val_ty = self.check_expr(&c.value)?;
                let declared_ty = match &c.ty {
                    Some(t) => (**t).clone(),
                    None => val_ty.clone(),
                };
                if c.ty.is_some() {
                    if let (Type::Array(_), Type::Array(_)) = (&declared_ty, &val_ty) {
                        self.expr_types
                            .insert(node_key_of(&c.value), declared_ty.clone());
                    }
                }
                if !value_assignable_to(&declared_ty, &val_ty) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch in const {}: declared tag={}, val tag={}",
                            c.name,
                            crate::types_util::type_desc(&declared_ty),
                            crate::types_util::type_desc(&val_ty),
                        ),
                    ));
                }
                self.define_symbol(&c.name, declared_ty, true)?;
            }
        }
        // Pass 3: check bodies. Impl blocks go through the trait
        // contract check (`check_impl`); overload bodies check as
        // regular functions, as in Zig.
        for decl in &program.decls {
            match decl {
                Node::FuncDecl(f) => {
                    if !f.is_decl_only && !f.is_extern {
                        self.check_func(f)?;
                    }
                }
                Node::TestDecl(t) => self.check_test(t)?,
                // R-3 (J-3): typed macro bodies are checked with
                // their declared parameter types visible.
                Node::MacroDecl(m) => self.check_macro_body(m)?,
                Node::ImplDecl(imp) => self.check_impl(imp)?,
                Node::OverloadDecl(o) => {
                    for method in &o.methods {
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
        // R-3: borrow-origin declaration gates + per-function
        // ledger reset; the current function is remembered for
        // the return gate.
        self.origin_check_decl(func)?;
        self.origin_reset();
        self.current_func = Some(func.clone());
        let ret_ty = (*func.ret_ty).clone();
        let frame = self.scopes.push_frame();
        let saved_root = self.fn_root_depth;
        let saved_ret = self.current_ret_ty.replace(ret_ty.clone());
        self.fn_root_depth = frame;

        let mut result = if func.is_arena {
            let frame = self.scopes.depth();
            crate::check_arena::push_arena_frame(frame);
            let r = self.check_func_body(func, &ret_ty);
            crate::check_arena::pop_arena_frame();
            r
        } else {
            self.check_func_body(func, &ret_ty)
        };
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

    /// Tail-expression typing, mirroring the tail check at the end
    /// of Zig `checkFunc` (type_checker.zig:2596-2604) EXACTLY:
    /// Zig has NO missing-return check at all. It only type-checks
    /// the tail when ALL of these hold: the return type is non-void,
    /// the body is non-empty, the last statement is an expression
    /// statement, and that statement does not terminate. Every
    /// other shape passes this stage untouched:
    ///   * a trailing `return` statement (INT-1: the previous port
    ///     invented a "missing return value" error here — Zig's
    ///     `return_stmt` arm already type-checked the value);
    ///   * a terminating tail expression (e.g. an `if` whose two
    ///     branches both `return`);
    ///   * a non-expression tail (let / assignment / loop / ...) or
    ///     an empty body — Zig simply never looks at them here.
    fn check_tail_type(&mut self, func: &FuncDecl, ret_ty: &Type) -> CheckResult<()> {
        if matches!(ret_ty, Type::Primitive(sla_ast::types::Primitive::VoidType)) {
            return Ok(());
        }
        let Some(last) = func.body.last() else {
            return Ok(());
        };
        let Node::ExprStmt(inner) = last else {
            return Ok(());
        };
        if stmt_terminates(last) {
            return Ok(());
        }
        // Zig: `expr_types.get(last) orelse return TypeError.CompileError`
        // (no setError on that path). Practically unreachable: the
        // body was just checked, so the tail type was recorded.
        let Some(ty) = self.expr_types.get(&node_key_of(inner)).cloned() else {
            return Err(Diagnostic::new(
                ErrorKind::CompileError,
                "function tail expression has no recorded type",
            ));
        };
        if value_assignable_to(ret_ty, &ty) {
            Ok(())
        } else {
            Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "TypeMismatch in function tail expression: expected {}, actual {}",
                    crate::types_util::type_desc(ret_ty),
                    crate::types_util::type_desc(&ty)
                ),
            ))
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

/// Mirrors Zig `typeName`: only user-defined types have a name;
/// anything else is a CompileError.
pub fn target_type_name(ty: &Type) -> CheckResult<String> {
    match ty {
        Type::UserDefined(u) => Ok(u.name.clone()),
        _ => Err(Diagnostic::new(
            ErrorKind::CompileError,
            "type name is only defined for user-defined types",
        )),
    }
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
        // Mirrors Zig `exprTerminates` match arm: a match whose
        // every case body terminates (and which has cases at all)
        // terminates.
        Node::MatchExpr(m) => {
            !m.cases.is_empty()
                && m.cases.iter().all(|c| block_terminates(&c.body))
        }
        // Mirrors Zig `exprTerminates` switch arm (#7 there): an
        // all-diverging switch terminates only when a `default`
        // arm exists, so no value can fall through.
        Node::SwitchExpr(s) => {
            !s.cases.is_empty()
                && s.cases
                    .iter()
                    .any(|c| crate::types_util::is_switch_default_pattern(&c.pattern))
                && s.cases.iter().all(|c| block_terminates(&c.body))
        }
        _ => false,
    }
}

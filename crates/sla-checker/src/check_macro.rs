//! Recursive macro analyses, mirroring the checker's own macro
//! helpers (type_checker.zig:2332-2550): whether a macro can
//! `?`-propagate at all (`macroHasTry`), whether a given parameter
//! is a try source (`macroParamIsTrySource`), and the recursive
//! form of parameter consumption (`macroParamConsumption`) that
//! forwards through nested macro calls. The direct, single-body
//! analyses live in rules.rs (control_flow_rules); these versions
//! recurse across the macro table, with a visiting set cutting
//! recursion cycles to `false` / `Never`, exactly as Zig's
//! `MacroConsumptionKey` visiting map does.

use crate::checker::Checker;
use crate::rules::{
    macro_body_has_try, macro_param_consumption, macro_param_is_try_source,
    MacroParamConsumption,
};
use crate::types_util::root_identifier_deep;
use crate::diag::CheckResult;
use sla_ast::decls::MacroDecl;
use sla_ast::node::Node;
use sla_ast::types::Type;
use std::collections::HashSet;

impl Checker {
    /// Mirrors Zig `macroHasTry`: the body directly contains a
    /// `?`, or calls (transitively) a macro that does.
    pub fn macro_has_try(&self, mac: &MacroDecl) -> bool {
        let mut visiting = HashSet::new();
        self.macro_has_try_recursive(mac, &mut visiting)
    }

    fn macro_has_try_recursive(
        &self,
        mac: &MacroDecl,
        visiting: &mut HashSet<String>,
    ) -> bool {
        if !visiting.insert(mac.name.clone()) {
            return false;
        }
        let result = macro_body_has_try(&mac.body)
            || mac
                .body
                .iter()
                .any(|n| self.macro_node_calls_try_macro(n, visiting));
        visiting.remove(&mac.name);
        result
    }

    fn macro_node_calls_try_macro(
        &self,
        node: &Node,
        visiting: &mut HashSet<String>,
    ) -> bool {
        match node {
            Node::CallExpr(c) => match self.macros.get(&c.func_name) {
                Some(nested) => self.macro_has_try_recursive(nested, visiting),
                None => false,
            },
            Node::ExprStmt(e) => self.macro_node_calls_try_macro(e, visiting),
            Node::AssignStmt(a) => self.macro_node_calls_try_macro(&a.value, visiting),
            Node::LetStmt(l) => self.macro_node_calls_try_macro(&l.value, visiting),
            Node::ReturnStmt(r) => r
                .value
                .as_ref()
                .map(|v| self.macro_node_calls_try_macro(v, visiting))
                .unwrap_or(false),
            Node::BlockStmt(b) => b
                .body
                .iter()
                .any(|n| self.macro_node_calls_try_macro(n, visiting)),
            Node::UnsafeExpr(u) => u
                .body
                .iter()
                .any(|n| self.macro_node_calls_try_macro(n, visiting)),
            Node::IfExpr(i) => {
                i.then_block
                    .iter()
                    .any(|n| self.macro_node_calls_try_macro(n, visiting))
                    || i.else_block
                        .as_ref()
                        .map(|b| {
                            b.iter()
                                .any(|n| self.macro_node_calls_try_macro(n, visiting))
                        })
                        .unwrap_or(false)
            }
            Node::ForStmt(f) => f
                .body
                .iter()
                .any(|n| self.macro_node_calls_try_macro(n, visiting)),
            Node::WhileStmt(w) => w
                .body
                .iter()
                .any(|n| self.macro_node_calls_try_macro(n, visiting)),
            _ => false,
        }
    }

    /// Mirrors Zig `macroParamIsTrySource`: the parameter is
    /// directly `?`-propagated in the body, or forwarded into a
    /// nested macro position that is a try source there.
    pub fn macro_param_is_try_source(&self, mac: &MacroDecl, index: usize) -> bool {
        let mut visiting = HashSet::new();
        self.macro_param_is_try_source_recursive(mac, index, &mut visiting)
    }

    fn macro_param_is_try_source_recursive(
        &self,
        mac: &MacroDecl,
        index: usize,
        visiting: &mut HashSet<(String, usize)>,
    ) -> bool {
        if index >= mac.params.len() {
            return false;
        }
        if !visiting.insert((mac.name.clone(), index)) {
            return false;
        }
        let name = mac.params[index].clone();
        let result = macro_param_is_try_source(&mac.body, &name)
            || mac
                .body
                .iter()
                .any(|n| self.macro_node_forwards_try_source(n, &name, visiting));
        visiting.remove(&(mac.name.clone(), index));
        result
    }

    fn macro_node_forwards_try_source(
        &self,
        node: &Node,
        name: &str,
        visiting: &mut HashSet<(String, usize)>,
    ) -> bool {
        match node {
            Node::CallExpr(c) => {
                if let Some(nested) = self.macros.get(&c.func_name).cloned() {
                    for (i, arg) in c.args.iter().enumerate() {
                        if root_identifier_deep(arg) == Some(name)
                            && self.macro_param_is_try_source_recursive(
                                &nested, i, visiting,
                            )
                        {
                            return true;
                        }
                    }
                }
                false
            }
            Node::ExprStmt(e) => {
                self.macro_node_forwards_try_source(e, name, visiting)
            }
            Node::AssignStmt(a) => {
                self.macro_node_forwards_try_source(&a.value, name, visiting)
            }
            Node::LetStmt(l) => {
                self.macro_node_forwards_try_source(&l.value, name, visiting)
            }
            Node::ReturnStmt(r) => r
                .value
                .as_ref()
                .map(|v| self.macro_node_forwards_try_source(v, name, visiting))
                .unwrap_or(false),
            Node::BlockStmt(b) => b
                .body
                .iter()
                .any(|n| self.macro_node_forwards_try_source(n, name, visiting)),
            Node::UnsafeExpr(u) => u
                .body
                .iter()
                .any(|n| self.macro_node_forwards_try_source(n, name, visiting)),
            Node::IfExpr(i) => {
                i.then_block
                    .iter()
                    .any(|n| self.macro_node_forwards_try_source(n, name, visiting))
                    || i.else_block
                        .as_ref()
                        .map(|b| {
                            b.iter().any(|n| {
                                self.macro_node_forwards_try_source(n, name, visiting)
                            })
                        })
                        .unwrap_or(false)
            }
            Node::ForStmt(f) => f
                .body
                .iter()
                .any(|n| self.macro_node_forwards_try_source(n, name, visiting)),
            Node::WhileStmt(w) => w
                .body
                .iter()
                .any(|n| self.macro_node_forwards_try_source(n, name, visiting)),
            _ => false,
        }
    }

    /// Mirrors Zig `macroParamConsumption` (the checker-side
    /// recursive form): the direct body analysis wins when it is
    /// not `Never`; otherwise consumption forwarded through nested
    /// macro calls is folded in sequence order.
    pub fn macro_param_consumption_recursive(
        &self,
        mac: &MacroDecl,
        index: usize,
    ) -> MacroParamConsumption {
        let mut visiting = HashSet::new();
        self.macro_param_consumption_inner(mac, index, &mut visiting)
    }

    fn macro_param_consumption_inner(
        &self,
        mac: &MacroDecl,
        index: usize,
        visiting: &mut HashSet<(String, usize)>,
    ) -> MacroParamConsumption {
        if index >= mac.params.len() {
            return MacroParamConsumption::Never;
        }
        if !visiting.insert((mac.name.clone(), index)) {
            return MacroParamConsumption::Never;
        }
        let name = mac.params[index].clone();
        let direct = macro_param_consumption(&mac.body, &name);
        let result = if direct != MacroParamConsumption::Never {
            direct
        } else {
            let mut effect = MacroParamConsumption::Never;
            for node in &mac.body {
                effect = sequence(
                    effect,
                    self.macro_node_forwarded_consumption(node, &name, visiting),
                );
            }
            effect
        };
        visiting.remove(&(mac.name.clone(), index));
        result
    }

    fn macro_node_forwarded_consumption(
        &self,
        node: &Node,
        name: &str,
        visiting: &mut HashSet<(String, usize)>,
    ) -> MacroParamConsumption {
        match node {
            Node::CallExpr(c) => {
                let mut effect = MacroParamConsumption::Never;
                if let Some(nested) = self.macros.get(&c.func_name).cloned() {
                    for (i, arg) in c.args.iter().enumerate() {
                        if root_identifier_deep(arg) == Some(name) {
                            effect = sequence(
                                effect,
                                self.macro_param_consumption_inner(
                                    &nested, i, visiting,
                                ),
                            );
                        }
                    }
                }
                for arg in &c.args {
                    effect = sequence(
                        effect,
                        self.macro_node_forwarded_consumption(arg, name, visiting),
                    );
                }
                effect
            }
            Node::ExprStmt(e) => {
                self.macro_node_forwarded_consumption(e, name, visiting)
            }
            Node::AssignStmt(a) => {
                self.macro_node_forwarded_consumption(&a.value, name, visiting)
            }
            Node::LetStmt(l) => {
                self.macro_node_forwarded_consumption(&l.value, name, visiting)
            }
            Node::ReturnStmt(r) => r
                .value
                .as_ref()
                .map(|v| self.macro_node_forwarded_consumption(v, name, visiting))
                .unwrap_or(MacroParamConsumption::Never),
            Node::BlockStmt(b) => {
                let mut effect = MacroParamConsumption::Never;
                for child in &b.body {
                    effect = sequence(
                        effect,
                        self.macro_node_forwarded_consumption(child, name, visiting),
                    );
                }
                effect
            }
            Node::IfExpr(i) => {
                let cond =
                    self.macro_node_forwarded_consumption(&i.cond, name, visiting);
                let mut then_effect = MacroParamConsumption::Never;
                for child in &i.then_block {
                    then_effect = sequence(
                        then_effect,
                        self.macro_node_forwarded_consumption(child, name, visiting),
                    );
                }
                let mut else_effect = MacroParamConsumption::Never;
                if let Some(else_block) = &i.else_block {
                    for child in else_block {
                        else_effect = sequence(
                            else_effect,
                            self.macro_node_forwarded_consumption(
                                child, name, visiting,
                            ),
                        );
                    }
                }
                sequence(cond, branch(then_effect, else_effect))
            }
            Node::BinaryExpr(b) => sequence(
                self.macro_node_forwarded_consumption(&b.left, name, visiting),
                self.macro_node_forwarded_consumption(&b.right, name, visiting),
            ),
            Node::FieldExpr(f) => {
                self.macro_node_forwarded_consumption(&f.expr, name, visiting)
            }
            Node::IndexExpr(i) => sequence(
                self.macro_node_forwarded_consumption(&i.target, name, visiting),
                self.macro_node_forwarded_consumption(&i.index, name, visiting),
            ),
            Node::CastExpr(c) => {
                self.macro_node_forwarded_consumption(&c.expr, name, visiting)
            }
            Node::BorrowExpr(b) => {
                self.macro_node_forwarded_consumption(&b.expr, name, visiting)
            }
            Node::MoveExpr(m) => {
                self.macro_node_forwarded_consumption(&m.expr, name, visiting)
            }
            Node::DerefExpr(d) => {
                self.macro_node_forwarded_consumption(&d.expr, name, visiting)
            }
            Node::TryExpr(t) => {
                self.macro_node_forwarded_consumption(&t.expr, name, visiting)
            }
            Node::AwaitExpr(a) => {
                self.macro_node_forwarded_consumption(&a.expr, name, visiting)
            }
            Node::ForStmt(f) => {
                let mut effect =
                    self.macro_node_forwarded_consumption(&f.start, name, visiting);
                if let Some(end) = &f.end {
                    effect = sequence(
                        effect,
                        self.macro_node_forwarded_consumption(end, name, visiting),
                    );
                }
                let mut body_effect = MacroParamConsumption::Never;
                for child in &f.body {
                    body_effect = sequence(
                        body_effect,
                        self.macro_node_forwarded_consumption(child, name, visiting),
                    );
                }
                if body_effect != MacroParamConsumption::Never {
                    body_effect = MacroParamConsumption::Conditional;
                }
                sequence(effect, body_effect)
            }
            Node::WhileStmt(w) => {
                let cond =
                    self.macro_node_forwarded_consumption(&w.cond, name, visiting);
                let mut body_effect = MacroParamConsumption::Never;
                for child in &w.body {
                    body_effect = sequence(
                        body_effect,
                        self.macro_node_forwarded_consumption(child, name, visiting),
                    );
                }
                if body_effect != MacroParamConsumption::Never {
                    body_effect = MacroParamConsumption::Conditional;
                }
                sequence(cond, body_effect)
            }
            _ => MacroParamConsumption::Never,
        }
    }
}

/// Zig `sequenceMacroParamConsumption` (control_flow_rules).
fn sequence(
    a: MacroParamConsumption,
    b: MacroParamConsumption,
) -> MacroParamConsumption {
    if a == MacroParamConsumption::Always || b == MacroParamConsumption::Always {
        return MacroParamConsumption::Always;
    }
    if a == MacroParamConsumption::Conditional
        || b == MacroParamConsumption::Conditional
    {
        return MacroParamConsumption::Conditional;
    }
    MacroParamConsumption::Never
}

/// Zig `branchMacroParamConsumption` (control_flow_rules).
fn branch(
    a: MacroParamConsumption,
    b: MacroParamConsumption,
) -> MacroParamConsumption {
    if a == b {
        return a;
    }
    MacroParamConsumption::Conditional
}

// -- imported macros ---------------------------------------------------------

/// Mirrors lowering_rules `importedMacroExpressionResultKind`
/// (:3020): a purely name-based result classification for imported
/// macros used in expression position. Order matters: exact-name
/// groups first, then suffix rules, in Zig's sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportedMacroResultKind {
    RawPointer,
    Boolean,
    U8,
    U32,
    U64,
    I32,
    I64,
    F64,
    SliceU8,
}

pub fn imported_macro_expression_result_kind(
    name: &str,
) -> Option<ImportedMacroResultKind> {
    use ImportedMacroResultKind as K;
    const U64_NAMES: &[&str] = &[
        "ENV_ARGS_JSON",
        "ENV_VARS_JSON",
        "ENV_SPLIT_PATHS_JSON",
        "ENV_JOIN_PATHS_JSON",
        "SLA_FS_OPEN_READ",
        "SLA_FS_READ_TO_STRING",
        "SLA_FS_READ_FILE",
        "SLA_FS_METADATA",
    ];
    if U64_NAMES.contains(&name) {
        return Some(K::U64);
    }
    const PTR_NAMES: &[&str] = &[
        "JSON_PARSE",
        "SLA_BUF_ALLOC",
        "SLA_JSON_OBJECT_GET",
        "SLA_JSON_ARRAY_GET",
        "SLA_JSON_STRINGIFY",
        "PIN_GET_REF",
        "PIN_MUT_GET_REF",
        "PIN_MUT_GET_MUT",
        "PIN_MUT_GET_UNCHECKED_MUT",
        "FUTURE_READY_POLL_STATE",
        "FUTURE_PENDING_POLL_STATE",
        "RAW_WAKER_NEW",
        "RAW_WAKER_CLONE",
        "RAW_WAKER_VTABLE_NEW",
        "FUTURE_POLL_FN_STATE_NEW",
        "FUTURE_POLL_FN_STATE_POLL",
        "PTR_READ_U64",
        "WAKER_FROM_RAW",
        "CONTEXT_FROM_WAKER",
        "ONCE_NEW",
        "LAZY_LOCK_NEW",
        "POLL_VALUE_PTR",
    ];
    if PTR_NAMES.contains(&name)
        || name.ends_with("_PTR")
        || name.ends_with("_DATA")
        || name.ends_with("_AS_PTR")
        || name.ends_with("_ADD")
        || name.ends_with("_NULL")
    {
        return Some(K::RawPointer);
    }
    const BOOL_NAMES: &[&str] = &[
        "POLL_IS_READY",
        "POLL_IS_PENDING",
        "PIN_PTR_EQ",
        "PIN_MUT_PTR_EQ",
    ];
    if BOOL_NAMES.contains(&name) {
        return Some(K::Boolean);
    }
    const U8_NAMES: &[&str] = &[
        "SLA_BYTE_AT",
        "JSON_AS_BOOL",
        "SLA_JSON_AS_BOOL",
        "SLA_FS_EXISTS",
        "SLA_FS_IS_FILE",
        "SLA_FS_IS_DIR",
    ];
    if U8_NAMES.contains(&name)
        || name.ends_with("_GET_BOOL")
        || name.ends_with("_READ_U8")
    {
        return Some(K::U8);
    }
    if name.ends_with("_KIND") {
        return Some(K::U32);
    }
    if name.ends_with("_READ_U64") {
        return Some(K::U64);
    }
    if name.ends_with("_READ_I32") {
        return Some(K::I32);
    }
    if name == "JSON_AS_I64" || name == "SLA_JSON_AS_I64" || name.ends_with("_GET_I64")
    {
        return Some(K::I64);
    }
    if name == "JSON_AS_F64" || name.ends_with("_GET_F64") {
        return Some(K::F64);
    }
    if name.ends_with("_LEN") || name.ends_with("_COUNT") {
        return Some(K::I64);
    }
    if name.ends_with("_FROM_PARTS")
        || name.ends_with("_AS_BYTES")
        || name.ends_with("_BYTES")
    {
        return Some(K::SliceU8);
    }
    None
}

/// Mirrors Zig `makeImportedMacroExpressionResultType` (:1262).
pub fn imported_macro_result_type(kind: ImportedMacroResultKind) -> Type {
    use sla_ast::types::Primitive as P;
    match kind {
        ImportedMacroResultKind::RawPointer => crate::types_util::raw_ptr_ty(),
        ImportedMacroResultKind::Boolean => crate::types_util::bool_ty(),
        ImportedMacroResultKind::U8 => crate::types_util::prim(P::U8),
        ImportedMacroResultKind::U32 => crate::types_util::prim(P::U32),
        ImportedMacroResultKind::U64 => crate::types_util::u64_ty(),
        ImportedMacroResultKind::I32 => crate::types_util::prim(P::I32),
        ImportedMacroResultKind::I64 => crate::types_util::i64_ty(),
        ImportedMacroResultKind::F64 => crate::types_util::f64_ty(),
        ImportedMacroResultKind::SliceU8 => Type::UserDefined(
            sla_ast::types::UserDefined {
                name: "Slice".to_string(),
                generics: vec![crate::types_util::prim(P::U8)],
            },
        ),
    }
}

impl Checker {
    /// Mirrors the imported-macro call path of the Zig call arm:
    /// exact arity -> void; arity-minus-one with a single leading
    /// output -> the name-table expression result type (unknown
    /// names degrade to `infer`, as in Zig); anything else is
    /// InvalidArgsCount with Zig's message shape.
    pub fn check_imported_macro_call(
        &mut self,
        name: &str,
        call: &sla_ast::exprs::CallExpr,
    ) -> Option<CheckResult<Type>> {
        let info = self.imported_macros.get(name)?.clone();
        Some((|| {
            for arg in &call.args {
                self.check_expr(arg)?;
            }
            if call.args.len() == info.arity {
                return Ok(crate::types_util::void_ty());
            }
            if info.leading_outputs == 1 && call.args.len() + 1 == info.arity {
                return Ok(
                    match imported_macro_expression_result_kind(name) {
                        Some(kind) => imported_macro_result_type(kind),
                        None => Type::Infer,
                    },
                );
            }
            Err(crate::diag::Diagnostic::new(
                crate::diag::ErrorKind::InvalidArgsCount,
                format!(
                    "macro {name} expects {} args or {} args when using expression output, got {}",
                    info.arity,
                    info.arity.saturating_sub(1),
                    call.args.len()
                ),
            ))
        })())
    }
}

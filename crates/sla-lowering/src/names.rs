//! Pure name / call-shape facts, mirrored from the "Shared pure ..."
//! predicate block of lowering_rules.zig. Each function cites its Zig
//! definition line; bodies are line-for-line ports.

use sla_ast::{CallExpr, Node};

/// lowering_rules.zig:3999 `isDiscardName`.
pub fn is_discard_name(name: &str) -> bool {
    name == "_"
}

/// lowering_rules.zig:4066 `isInternalSymbol`. Only the typechecker
/// sentinel is internal; notably `tmp_N` registers are NOT internal and
/// do get released.
pub fn is_internal_symbol(name: &str) -> bool {
    name == "return_ty_sentinel"
}

/// lowering_rules.zig:4263 `isMainName`.
pub fn is_main_name(name: &str) -> bool {
    name == "main"
}

/// lowering_rules.zig:4108 `isPanicBuiltinName`.
pub fn is_panic_builtin_name(name: &str) -> bool {
    name == "panic" || name == "panic_msg"
}

/// lowering_rules.zig:4113 `isPanicBuiltinCall`.
pub fn is_panic_builtin_call(call: &CallExpr) -> bool {
    call.associated_target.is_none() && is_panic_builtin_name(&call.func_name)
}

/// lowering_rules.zig:4173 `isPrintlnCall` (name-only, target ignored).
pub fn is_println_call(call: &CallExpr) -> bool {
    call.func_name == "println"
}

/// lowering_rules.zig:4120 `isOptionNoneName`.
pub fn is_option_none_name(name: &str) -> bool {
    name == "None"
}

/// lowering_rules.zig:4056 `isStackAllocCall`.
pub fn is_stack_alloc_call(call: &CallExpr) -> bool {
    call.associated_target.is_none() && call.func_name == "stack_alloc"
}

/// lowering_rules.zig:4061 `isStackAllocNode`.
pub fn is_stack_alloc_node(node: &Node) -> bool {
    matches!(node, Node::CallExpr(call) if is_stack_alloc_call(call))
}

/// lowering_rules.zig (shared switch-default fact) `isSwitchDefaultPattern`.
pub fn is_switch_default_pattern(pattern: &Node) -> bool {
    matches!(pattern, Node::Identifier(name) if name == "default")
}

/// lowering_rules.zig:1171 `isTemporaryRegisterName`.
pub fn is_temporary_register_name(name: &str) -> bool {
    name.starts_with("tmp_")
}

/// lowering_rules.zig:6031 `rootIdentifier`: peel field/index/borrow/move
/// down to the root identifier, if any.
pub fn root_identifier(expr: &Node) -> Option<&str> {
    match expr {
        Node::Identifier(name) => Some(name),
        Node::FieldExpr(field) => root_identifier(&field.expr),
        Node::IndexExpr(idx) => root_identifier(&idx.target),
        Node::BorrowExpr(borrow) => root_identifier(&borrow.expr),
        Node::MoveExpr(mv) => root_identifier(&mv.expr),
        _ => None,
    }
}

/// lowering_rules.zig:5899 `borrowedIdentifierName` (`&name` shape only).
pub fn borrowed_identifier_name(expr: &Node) -> Option<&str> {
    if let Node::BorrowExpr(borrow) = expr {
        if let Node::Identifier(name) = &*borrow.expr {
            return Some(name);
        }
    }
    None
}

/// lowering_rules.zig:5881 `callArgPrefix`.
pub fn call_arg_prefix(arg: &Node) -> Option<u8> {
    match arg {
        Node::BorrowExpr(_) => Some(b'&'),
        Node::MoveExpr(_) => Some(b'^'),
        _ => None,
    }
}

/// A `&name` / `^name` call argument (lowering_rules.zig
/// `PrefixedIdentifierArg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefixedIdentifierArg<'a> {
    pub prefix: u8,
    pub name: &'a str,
}

/// lowering_rules.zig:5889 `prefixedIdentifierCallArg`.
pub fn prefixed_identifier_call_arg(arg: &Node) -> Option<PrefixedIdentifierArg<'_>> {
    let prefix = call_arg_prefix(arg)?;
    let inner = match arg {
        Node::BorrowExpr(borrow) => &borrow.expr,
        Node::MoveExpr(mv) => &mv.expr,
        _ => return None,
    };
    if let Node::Identifier(name) = &**inner {
        return Some(PrefixedIdentifierArg { prefix, name });
    }
    None
}

/// lowering_rules.zig:5757 `mangleMethodName` (`{ty}_{method}`).
pub fn mangle_method_name(ty_name: &str, method_name: &str) -> String {
    format!("{ty_name}_{method_name}")
}

/// lowering_rules.zig:5761 `mangleTraitMethodName` (`{ty}__{trait}_{method}`).
pub fn mangle_trait_method_name(ty_name: &str, trait_name: &str, method_name: &str) -> String {
    format!("{ty_name}__{trait_name}_{method_name}")
}

//! Pure name / call-shape facts, mirrored from the "Shared pure ..."
//! predicate block of lowering_rules.zig. Each function cites its Zig
//! definition line; bodies are line-for-line ports.

use sla_ast::{CallExpr, EnumPattern, Node};

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

/// lowering_rules.zig:1361 `isExcludedAggregateHomeName`:
/// user-defined type names that never receive an aggregate stack
/// home. These are runtime handle / cell types whose bindings are
/// not plain record values: string and collection headers are
/// served by their buffer registries, Cell/RefCell/Atomic* live
/// in heap cells by design (their addresses may escape into
/// owners), and the channel/concurrency handles are copyable
/// runtime words. The aggregate-home class is plain struct values
/// and tuples only.
pub fn is_excluded_aggregate_home_name(name: &str) -> bool {
    matches!(
        name,
        "String"
            | "Vec"
            | "GrowVec"
            | "VecDeque"
            | "HashMap"
            | "HashSet"
            | "BTreeMap"
            | "BTreeSet"
            | "Cell"
            | "RefCell"
            | "AtomicI32"
            | "AtomicUsize"
            | "AtomicPtr"
            | "AtomicBool"
            | "ManuallyDrop"
            | "Chan"
            | "Sender"
            | "Receiver"
            | "Mutex"
            | "RwLock"
    )
}

/// lowering_rules.zig:4157 `patternUsesResultMacros`: a pattern
/// over the `Result` enum (or an `Ok` / `Err` variant of any enum
/// spelling) dispatches through the RESULT_* macros.
pub fn pattern_uses_result_macros(enum_name: &str, variant_name: &str) -> bool {
    enum_name == "Result" || variant_name == "Ok" || variant_name == "Err"
}

/// lowering_rules.zig:4161 `patternUsesOptionMacros`: a pattern
/// over the `Option` enum (or a `Some` / `None` variant) dispatches
/// through the OPTION_* macros.
pub fn pattern_uses_option_macros(enum_name: &str, variant_name: &str) -> bool {
    enum_name == "Option" || variant_name == "Some" || variant_name == "None"
}

/// lowering_rules.zig:4177 `enumNameMatchesDecl`: a pattern's enum
/// name matches the declaration name exactly, or as a prefix
/// ending at an underscore (generic-instantiation spellings).
pub fn enum_name_matches_decl(pattern_name: &str, decl_name: &str) -> bool {
    if pattern_name == decl_name {
        return true;
    }
    if decl_name.len() <= pattern_name.len() {
        return false;
    }
    decl_name.starts_with(pattern_name)
        && decl_name.as_bytes()[pattern_name.len()] == b'_'
}

//! Type predicates and equality, mirroring the free helpers of
//! `type_checker.zig` and the few `lowering_rules.zig` predicates the
//! checker consults.
//!
//! The load-bearing subtlety (D-C8): the Zig checker has TWO type
//! equalities and picks per call site by hand —
//!   * `typesEqual` (type_checker.zig:5752) is WIDTH-INSENSITIVE:
//!     any integer type equals any integer type, any float any float,
//!     and `infer` is a wildcard;
//!   * `typesStructurallyEqual` (type_checker.zig:376) compares
//!     primitives strictly.
//! Both are ported as-is under distinct names; nothing is "fixed".

use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

pub fn prim(p: Primitive) -> Type {
    Type::Primitive(p)
}

pub fn i32_ty() -> Type {
    prim(Primitive::I32)
}
pub fn i64_ty() -> Type {
    prim(Primitive::I64)
}
pub fn u64_ty() -> Type {
    prim(Primitive::U64)
}
pub fn f64_ty() -> Type {
    prim(Primitive::F64)
}
pub fn bool_ty() -> Type {
    prim(Primitive::Boolean)
}
pub fn void_ty() -> Type {
    prim(Primitive::VoidType)
}
pub fn raw_ptr_ty() -> Type {
    prim(Primitive::RawPtr)
}

pub fn is_integer_primitive(p: Primitive) -> bool {
    matches!(
        p,
        Primitive::I8
            | Primitive::I16
            | Primitive::I32
            | Primitive::I64
            | Primitive::Isize
            | Primitive::U8
            | Primitive::U16
            | Primitive::U32
            | Primitive::U64
            | Primitive::Usize
            | Primitive::Integer
    )
}

pub fn is_float_primitive(p: Primitive) -> bool {
    matches!(p, Primitive::F32 | Primitive::F64 | Primitive::Float)
}

pub fn is_numeric_primitive(p: Primitive) -> bool {
    is_integer_primitive(p) || is_float_primitive(p)
}

pub fn is_integer_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(p) if is_integer_primitive(*p))
}

pub fn is_float_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(p) if is_float_primitive(*p))
}

pub fn is_numeric_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(p) if is_numeric_primitive(*p))
}

pub fn is_boolean_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(Primitive::Boolean))
}

/// `Type.isPrimitiveScalar`-style test used by the `var` statement arm:
/// a scalar primitive slot (Zig `var` Phase 1 restriction).
pub fn is_primitive_scalar(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(_))
}

/// Mirrors lowering_rules `isBorrowLikeType`: a borrow or a raw
/// pointer — values that transfer by reference, never by consumption.
pub fn is_borrow_like_type(ty: &Type) -> bool {
    matches!(ty, Type::Borrow(_)) || matches!(ty, Type::Primitive(Primitive::RawPtr))
}

/// Mirrors lowering_rules `isInternalSymbol` (the sentinel is the only
/// internal symbol; this port keeps the name test for injected
/// bindings even though the sentinel itself became a field, see D-C5).
pub fn is_internal_symbol(name: &str) -> bool {
    name == "return_ty_sentinel"
}

/// Zig `isDiscardName`: `_` and `_foo` bindings are never defined.
pub fn is_discard_name(name: &str) -> bool {
    name.starts_with('_')
}

/// Primitive's Zig `@tagName`.
pub fn primitive_tag(p: Primitive) -> &'static str {
    match p {
        Primitive::I8 => "i8",
        Primitive::I16 => "i16",
        Primitive::I32 => "i32",
        Primitive::I64 => "i64",
        Primitive::Isize => "isize",
        Primitive::U8 => "u8",
        Primitive::U16 => "u16",
        Primitive::U32 => "u32",
        Primitive::U64 => "u64",
        Primitive::Usize => "usize",
        Primitive::F32 => "f32",
        Primitive::F64 => "f64",
        Primitive::Integer => "integer",
        Primitive::Float => "float",
        Primitive::Boolean => "boolean",
        Primitive::RawPtr => "raw_ptr",
        Primitive::VoidType => "void_type",
    }
}

/// Mirrors Zig `typeDesc` (type_checker.zig:5824): a coarse label —
/// primitives by tag name, composites by union tag, user types by
/// name. (The Zig comment notes this deliberately replaced width-
/// collapsing labels; the labels still erase pointees and widths of
/// composites, e.g. every pointer is just "pointer".)
pub fn type_desc(ty: &Type) -> String {
    match ty {
        Type::Infer => "infer".to_string(),
        Type::Primitive(p) => primitive_tag(*p).to_string(),
        Type::Pointer(_) => "pointer".to_string(),
        Type::Borrow(_) => "borrow".to_string(),
        Type::Array(_) => "array".to_string(),
        Type::Tuple(_) => "tuple".to_string(),
        Type::Future(_) => "future".to_string(),
        Type::Closure(_) => "closure".to_string(),
        Type::FnPtr(_) => "fn_ptr".to_string(),
        Type::UserDefined(u) => u.name.clone(),
    }
}

/// Mirrors Zig `typesEqual` (type_checker.zig:5752) — the checker's
/// default equality. Source-verified properties:
///   * different union tags are NEVER equal (no pointer<->borrow
///     cross-comparison);
///   * primitives: exact match, else integer-class <-> integer-class
///     and float-class <-> float-class (WIDTH-INSENSITIVE);
///   * arrays compare length AND element type;
///   * fn_ptr compares the abi string as well (null only equals null);
///   * `infer` is a wildcard.
pub fn types_equal(a: &Type, b: &Type) -> bool {
    if matches!(a, Type::Infer) || matches!(b, Type::Infer) {
        return true;
    }
    match (a, b) {
        (Type::Primitive(pa), Type::Primitive(pb)) => {
            if pa == pb {
                return true;
            }
            if is_integer_primitive(*pa) && is_integer_primitive(*pb) {
                return true;
            }
            if is_float_primitive(*pa) && is_float_primitive(*pb) {
                return true;
            }
            false
        }
        (Type::Pointer(x), Type::Pointer(y)) => types_equal(x, y),
        (Type::Borrow(x), Type::Borrow(y)) => types_equal(x, y),
        (Type::Future(x), Type::Future(y)) => types_equal(x, y),
        (Type::Array(x), Type::Array(y)) => {
            x.len == y.len && types_equal(&x.elem, &y.elem)
        }
        (Type::Tuple(x), Type::Tuple(y)) => {
            x.elems.len() == y.elems.len()
                && x.elems
                    .iter()
                    .zip(y.elems.iter())
                    .all(|(u, v)| types_equal(u, v))
        }
        (Type::UserDefined(x), Type::UserDefined(y)) => {
            x.name == y.name
                && x.generics.len() == y.generics.len()
                && x.generics
                    .iter()
                    .zip(y.generics.iter())
                    .all(|(u, v)| types_equal(u, v))
        }
        (Type::Closure(x), Type::Closure(y)) => {
            x.params.len() == y.params.len()
                && x.params
                    .iter()
                    .zip(y.params.iter())
                    .all(|(u, v)| types_equal(u, v))
                && types_equal(&x.ret, &y.ret)
        }
        (Type::FnPtr(x), Type::FnPtr(y)) => {
            x.abi == y.abi
                && x.params.len() == y.params.len()
                && x.params
                    .iter()
                    .zip(y.params.iter())
                    .all(|(u, v)| types_equal(u, v))
                && types_equal(&x.ret, &y.ret)
        }
        _ => false,
    }
}

/// Mirrors Zig `typesStructurallyEqual` (type_checker.zig:376):
/// primitives compare STRICTLY (`p == b.primitive`).
pub fn types_structurally_equal(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Primitive(pa), Type::Primitive(pb)) => pa == pb,
        (Type::Pointer(x), Type::Pointer(y)) => types_structurally_equal(x, y),
        (Type::Borrow(x), Type::Borrow(y)) => types_structurally_equal(x, y),
        (Type::Array(x), Type::Array(y)) => {
            x.len == y.len && types_structurally_equal(&x.elem, &y.elem)
        }
        (Type::Tuple(x), Type::Tuple(y)) => {
            x.elems.len() == y.elems.len()
                && x.elems
                    .iter()
                    .zip(y.elems.iter())
                    .all(|(u, v)| types_structurally_equal(u, v))
        }
        (Type::UserDefined(x), Type::UserDefined(y)) => {
            x.name == y.name
                && x.generics.len() == y.generics.len()
                && x.generics
                    .iter()
                    .zip(y.generics.iter())
                    .all(|(u, v)| types_structurally_equal(u, v))
        }
        (Type::Future(x), Type::Future(y)) => types_structurally_equal(x, y),
        _ => false,
    }
}

/// Mirrors Zig `valueAssignableTo` (type_checker.zig:891): the
/// checker's assignment compatibility = `typesEqual`, plus a raw
/// pointer expected type accepts any pointer actual.
pub fn value_assignable_to(expected: &Type, actual: &Type) -> bool {
    if types_equal(expected, actual) {
        return true;
    }
    matches!(expected, Type::Primitive(Primitive::RawPtr))
        && matches!(actual, Type::Pointer(_))
}

/// Address-based identity of a node inside the borrowed AST,
/// mirroring the `*const ast.Node` keys of Zig's side tables
/// (`expr_types`, `cleanups`, `phi_cleanups`).
pub fn node_key_of(node: &Node) -> usize {
    node as *const Node as usize
}

/// Zig `@tagName(node.*)` for the whole `Node` union; used by the
/// fallback diagnostics the Zig wrappers synthesize.
pub fn node_tag(node: &Node) -> &'static str {
    match node {
        Node::Program(_) => "program",
        Node::StructDecl(_) => "struct_decl",
        Node::EnumDecl(_) => "enum_decl",
        Node::TraitDecl(_) => "trait_decl",
        Node::ImplDecl(_) => "impl_decl",
        Node::TypeAliasDecl(_) => "type_alias_decl",
        Node::OverloadDecl(_) => "overload_decl",
        Node::FuncDecl(_) => "func_decl",
        Node::MacroDecl(_) => "macro_decl",
        Node::ImportDecl(_) => "import_decl",
        Node::UsingDecl(_) => "using_decl",
        Node::TestDecl(_) => "test_decl",
        Node::LetStmt(_) => "let_stmt",
        Node::LetElseStmt(_) => "let_else_stmt",
        Node::LetDestructureStmt(_) => "let_destructure_stmt",
        Node::ConstStmt(_) => "const_stmt",
        Node::VarStmt(_) => "var_stmt",
        Node::AssignStmt(_) => "assign_stmt",
        Node::BlockStmt(_) => "block_stmt",
        Node::ExprStmt(_) => "expr_stmt",
        Node::ReturnStmt(_) => "return_stmt",
        Node::ForStmt(_) => "for_stmt",
        Node::WhileStmt(_) => "while_stmt",
        Node::BreakStmt(_) => "break_stmt",
        Node::ContinueStmt(_) => "continue_stmt",
        Node::ReleaseStmt(_) => "release_stmt",
        Node::Literal(_) => "literal",
        Node::Identifier(_) => "identifier",
        Node::GenericFuncRef(_) => "generic_func_ref",
        Node::IfExpr(_) => "if_expr",
        Node::SwitchExpr(_) => "switch_expr",
        Node::MatchExpr(_) => "match_expr",
        Node::UnsafeExpr(_) => "unsafe_expr",
        Node::AwaitExpr(_) => "await_expr",
        Node::InlineAsmExpr(_) => "inline_asm_expr",
        Node::BinaryExpr(_) => "binary_expr",
        Node::CallExpr(_) => "call_expr",
        Node::ClosureLiteral(_) => "closure_literal",
        Node::BorrowExpr(_) => "borrow_expr",
        Node::MoveExpr(_) => "move_expr",
        Node::DerefExpr(_) => "deref_expr",
        Node::CastExpr(_) => "cast_expr",
        Node::FieldExpr(_) => "field_expr",
        Node::StructLiteral(_) => "struct_literal",
        Node::EnumLiteral(_) => "enum_literal",
        Node::TupleLiteral(_) => "tuple_literal",
        Node::ArrayLiteral(_) => "array_literal",
        Node::RepeatArrayLiteral(_) => "repeat_array_literal",
        Node::IndexExpr(_) => "index_expr",
        Node::SliceExpr(_) => "slice_expr",
        Node::TryExpr(_) => "try_expr",
    }
}

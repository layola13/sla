//! Expression nodes, mirroring the expression section of the Zig
//! plugin's `src/ast.zig`. Representation mapping as in `decls.rs` (D-A1).

use crate::decls::Param;
use crate::node::Node;
use crate::types::Type;

/// Mirrors Zig `Literal`.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    IntVal(i64),
    FloatVal(f64),
    BoolVal(bool),
    StringVal(String),
}

/// Mirrors Zig `IfExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub cond: Box<Node>,
    /// Zig default: null.
    pub let_chain: Option<Vec<IfLetCond>>,
    pub then_block: Vec<Node>,
    pub else_block: Option<Vec<Node>>,
}

/// Mirrors Zig `IfLetCond`.
#[derive(Debug, Clone, PartialEq)]
pub struct IfLetCond {
    pub pattern: EnumPattern,
    pub value: Box<Node>,
}

/// Mirrors Zig `Case` (a `switch` arm).
#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    /// Literal or Identifier (for the default pattern, e.g. "default").
    pub pattern: Box<Node>,
    pub body: Vec<Node>,
}

/// Mirrors Zig `SwitchExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchExpr {
    pub val: Box<Node>,
    pub cases: Vec<Case>,
}

/// Mirrors Zig `MatchExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub val: Box<Node>,
    pub cases: Vec<MatchCase>,
}

/// Mirrors Zig `MatchCase`.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchCase {
    pub pattern: EnumPattern,
    /// Zig default: null.
    pub guard: Option<Box<Node>>,
    pub body: Vec<Node>,
}

/// Mirrors Zig `UnsafeExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnsafeExpr {
    pub body: Vec<Node>,
}

/// Mirrors Zig `AwaitExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct AwaitExpr {
    pub expr: Box<Node>,
}

/// Mirrors Zig `InlineAsmOperand`.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineAsmOperand {
    pub constraint: String,
    pub var_name: String,
}

/// Mirrors Zig `InlineAsmExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineAsmExpr {
    pub template: String,
    pub operands: Vec<InlineAsmOperand>,
}

/// Mirrors Zig `ClosureLiteral`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureLiteral {
    pub params: Vec<Param>,
    pub body: Box<Node>,
}

/// Mirrors Zig `EnumPattern`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumPattern {
    pub enum_name: String,
    pub variant_name: String,
    pub bindings: Vec<String>,
}

/// Mirrors Zig `BinaryOp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Spaceship,
    LogicalAnd,
    LogicalOr,
}

/// Mirrors Zig `BinaryExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub left: Box<Node>,
    pub right: Box<Node>,
}

/// Mirrors Zig `CallExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub func_name: String,
    /// Zig default: null.
    pub associated_target: Option<String>,
    pub generics: Vec<Type>,
    pub args: Vec<Node>,
    /// Immediately-invoked closure literal callee (`(|x| ...)(args)`).
    /// `None` for every ordinary named call; when set, `func_name` is ""
    /// and all name-keyed dispatch must be bypassed in favor of this node.
    /// Zig default: null.
    pub callee: Option<Box<Node>>,
}

/// Mirrors Zig `GenericFuncRef`.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericFuncRef {
    pub func_name: String,
    pub generics: Vec<Type>,
}

/// Mirrors Zig `BorrowExpr` (the `&expr` form).
#[derive(Debug, Clone, PartialEq)]
pub struct BorrowExpr {
    pub expr: Box<Node>,
}

/// Mirrors Zig `MoveExpr` (the `^expr` form).
#[derive(Debug, Clone, PartialEq)]
pub struct MoveExpr {
    pub expr: Box<Node>,
}

/// Mirrors Zig `DerefExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct DerefExpr {
    pub expr: Box<Node>,
}

/// Mirrors Zig `CastExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct CastExpr {
    pub expr: Box<Node>,
    pub ty: Box<Type>,
}

/// Mirrors Zig `FieldExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldExpr {
    pub expr: Box<Node>,
    pub field_name: String,
}

/// Mirrors Zig `StructLiteralField`.
#[derive(Debug, Clone, PartialEq)]
pub struct StructLiteralField {
    pub name: String,
    pub value: Box<Node>,
}

/// Mirrors Zig `StructLiteral`.
#[derive(Debug, Clone, PartialEq)]
pub struct StructLiteral {
    pub ty: Box<Type>,
    pub fields: Vec<StructLiteralField>,
    /// Zig default: null.
    pub update_expr: Option<Box<Node>>,
}

/// Mirrors Zig `EnumLiteralField`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumLiteralField {
    pub name: String,
    pub value: Box<Node>,
}

/// Mirrors Zig `EnumLiteral`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumLiteral {
    pub enum_name: String,
    pub variant_name: String,
    pub fields: Vec<EnumLiteralField>,
}

/// Mirrors Zig `TupleLiteral`.
#[derive(Debug, Clone, PartialEq)]
pub struct TupleLiteral {
    pub elements: Vec<Node>,
}

/// Mirrors Zig `ArrayLiteral`.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayLiteral {
    pub elements: Vec<Node>,
}

/// Mirrors Zig `RepeatArrayLiteral` (`[value; len]`).
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatArrayLiteral {
    pub value: Box<Node>,
    pub len: usize,
}

/// Mirrors Zig `IndexExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexExpr {
    pub target: Box<Node>,
    pub index: Box<Node>,
}

/// Mirrors Zig `SliceExpr`.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceExpr {
    pub target: Box<Node>,
    pub start: Box<Node>,
    pub end: Box<Node>,
}

/// Mirrors Zig `TryExpr` (the postfix `?` form, Zig's `try`).
#[derive(Debug, Clone, PartialEq)]
pub struct TryExpr {
    pub expr: Box<Node>,
}

//! Statement nodes, mirroring the statement section of the Zig plugin's
//! `src/ast.zig`. Representation mapping as in `decls.rs` (D-A1).

use crate::exprs::EnumPattern;
use crate::node::Node;
use crate::types::Type;

/// Mirrors Zig `LetStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub name: String,
    pub ty: Option<Box<Type>>,
    pub value: Box<Node>,
}

/// Mirrors Zig `LetElseStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct LetElseStmt {
    pub pattern: EnumPattern,
    pub value: Box<Node>,
    pub else_block: Vec<Node>,
}

/// Mirrors Zig `LetDestructureStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct LetDestructureStmt {
    pub names: Vec<String>,
    pub value: Box<Node>,
    /// Zig default: null.
    pub rest_name: Option<String>,
    /// Zig default: null.
    pub rest_alias: Option<String>,
    /// Zig default: false.
    pub is_slice: bool,
}

/// Mirrors Zig `ConstStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstStmt {
    pub name: String,
    pub ty: Option<Box<Type>>,
    pub value: Box<Node>,
}

/// Mirrors Zig `VarStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct VarStmt {
    pub name: String,
    pub ty: Box<Type>,
}

/// Mirrors Zig `AssignStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub target: Box<Node>,
    pub value: Box<Node>,
}

/// Mirrors Zig `BlockStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockStmt {
    pub body: Vec<Node>,
}

/// Mirrors Zig `ReturnStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Box<Node>>,
}

/// Mirrors Zig `ForStmt`. Ranges are `start..end`; `end` is `None` for
/// the collection form (`for x in coll`), matching the Zig shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub var_name: String,
    pub start: Box<Node>,
    pub end: Option<Box<Node>>,
    pub body: Vec<Node>,
}

/// Mirrors Zig `WhileStmt`.
#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub cond: Box<Node>,
    /// Zig default: null.
    pub let_pattern: Option<EnumPattern>,
    pub body: Vec<Node>,
}

/// Mirrors Zig `BreakStmt` (empty struct).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakStmt;

/// Mirrors Zig `ContinueStmt` (empty struct).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinueStmt;

/// Mirrors Zig `ReleaseStmt` (the `!name` explicit-drop statement).
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseStmt {
    pub var_name: String,
}

/// R-3 goroutine batch: `go call;` spawn statement. The call is
/// kept whole; the checker gates its shape (named non-async void
/// callee, at most one argument, no borrow payloads).
#[derive(Debug, Clone, PartialEq)]
pub struct GoStmt {
    pub call: Box<Node>,
}

/// R-3 goroutine batch: `defer call;` — the target must be a
/// call expression (checked); execution is block-scoped LIFO in
/// the codegens.
#[derive(Debug, Clone, PartialEq)]
pub struct DeferStmt {
    pub call: Box<Node>,
}

/// One arm of a `select` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectArm {
    /// `Some((binding, Option<closed_binding>))` for the
    /// `case let (v, closed) = ch.recv() { ... }` and
    /// `case let v = ch.recv() { ... }` arm forms.
    pub pattern: Option<(String, Option<String>)>,
    /// The receive call for a case arm; `None` for `default`.
    pub recv: Option<Box<Node>>,
    pub body: Vec<Node>,
    pub is_default: bool,
}

/// R-3 goroutine batch: `select { case ... default ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStmt {
    pub arms: Vec<SelectArm>,
}

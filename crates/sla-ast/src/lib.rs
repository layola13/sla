//! sla-ast: AST node definitions mirrored from sa_plugin_sla/src/ast.zig.
//!
//! Crate wiring (this file) is coordinator-owned; module internals are
//! owned by the lexer/ast lane. See docs/COORDINATION.md.

pub mod decls;
pub mod exprs;
pub mod node;
pub mod stmts;
pub mod types;

pub use decls::{
    EnumDecl, EnumVariant, Field, FuncDecl, ImplDecl, ImportDecl, MacroDecl, OverloadDecl,
    Param, Program, StructDecl, TestDecl, TraitDecl, TraitMethod, TypeAliasComponent,
    TypeAliasDecl, UsingDecl,
};
pub use exprs::{
    ArrayLiteral, AwaitExpr, BinaryExpr, BinaryOp, BorrowExpr, CallExpr, Case, CastExpr,
    ClosureLiteral, DerefExpr, EnumLiteral, EnumLiteralField, EnumPattern, FieldExpr,
    GenericFuncRef, IfExpr, IfLetCond, IndexExpr, InlineAsmExpr, InlineAsmOperand, Literal,
    MatchCase, MatchExpr, MoveExpr, RepeatArrayLiteral, SliceExpr, StructLiteral,
    StructLiteralField, SwitchExpr, TryExpr, TupleLiteral, UnsafeExpr,
};
pub use node::Node;
pub use stmts::{
    AssignStmt, BlockStmt, BreakStmt, ConstStmt, ContinueStmt, ForStmt, LetDestructureStmt,
    LetElseStmt, LetStmt, ReleaseStmt, ReturnStmt, VarStmt, WhileStmt,
};
pub use types::{
    ArrayType, ClosureType, FnPtrType, Primitive, TupleType, Type, UserDefined,
};

//! The unified AST node, mirroring the tagged union `Node` at the top of
//! the Zig plugin's `src/ast.zig`.
//!
//! Variant mapping rule: Zig snake_case variant -> Rust CamelCase variant
//! carrying the same-named payload struct (`let_stmt: LetStmt` ->
//! `LetStmt(LetStmt)`). Declaration order matches the Zig union.

use crate::decls::*;
use crate::exprs::*;
use crate::stmts::*;

/// Mirrors Zig `Node`.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Program(Program),

    // Declarations
    StructDecl(StructDecl),
    EnumDecl(EnumDecl),
    TraitDecl(TraitDecl),
    ImplDecl(ImplDecl),
    TypeAliasDecl(TypeAliasDecl),
    OverloadDecl(OverloadDecl),
    FuncDecl(FuncDecl),
    MacroDecl(MacroDecl),
    CheckHookDecl(CheckHookDecl),
    SublanguageDecl(SublanguageDecl),
    ShaderDecl(ShaderDecl),
    ImportDecl(ImportDecl),
    UsingDecl(UsingDecl),
    TestDecl(TestDecl),

    // Statements
    LetStmt(LetStmt),
    LetElseStmt(LetElseStmt),
    LetDestructureStmt(LetDestructureStmt),
    ConstStmt(ConstStmt),
    VarStmt(VarStmt),
    AssignStmt(AssignStmt),
    BlockStmt(BlockStmt),
    /// R-3: `arena { ... }` block statement (Zig `arena_stmt`).
    ArenaStmt(BlockStmt),
    /// Zig: `expr_stmt: *Node`.
    ExprStmt(Box<Node>),
    ReturnStmt(ReturnStmt),
    ForStmt(ForStmt),
    WhileStmt(WhileStmt),
    BreakStmt(BreakStmt),
    ContinueStmt(ContinueStmt),
    ReleaseStmt(ReleaseStmt),
    /// R-3 goroutine batch statements.
    GoStmt(GoStmt),
    DeferStmt(DeferStmt),
    SelectStmt(SelectStmt),

    // Expressions
    Literal(Literal),
    /// Zig: `identifier: []const u8`.
    Identifier(String),
    GenericFuncRef(GenericFuncRef),
    IfExpr(IfExpr),
    SwitchExpr(SwitchExpr),
    MatchExpr(MatchExpr),
    UnsafeExpr(UnsafeExpr),
    AwaitExpr(AwaitExpr),
    InlineAsmExpr(InlineAsmExpr),
    BinaryExpr(BinaryExpr),
    CallExpr(CallExpr),
    ClosureLiteral(ClosureLiteral),
    BorrowExpr(BorrowExpr),
    MoveExpr(MoveExpr),
    DerefExpr(DerefExpr),
    CastExpr(CastExpr),
    FieldExpr(FieldExpr),
    StructLiteral(StructLiteral),
    EnumLiteral(EnumLiteral),
    TupleLiteral(TupleLiteral),
    ArrayLiteral(ArrayLiteral),
    RepeatArrayLiteral(RepeatArrayLiteral),
    IndexExpr(IndexExpr),
    SliceExpr(SliceExpr),
    TryExpr(TryExpr),
}

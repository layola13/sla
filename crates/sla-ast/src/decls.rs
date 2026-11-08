//! Declaration nodes, mirroring the declaration section of the Zig
//! plugin's `src/ast.zig` (plus the shared `Field` / `Param` helpers).
//!
//! Representation mapping (see `docs/divergences/lexer-ast.md` D-A1):
//! Zig `*Node` -> `Box<Node>`, `[]const *Node` -> `Vec<Node>`,
//! `*Type` -> `Box<Type>`, `[]const u8` -> `String`,
//! `[]const []const u8` -> `Vec<String>`, `?T` -> `Option<T>`.
//! Fields that carry a Zig default (`= false`, `= &.{}` ...) are plain
//! fields here; construction sites set them explicitly.

use crate::node::Node;
use crate::types::Type;

/// Mirrors Zig `Program`.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub decls: Vec<Node>,
}

/// Mirrors Zig `StructDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    /// Zig default: empty.
    pub derives: Vec<String>,
    pub generics: Vec<String>,
    pub fields: Vec<Field>,
    /// Zig default: false.
    pub is_union: bool,
    /// Zig default: false.
    pub is_opaque: bool,
}

/// Mirrors Zig `EnumDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<EnumVariant>,
}

/// Mirrors Zig `EnumVariant`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<Field>,
    /// Explicit discriminant from `Variant = N` (C/TS-like). `None` means
    /// auto-increment (first variant 0, then previous + 1). Only allowed
    /// on fieldless variants; see the Zig `enumVariantValue` for resolution.
    pub discriminant: Option<i64>,
}

/// Mirrors Zig `ImplDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImplDecl {
    /// Zig default: null.
    pub trait_name: Option<String>,
    pub target_ty: Box<Type>,
    pub methods: Vec<Node>,
}

/// Mirrors Zig `TypeAliasDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasDecl {
    pub name: String,
    pub components: Vec<TypeAliasComponent>,
}

/// Mirrors Zig `TypeAliasComponent`.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeAliasComponent {
    Ty(Box<Type>),
    InlineStruct(Vec<Field>),
}

/// Mirrors Zig `OverloadDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct OverloadDecl {
    pub target_ty: Box<Type>,
    pub methods: Vec<Node>,
}

/// Mirrors Zig `TraitDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDecl {
    pub name: String,
    /// Zig default: empty.
    pub supertraits: Vec<String>,
    pub methods: Vec<TraitMethod>,
}

/// Mirrors Zig `TraitMethod`.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Box<Type>,
}

/// Mirrors Zig `Field`.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: Box<Type>,
}

/// Mirrors Zig `FuncDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct FuncDecl {
    pub name: String,
    /// Zig default: false.
    pub is_pub: bool,
    /// Zig default: false.
    pub is_extern: bool,
    /// Zig default: null.
    pub abi: Option<String>,
    /// Zig default: false.
    pub no_mangle: bool,
    /// Zig default: false.
    pub is_decl_only: bool,
    pub generics: Vec<String>,
    pub params: Vec<Param>,
    pub ret_ty: Box<Type>,
    pub body: Vec<Node>,
    pub is_inline: bool,
    /// Zig default: false.
    pub is_async: bool,
    /// Zig default: null.
    pub operator: Option<crate::exprs::BinaryOp>,
    /// R-3: `@arena fn` sugar — the whole body is one arena domain.
    /// Zig default: false.
    pub is_arena: bool,
    /// R-3 (A_min producer): the return was written `-> T!`;
    /// `ret_ty` already holds the desugared `Result<T, i32>`.
    /// Zig: `FuncDecl.ret_fallible_sugar`.
    pub ret_fallible_sugar: bool,
    /// R-3 (borrow origins): the name inside a trailing
    /// `-> &[T][name]` return annotation, when present.
    pub ret_borrow_origin: Option<String>,
    /// R-3: extern declaration ends with a C-variadic `...`
    /// marker parameter.
    pub is_variadic: bool,
}

/// Mirrors Zig `Param`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Box<Type>,
    /// Zig default: false.
    pub is_borrow: bool,
    /// Zig default: false.
    pub is_move: bool,
    /// R-3 (A_min scope limit): the parameter type was written
    /// with the `T!` sugar, which is return-position only; the
    /// checker rejects it with FallibleSugarParam.
    pub fallible_sugar: bool,
    /// R-3 (var Phase-2): the parameter was declared `var name: T`
    /// (or the callee is extern-variadic and this param is the
    /// trailing marker handled at FuncDecl level).
    pub is_var: bool,
}

/// Mirrors Zig `MacroDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct MacroDecl {
    pub name: String,
    pub params: Vec<String>,
    /// R-3 (J-3 typed macro params): parallel to `params`; the
    /// declared type of each parameter, or `None` for a bare
    /// (untyped) parameter. Untyped everywhere for pre-J-3 macros.
    pub param_tys: Vec<Option<Type>>,
    pub body: Vec<Node>,
}

/// Mirrors Zig `ImportDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub path: String,
}

/// R-3 (J-5): `@check_hook <name>;` — attaches a registered
/// read-only check hook to the whole file.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckHookDecl {
    pub name: String,
}

/// Mirrors Zig `UsingDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct UsingDecl {
    pub path: String,
}

/// Mirrors Zig `TestDecl`.
#[derive(Debug, Clone, PartialEq)]
pub struct TestDecl {
    pub name: String,
    pub is_ignored: bool,
    pub should_panic: bool,
    pub body: Vec<Node>,
}
/// R-3 (J-1): `@sublanguage <name>;` — registers a declaration
/// sublanguage kind for this file (see SublanguageDecl below).
// R-3 (J-1): shader sublanguage AST shapes (DESIGN.md 6.2 subset).
// A `shader Name { ... }` declaration after `@sublanguage shader;`
// registration. Bodies are kept as raw source text: the shader
// checker (checker-side) lexes and validates them per stage;
// the SLA codegens treat shader declarations as declarations
// with no SLA body (the sublanguage owns its lowering).

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderDecl {
    pub name: String,
    pub priority: Option<i64>,
    pub globals: Vec<ShaderGlobalStruct>,
    pub inputs: Vec<ShaderField>,
    pub params: Vec<ShaderField>,
    pub consts: Vec<ShaderConst>,
    pub vars: Vec<ShaderField>,
    /// (stage name, raw body source) in declaration order;
    /// stage is one of "vertex" / "fragment" / "init".
    pub stages: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderGlobalStruct {
    pub name: String,
    pub fields: Vec<ShaderField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderField {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderConst {
    pub name: String,
    pub ty: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SublanguageDecl {
    pub name: String,
}

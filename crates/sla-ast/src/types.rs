//! The type representation, mirroring the "Type System representation"
//! section of the Zig plugin's `src/ast.zig`.
//!
//! Representation mapping (see `docs/divergences/lexer-ast.md` D-A1):
//! Zig `*Type` -> `Box<Type>`, `[]const *Type` -> `Vec<Type>`,
//! `[]const u8` -> `String`.

/// Mirrors Zig `Type`.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Infer,
    Primitive(Primitive),
    Pointer(Box<Type>),
    Borrow(Box<Type>),
    Array(ArrayType),
    Tuple(TupleType),
    Future(Box<Type>),
    Closure(ClosureType),
    FnPtr(FnPtrType),
    UserDefined(UserDefined),
}

/// Mirrors Zig `Primitive`. Legacy aliases (`integer`, `float`, `boolean`,
/// `raw_ptr`, `void_type`) are kept, as in the original.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    // Integer primitives
    I8,
    I16,
    I32,
    I64,
    Isize,
    U8,
    U16,
    U32,
    U64,
    Usize,
    // Floating-point primitives
    F32,
    F64,
    // Legacy aliases kept during migration.
    Integer,
    Float,
    Boolean,
    RawPtr,
    VoidType,
}

/// Mirrors Zig `UserDefined`.
#[derive(Debug, Clone, PartialEq)]
pub struct UserDefined {
    pub name: String,
    pub generics: Vec<Type>,
}

/// Mirrors Zig `ArrayType`.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayType {
    pub elem: Box<Type>,
    pub len: usize,
}

/// Mirrors Zig `TupleType`.
#[derive(Debug, Clone, PartialEq)]
pub struct TupleType {
    pub elems: Vec<Type>,
}

/// Mirrors Zig `ClosureType`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureType {
    pub params: Vec<Type>,
    pub ret: Box<Type>,
}

/// Mirrors Zig `FnPtrType`. `abi` defaults to `None` in Zig.
#[derive(Debug, Clone, PartialEq)]
pub struct FnPtrType {
    pub abi: Option<String>,
    pub params: Vec<Type>,
    pub ret: Box<Type>,
}

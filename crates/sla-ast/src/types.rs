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

/// Deterministic SA symbol for one member of a plain-function
/// overload set: `sla__{name}__ov_{tag}_{tag}...`, one tag per
/// parameter type. The checker records the winning member's
/// symbol for each call node; the text emitter recomputes the
/// same string for each member's definition, so declarations
/// and call sites agree by construction. (Before this, every
/// member was emitted under the bare `sla__{name}` symbol:
/// duplicate globals for the LLVM route, first-decl binding and
/// callee-contract mismatches for the SA route.)
pub fn overload_member_symbol(name: &str, param_tys: &[Type]) -> String {
    let mut out = format!("sla__{name}__ov");
    for t in param_tys {
        out.push('_');
        out.push_str(&overload_type_tag(t));
    }
    out
}

/// Compact, deterministic type tag for `overload_member_symbol`.
/// Deliberately coarse for composite types (the checker has
/// already proven exactly one winner per call; a tag collision
/// between two members would surface as a loud duplicate-symbol
/// error, never a silent misbind).
pub fn overload_type_tag(ty: &Type) -> String {
    match ty {
        Type::Primitive(p) => match p {
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
            Primitive::Integer => "int",
            Primitive::Float => "float",
            Primitive::Boolean => "bool",
            Primitive::RawPtr => "rawptr",
            Primitive::VoidType => "void",
            Primitive::VaList => "valist",
        }
        .to_string(),
        Type::Pointer(_) => "ptr".to_string(),
        Type::Borrow(_) => "ref".to_string(),
        Type::Array(_) => "array".to_string(),
        Type::Tuple(_) => "tuple".to_string(),
        Type::Future(_) => "future".to_string(),
        Type::Closure(_) => "closure".to_string(),
        Type::FnPtr(_) => "fnptr".to_string(),
        Type::UserDefined(ud) => ud.name.clone(),
        Type::Infer => "infer".to_string(),
    }
}

/// Whether one declared parameter type of an extern overload-set
/// member converts back from its ABI spelling, i.e. whether the
/// checker would record a member symbol for a call to this member
/// (R539 form A). Derived arm-by-arm from the checker's own two
/// functions (`sla-checker/src/check_overload2.rs`):
/// `register_source_extern` renders a source-level extern member's
/// contract spelling as `param_spelling(ty)`, and
/// `extern_member_symbol` converts each spelling back with
/// `extern_spelling_to_type`, skipping the recording when any
/// spelling has no AST counterpart. The composition
/// `extern_spelling_to_type(param_spelling(ty))` succeeds exactly
/// when:
///   * primitives convert by tag name (`isize`/`usize` stay
///     Isize/Usize, `ptr`/`raw_ptr` stay RawPtr); the variadic
///     marker `VaList` is the exception — it renders as `"..."`,
///     which has no counterpart, but the checker never converts
///     it (it adds the marker from `is_variadic`), so it counts
///     as convertible here;
///   * pointers/borrows recurse (`*`/`&` prefixes strip and the
///     inner type must convert);
///   * composites and `infer` render (via `type_desc`) to fixed
///     identifier-safe words (`array`, `tuple`, `future`,
///     `closure`, `fn_ptr`, `infer`), which convert back to a
///     UserDefined placeholder whose tag agrees with the
///     emitter-side tag;
///   * a user type converts back iff its name is a bare
///     identifier — a qualified/generic spelling (e.g. `a::b`,
///     `Vec<i32>`) has no AST counterpart.
/// A member with any unconvertible parameter must be emitted
/// under the bare name (`decl_symbol` in sla-codegen-text keeps
/// it bare): the checker records nothing for it, so calls bind
/// the bare name, and a mangled definition would leave the call
/// with no declaration to bind to.
pub fn extern_spelling_convertible(ty: &Type) -> bool {
    match ty {
        Type::Primitive(_) => true,
        Type::Pointer(inner) | Type::Borrow(inner) => extern_spelling_convertible(inner),
        Type::Array(_)
        | Type::Tuple(_)
        | Type::Future(_)
        | Type::Closure(_)
        | Type::FnPtr(_)
        | Type::Infer => true,
        Type::UserDefined(ud) => {
            !ud.name.is_empty()
                && ud.name.chars().all(|c| c.is_alphanumeric() || c == '_')
        }
    }
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
    /// R-3: C va_list (a named `args: ...` variadic parameter).
    VaList,
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
    /// Extern variadic batch (LINE-CENSUS-VARIADIC): the type was
    /// spelled with a trailing `...` (`extern "C" fn(i32, ...)`);
    /// `params` holds only the fixed prefix. Mirrors Zig
    /// ast.zig `FnPtrType.is_variadic` (default false).
    pub is_variadic: bool,
}

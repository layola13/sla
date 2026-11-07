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

/// Mirrors Zig `normalizeUsingPath`: every `::` pair in a `using`
/// path becomes `__` (byte-wise scan, as in Zig).
pub fn normalize_using_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = String::with_capacity(path.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b':' && bytes[i + 1] == b':' {
            out.push_str("__");
            i += 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// Mirrors lowering_rules `isStringType` for the checker's
/// purposes: the user-defined `String` type.
pub fn is_string_type(ty: &Type) -> bool {
    matches!(ty, Type::UserDefined(u) if u.name == "String")
}

/// Mirrors lowering_rules `isStringLikeType` (venue
/// lowering_rules.zig:5201): after a borrow/pointer peel, a
/// raw_ptr primitive, any array, or the user-defined `String`
/// type. String literals type as raw_ptr in this checker, so
/// they qualify — as do array values (venue probe: `str_eq` on
/// `[u8; 3]` parameters checks rc=0).
pub fn is_string_like_type(ty: &Type) -> bool {
    match peel_borrow_pointer(ty) {
        Type::Primitive(Primitive::RawPtr) => true,
        Type::Array(_) => true,
        Type::UserDefined(u) => u.name == "String",
        _ => false,
    }
}

/// The Zig `@tagName` of an `ast.Type` value, as used by the
/// `using`-ambiguity diagnostic for non-user-defined receivers
/// (venue probe: an `i32` receiver reports type `primitive`).
pub fn type_tag_name(ty: &Type) -> &'static str {
    match ty {
        Type::Infer => "infer",
        Type::Primitive(_) => "primitive",
        Type::Pointer(_) => "pointer",
        Type::Borrow(_) => "borrow",
        Type::Array(_) => "array",
        Type::Tuple(_) => "tuple",
        Type::UserDefined(_) => "user_defined",
        Type::Future(_) => "future",
        Type::Closure(_) => "closure",
        Type::FnPtr(_) => "fn_ptr",
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

/// Mirrors lowering_rules `peelBorrowPointerType`: strip borrow /
/// pointer layers until a non-carrier type remains.
pub fn peel_borrow_pointer(ty: &Type) -> &Type {
    let mut curr = ty;
    loop {
        match curr {
            Type::Borrow(inner) | Type::Pointer(inner) => curr = inner,
            other => return other,
        }
    }
}

/// Mirrors lowering_rules `optionInnerType`: `Option<T>` (after a
/// borrow/pointer peel) yields `T`.
pub fn option_inner_type(ty: &Type) -> Option<Type> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == "Option" && u.generics.len() == 1 => {
            Some(u.generics[0].clone())
        }
        _ => None,
    }
}

/// Mirrors lowering_rules `resultOkType`: `Result<T, E>` yields `T`.
pub fn result_ok_type(ty: &Type) -> Option<Type> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == "Result" && u.generics.len() == 2 => {
            Some(u.generics[0].clone())
        }
        _ => None,
    }
}

/// Mirrors lowering_rules `resultErrType`: `Result<T, E>` yields `E`.
pub fn result_err_type(ty: &Type) -> Option<Type> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == "Result" && u.generics.len() == 2 => {
            Some(u.generics[1].clone())
        }
        _ => None,
    }
}

/// Shared shape of lowering_rules `userDefinedGenericInner`:
/// `Name<T>` (after a peel) yields `T`.
fn user_defined_generic_inner(ty: &Type, name: &str) -> Option<Type> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == name && u.generics.len() == 1 => {
            Some(u.generics[0].clone())
        }
        _ => None,
    }
}

/// Mirrors lowering_rules `sliceElementType` (`Slice<T>` -> `T`).
pub fn slice_element_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Slice")
}

/// Mirrors lowering_rules `vecElementType` (`Vec<T>` -> `T`).
pub fn vec_element_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Vec")
}

/// Mirrors lowering_rules `boxInnerType` (`Box<T>` -> `T`).
pub fn box_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Box")
}

/// Mirrors lowering_rules `rcInnerType` (`Rc<T>` -> `T`).
pub fn rc_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Rc")
}

/// Mirrors lowering_rules `growVecElementType` (`GrowVec<T>` -> `T`).
pub fn grow_vec_element_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "GrowVec")
}

/// Mirrors lowering_rules `vecDequeElementType` (`VecDeque<T>` -> `T`).
pub fn vec_deque_element_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "VecDeque")
}

/// Mirrors lowering_rules `arcInnerType` (`Arc<T>` -> `T`).
pub fn arc_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Arc")
}

/// Mirrors the checker's `cellInnerType` (`Cell<T>` -> `T`).
pub fn cell_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Cell")
}

/// Mirrors the checker's `refCellInnerType` (`RefCell<T>` -> `T`).
pub fn ref_cell_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "RefCell")
}

/// Mirrors the checker's `mutexInnerType` (`Mutex<T>` -> `T`).
pub fn mutex_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Mutex")
}

/// Mirrors the checker's `rwLockInnerType` (`RwLock<T>` -> `T`).
pub fn rw_lock_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "RwLock")
}

/// Mirrors lowering_rules `senderInnerType` (`Sender<T>` -> `T`).
pub fn sender_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Sender")
}

/// Mirrors lowering_rules `receiverInnerType` (`Receiver<T>` -> `T`).
pub fn receiver_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Receiver")
}

/// Mirrors lowering_rules `joinHandleInnerType`
/// (`JoinHandle<T>` -> `T`).
pub fn join_handle_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "JoinHandle")
}

/// Mirrors the checker's `atomicPtrInnerType`
/// (`AtomicPtr<T>` -> `T`).
pub fn atomic_ptr_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "AtomicPtr")
}

/// Two-generic shape shared by the map extractors:
/// `Name<K, V>` (after a peel) yields `(K, V)`.
fn user_defined_pair(ty: &Type, name: &str) -> Option<(Type, Type)> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == name && u.generics.len() == 2 => {
            Some((u.generics[0].clone(), u.generics[1].clone()))
        }
        _ => None,
    }
}

/// Mirrors the checker's `hashMapTypes` (`HashMap<K, V>`).
pub fn hash_map_types(ty: &Type) -> Option<(Type, Type)> {
    user_defined_pair(ty, "HashMap")
}

/// Mirrors the checker's `hashSetTypes` (`HashSet<K>` -> key type).
pub fn hash_set_element_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "HashSet")
}

/// Mirrors the checker's `btreeMapTypes` (`BTreeMap<K, V>`).
pub fn btree_map_types(ty: &Type) -> Option<(Type, Type)> {
    user_defined_pair(ty, "BTreeMap")
}

/// Mirrors the checker's `btreeSetTypes` (`BTreeSet<K>` -> key type).
pub fn btree_set_element_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "BTreeSet")
}

/// Name-only user-defined shape (after a peel), for the nominal
/// std types whose generics do not carry the checked payload
/// (`AtomicI32`, `File`, `Ordering`, ...).
fn is_named_type(ty: &Type, name: &str) -> bool {
    matches!(peel_borrow_pointer(ty), Type::UserDefined(u) if u.name == name)
}

/// Mirrors the checker's `isAtomicI32Type`.
pub fn is_atomic_i32_type(ty: &Type) -> bool {
    is_named_type(ty, "AtomicI32")
}

/// Mirrors the checker's `isAtomicUsizeType`.
pub fn is_atomic_usize_type(ty: &Type) -> bool {
    is_named_type(ty, "AtomicUsize")
}

/// Mirrors lowering_rules `isFileType`.
pub fn is_file_type(ty: &Type) -> bool {
    is_named_type(ty, "File")
}

/// Mirrors lowering_rules `isMetadataType`.
pub fn is_metadata_type(ty: &Type) -> bool {
    is_named_type(ty, "Metadata")
}

/// Mirrors lowering_rules `isOrderingType`.
pub fn is_ordering_type(ty: &Type) -> bool {
    is_named_type(ty, "Ordering")
}

/// Mirrors lowering_rules `vecLikeElementType`: `Vec<T>` or
/// `GrowVec<T>` yields `T`.
pub fn vec_like_element_type(ty: &Type) -> Option<Type> {
    vec_element_type(ty).or_else(|| grow_vec_element_type(ty))
}

/// Mirrors the checker's `iterableElementType`: array element,
/// else Vec-like element, else slice element.
pub fn iterable_element_type(ty: &Type) -> Option<Type> {
    if let Some((elem, _)) = array_type_of(ty) {
        return Some(elem);
    }
    vec_like_element_type(ty).or_else(|| slice_element_type(ty))
}

/// Mirrors lowering_rules `arrayType`: the array shape under any
/// borrow/pointer peel.
pub fn array_type_of(ty: &Type) -> Option<(Type, usize)> {
    match peel_borrow_pointer(ty) {
        Type::Array(a) => Some(((*a.elem).clone(), a.len)),
        _ => None,
    }
}

/// Mirrors lowering_rules `concreteTypeName`: the user-defined name
/// under any borrow/pointer peel.
pub fn concrete_type_name(ty: &Type) -> Option<&str> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) => Some(u.name.as_str()),
        _ => None,
    }
}

/// Mirrors lowering_rules `dynTraitName`: a user type named
/// `__dyn_<Trait>` denotes a dyn-trait carrier; yields the trait.
pub fn dyn_trait_name(ty: &Type) -> Option<&str> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) => u.name.strip_prefix("__dyn_"),
        _ => None,
    }
}

/// Mirrors the checker's `rootIdentifier` (type_checker.zig:1808):
/// the binding an expression is rooted at, peeling index and field
/// projections (NOT borrows/moves). Used by the destructure and
/// await consumption sites, which consume the whole root binding.
pub fn root_identifier_deep(node: &Node) -> Option<&str> {
    match node {
        Node::Identifier(name) => Some(name.as_str()),
        Node::IndexExpr(i) => root_identifier_deep(&i.target),
        Node::FieldExpr(f) => root_identifier_deep(&f.expr),
        _ => None,
    }
}

/// Mirrors lowering_rules `literalZero`.
pub fn literal_zero(node: &Node) -> bool {
    match node {
        Node::Literal(sla_ast::exprs::Literal::IntVal(v)) => *v == 0,
        Node::Literal(sla_ast::exprs::Literal::FloatVal(v)) => *v == 0.0,
        _ => false,
    }
}

/// Mirrors the checker's `enumNameMatchesDecl`: a pattern's enum
/// name matches a declaration name exactly, or as a prefix followed
/// by `_` (e.g. pattern `Message` against decl `Message_Kind`).
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

/// Mirrors lowering_rules `isSwitchDefaultPattern`: the pattern is
/// the bare identifier `default`.
pub fn is_switch_default_pattern(node: &Node) -> bool {
    matches!(node, Node::Identifier(name) if name == "default")
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

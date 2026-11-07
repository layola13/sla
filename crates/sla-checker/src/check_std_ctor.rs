//! Std associated-constructor leaves — the name-keyed
//! static-constructor arms of the Zig associated block
//! (type_checker.zig, venue :8940-9350): `Vec::new`,
//! `HashMap::new`, `Box::new`, `Cell::new` and kin.
//!
//! Until this port (phase-6 census root class R-2/R-1), the Rust
//! checker had NO arm for these: every call fell through to
//! `Undefined call: new`, in whichever function bodies the
//! frontend actually parsed — plain `fn` bodies under `sla check`
//! (R-1) and `@test` bodies under `sla test` (R-2). The arms are
//! wired into `check_associated_call` AFTER the consumption
//! specials / trait / pointer leaves and BEFORE the inherent
//! `{Target}_{name}` lookup, mirroring the Zig block order.
//!
//! Deliberately NOT in this batch (registered in
//! docs/divergences/checker.md): the `future::` / `poll::` /
//! `task::` / `executor::` associated families and
//! `checkArenaMoveArg` escape validation on constructor
//! arguments.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    is_integer_type, is_numeric_type, is_string_like_type, node_key_of,
    type_desc, types_equal,
};
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type, UserDefined};

pub fn user_ty(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

pub fn option_ty(inner: Type) -> Type {
    user_ty("Option", vec![inner])
}

pub fn result_ty(ok: Type, err: Type) -> Type {
    user_ty("Result", vec![ok, err])
}

pub fn string_ty() -> Type {
    user_ty("String", vec![])
}

fn arity_err(name: &str, want: usize, got: usize) -> Diagnostic {
    Diagnostic::new(
        ErrorKind::InvalidArgsCount,
        format!("InvalidArgsCount: `{name}` expects {want} arguments, got {got}"),
    )
}

fn mismatch(msg: String) -> Diagnostic {
    Diagnostic::new(ErrorKind::TypeMismatch, msg)
}

/// Std-owner names for `userDefinedStdOwnerIsNonCopy`
/// (lowering_rules): types that own heap handles and are never
/// shallow-copy values at depth 0.
fn is_std_owner_name(name: &str) -> bool {
    matches!(
        name,
        "Vec"
            | "GrowVec"
            | "VecDeque"
            | "String"
            | "Box"
            | "Rc"
            | "Arc"
            | "HashMap"
            | "BTreeMap"
            | "HashSet"
            | "BTreeSet"
            | "RefCell"
    )
}

impl Checker {
    /// Zig `typeIsShallowCopyCallArgValue` (type_checker.zig)
    /// over the lowering_rules bases: the DEF-5 shallow-copy
    /// classification for call arguments — primitives / pointers
    /// / borrows / fn pointers are copy leaves; tuples and arrays
    /// recurse; std collections and owner types are shallow-copy
    /// only at depth > 0 (nested inside a struct); enums are
    /// shallow-copy; user structs recurse through their fields
    /// when non-opaque and non-union.
    pub fn type_is_shallow_copy_call_arg_value(&self, ty: &Type, depth: usize) -> bool {
        use crate::types_util::*;
        if depth > 8 {
            return false;
        }
        // shallowCopyCallArgValueBase.
        let is_collection = vec_element_type(ty).is_some()
            || grow_vec_element_type(ty).is_some()
            || vec_deque_element_type(ty).is_some()
            || hash_map_types(ty).is_some()
            || btree_map_types(ty).is_some()
            || hash_set_element_type(ty).is_some()
            || btree_set_element_type(ty).is_some();
        if is_collection {
            return depth > 0;
        }
        match ty {
            Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_) | Type::FnPtr(_) => {
                return true;
            }
            Type::Tuple(t) => {
                return t
                    .elems
                    .iter()
                    .all(|e| self.type_is_shallow_copy_call_arg_value(e, depth + 1));
            }
            Type::Array(a) => {
                return self.type_is_shallow_copy_call_arg_value(&a.elem, depth + 1);
            }
            Type::UserDefined(u) => {
                // shallowCopyCallArgUserDefinedBase.
                let smart_pointer = box_inner_type(ty).is_some()
                    || rc_inner_type(ty).is_some()
                    || arc_inner_type(ty).is_some()
                    || ref_cell_inner_type(ty).is_some();
                if is_std_owner_name(&u.name) || smart_pointer {
                    return depth > 0;
                }
                if self.enums.contains_key(&u.name) {
                    return true;
                }
                let Some((_, info)) = self.struct_decl_for_type(ty) else {
                    return false;
                };
                if info.is_opaque || info.is_union {
                    return false;
                }
                return info
                    .fields
                    .iter()
                    .all(|(_, fty)| self.type_is_shallow_copy_call_arg_value(fty, depth + 1));
            }
            _ => return false,
        }
    }

    /// Zig `consumeConstructorArg` (type_checker.zig:519): a
    /// non-Copy identifier argument is consumed by the
    /// constructor; a consumed one is UseAfterMove, an
    /// uninitialized one UseBeforeInit (message kept).
    pub fn consume_std_constructor_arg(
        &mut self,
        arg: &Node,
        arg_ty: &Type,
        operation: &str,
    ) -> CheckResult<()> {
        let Node::Identifier(name) = arg else {
            return Ok(());
        };
        if self.type_is_copy(arg_ty) {
            return Ok(());
        }
        let state = match self.scopes.lookup(name) {
            Some(b) => b.state,
            None => {
                return Err(Diagnostic::new(
                    ErrorKind::UndefinedVariable,
                    format!("UndefinedVariable: `{name}`"),
                ))
            }
        };
        match state {
            crate::rules::ValueState::Consumed => Err(Diagnostic::new(
                ErrorKind::UseAfterMove,
                format!("UseAfterMove: `{name}` was already consumed"),
            )),
            crate::rules::ValueState::Uninitialized => Err(Diagnostic::new(
                ErrorKind::UseBeforeInit,
                format!("UseBeforeInit: var `{name}` is read before assignment"),
            )),
            _ => self.consume_binding(name, operation),
        }
    }

    /// The std constructor leaves. Returns `None` when the
    /// (target, name) pair is not a std leaf, leaving the
    /// inherent lookup to run.
    pub fn check_std_associated_leaf(
        &mut self,
        target: &str,
        call: &CallExpr,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        let argc = call.args.len();
        let generic_or_infer = |idx: usize| -> Type {
            call.generics.get(idx).cloned().unwrap_or(Type::Infer)
        };
        match (target, name) {
            // ManuallyDrop::new: wrap the argument type.
            ("ManuallyDrop", "new") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("ManuallyDrop::new", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                Ok(user_ty("ManuallyDrop", vec![arg_ty]))
            })()),
            // mpsc::channel: the i32-payload legacy form (Zig arm).
            ("mpsc", "channel") => Some((|| {
                if argc != 0 {
                    return Err(arity_err("mpsc::channel", 0, argc));
                }
                let i32t = crate::types_util::i32_ty();
                Ok(Type::Tuple(sla_ast::types::TupleType {
                    elems: vec![
                        user_ty("Sender", vec![i32t.clone()]),
                        user_ty("Receiver", vec![i32t]),
                    ],
                }))
            })()),
            // thread::spawn: a nullary closure yields a JoinHandle
            // of its return type; an integer return refines to i32
            // (Zig arm).
            ("thread", "spawn") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("thread::spawn", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                let Type::Closure(c) = arg_ty else {
                    return Err(mismatch(format!(
                        "TypeMismatch: thread::spawn expects a closure, actual {}",
                        type_desc(&arg_ty)
                    )));
                };
                if !c.params.is_empty() {
                    return Err(arity_err("thread::spawn closure", 0, c.params.len()));
                }
                let ret = (*c.ret).clone();
                let ret = if is_integer_type(&ret) {
                    crate::types_util::i32_ty()
                } else {
                    ret
                };
                Ok(user_ty("JoinHandle", vec![ret]))
            })()),
            // Box::new / Rc::new / Arc::new: consume the argument,
            // wrap its type.
            ("Box", "new") | ("Rc", "new") | ("Arc", "new") => {
                Some((|| {
                    if argc != 1 {
                        return Err(arity_err(name, 1, argc));
                    }
                    let arg_ty = self.check_expr(&call.args[0])?;
                    self.consume_std_constructor_arg(
                        &call.args[0],
                        &arg_ty,
                        "constructor argument",
                    )?;
                    Ok(user_ty(target, vec![arg_ty]))
                })())
            }
            // Box::from_raw: unsafe-gated pointer wrap.
            ("Box", "from_raw") => Some((|| {
                if self.unsafe_depth == 0 {
                    return Err(Diagnostic::new(
                        ErrorKind::CompileError,
                        "Box::from_raw requires an unsafe context".to_string(),
                    ));
                }
                if argc != 1 {
                    return Err(arity_err("Box::from_raw", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                match arg_ty {
                    Type::Pointer(inner) => Ok(user_ty("Box", vec![(*inner).clone()])),
                    Type::Borrow(inner) => Ok(user_ty("Box", vec![(*inner).clone()])),
                    other => Err(mismatch(format!(
                        "TypeMismatch: Box::from_raw expects a pointer, actual {}",
                        type_desc(&other)
                    ))),
                }
            })()),
            // AtomicI32::new / AtomicUsize::new: numeric payload.
            ("AtomicI32", "new") | ("AtomicUsize", "new") => Some((|| {
                if argc != 1 {
                    return Err(arity_err(name, 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                if !is_numeric_type(&arg_ty) {
                    return Err(mismatch(format!(
                        "TypeMismatch: {target}::new expects a numeric argument, actual {}",
                        type_desc(&arg_ty)
                    )));
                }
                Ok(user_ty(target, vec![]))
            })()),
            // AtomicPtr::new: pointer payload.
            ("AtomicPtr", "new") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("AtomicPtr::new", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                match arg_ty {
                    Type::Pointer(inner) => {
                        Ok(user_ty("AtomicPtr", vec![(*inner).clone()]))
                    }
                    other => Err(mismatch(format!(
                        "TypeMismatch: AtomicPtr::new expects a pointer, actual {}",
                        type_desc(&other)
                    ))),
                }
            })()),
            // Cell::new: numeric-or-boolean payloads only
            // (lowering_rules `isCellValueType`).
            ("Cell", "new") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("Cell::new", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                if !is_numeric_type(&arg_ty) && !matches!(arg_ty, Type::Primitive(Primitive::Boolean)) {
                    return Err(mismatch(format!(
                        "TypeMismatch: Cell::new expects a numeric or boolean value, actual {}",
                        type_desc(&arg_ty)
                    )));
                }
                Ok(user_ty("Cell", vec![arg_ty]))
            })()),
            // RefCell::new: any payload.
            ("RefCell", "new") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("RefCell::new", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                Ok(user_ty("RefCell", vec![arg_ty]))
            })()),
            // Mutex::new / RwLock::new: numeric payload; an integer
            // LITERAL argument refines the payload to i32, and the
            // argument node's recorded type is rewritten (Zig arm).
            ("Mutex", "new") | ("RwLock", "new") => Some((|| {
                if argc != 1 {
                    return Err(arity_err(name, 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                if !is_numeric_type(&arg_ty) {
                    return Err(mismatch(format!(
                        "TypeMismatch: {target}::new expects a numeric argument, actual {}",
                        type_desc(&arg_ty)
                    )));
                }
                let payload = if matches!(
                    &call.args[0],
                    Node::Literal(sla_ast::exprs::Literal::IntVal(_))
                ) {
                    let i32t = crate::types_util::i32_ty();
                    self.expr_types
                        .insert(node_key_of(&call.args[0]), i32t.clone());
                    i32t
                } else {
                    arg_ty
                };
                Ok(user_ty(target, vec![payload]))
            })()),
            // File::open: string-like path -> Result<File, i32>.
            ("File", "open") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("File::open", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                if !is_string_like_type(&arg_ty) {
                    return Err(mismatch(format!(
                        "TypeMismatch: File::open expects a string-like path, actual {}",
                        type_desc(&arg_ty)
                    )));
                }
                Ok(result_ty(
                    user_ty("File", vec![]),
                    crate::types_util::i32_ty(),
                ))
            })()),
            // VecDeque::from(array) / VecDeque::new.
            ("VecDeque", "from") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("VecDeque::from", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                match crate::types_util::array_type_of(&arg_ty) {
                    Some((elem, _)) => Ok(user_ty("VecDeque", vec![elem])),
                    None => Err(mismatch(format!(
                        "TypeMismatch: VecDeque::from expects an array, actual {}",
                        type_desc(&arg_ty)
                    ))),
                }
            })()),
            ("VecDeque", "new") => Some((|| {
                if argc != 0 {
                    return Err(arity_err("VecDeque::new", 0, argc));
                }
                let elem = if call.generics.len() == 1 {
                    call.generics[0].clone()
                } else {
                    Type::Infer
                };
                Ok(user_ty("VecDeque", vec![elem]))
            })()),
            // HashMap::new / HashSet::new / BTreeMap::new /
            // BTreeSet::new: generic parameters or Infer.
            ("HashMap", "new") => Some((|| {
                if argc != 0 {
                    return Err(arity_err("HashMap::new", 0, argc));
                }
                Ok(user_ty(
                    "HashMap",
                    vec![generic_or_infer(0), generic_or_infer(1)],
                ))
            })()),
            ("HashSet", "new") => Some((|| {
                if argc != 0 {
                    return Err(arity_err("HashSet::new", 0, argc));
                }
                Ok(user_ty("HashSet", vec![generic_or_infer(0)]))
            })()),
            ("BTreeMap", "new") => Some((|| {
                if argc != 0 {
                    return Err(arity_err("BTreeMap::new", 0, argc));
                }
                Ok(user_ty(
                    "BTreeMap",
                    vec![generic_or_infer(0), generic_or_infer(1)],
                ))
            })()),
            ("BTreeSet", "new") => Some((|| {
                if argc != 0 {
                    return Err(arity_err("BTreeSet::new", 0, argc));
                }
                Ok(user_ty("BTreeSet", vec![generic_or_infer(0)]))
            })()),
            // Vec::new.
            ("Vec", "new") => Some((|| {
                if argc != 0 {
                    return Err(arity_err("Vec::new", 0, argc));
                }
                let elem = if call.generics.len() == 1 {
                    call.generics[0].clone()
                } else {
                    Type::Infer
                };
                Ok(user_ty("Vec", vec![elem]))
            })()),
            // GrowVec::new / GrowVec::with_max(numeric).
            ("GrowVec", "new") => Some((|| {
                if argc != 0 {
                    return Err(arity_err("GrowVec::new", 0, argc));
                }
                let elem = if call.generics.len() == 1 {
                    call.generics[0].clone()
                } else {
                    Type::Infer
                };
                Ok(user_ty("GrowVec", vec![elem]))
            })()),
            ("GrowVec", "with_max") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("GrowVec::with_max", 1, argc));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                if !is_numeric_type(&arg_ty) {
                    return Err(mismatch(format!(
                        "TypeMismatch: GrowVec::with_max expects a numeric argument, actual {}",
                        type_desc(&arg_ty)
                    )));
                }
                let elem = if call.generics.len() == 1 {
                    call.generics[0].clone()
                } else {
                    Type::Infer
                };
                Ok(user_ty("GrowVec", vec![elem]))
            })()),
            // str::from_utf8: Slice<u8> (or a borrow of a u8
            // array) -> Result<String, i32>. Zig places this arm
            // after the macro stages; here it sits with the other
            // associated leaves — it can only fire for the literal
            // `str` target, which no user function or macro can
            // shadow (macros are bare-name).
            ("str", "from_utf8") => Some((|| {
                if argc != 1 {
                    return Err(arity_err("str::from_utf8", 1, argc));
                }
                let bytes_ty = self.check_expr(&call.args[0])?;
                let ok = match crate::types_util::slice_element_type(&bytes_ty) {
                    Some(elem) => types_equal(
                        &elem,
                        &Type::Primitive(Primitive::U8),
                    ),
                    None => {
                        if let Node::BorrowExpr(b) = &call.args[0] {
                            let borrowed_ty = self.check_expr(&b.expr)?;
                            match crate::types_util::array_type_of(&borrowed_ty) {
                                Some((elem, _)) => types_equal(
                                    &elem,
                                    &Type::Primitive(Primitive::U8),
                                ),
                                None => false,
                            }
                        } else {
                            false
                        }
                    }
                };
                if !ok {
                    return Err(mismatch(format!(
                        "TypeMismatch: str::from_utf8 expects a u8 slice, actual {}",
                        type_desc(&bytes_ty)
                    )));
                }
                Ok(result_ty(string_ty(), crate::types_util::i32_ty()))
            })()),
            _ => None,
        }
    }
}

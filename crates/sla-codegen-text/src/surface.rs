//! Standard-surface call emission for the SA text backend:
//! the `EXPAND <MACRO>` arms of codegen.zig's call dispatch
//! (constructors at 17818-17880, method arms around 17918 and
//! 18173-19010), driven by the same rule table the SAB backend
//! loads from sla_std/std_surface.sla_meta (58 rules; this lane
//! hard-codes the tier-1 subset — constructors, container
//! push/insert/contains, len, Cell, Box/Rc/Arc access, Option /
//! Result queries, and the fallible Vec remove). Rules not in
//! this subset (HashMap insert/get templates, Vec pop, iterator
//! combinators) stay on the existing loud-unsupported path and
//! are registered in docs/divergences/lowering-text.md.
//!
//! Operand discipline mirrors the Zig arms: the receiver is
//! generated as a plain expression (identifier receivers yield
//! the binding's own register, which the macros update in
//! place); value operands go through the planned-argument
//! materialization and its cleanup bookkeeping, except that a
//! `consume_value` rule forgets a consumed temp instead of
//! releasing it (the value now lives inside the container).

use sla_ast::{CallExpr, Node, Param, Primitive, Type};

use crate::call::LoweredCallArg;
use crate::codegen::{type_size, Codegen};
use crate::Result;

pub(crate) enum SurfaceCall {
    Value(String),
    Void,
}

fn peel_name(ty: &Type) -> Option<(&str, &[Type])> {
    let mut cur = ty;
    loop {
        match cur {
            Type::Borrow(inner) | Type::Pointer(inner) => cur = inner,
            Type::UserDefined(inner) => return Some((inner.name.as_str(), &inner.generics)),
            _ => return None,
        }
    }
}

fn missing_fact() -> crate::TextError {
    crate::TextError::MissingType {
        context: "std surface call".to_string(),
    }
}

fn slot_size(elem: &Type) -> usize {
    type_size(elem).max(8)
}

/// codegen.zig:6686 `atomicOrderingToken` over
/// lowering_rules.zig:5012: the ordering operand of an atomic
/// call is written in SLA as the identifier `Ordering::SeqCst`
/// (etc.) and lowers to the macro's lowercase token. Any other
/// shape is a loud error, never a literal passthrough (the
/// pre-fix catch-all passed `Ordering::SeqCst` through as an
/// operand verbatim).
fn atomic_ordering_token(arg: &Node) -> Option<&'static str> {
    let name = match arg {
        Node::Identifier(name) => name.as_str(),
        Node::CallExpr(call)
            if call.associated_target.as_deref() == Some("Ordering") =>
        {
            call.func_name.as_str()
        }
        _ => return None,
    };
    match name {
        "Ordering::SeqCst" | "SeqCst" => Some("seq_cst"),
        "Ordering::Acquire" | "Acquire" => Some("acquire"),
        "Ordering::Release" | "Release" => Some("release"),
        "Ordering::Relaxed" | "Relaxed" => Some("relaxed"),
        "Ordering::AcqRel" | "AcqRel" => Some("acq_rel"),
        _ => None,
    }
}

impl<'c> Codegen<'c> {
    /// Pure classification: the receiver's peeled surface type
    /// name + generics, or None when the checker recorded no
    /// usable receiver type. The surface stage must stay
    /// non-committal until a rule actually matches — ordinary
    /// user calls flow through the same dispatch and may carry
    /// no receiver facts at all (the fixture CheckInfo stubs).
    fn classify_receiver(&self, call: &CallExpr) -> Option<(String, Vec<Type>)> {
        let recv_ty = self.expr_type(&call.args[0])?;
        let (name, generics) = peel_name(&recv_ty)?;
        Some((name.to_string(), generics.to_vec()))
    }

    /// lowering_rules.zig:4287 `isNonOwningPointerCarrierCastArg`:
    /// a cast of an identifier to a pointer-carrier type lowers
    /// to the identifier's own register, so releasing the
    /// "temp" would retire the live binding. Atomic arms skip
    /// the receiver release for such an argument.
    fn is_non_owning_pointer_carrier_cast_arg(&self, arg: &Node) -> bool {
        if let Node::CastExpr(cast) = arg {
            if matches!(&*cast.expr, Node::Identifier(_)) {
                return matches!(
                    &*cast.ty,
                    Type::Pointer(_)
                        | Type::Borrow(_)
                        | Type::Primitive(Primitive::RawPtr)
                );
            }
        }
        false
    }

    fn surface_receiver(&mut self, call: &CallExpr) -> Result<(String, String, Vec<Type>)> {
        let (name, generics) = self.classify_receiver(call).ok_or_else(|| missing_fact())?;
        let reg = self.gen_expr(&call.args[0])?;
        Ok((reg, name, generics))
    }

    fn surface_value(&mut self, call: &CallExpr, idx: usize) -> Result<LoweredCallArg> {
        self.gen_planned_call_arg(&call.args[idx], None, false)
    }

    /// Plan a value the surface operation CONSUMES (push / insert
    /// / set / heap-cell new): the argument is planned against a
    /// by-value parameter of its own checker type, so an owned
    /// non-Copy value plans as an ownership transfer (`^reg` +
    /// consume mark). Planning with no parameter left the source
    /// register Active while the checker's cleanup lists already
    /// considered it moved — a MemoryLeak at function exit
    /// (caught against a Vec<String> push of format results).
    fn surface_value_consumed(
        &mut self,
        call: &CallExpr,
        idx: usize,
    ) -> Result<LoweredCallArg> {
        // String literals keep the unplanned path: planning them
        // against a String parameter selects the
        // RawPointerStringLiteral kind (a C-string pointer),
        // which collection inserts do not accept — they take the
        // string value the literal materialises as.
        if matches!(
            &call.args[idx],
            Node::Literal(sla_ast::Literal::StringVal(_))
        ) {
            return self.surface_value(call, idx);
        }
        let param = self.expr_type(&call.args[idx]).map(|ty| Param {
            name: String::new(),
            ty: Box::new(ty),
            is_borrow: false,
            is_move: false,
            fallible_sugar: false,
            is_var: false,
        });
        let mut planned = self.gen_planned_call_arg(&call.args[idx], param.as_ref(), false)?;
        // EXPAND operands do not carry the `^` capability prefix
        // (the host rejects it with CapabilityMismatch): the
        // transfer is expressed by the plain operand plus the
        // post-EXPAND forget in surface_finish_value — the same
        // plain-store + consume-mark shape as aggregate element
        // stores (DEF-ACL).
        if let Some(raw) = planned.reg.strip_prefix('^') {
            let raw = raw.to_string();
            planned.reg = raw.clone();
            planned.consume_reg = Some(raw);
        }
        Ok(planned)
    }

    /// Release/forget bookkeeping for one value operand after
    /// its EXPAND line. `consumed` marks the meta file's
    /// `consume_value` modifier: a temp that fed the container
    /// is forgotten (moved), never released.
    fn surface_finish_value(&mut self, planned: &LoweredCallArg, consumed: bool) -> Result<()> {
        if consumed {
            if planned.release_after_call || planned.consume_reg.is_some() {
                let reg = planned.reg.clone();
                return self.emit_forget_moved_value(&reg);
            }
            return Ok(());
        }
        self.emit_lowered_call_arg_cleanups(std::slice::from_ref(planned), None)
    }

    pub(crate) fn gen_std_surface_call(
        &mut self,
        expr: &Node,
        call: &CallExpr,
    ) -> Option<Result<SurfaceCall>> {
        match self.gen_std_surface_call_inner(expr, call) {
            Ok(Some(v)) => Some(Ok(v)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }

    fn gen_std_surface_call_inner(
        &mut self,
        expr: &Node,
        call: &CallExpr,
    ) -> Result<Option<SurfaceCall>> {
        // ---- associated constructors / associated functions ----
        if let Some(target) = &call.associated_target {
            let name = call.func_name.as_str();
            let simple_new: Option<&str> = match (target.as_str(), name) {
                ("Vec", "new") => Some("VEC_NEW"),
                ("HashMap", "new") => Some("MAP_NEW"),
                ("HashSet", "new") => Some("SET_NEW"),
                ("BTreeMap", "new") => Some("BTREE_MAP_NEW"),
                ("BTreeSet", "new") => Some("BTREE_SET_NEW"),
                ("VecDeque", "new") => Some("VEC_DEQUE_NEW"),
                _ => None,
            };
            if let Some(macro_name) = simple_new {
                if !call.args.is_empty() {
                    return Err(missing_fact());
                }
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND {macro_name} {reg}"));
                return Ok(Some(SurfaceCall::Value(reg)));
            }
            match (target.as_str(), name) {
                ("GrowVec", "new") | ("GrowVec", "with_max") => {
                    let result_ty = self
                        .expr_type(expr)
                        .ok_or(missing_fact())?;
                    let (_, generics) = peel_name(&result_ty)
                        .ok_or(missing_fact())?;
                    let elem = generics
                        .first()
                        .ok_or(missing_fact())?
                        .clone();
                    let max = if name == "with_max" {
                        if call.args.len() != 1 {
                            return Err(missing_fact());
                        }
                        let planned = self.surface_value(call, 0)?;
                        let reg = planned.reg.clone();
                        self.surface_finish_value(&planned, false)?;
                        reg
                    } else {
                        "0".to_string()
                    };
                    let reg = self.new_tmp();
                    self.ins(&format!("EXPAND GROWVEC_NEW {reg}, {}, {max}", slot_size(&elem)));
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
                ("Cell", "new")
                | ("AtomicI32", "new")
                | ("AtomicUsize", "new")
                | ("AtomicPtr", "new") => {
                    // Heap-cell constructors (codegen.zig:17652-
                    // 17698): alloc the cell, then initialize it in
                    // place — the meta file's CELL_NEW_VALUE rule
                    // is the SAB dispatch's form, not this lane's.
                    if call.args.len() != 1 {
                        return Err(missing_fact());
                    }
                    let (size_name, init_macro) = match target.as_str() {
                        "Cell" => ("Cell_SIZE", "CELL_SET"),
                        "AtomicI32" => ("AtomicI32_SIZE", "ATOMIC_I32_INIT"),
                        "AtomicUsize" => ("AtomicUsize_SIZE", "ATOMIC_USIZE_INIT"),
                        _ => ("AtomicPtr_SIZE", "ATOMIC_PTR_INIT"),
                    };
                    let value = self.gen_expr(&call.args[0])?;
                    let reg = self.new_tmp();
                    self.ins(&format!("{reg} = alloc {size_name}"));
                    self.ins(&format!("EXPAND {init_macro} {reg}, {value}"));
                    if sla_lowering::call_arg_needs_release(&call.args[0]) {
                        self.emit_release(&value)?;
                    }
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
                ("Box", "new")
                | ("Box", "into_raw")
                | ("Box", "from_raw")
                | ("Rc", "new")
                | ("Arc", "new")
                | ("RefCell", "new")
                | ("Mutex", "new")
                | ("RwLock", "new") => {
                    if call.args.len() != 1 {
                        return Err(missing_fact());
                    }
                    let macro_name = match (target.as_str(), name) {
                        ("Box", "new") => "BOX_NEW",
                        ("Box", "into_raw") => "BOX_INTO_RAW",
                        ("Box", "from_raw") => "BOX_FROM_RAW",
                        ("Rc", "new") => "RC_NEW",
                        ("Arc", "new") => "ARC_NEW",
                        ("RefCell", "new") => "REFCELL_U64_NEW",
                        ("Mutex", "new") => "MUTEX_NEW_I32",
                        _ => "RWLOCK_NEW_I32",
                    };
                    let consumed = !matches!(
                        (target.as_str(), name),
                        ("Box", "from_raw")
                    );
                    let planned = if consumed {
                        self.surface_value_consumed(call, 0)?
                    } else {
                        self.surface_value(call, 0)?
                    };
                    let value = planned.reg.clone();
                    let reg = self.new_tmp();
                    self.ins(&format!("EXPAND {macro_name} {reg}, {value}"));
                    self.surface_finish_value(&planned, consumed)?;
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
                _ => return Ok(None),
            }
        }
        // ---- `copied` on Option (codegen.zig:14018) ----
        if call.func_name == "copied" && call.args.len() == 1 {
            // Zig: when the argument's type is Option<T>, emit
            // OPTION_COPIED_U64 and release the receiver temp when
            // it needs release; otherwise pass the argument through
            // unchanged. Returning None here would drop the call to
            // the generic catch-all as a bare `EXPAND copied`.
            let arg_ty = self.expr_type(&call.args[0]);
            let is_option = matches!(
                arg_ty.as_ref().and_then(|ty| peel_name(ty)),
                Some((name, generics)) if name == "Option" && generics.len() == 1
            );
            if is_option {
                let recv = self.gen_expr(&call.args[0])?;
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND OPTION_COPIED_U64 {reg}, {recv}"));
                if sla_lowering::call_arg_needs_release(&call.args[0]) {
                    self.emit_release(&recv)?;
                }
                return Ok(Some(SurfaceCall::Value(reg)));
            }
            let reg = self.gen_expr(&call.args[0])?;
            return Ok(Some(SurfaceCall::Value(reg)));
        }
        // ---- `unwrap_or_default` on Option (codegen.zig:14523) ----
        if call.func_name == "unwrap_or_default" && call.args.len() == 1 {
            let arg_ty = self.expr_type(&call.args[0]);
            let is_option = matches!(
                arg_ty.as_ref().and_then(|ty| peel_name(ty)),
                Some((name, generics)) if name == "Option" && generics.len() == 1
            );
            if is_option {
                let recv = self.gen_expr(&call.args[0])?;
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND OPTION_UNWRAP_OR_DEFAULT {reg}, {recv}"));
                if sla_lowering::call_arg_needs_release(&call.args[0]) {
                    self.emit_release(&recv)?;
                }
                return Ok(Some(SurfaceCall::Value(reg)));
            }
        }
        // ---- free `len` over the surface containers ----
        if call.func_name == "len" && call.args.len() == 1 && call.callee.is_none() {
            let macro_name = match self.classify_receiver(call) {
                Some((tname, _)) => match tname.as_str() {
                    "Vec" => "VEC_LEN",
                    "HashMap" => "MAP_LEN",
                    "BTreeMap" => "BTREE_MAP_LEN",
                    "HashSet" => "SET_LEN",
                    "BTreeSet" => "BTREE_SET_LEN",
                    "String" => "STRING_BUF_LEN",
                    "GrowVec" => "GROWVEC_LEN",
                    _ => return Ok(None),
                },
                None => return Ok(None),
            };
            let (recv, _, _) = self.surface_receiver(call)?;
            let reg = self.new_tmp();
            self.ins(&format!("EXPAND {macro_name} {reg}, {recv}"));
            return Ok(Some(SurfaceCall::Value(reg)));
        }
        // ---- receiver methods ----
        if call.args.is_empty() || call.callee.is_some() {
            return Ok(None);
        }
        let (tname, generics) = match self.classify_receiver(call) {
            Some(pair) => pair,
            None => return Ok(None),
        };
        let name = call.func_name.as_str();
        let unary_value: Option<&str> = match (tname.as_str(), name) {
            ("Option", "is_some") => Some("OPTION_IS_SOME"),
            ("Option", "is_none") => Some("OPTION_IS_NONE"),
            ("Option", "unwrap") => Some("OPTION_UNWRAP"),
            ("Result", "is_ok") => Some("RESULT_IS_OK"),
            ("Result", "is_err") => Some("RESULT_IS_ERR"),
            ("Cell", "get") => Some("CELL_GET"),
            ("Box", "get") => Some("BOX_GET_U64"),
            ("Box", "as_ptr") => Some("BOX_AS_PTR"),
            ("Rc", "get") => Some("RC_GET"),
            ("Rc", "as_ptr") => Some("RC_AS_PTR"),
            ("Arc", "get") => Some("ARC_GET"),
            ("Arc", "as_ptr") => Some("ARC_AS_PTR"),
            _ => None,
        };
        if let Some(macro_name) = unary_value {
            if call.args.len() != 1 {
                return Err(missing_fact());
            }
            let (recv, _, _) = self.surface_receiver(call)?;
            let reg = self.new_tmp();
            self.ins(&format!("EXPAND {macro_name} {reg}, {recv}"));
            return Ok(Some(SurfaceCall::Value(reg)));
        }
        match (tname.as_str(), name) {
            ("Option", "unwrap_or") | ("Result", "unwrap_or") => {
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let macro_name = if tname == "Option" {
                    "OPTION_UNWRAP_OR"
                } else {
                    "RESULT_UNWRAP_OR"
                };
                let planned = self.surface_value(call, 1)?;
                let value = planned.reg.clone();
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND {macro_name} {reg}, {recv}, {value}"));
                // The receiver IS retired when it is an owned
                // temp (codegen.zig:20665, isUnwrapOrCall arm):
                // a copied()/call-result Option temp otherwise
                // leaks at function exit.
                if sla_lowering::call_arg_needs_release(&call.args[0]) {
                    self.emit_release(&recv)?;
                }
                // No post-call disposition for the default
                // (Zig DEF-R8-FIX): passing it to EXPAND already
                // retires the register — the macro moves it into
                // the result on the None arm and drops it on the
                // Some arm. Releasing it here freed the live
                // result (host UseAfterMove on the default temp).
                let _ = &planned;
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("Vec", "push") | ("GrowVec", "push") => {
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let elem = generics
                    .first()
                    .ok_or(missing_fact())?
                    .clone();
                let macro_name = if tname == "Vec" {
                    "VEC_PUSH"
                } else {
                    "GROWVEC_PUSH"
                };
                // CLASSB-DEF1: a copy-like element pushed from a
                // live binding stores an independent copy; the
                // source stays usable and owning.
                if let Some(copy_reg) = self.push_arg_copy_materialization(call, &elem)? {
                    self.ins(&format!(
                        "EXPAND {macro_name} {recv}, {copy_reg}, {}",
                        slot_size(&elem)
                    ));
                    // The copy's ownership moves into the vec:
                    // forget it, never release (a release would
                    // free the record the new slot references).
                    self.emit_forget_moved_value(&copy_reg)?;
                    return Ok(Some(SurfaceCall::Void));
                }
                let planned = self.surface_value_consumed(call, 1)?;
                let value = planned.reg.clone();
                self.ins(&format!(
                    "EXPAND {macro_name} {recv}, {value}, {}",
                    slot_size(&elem)
                ));
                self.surface_finish_value(&planned, true)?;
                Ok(Some(SurfaceCall::Void))
            }
            ("HashSet", "insert")
            | ("BTreeSet", "insert")
            | ("HashSet", "contains")
            | ("BTreeSet", "contains") => {
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let consumed = name == "insert";
                // HashSet<String> membership keys on string
                // CONTENT (Zig bridge set_insert / set_contains,
                // codegen.zig:13912): the identity SET_* macros
                // never match a fresh equal-content string.
                let elem_ty = generics.first().cloned();
                let str_elem = tname == "HashSet"
                    && elem_ty
                        .as_ref()
                        .map(|t| crate::map::is_string_key_type(t))
                        .unwrap_or(false);
                if str_elem {
                    let lit = self.gen_hashmap_literal_key_reg(
                        &call.args[1],
                        elem_ty.as_ref(),
                    )?;
                    let (value_reg, planned, owned_literal) = match lit {
                        Some(kr) => (kr.reg, None, kr.owned_literal_string),
                        None => {
                            let planned = if consumed {
                                self.surface_value_consumed(call, 1)?
                            } else {
                                self.surface_value(call, 1)?
                            };
                            let reg = planned.reg.clone();
                            (reg, Some(planned), false)
                        }
                    };
                    let func_name = if consumed {
                        "@sa_set_insert_str"
                    } else {
                        "@sa_set_contains_str"
                    };
                    let reg = self.new_tmp();
                    self.ins(&format!(
                        "{reg} = call {func_name}(&{recv}, &{value_reg})"
                    ));
                    if let Some(planned) = &planned {
                        self.surface_finish_value(planned, consumed)?;
                    }
                    if owned_literal {
                        if consumed {
                            self.emit_forget_moved_value(&value_reg)?;
                        } else {
                            self.emit_release(&value_reg)?;
                        }
                    }
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
                let macro_name = match (tname.as_str(), name) {
                    ("HashSet", "insert") => "SET_INSERT",
                    ("HashSet", _) => "SET_CONTAINS",
                    ("BTreeSet", "insert") => "BTREE_SET_INSERT",
                    _ => "BTREE_SET_CONTAINS",
                };
                let planned = if consumed {
                    self.surface_value_consumed(call, 1)?
                } else {
                    self.surface_value(call, 1)?
                };
                let value = planned.reg.clone();
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND {macro_name} {reg}, {recv}, {value}"));
                self.surface_finish_value(&planned, consumed)?;
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("HashMap", "insert") | ("BTreeMap", "insert") => {
                if call.args.len() != 3 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let is_btree = tname == "BTreeMap";
                // Bridge classification (Zig `mapMacroBridge` +
                // the method arms at codegen.zig:21362): the
                // STR/identity routing keys on the RECEIVER's key
                // type, never the argument's — a string literal
                // argument is a Slice on its own and would always
                // misroute to the identity macros.
                let key_ty = generics.first().cloned();
                let lit_key = self.gen_hashmap_literal_key_reg(
                    &call.args[1],
                    if is_btree { None } else { key_ty.as_ref() },
                )?;
                let (key_reg, key_planned, key_owned_literal) = match lit_key {
                    Some(kr) => (kr.reg, None, kr.owned_literal_string),
                    None => {
                        let planned = self.surface_value_consumed(call, 1)?;
                        let reg = planned.reg.clone();
                        (reg, Some(planned), false)
                    }
                };
                if is_btree {
                    // BTreeMap keeps the plain value register
                    // (its nodes store the word; no heap cell —
                    // codegen.zig:21453).
                    let value = self.surface_value_consumed(call, 2)?;
                    let value_reg = value.reg.clone();
                    let reg = self.new_tmp();
                    self.ins(&format!(
                        "EXPAND SLA_BTREE_MAP_INSERT_OPTION_U64 {reg}, {recv}, {key_reg}, {value_reg}"
                    ));
                    if let Some(kp) = &key_planned {
                        self.surface_finish_value(kp, true)?;
                    }
                    self.surface_finish_value(&value, true)?;
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
                // DEF-HASHMAP-XTALK A-form: the value is staged
                // into a per-entry heap cell and the cell is
                // forgotten into the map after the expansion.
                let value = self.surface_value_consumed(call, 2)?;
                let value_reg = value.reg.clone();
                let cell = self.gen_hashmap_value_slot(&value_reg);
                self.surface_finish_value(&value, true)?;
                let str_key = key_ty
                    .as_ref()
                    .map(|t| crate::map::is_string_key_type(t))
                    .unwrap_or(false);
                let macro_name = if str_key {
                    "SLA_MAP_INSERT_STR_OPTION_U64"
                } else {
                    "SLA_MAP_INSERT_OPTION_U64"
                };
                let reg = self.new_tmp();
                self.ins(&format!(
                    "EXPAND {macro_name} {reg}, {recv}, {key_reg}, {cell}"
                ));
                self.emit_forget_moved_value(&cell)?;
                if let Some(kp) = &key_planned {
                    self.surface_finish_value(kp, true)?;
                }
                if key_owned_literal {
                    // The map adopts the owned literal key
                    // (codegen.zig:21377).
                    self.emit_forget_moved_value(&key_reg)?;
                }
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("HashMap", "clear") | ("BTreeMap", "clear") => {
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                if tname == "HashMap" {
                    // A-form ownership (codegen.zig:21440):
                    // return every entry's heap value cell
                    // before the std clear zeroes the slots.
                    self.emit_hashmap_value_cells_free(&recv)?;
                    self.ins(&format!("EXPAND MAP_CLEAR {recv}"));
                } else {
                    self.ins(&format!("EXPAND BTREE_MAP_CLEAR {recv}"));
                }
                Ok(Some(SurfaceCall::Void))
            }
            ("HashMap", "get") | ("BTreeMap", "get") => {
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let is_btree = tname == "BTreeMap";
                let key_ty = generics.first().cloned();
                let lit_key = self.gen_hashmap_literal_key_reg(
                    &call.args[1],
                    if is_btree { None } else { key_ty.as_ref() },
                )?;
                let (key_reg, key_planned, key_owned_literal) = match lit_key {
                    Some(kr) => (kr.reg, None, kr.owned_literal_string),
                    None => {
                        let planned = self.surface_value(call, 1)?;
                        let reg = planned.reg.clone();
                        (reg, Some(planned), false)
                    }
                };
                let str_key = !is_btree
                    && key_ty
                        .as_ref()
                        .map(|t| crate::map::is_string_key_type(t))
                        .unwrap_or(false);
                let macro_name = match (tname.as_str(), str_key) {
                    ("HashMap", true) => "SLA_MAP_TRY_GET_STR_OPTION",
                    ("HashMap", false) => "SLA_MAP_TRY_GET_OPTION",
                    _ => "SLA_BTREE_MAP_TRY_GET_OPTION",
                };
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND {macro_name} {reg}, {recv}, {key_reg}"));
                if let Some(kp) = &key_planned {
                    self.surface_finish_value(kp, false)?;
                }
                if key_owned_literal {
                    // A probe does not adopt its key
                    // (codegen.zig:21394).
                    self.emit_release(&key_reg)?;
                }
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("Cell", "set") => {
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let planned = self.surface_value_consumed(call, 1)?;
                let value = planned.reg.clone();
                self.ins(&format!("EXPAND CELL_SET {recv}, {value}"));
                self.surface_finish_value(&planned, true)?;
                Ok(Some(SurfaceCall::Void))
            }
            ("Vec", "remove") => {
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let elem = generics
                    .first()
                    .ok_or(missing_fact())?
                    .clone();
                let index = self.gen_expr(&call.args[1])?;
                let ok_reg = self.new_tmp();
                let raw_reg = self.new_tmp();
                let hit_label = self.new_label("L_VEC_REMOVE_OK");
                let miss_label = self.new_label("L_VEC_REMOVE_OOB");
                self.ins(&format!(
                    "EXPAND VEC_REMOVE {ok_reg}, {raw_reg}, {recv}, {index}, {}",
                    slot_size(&elem)
                ));
                self.ins(&format!("br {ok_reg} -> {hit_label}, {miss_label}"));
                self.ins(&format!("{miss_label}:"));
                self.ins("panic(86)");
                self.ins(&format!("{hit_label}:"));
                self.ins(&format!("!{ok_reg}"));
                if crate::codegen::type_string(&elem) == "u64" {
                    Ok(Some(SurfaceCall::Value(raw_reg)))
                } else {
                    let reg = self.new_tmp();
                    let elem_s = crate::codegen::type_string(&elem);
                    self.ins(&format!("{reg} = {raw_reg} as {elem_s}"));
                    Ok(Some(SurfaceCall::Value(reg)))
                }
            }
            // ---- stage 9 macro family (RUST-S7-EXPAND-MACRO-FAMILY,
            // R122 B_mirror_zig): the receiver methods below used to
            // fall through to the bare-name EXPAND catch-all in
            // call.rs, synthesising `EXPAND clone/strong_count/
            // fetch_add/lock/join/RESULT_UNWRAP` for macros that
            // exist in no std tree. Each arm mirrors the Zig TEXT
            // emission verbatim (codegen.zig line refs inline).
            ("Rc", "clone") | ("Arc", "clone") => {
                // codegen.zig:21657-21686: copy the handle, then
                // the in-place count bump (`RC_CLONE %rc_reg`,
                // rc.sa:20 — single operand, no output register).
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let reg = self.new_tmp();
                self.ins(&format!("{reg} = add {recv}, 0"));
                let macro_name = if tname == "Rc" { "RC_CLONE" } else { "ARC_CLONE" };
                self.ins(&format!("EXPAND {macro_name} {reg}"));
                // TEXT35-OWNERSHIP (DEF-TEXT-EXPAND-RECV-TEMP):
                // retire a temp receiver; a live binding stays.
                if sla_lowering::call_arg_needs_release(&call.args[0]) {
                    self.emit_release(&recv)?;
                }
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("Rc", "strong_count")
            | ("Rc", "weak_count")
            | ("Arc", "strong_count")
            | ("Arc", "weak_count") => {
                // codegen.zig:21687-21726 (DEF-ARC-DROP F1): copy
                // the receiver to a fresh register first — the
                // count macros key a temporary on the receiver
                // name, and a fresh name keeps repeated queries
                // on one receiver collision-free.
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let src_reg = self.new_tmp();
                let reg = self.new_tmp();
                self.ins(&format!("{src_reg} = add {recv}, 0"));
                let macro_name = match (tname.as_str(), name) {
                    ("Rc", "strong_count") => "RC_STRONG_COUNT",
                    ("Rc", "weak_count") => "RC_WEAK_COUNT",
                    ("Arc", "strong_count") => "ARC_STRONG_COUNT",
                    _ => "ARC_WEAK_COUNT",
                };
                self.ins(&format!("EXPAND {macro_name} {reg}, {src_reg}"));
                self.emit_release(&src_reg)?;
                if sla_lowering::call_arg_needs_release(&call.args[0]) {
                    self.emit_release(&recv)?;
                }
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("AtomicI32", "load") | ("AtomicUsize", "load") | ("AtomicPtr", "load") => {
                // codegen.zig:21406-21417 / 21468-21479 / 21504-21515.
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let ordering = atomic_ordering_token(&call.args[1]).ok_or_else(|| {
                    crate::TextError::Unsupported {
                        feature: "atomic load ordering (non-Ordering::* variant)"
                            .to_string(),
                        zig_ref: "codegen.zig:6686 (atomicOrderingToken)",
                    }
                })?;
                let (recv, _, _) = self.surface_receiver(call)?;
                let macro_name = match tname.as_str() {
                    "AtomicI32" => "ATOMIC_I32_LOAD",
                    "AtomicUsize" => "ATOMIC_USIZE_LOAD",
                    _ => "ATOMIC_PTR_LOAD",
                };
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND {macro_name} {reg}, {recv}, {ordering}"));
                if sla_lowering::call_arg_needs_release(&call.args[0])
                    && !self.is_non_owning_pointer_carrier_cast_arg(&call.args[0])
                {
                    self.emit_release(&recv)?;
                }
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("AtomicI32", "fetch_add") | ("AtomicUsize", "fetch_add") => {
                // codegen.zig:21426-21444 / 21488-21500.
                if call.args.len() != 3 {
                    return Err(missing_fact());
                }
                let ordering = atomic_ordering_token(&call.args[2]).ok_or_else(|| {
                    crate::TextError::Unsupported {
                        feature: "atomic fetch_add ordering (non-Ordering::* variant)"
                            .to_string(),
                        zig_ref: "codegen.zig:6686 (atomicOrderingToken)",
                    }
                })?;
                let (recv, _, _) = self.surface_receiver(call)?;
                let value_reg = self.gen_expr(&call.args[1])?;
                let macro_name = if tname == "AtomicI32" {
                    "ATOMIC_I32_FETCH_ADD"
                } else {
                    "ATOMIC_USIZE_FETCH_ADD"
                };
                let reg = self.new_tmp();
                self.ins(&format!(
                    "EXPAND {macro_name} {reg}, {recv}, {value_reg}, {ordering}"
                ));
                if sla_lowering::call_arg_needs_release(&call.args[0])
                    && !self.is_non_owning_pointer_carrier_cast_arg(&call.args[0])
                {
                    self.emit_release(&recv)?;
                }
                if sla_lowering::call_arg_needs_release(&call.args[1]) {
                    self.emit_release(&value_reg)?;
                }
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("Mutex", "lock") => {
                // codegen.zig:21569-21590: the lock itself is a
                // statement (`MUTEX_LOCK %lock_ptr`, mutex.sa:56
                // — a spin lock, no output); the guard is the
                // `Mutex_data` pointer wrapped in a fresh Ok
                // Result. The Result register is recorded in
                // `mutex_lock_results` so `unwrap` can transfer
                // it to the guard map and `emitRelease` can
                // unlock it (codegen.zig:1186).
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let guard_reg = self.new_tmp();
                let result_reg = self.new_tmp();
                self.ins(&format!("EXPAND MUTEX_LOCK {recv}"));
                self.ins(&format!("{guard_reg} = ptr_add {recv}, Mutex_data"));
                self.ins(&format!(
                    "EXPAND RESULT_NEW_OK {result_reg}, {guard_reg}"
                ));
                self.mutex_lock_results
                    .insert(result_reg.clone(), recv.clone());
                self.emit_release(&guard_reg)?;
                if let Node::Identifier(name) = &call.args[0] {
                    let resolved = self.resolve_binding_name(name);
                    self.consumed_bindings.insert(resolved);
                }
                Ok(Some(SurfaceCall::Value(result_reg)))
            }
            ("JoinHandle", "join") => {
                // codegen.zig:21030-21180: inline handles read the
                // slot directly; the general path branches on the
                // handle word (-1 = inline, else pthread) and
                // builds the 24-byte Result in both arms.
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let inner_ty = generics
                    .first()
                    .ok_or(missing_fact())?
                    .clone();
                let inner_s = crate::codegen::type_string(&inner_ty);
                let (recv, _, _) = self.surface_receiver(call)?;
                let result_reg = self.new_tmp();
                if self.inline_thread_handles.contains(&recv) {
                    let value = self.new_tmp();
                    let ok_tag = self.new_tmp();
                    self.ins(&format!("{value} = load {recv}+8 as {inner_s}"));
                    self.ins(&format!("{result_reg} = alloc 24"));
                    self.ins(&format!("{ok_tag} = 0"));
                    self.ins(&format!("store {result_reg}+0, {ok_tag} as u64"));
                    self.ins(&format!(
                        "store {result_reg}+8, {value} as {inner_s}"
                    ));
                    self.ins(&format!("store {result_reg}+16, {ok_tag} as u64"));
                    self.ins(&format!("!{ok_tag}"));
                    self.ins(&format!("!{value}"));
                    self.inline_thread_handles.remove(&recv);
                    self.emit_release(&recv)?;
                    return Ok(Some(SurfaceCall::Value(result_reg)));
                }
                let handle = self.new_tmp();
                let is_inline = self.new_tmp();
                let status = self.new_tmp();
                let is_ok = self.new_tmp();
                let inline_label = self.new_label("L_THREAD_JOIN_INLINE");
                let pthread_label = self.new_label("L_THREAD_JOIN_PTHREAD");
                let ok_label = self.new_label("L_THREAD_JOIN_OK");
                let err_label = self.new_label("L_THREAD_JOIN_ERR");
                let end_label = self.new_label("L_THREAD_JOIN_END");
                self.ins(&format!("{handle} = load {recv}+0 as i32"));
                self.ins(&format!("{is_inline} = eq {handle}, -1"));
                self.ins(&format!(
                    "br {is_inline} -> {inline_label}, {pthread_label}"
                ));
                self.wl(&format!("{inline_label}:"));
                self.ins(&format!("!{is_inline}"));
                let inline_value = self.new_tmp();
                self.ins(&format!(
                    "{inline_value} = load {recv}+8 as {inner_s}"
                ));
                let inline_ok_tag = self.new_tmp();
                self.ins(&format!("{result_reg} = alloc 24"));
                self.ins(&format!("{inline_ok_tag} = 0"));
                self.ins(&format!(
                    "store {result_reg}+0, {inline_ok_tag} as u64"
                ));
                self.ins(&format!(
                    "store {result_reg}+8, {inline_value} as {inner_s}"
                ));
                self.ins(&format!(
                    "store {result_reg}+16, {inline_ok_tag} as u64"
                ));
                self.ins(&format!("!{inline_ok_tag}"));
                self.ins(&format!("!{inline_value}"));
                self.ins(&format!("jmp {end_label}"));
                self.wl(&format!("{pthread_label}:"));
                self.ins(&format!("!{is_inline}"));
                self.ins(&format!(
                    "EXPAND THREAD_JOIN_STATUS {status}, {handle}, *{recv}"
                ));
                self.ins(&format!("EXPAND THREAD_DROP {handle}"));
                self.ins(&format!("{is_ok} = eq {status}, 0"));
                self.ins(&format!("br {is_ok} -> {ok_label}, {err_label}"));
                self.wl(&format!("{ok_label}:"));
                self.ins(&format!("!{is_ok}"));
                let value = self.new_tmp();
                self.ins(&format!("{value} = load {recv}+8 as {inner_s}"));
                let ok_tag = self.new_tmp();
                self.ins(&format!("{result_reg} = alloc 24"));
                self.ins(&format!("{ok_tag} = 0"));
                self.ins(&format!("store {result_reg}+0, {ok_tag} as u64"));
                self.ins(&format!(
                    "store {result_reg}+8, {value} as {inner_s}"
                ));
                self.ins(&format!("store {result_reg}+16, {ok_tag} as u64"));
                self.ins(&format!("!{ok_tag}"));
                self.ins(&format!("!{value}"));
                self.ins(&format!("!{status}"));
                self.ins(&format!("jmp {end_label}"));
                self.wl(&format!("{err_label}:"));
                self.ins(&format!("!{is_ok}"));
                let err_value = self.new_tmp();
                self.ins(&format!("{err_value} = add {status}, 0"));
                let err_tag = self.new_tmp();
                let err_zero = self.new_tmp();
                self.ins(&format!("{result_reg} = alloc 24"));
                self.ins(&format!("{err_tag} = 1"));
                self.ins(&format!("{err_zero} = 0"));
                self.ins(&format!("store {result_reg}+0, {err_tag} as u64"));
                self.ins(&format!(
                    "store {result_reg}+8, {err_zero} as {inner_s}"
                ));
                self.ins(&format!(
                    "store {result_reg}+16, {err_value} as i64"
                ));
                self.ins(&format!("!{err_zero}"));
                self.ins(&format!("!{err_tag}"));
                self.ins(&format!("!{err_value}"));
                self.ins(&format!("!{status}"));
                self.ins(&format!("jmp {end_label}"));
                self.wl(&format!("{end_label}:"));
                self.ins(&format!("!{handle}"));
                // DEF-ARC-DROP: the receiver register itself must
                // be released here (the inline branch above does);
                // marking it consumed in bookkeeping alone leaves
                // it Active in the emitted SA and traps
                // MemoryLeak at function exit.
                self.emit_release(&recv)?;
                Ok(Some(SurfaceCall::Value(result_reg)))
            }
            ("Result", "unwrap") => {
                // codegen.zig:21340-21380: inline branch code —
                // `RESULT_UNWRAP` has no [MACRO] definition in
                // any std tree (result.sa defines only
                // RESULT_UNWRAP_OR*), so the old unary mapping
                // was a bogus EXPAND. Ok loads the payload at
                // +8; Err panics (17) after defining the result
                // register on that path too.
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let ok_ty = generics
                    .first()
                    .ok_or(missing_fact())?
                    .clone();
                let ok_s = crate::codegen::type_string(&ok_ty);
                let (recv, _, _) = self.surface_receiver(call)?;
                let reg = self.new_tmp();
                let tag = self.new_tmp();
                let is_ok = self.new_tmp();
                let ok_label = self.new_label("L_RESULT_UNWRAP_OK");
                let err_label = self.new_label("L_RESULT_UNWRAP_ERR");
                let end_label = self.new_label("L_RESULT_UNWRAP_END");
                self.ins(&format!("{tag} = load {recv}+0 as u64"));
                self.ins(&format!("{is_ok} = eq {tag}, 0"));
                self.ins(&format!("br {is_ok} -> {ok_label}, {err_label}"));
                self.wl(&format!("{ok_label}:"));
                self.ins(&format!("!{is_ok}"));
                self.ins(&format!("{reg} = load {recv}+8 as {ok_s}"));
                self.ins(&format!("!{tag}"));
                self.ins(&format!("jmp {end_label}"));
                self.wl(&format!("{err_label}:"));
                self.ins(&format!("!{is_ok}"));
                self.ins("panic(17)");
                self.ins(&format!("{reg} = 0"));
                self.ins(&format!("!{tag}"));
                self.ins(&format!("jmp {end_label}"));
                self.wl(&format!("{end_label}:"));
                // A `Mutex::lock` Result unwrapped here hands its
                // guard handle to the unwrapped value
                // (codegen.zig:21373).
                if let Some(mutex_reg) = self.mutex_lock_results.remove(&recv) {
                    self.mutex_guard_handles.insert(reg.clone(), mutex_reg);
                }
                if sla_lowering::call_arg_needs_release(&call.args[0]) {
                    self.emit_release(&recv)?;
                }
                Ok(Some(SurfaceCall::Value(reg)))
            }
            _ => Ok(None),
        }
    }
}

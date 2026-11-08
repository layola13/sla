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

use sla_ast::{CallExpr, Node, Type};

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

    fn surface_receiver(&mut self, call: &CallExpr) -> Result<(String, String, Vec<Type>)> {
        let (name, generics) = self.classify_receiver(call).ok_or_else(|| missing_fact())?;
        let reg = self.gen_expr(&call.args[0])?;
        Ok((reg, name, generics))
    }

    fn surface_value(&mut self, call: &CallExpr, idx: usize) -> Result<LoweredCallArg> {
        self.gen_planned_call_arg(&call.args[idx], None, false)
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
                    let planned = self.surface_value(call, 0)?;
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
            ("Result", "unwrap") => Some("RESULT_UNWRAP"),
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
                self.surface_finish_value(&planned, false)?;
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
                let planned = self.surface_value(call, 1)?;
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
                let macro_name = match (tname.as_str(), name) {
                    ("HashSet", "insert") => "SET_INSERT",
                    ("HashSet", _) => "SET_CONTAINS",
                    ("BTreeSet", "insert") => "BTREE_SET_INSERT",
                    _ => "BTREE_SET_CONTAINS",
                };
                let consumed = name == "insert";
                let planned = self.surface_value(call, 1)?;
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
                let key = self.surface_value(call, 1)?;
                let key_reg = key.reg.clone();
                let value = self.surface_value(call, 2)?;
                let value_reg = value.reg.clone();
                let key_ty = self.expr_type(&call.args[1]);
                let str_key =
                    matches!(key_ty, Some(Type::UserDefined(u)) if u.name == "String");
                let macro_name = match (tname.as_str(), str_key) {
                    ("HashMap", true) => "SLA_MAP_INSERT_STR_OPTION_U64",
                    ("HashMap", false) => "SLA_MAP_INSERT_OPTION_U64",
                    _ => "SLA_BTREE_MAP_INSERT_OPTION_U64",
                };
                let reg = self.new_tmp();
                self.ins(&format!(
                    "EXPAND {macro_name} {reg}, {recv}, {key_reg}, {value_reg}"
                ));
                self.surface_finish_value(&key, true)?;
                self.surface_finish_value(&value, true)?;
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("HashMap", "get") | ("BTreeMap", "get") => {
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let key = self.surface_value(call, 1)?;
                let key_reg = key.reg.clone();
                let key_ty = self.expr_type(&call.args[1]);
                let str_key =
                    matches!(key_ty, Some(Type::UserDefined(u)) if u.name == "String");
                let macro_name = match (tname.as_str(), str_key) {
                    ("HashMap", true) => "SLA_MAP_TRY_GET_STR_OPTION",
                    ("HashMap", false) => "SLA_MAP_TRY_GET_OPTION",
                    _ => "SLA_BTREE_MAP_TRY_GET_OPTION",
                };
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND {macro_name} {reg}, {recv}, {key_reg}"));
                self.surface_finish_value(&key, false)?;
                Ok(Some(SurfaceCall::Value(reg)))
            }
            ("Cell", "set") => {
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let planned = self.surface_value(call, 1)?;
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
            _ => Ok(None),
        }
    }
}

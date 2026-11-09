//! D surface (surface_call) — the associated-call probes of
//! Zig `genCallInner` (sab_codegen.zig, current head), ported
//! for the S1-ctor slice: `genVecNewCall` (15747),
//! `genGrowVecNewCall` (15760), `genVecDequeFromCall` (17615),
//! `genPtrBuiltinCall` (associated-target forms only),
//! `genMutexNewCall` (17306), `genAtomicNewCall` (17332), and
//! the associated-target branch of `genStdSurfaceCall` (21345),
//! over the D1/D4 std-surface machinery (rules loaded at driver
//! step 2; emission through the F fragment machine).
//!
//! Probe order mirrors Zig's dispatch chain restricted to the
//! ported probes; every probe self-guards on
//! `associated_target`, so wiring the chain at `gen_call`'s
//! associated-target site is behaviour-identical for bare
//! calls. Probes owned by other groups — thread spawn and the
//! future/task family, `mpsc::channel`, the Option/Result
//! closure families, method-form surface rules (bare calls
//! with a receiver argument) — are NOT ported here; those
//! calls keep hitting the loud stop in `gen_call`, never a
//! silent reroute.

use sla_ast::{CallExpr, Node, Type};

use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::fragment::FragmentMachine;
use crate::fragment_imports::{MachineStdDeps, MachineSurfaceEmitter};
use crate::layout::{
    element_slot_size, type_base_name, user_defined_generic_inner, vec_element_slot_size,
    vec_element_type, LayoutAdapter,
};
use crate::surface_query::{
    emit_std_surface_rule, find_std_surface_rule, PendingStdDeps, StdDepEnsurer, StdSurfaceValues,
};
use crate::types::{StdSurfaceArgKind, StdSurfaceRule, StdSurfaceRuleKind};

impl<'c> Driver<'c> {
    /// The ported probe chain for `Type::name(args)` calls, in
    /// Zig `genCallInner` order. `Ok(None)` = no ported probe
    /// claimed the call (the caller keeps the loud stop).
    pub(crate) fn gen_associated_call(
        &mut self,
        expr: &Node,
        call: &CallExpr,
    ) -> Result<Option<u32>> {
        if let Some(reg) = self.gen_vec_new_call(expr, call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_grow_vec_new_call(expr, call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_vec_deque_from_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_ptr_builtin_call(expr, call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_mutex_new_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_atomic_new_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_std_surface_call(expr, call)? {
            return Ok(Some(reg));
        }
        Ok(None)
    }

    // -- shared emission helpers --------------------------------

    /// Zig `ensureStdDeps` through the F machine (both
    /// branches, as in `gen_print`'s dep wiring).
    fn ensure_surface_std_deps(&mut self, import_path: &str, deps: &[&str]) -> Result<()> {
        let owned: Vec<String> = deps.iter().map(|dep| dep.to_string()).collect();
        let mut ensurer = MachineStdDeps {
            machine: FragmentMachine::new(&mut self.state),
            flattener: &mut *self.flattener,
        };
        ensurer.ensure_std_deps(import_path, &owned)
    }

    /// Zig `emitCallBody` over the fragment machine.
    fn emit_surface_call_body(&mut self, dst: Option<u32>, body: String) -> Result<()> {
        FragmentMachine::new(&mut self.state).emit_call_body(dst, body)
    }

    /// Zig `emitStdMacroFragment` (plain form: no literal
    /// args) over the fragment machine.
    fn emit_surface_macro_fragment(
        &mut self,
        import_path: &str,
        macro_name: &str,
        args: &[String],
    ) -> Result<()> {
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
            &mut *self.flattener,
            import_path,
            macro_name,
            args,
        )
    }

    /// Zig `emitStdSurfaceRule` (D4) with the production
    /// emitter. The dep ensurer takes D4's documented
    /// in-function-body branch (queue into `pending_std_deps`,
    /// flushed at function-body end); the queue fields are
    /// taken out of the program state for the call so the
    /// emitter can hold the whole state mutably, then put
    /// back. The symbol table is cloned for the same reason:
    /// D4 renders every argument text from it before the
    /// emitter appends anything, and every register rendered
    /// here was interned before this call, so the clone is
    /// exact for the values read.
    fn emit_surface_rule(
        &mut self,
        rule: &StdSurfaceRule,
        values: &StdSurfaceValues,
    ) -> Result<()> {
        let symbols: Vec<String> = self.b().symbols().to_vec();
        let mut included = std::mem::take(&mut self.state.program.included_imports);
        let mut pending = std::mem::take(&mut self.state.program.pending_std_deps);
        let result = {
            let mut emitter = MachineSurfaceEmitter {
                machine: FragmentMachine::new(&mut self.state),
                flattener: &mut *self.flattener,
            };
            let mut ensurer = PendingStdDeps::new(&mut included, &mut pending);
            emit_std_surface_rule(&mut emitter, &mut ensurer, rule, values, &symbols)
        };
        self.state.program.included_imports = included;
        self.state.program.pending_std_deps = pending;
        result
    }

    /// Zig `genAssociatedValueArg` (14721): a `clone` rule on
    /// a smart-pointer type unwraps a borrow argument to the
    /// underlying value; every other associated value argument
    /// is generated plainly.
    fn gen_associated_value_arg(
        &mut self,
        target_name: &str,
        member_name: &str,
        arg: &Node,
    ) -> Result<u32> {
        if associated_rule_needs_underlying_smart_pointer(target_name, member_name) {
            if let Node::BorrowExpr(borrow) = arg {
                return self.gen_expr(&borrow.expr);
            }
        }
        self.gen_expr(arg)
    }

    // -- probes --------------------------------------------------

    /// Zig `genVecNewCall` (15747): `Vec::new()` is a direct
    /// `@sa_vec_new()` call body over an ensured dep — it
    /// precedes (and therefore shadows) the `Vec new`
    /// associated surface rule, exactly as in Zig's chain.
    fn gen_vec_new_call(&mut self, expr: &Node, call: &CallExpr) -> Result<Option<u32>> {
        let target = call.associated_target.as_deref();
        if target != Some("Vec") || call.func_name != "new" || !call.args.is_empty() {
            return Ok(None);
        }
        let vec_ty = self
            .check
            .expr_type(expr)
            .cloned()
            .ok_or_else(|| Error::MissingType("Vec::new result type".to_string()))?;
        if vec_element_type(&vec_ty).is_none() {
            return Ok(None);
        }
        self.ensure_surface_std_deps("sa_std/vec.sa", &["sa_vec_new"])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        self.emit_surface_call_body(Some(dst), "@sa_vec_new()".to_string())?;
        Ok(Some(dst))
    }

    /// Zig `genGrowVecNewCall` (15760): `GrowVec::new()` /
    /// `GrowVec::with_max(n)` as `@sa_growvec_new(slot, max)`
    /// call bodies.
    fn gen_grow_vec_new_call(&mut self, expr: &Node, call: &CallExpr) -> Result<Option<u32>> {
        let target = call.associated_target.as_deref();
        if target != Some("GrowVec") {
            return Ok(None);
        }
        let is_new = call.func_name == "new" && call.args.is_empty();
        let is_with_max = call.func_name == "with_max" && call.args.len() == 1;
        if !is_new && !is_with_max {
            return Ok(None);
        }
        let gv_ty = self
            .check
            .expr_type(expr)
            .cloned()
            .ok_or_else(|| Error::MissingType("GrowVec constructor result type".to_string()))?;
        let elem_ty = match user_defined_generic_inner(&gv_ty, "GrowVec") {
            Some(elem) => elem,
            None => return Ok(None),
        };
        self.ensure_surface_std_deps("sa_std/growvec.sa", &["sa_growvec_new"])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let slot = vec_element_slot_size(&elem_ty);
        if is_new {
            self.emit_surface_call_body(Some(dst), format!("@sa_growvec_new({slot}, 0)"))?;
        } else {
            let max_reg = self.gen_expr(&call.args[0])?;
            let max_name = self.b().symbol_name(max_reg).to_string();
            self.emit_surface_call_body(
                Some(dst),
                format!("@sa_growvec_new({slot}, {max_name})"),
            )?;
            if !self.is_local_reg(max_reg) {
                self.emit_release(max_reg)?;
            }
        }
        Ok(Some(dst))
    }

    /// Zig `genVecDequeFromCall` (17615): `VecDeque::from([..])`
    /// is `VEC_DEQUE_NEW` plus one `VEC_DEQUE_PUSH_BACK`
    /// fragment per element.
    fn gen_vec_deque_from_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if !is_vec_deque_from_call(call) {
            return Ok(None);
        }
        let Node::ArrayLiteral(lit) = &call.args[0] else {
            return Err(unsupported(
                "VecDeque::from of a non-array-literal source",
                "D surface (surface_call)",
                "17615-17620",
            ));
        };
        let elements = lit.elements.clone();
        self.ensure_surface_std_deps(
            "sa_std/vec_deque.sa",
            &["sa_vec_deque_new", "sa_vec_deque_push_back"],
        )?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let dst_name = self.b().symbol_name(dst).to_string();
        self.emit_surface_macro_fragment(
            "sa_std/vec_deque.sa",
            "VEC_DEQUE_NEW",
            std::slice::from_ref(&dst_name),
        )?;
        for elem in &elements {
            let elem_reg = self.gen_expr(elem)?;
            let mut store_reg = elem_reg;
            let mut release = !self.is_local_reg(elem_reg);
            if let Some(elem_ty) = self.check.expr_type(elem).cloned() {
                let stored = self.gen_vec_deque_element_store_reg(elem, elem_reg, &elem_ty)?;
                store_reg = stored.0;
                release = stored.1 && release;
            }
            let store_name = self.b().symbol_name(store_reg).to_string();
            self.emit_surface_macro_fragment(
                "sa_std/vec_deque.sa",
                "VEC_DEQUE_PUSH_BACK",
                &[dst_name.clone(), store_name],
            )?;
            if release {
                self.emit_release(elem_reg)?;
            }
        }
        Ok(Some(dst))
    }

    /// Zig `genVecDequeElementStoreReg` (22602), the arms the
    /// driver can already serve: non-struct elements store
    /// directly; fresh owned records (struct literals, call
    /// results) transfer. The shallow-copy and byte-copy arms
    /// for live struct sources stop loudly with the Zig
    /// coordinate instead of being approximated.
    fn gen_vec_deque_element_store_reg(
        &mut self,
        arg: &Node,
        value_reg: u32,
        elem_ty: &Type,
    ) -> Result<(u32, bool)> {
        let mut layouts = LayoutAdapter::new(self.check);
        let decl = layouts.struct_decl_for_type(elem_ty);
        let Some(decl) = decl else {
            return Ok((value_reg, true));
        };
        if decl.is_opaque || decl.is_union {
            return Ok((value_reg, true));
        }
        if matches!(arg, Node::StructLiteral(_) | Node::CallExpr(_)) {
            return Ok((value_reg, false));
        }
        Err(unsupported(
            "VecDeque::from element store for a live struct source (shallow/byte copy arms)",
            "D surface (surface_call)",
            "22602-22640",
        ))
    }

    /// Zig `genPtrBuiltinCall`, associated-target forms only
    /// (`ptr::null()` / `ptr::read_volatile(p)`); the bare
    /// builtin-name forms belong to the helper-function slice.
    fn gen_ptr_builtin_call(&mut self, expr: &Node, call: &CallExpr) -> Result<Option<u32>> {
        if !call_needs_ptr_macros_associated(call) {
            return Ok(None);
        }
        let is_null = call.func_name == "null";
        if is_null {
            if !call.args.is_empty() {
                return Err(unsupported(
                    "ptr::null with arguments",
                    "D surface (surface_call)",
                    "genPtrBuiltinCall",
                ));
            }
            let dst = self.b().new_tmp();
            let dst_name = self.b().symbol_name(dst).to_string();
            self.emit_surface_macro_fragment(
                "sa_std/ptr.sa",
                "PTR_NULL",
                std::slice::from_ref(&dst_name),
            )?;
            return Ok(Some(dst));
        }
        if call.args.len() != 1 {
            return Err(unsupported(
                "ptr::read_volatile without exactly one argument",
                "D surface (surface_call)",
                "genPtrBuiltinCall",
            ));
        }
        let read_ty = self
            .check
            .expr_type(expr)
            .cloned()
            .ok_or_else(|| {
                unsupported(
                    "ptr::read_volatile result type is unknown",
                    "D surface (surface_call)",
                    "genPtrBuiltinCall",
                )
            })?;
        let macro_name = ptr_read_volatile_macro_name(&read_ty).ok_or_else(|| {
            unsupported(
                "ptr::read_volatile result type has no macro",
                "D surface (surface_call)",
                "genPtrBuiltinCall",
            )
        })?;
        let ptr_reg = self.gen_expr(&call.args[0])?;
        let dst = self.b().new_tmp();
        let dst_name = self.b().symbol_name(dst).to_string();
        let ptr_name = self.b().symbol_name(ptr_reg).to_string();
        self.emit_surface_macro_fragment("sa_std/ptr.sa", macro_name, &[dst_name, ptr_name])?;
        if !self.is_local_reg(ptr_reg) && sla_lowering::call_arg_needs_release(&call.args[0]) {
            self.emit_release(ptr_reg)?;
        }
        Ok(Some(dst))
    }

    /// Zig `genMutexNewCall` (17306): `Mutex::new(i32-like)`
    /// is the `MUTEX_NEW_I32` fragment.
    fn gen_mutex_new_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        let target = call.associated_target.as_deref();
        if target != Some("Mutex") || call.func_name != "new" {
            return Ok(None);
        }
        if call.args.len() != 1 {
            return Err(unsupported(
                "Mutex::new without exactly one argument",
                "D surface (surface_call)",
                "17306-17310",
            ));
        }
        let arg_ty = self
            .check
            .expr_type(&call.args[0])
            .cloned()
            .ok_or_else(|| Error::MissingType("Mutex::new argument type".to_string()))?;
        if !sla_lowering::is_i32_like_type(&arg_ty) {
            return Err(unsupported(
                "Mutex::new of a non-i32-like value",
                "D surface (surface_call)",
                "17306-17310",
            ));
        }
        let value_reg = self.gen_associated_value_arg("Mutex", "new", &call.args[0])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let dst_name = self.b().symbol_name(dst).to_string();
        let value_name = self.b().symbol_name(value_reg).to_string();
        self.emit_surface_macro_fragment(
            "sa_std/sync/mutex.sa",
            "MUTEX_NEW_I32",
            &[dst_name, value_name],
        )?;
        if !self.is_local_reg(value_reg) {
            self.emit_release(value_reg)?;
        }
        Ok(Some(dst))
    }

    /// Zig `genAtomicNewCall` (17332) over
    /// `planAtomicNewCall`: heap-allocate the value cell,
    /// then store the initial value through the `*_INIT`
    /// macro (the cell is heap-owned; a stack home would
    /// trap StackEscape on the consuming move).
    fn gen_atomic_new_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        let Some(plan) = plan_atomic_new_call(call) else {
            return Ok(None);
        };
        let target = call.associated_target.as_deref().unwrap_or("");
        let value_reg = self.gen_associated_value_arg(target, &call.func_name, &call.args[0])?;
        let dst = self.b().new_tmp();
        self.emit_alloc(dst, plan.size);
        let dst_name = self.b().symbol_name(dst).to_string();
        let value_name = self.b().symbol_name(value_reg).to_string();
        self.emit_surface_macro_fragment(
            "sa_std/sync/atomic.sa",
            plan.macro_name,
            &[dst_name, value_name],
        )?;
        if !self.is_local_reg(value_reg) {
            self.emit_release(value_reg)?;
        }
        Ok(Some(dst))
    }

    /// Zig `genStdSurfaceCall` (21345), the
    /// associated-target branch: explicit `Box::from_raw`,
    /// then the `associated` rule table, then the
    /// `Option::`/`Result::` constructor shorthand routed
    /// through the `constructor` rules of the result type.
    /// (The bare-call branches — constructor-by-result-type
    /// and the function/method/fallible-method rules keyed on
    /// a receiver argument — belong to later surface slices
    /// and are not ported here.)
    fn gen_std_surface_call(&mut self, expr: &Node, call: &CallExpr) -> Result<Option<u32>> {
        let target = call
            .associated_target
            .as_deref()
            .unwrap_or("")
            .to_string();
        // Explicit `Box::from_raw` (Zig 21345ff): the box IS
        // the reinterpreted raw pointer — one `ptr_add dst,
        // raw, 0` and a real move of the source; the generic
        // BOX_FROM_RAW rule leaves an owning intermediate
        // that leaks at function exit.
        if is_box_from_raw_call(call) {
            if call.args.len() != 1 {
                return Err(unsupported(
                    "Box::from_raw without exactly one argument",
                    "D surface (surface_call)",
                    "21345-21360",
                ));
            }
            let raw_reg = self.gen_associated_value_arg(&target, &call.func_name, &call.args[0])?;
            let dst = self.b().new_tmp();
            self.b().record_reg(dst);
            self.emit_ptr_add_imm(dst, raw_reg, 0);
            self.emit_move(raw_reg);
            return Ok(Some(dst));
        }
        let rule = find_std_surface_rule(
            &self.state.program.std_surface_rules,
            StdSurfaceRuleKind::Associated,
            &target,
            Some(&call.func_name),
        )
        .cloned();
        if let Some(rule) = rule {
            let value_reg = if rule.args.contains(&StdSurfaceArgKind::Value) {
                if call.args.len() != 1 {
                    return Err(unsupported(
                        "associated surface call with a value rule but not exactly one argument",
                        "D surface (surface_call)",
                        "21361-21375",
                    ));
                }
                Some(self.gen_associated_value_arg(&target, &call.func_name, &call.args[0])?)
            } else {
                if !call.args.is_empty() {
                    return Err(unsupported(
                        "associated surface call with arguments but no value rule",
                        "D surface (surface_call)",
                        "21361-21375",
                    ));
                }
                None
            };
            let dst = self.b().new_tmp();
            self.b().record_reg(dst);
            let expr_ty = self
                .check
                .expr_type(expr)
                .cloned()
                .ok_or_else(|| {
                    Error::MissingType("associated surface call result type".to_string())
                })?;
            let values = StdSurfaceValues {
                out: Some(dst),
                value: value_reg,
                elem_size: element_slot_size(&expr_ty),
                ..Default::default()
            };
            self.emit_surface_rule(&rule, &values)?;
            if let Some(reg) = value_reg {
                if rule.consume_value {
                    // The rule moves ownership out of the
                    // source; a real `move_` retires it in
                    // the emitted stream (markConsumed alone
                    // would leave it live at function exit).
                    self.emit_move(reg);
                } else if !self.is_local_reg(reg) {
                    self.emit_release(reg)?;
                }
            }
            return Ok(Some(dst));
        }
        // SA-text parity: `Option::Some/None`, `Result::Ok/Err`
        // lower exactly like their bare forms through the
        // constructor rules of the result type.
        if is_enum_ctor_shorthand(&target, &call.func_name) {
            if let Some(expr_ty) = self.check.expr_type(expr).cloned() {
                if let Some(type_name) = type_base_name(&expr_ty) {
                    let ctor_rule = find_std_surface_rule(
                        &self.state.program.std_surface_rules,
                        StdSurfaceRuleKind::Constructor,
                        &type_name,
                        Some(&call.func_name),
                    )
                    .cloned();
                    if let Some(ctor_rule) = ctor_rule {
                        if call.args.len() != ctor_shorthand_arity(&call.func_name) {
                            return Err(unsupported(
                                "enum constructor shorthand with wrong arity",
                                "D surface (surface_call)",
                                "21376-21410",
                            ));
                        }
                        let ctor_value = if !call.args.is_empty() {
                            Some(self.gen_expr(&call.args[0])?)
                        } else {
                            None
                        };
                        let dst = self.b().new_tmp();
                        self.b().record_reg(dst);
                        let values = StdSurfaceValues {
                            out: Some(dst),
                            value: ctor_value,
                            elem_size: element_slot_size(&expr_ty),
                            ..Default::default()
                        };
                        self.emit_surface_rule(&ctor_rule, &values)?;
                        if let Some(reg) = ctor_value {
                            // The macro stores the payload
                            // into the fresh enum object: a
                            // non-Copy payload is consumed by
                            // the store, never released.
                            let value_ty = self.check.expr_type(&call.args[0]).cloned();
                            match value_ty {
                                Some(ty) if !self.check.type_is_copy_value(&ty) => {
                                    self.mark_consumed(reg);
                                }
                                _ => {
                                    if !self.is_local_reg(reg) {
                                        self.emit_release(reg)?;
                                    }
                                }
                            }
                        }
                        return Ok(Some(dst));
                    }
                }
            }
        }
        Ok(None)
    }
}

/// Zig `associatedRuleNeedsUnderlyingSmartPointer`
/// (lowering_rules.zig:5397).
fn associated_rule_needs_underlying_smart_pointer(type_name: &str, member_name: &str) -> bool {
    if member_name != "clone" {
        return false;
    }
    type_name == "Rc" || type_name == "Arc"
}

/// Zig `isVecDequeFromCall` (lowering_rules.zig).
fn is_vec_deque_from_call(call: &CallExpr) -> bool {
    call.associated_target.as_deref() == Some("VecDeque")
        && call.func_name == "from"
        && call.args.len() == 1
}

/// Zig `isBoxFromRawCall` (lowering_rules.zig:4709) over
/// `isBoxTypeName`.
fn is_box_from_raw_call(call: &CallExpr) -> bool {
    call.associated_target.as_deref() == Some("Box") && call.func_name == "from_raw"
}

/// Zig `callNeedsPtrMacros` (lowering_rules.zig:4329),
/// associated-target forms only: target `ptr` (or its
/// mangled `std__ptr`) with member `null` / `read_volatile`.
fn call_needs_ptr_macros_associated(call: &CallExpr) -> bool {
    match call.associated_target.as_deref() {
        Some(target) => {
            (target == "std__ptr" || target == "ptr")
                && (call.func_name == "null" || call.func_name == "read_volatile")
        }
        None => false,
    }
}

/// Zig `ptrReadVolatileMacroName` (lowering_rules.zig:1507)
/// over `abiTypeString`.
fn ptr_read_volatile_macro_name(ty: &Type) -> Option<&'static str> {
    match sla_lowering::abi_type_string(ty) {
        "i32" => Some("PTR_READ_VOLATILE_I32"),
        "u64" => Some("PTR_READ_VOLATILE_U64"),
        "u8" => Some("PTR_READ_VOLATILE_U8"),
        _ => None,
    }
}

/// Zig `AtomicNewPlan` (lowering_rules.zig:4646).
struct AtomicNewPlan {
    size: usize,
    macro_name: &'static str,
}

/// Zig `planAtomicNewCall` (lowering_rules.zig:4646).
fn plan_atomic_new_call(call: &CallExpr) -> Option<AtomicNewPlan> {
    if call.func_name != "new" || call.args.len() != 1 {
        return None;
    }
    match call.associated_target.as_deref()? {
        "AtomicI32" => Some(AtomicNewPlan {
            size: 4,
            macro_name: "ATOMIC_I32_INIT",
        }),
        "AtomicUsize" => Some(AtomicNewPlan {
            size: 8,
            macro_name: "ATOMIC_USIZE_INIT",
        }),
        "AtomicPtr" => Some(AtomicNewPlan {
            size: 8,
            macro_name: "ATOMIC_PTR_INIT",
        }),
        _ => None,
    }
}

/// Zig `isEnumCtorShorthand` (sab_codegen.zig:21330).
fn is_enum_ctor_shorthand(target_name: &str, func_name: &str) -> bool {
    if target_name == "Option" {
        return func_name == "Some" || func_name == "None";
    }
    if target_name == "Result" {
        return func_name == "Ok" || func_name == "Err";
    }
    false
}

/// Zig `ctorShorthandArity` (sab_codegen.zig:21340).
fn ctor_shorthand_arity(func_name: &str) -> usize {
    if func_name == "None" {
        return 0;
    }
    1
}

#[cfg(test)]
mod tests;

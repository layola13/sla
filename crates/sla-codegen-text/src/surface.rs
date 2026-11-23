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
use crate::codegen::{type_size, vec_element_slot_size, Codegen};
use crate::Result;

pub(crate) enum SurfaceCall {
    Value(String),
    Void,
}

pub(crate) fn peel_name(ty: &Type) -> Option<(&str, &[Type])> {
    let mut cur = ty;
    loop {
        match cur {
            Type::Borrow(inner) | Type::Pointer(inner) => cur = inner,
            Type::UserDefined(inner) => return Some((inner.name.as_str(), &inner.generics)),
            _ => return None,
        }
    }
}

/// Peel `&T` / `*T` layers and return the innermost type,
/// whatever its kind (unlike `peel_name`, which only admits
/// user-defined types).
pub(crate) fn peel_borrow_pointer(ty: &Type) -> &Type {
    let mut cur = ty;
    loop {
        match cur {
            Type::Borrow(inner) | Type::Pointer(inner) => cur = inner,
            _ => return cur,
        }
    }
}

fn missing_fact() -> crate::TextError {
    crate::TextError::MissingType {
        context: "std surface call".to_string(),
    }
}

/// VECFIX compact stride: the VEC_PUSH / GROWVEC_NEW / VEC_REMOVE
/// stride is the shared rule in codegen.rs, not a word slot.
fn slot_size(elem: &Type) -> usize {
    vec_element_slot_size(elem)
}

/// Task 101/106: scalar element types whose Vec pop/push can be
/// inlined in the text codegen. Numeric/bool primitives only: the
/// value is a plain register word with no drop glue, so moving it
/// into/out of a slot is a single typed store/load (sign-extended
/// for narrow signed types, exactly like the runtime pop's load).
/// Pointer-like primitives (RawPtr/VoidType/VaList) and all
/// non-primitives keep the runtime calls (`sa_vec_try_pop_sized`
/// / `sa_vec_push`). `pub(crate)` so the task-114 lift analysis
/// (vec_lift.rs) applies the same claim condition.
pub(crate) fn scalar_vec_inline_eligible(elem: &Type) -> bool {
    matches!(
        elem,
        Type::Primitive(
            Primitive::Boolean
                | Primitive::I8
                | Primitive::I16
                | Primitive::I32
                | Primitive::I64
                | Primitive::Isize
                | Primitive::U8
                | Primitive::U16
                | Primitive::U32
                | Primitive::U64
                | Primitive::Usize
                | Primitive::F32
                | Primitive::F64
                | Primitive::Integer
                | Primitive::Float
        )
    )
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
    /// `pub(crate)` so the task-114 lift analysis (vec_lift.rs)
    /// applies the same claim condition as the surface arms.
    pub(crate) fn classify_receiver(&self, call: &CallExpr) -> Option<(String, Vec<Type>)> {
        let recv_ty = self.expr_type(&call.args[0]).or_else(|| {
            // Worker bodies are generated from the spawn helper's
            // CLONED closure AST (thread_spawn.rs stores
            // `closure.clone()`), so node-address-keyed checker
            // facts miss for every node inside the body. Captures
            // are seeded as local bindings with their types
            // (thread_spawn_emit.rs), so an identifier receiver
            // falls back to the binding's registered type.
            if let Node::Identifier(name) = &call.args[0] {
                self.local_binding_type_for_name(name)
            } else {
                None
            }
        })?;
        let (name, generics) = peel_name(&recv_ty)?;
        Some((name.to_string(), generics.to_vec()))
    }

    /// L4R2 Root S helper: does a user function shadow the
    /// surface method this call would dispatch to? True when a
    /// declaration with the call's name exists, with the call's
    /// arity, whose first parameter peels to the receiver's type
    /// NAME. Generics are deliberately NOT compared: the
    /// checker-recorded type of the argument at the call site
    /// can carry an untyped-literal inference artifact (the
    /// shadow corpus: the binding is Arc<i32> from the
    /// constructor, the recorded arg type is Arc<i64>), while
    /// the checker's own stage-5 resolution matched the call to
    /// the user decl regardless. Every declaration source is
    /// consulted (the program's own func_index AND the checker's
    /// registered funcs): the first-match func_decl lookup can
    /// return a same-named std wrapper whose signature does not
    /// match this call, and stopping there would miss the
    /// program's own shadowing declaration. `pub(crate)` so the
    /// task-114 lift analysis (vec_lift.rs) applies the same claim
    /// condition as the surface arms.
    pub(crate) fn user_fn_shadows_surface(
        &self,
        call: &CallExpr,
        tname: &str,
        _generics: &[Type],
    ) -> bool {
        if call.associated_target.is_some() {
            return false;
        }
        let matches = |decl: &sla_ast::FuncDecl| -> bool {
            if decl.params.len() != call.args.len() {
                return false;
            }
            let Some(first) = decl.params.first() else {
                return false;
            };
            let Some((pname, _)) = peel_name(&first.ty) else {
                return false;
            };
            pname == tname
        };
        if self
            .func_index
            .iter()
            .filter(|(n, _)| n == &call.func_name)
            .any(|(_, d)| matches(d))
        {
            return true;
        }
        if let Some(decl) = self.check.func(&call.func_name) {
            if matches(decl) {
                return true;
            }
        }
        false
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

    /// FUSED `v.pop().unwrap()` (DEF-RUST-POP-OPTION-UNBOXING, DIAG
    /// fix-4; mirror of the Zig task-48 fusion, codegen.zig
    /// isUnwrapCall arm): when the receiver is a direct `Vec::pop()`
    /// call, skip the heap Option box entirely and run try_pop
    /// straight into the unwrapped value, panicking on None exactly
    /// like OPTION_UNWRAP does. Saves 2 heap malloc/free per call
    /// (the try_pop out slot + the 16-byte Option box). The out slot
    /// lives on the stack (`stack_alloc 8`), mirroring the chan_make
    /// arm (call.rs:868) — a stack slot is released implicitly at
    /// scope exit; an explicit `!` on it is a host StackEscape error.
    /// The try_pop call and the load mirror the unfused pop arm
    /// exactly (sized entry + load by element type), so the fused
    /// path is behavior-identical to the unfused one for every
    /// element type.
    /// Returns None (fall through to the normal OPTION_UNWRAP) unless
    /// the receiver is a direct Vec pop the surface pop arm would
    /// claim — same claim conditions (Vec receiver, no user-fn
    /// shadow), so a user `fn pop` or a non-Vec pop never fuses.

    /// Task 101: inline scalar Vec pop. Emits the len check +
    /// len decrement + direct slot load for a scalar element
    /// type, with no runtime call and no stack out-slot. The
    /// emitted shape mirrors the Vec index-read arm for the load
    /// sequence (expr.rs) and the fused pop arm for the branch
    /// structure:
    ///   {len} = load {recv}+{len_def} as u64
    ///   {zero} = 0
    ///   {ok} = ult {zero}, {len}
    ///   !{zero}
    ///   br {ok} -> {some}, {none}
    /// {some}:
    ///   !{ok}
    ///   {one} = 1
    ///   {newlen} = sub {len}, {one}
    ///   store {recv}+{len_def}, {newlen} as u64
    ///   {data} = load {recv}+{ptr_def} as ptr
    ///   {off} = mul {newlen}, {slot}
    ///   {ptr} = ptr_add {data}, {off}
    ///   {result} = load {ptr}+0 as {load_ty}
    ///   [!{ptr} !{off} !{data} !{newlen} !{one} !{len}]
    /// On return the insertion point is inside the SOME arm right
    /// after the load; the caller emits its SOME tail, then the
    /// NONE label + tail (starting with `!{ok}`), then the END
    /// label. Behavior is identical to `sa_vec_try_pop_sized`
    /// for scalar elements: an empty vec takes the NONE arm
    /// without touching the buffer (no null deref -- the slot load
    /// only runs when len > 0), and a narrow signed element
    /// comes back sign-extended via the typed load (same rule as
    /// the index-read and unfused pop arms).
    ///
    /// Task 114: when the receiver is lifted by an enclosing loop
    /// (`vec_lift.rs`), `len_reg`/`data_reg` are the lift locals
    /// instead of fresh temps: no header loads are emitted, the
    /// len store becomes a retire-then-copy into the len local
    /// (`!len; len = add newlen, 0` — the `add x, 0` copy idiom,
    /// so `newlen` stays live for the offset below; a plain
    /// `{len} = {newlen}` would be an SA-level move and the offset
    /// would trap UseAfterMove), and the locals are never released
    /// (they stay live across iterations; the loop exit retires
    /// them). The unlifted shape is byte-identical to before.
    fn emit_inline_pop_scalar(
        &mut self,
        recv: &str,
        elem: &Type,
        len_def: &str,
        ptr_def: &str,
        ok_reg: &str,
        result_reg: &str,
        some_label: &str,
        none_label: &str,
    ) -> Result<()> {
        // Task 114: lift lookup (cloned up front so the borrow ends
        // before the mutable emission below).
        let lift = self.lifted_vec_header(recv).cloned();
        let lifted = lift.is_some();
        let (len_reg, data_reg) = match &lift {
            Some(l) => (l.len_local.clone(), l.ptr_local.clone()),
            None => (self.new_tmp(), self.new_tmp()),
        };
        let zero_reg = self.new_tmp();
        let one_reg = self.new_tmp();
        let newlen_reg = self.new_tmp();
        let offset_reg = self.new_tmp();
        let ptr_reg = self.new_tmp();
        let load_ty = if matches!(elem, Type::Primitive(_)) {
            crate::codegen::type_string(elem)
        } else {
            "ptr"
        };
        if !lifted {
            self.ins(&format!("{len_reg} = load {recv}+{len_def} as u64"));
        }
        self.emit_int_const(&zero_reg, 0);
        self.ins(&format!("{ok_reg} = ult {zero_reg}, {len_reg}"));
        self.w(&format!("    !{zero_reg}\n"));
        self.w(&format!(
            "    br {ok_reg} -> {some_label}, {none_label}\n\n"
        ));
        self.wl(&format!("{some_label}:"));
        self.w(&format!("    !{ok_reg}\n"));
        self.emit_int_const(&one_reg, 1);
        self.ins(&format!("{newlen_reg} = sub {len_reg}, {one_reg}"));
        if lifted {
            // Retire the old len value, then copy the new len into
            // the lift local (a copy, not a move: `newlen` is still
            // needed for the offset below).
            self.w(&format!("    !{len_reg}\n"));
            self.ins(&format!("{len_reg} = add {newlen_reg}, 0"));
        } else {
            self.ins(&format!(
                "store {recv}+{len_def}, {newlen_reg} as u64"
            ));
        }
        if !lifted {
            self.ins(&format!("{data_reg} = load {recv}+{ptr_def} as ptr"));
        }
        self.ins(&format!(
            "{offset_reg} = mul {newlen_reg}, {}",
            vec_element_slot_size(elem)
        ));
        self.ins(&format!("{ptr_reg} = ptr_add {data_reg}, {offset_reg}"));
        self.ins(&format!(
            "{result_reg} = load {ptr_reg}+0 as {load_ty}"
        ));
        self.emit_release(&ptr_reg)?;
        self.emit_release(&offset_reg)?;
        if !lifted {
            self.emit_release(&data_reg)?;
        }
        self.emit_release(&newlen_reg)?;
        self.emit_release(&one_reg)?;
        if !lifted {
            self.emit_release(&len_reg)?;
        }
        Ok(())
    }


    /// Task 106: inline scalar Vec push. Emits the capacity check
    /// + direct slot store for a scalar element type, with the
    /// runtime `VEC_PUSH` macro only on the grow path (len ==
    /// cap). The emitted shape mirrors the task-101 inline pop
    /// for the branch structure and the Vec index-assign arm for
    /// the slot store:
    ///   {len} = load {recv}+{len_def} as u64
    ///   {cap} = load {recv}+{cap_def} as u64
    ///   {ok} = ult {len}, {cap}
    ///   !{cap}
    ///   br {ok} -> {fast}, {slow}
    /// {fast}:
    ///   !{ok}
    ///   {data} = load {recv}+{ptr_def} as ptr
    ///   {off} = mul {len}, {slot}
    ///   {ptr} = ptr_add {data}, {off}
    ///   store {ptr}+0, {value} as {store_ty}
    ///   {one} = 1
    ///   {newlen} = add {len}, {one}
    ///   store {recv}+{len_def}, {newlen} as u64
    ///   [!{ptr} !{off} !{data} !{newlen} !{one} !{len}]
    ///   jmp {end}
    /// {slow}:
    ///   !{ok}
    ///   EXPAND {macro} {recv}, {value}, {slot}
    ///   jmp {end}
    /// {end}:
    /// On return the insertion point is at the END label; the
    /// caller runs the arm's value-finish step (shared by both
    /// paths, exactly as the call path did) and returns Void.
    /// Behavior is identical to `sa_vec_push` for scalar
    /// elements: the fast path stores the value word into the
    /// slot and bumps len (the buffer is untouched otherwise --
    /// the slot write only runs when len < cap, so no OOB
    /// store); the slow path is the runtime call verbatim, so
    /// growth, reallocation, and the first push into an empty
    /// vec all behave exactly as before. Narrow values are
    /// stored with the typed store (same rule as the index-assign
    /// arm); the value register itself is only read, never
    /// consumed, by the inline sequence -- the caller's
    /// surface_finish_value runs unchanged after END.
    ///
    /// Task 114: when the receiver is lifted by an enclosing loop
    /// (`vec_lift.rs`), `len_reg`/`cap_reg`/`data_reg` are the lift
    /// locals instead of fresh temps: no header loads are emitted,
    /// the len store becomes a retire-then-copy into the len local
    /// (`!len; len = add newlen, 0` — the `add x, 0` copy idiom, so
    /// the shape stays identical to the unlifted path), and the
    /// locals are never released. The slow path syncs the lifted len
    /// back before the runtime call (the call reads the header) and
    /// reloads all three fields into the lift locals after it
    /// (growth may have reallocated). The unlifted shape is
    /// byte-identical to before.
    fn emit_inline_push_scalar(
        &mut self,
        recv: &str,
        elem: &Type,
        len_def: &str,
        cap_def: &str,
        ptr_def: &str,
        value_reg: &str,
        macro_name: &str,
        ok_reg: &str,
        fast_label: &str,
        slow_label: &str,
        end_label: &str,
    ) -> Result<()> {
        // Task 114: lift lookup (cloned up front so the borrow ends
        // before the mutable emission below).
        let lift = self.lifted_vec_header(recv).cloned();
        let lifted = lift.is_some();
        let (len_reg, cap_reg, data_reg) = match &lift {
            Some(l) => (
                l.len_local.clone(),
                l.cap_local.clone(),
                l.ptr_local.clone(),
            ),
            None => (self.new_tmp(), self.new_tmp(), self.new_tmp()),
        };
        let offset_reg = self.new_tmp();
        let ptr_reg = self.new_tmp();
        let one_reg = self.new_tmp();
        let newlen_reg = self.new_tmp();
        let store_ty = if matches!(elem, Type::Primitive(_)) {
            crate::codegen::type_string(elem)
        } else {
            "ptr"
        };
        if !lifted {
            self.ins(&format!("{len_reg} = load {recv}+{len_def} as u64"));
            self.ins(&format!("{cap_reg} = load {recv}+{cap_def} as u64"));
        }
        self.ins(&format!("{ok_reg} = ult {len_reg}, {cap_reg}"));
        if !lifted {
            self.w(&format!("    !{cap_reg}\n"));
        }
        self.w(&format!(
            "    br {ok_reg} -> {fast_label}, {slow_label}\n\n"
        ));
        self.wl(&format!("{fast_label}:"));
        self.w(&format!("    !{ok_reg}\n"));
        if !lifted {
            self.ins(&format!("{data_reg} = load {recv}+{ptr_def} as ptr"));
        }
        self.ins(&format!(
            "{offset_reg} = mul {len_reg}, {}",
            vec_element_slot_size(elem)
        ));
        self.ins(&format!("{ptr_reg} = ptr_add {data_reg}, {offset_reg}"));
        self.ins(&format!(
            "store {ptr_reg}+0, {value_reg} as {store_ty}"
        ));
        self.emit_int_const(&one_reg, 1);
        self.ins(&format!("{newlen_reg} = add {len_reg}, {one_reg}"));
        if lifted {
            // Retire the old len value, then copy the new len into
            // the lift local (a copy, not a move, so the release
            // list below stays the same shape as the unlifted path).
            self.w(&format!("    !{len_reg}\n"));
            self.ins(&format!("{len_reg} = add {newlen_reg}, 0"));
        } else {
            self.ins(&format!(
                "store {recv}+{len_def}, {newlen_reg} as u64"
            ));
        }
        self.emit_release(&ptr_reg)?;
        self.emit_release(&offset_reg)?;
        if !lifted {
            self.emit_release(&data_reg)?;
        }
        self.emit_release(&newlen_reg)?;
        self.emit_release(&one_reg)?;
        if !lifted {
            self.emit_release(&len_reg)?;
        }
        self.w(&format!("    jmp {end_label}\n\n"));
        self.wl(&format!("{slow_label}:"));
        self.w(&format!("    !{ok_reg}\n"));
        if lifted {
            // Sync the lifted len back: the runtime push reads the
            // header, and only len can be stale (the fast path never
            // changes ptr/cap).
            self.ins(&format!("store {recv}+{len_def}, {len_reg} as u64"));
        }
        self.ins(&format!(
            "EXPAND {macro_name} {recv}, {value_reg}, {}",
            slot_size(elem)
        ));
        if let Some(l) = &lift {
            // Growth may have reallocated: reload all three header
            // fields into the lift locals before the next iteration
            // reads them. Each reload retires the local's old value
            // first, then moves the fresh load in.
            let tmp = self.new_tmp();
            self.ins(&format!("{tmp} = load {recv}+{ptr_def} as ptr"));
            self.w(&format!("    !{}\n", l.ptr_local));
            self.ins(&format!("{} = {tmp}", l.ptr_local));
            let tmp = self.new_tmp();
            self.ins(&format!("{tmp} = load {recv}+{len_def} as u64"));
            self.w(&format!("    !{}\n", l.len_local));
            self.ins(&format!("{} = {tmp}", l.len_local));
            let tmp = self.new_tmp();
            self.ins(&format!("{tmp} = load {recv}+{cap_def} as u64"));
            self.w(&format!("    !{}\n", l.cap_local));
            self.ins(&format!("{} = {tmp}", l.cap_local));
        }
        self.w(&format!("    jmp {end_label}\n\n"));
        self.wl(&format!("{end_label}:"));
        Ok(())
    }

    fn try_gen_pop_unwrap_fused(
        &mut self,
        call: &CallExpr,
    ) -> Result<Option<SurfaceCall>> {
        let Node::CallExpr(inner) = &call.args[0] else {
            return Ok(None);
        };
        if inner.func_name != "pop"
            || inner.args.len() != 1
            || inner.callee.is_some()
            || inner.associated_target.is_some()
        {
            return Ok(None);
        }
        let Some((tname, generics)) = self.classify_receiver(inner) else {
            return Ok(None);
        };
        if tname != "Vec"
            || generics.is_empty()
            || self.user_fn_shadows_surface(inner, "Vec", &generics)
        {
            return Ok(None);
        }
        let recv = self.gen_expr(&inner.args[0])?;
        let ok_reg = self.new_tmp();
        let value_slot = self.new_tmp();
        let result_reg = self.new_tmp();
        let some_label = self.new_label("L_VEC_POP_UNWRAP_SOME");
        let none_label = self.new_label("L_VEC_POP_UNWRAP_NONE");
        let end_label = self.new_label("L_VEC_POP_UNWRAP_END");
        // Element type of the popped value: the sized try_pop entry
        // needs the slot size, and the load must read the slot as the
        // element type (task 54 §2a — the unfused pop arm does exactly
        // this; the legacy word-stride entry is stale since task 54
        // landed and reads the wrong slot for narrow elements).
        let elem = generics
            .first()
            .ok_or_else(|| missing_fact())?
            .clone();
        if scalar_vec_inline_eligible(&elem) {
            // Task 101: inline scalar pop -- no call, no
            // out-slot. The helper leaves the insertion point at
            // the SOME label with result_reg holding the popped
            // value; the None arm + END below are the same shape
            // as the call path (PANIC_MSG 17 verbatim).
            self.emit_inline_pop_scalar(
                &recv, &elem, "Vec_len", "Vec_ptr", &ok_reg, &result_reg, &some_label,
                &none_label,
            )?;
            self.w(&format!("    jmp {end_label}\n\n"));
            self.wl(&format!("{none_label}:"));
            self.w(&format!("    !{ok_reg}\n"));
            self.ins("EXPAND PANIC_MSG 17, *OPTION_UNWRAP_PANIC, 39");
            self.ins(&format!("{result_reg} = add 0, 0"));
            self.wl(&format!("{end_label}:"));
            if sla_lowering::call_arg_needs_release(&inner.args[0]) {
                self.emit_release(&recv)?;
            }
            return Ok(Some(SurfaceCall::Value(result_reg)));
        }
        self.ins(&format!("{value_slot} = stack_alloc 8"));
        self.ins(&format!(
            "{ok_reg} = call @sa_vec_try_pop_sized(&{recv}, &{value_slot}, {})",
            vec_element_slot_size(&elem)
        ));
        self.w(&format!(
            "    br {ok_reg} -> {some_label}, {none_label}\n\n"
        ));
        self.wl(&format!("{some_label}:"));
        self.w(&format!("    !{ok_reg}\n"));
        // The sized entry zeroes the slot and moves the element at its
        // own width, so the slot holds the width-exact value: load it
        // as the element type, exactly like the unfused pop arm, so a
        // narrow signed element comes back sign-extended instead of
        // zero-extended.
        let load_ty = if matches!(elem, Type::Primitive(_)) {
            crate::codegen::type_string(&elem)
        } else {
            "ptr"
        };
        self.ins(&format!(
            "{result_reg} = load {value_slot}+0 as {load_ty}"
        ));
        self.w(&format!("    jmp {end_label}\n\n"));
        self.wl(&format!("{none_label}:"));
        self.w(&format!("    !{ok_reg}\n"));
        // OPTION_UNWRAP's None arm, verbatim (option.sa:497-498):
        // panic 17 with the same message and length.
        self.ins("EXPAND PANIC_MSG 17, *OPTION_UNWRAP_PANIC, 39");
        self.ins(&format!("{result_reg} = add 0, 0"));
        self.wl(&format!("{end_label}:"));
        if sla_lowering::call_arg_needs_release(&inner.args[0]) {
            self.emit_release(&recv)?;
        }
        Ok(Some(SurfaceCall::Value(result_reg)))
    }

    /// codegen.zig:13671 `constructorArgNeedsConsume`
    /// (DEF-TEXT31-BOX-CONSUME): retire the source of a
    /// Box/Rc/Arc::new argument as consumed when the argument
    /// needed a release temp, when gen_expr materialised a fresh
    /// temp for it, or when it is a root identifier the checker
    /// consumed (its type is not Copy under the checker's own
    /// oracle — the same fact consume_std_constructor_arg used,
    /// exposed here through CheckInfo::type_is_copy_value).
    fn constructor_arg_needs_consume(&self, arg: &Node, arg_reg: &str) -> bool {
        if sla_lowering::call_arg_needs_release(arg) {
            return true;
        }
        if sla_lowering::is_temporary_register_name(arg_reg) {
            return true;
        }
        if !matches!(arg, Node::Identifier(_)) {
            return false;
        }
        match self.expr_type(arg) {
            Some(ty) => !self.check.type_is_copy_value(&ty),
            None => false,
        }
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
        self.surface_value_consumed_node(&call.args[idx])
    }

    /// Node form of `surface_value_consumed` (the bare `vec(...)`
    /// constructor plans its element arguments the same way a
    /// push plans its value argument, but they are not at a
    /// fixed call-argument index).
    fn surface_value_consumed_node(&mut self, arg: &Node) -> Result<LoweredCallArg> {
        // String literals keep the unplanned path: planning them
        // against a String parameter selects the
        // RawPointerStringLiteral kind (a C-string pointer),
        // which collection inserts do not accept — they take the
        // string value the literal materialises as.
        if matches!(arg, Node::Literal(sla_ast::Literal::StringVal(_))) {
            return self.gen_planned_call_arg(arg, None, false);
        }
        let param = self.expr_type(arg).map(|ty| Param {
            name: String::new(),
            ty: Box::new(ty),
            is_borrow: false,
            is_move: false,
            fallible_sugar: false,
            is_var: false,
        });
        let mut planned = self.gen_planned_call_arg(arg, param.as_ref(), false)?;
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
        // Same for the `&` capability marker: a BorrowExpr value
        // (e.g. `&g[i]`) planned against a borrow-typed parameter
        // comes back as `&tmp` from the shared call planner, where
        // the marker is required for declared borrow callees. As
        // an EXPAND operand the marker is rejected
        // (CapabilityMismatch on Vec<&T> pushes of &g[i]), so
        // strip it here: the operand is the slot-address value
        // itself, and the address temp is forgotten in
        // surface_finish_value like the plain-operand path
        // forgets it. Only a temporary register is stripped —
        // a `&name` operand is the fused marker form for an
        // identifier borrow and is left as planned.
        if let Some(raw) = planned.reg.strip_prefix('&') {
            if sla_lowering::is_temporary_register_name(raw) {
                let raw = raw.to_string();
                planned.reg = raw.clone();
                if planned.release_reg.take().is_some() {
                    planned.consume_reg = Some(raw);
                }
            }
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

    /// One `VEC_PUSH` of `arg` into the vec in `reg`, planned exactly
    /// as the ("Vec", "push") receiver arm plans its value. Extracted
    /// from the bare `vec(...)` arm (task 137) so the stack-allocated
    /// ctor (`emit_stack_vec_ctor`) can reuse it for per-element
    /// initialisation. `is_stack` selects the borrow-form push
    /// (`VEC_PUSH_STACK_B`, task 144 shape (b)) for a stack-allocated
    /// header — the move-form `VEC_PUSH` would trap (host StackEscape,
    /// trap 1025) on a stack allocation; a heap vec in a temp takes
    /// the plain form, exactly as before.
    fn emit_vec_push_element(
        &mut self,
        reg: &str,
        arg: &Node,
        elem: &Type,
        is_stack: bool,
    ) -> Result<()> {
        let push_macro = if is_stack { "VEC_PUSH_STACK_B" } else { "VEC_PUSH" };
        if let Some(copy_reg) = self.push_arg_copy_materialization_node(arg, elem)? {
            self.ins(&format!(
                "EXPAND {push_macro} {reg}, {copy_reg}, {}",
                slot_size(elem)
            ));
            // The copy's ownership moves into the vec:
            // forget it, never release (same as the push receiver arm).
            self.emit_forget_moved_value(&copy_reg)?;
            return Ok(());
        }
        let planned = self.surface_value_consumed_node(arg)?;
        let value = planned.reg.clone();
        self.ins(&format!(
            "EXPAND {push_macro} {reg}, {value}, {}",
            slot_size(elem)
        ));
        self.surface_finish_value(&planned, true)
    }

    /// Task 137 (P1): emit a stack-allocated Vec header into `target`
    /// for `let target: Vec<elem> = Vec::new()` (or `vec(...)`).
    /// Precondition: `plan_vec_stack_header` approved this binding
    /// (non-escaping over its whole lifetime) and the caller applied
    /// the emission-time filters (temp name, assigned-value slot,
    /// addressable, captures). Emits `stack_alloc` for the header
    /// plus the three in-place init stores via `VEC_NEW_STACK`
    /// (the `Vec_SIZE` form mirrors the Slice precedent in
    /// call.rs:599), then one `VEC_PUSH_STACK_B` per `vec(...)`
    /// element (borrow form, task 144 shape (b) — the move-form
    /// `VEC_PUSH` would trap on a stack allocation); `Vec::new()`
    /// takes no elements.
    /// The scope-exit release frees only the buffer (`VEC_FREE_BUF`)
    /// — see release.rs.
    pub(crate) fn emit_stack_vec_ctor(
        &mut self,
        target: &str,
        call: &CallExpr,
        elem: &Type,
    ) -> Result<()> {
        self.ins(&format!("{target} = stack_alloc Vec_SIZE"));
        self.ins(&format!("EXPAND VEC_NEW_STACK {target}"));
        for arg in &call.args {
            self.emit_vec_push_element(target, arg, elem, true)?;
        }
        Ok(())
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
                ("VecDeque", "from") => {
                    // codegen.zig:21330 `genVecDequeFromCall`: NEW plus one
                    // PUSH_BACK per element; each element is planned the
                    // way the push_back receiver arm plans its value
                    // (copy-materialization for live copy-like bindings,
                    // ownership transfer otherwise) — the Rust text
                    // backend has no transfer instruction, so the
                    // transfer is the plain operand plus the post-EXPAND
                    // forget in surface_finish_value, as in the push arm.
                    if call.args.len() != 1 {
                        return Err(missing_fact());
                    }
                    let Node::ArrayLiteral(lit) = &call.args[0] else {
                        return Err(missing_fact());
                    };
                    let result_ty = self.expr_type(expr).ok_or(missing_fact())?;
                    let (_, generics) = peel_name(&result_ty).ok_or(missing_fact())?;
                    let elem = generics
                        .first()
                        .ok_or(missing_fact())?
                        .clone();
                    let reg = self.new_tmp();
                    self.ins(&format!("EXPAND VEC_DEQUE_NEW {reg}"));
                    for elem_node in &lit.elements {
                        if let Some(copy_reg) =
                            self.push_arg_copy_materialization_node(elem_node, &elem)?
                        {
                            self.ins(&format!(
                                "EXPAND VEC_DEQUE_PUSH_BACK {reg}, {copy_reg}"
                            ));
                            self.emit_forget_moved_value(&copy_reg)?;
                        } else {
                            let planned = self.surface_value_consumed_node(elem_node)?;
                            let value = planned.reg.clone();
                            self.ins(&format!(
                                "EXPAND VEC_DEQUE_PUSH_BACK {reg}, {value}"
                            ));
                            self.surface_finish_value(&planned, true)?;
                        }
                    }
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
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
                ("Box", "new") | ("Rc", "new") | ("Arc", "new") => {
                    // codegen.zig isBoxNewCall / isRcNewCall /
                    // isArcNewCall arms: the argument is generated
                    // plainly (genExpr — no call-argument plan) and
                    // the source retires as consumed exactly when
                    // constructorArgNeedsConsume says so. Planning
                    // it through surface_value_consumed selected
                    // the shallow-copy plan for plain-field struct
                    // identifiers: the copy was forgotten and the
                    // source binding — already consumed in the
                    // checker's plan, so absent from the exit
                    // cleanups — stayed Active at function exit
                    // (host MemoryLeak 1012, LINE-PARALLEL).
                    if call.args.len() != 1 {
                        return Err(missing_fact());
                    }
                    let macro_name = match (target.as_str(), name) {
                        ("Box", "new") => "BOX_NEW",
                        ("Rc", "new") => "RC_NEW",
                        _ => "ARC_NEW",
                    };
                    let value = self.gen_expr(&call.args[0])?;
                    // F-DYN L3: when the checker recorded a dyn
                    // coercion for this call (a let declared
                    // `Rc<__dyn_T>` / `Box<__dyn_T>`), the wrapper
                    // must hold a trait-object fat cell, not the
                    // raw payload — materialise it exactly as the
                    // Zig emission does (alloc Dyn_SIZE, store the
                    // data pointer and the impl vtable) and let
                    // the *_NEW macro consume the cell. Without
                    // this the RC_GET / box payload later feeds a
                    // dispatch that loads Dyn_vtable out of the
                    // payload's own bytes (signal 11).
                    if let Some(trait_name) = self
                        .check
                        .dyn_smart_coercion(expr)
                        .map(str::to_string)
                    {
                        let arg_ty = self
                            .expr_type(&call.args[0])
                            .ok_or_else(|| missing_fact())?;
                        let type_name =
                            sla_lowering::concrete_type_name(&arg_ty)
                                .ok_or_else(|| missing_fact())?
                                .to_string();
                        let vt_name = Self::trait_vtable_name(
                            &trait_name,
                            &type_name,
                        );
                        let fat = self.new_tmp();
                        self.ins(&format!("{fat} = alloc Dyn_SIZE"));
                        self.ins(&format!(
                            "store {fat}+Dyn_data, {value} as ptr"
                        ));
                        self.ins(&format!(
                            "store {fat}+Dyn_vtable, &{vt_name} as ptr"
                        ));
                        if self.constructor_arg_needs_consume(
                            &call.args[0],
                            &value,
                        ) {
                            self.emit_forget_moved_value(&value)?;
                        }
                        let reg = self.new_tmp();
                        self.ins(&format!(
                            "EXPAND {macro_name} {reg}, {fat}"
                        ));
                        self.emit_forget_moved_value(&fat)?;
                        return Ok(Some(SurfaceCall::Value(reg)));
                    }
                    let reg = self.new_tmp();
                    self.ins(&format!("EXPAND {macro_name} {reg}, {value}"));
                    if self.constructor_arg_needs_consume(&call.args[0], &value) {
                        self.emit_forget_moved_value(&value)?;
                    }
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
                ("Box", "into_raw")
                | ("Box", "from_raw")
                | ("RefCell", "new")
                | ("Mutex", "new")
                | ("RwLock", "new") => {
                    if call.args.len() != 1 {
                        return Err(missing_fact());
                    }
                    let macro_name = match (target.as_str(), name) {
                        ("Box", "into_raw") => "BOX_INTO_RAW",
                        ("Box", "from_raw") => "BOX_FROM_RAW",
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
        // ---- bare `vec(...)` constructor (codegen.zig:16097) ----
        // VEC_NEW plus one VEC_PUSH per element, each element
        // planned exactly as the ("Vec", "push") receiver arm
        // plans its value. Without this arm the call fell to the
        // bare-name catch-all and synthesised `EXPAND vec`, a
        // macro that exists in no std tree (host UndefinedMacro,
        // test_unit_def_zrange_vec_handle).
        if call.func_name == "vec"
            && call.associated_target.is_none()
            && call.callee.is_none()
        {
            let result_ty = self.expr_type(expr).ok_or(missing_fact())?;
            let (_, generics) = peel_name(&result_ty).ok_or(missing_fact())?;
            let elem = generics.first().ok_or(missing_fact())?.clone();
            let reg = self.new_tmp();
            self.ins(&format!("EXPAND VEC_NEW {reg}"));
            for arg in &call.args {
                self.emit_vec_push_element(&reg, arg, &elem, false)?;
            }
            return Ok(Some(SurfaceCall::Value(reg)));
        }
        // ---- free `len` over the surface containers ----
        if call.func_name == "len" && call.args.len() == 1 && call.callee.is_none() {
            // DEF-2 / bend2 (Zig len arm, codegen.zig:16134): a
            // fixed array's length is the compile-time element
            // count (an integer constant, the argument is not
            // generated); a bare `ptr` carries no length field
            // and is NUL-scanned with CSTR_LEN (shared plan
            // planLenRawPtrReceiver — parameters and STR_PTR
            // results are genuine C strings; an inferred
            // `let s = "lit"` binding types as a Slice in this
            // checker and takes the SLICE_GET_LEN arm below).
            // Both shapes fell to the EXPAND catch-all before
            // (`EXPAND len`, host UndefinedMacro:
            // test_unit_bend2_parity_array, test_unit_def2_ptr_len).
            if let Some(arg_ty) = self.expr_type(&call.args[0]) {
                match peel_borrow_pointer(&arg_ty) {
                    Type::Array(arr) => {
                        let n = arr.len;
                        let reg = self.new_tmp();
                        self.ins(&format!("{reg} = {n}"));
                        return Ok(Some(SurfaceCall::Value(reg)));
                    }
                    Type::Primitive(Primitive::RawPtr) => {
                        let recv = self.gen_expr(&call.args[0])?;
                        let reg = self.new_tmp();
                        self.ins(&format!("EXPAND CSTR_LEN {reg}, {recv}"));
                        return Ok(Some(SurfaceCall::Value(reg)));
                    }
                    _ => {}
                }
            }
            let macro_name = match self.classify_receiver(call) {
                Some((tname, _)) => match tname.as_str() {
                    "Vec" => "VEC_LEN",
                    "HashMap" => "MAP_LEN",
                    "BTreeMap" => "BTREE_MAP_LEN",
                    "HashSet" => "SET_LEN",
                    "BTreeSet" => "BTREE_SET_LEN",
                    "String" => "STRING_BUF_LEN",
                    "GrowVec" => "GROWVEC_LEN",
                    // F-B6 (`len` on a slice borrow, LINE-LEN
                    // follow-up): `view.len()` on `&[u8]` classifies
                    // to ("Slice", [u8]) but fell to the EXPAND
                    // catch-all, synthesising `EXPAND len tmp, view`
                    // — `len` is not a sa_std macro (UndefinedMacro
                    // in test_unit_borrow_return_views @sla__view_eq).
                    // Zig emits `EXPAND SLICE_GET_LEN len, slice`
                    // (sa_std/core/slice.sa; demos/c/*/main.sa).
                    "Slice" => "SLICE_GET_LEN",
                    _ => return Ok(None),
                },
                None => return Ok(None),
            };
            let (recv, _, _) = self.surface_receiver(call)?;
            let reg = self.new_tmp();
            self.ins(&format!("EXPAND {macro_name} {reg}, {recv}"));
            // The receiver IS retired when it is an owned temp
            // (same settle as the copied()/unwrap_or arms above):
            // `len(redis_value_bulk(&e))` otherwise leaves the
            // call-result register Active at function exit
            // (MemoryLeak, LINE-REG3 S2).
            if sla_lowering::call_arg_needs_release(&call.args[0]) {
                self.emit_release(&recv)?;
            }
            return Ok(Some(SurfaceCall::Value(reg)));
        }
        // ---- String `as_bytes` / `bytes` (codegen.zig:17391) ----
        // An owned String expands STRING_BUF_AS_BYTES; a bare
        // C-string pointer or a byte array expands STR_AS_BYTES
        // (both in sa_std/string.sa; the min-import scan already
        // keeps string.sa for String-receiver method calls).
        // Without this arm the call fell to the EXPAND catch-all
        // (`EXPAND as_bytes`, host UndefinedMacro:
        // test_unit_def2_string_as_bytes, test_unit_def6_string_rebind).
        // The receiver is not released (Zig emits no release here).
        if (call.func_name == "as_bytes" || call.func_name == "bytes")
            && call.args.len() == 1
            && call.callee.is_none()
        {
            let macro_name = match self.expr_type(&call.args[0]) {
                Some(ty) => match peel_borrow_pointer(&ty) {
                    Type::UserDefined(u) if u.name == "String" => {
                        "STRING_BUF_AS_BYTES"
                    }
                    Type::Primitive(Primitive::RawPtr) | Type::Array(_) => {
                        "STR_AS_BYTES"
                    }
                    _ => return Ok(None),
                },
                None => return Ok(None),
            };
            let recv = self.gen_expr(&call.args[0])?;
            let reg = self.new_tmp();
            self.ins(&format!("EXPAND {macro_name} {reg}, {recv}"));
            return Ok(Some(SurfaceCall::Value(reg)));
        }
        // ---- Option closure methods (codegen.zig:14400
        // planOptionClosureCall arms): map / and_then /
        // unwrap_or_else expand the closure argument inline
        // under the Some branch; the None branch rebuilds the
        // empty/default side. Placed ahead of the receiver
        // match because the closure argument is not a value
        // the generic arg planning can generate.
        if matches!(call.func_name.as_str(), "map" | "and_then" | "unwrap_or_else")
            && call.args.len() == 2
        {
            if let Some(lit) = Self::closure_literal_of(&call.args[1]).cloned() {
                let recv_ty = self.expr_type(&call.args[0]);
                let inner = recv_ty.as_ref().and_then(|ty| {
                    match peel_name(ty) {
                        Some((name, generics)) if name == "Option" && generics.len() == 1 => {
                            Some(generics[0].clone())
                        }
                        _ => None,
                    }
                });
                if let Some(inner_ty) = inner {
                    let recv = self.gen_expr(&call.args[0])?;
                    let is_some = self.new_tmp();
                    self.ins(&format!("EXPAND OPTION_IS_SOME {is_some}, {recv}"));
                    let kind = call.func_name.as_str();
                    let (some_label, none_label, end_label) = match kind {
                        "map" => (
                            self.new_label("L_OPTION_MAP_SOME"),
                            self.new_label("L_OPTION_MAP_NONE"),
                            self.new_label("L_OPTION_MAP_END"),
                        ),
                        "and_then" => (
                            self.new_label("L_OPTION_AND_THEN_SOME"),
                            self.new_label("L_OPTION_AND_THEN_NONE"),
                            self.new_label("L_OPTION_AND_THEN_END"),
                        ),
                        _ => (
                            self.new_label("L_OPTION_UNWRAP_OR_ELSE_SOME"),
                            self.new_label("L_OPTION_UNWRAP_OR_ELSE_NONE"),
                            self.new_label("L_OPTION_UNWRAP_OR_ELSE_END"),
                        ),
                    };
                    if kind == "map" {
                        let result = self.new_tmp();
                        self.w(&format!(
                            "    br {is_some} -> {some_label}, {none_label}\n\n"
                        ));
                        self.wl(&format!("{some_label}:"));
                        self.ins(&format!("!{is_some}"));
                        let value = self.new_tmp();
                        self.ins(&format!("EXPAND OPTION_GET {value}, {recv}"));
                        let mapped = self.gen_inline_closure_unary(&lit, &value)?;
                        self.ins(&format!("EXPAND OPTION_NEW_SOME {result}, {mapped}"));
                        self.emit_release(&value)?;
                        if mapped != value {
                            self.emit_release(&mapped)?;
                        }
                        self.w(&format!("    jmp {end_label}\n\n"));
                        self.wl(&format!("{none_label}:"));
                        self.ins(&format!("!{is_some}"));
                        self.ins(&format!("EXPAND OPTION_NEW_NONE {result}"));
                        self.w(&format!("    jmp {end_label}\n\n"));
                        self.wl(&format!("{end_label}:"));
                        if sla_lowering::call_arg_needs_release(&call.args[0]) {
                            self.emit_release(&recv)?;
                        }
                        return Ok(Some(SurfaceCall::Value(result)));
                    }
                    // and_then / unwrap_or_else merge through a
                    // result slot (the two arms produce values
                    // of the same type from different shapes).
                    let slot = self.new_tmp();
                    self.ins(&format!("{slot} = stack_alloc 8"));
                    self.w(&format!(
                        "    br {is_some} -> {some_label}, {none_label}\n\n"
                    ));
                    self.wl(&format!("{some_label}:"));
                    self.ins(&format!("!{is_some}"));
                    let value = self.new_tmp();
                    self.ins(&format!("EXPAND OPTION_GET {value}, {recv}"));
                    if kind == "and_then" {
                        let chained = self.gen_inline_closure_unary(&lit, &value)?;
                        self.ins(&format!("store {slot}+0, {chained} as ptr"));
                        self.store_refcell_handle_companion_for_slot(&slot, &chained);
                        // LINE-CENSUS-MEMLEAK: the closure result
                        // temp now lives in the slot; retire it
                        // (when it is the OPTION_GET value itself,
                        // the release just below settles it).
                        // Without this, `Some(..)` built inside
                        // the closure stranded its temp at
                        // function exit (MemoryLeak 1012,
                        // test_unit_closure_forms_minim /
                        // test_unit_option_methods).
                        if chained != value
                            && sla_lowering::is_temporary_register_name(&chained)
                        {
                            self.emit_release(&chained)?;
                        }
                        self.emit_release(&value)?;
                        self.w(&format!("    jmp {end_label}\n\n"));
                        self.wl(&format!("{none_label}:"));
                        self.ins(&format!("!{is_some}"));
                        let none_reg = self.new_tmp();
                        self.ins(&format!("EXPAND OPTION_NEW_NONE {none_reg}"));
                        self.ins(&format!("store {slot}+0, {none_reg} as ptr"));
                        self.store_refcell_handle_companion_for_slot(&slot, &none_reg);
                        // LINE-CENSUS-MEMLEAK: same slot-store
                        // settle for the None arm's fresh temp.
                        self.emit_release(&none_reg)?;
                        self.w(&format!("    jmp {end_label}\n\n"));
                        self.wl(&format!("{end_label}:"));
                        let result = self.new_tmp();
                        self.ins(&format!("{result} = load {slot}+0 as ptr"));
                        if sla_lowering::call_arg_needs_release(&call.args[0]) {
                            self.emit_release(&recv)?;
                        }
                        return Ok(Some(SurfaceCall::Value(result)));
                    }
                    // unwrap_or_else
                    let ty_str = crate::codegen::type_string(&inner_ty);
                    self.ins(&format!("store {slot}+0, {value} as {ty_str}"));
                    self.store_refcell_handle_companion_for_slot(&slot, &value);
                    // LINE-CENSUS-MEMLEAK: the OPTION_GET temp
                    // now lives in the slot; retire it (same
                    // slot-store settle as the and_then arms).
                    self.emit_release(&value)?;
                    self.w(&format!("    jmp {end_label}\n\n"));
                    self.wl(&format!("{none_label}:"));
                    self.ins(&format!("!{is_some}"));
                    let default = self.gen_inline_closure_nullary(&lit)?;
                    self.ins(&format!("store {slot}+0, {default} as {ty_str}"));
                    self.store_refcell_handle_companion_for_slot(&slot, &default);
                    // LINE-CENSUS-MEMLEAK: same settle for the
                    // default arm's closure result temp.
                    if sla_lowering::is_temporary_register_name(&default) {
                        self.emit_release(&default)?;
                    }
                    self.w(&format!("    jmp {end_label}\n\n"));
                    self.wl(&format!("{end_label}:"));
                    let reg = self.new_tmp();
                    self.ins(&format!("{reg} = load {slot}+0 as {ty_str}"));
                    if sla_lowering::call_arg_needs_release(&call.args[0]) {
                        self.emit_release(&recv)?;
                    }
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
            }
        }
        // ---- receiver methods ----
        if call.args.is_empty() || call.callee.is_some() {
            return Ok(None);
        }
        let (tname, generics) = match self.classify_receiver(call) {
            Some(pair) => pair,
            None => return Ok(None),
        };
        // L4R2 Root S: user-function shadowing. The checker's
        // bare-name cascade resolves user functions (stage 5)
        // BEFORE the std receiver leaves, so a user function
        // whose name matches a surface method and whose first
        // parameter is the receiver type wins the call
        // (test_unit_def_sab_surface_get_shadow: `get(a)` on an
        // Arc<i32> is the user's `fn get(a: Arc<i32>)`, not the
        // ARC_GET projection). This dispatch keyed on (receiver
        // type, name) only and silently substituted the
        // projection; the argument binding was then never
        // consumed and leaked at the caller's exit. Decline the
        // surface claim when such a user function exists — the
        // plain-call path binds exactly the decl consulted here.
        if self.user_fn_shadows_surface(call, &tname, &generics) {
            return Ok(None);
        }
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
            // FUSED pop().unwrap(): try the fusion first; it returns
            // None unless the receiver is a direct Vec pop (see
            // try_gen_pop_unwrap_fused), in which case the normal
            // OPTION_UNWRAP below never runs.
            if macro_name == "OPTION_UNWRAP" {
                if let Some(fused) = self.try_gen_pop_unwrap_fused(call)? {
                    return Ok(Some(fused));
                }
            }
            let (recv, _, _) = self.surface_receiver(call)?;
            let reg = self.new_tmp();
            self.ins(&format!("EXPAND {macro_name} {reg}, {recv}"));
            // Same receiver settle as the free-len arm: a
            // call-result Option/Result temp read by a unary
            // projection otherwise stays Active at function exit
            // (MemoryLeak, LINE-REG3 S2).
            if sla_lowering::call_arg_needs_release(&call.args[0]) {
                self.emit_release(&recv)?;
            }
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
                // Task 137 (P1, R908) as reworked by task 144 (shape
                // (b)): a stack-allocated Vec header cannot be passed
                // by move (host StackEscape, trap 1025) — push it by
                // borrow (`VEC_PUSH_STACK_B`), handing the old buffer
                // back to the caller for the free.
                // `recv` is the binding's own register when the
                // receiver is a direct `let` binding (the only shape
                // the analysis approves); any other receiver shape is
                // not in `stack_header_bindings` and keeps the move
                // form. GrowVec bindings are never stack headers.
                // The inline-eligibility check below still keys on
                // `macro_name` ("VEC_PUSH"); `push_macro` only
                // selects which macro the slow path expands.
                let push_macro = if tname == "Vec"
                    && self.stack_header_bindings.contains(&recv)
                {
                    "VEC_PUSH_STACK_B"
                } else {
                    macro_name
                };
                // CLASSB-DEF1: a copy-like element pushed from a
                // live binding stores an independent copy; the
                // source stays usable and owning.
                if let Some(copy_reg) = self.push_arg_copy_materialization(call, &elem)? {
                    self.ins(&format!(
                        "EXPAND {push_macro} {recv}, {copy_reg}, {}",
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
                if macro_name == "VEC_PUSH" && scalar_vec_inline_eligible(&elem) {
                    // Task 106: inline scalar push -- capacity check +
                    // direct slot store on the fast path, runtime call on
                    // the grow path. The helper leaves the insertion point
                    // at the END label; the value-finish step below is
                    // shared by both paths, exactly as the call path did
                    // it. GrowVec keeps the macro (different growth
                    // semantics -- not inlined).
                    let ok_reg = self.new_tmp();
                    let fast_label = self.new_label("L_VEC_PUSH_FAST");
                    let slow_label = self.new_label("L_VEC_PUSH_SLOW");
                    let end_label = self.new_label("L_VEC_PUSH_END");
                    self.emit_inline_push_scalar(
                        &recv, &elem, "Vec_len", "Vec_cap", "Vec_ptr", &value, push_macro,
                        &ok_reg, &fast_label, &slow_label, &end_label,
                    )?;
                    self.surface_finish_value(&planned, true)?;
                    return Ok(Some(SurfaceCall::Void));
                }
                self.ins(&format!(
                    "EXPAND {push_macro} {recv}, {value}, {}",
                    slot_size(&elem)
                ));
                self.surface_finish_value(&planned, true)?;
                Ok(Some(SurfaceCall::Void))
            }
            ("VecDeque", "push_back") => {
                // Mirror of the ("Vec", "push") arm: a copy-like element
                // pushed from a live binding stores an independent copy;
                // the source stays usable and owning. The deque slot
                // holds the record address itself (the VEC_DEQUE_*
                // slot contract, DEF-VECDEQUE-STRUCT-ELEM), so the macro
                // takes no slot_size operand — `sa_vec_deque_push_back`
                // stores the value word as the slot.
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let elem = generics
                    .first()
                    .ok_or(missing_fact())?
                    .clone();
                if let Some(copy_reg) = self.push_arg_copy_materialization(call, &elem)? {
                    self.ins(&format!(
                        "EXPAND VEC_DEQUE_PUSH_BACK {recv}, {copy_reg}"
                    ));
                    // The copy's ownership moves into the deque:
                    // forget it, never release (a release would
                    // free the record the new slot references).
                    self.emit_forget_moved_value(&copy_reg)?;
                    return Ok(Some(SurfaceCall::Void));
                }
                let planned = self.surface_value_consumed(call, 1)?;
                let value = planned.reg.clone();
                self.ins(&format!(
                    "EXPAND VEC_DEQUE_PUSH_BACK {recv}, {value}"
                ));
                self.surface_finish_value(&planned, true)?;
                Ok(Some(SurfaceCall::Void))
            }
            ("Vec", "pop") => {
                // codegen.zig isPopCall arm: try_pop into a
                // one-word slot, then wrap the word in an Option.
                // The popped word is the element value as stored:
                // for a struct element it is the record pointer
                // itself (VEC_PUSH stored it, and the VEC_REMOVE
                // arm likewise returns its word as the record
                // pointer). The Zig arm materializes an inline-POD
                // word into a fresh record cell
                // (materializeInlinePodRecord) because Zig's
                // VEC_PUSH stores the packed record image; under
                // this compiler's storage model that would build a
                // record whose first word is the pointer's own
                // bits (DEF-RUST-CLASSB-FIELDWRITE-STORAGE-MODEL:
                // popped elements read back as garbage).
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let elem = generics
                    .first()
                    .ok_or_else(|| missing_fact())?
                    .clone();
                let ok_reg = self.new_tmp();
                let value_reg = self.new_tmp();
                let value_slot = self.new_tmp();
                let option_reg = self.new_tmp();
                let some_label = self.new_label("L_VEC_POP_SOME");
                let none_label = self.new_label("L_VEC_POP_NONE");
                let end_label = self.new_label("L_VEC_POP_END");
        if scalar_vec_inline_eligible(&elem) {
            // Task 101: inline scalar pop -- no call, no
            // out-slot. The helper leaves the insertion point at
            // the SOME label with value_reg holding the popped
            // value; wrap it in Some below. NOTE: no
            // `!{value_reg}` in the NONE arm -- unlike the call
            // path, value_reg is only defined on the SOME path
            // here (the slot load runs after the branch).
            self.emit_inline_pop_scalar(
                &recv, &elem, "Vec_len", "Vec_ptr", &ok_reg, &value_reg, &some_label,
                &none_label,
            )?;
            self.ins(&format!(
                "EXPAND OPTION_NEW_SOME {option_reg}, {value_reg}"
            ));
            self.w(&format!("    !{value_reg}\n"));
            self.w(&format!("    jmp {end_label}\n\n"));
            self.wl(&format!("{none_label}:"));
            self.w(&format!("    !{ok_reg}\n"));
            self.ins(&format!("EXPAND OPTION_NEW_NONE {option_reg}"));
            self.w(&format!("    jmp {end_label}\n\n"));
            self.wl(&format!("{end_label}:"));
            if sla_lowering::call_arg_needs_release(&call.args[0]) {
                self.emit_release(&recv)?;
            }
            return Ok(Some(SurfaceCall::Value(option_reg)));
        }
                self.ins(&format!("{value_slot} = alloc 8"));
                self.ins(&format!(
                    "{ok_reg} = call @sa_vec_try_pop_sized(&{recv}, &{value_slot}, {})",
                    vec_element_slot_size(&elem)
                ));
                // The sized entry zeroes the slot and moves the
                // element at its own width, so the slot holds the
                // width-exact value: load it as the element type,
                // exactly like the Vec index-load path (expr.rs),
                // so a narrow signed element comes back
                // sign-extended instead of zero-extended.
                let load_ty = if matches!(elem, Type::Primitive(_)) {
                    crate::codegen::type_string(&elem)
                } else {
                    "ptr"
                };
                self.ins(&format!(
                    "{value_reg} = load {value_slot}+0 as {load_ty}"
                ));
                self.emit_release(&value_slot)?;
                self.w(&format!(
                    "    br {ok_reg} -> {some_label}, {none_label}\n\n"
                ));
                self.wl(&format!("{some_label}:"));
                self.w(&format!("    !{ok_reg}\n"));
                self.ins(&format!(
                    "EXPAND OPTION_NEW_SOME {option_reg}, {value_reg}"
                ));
                self.w(&format!("    !{value_reg}\n"));
                self.w(&format!("    jmp {end_label}\n\n"));
                self.wl(&format!("{none_label}:"));
                self.w(&format!("    !{ok_reg}\n"));
                self.w(&format!("    !{value_reg}\n"));
                self.ins(&format!("EXPAND OPTION_NEW_NONE {option_reg}"));
                self.w(&format!("    jmp {end_label}\n\n"));
                self.wl(&format!("{end_label}:"));
                if sla_lowering::call_arg_needs_release(&call.args[0]) {
                    self.emit_release(&recv)?;
                }
                Ok(Some(SurfaceCall::Value(option_reg)))
            }
            ("VecDeque", "pop_front") => {
                // Zig codegen.zig `isPopFrontCall` arm: the
                // VEC_DEQUE_TRY_POP_FRONT macro carries the slot
                // alloc + try call + load internally, so the arm
                // expands it directly (no slot/call of its own) and
                // wraps the macro's out value — the deque slot holds
                // the element's record address itself — in an Option
                // as-is. No inline-POD materialization: the value
                // word IS the record cell, not a packed image (that
                // is the Vec pop arm's shape, not this one).
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let ok_reg = self.new_tmp();
                let value_reg = self.new_tmp();
                let option_reg = self.new_tmp();
                let some_label = self.new_label("L_VEC_DEQUE_POP_FRONT_SOME");
                let none_label = self.new_label("L_VEC_DEQUE_POP_FRONT_NONE");
                let end_label = self.new_label("L_VEC_DEQUE_POP_FRONT_END");
                self.ins(&format!(
                    "EXPAND VEC_DEQUE_TRY_POP_FRONT {ok_reg}, {value_reg}, {recv}"
                ));
                self.w(&format!(
                    "    br {ok_reg} -> {some_label}, {none_label}\n\n"
                ));
                self.wl(&format!("{some_label}:"));
                self.w(&format!("    !{ok_reg}\n"));
                self.ins(&format!(
                    "EXPAND OPTION_NEW_SOME {option_reg}, {value_reg}"
                ));
                self.w(&format!("    !{value_reg}\n"));
                self.w(&format!("    jmp {end_label}\n\n"));
                self.wl(&format!("{none_label}:"));
                self.w(&format!("    !{ok_reg}\n"));
                self.w(&format!("    !{value_reg}\n"));
                self.ins(&format!("EXPAND OPTION_NEW_NONE {option_reg}"));
                self.w(&format!("    jmp {end_label}\n\n"));
                self.wl(&format!("{end_label}:"));
                if sla_lowering::call_arg_needs_release(&call.args[0]) {
                    self.emit_release(&recv)?;
                }
                Ok(Some(SurfaceCall::Value(option_reg)))
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
            ("HashSet", "remove") => {
                // DEF-RUST-STRKEY (std_surface.sla_meta: `method
                // HashSet remove sa_std/hashset.sa SET_REMOVE
                // out,receiver,value deps=sa_set_remove,sa_map_del`).
                // The checker half already landed (check_std_leaf3:
                // the key is a probe, NOT consumed, result bool —
                // as with contains); this is the missing codegen
                // arm. Without it the call fell to the EXPAND
                // catch-all (`EXPAND remove`, host UndefinedMacro:
                // test_unit_def_rust_set_remove_str). String keys
                // key on CONTENT via @sa_set_remove_str (the
                // SET_REMOVE_STR dep chain), mirroring the
                // insert/contains str path above.
                if call.args.len() != 2 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let elem_ty = generics.first().cloned();
                let str_elem = elem_ty
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
                            let planned = self.surface_value(call, 1)?;
                            let reg = planned.reg.clone();
                            (reg, Some(planned), false)
                        }
                    };
                    let reg = self.new_tmp();
                    self.ins(&format!(
                        "{reg} = call @sa_set_remove_str(&{recv}, &{value_reg})"
                    ));
                    if let Some(planned) = &planned {
                        self.surface_finish_value(planned, false)?;
                    }
                    if owned_literal {
                        // A probe does not adopt its key (same
                        // as the HashMap get arm).
                        self.emit_release(&value_reg)?;
                    }
                    return Ok(Some(SurfaceCall::Value(reg)));
                }
                let planned = self.surface_value(call, 1)?;
                let value = planned.reg.clone();
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND SET_REMOVE {reg}, {recv}, {value}"));
                self.surface_finish_value(&planned, false)?;
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
            ("RefCell", "borrow") | ("RefCell", "borrow_mut") => {
                // codegen.zig:22461 `planRefCellBorrowCall` arm:
                // the try-borrow expansion writes a status and
                // the borrow slot; the status branches to a
                // panic block (code 107) on conflict and is
                // retired on both arms. The result is the slot
                // itself for a scalar payload, or the pointer
                // stored in the slot for a pointer / smart
                // pointer payload (the slot register is released
                // right after the load). The result register is
                // registered as a borrow handle so its cleanup
                // settles the dynamic borrow (release.rs).
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let (recv, _, _) = self.surface_receiver(call)?;
                let inner = generics.first().ok_or(missing_fact())?.clone();
                let kind = if name == "borrow" {
                    sla_lowering::refcell::RefCellBorrowKind::Shared
                } else {
                    sla_lowering::refcell::RefCellBorrowKind::Mutable
                };
                let value_kind =
                    sla_lowering::refcell::plan_refcell_borrow_value_kind(&inner);
                let guard = sla_lowering::refcell::plan_refcell_borrow_runtime_guard();
                let ok_reg = self.new_tmp();
                let slot_reg = self.new_tmp();
                let err_label = self.new_label("L_REFCELL_BORROW_PANIC");
                let end_label = self.new_label("L_REFCELL_BORROW_END");
                self.ins(&format!(
                    "EXPAND {} {ok_reg}, {slot_reg}, {recv}",
                    kind.try_borrow_macro_name()
                ));
                self.ins(&format!("br {ok_reg} -> {end_label}, {err_label}"));
                self.wl(&format!("{err_label}:"));
                if guard.release_status_on_conflict {
                    self.ins(&format!("!{ok_reg}"));
                }
                self.ins(&format!("panic({})", guard.conflict_panic_code));
                self.wl(&format!("{end_label}:"));
                if guard.release_status_on_success {
                    self.ins(&format!("!{ok_reg}"));
                }
                let (action, release_slot) =
                    sla_lowering::refcell::plan_refcell_borrow_result_sa_text(value_kind);
                let borrow_reg = match action {
                    sla_lowering::refcell::RefCellBorrowResultAction::UseBorrowSlot => {
                        slot_reg.clone()
                    }
                    sla_lowering::refcell::RefCellBorrowResultAction::LoadPointerPayload => {
                        let payload_reg = self.new_tmp();
                        self.ins(&format!("{payload_reg} = load {slot_reg}+0 as ptr"));
                        if release_slot {
                            self.emit_release(&slot_reg)?;
                        }
                        payload_reg
                    }
                };
                self.refcell_borrow_handles.insert(
                    borrow_reg.clone(),
                    crate::codegen::RefCellBorrowHandle {
                        cell_reg: recv,
                        kind,
                        cell_release_temp: None,
                        // DEF-RUST-REFCELL-BRANCH-HANDLE-MERGE
                        // (task 21): a scalar-slot borrow returns
                        // the slot register itself, which IS the
                        // cell (`ptr_add cell, +0`); a pointer
                        // payload loads the payload out of the
                        // slot, so the handle is not the cell.
                        handle_is_cell: matches!(
                            action,
                            sla_lowering::refcell::RefCellBorrowResultAction::UseBorrowSlot
                        ),
                    },
                );
                Ok(Some(SurfaceCall::Value(borrow_reg)))
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
            ("RwLock", "read") | ("RwLock", "write") => {
                // codegen.zig:17263-17295: a try-acquire with an
                // ok flag — the Err arm builds
                // `RESULT_NEW_ERR result, 1`, the Ok arm wraps
                // the `RwLock_data` pointer in `RESULT_NEW_OK`.
                // The Result register is recorded in
                // `rwlock_lock_results` so `unwrap` can transfer
                // it to the guard map and `emitRelease` can
                // release the lock (codegen.zig:1084). Without
                // this arm the call fell to the EXPAND catch-all
                // (`EXPAND read`, host UndefinedMacro:
                // test_unit_p1_rwlock_new).
                if call.args.len() != 1 {
                    return Err(missing_fact());
                }
                let is_write = name == "write";
                let (recv, _, _) = self.surface_receiver(call)?;
                let ok_reg = self.new_tmp();
                let lock_guard_reg = self.new_tmp();
                let guard_reg = self.new_tmp();
                let result_reg = self.new_tmp();
                let err_label = self.new_label("L_RWLOCK_RESULT_ERR");
                let ok_label = self.new_label("L_RWLOCK_RESULT_OK");
                let end_label = self.new_label("L_RWLOCK_RESULT_END");
                let try_macro = if is_write {
                    "RWLOCK_TRY_WRITE_NOERR"
                } else {
                    "RWLOCK_TRY_READ_NOERR"
                };
                self.ins(&format!(
                    "EXPAND {try_macro} {ok_reg}, {lock_guard_reg}, {recv}"
                ));
                self.ins(&format!("br {ok_reg} -> {ok_label}, {err_label}"));
                self.wl(&format!("{err_label}:"));
                self.ins(&format!("EXPAND RESULT_NEW_ERR {result_reg}, 1"));
                self.ins(&format!("jmp {end_label}"));
                self.wl(&format!("{ok_label}:"));
                self.ins(&format!("{guard_reg} = ptr_add {recv}, RwLock_data"));
                self.ins(&format!(
                    "EXPAND RESULT_NEW_OK {result_reg}, {guard_reg}"
                ));
                self.emit_release(&guard_reg)?;
                self.ins(&format!("jmp {end_label}"));
                self.wl(&format!("{end_label}:"));
                self.rwlock_lock_results.insert(
                    result_reg.clone(),
                    crate::codegen::RwLockGuardHandle {
                        lock_reg: recv.clone(),
                        is_write,
                    },
                );
                self.emit_release(&ok_reg)?;
                self.emit_release(&lock_guard_reg)?;
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
                // Same transfer for an `RwLock::read` / `write`
                // Result (codegen.zig:21373 family).
                if let Some(handle) = self.rwlock_lock_results.remove(&recv) {
                    self.rwlock_guard_handles.insert(reg.clone(), handle);
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

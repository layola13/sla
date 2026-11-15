//! Extern-contract calls and imported-macro calls, mirrored
//! from codegen.zig:3342 `genExternPayloadCall`, 13968
//! `genImportedMacroCall` and 13471 `genImportedMacroArg`.
//! Split out of call.rs for the per-file line cap.

use sla_ast::{Node, Param, Primitive, Type};
use sla_lowering::{ExternSigView, ImportedMacroView};

use crate::call::{abi_type_from_spelling, LoweredCallArg};
use crate::codegen::Codegen;
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// Prefix a display register with a capability marker unless
    /// it already carries one (Zig `abiPrefixedArg`).
    fn prefix_cap(reg: &str, cap: char) -> String {
        if reg.starts_with('&') || reg.starts_with('^') {
            reg.to_string()
        } else {
            format!("{cap}{reg}")
        }
    }

    /// Zig `abiRawPrefixedArg`: a raw-capability (`*`-marked)
    /// extern parameter keeps the same register; a borrow/move
    /// annotation on the operand is REPLACED by the raw `*`
    /// annotation the SA contract expects.
    fn raw_prefixed_arg(reg: &str) -> String {
        if let Some(rest) = reg.strip_prefix(['&', '^']) {
            format!("*{rest}")
        } else {
            Self::prefix_cap(reg, '*')
        }
    }

    /// Shared extern argument planning (Zig `genExternPayloadCall`
    /// arg loop): each argument is planned against the contract
    /// parameter (the ABI type becomes the AST param type, as in
    /// `externPtrParamAsAstParam`), then the contract capability
    /// prefix is applied to the display register (`abiCallArgPrefix`:
    /// borrow `&`, move `^`). Cleanups keep the unprefixed planned
    /// registers, as in the Zig cleanup split.
    fn plan_extern_call_args(
        &mut self,
        call: &sla_ast::CallExpr,
        sig: &ExternSigView,
    ) -> Result<(Vec<String>, Vec<LoweredCallArg>)> {
        let mut planned_args: Vec<LoweredCallArg> = Vec::new();
        let mut display: Vec<String> = Vec::new();
        for (i, arg) in call.args.iter().enumerate() {
            let view = sig.params.get(i);
            let param = view.map(|p| Param {
                name: p.name.clone(),
                ty: Box::new(abi_type_from_spelling(&p.abi_ty)),
                is_borrow: p.is_borrow,
                is_move: p.is_move,
                fallible_sugar: false,
                is_var: false,
            });
            let planned = self.gen_planned_call_arg(arg, param.as_ref(), false)?;
            let mut planned = planned;
            // Zig `appendExternLoweredCallArgCleanups`
            // (codegen.zig:3913 in the box tree): for an extern
            // is_move parameter the call itself consumes the
            // argument — force release_after_call=false and
            // consume the display register, so the cleanup phase
            // marks it consumed instead of releasing the
            // already-moved register (the old plan released it and
            // the next `!reg` trapped UseAfterMove:
            // test_unit_fallible_result_direct,
            // test_unit_reg1_dns_client_a). When the planner
            // already moved the argument (its reg carries `^` and
            // consume_reg is set) the plan is left alone.
            if view.map(|p| p.is_move).unwrap_or(false)
                && !planned.reg.starts_with('^')
                && !planned.reg.starts_with('&')
            {
                planned.release_after_call = false;
                planned.release_reg = None;
                planned.consume_reg = Some(format!("^{}", planned.reg));
            }
            let reg = match view {
                Some(p) if p.is_raw => Self::raw_prefixed_arg(&planned.reg),
                Some(p) if p.is_borrow => Self::prefix_cap(&planned.reg, '&'),
                Some(p) if p.is_move => Self::prefix_cap(&planned.reg, '^'),
                _ => planned.reg.clone(),
            };
            display.push(reg);
            planned_args.push(planned);
        }
        Ok((display, planned_args))
    }

    /// codegen.zig:3342 `genExternPayloadCall`. Before this arm
    /// existed, expression-position extern calls fell into the
    /// EXPAND catch-all below and the host rejected them with
    /// error[UndefinedMacro] (14 corpus files). Fallible externs
    /// (`T!` returns) go through `gen_fallible_extern_result_call`
    /// instead: the runtime returns a `Fallible(T)` record pointer
    /// (status i32 at +0, payload after), not a `Result` record.
    pub(crate) fn gen_extern_call(
        &mut self,
        call: &sla_ast::CallExpr,
        sig: &ExternSigView,
    ) -> Result<String> {
        if sig.fallible {
            return self.gen_fallible_extern_result_call(call, sig);
        }
        let (display, planned_args) = self.plan_extern_call_args(call, sig)?;
        let result = self.new_tmp();
        let lowered = self.lowered_func_symbol(&call.func_name);
        self.ins(&format!(
            "{result} = call @{lowered}({})",
            display.join(", ")
        ));
        self.emit_lowered_call_arg_cleanups(&planned_args, Some(&call.func_name))?;
        Ok(result)
    }

    /// codegen.zig:3255 `genFallibleExternResultCall` (A_min
    /// fallible `T!` batch): a fallible extern call materializes
    /// its cell into a `Result<T, i32>` value. The status word at
    /// offset 0 selects the variant; the payload is loaded only on
    /// the Ok arm (on the Err arm it is not a value), and the
    /// status itself becomes the Err payload. Without this arm the
    /// raw `Fallible(T)` pointer was bound as a `Result` record
    /// and the match read its payload past the end of the struct
    /// (test_unit_fallible_result_direct panicked 45141 with a
    /// garbage Ok payload; test_unit_reg1_dns_client_a trapped
    /// StackEscape the same way). Branch/temp discipline mirrors
    /// File::open's RESULT_NEW_OK / RESULT_NEW_ERR emission.
    fn gen_fallible_extern_result_call(
        &mut self,
        call: &sla_ast::CallExpr,
        sig: &ExternSigView,
    ) -> Result<String> {
        if !Self::abi_fallible_payload_supported(&sig.ret_abi) {
            return crate::unsupported(
                "fallible extern payload outside the scalar set",
                "lowering_rules.zig:1505 (abiFalliblePayloadSupported)",
            );
        }
        let (display, planned_args) = self.plan_extern_call_args(call, sig)?;
        let fallible_reg = self.new_tmp();
        let result = self.new_tmp();
        let lowered = self.lowered_func_symbol(&call.func_name);
        self.ins(&format!(
            "{fallible_reg} = call @{lowered}({})",
            display.join(", ")
        ));
        self.emit_lowered_call_arg_cleanups(&planned_args, Some(&call.func_name))?;
        let status_reg = self.new_tmp();
        self.ins(&format!(
            "{status_reg} = load {fallible_reg}+0 as i32"
        ));
        let ok_reg = self.new_tmp();
        self.ins(&format!("{ok_reg} = eq {status_reg}, 0"));
        let ok_label = self.new_label("L_FALLIBLE_OK");
        let err_label = self.new_label("L_FALLIBLE_ERR");
        let end_label = self.new_label("L_FALLIBLE_END");
        self.w(&format!(
            "    br {ok_reg} -> {ok_label}, {err_label}\n\n"
        ));
        self.wl(&format!("{ok_label}:"));
        let payload_reg = self.new_tmp();
        self.ins(&format!(
            "{payload_reg} = load {fallible_reg}+{} as {}",
            Self::abi_fallible_payload_offset(&sig.ret_abi),
            Self::abi_raw_payload_type_string(&sig.ret_abi),
        ));
        self.ins(&format!(
            "EXPAND RESULT_NEW_OK {result}, {payload_reg}"
        ));
        self.w(&format!("    !{status_reg}\n    !{payload_reg}\n"));
        self.w(&format!("    jmp {end_label}\n\n"));
        self.wl(&format!("{err_label}:"));
        self.ins(&format!(
            "EXPAND RESULT_NEW_ERR {result}, {status_reg}"
        ));
        self.w(&format!("    !{status_reg}\n"));
        self.w(&format!("    jmp {end_label}\n\n"));
        self.wl(&format!("{end_label}:"));
        self.emit_release(&ok_reg)?;
        self.emit_release(&fallible_reg)?;
        Ok(result)
    }

    /// Statement position (the DEF-10 shape): a void extern emits
    /// a bare call with the contract prefixes intact; a
    /// value-returning extern goes through the expression path and
    /// its discarded result temp is released like a discarded
    /// surface value. Before this arm existed, statement-position
    /// extern calls hit the gen_call_stmt fallthrough.
    pub(crate) fn gen_extern_call_stmt(
        &mut self,
        expr: &Node,
        call: &sla_ast::CallExpr,
        sig: &ExternSigView,
    ) -> Result<()> {
        // A fallible extern in statement position still
        // materializes its Result cell (Zig genCallStmt skips the
        // DEF-10 void block for return_fallible); the discarded
        // Result is released like any discarded surface value.
        if sig.fallible {
            let reg = self.gen_extern_call(call, sig)?;
            if self.expr_result_reg_needs_release(expr) {
                self.emit_release(&reg)?;
            }
            return Ok(());
        }
        let ret = sig
            .ret_abi
            .trim()
            .trim_start_matches(['&', '^', '*'])
            .trim_end_matches('!')
            .trim();
        if ret.is_empty() || ret == "void" {
            let (display, planned_args) = self.plan_extern_call_args(call, sig)?;
            let lowered = self.lowered_func_symbol(&call.func_name);
            self.ins(&format!("call @{lowered}({})", display.join(", ")));
            self.emit_lowered_call_arg_cleanups(&planned_args, Some(&call.func_name))?;
            return Ok(());
        }
        let reg = self.gen_extern_call(call, sig)?;
        if self.expr_result_reg_needs_release(expr) {
            self.emit_release(&reg)?;
        }
        Ok(())
    }

    /// lowering_rules.zig:3985 `abiFalliblePayloadOffset`: offset of
/// the payload word inside the runtime `Fallible(T)` record
/// (`status: i32` at +0, payload aligned after it: 8-byte payloads
/// at +8, 4-byte payloads at +4).
fn abi_fallible_payload_offset(ret_abi: &str) -> usize {
    let name = Self::abi_fallible_payload_name(ret_abi);
    let payload_align: usize = match name {
        "ptr" | "i64" | "u64" | "isize" | "usize" | "int" | "f64" | "float" => 8,
        _ => 4,
    };
    (4 + payload_align - 1) & !(payload_align - 1)
}

/// lowering_rules.zig:1505 `abiRawPayloadTypeString`: the SA load
/// type for a fallible payload spelling. Anything outside the
/// scalar set maps to `ptr` here; callers must gate on
/// `abi_fallible_payload_supported` first.
fn abi_raw_payload_type_string(ret_abi: &str) -> &'static str {
    match Self::abi_fallible_payload_name(ret_abi) {
        "ptr" => "ptr",
        "bool" => "u8",
        "i8" => "i8",
        "i16" => "i16",
        "i32" => "i32",
        "i64" | "int" | "isize" => "i64",
        "u8" => "u8",
        "u16" => "u16",
        "u32" => "u32",
        "u64" | "usize" => "u64",
        "f32" => "f32",
        "f64" | "float" => "f64",
        _ => "ptr",
    }
}

/// lowering_rules.zig `abiFalliblePayloadSupported`: the payload
/// scalar set the fallible ABI can carry. Anything else must be
/// rejected loudly at the contract boundary instead of being
/// silently treated as pointer-width.
fn abi_fallible_payload_supported(ret_abi: &str) -> bool {
    matches!(
        Self::abi_fallible_payload_name(ret_abi),
        "ptr" | "bool"
            | "i8" | "i16" | "i32" | "i64" | "int" | "isize"
            | "u8" | "u16" | "u32" | "u64" | "usize"
            | "f32" | "f64" | "float"
    )
}

/// Shared spelling normalization for the fallible helpers: strip
/// a leading capability marker (`&`/`^`/`*`), surrounding
/// whitespace, and a trailing `!`.
fn abi_fallible_payload_name(ret_abi: &str) -> &str {
    let mut name = ret_abi.trim();
    if let Some(rest) = name.strip_prefix(['&', '^', '*']) {
        name = rest.trim();
    }
    if let Some(rest) = name.strip_suffix('!') {
        name = rest.trim();
    }
    name
}

/// One imported-macro operand (Zig `genImportedMacroArg`,
    /// core subset): address-slot operands take the argument's
    /// address — a stack-slot binding passes the slot itself, any
    /// other identifier its `&name`; borrowed operands take
    /// `&reg`; everything else is the plain value register.
    /// Non-identifier address-slot arguments stay loud.
    fn gen_imported_macro_arg(
        &mut self,
        arg: &Node,
        view: &ImportedMacroView,
        operand_index: usize,
    ) -> Result<String> {
        let bit = 1u64 << operand_index;
        if view.address_slot_arg_mask & bit != 0 {
            return match arg {
                Node::Identifier(name) => {
                    let resolved = self.resolve_binding_name(name);
                    // A pointer-typed binding already holds the
                    // address the macro expects (e.g. PTR_READ_U8's
                    // %ptr_reg, whose body loads through it): pass
                    // it plainly. Prefixing it produced `load &p+0`
                    // and a host UnknownRegister.
                    let arg_ty = self.expr_type(arg);
                    if matches!(
                        arg_ty,
                        Some(
                            Type::Pointer(_)
                                | Type::Borrow(_)
                                | Type::Primitive(Primitive::RawPtr)
                        )
                    ) {
                        return Ok(resolved);
                    }
                    if self.stack_alloc_bindings.contains(&resolved) {
                        return Ok(resolved);
                    }
                    // Pointer-represented composites (Slice,
                    // String, structs, arrays, tuples): the
                    // register already holds the value's address.
                    if matches!(
                        arg_ty,
                        Some(Type::UserDefined(_) | Type::Array(_) | Type::Tuple(_))
                    ) {
                        return Ok(resolved);
                    }
                    // A scalar in a plain register has no address:
                    // materialise a stack slot for it (Zig's
                    // MaterializeStackSlot action).
                    let ty = arg_ty.ok_or_else(|| crate::TextError::MissingType {
                        context: "macro address-slot argument type".to_string(),
                    })?;
                    let slot = self.new_tmp();
                    self.ins(&format!(
                        "{slot} = stack_alloc {}",
                        crate::codegen::type_size(&ty)
                    ));
                    self.ins(&format!(
                        "store {slot}+0, {resolved} as {}",
                        crate::codegen::type_string(&ty)
                    ));
                    Ok(slot)
                }
                // Composite values are pointer-represented in
                // this codegen (a string literal's register is
                // its slice image's stack slot, a Vec/String is
                // its handle), so a non-identifier address-slot
                // operand's generated register already is the
                // address the macro expects — the materialised
                // form of Zig's MaterializeStackSlot for these
                // shapes.
                _ => self.gen_expr(arg),
            };
        }
        // A borrow of a slot-backed or pointer-represented
        // identifier passes the binding register itself (Zig
        // planAddressOf identifier shape — the register already
        // IS the value's address). Macro operands are never
        // released by the caller emission, so the ptr_add temp
        // the generic borrow path materialises would leak at
        // function exit (PTR_BYTE_ADD(&big, 0) in
        // test_smoke1_net).
        if let Node::BorrowExpr(b) = arg {
            if let Node::Identifier(name) = &*b.expr {
                let resolved = self.resolve_binding_name(name);
                let pointer_represented = matches!(
                    self.expr_type(&b.expr),
                    Some(
                        Type::Array(_)
                            | Type::Tuple(_)
                            | Type::UserDefined(_)
                            | Type::Pointer(_)
                            | Type::Borrow(_)
                    )
                );
                if self.stack_alloc_bindings.contains(&resolved) || pointer_represented {
                    return Ok(resolved);
                }
            }
        }
        // Borrowed operands (borrowed_arg_mask) pass their plain
        // register: the mask records that the macro does not
        // consume the value (cleanup bookkeeping in the Zig
        // plan), it does NOT prefix the operand with `&` — doing
        // so substituted `&p` into macro bodies that use the
        // parameter as a load base (`load &p+0`, rejected by the
        // host with UnknownRegister; regression caught against
        // test_unit_ptr_byte_add_read_type_sa).
        self.gen_expr(arg)
    }

    /// Expression position (Zig `genImportedMacroCall`): the
    /// macro's single leading output is a fresh result register
    /// and the call's arguments fill the remaining operands.
    /// Multi-output macros are statement-only and stay loud here.
    pub(crate) fn gen_imported_macro_call_expr(
        &mut self,
        call: &sla_ast::CallExpr,
        view: &ImportedMacroView,
    ) -> Result<String> {
        if view.leading_outputs != 1 || call.args.len() + view.leading_outputs != view.arity {
            return unsupported(
                "imported macro call (multi-output / arity shape)",
                "codegen.zig:13968 (genImportedMacroCall)",
            );
        }
        let result = self.new_tmp();
        let mut operands = vec![result.clone()];
        for (i, arg) in call.args.iter().enumerate() {
            operands.push(self.gen_imported_macro_arg(arg, view, view.leading_outputs + i)?);
        }
        self.ins(&format!(
            "EXPAND {} {}",
            call.func_name,
            operands.join(", ")
        ));
        Ok(result)
    }

    /// Statement position: the leading outputs are the call's own
    /// first arguments (bindings the macro writes through); the
    /// remaining arguments are ordinary macro operands.
    pub(crate) fn gen_imported_macro_call_stmt(
        &mut self,
        call: &sla_ast::CallExpr,
        view: &ImportedMacroView,
    ) -> Result<()> {
        if call.args.len() != view.arity {
            return unsupported(
                "imported macro statement call (arity shape)",
                "codegen.zig:13968 (genImportedMacroCall)",
            );
        }
        let mut operands: Vec<String> = Vec::new();
        for (i, arg) in call.args.iter().enumerate() {
            if i < view.leading_outputs {
                match arg {
                    Node::Identifier(name) => operands.push(self.resolve_binding_name(name)),
                    _ => {
                        return unsupported(
                            "imported macro statement output argument",
                            "codegen.zig:13968 (genImportedMacroCall)",
                        )
                    }
                }
            } else {
                operands.push(self.gen_imported_macro_arg(arg, view, i)?);
            }
        }
        self.ins(&format!(
            "EXPAND {} {}",
            call.func_name,
            operands.join(", ")
        ));
        Ok(())
    }

}

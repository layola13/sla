//! Call emission, mirrored from codegen.zig: `genExpr`'s call path
//! (14400-15600), `genCallStmt` (1868-2125), `genPlannedCallArg`
//! (9986), `genCallArgFromMaterializationPlan` (9860) and
//! `emitLoweredCallArgCleanups` (9820). The long builtin dispatch in
//! the Zig (closures, futures, thread spawn, method resolution,
//! hash/format helpers) is deferred family by family; the plain
//! known-function path, the panic path, the Option/Result
//! constructors and the unknown-name EXPAND fallback are ported.
//!
//! Two Zig behaviors are mirrored deliberately (see divergences):
//! the hard-coded `"sum"` release skip in the cleanup emitter, and
//! the catch-all that treats an unknown callee name as a macro
//! EXPAND rather than an error.

use sla_ast::{Literal, Node, Param, Primitive, Type, UserDefined};
use sla_lowering::{
    abi_call_arg_cap_kind, abi_param_needs_borrow_arg, call_arg_is_copy_struct_value,
    call_arg_needs_release, is_panic_builtin_name, is_println_call,
    is_temporary_register_name, plan_call_arg_materialization, plan_refcell_call_arg_lifecycle,
    plan_stack_slot_identifier_call_arg_temp, value_arg_transfers_ownership_from_param,
    AbiCapKind, CallArgMaterializationInput, CallArgMaterializationKind, CallArgTarget,
};

use crate::codegen::Codegen;
use crate::{unsupported, Result};

/// codegen.zig `LoweredCallArg`.
#[derive(Debug, Clone)]
pub(crate) struct LoweredCallArg {
    pub reg: String,
    pub release_after_call: bool,
    pub release_reg: Option<String>,
    pub consume_reg: Option<String>,
}

impl<'c> Codegen<'c> {
    /// codegen.zig `plannedCallArgReleaseReg`. The RefCell-borrow
    /// probe is always false in this lane (no handle tracking), so
    /// the lifecycle plan reduces to the release flag.
    pub(crate) fn planned_call_arg_release_reg(
        &self,
        planned: &LoweredCallArg,
    ) -> Option<String> {
        let candidate = match &planned.release_reg {
            Some(reg) => reg.clone(),
            None => {
                if planned.release_after_call {
                    planned.reg.clone()
                } else {
                    return None;
                }
            }
        };
        let plan = plan_refcell_call_arg_lifecycle(
            planned.release_reg.is_some() || planned.release_after_call,
            false,
        );
        if plan.should_release() {
            Some(candidate)
        } else {
            None
        }
    }

    /// codegen.zig:9820 `emitLoweredCallArgCleanups`: releases in
    /// argument order, then forgets in argument order — except that a
    /// callee literally named `sum` skips the release phase entirely
    /// (hard-coded special case in the Zig, mirrored deliberately).
    pub(crate) fn emit_lowered_call_arg_cleanups(
        &mut self,
        planned_args: &[LoweredCallArg],
        skip_release_name: Option<&str>,
    ) -> Result<()> {
        if skip_release_name != Some("sum") {
            for planned in planned_args {
                if let Some(reg) = self.planned_call_arg_release_reg(planned) {
                    self.emit_release(&reg)?;
                }
            }
        }
        for planned in planned_args {
            if let Some(reg) = &planned.consume_reg {
                let reg = reg.clone();
                self.emit_forget_moved_value(&reg)?;
            }
        }
        Ok(())
    }

    /// codegen.zig `identifierCallArgTempNeedsRelease`.
    fn identifier_call_arg_temp_needs_release(&self, arg: &Node, reg: &str) -> bool {
        match arg {
            Node::Identifier(name) => {
                is_temporary_register_name(reg)
                    && reg != name
                    && reg != self.resolve_binding_name(name)
            }
            _ => false,
        }
    }

    /// codegen.zig:9801 `callArgResultTempNeedsRelease`.
    pub(crate) fn call_arg_result_temp_needs_release(&self, arg: &Node, reg: &str) -> bool {
        self.expr_result_reg_needs_release(arg)
            || self.identifier_call_arg_temp_needs_release(arg, reg)
    }

    /// codegen.zig:9809 `callArgResultTempNeedsReleaseForParam`.
    fn call_arg_result_temp_needs_release_for_param(
        &self,
        arg: &Node,
        reg: &str,
        param: Option<&Param>,
    ) -> bool {
        let action = plan_stack_slot_identifier_call_arg_temp(
            param,
            self.identifier_call_arg_temp_needs_release(arg, reg),
        );
        action.releases_temp()
            || (action.is_keep() && self.call_arg_result_temp_needs_release(arg, reg))
    }

    /// codegen.zig `callArgResultTempNeedsConsumeForParam`.
    fn call_arg_result_temp_needs_consume_for_param(
        &self,
        arg: &Node,
        reg: &str,
        param: Option<&Param>,
    ) -> bool {
        plan_stack_slot_identifier_call_arg_temp(
            param,
            self.identifier_call_arg_temp_needs_release(arg, reg),
        )
        .consumes_temp()
    }


    /// codegen.zig:9860 `genCallArgFromMaterializationPlan`. The
    /// Value and AutoBorrow arms are ported; every other plan kind
    /// belongs to a deferred family and fails loudly.
    pub(crate) fn gen_call_arg_from_materialization_plan(
        &mut self,
        arg: &Node,
        param: Option<&Param>,
        plan: &sla_lowering::CallArgMaterializationPlan,
    ) -> Result<LoweredCallArg> {
        match plan.kind {
            CallArgMaterializationKind::Value => {
                let raw_reg = self.gen_expr(arg)?;
                // codegen.zig Value arm head: `&ident` of an
                // addressable binding is passed as `&` of its
                // materialised address register, never as the
                // bare address (the callee contract is a borrow).
                if let Some(borrowed_name) = sla_lowering::borrowed_identifier_name(arg) {
                    let resolved = self.resolve_binding_name(borrowed_name);
                    if self.addressable_bindings.contains(borrowed_name)
                        || self.addressable_bindings.contains(&resolved)
                    {
                        return Ok(LoweredCallArg {
                            reg: format!("&{raw_reg}"),
                            release_after_call: false,
                            release_reg: if plan.release_after_call {
                                Some(raw_reg.clone())
                            } else {
                                None
                            },
                            consume_reg: None,
                        });
                    }
                }
                if plan.transfers_ownership {
                    // The Vec-clone preservation arm (identifier must
                    // stay live for a later use) needs the collection
                    // family; plain transfers consume directly. The
                    // consume_reg carries the ^-prefixed register
                    // (as in the Zig): its forget is a bookkeeping
                    // mark only, since the call operand's own `^`
                    // already consumed the value — a raw consume_reg
                    // here emitted a second `^reg` line and the host
                    // rejected it with UseAfterMove.
                    let mv = format!("^{raw_reg}");
                    return Ok(LoweredCallArg {
                        reg: mv.clone(),
                        release_after_call: false,
                        release_reg: None,
                        consume_reg: Some(mv),
                    });
                }
                let cap = param.map(|p| abi_call_arg_cap_kind(p.is_borrow, p.is_move));
                // lowering_rules.abiParamNeedsBorrowArg: the cap
                // kind covers the `&param` spelling; a parameter
                // whose TYPE is a borrow needs the same `&` prefix
                // (unless the register already carries one).
                let needs_borrow = matches!(cap, Some(AbiCapKind::Borrow))
                    || param
                        .map(|p| p.is_borrow || matches!(&*p.ty, sla_ast::Type::Borrow(_)))
                        .unwrap_or(false);
                if needs_borrow && !raw_reg.starts_with('&') {
                    let release = self.call_arg_result_temp_needs_release_for_param(
                        arg,
                        &raw_reg,
                        param,
                    );
                    return Ok(LoweredCallArg {
                        reg: format!("&{raw_reg}"),
                        release_after_call: false,
                        release_reg: if release { Some(raw_reg) } else { None },
                        consume_reg: None,
                    });
                }
                let release_after = plan.release_after_call
                    || self.call_arg_result_temp_needs_release_for_param(arg, &raw_reg, param);
                let consume =
                    self.call_arg_result_temp_needs_consume_for_param(arg, &raw_reg, param);
                Ok(LoweredCallArg {
                    reg: raw_reg.clone(),
                    release_after_call: release_after,
                    release_reg: None,
                    consume_reg: if consume { Some(raw_reg) } else { None },
                })
            }
            CallArgMaterializationKind::AutoBorrow => {
                let raw_reg = self.gen_expr(arg)?;
                let release =
                    plan.release_after_call || self.call_arg_result_temp_needs_release(arg, &raw_reg);
                Ok(LoweredCallArg {
                    reg: format!("&{raw_reg}"),
                    release_after_call: false,
                    release_reg: if release { Some(raw_reg) } else { None },
                    consume_reg: None,
                })
            }
            CallArgMaterializationKind::RawPointerStringLiteral => {
                // codegen.zig:12872 + 14312: a string literal for a
                // raw-pointer-backed parameter materialises as a
                // NUL-terminated @const and its address. A `^`
                // parameter consumes that address directly.
                let value = match arg {
                    Node::Literal(Literal::StringVal(s)) => s.clone(),
                    _ => {
                        return unsupported(
                            "raw-pointer string literal for non-literal arg",
                            "codegen.zig:12872 (genCallArgFromMaterializationPlan)",
                        )
                    }
                };
                let ptr_reg = self.gen_raw_pointer_string_literal_arg(&value);
                if param.map(|p| p.is_move).unwrap_or(false) {
                    let mv = format!("^{ptr_reg}");
                    return Ok(LoweredCallArg {
                        reg: mv.clone(),
                        release_after_call: false,
                        release_reg: None,
                        consume_reg: Some(mv),
                    });
                }
                Ok(LoweredCallArg {
                    reg: ptr_reg,
                    release_after_call: plan.release_after_call,
                    release_reg: None,
                    consume_reg: None,
                })
            }
            CallArgMaterializationKind::CopyStructValue => {
                // Zig genCallArgFromMaterializationPlan copy arm:
                // deep-copy the value into a fresh record; the
                // copy passes plainly (the Copy-struct contract
                // is a plain pointer) and is FORGOTTEN after the
                // call, never released — the callee owns the
                // by-value parameter and may return it.
                let target_param = param.ok_or_else(|| crate::TextError::MissingType {
                    context: "copy struct value parameter".to_string(),
                })?;
                let source_reg = self.gen_expr(arg)?;
                let copied = self.new_tmp();
                self.gen_copy_value_into(&copied, &source_reg, &target_param.ty)?;
                Ok(LoweredCallArg {
                    reg: copied.clone(),
                    release_after_call: plan.release_after_call,
                    release_reg: None,
                    consume_reg: Some(copied),
                })
            }
            CallArgMaterializationKind::ShallowCopyPreservedValue => {
                // Zig shallow-copy arm: copy the record, pass
                // the copy as a move; the source binding stays
                // live and owning.
                let arg_ty = self
                    .expr_type(arg)
                    .ok_or_else(|| crate::TextError::MissingType {
                        context: "shallow copy argument type".to_string(),
                    })?;
                let source_reg = self.gen_expr(arg)?;
                let copied = self
                    .gen_record_copy(&source_reg, &arg_ty)?
                    .ok_or_else(|| crate::TextError::Unsupported {
                        feature: "shallow copy of handle-field record".to_string(),
                        zig_ref: "codegen.zig:5705 (genShallowCopyCallArgValue)",
                    })?;
                Ok(LoweredCallArg {
                    reg: format!("^{copied}"),
                    release_after_call: false,
                    release_reg: None,
                    consume_reg: None,
                })
            }
            kind => Err(crate::TextError::Unsupported {
                feature: format!("call arg materialization kind {kind:?}"),
                zig_ref: "codegen.zig:9860 (genCallArgFromMaterializationPlan)",
            }),
        }
    }

    /// Shared argument planning for expr/statement calls to a known
    /// function: returns the display registers and the planned args.
    fn plan_known_call_args(
        &mut self,
        call: &sla_ast::CallExpr,
        params: &[Param],
    ) -> Result<(Vec<String>, Vec<LoweredCallArg>)> {
        let mut planned_args: Vec<LoweredCallArg> = Vec::new();
        for (i, arg) in call.args.iter().enumerate() {
            let param = params.get(i);
            let planned = self.gen_planned_call_arg(arg, param, i == 0)?;
            planned_args.push(planned);
        }
        let display: Vec<String> = planned_args.iter().map(|p| p.reg.clone()).collect();
        Ok((display, planned_args))
    }

    /// codegen.zig `genExpr` call path (plain known-function arm,
    /// panic arm, constructors, EXPAND fallback).
    pub(crate) fn gen_expr_call(&mut self, expr: &Node) -> Result<String> {
        let call = match expr {
            Node::CallExpr(c) => c,
            _ => unreachable!(),
        };
        if call.callee.is_some() {
            return unsupported(
                "closure callee call",
                "codegen.zig:14410 (closure callee dispatch)",
            );
        }
        if self.check.resolved_call_symbol(expr).is_some() {
            return unsupported(
                "call via resolved static call plan",
                "lowering_rules.zig:5819 (resolved static call plans)",
            );
        }
        if let Some(surface) = self.gen_std_surface_call(expr, call) {
            return match surface? {
                crate::surface::SurfaceCall::Value(reg) => Ok(reg),
                crate::surface::SurfaceCall::Void => Ok(self.new_tmp()),
            };
        }
        if call.associated_target.is_some() {
            return unsupported(
                "associated-target call (method/special families deferred)",
                "codegen.zig:14600 (associated call dispatch)",
            );
        }
        let func_name = call.func_name.clone();

        // Option/Result constructors (EXPAND forms).
        match func_name.as_str() {
            "Some" | "Ok" | "Err" => {
                let expand_name = match func_name.as_str() {
                    "Some" => "OPTION_NEW_SOME",
                    "Ok" => "RESULT_NEW_OK",
                    _ => "RESULT_NEW_ERR",
                };
                if call.args.len() != 1 {
                    return unsupported(
                        "Option/Result constructor arity",
                        "codegen.zig:14700 (constructor builtins)",
                    );
                }
                let planned = self.gen_planned_call_arg(&call.args[0], None, false)?;
                let result = self.new_tmp();
                self.ins(&format!("EXPAND {expand_name} {result}, {}", planned.reg));
                let planned_args = vec![planned];
                self.emit_lowered_call_arg_cleanups(&planned_args, None)?;
                return Ok(result);
            }
            "None" => {
                let result = self.new_tmp();
                self.ins(&format!("EXPAND OPTION_NEW_NONE {result}"));
                return Ok(result);
            }
            "format" => {
                return self.gen_format_call(call);
            }
            _ => {}
        }

        if is_panic_builtin_name(&func_name) {
            return self.gen_panic_call(call);
        }
        if let Some(view) = self.check.imported_macro(&func_name) {
            return self.gen_imported_macro_call_expr(call, &view);
        }
        if self.check.is_macro(&func_name) {
            return unsupported(
                "user macro call expansion",
                "codegen.zig:15480 (imported/user macro call plans)",
            );
        }
        if let Some(sig) = self.check.extern_sig(&func_name) {
            return self.gen_extern_call(call, &sig);
        }

        // Plain known-function call; a `using` extension call
        // site keeps the bare name, so fall back to the unique
        // tail-segment match (SAB driver tail) before the
        // EXPAND catch-all.
        let known = self
            .func_decl(&func_name)
            .map(|d| (func_name.clone(), d))
            .or_else(|| self.func_decl_tail_match(&func_name));
        if let Some((callee_name, decl)) = known {
            let (display, planned_args) = self.plan_known_call_args(call, &decl.params)?;
            let result = self.new_tmp();
            let lowered = self.lowered_func_symbol(&callee_name);
            self.ins(&format!(
                "{result} = call @{lowered}({})",
                display.join(", ")
            ));
            self.emit_lowered_call_arg_cleanups(&planned_args, Some(&callee_name))?;
            return Ok(result);
        }

        if sla_lowering::is_stack_alloc_node(expr) {
            let size = self.stack_alloc_size_of(expr);
            let result = self.new_tmp();
            self.ins(&format!("{result} = stack_alloc {size}"));
            return Ok(result);
        }

        // Catch-all (mirrored deliberately): an unknown callee name
        // is assumed to be a macro and emitted as an EXPAND, with no
        // error. See docs/divergences/lowering-text.md.
        let mut arg_regs: Vec<String> = Vec::new();
        for arg in &call.args {
            arg_regs.push(self.gen_expr(arg)?);
        }
        let result = self.new_tmp();
        let mut line = format!("EXPAND {func_name} {result}");
        for reg in &arg_regs {
            line.push_str(&format!(", {reg}"));
        }
        self.ins(&line);
        let _ = call_arg_needs_release;
        Ok(result)
    }

    /// codegen.zig:14312 `genRawPointerStringLiteralArg`: the
    /// @const carries a NUL terminator so the pointer can also
    /// be scanned as a C string; explicit-length users pass the
    /// escaped byte length, which excludes the terminator.
    fn gen_raw_pointer_string_literal_arg(&mut self, value: &str) -> String {
        let label = self.new_string_const();
        self.ins(&format!("@const {label} = utf8:\"{value}\\0\""));
        let ptr_reg = self.new_tmp();
        self.ins(&format!("{ptr_reg} = &{label}"));
        ptr_reg
    }

    /// codegen.zig panic builtin path.
    fn gen_panic_call(&mut self, call: &sla_ast::CallExpr) -> Result<String> {
        if call.args.is_empty() {
            self.ins("panic(1)");
            return Ok(self.new_tmp());
        }
        let arg = &call.args[0];
        match arg {
            Node::Literal(Literal::IntVal(value)) => {
                self.ins(&format!("panic({value})"));
            }
            Node::Literal(Literal::BoolVal(value)) => {
                self.ins(&format!("panic({})", if *value { 1 } else { 0 }));
            }
            Node::Literal(Literal::StringVal(text)) => {
                let label = self.new_string_const();
                self.ins(&format!("@const {label} = utf8:\"{text}\""));
                let len = crate::codegen::escaped_string_byte_len(text);
                let result = self.new_tmp();
                self.ins(&format!("EXPAND PANIC_MSG {result}, &{label}, {len}"));
                return Ok(result);
            }
            _ => {
                let reg = self.gen_expr(arg)?;
                self.ins(&format!("panic({reg})"));
            }
        }
        Ok(self.new_tmp())
    }

    /// codegen.zig:1868 `genCallStmt`.
    pub(crate) fn gen_call_stmt(&mut self, expr: &Node) -> Result<()> {
        let call = match expr {
            Node::CallExpr(c) => c,
            _ => unreachable!(),
        };
        if is_println_call(call) {
            return self.emit_println(call);
        }
        if let Some(surface) = self.gen_std_surface_call(expr, call) {
            match surface? {
                crate::surface::SurfaceCall::Value(reg) => {
                    if self.expr_result_reg_needs_release(expr) {
                        self.emit_release(&reg)?;
                    }
                }
                crate::surface::SurfaceCall::Void => {}
            }
            return Ok(());
        }
        if call.callee.is_some() || call.associated_target.is_some() {
            return unsupported(
                "statement call with callee/associated target",
                "codegen.zig:1868 (genCallStmt dispatch)",
            );
        }
        let func_name = call.func_name.clone();
        if let Some(view) = self.check.imported_macro(&func_name) {
            return self.gen_imported_macro_call_stmt(call, &view);
        }
        if self.check.is_macro(&func_name) {
            return unsupported(
                "user macro statement call",
                "codegen.zig:1900 (genCallStmt macro path)",
            );
        }
        if let Some(sig) = self.check.extern_sig(&func_name) {
            return self.gen_extern_call_stmt(expr, call, &sig);
        }
        if let Some(decl) = self.func_decl(&func_name) {
            let (display, planned_args) = self.plan_known_call_args(call, &decl.params)?;
            let lowered = self.lowered_func_symbol(&func_name);
            self.ins(&format!("call @{lowered}({})", display.join(", ")));
            self.emit_lowered_call_arg_cleanups(&planned_args, Some(&func_name))?;
            return Ok(());
        }
        unsupported(
            "unknown statement call",
            "codegen.zig:1868 (genCallStmt fallthrough)",
        )
    }

    fn stack_alloc_size_of(&self, expr: &Node) -> i64 {
        stack_alloc_size_of(expr, self)
    }
}

/// Map a contract ABI type spelling to the AST type the call-arg
/// planner consumes (the planner only reads shape facts from it:
/// copy-struct-ness, borrow needs, pointer-ness). Spellings are
/// the `.sai` surface forms (`i32`, `ptr`, `*u8`, `Slice`, …);
/// anything unrecognised becomes a plain user-defined name, which
/// the planner treats as an opaque value — the same treatment the
/// SAB side's `abi_prim_type` gives unknown spellings.
pub(crate) fn abi_type_from_spelling(spelling: &str) -> Type {
    let s = spelling.trim().trim_end_matches('!').trim();
    if let Some(inner) = s.strip_prefix('*') {
        return Type::Pointer(Box::new(abi_type_from_spelling(inner)));
    }
    if let Some(inner) = s.strip_prefix('&') {
        return Type::Borrow(Box::new(abi_type_from_spelling(inner)));
    }
    let prim = match s {
        "i8" => Some(Primitive::I8),
        "i16" => Some(Primitive::I16),
        "i32" => Some(Primitive::I32),
        "i64" => Some(Primitive::I64),
        "isize" => Some(Primitive::Isize),
        "u8" => Some(Primitive::U8),
        "u16" => Some(Primitive::U16),
        "u32" => Some(Primitive::U32),
        "u64" => Some(Primitive::U64),
        "usize" => Some(Primitive::Usize),
        "f32" => Some(Primitive::F32),
        "f64" => Some(Primitive::F64),
        "bool" => Some(Primitive::Boolean),
        "ptr" | "raw_ptr" => Some(Primitive::RawPtr),
        "void" => Some(Primitive::VoidType),
        "integer" => Some(Primitive::Integer),
        "float" => Some(Primitive::Float),
        _ => None,
    };
    match prim {
        Some(p) => Type::Primitive(p),
        None => Type::UserDefined(UserDefined {
            name: s.to_string(),
            generics: Vec::new(),
        }),
    }
}

fn stack_alloc_size_of(expr: &Node, cg: &Codegen) -> i64 {
        if let Node::CallExpr(call) = expr {
            if let Some(first) = call.args.first() {
                let lookup = |name: &str| match cg.global_scalar_const(name) {
                    Some(Literal::IntVal(v)) => Some(v),
                    _ => None,
                };
                if let Some(v) = sla_lowering::int_constant_expr_value(first, &lookup) {
                    return v;
                }
            }
        }
        8
}

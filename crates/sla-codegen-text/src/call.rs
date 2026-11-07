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

use sla_ast::{Literal, Node, Param, Type};
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

    /// codegen.zig:9986 `genPlannedCallArg`.
    pub(crate) fn gen_planned_call_arg(
        &mut self,
        arg: &Node,
        param: Option<&Param>,
        receiver_style_auto_borrow: bool,
    ) -> Result<LoweredCallArg> {
        let arg_ty = self.expr_type(arg);
        let arg_is_copy = arg_ty
            .as_ref()
            .map(|t| self.type_is_copy_value(t))
            .unwrap_or(false);
        let input = CallArgMaterializationInput {
            param: param.cloned(),
            arg_ty: arg_ty.clone(),
            arg_index: if receiver_style_auto_borrow { 0 } else { 1 },
            auto_borrow_receiver: receiver_style_auto_borrow,
            receiver_style_auto_borrow,
            statement_receiver_auto_borrow: false,
            abi_borrow_auto_borrow: param
                .map(|p| abi_param_needs_borrow_arg(p.is_borrow, &p.ty))
                .unwrap_or(false),
            array_to_slice_borrow: false,
            dyn_borrow_trait_name: None,
            copy_struct_value: call_arg_is_copy_struct_value(
                arg,
                param,
                param
                    .map(|p| self.check.type_is_copy_struct(&p.ty))
                    .unwrap_or(false),
            ),
            generated_fn_ptr_identifier: false,
            generated_scalar_const_identifier: false,
            local_fn_ptr_identifier: matches!(arg, Node::Identifier(_))
                && matches!(arg_ty, Some(Type::FnPtr(_))),
            shallow_copy_value: arg_ty
                .as_ref()
                .map(|t| self.check.type_is_shallow_copy_call_arg_value(t))
                .unwrap_or(false),
            value_arg_transfers_ownership: value_arg_transfers_ownership_from_param(
                param,
                arg_ty.as_ref(),
                arg_is_copy,
            ),
            target: CallArgTarget::SaText,
        };
        let plan = plan_call_arg_materialization(arg, &input);
        self.gen_call_arg_from_materialization_plan(arg, param, &plan)
    }

    /// codegen.zig:9860 `genCallArgFromMaterializationPlan`. The
    /// Value and AutoBorrow arms are ported; every other plan kind
    /// belongs to a deferred family and fails loudly.
    fn gen_call_arg_from_materialization_plan(
        &mut self,
        arg: &Node,
        param: Option<&Param>,
        plan: &sla_lowering::CallArgMaterializationPlan,
    ) -> Result<LoweredCallArg> {
        match plan.kind {
            CallArgMaterializationKind::Value => {
                let raw_reg = self.gen_expr(arg)?;
                if plan.transfers_ownership {
                    // The Vec-clone preservation arm (identifier must
                    // stay live for a later use) needs the collection
                    // family; plain transfers consume directly.
                    return Ok(LoweredCallArg {
                        reg: format!("^{raw_reg}"),
                        release_after_call: false,
                        release_reg: None,
                        consume_reg: Some(raw_reg),
                    });
                }
                let cap = param.map(|p| abi_call_arg_cap_kind(p.is_borrow, p.is_move));
                if matches!(cap, Some(AbiCapKind::Borrow)) {
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
                return unsupported(
                    "format builtin call",
                    "codegen.zig:14520 (format builtin)",
                )
            }
            _ => {}
        }

        if is_panic_builtin_name(&func_name) {
            return self.gen_panic_call(call);
        }
        if self.check.is_macro(&func_name) {
            return unsupported(
                "user macro call expansion",
                "codegen.zig:15480 (imported/user macro call plans)",
            );
        }

        if let Some(decl) = self.func_decl(&func_name) {
            let (display, planned_args) = self.plan_known_call_args(call, &decl.params)?;
            let result = self.new_tmp();
            let lowered = self.lowered_func_symbol(&func_name);
            self.ins(&format!(
                "{result} = call @{lowered}({})",
                display.join(", ")
            ));
            self.emit_lowered_call_arg_cleanups(&planned_args, Some(&func_name))?;
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
        if self.check.is_macro(&func_name) {
            return unsupported(
                "user macro statement call",
                "codegen.zig:1900 (genCallStmt macro path)",
            );
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
        if let Node::CallExpr(call) = expr {
            if let Some(first) = call.args.first() {
                let lookup = |name: &str| match self.global_scalar_const(name) {
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
}

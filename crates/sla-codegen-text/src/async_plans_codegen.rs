//! Async continuation plan emission (poll helpers + plan-shaped
//! function declarations), mirrored from codegen.zig:
//! emitAsyncSingleAwaitPollHelper (8483), genAsyncSingleAwait-
//! FuncDeclNamed (8879), emitAsyncTwoAwaitPollHelper (8601),
//! genAsyncTwoAwaitFuncDeclNamed (9334), emitAsyncJoin2Await-
//! PollHelper (8819), genAsyncJoin2AwaitFuncDeclNamed (9369),
//! emitAsyncLinearAwaitPollHelper (8697), genAsyncLinearAwait-
//! FuncDeclNamed (8938), emitAsyncGeneralAwaitPollHelper (9201),
//! genAsyncGeneralAwaitFuncDeclNamed (8980), emitAsyncLoopAwait-
//! PollHelper (9046), genAsyncLoopAwaitFuncDeclNamed (9160).
//! Stage 6 async cluster.

use sla_ast::{FuncDecl, Node, Type};
use sla_lowering::{
    abi_param_type_string, async_continuation_condition_op_name, by_value_raw_pointer_param,
    is_borrow_like_type, plan_async_function_return, shared_abi_param_prefix,
    AsyncContinuationScalarPlan, AsyncContinuationCaptureStorage, AsyncGeneralAwaitContinuationPlan,
    AsyncJoin2AwaitContinuationPlan, AsyncLinearAwaitContinuationPlan,
    AsyncLoopAwaitContinuationPlan, AsyncSingleAwaitContinuationPlan,
    AsyncTwoAwaitContinuationPlan, FutureReadiness,
};

use crate::async_scalar::{
    async_general_poll, async_general_vt, async_join2_poll, async_join2_vt, async_linear_poll,
    async_linear_vt, async_loop_poll, async_loop_vt, async_single_poll, async_single_vt,
    async_two_poll, async_two_vt,
};
use crate::codegen::{type_string, Codegen};
use crate::{unsupported, Result};

/// Tiny `%TOKEN%` substitution for the raw-text poll helpers (the
/// SA text contains braces, so `format!` templates do not apply).
pub(crate) fn tpl(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in pairs {
        out = out.replace(k, v);
    }
    out
}

impl<'c> Codegen<'c> {
    /// Shared prologue of every genAsync*FuncDeclNamed: signature
    /// with the async return plan (raw ptr return), L_ENTRY, and
    /// the hoisted-alloc collection (collected, not emitted).
    pub(crate) fn emit_async_fn_signature(&mut self, name: &str, f: &FuncDecl) {
        let lowered_name = self.lowered_func_symbol(name);
        let mut sig = format!("@{lowered_name}(");
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                sig.push_str(", ");
            }
            let prefix = shared_abi_param_prefix(
                p.is_borrow,
                p.is_move,
                matches!(&*p.ty, Type::Borrow(_)),
                by_value_raw_pointer_param(p),
                self.type_is_copy_value(&p.ty),
                is_borrow_like_type(&p.ty),
            );
            sig.push_str(&format!(
                "{prefix}{}: {}",
                p.name,
                abi_param_type_string(p.is_borrow, p.is_move, &p.ty)
            ));
        }
        let plan = plan_async_function_return(f);
        debug_assert!(plan.abi_ret_is_ptr);
        sig.push_str(") -> ptr:");
        self.wl(&sig);
        self.wl("L_ENTRY:");
        let mut hoisted: Vec<String> = Vec::new();
        self.collect_hoisted_allocs(&f.body, &mut hoisted);
        self.hoisted_allocs = hoisted;
    }

    /// Adopt a heap register into an async state slot: a MOVE
    /// store (`^reg`) transfers ownership to the state. A plain
    /// store + `!reg` release (the literal Zig text) frees the
    /// block on the pinned SA host and the next alloc reuses it —
    /// proven by SA probes (probe9/probe10/probe11, stage-6
    /// evidence): the stored sub-state must be moved, not copied
    /// and released. The register is marked consumed so later
    /// cleanup passes skip it.
    pub(crate) fn adopt_store(&mut self, state: &str, offset: usize, reg: &str) {
        let bare = reg.strip_prefix('^').unwrap_or(reg);
        self.ins(&format!("store {state}+{offset}, ^{bare} as ptr"));
        let resolved = self.resolve_binding_name(bare);
        self.consumed_bindings.insert(resolved);
    }

    pub(crate) fn finish_async_state(&mut self, async_state: &str, vt_name: &str, f: &FuncDecl) -> Result<()> {
        for p in &f.params {
            self.emit_release(&p.name)?;
        }
        self.future_state_vtables
            .insert(async_state.to_string(), vt_name.to_string());
        self.record_future_readiness(async_state, FutureReadiness::Unknown);
        self.ins(&format!("return {async_state}"));
        self.w("\n");
        Ok(())
    }

    // -- single await ---------------------------------------------------

    /// codegen.zig:8483 `emitAsyncSingleAwaitPollHelper`.
    fn emit_async_single_await_poll_helper(
        &mut self,
        name: &str,
        plan: &AsyncSingleAwaitContinuationPlan,
    ) -> Result<()> {
        let vt_name = async_single_vt(name);
        let poll_name = async_single_poll(name);
        self.w(&tpl(
            "@const %VT% = vtable { poll = @%POLL% }\n\
             @%POLL%(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   async_stage = load data_slot+0 as u64\n\
             \x20   async_done = eq async_stage, 1\n\
             \x20   br async_done -> L_ASYNC_SINGLE_AWAIT_EMPTY, L_ASYNC_SINGLE_AWAIT_POLL\n\
             L_ASYNC_SINGLE_AWAIT_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_SINGLE_AWAIT_DONE\n\
             L_ASYNC_SINGLE_AWAIT_POLL:\n\
             \x20   async_inner_state = load data_slot+8 as ptr\n\
             \x20   async_inner_stage = load async_inner_state+0 as u64\n\
             \x20   async_inner_initial = eq async_inner_stage, 0\n\
             \x20   br async_inner_initial -> L_ASYNC_SINGLE_AWAIT_PENDING, L_ASYNC_SINGLE_AWAIT_CHECK_READY\n\
             L_ASYNC_SINGLE_AWAIT_PENDING:\n\
             \x20   store async_inner_state+0, 1 as u64\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_SINGLE_AWAIT_CLEAN\n\
             L_ASYNC_SINGLE_AWAIT_CHECK_READY:\n\
             \x20   async_inner_ready = eq async_inner_stage, 1\n\
             \x20   br async_inner_ready -> L_ASYNC_SINGLE_AWAIT_READY, L_ASYNC_SINGLE_AWAIT_INNER_EMPTY\n\
             L_ASYNC_SINGLE_AWAIT_INNER_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   !async_inner_ready\n\
             \x20   jmp L_ASYNC_SINGLE_AWAIT_CLEAN\n\
             L_ASYNC_SINGLE_AWAIT_READY:\n",
            &[("%VT%", &vt_name), ("%POLL%", &poll_name)],
        ));
        self.ins(&format!("{} = load async_inner_state+8 as u64", plan.binding_name));
        let mut captured_addend_regs: [Option<String>; 2] = [None, None];
        let mut captured_storage_regs: [Option<String>; 2] = [None, None];
        let scalar = &plan.scalar;
        for capture_idx in 0..plan.capture_count {
            let capture = plan.captures[capture_idx]
                .as_ref()
                .expect("plan capture present");
            let reg_name = if capture_idx == 0 {
                "async_captured_addend"
            } else {
                "async_captured_addend2"
            };
            match capture.storage {
                AsyncContinuationCaptureStorage::Scalar => {
                    self.ins(&format!(
                        "{reg_name} = load data_slot+{} as u64",
                        capture.offset
                    ));
                    captured_addend_regs[capture_idx] = Some(reg_name.to_string());
                }
                AsyncContinuationCaptureStorage::CopyStruct => {
                    if plan.branch.is_some() {
                        return unsupported(
                            "async single-await copy-struct capture with branch",
                            "codegen.zig:8483",
                        );
                    }
                    let field_name = if capture_idx == 0 {
                        scalar.captured_field_name.as_deref()
                    } else {
                        scalar.captured2_field_name.as_deref()
                    };
                    let field = match field_name {
                        Some(f) => f,
                        None => {
                            return unsupported(
                                "async copy-struct capture field",
                                "codegen.zig:8483",
                            )
                        }
                    };
                    let capture_ty = match self.expr_type(capture.expr) {
                        Some(t) => t,
                        None => {
                            return Err(crate::TextError::MissingType {
                                context: "async capture type".to_string(),
                            })
                        }
                    };
                    if !self.type_is_copy_struct(&capture_ty) {
                        return unsupported(
                            "async capture is not a Copy struct",
                            "codegen.zig:8483",
                        );
                    }
                    let (offset, field_ty) = match self.async_field_layout(&capture_ty, field) {
                        Some(l) => l,
                        None => {
                            return unsupported(
                                "async capture field layout",
                                "codegen.zig:8483",
                            )
                        }
                    };
                    let ptr_reg = if capture_idx == 0 {
                        "async_captured_struct"
                    } else {
                        "async_captured_struct2"
                    };
                    self.ins(&format!(
                        "{ptr_reg} = load data_slot+{} as ptr",
                        capture.offset
                    ));
                    self.ins(&format!(
                        "{reg_name} = load {ptr_reg}+{offset} as {}",
                        type_string(&field_ty)
                    ));
                    captured_addend_regs[capture_idx] = Some(reg_name.to_string());
                    captured_storage_regs[capture_idx] = Some(ptr_reg.to_string());
                }
            }
        }
        let captured_refs: [Option<&str>; 2] = [
            captured_addend_regs[0].as_deref(),
            captured_addend_regs[1].as_deref(),
        ];
        let result_reg: String = if plan.branch.is_some() {
            "async_result".to_string()
        } else if let Some(post) = plan.result_binding_name() {
            post.to_string()
        } else if scalar.is_identity() {
            plan.binding_name.clone()
        } else {
            "async_result".to_string()
        };
        if let Some(branch) = &plan.branch {
            let condition_op = match async_continuation_condition_op_name(branch.condition_op) {
                Some(op) => op,
                None => {
                    return unsupported(
                        "async branch condition op",
                        "lowering_rules.zig:3946",
                    )
                }
            };
            self.ins(&format!(
                "async_branch_cond = {condition_op} {}, {}",
                plan.binding_name, branch.threshold
            ));
            self.w("    br async_branch_cond -> L_ASYNC_SINGLE_AWAIT_BRANCH_THEN, L_ASYNC_SINGLE_AWAIT_BRANCH_ELSE\n");
            self.wl("L_ASYNC_SINGLE_AWAIT_BRANCH_THEN:");
            self.emit_async_continuation_scalar_value(
                &branch.then_scalar,
                &result_reg,
                &plan.binding_name,
                captured_refs,
                "async_then",
            )?;
            self.w("    jmp L_ASYNC_SINGLE_AWAIT_BRANCH_DONE\n");
            self.wl("L_ASYNC_SINGLE_AWAIT_BRANCH_ELSE:");
            self.emit_async_continuation_scalar_value(
                &branch.else_scalar,
                &result_reg,
                &plan.binding_name,
                captured_refs,
                "async_else",
            )?;
            self.w("    jmp L_ASYNC_SINGLE_AWAIT_BRANCH_DONE\n");
            self.wl("L_ASYNC_SINGLE_AWAIT_BRANCH_DONE:");
        } else {
            self.emit_async_continuation_scalar_value(
                scalar,
                &result_reg,
                &plan.binding_name,
                captured_refs,
                "async",
            )?;
        }
        self.ins("store async_inner_state+0, 2 as u64");
        self.ins("store data_slot+0, 1 as u64");
        self.ins(&format!("EXPAND POLL_SET_READY out_poll_slot, {result_reg}"));
        if result_reg != plan.binding_name {
            self.ins(&format!("!{result_reg}"));
        }
        if plan.branch.is_some() {
            self.ins("!async_branch_cond");
        }
        for reg in captured_addend_regs.iter().flatten() {
            self.ins(&format!("!{reg}"));
        }
        for reg in captured_storage_regs.iter().flatten() {
            self.ins(&format!("!{reg}"));
        }
        self.w(&tpl(
            "\x20   !%BIND%\n\
             \x20   !async_inner_ready\n\
             L_ASYNC_SINGLE_AWAIT_CLEAN:\n\
             \x20   !async_inner_initial\n\
             \x20   !async_inner_stage\n\
             \x20   !async_inner_state\n\
             L_ASYNC_SINGLE_AWAIT_DONE:\n\
             \x20   !async_done\n\
             \x20   !async_stage\n\
             \x20   !data_slot\n\
             \x20   !ctx_slot\n\
             \x20   !out_poll_slot\n\
             \x20   return\n\n",
            &[("%BIND%", &plan.binding_name)],
        ));
        Ok(())
    }

    /// Zig `aggregateFieldLayout` over a Copy-struct capture type.
    fn async_field_layout(&self, ty: &Type, field: &str) -> Option<(usize, Type)> {
        let name = match ty {
            Type::UserDefined(ud) => &ud.name,
            _ => return None,
        };
        let decl = self.check.struct_decl(name)?;
        crate::layout::struct_field_layout(decl, field)
    }

    /// codegen.zig:8879 `genAsyncSingleAwaitFuncDeclNamed`.
    pub(crate) fn gen_async_single_await_func_decl_named(
        &mut self,
        name: &str,
        f: &FuncDecl,
        plan: &AsyncSingleAwaitContinuationPlan,
    ) -> Result<()> {
        self.emit_async_single_await_poll_helper(name, plan)?;
        self.emit_async_fn_signature(name, f);
        let mut captured_addends: [Option<String>; 2] = [None, None];
        for capture_idx in 0..plan.capture_count {
            let capture = plan.captures[capture_idx]
                .as_ref()
                .expect("plan capture present");
            let capture_reg = self.gen_expr(capture.expr)?;
            captured_addends[capture_idx] = Some(match capture.storage {
                AsyncContinuationCaptureStorage::Scalar => capture_reg,
                AsyncContinuationCaptureStorage::CopyStruct => {
                    let capture_ty = match self.expr_type(capture.expr) {
                        Some(t) => t,
                        None => {
                            return Err(crate::TextError::MissingType {
                                context: "async capture type".to_string(),
                            })
                        }
                    };
                    if !self.type_is_copy_struct(&capture_ty) {
                        return unsupported(
                            "async capture is not a Copy struct",
                            "codegen.zig:8879",
                        );
                    }
                    if matches!(capture.expr, Node::Identifier(_)) {
                        let copied = self.new_tmp();
                        self.gen_copy_value_into(&copied, &capture_reg, &capture_ty)?;
                        copied
                    } else {
                        capture_reg
                    }
                }
            });
        }
        let inner_state = self.gen_expr(plan.await_expr)?;
        let async_state = self.new_tmp();
        self.ins(&format!("{async_state} = alloc {}", plan.async_state_size()));
        self.ins(&format!("store {async_state}+0, 0 as u64"));
        self.adopt_store(&async_state.clone(), 8, &inner_state);
        for capture_idx in 0..plan.capture_count {
            let capture = plan.captures[capture_idx]
                .as_ref()
                .expect("plan capture present");
            let addend_reg = captured_addends[capture_idx].as_deref().unwrap();
            match capture.storage {
                AsyncContinuationCaptureStorage::Scalar => {
                    self.ins(&format!(
                        "store {async_state}+{}, {addend_reg} as u64",
                        capture.offset
                    ));
                    self.emit_release(addend_reg)?;
                }
                AsyncContinuationCaptureStorage::CopyStruct => {
                    self.adopt_store(&async_state.clone(), capture.offset, addend_reg);
                }
            }
        }
        self.finish_async_state(&async_state, &async_single_vt(name), f)
    }

    // -- two await ---------------------------------------------------------

    /// codegen.zig:8601 `emitAsyncTwoAwaitPollHelper`.
    fn emit_async_two_await_poll_helper(
        &mut self,
        name: &str,
        plan: &AsyncTwoAwaitContinuationPlan,
    ) -> Result<()> {
        let vt_name = async_two_vt(name);
        let poll_name = async_two_poll(name);
        self.w(&tpl(
            "@const %VT% = vtable { poll = @%POLL% }\n\
             @%POLL%(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   async_stage = load data_slot+0 as u64\n\
             \x20   async_poll_first = eq async_stage, 0\n\
             \x20   async_done = eq async_stage, 2\n\
             \x20   br async_done -> L_ASYNC_TWO_AWAIT_EMPTY, L_ASYNC_TWO_AWAIT_DISPATCH\n\
             L_ASYNC_TWO_AWAIT_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_TWO_AWAIT_DONE\n\
             L_ASYNC_TWO_AWAIT_DISPATCH:\n\
             \x20   br async_poll_first -> L_ASYNC_TWO_AWAIT_FIRST, L_ASYNC_TWO_AWAIT_SECOND\n\
             L_ASYNC_TWO_AWAIT_FIRST:\n\
             \x20   async_first_state = load data_slot+8 as ptr\n\
             \x20   async_first_stage = load async_first_state+0 as u64\n\
             \x20   async_first_initial = eq async_first_stage, 0\n\
             \x20   br async_first_initial -> L_ASYNC_TWO_AWAIT_FIRST_PENDING, L_ASYNC_TWO_AWAIT_FIRST_CHECK_READY\n\
             L_ASYNC_TWO_AWAIT_FIRST_PENDING:\n\
             \x20   store async_first_state+0, 1 as u64\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_TWO_AWAIT_FIRST_CLEAN\n\
             L_ASYNC_TWO_AWAIT_FIRST_CHECK_READY:\n\
             \x20   async_first_ready = eq async_first_stage, 1\n\
             \x20   br async_first_ready -> L_ASYNC_TWO_AWAIT_FIRST_READY, L_ASYNC_TWO_AWAIT_FIRST_EMPTY\n\
             L_ASYNC_TWO_AWAIT_FIRST_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   !async_first_ready\n\
             \x20   jmp L_ASYNC_TWO_AWAIT_FIRST_CLEAN\n\
             L_ASYNC_TWO_AWAIT_FIRST_READY:\n",
            &[("%VT%", &vt_name), ("%POLL%", &poll_name)],
        ));
        let first = &plan.first_binding_name;
        let second = &plan.second_binding_name;
        self.ins(&format!("{first} = load async_first_state+8 as u64"));
        self.ins(&format!("store data_slot+24, {first} as u64"));
        self.ins("store async_first_state+0, 2 as u64");
        self.ins("store data_slot+0, 1 as u64");
        self.ins(&format!("!{first}"));
        self.w(
            "\x20   !async_first_ready\n\
             \x20   !async_first_initial\n\
             \x20   !async_first_stage\n\
             \x20   !async_first_state\n\
             \x20   jmp L_ASYNC_TWO_AWAIT_SECOND\n\
             L_ASYNC_TWO_AWAIT_FIRST_CLEAN:\n\
             \x20   !async_first_initial\n\
             \x20   !async_first_stage\n\
             \x20   !async_first_state\n\
             \x20   jmp L_ASYNC_TWO_AWAIT_DONE\n\
             L_ASYNC_TWO_AWAIT_SECOND:\n\
             \x20   async_second_state = load data_slot+16 as ptr\n\
             \x20   async_second_stage = load async_second_state+0 as u64\n\
             \x20   async_second_initial = eq async_second_stage, 0\n\
             \x20   br async_second_initial -> L_ASYNC_TWO_AWAIT_SECOND_PENDING, L_ASYNC_TWO_AWAIT_SECOND_CHECK_READY\n\
             L_ASYNC_TWO_AWAIT_SECOND_PENDING:\n\
             \x20   store async_second_state+0, 1 as u64\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_TWO_AWAIT_SECOND_CLEAN\n\
             L_ASYNC_TWO_AWAIT_SECOND_CHECK_READY:\n\
             \x20   async_second_ready = eq async_second_stage, 1\n\
             \x20   br async_second_ready -> L_ASYNC_TWO_AWAIT_SECOND_READY, L_ASYNC_TWO_AWAIT_SECOND_EMPTY\n\
             L_ASYNC_TWO_AWAIT_SECOND_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   !async_second_ready\n\
             \x20   jmp L_ASYNC_TWO_AWAIT_SECOND_CLEAN\n\
             L_ASYNC_TWO_AWAIT_SECOND_READY:\n",
        );
        self.ins(&format!("{first} = load data_slot+24 as u64"));
        self.ins(&format!("{second} = load async_second_state+8 as u64"));
        let scalar = AsyncContinuationScalarPlan {
            awaited_coeff: plan.scalar.second_coeff,
            captured_coeff: plan.scalar.first_coeff,
            captured2_coeff: 0,
            captured_field_name: None,
            captured2_field_name: None,
            immediate: plan.scalar.immediate,
        };
        self.emit_async_continuation_scalar_value(
            &scalar,
            "async_two_result",
            second,
            [Some(first.as_str()), None],
            "async_two",
        )?;
        self.ins("store async_second_state+0, 2 as u64");
        self.ins("store data_slot+0, 2 as u64");
        self.ins("EXPAND POLL_SET_READY out_poll_slot, async_two_result");
        self.ins("!async_two_result");
        self.ins(&format!("!{second}"));
        self.ins(&format!("!{first}"));
        self.w(
            "\x20   !async_second_ready\n\
             L_ASYNC_TWO_AWAIT_SECOND_CLEAN:\n\
             \x20   !async_second_initial\n\
             \x20   !async_second_stage\n\
             \x20   !async_second_state\n\
             L_ASYNC_TWO_AWAIT_DONE:\n\
             \x20   !async_poll_first\n\
             \x20   !async_done\n\
             \x20   !async_stage\n\
             \x20   !data_slot\n\
             \x20   !ctx_slot\n\
             \x20   !out_poll_slot\n\
             \x20   return\n\n",
        );
        Ok(())
    }

    /// codegen.zig:9334 `genAsyncTwoAwaitFuncDeclNamed`.
    pub(crate) fn gen_async_two_await_func_decl_named(
        &mut self,
        name: &str,
        f: &FuncDecl,
        plan: &AsyncTwoAwaitContinuationPlan,
    ) -> Result<()> {
        self.emit_async_two_await_poll_helper(name, plan)?;
        self.emit_async_fn_signature(name, f);
        let first_state = self.gen_expr(plan.first_await_expr)?;
        let second_state = self.gen_expr(plan.second_await_expr)?;
        let async_state = self.new_tmp();
        self.ins(&format!("{async_state} = alloc {}", plan.async_state_size()));
        self.ins(&format!("store {async_state}+0, 0 as u64"));
        self.adopt_store(&async_state.clone(), 8, &first_state);
        self.adopt_store(&async_state.clone(), 16, &second_state);
        self.ins(&format!("store {async_state}+24, 0 as u64"));
        self.finish_async_state(&async_state, &async_two_vt(name), f)
    }

    // -- join2 await ---------------------------------------------------------

    /// codegen.zig:8819 `emitAsyncJoin2AwaitPollHelper`.
    fn emit_async_join2_await_poll_helper(
        &mut self,
        name: &str,
        plan: &AsyncJoin2AwaitContinuationPlan,
    ) -> Result<()> {
        let vt_name = async_join2_vt(name);
        let poll_name = async_join2_poll(name);
        self.w(&tpl(
            "@const %VT% = vtable { poll = @%POLL% }\n\
             @%POLL%(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   async_stage = load data_slot+0 as u64\n\
             \x20   async_done = eq async_stage, 1\n\
             \x20   br async_done -> L_ASYNC_JOIN2_AWAIT_EMPTY, L_ASYNC_JOIN2_AWAIT_POLL\n\
             L_ASYNC_JOIN2_AWAIT_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_JOIN2_AWAIT_DONE\n\
             L_ASYNC_JOIN2_AWAIT_POLL:\n\
             \x20   async_join_state = load data_slot+8 as ptr\n\
             \x20   async_join_vt = &SLA_JOIN2_FUTURE_VT\n\
             \x20   EXPAND FUTURE_NEW async_join_future, async_join_state, async_join_vt\n\
             \x20   async_join_ctx = 0\n\
             \x20   EXPAND FUTURE_POLL async_join_poll, async_join_future, async_join_ctx\n\
             \x20   EXPAND POLL_IS_READY async_join_ready, async_join_poll\n\
             \x20   br async_join_ready -> L_ASYNC_JOIN2_AWAIT_READY, L_ASYNC_JOIN2_AWAIT_PENDING\n\
             L_ASYNC_JOIN2_AWAIT_PENDING:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_JOIN2_AWAIT_CLEAN\n\
             L_ASYNC_JOIN2_AWAIT_READY:\n",
            &[("%VT%", &vt_name), ("%POLL%", &poll_name)],
        ));
        let bind = &plan.binding_name;
        self.ins(&format!("{bind} = load async_join_poll+8 as ptr"));
        self.ins("async_pair_left = 0");
        self.ins("async_pair_right = 0");
        self.ins(&format!("EXPAND FUTURE_PAIR_LEFT async_pair_left, {bind}"));
        self.ins(&format!("EXPAND FUTURE_PAIR_RIGHT async_pair_right, {bind}"));
        let scalar = AsyncContinuationScalarPlan {
            awaited_coeff: plan.scalar.right_coeff,
            captured_coeff: plan.scalar.left_coeff,
            captured2_coeff: 0,
            captured_field_name: None,
            captured2_field_name: None,
            immediate: plan.scalar.immediate,
        };
        self.emit_async_continuation_scalar_value(
            &scalar,
            "async_join_result",
            "async_pair_right",
            [Some("async_pair_left"), None],
            "async_join",
        )?;
        self.ins("store data_slot+0, 1 as u64");
        self.ins("EXPAND POLL_SET_READY out_poll_slot, async_join_result");
        self.ins("!async_join_result");
        self.ins("!async_pair_right");
        self.ins("!async_pair_left");
        self.ins(&format!("!{bind}"));
        self.w(
            "L_ASYNC_JOIN2_AWAIT_CLEAN:\n\
             \x20   !async_join_ready\n\
             \x20   !async_join_poll\n\
             \x20   !async_join_ctx\n\
             \x20   !async_join_future\n\
             \x20   !async_join_vt\n\
             \x20   !async_join_state\n\
             L_ASYNC_JOIN2_AWAIT_DONE:\n\
             \x20   !async_done\n\
             \x20   !async_stage\n\
             \x20   !data_slot\n\
             \x20   !ctx_slot\n\
             \x20   !out_poll_slot\n\
             \x20   return\n\n",
        );
        Ok(())
    }

    /// codegen.zig:9369 `genAsyncJoin2AwaitFuncDeclNamed`.
    pub(crate) fn gen_async_join2_await_func_decl_named(
        &mut self,
        name: &str,
        f: &FuncDecl,
        plan: &AsyncJoin2AwaitContinuationPlan,
    ) -> Result<()> {
        self.emit_async_join2_await_poll_helper(name, plan)?;
        self.emit_async_fn_signature(name, f);
        for stmt in &f.body[..plan.prefix_len] {
            self.gen_stmt(stmt)?;
        }
        let join_state = self.gen_expr(plan.await_expr)?;
        let async_state = self.new_tmp();
        self.ins(&format!("{async_state} = alloc {}", plan.async_state_size()));
        self.ins(&format!("store {async_state}+0, 0 as u64"));
        self.adopt_store(&async_state.clone(), 8, &join_state);
        self.finish_async_state(&async_state, &async_join2_vt(name), f)
    }
}

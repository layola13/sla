//! Async linear/general/loop plan emission, mirrored from
//! codegen.zig: emitAsyncLinearAwaitPollHelper (8697),
//! genAsyncLinearAwaitFuncDeclNamed (8938),
//! emitAsyncGeneralAwaitPollHelper (9201),
//! genAsyncGeneralAwaitFuncDeclNamed (8980),
//! emitAsyncLoopAwaitPollHelper (9046) with its load/store/free
//! local helpers (9022-9044), genAsyncLoopAwaitFuncDeclNamed
//! (9160). Stage 6 async cluster.

use sla_ast::FuncDecl;
use sla_lowering::{
    AsyncGeneralAwaitContinuationPlan, AsyncLinearAwaitContinuationPlan,
    AsyncLoopAwaitContinuationPlan,
};

use crate::async_plans_codegen::tpl;
use crate::async_scalar::{
    async_general_poll, async_general_vt, async_linear_poll, async_linear_vt, async_loop_poll,
    async_loop_vt,
};
use crate::codegen::Codegen;
use crate::Result;

impl<'c> Codegen<'c> {
    // -- linear await ------------------------------------------------------

    /// codegen.zig:8697 `emitAsyncLinearAwaitPollHelper`.
    fn emit_async_linear_await_poll_helper(
        &mut self,
        name: &str,
        plan: &AsyncLinearAwaitContinuationPlan,
    ) -> Result<()> {
        let vt_name = async_linear_vt(name);
        let poll_name = async_linear_poll(name);
        self.w(&tpl(
            "@const %VT% = vtable { poll = @%POLL% }\n\
             @%POLL%(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   async_stage = load data_slot+0 as u64\n",
            &[("%VT%", &vt_name), ("%POLL%", &poll_name)],
        ));
        self.ins(&format!("async_done = eq async_stage, {}", plan.count));
        self.w("    br async_done -> L_ASYNC_LINEAR_AWAIT_EMPTY, L_ASYNC_LINEAR_AWAIT_DISPATCH\n");
        self.wl("L_ASYNC_LINEAR_AWAIT_DISPATCH:");
        for s in 0..plan.count {
            self.ins(&format!("async_linear_is_{s} = eq async_stage, {s}"));
            self.w(&format!(
                "    br async_linear_is_{s} -> L_ASYNC_LINEAR_AWAIT_STAGE_{s}, L_ASYNC_LINEAR_AWAIT_NEXT_{s}\n"
            ));
            self.wl(&format!("L_ASYNC_LINEAR_AWAIT_NEXT_{s}:"));
        }
        self.w(
            "\x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_LINEAR_AWAIT_DONE\n\
             L_ASYNC_LINEAR_AWAIT_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_LINEAR_AWAIT_DONE\n",
        );
        for s in 0..plan.count {
            let sub_off = plan.sub_state_offset(s);
            let val_off = plan.value_offset(s);
            let bind_name = &plan.binding_names[s];
            self.wl(&format!("L_ASYNC_LINEAR_AWAIT_STAGE_{s}:"));
            self.ins(&format!(
                "async_linear_sub_{s} = load data_slot+{sub_off} as ptr"
            ));
            self.ins(&format!(
                "async_linear_st_{s} = load async_linear_sub_{s}+0 as u64"
            ));
            self.ins(&format!("async_linear_init_{s} = eq async_linear_st_{s}, 0"));
            self.w(&format!(
                "    br async_linear_init_{s} -> L_ASYNC_LINEAR_AWAIT_PENDING_{s}, L_ASYNC_LINEAR_AWAIT_CHECK_{s}\n"
            ));
            self.wl(&format!("L_ASYNC_LINEAR_AWAIT_PENDING_{s}:"));
            self.ins(&format!("store async_linear_sub_{s}+0, 1 as u64"));
            self.ins("EXPAND POLL_SET_PENDING out_poll_slot");
            self.w(&format!("    jmp L_ASYNC_LINEAR_AWAIT_CLEAN_{s}\n"));
            self.wl(&format!("L_ASYNC_LINEAR_AWAIT_CHECK_{s}:"));
            self.ins(&format!(
                "async_linear_ready_{s} = eq async_linear_st_{s}, 1"
            ));
            self.w(&format!(
                "    br async_linear_ready_{s} -> L_ASYNC_LINEAR_AWAIT_READY_{s}, L_ASYNC_LINEAR_AWAIT_SUBEMPTY_{s}\n"
            ));
            self.wl(&format!("L_ASYNC_LINEAR_AWAIT_SUBEMPTY_{s}:"));
            self.ins("EXPAND POLL_SET_PENDING out_poll_slot");
            self.ins(&format!("!async_linear_ready_{s}"));
            self.w(&format!("    jmp L_ASYNC_LINEAR_AWAIT_CLEAN_{s}\n"));
            self.wl(&format!("L_ASYNC_LINEAR_AWAIT_READY_{s}:"));
            self.ins(&format!("{bind_name} = load async_linear_sub_{s}+8 as u64"));
            self.ins(&format!("store data_slot+{val_off}, {bind_name} as u64"));
            self.ins(&format!("store async_linear_sub_{s}+0, 2 as u64"));
            self.ins(&format!("store data_slot+0, {} as u64", s + 1));
            self.ins(&format!("!{bind_name}"));
            self.ins(&format!("!async_linear_ready_{s}"));
            self.ins(&format!("!async_linear_init_{s}"));
            self.ins(&format!("!async_linear_st_{s}"));
            self.ins(&format!("!async_linear_sub_{s}"));
            if s + 1 < plan.count {
                self.w(&format!("    jmp L_ASYNC_LINEAR_AWAIT_STAGE_{}\n", s + 1));
            } else {
                self.emit_async_linear_await_scalar_ready(plan)?;
            }
            self.wl(&format!("L_ASYNC_LINEAR_AWAIT_CLEAN_{s}:"));
            self.ins(&format!("!async_linear_init_{s}"));
            self.ins(&format!("!async_linear_st_{s}"));
            self.ins(&format!("!async_linear_sub_{s}"));
            self.w("    jmp L_ASYNC_LINEAR_AWAIT_DONE\n");
        }
        self.wl("L_ASYNC_LINEAR_AWAIT_DONE:");
        for s in 0..plan.count {
            self.ins(&format!("!async_linear_is_{s}"));
        }
        self.ins("!async_done");
        self.ins("!async_stage");
        self.ins("!data_slot");
        self.ins("!ctx_slot");
        self.ins("!out_poll_slot");
        self.ins("return");
        self.w("\n");
        Ok(())
    }

    /// codegen.zig:8938 `genAsyncLinearAwaitFuncDeclNamed`.
    pub(crate) fn gen_async_linear_await_func_decl_named(
        &mut self,
        name: &str,
        f: &FuncDecl,
        plan: &AsyncLinearAwaitContinuationPlan,
    ) -> Result<()> {
        self.emit_async_linear_await_poll_helper(name, plan)?;
        self.emit_async_fn_signature(name, f);
        let mut sub_states: Vec<String> = Vec::new();
        for await_expr in &plan.await_exprs {
            sub_states.push(self.gen_expr(await_expr)?);
        }
        let async_state = self.new_tmp();
        self.ins(&format!("{async_state} = alloc {}", plan.async_state_size()));
        self.ins(&format!("store {async_state}+0, 0 as u64"));
        for (i, sub_state) in sub_states.iter().enumerate() {
            self.adopt_store(&async_state.clone(), plan.sub_state_offset(i), sub_state);
            self.ins(&format!(
                "store {async_state}+{}, 0 as u64",
                plan.value_offset(i)
            ));
        }
        self.finish_async_state(&async_state, &async_linear_vt(name), f)
    }

    // -- general await -------------------------------------------------------

    /// codegen.zig:9201 `emitAsyncGeneralAwaitPollHelper`.
    fn emit_async_general_await_poll_helper(
        &mut self,
        name: &str,
        plan: &AsyncGeneralAwaitContinuationPlan,
    ) -> Result<()> {
        let vt_name = async_general_vt(name);
        let poll_name = async_general_poll(name);
        self.w(&tpl(
            "@const %VT% = vtable { poll = @%POLL% }\n\
             @%POLL%(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   async_stage = load data_slot+0 as u64\n",
            &[("%VT%", &vt_name), ("%POLL%", &poll_name)],
        ));
        self.ins(&format!("async_done = eq async_stage, {}", plan.count));
        self.w("    br async_done -> L_ASYNC_GENERAL_AWAIT_EMPTY, L_ASYNC_GENERAL_AWAIT_DISPATCH\n");
        self.wl("L_ASYNC_GENERAL_AWAIT_DISPATCH:");
        for s in 0..plan.count {
            self.ins(&format!("async_general_is_{s} = eq async_stage, {s}"));
            self.w(&format!(
                "    br async_general_is_{s} -> L_ASYNC_GENERAL_AWAIT_STAGE_{s}, L_ASYNC_GENERAL_AWAIT_NEXT_{s}\n"
            ));
            self.wl(&format!("L_ASYNC_GENERAL_AWAIT_NEXT_{s}:"));
        }
        self.w(
            "\x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_GENERAL_AWAIT_DONE\n\
             L_ASYNC_GENERAL_AWAIT_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_GENERAL_AWAIT_DONE\n",
        );
        for s in 0..plan.count {
            let obj_off = plan.object_offset(s);
            let val_off = plan.value_offset(s);
            let bind_name = &plan.binding_names[s];
            self.wl(&format!("L_ASYNC_GENERAL_AWAIT_STAGE_{s}:"));
            self.ins(&format!(
                "async_general_obj_{s} = load data_slot+{obj_off} as ptr"
            ));
            self.ins(&format!("async_general_ctx_{s} = 0"));
            self.ins(&format!(
                "EXPAND FUTURE_POLL async_general_poll_{s}, async_general_obj_{s}, async_general_ctx_{s}"
            ));
            self.ins(&format!(
                "EXPAND POLL_IS_READY async_general_ready_{s}, async_general_poll_{s}"
            ));
            self.w(&format!(
                "    br async_general_ready_{s} -> L_ASYNC_GENERAL_AWAIT_READY_{s}, L_ASYNC_GENERAL_AWAIT_SUBPENDING_{s}\n"
            ));
            self.wl(&format!("L_ASYNC_GENERAL_AWAIT_SUBPENDING_{s}:"));
            self.ins("EXPAND POLL_SET_PENDING out_poll_slot");
            self.ins(&format!("!async_general_ready_{s}"));
            self.ins(&format!("!async_general_poll_{s}"));
            self.ins(&format!("!async_general_ctx_{s}"));
            self.ins(&format!("!async_general_obj_{s}"));
            self.w(&format!("    jmp L_ASYNC_GENERAL_AWAIT_CLEAN_{s}\n"));
            self.wl(&format!("L_ASYNC_GENERAL_AWAIT_READY_{s}:"));
            self.ins(&format!(
                "{bind_name} = load async_general_poll_{s}+8 as u64"
            ));
            self.ins(&format!("store data_slot+{val_off}, {bind_name} as u64"));
            if s + 1 < plan.count {
                let next_state = self.gen_expr(plan.await_exprs[s + 1])?;
                let next_obj = self.gen_future_object_for_state(&next_state)?;
                self.adopt_store("data_slot", plan.object_offset(s + 1), &next_obj);
                self.ins(&format!("store data_slot+0, {} as u64", s + 1));
                self.ins(&format!("!{bind_name}"));
                self.ins(&format!("!async_general_ready_{s}"));
                self.ins(&format!("!async_general_poll_{s}"));
                self.ins(&format!("!async_general_obj_{s}"));
                self.ins(&format!("!async_general_ctx_{s}"));
                self.w(&format!("    jmp L_ASYNC_GENERAL_AWAIT_STAGE_{}\n", s + 1));
            } else {
                self.emit_async_general_await_scalar_ready(plan)?;
                self.ins(&format!("!{bind_name}"));
                self.ins(&format!("!async_general_ready_{s}"));
                self.ins(&format!("!async_general_poll_{s}"));
                self.ins(&format!("!async_general_obj_{s}"));
                self.ins(&format!("!async_general_ctx_{s}"));
            }
            self.wl(&format!("L_ASYNC_GENERAL_AWAIT_CLEAN_{s}:"));
            self.w("    jmp L_ASYNC_GENERAL_AWAIT_DONE\n");
        }
        self.wl("L_ASYNC_GENERAL_AWAIT_DONE:");
        for s in 0..plan.count {
            self.ins(&format!("!async_general_is_{s}"));
        }
        self.ins("!async_done");
        self.ins("!async_stage");
        self.ins("!data_slot");
        self.ins("!ctx_slot");
        self.ins("!out_poll_slot");
        self.ins("return");
        self.w("\n");
        Ok(())
    }

    /// codegen.zig:8980 `genAsyncGeneralAwaitFuncDeclNamed`.
    pub(crate) fn gen_async_general_await_func_decl_named(
        &mut self,
        name: &str,
        f: &FuncDecl,
        plan: &AsyncGeneralAwaitContinuationPlan,
    ) -> Result<()> {
        self.emit_async_general_await_poll_helper(name, plan)?;
        self.emit_async_fn_signature(name, f);
        for stmt in &f.body[..plan.prefix_len] {
            self.gen_stmt(stmt)?;
        }
        let first_state = self.gen_expr(plan.await_exprs[0])?;
        let first_obj = self.gen_future_object_for_state(&first_state)?;
        let async_state = self.new_tmp();
        self.ins(&format!("{async_state} = alloc {}", plan.async_state_size()));
        self.ins(&format!("store {async_state}+0, 0 as u64"));
        self.adopt_store(&async_state.clone(), plan.object_offset(0), &first_obj);
        for zi in 0..plan.count {
            self.ins(&format!(
                "store {async_state}+{}, 0 as u64",
                plan.value_offset(zi)
            ));
        }
        self.finish_async_state(&async_state, &async_general_vt(name), f)
    }

    // -- loop await ------------------------------------------------------------

    /// codegen.zig:9022 `emitLoopAwaitLoadLocals`.
    fn emit_loop_await_load_locals(
        &mut self,
        plan: &AsyncLoopAwaitContinuationPlan,
        from: usize,
        to: usize,
    ) {
        for i in from..to {
            let local = &plan.locals[i];
            self.ins(&format!(
                "{} = load data_slot+{} as {}",
                local.name,
                AsyncLoopAwaitContinuationPlan::local_offset(i),
                if local.is_ptr { "ptr" } else { "u64" }
            ));
        }
    }

    /// codegen.zig:9032 `emitLoopAwaitStoreLocals`.
    fn emit_loop_await_store_locals(
        &mut self,
        plan: &AsyncLoopAwaitContinuationPlan,
        from: usize,
        to: usize,
    ) {
        for i in from..to {
            let local = &plan.locals[i];
            self.ins(&format!(
                "store data_slot+{}, {} as {}",
                AsyncLoopAwaitContinuationPlan::local_offset(i),
                local.name,
                if local.is_ptr { "ptr" } else { "u64" }
            ));
        }
    }

    /// Build-site variant of store+free locals: pointer locals
    /// are adopted by the state via move store (a plain store +
    /// release frees a fresh owning value on the pinned host —
    /// see adopt_store); scalar locals keep the plain store +
    /// release pair.
    fn emit_loop_await_store_locals_adopt(
        &mut self,
        plan: &AsyncLoopAwaitContinuationPlan,
        from: usize,
        to: usize,
    ) {
        for i in from..to {
            let local = &plan.locals[i];
            if local.is_ptr {
                self.adopt_store(
                    "data_slot",
                    AsyncLoopAwaitContinuationPlan::local_offset(i),
                    &local.name.clone(),
                );
            } else {
                self.ins(&format!(
                    "store data_slot+{}, {} as u64",
                    AsyncLoopAwaitContinuationPlan::local_offset(i),
                    local.name
                ));
                self.ins(&format!("!{}", local.name));
            }
        }
    }

    /// codegen.zig:9042 `emitLoopAwaitFreeLocals`.
    fn emit_loop_await_free_locals(
        &mut self,
        plan: &AsyncLoopAwaitContinuationPlan,
        from: usize,
        to: usize,
        skip: Option<&str>,
    ) {
        for i in from..to {
            let local = &plan.locals[i];
            if let Some(skip_name) = skip {
                if skip_name == local.name {
                    continue;
                }
            }
            self.ins(&format!("!{}", local.name));
        }
    }

    /// codegen.zig:9046 `emitAsyncLoopAwaitPollHelper`.
    fn emit_async_loop_await_poll_helper(
        &mut self,
        name: &str,
        plan: &AsyncLoopAwaitContinuationPlan,
    ) -> Result<()> {
        let vt_name = async_loop_vt(name);
        let poll_name = async_loop_poll(name);
        self.w(&tpl(
            "@const %VT% = vtable { poll = @%POLL% }\n\
             @%POLL%(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   async_loop_stage = load data_slot+0 as u64\n\
             \x20   async_loop_done = eq async_loop_stage, 2\n\
             \x20   async_loop_awaiting = eq async_loop_stage, 1\n\
             \x20   br async_loop_done -> L_ASYNC_LOOP_EMPTY, L_ASYNC_LOOP_DISPATCH\n\
             L_ASYNC_LOOP_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_ASYNC_LOOP_DONE\n\
             L_ASYNC_LOOP_DISPATCH:\n\
             \x20   br async_loop_awaiting -> L_ASYNC_LOOP_AWAIT, L_ASYNC_LOOP_HEAD\n\
             L_ASYNC_LOOP_HEAD:\n",
            &[("%VT%", &vt_name), ("%POLL%", &poll_name)],
        ));
        self.emit_loop_await_load_locals(plan, 0, plan.head_local_count);
        let cond_reg = self.gen_expr(plan.cond_expr)?;
        self.w(&format!(
            "    br {cond_reg} -> L_ASYNC_LOOP_BODY, L_ASYNC_LOOP_FINISH\n"
        ));
        let cond_is_tmp = cond_reg.starts_with("tmp_");
        self.wl("L_ASYNC_LOOP_BODY:");
        if cond_is_tmp {
            self.ins(&format!("!{cond_reg}"));
        }
        for stmt in plan.pre_stmts {
            self.gen_stmt(stmt)?;
        }
        let sub_state = self.gen_expr(plan.await_expr)?;
        let sub_obj = self.gen_future_object_for_state(&sub_state)?;
        self.adopt_store("data_slot", 8, &sub_obj);
        self.ins("store data_slot+0, 1 as u64");
        self.emit_loop_await_store_locals_adopt(plan, 0, plan.binding_index());
        self.w("    jmp L_ASYNC_LOOP_AWAIT\n");
        self.w(
            "L_ASYNC_LOOP_AWAIT:\n\
             \x20   async_loop_obj = load data_slot+8 as ptr\n\
             \x20   async_loop_ctx = 0\n\
             \x20   EXPAND FUTURE_POLL async_loop_poll, async_loop_obj, async_loop_ctx\n\
             \x20   EXPAND POLL_IS_READY async_loop_ready, async_loop_poll\n\
             \x20   br async_loop_ready -> L_ASYNC_LOOP_SUBREADY, L_ASYNC_LOOP_SUBPENDING\n\
             L_ASYNC_LOOP_SUBPENDING:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   !async_loop_ready\n\
             \x20   !async_loop_poll\n\
             \x20   !async_loop_ctx\n\
             \x20   !async_loop_obj\n\
             \x20   jmp L_ASYNC_LOOP_DONE\n\
             L_ASYNC_LOOP_SUBREADY:\n",
        );
        self.ins("async_loop_value = load async_loop_poll+8 as u64");
        self.ins(&format!(
            "store data_slot+{}, async_loop_value as u64",
            AsyncLoopAwaitContinuationPlan::local_offset(plan.binding_index())
        ));
        self.w(
            "\x20   !async_loop_value\n\
             \x20   !async_loop_ready\n\
             \x20   !async_loop_poll\n\
             \x20   !async_loop_ctx\n\
             \x20   !async_loop_obj\n",
        );
        self.emit_loop_await_load_locals(plan, 0, plan.locals.len());
        for stmt in plan.post_stmts {
            self.gen_stmt(stmt)?;
        }
        self.emit_loop_await_store_locals(plan, 0, plan.head_local_count);
        self.ins("store data_slot+0, 0 as u64");
        self.emit_loop_await_free_locals(plan, 0, plan.locals.len(), None);
        for let_name in &plan.post_let_names {
            self.ins(&format!("!{let_name}"));
        }
        self.w("    jmp L_ASYNC_LOOP_HEAD\n");
        self.wl("L_ASYNC_LOOP_FINISH:");
        if cond_is_tmp {
            self.ins(&format!("!{cond_reg}"));
        }
        let result_reg = self.gen_expr(plan.ret_expr)?;
        self.ins("store data_slot+0, 2 as u64");
        self.ins(&format!("EXPAND POLL_SET_READY out_poll_slot, {result_reg}"));
        if result_reg.starts_with("tmp_") {
            self.ins(&format!("!{result_reg}"));
        }
        self.emit_loop_await_free_locals(plan, 0, plan.head_local_count, Some(&result_reg));
        self.w(
            "\x20   jmp L_ASYNC_LOOP_DONE\n\
             L_ASYNC_LOOP_DONE:\n\
             \x20   !async_loop_awaiting\n\
             \x20   !async_loop_done\n\
             \x20   !async_loop_stage\n\
             \x20   !data_slot\n\
             \x20   !ctx_slot\n\
             \x20   !out_poll_slot\n\
             \x20   return\n\n",
        );
        Ok(())
    }

    /// codegen.zig:9160 `genAsyncLoopAwaitFuncDeclNamed`.
    pub(crate) fn gen_async_loop_await_func_decl_named(
        &mut self,
        name: &str,
        f: &FuncDecl,
        plan: &AsyncLoopAwaitContinuationPlan,
    ) -> Result<()> {
        self.emit_async_loop_await_poll_helper(name, plan)?;
        self.emit_async_fn_signature(name, f);
        for stmt in &f.body[..plan.prefix_len] {
            self.gen_stmt(stmt)?;
        }
        let async_state = self.new_tmp();
        self.ins(&format!("{async_state} = alloc {}", plan.async_state_size()));
        self.ins(&format!("store {async_state}+0, 0 as u64"));
        self.ins(&format!("store {async_state}+8, 0 as u64"));
        for i in 0..plan.locals.len() {
            let local = &plan.locals[i];
            if i < plan.head_local_count {
                if local.is_ptr {
                    // Pointer head locals are adopted by the state
                    // (move store; see adopt_store).
                    self.adopt_store(
                        &async_state.clone(),
                        AsyncLoopAwaitContinuationPlan::local_offset(i),
                        &local.name.clone(),
                    );
                } else {
                    self.ins(&format!(
                        "store {async_state}+{}, {} as u64",
                        AsyncLoopAwaitContinuationPlan::local_offset(i),
                        local.name
                    ));
                    // LINE-CENSUS-MEMLEAK: a scalar head local's
                    // register is dead once its value lives in
                    // the state slot (the poll helper's
                    // store-locals-adopt arm pairs the same store
                    // with `!name`). Without this, prefix locals
                    // of a loop-await async fn stayed Active in
                    // the constructor (MemoryLeak 1012,
                    // `deadline` in @sla__delay_ms via
                    // test_unit_async_await_loop_future). Params
                    // are exempt: finish_async_state releases
                    // them.
                    if !f.params.iter().any(|p| p.name == local.name) {
                        self.ins(&format!("!{}", local.name));
                    }
                }
            } else {
                self.ins(&format!(
                    "store {async_state}+{}, 0 as u64",
                    AsyncLoopAwaitContinuationPlan::local_offset(i)
                ));
            }
        }
        self.finish_async_state(&async_state, &async_loop_vt(name), f)
    }
}

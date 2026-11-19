//! Future/task/executor/poll runtime-call emission and the await
//! expression, mirrored from codegen.zig (genFutureRuntimeCall
//! 8206, genTaskRuntimeCall 8268, genPollRuntimeCall 9420,
//! genExecutorRuntimeCall 9450, genFutureObjectForState 8172,
//! genReadyFutureI64 8115, the await_expr arm of genExpr 13253,
//! emitFutureTaskHelpers 6822). Stage 6 async cluster.

use sla_ast::{CallExpr, Node};
use sla_lowering::{
    call_arg_needs_release, executor_task_buffer_plan, plan_await_future_with_readiness,
    plan_executor_runtime_call, plan_future_runtime_call, plan_poll_runtime_call,
    plan_task_runtime_call, await_expr_future_is_custom, ExecutorRuntimeCallPlan,
    FutureReadiness, FutureRuntimeCallPlan, PollRuntimeCallPlan, TaskRuntimeCallPlan,
};

use crate::codegen::Codegen;
use crate::{unsupported, Result};

pub(crate) const READY_FUTURE_VT: &str = "SLA_READY_FUTURE_VT";
pub(crate) const DEFER_READY_FUTURE_VT: &str = "SLA_DEFER_READY_FUTURE_VT";
pub(crate) const JOIN2_FUTURE_VT: &str = "SLA_JOIN2_FUTURE_VT";
pub(crate) const SELECT2_FUTURE_VT: &str = "SLA_SELECT2_FUTURE_VT";

impl<'c> Codegen<'c> {
    // -- future state constructors (codegen.zig:8115-8204) -----------

    pub(crate) fn gen_ready_future_i64(&mut self, value_reg: &str) -> Result<String> {
        let future_reg = self.new_tmp();
        self.ins(&format!(
            "EXPAND FUTURE_READY_STATE_NEW {future_reg}, {value_reg}"
        ));
        self.future_state_vtables
            .insert(future_reg.clone(), READY_FUTURE_VT.to_string());
        self.future_readiness
            .insert(future_reg.clone(), FutureReadiness::Ready);
        Ok(future_reg)
    }

    fn gen_pending_future(&mut self) -> Result<String> {
        let future_reg = self.new_tmp();
        self.ins(&format!("EXPAND FUTURE_PENDING_STATE_NEW {future_reg}"));
        self.future_state_vtables
            .insert(future_reg.clone(), READY_FUTURE_VT.to_string());
        self.future_readiness
            .insert(future_reg.clone(), FutureReadiness::Pending);
        Ok(future_reg)
    }

    fn gen_defer_ready_future_i64(&mut self, value_reg: &str) -> Result<String> {
        let future_reg = self.new_tmp();
        self.ins(&format!("{future_reg} = alloc 16"));
        self.ins(&format!("store {future_reg}+0, 0 as u64"));
        self.ins(&format!("store {future_reg}+8, {value_reg} as u64"));
        self.future_state_vtables
            .insert(future_reg.clone(), DEFER_READY_FUTURE_VT.to_string());
        self.record_future_readiness(&future_reg, FutureReadiness::Unknown);
        Ok(future_reg)
    }

    pub(crate) fn future_readiness_for_state(&self, state_reg: &str) -> FutureReadiness {
        let resolved = self.resolve_binding_name(state_reg);
        if let Some(r) = self.future_readiness.get(&resolved) {
            return *r;
        }
        if let Some(r) = self.future_readiness.get(state_reg) {
            return *r;
        }
        FutureReadiness::Unknown
    }

    pub(crate) fn record_future_readiness(&mut self, state_reg: &str, readiness: FutureReadiness) {
        if readiness == FutureReadiness::Unknown {
            self.future_readiness.remove(state_reg);
            return;
        }
        self.future_readiness
            .insert(state_reg.to_string(), readiness);
    }

    #[allow(dead_code)]
    pub(crate) fn transfer_future_readiness(&mut self, src: &str, dst: &str) {
        if let Some(r) = self.future_readiness.get(src).copied() {
            self.record_future_readiness(dst, r);
            self.future_readiness.remove(src);
            return;
        }
        self.future_readiness.remove(dst);
    }

    /// codegen.zig:1116 `transferResultSlotValueState` (the async
    /// maps): move the vtable / readiness / task-object /
    /// executor-count entries from a value register to the binding
    /// that takes it over.
    pub(crate) fn transfer_future_state(&mut self, src: &str, dst: &str) {
        if src == dst {
            return;
        }
        if let Some(v) = self.task_future_objects.remove(src) {
            self.task_future_objects.insert(dst.to_string(), v);
        }
        if let Some(v) = self.future_state_vtables.remove(src) {
            self.future_state_vtables.insert(dst.to_string(), v);
        }
        if let Some(v) = self.future_readiness.remove(src) {
            self.future_readiness.insert(dst.to_string(), v);
        }
        if let Some(v) = self.executor_task_counts.remove(src) {
            self.executor_task_counts.insert(dst.to_string(), v);
        }
    }

    /// codegen.zig:15392: after a plain call to an async function
    /// whose declaration matches a continuation plan, register the
    /// plan vtable + unknown readiness on the result register.
    pub(crate) fn register_async_call_result(&mut self, reg: &str, decl: &sla_ast::FuncDecl) {
        use sla_lowering::{
            plan_async_general_await_continuation, plan_async_join2_await_continuation,
            plan_async_linear_await_continuation, plan_async_loop_await_continuation,
            plan_async_single_await_continuation, plan_async_two_await_continuation,
        };
        let name = decl.name.as_str();
        let vt = if plan_async_join2_await_continuation(decl).is_some() {
            Some(crate::async_scalar::async_join2_vt(name))
        } else if plan_async_two_await_continuation(decl).is_some() {
            Some(crate::async_scalar::async_two_vt(name))
        } else if plan_async_linear_await_continuation(decl).is_some() {
            Some(crate::async_scalar::async_linear_vt(name))
        } else if plan_async_general_await_continuation(decl).is_some() {
            Some(crate::async_scalar::async_general_vt(name))
        } else if plan_async_loop_await_continuation(decl).is_some() {
            Some(crate::async_scalar::async_loop_vt(name))
        } else if plan_async_single_await_continuation(decl).is_some() {
            Some(crate::async_scalar::async_single_vt(name))
        } else {
            None
        };
        if let Some(vt) = vt {
            self.future_state_vtables.insert(reg.to_string(), vt);
            self.record_future_readiness(reg, FutureReadiness::Unknown);
        }
    }

    pub(crate) fn future_vtable_for_state(&self, state_reg: &str) -> String {
        let resolved = self.resolve_binding_name(state_reg);
        if let Some(vt) = self.future_state_vtables.get(&resolved) {
            return vt.clone();
        }
        if let Some(vt) = self.future_state_vtables.get(state_reg) {
            return vt.clone();
        }
        READY_FUTURE_VT.to_string()
    }

    pub(crate) fn gen_future_object_for_state(&mut self, state_reg: &str) -> Result<String> {
        let vt_reg = self.new_tmp();
        let future_obj = self.new_tmp();
        let vt_name = self.future_vtable_for_state(state_reg);
        self.ins(&format!("{vt_reg} = &{vt_name}"));
        self.ins(&format!(
            "EXPAND FUTURE_NEW {future_obj}, {state_reg}, {vt_reg}"
        ));
        self.emit_release(&vt_reg)?;
        // The state operand is adopted by the future object: it
        // is retired with the FORGET form at wrap time (the
        // loop-defer-ready minim's header: a release-form retire
        // drops the state block before the first poll, and no
        // retire leaks the register at function exit).
        if !state_reg.starts_with('&') {
            self.emit_forget_moved_value(state_reg)?;
        }
        Ok(future_obj)
    }

    fn gen_join2_future(&mut self, left_state: &str, right_state: &str) -> Result<String> {
        let left_future = self.gen_future_object_for_state(left_state)?;
        let right_future = self.gen_future_object_for_state(right_state)?;
        let join_state = self.new_tmp();
        self.ins(&format!(
            "EXPAND FUTURE_JOIN2_STATE_NEW {join_state}, {left_future}, {right_future}"
        ));
        self.future_state_vtables
            .insert(join_state.clone(), JOIN2_FUTURE_VT.to_string());
        let readiness = sla_lowering::join2_readiness(
            self.future_readiness_for_state(left_state),
            self.future_readiness_for_state(right_state),
        );
        self.record_future_readiness(&join_state, readiness);
        // The two future objects are adopted by the join state
        // (the macro stores their pointers); releasing them would
        // free live objects on the pinned SA host (see
        // adopt_store), so they are retired with the FORGET form
        // (`^reg`: consumed, not freed — the retire form the
        // loop-defer-ready minim's header prescribes for
        // FUTURE_NEW operands).
        self.emit_forget_moved_value(&left_future.clone())?;
        self.emit_forget_moved_value(&right_future.clone())?;
        self.consumed_bindings.insert(left_future);
        self.consumed_bindings.insert(right_future);
        Ok(join_state)
    }

    fn gen_select2_future(&mut self, left_state: &str, right_state: &str) -> Result<String> {
        let left_future = self.gen_future_object_for_state(left_state)?;
        let right_future = self.gen_future_object_for_state(right_state)?;
        let select_state = self.new_tmp();
        self.ins(&format!(
            "EXPAND FUTURE_SELECT2_STATE_NEW {select_state}, {left_future}, {right_future}"
        ));
        self.future_state_vtables
            .insert(select_state.clone(), SELECT2_FUTURE_VT.to_string());
        let readiness = sla_lowering::select2_readiness(
            self.future_readiness_for_state(left_state),
            self.future_readiness_for_state(right_state),
        );
        self.record_future_readiness(&select_state, readiness);
        // Adopted by the select state, as in gen_join2_future:
        // forget form, not a freeing release.
        self.emit_forget_moved_value(&left_future.clone())?;
        self.emit_forget_moved_value(&right_future.clone())?;
        self.consumed_bindings.insert(left_future);
        self.consumed_bindings.insert(right_future);
        Ok(select_state)
    }

    // -- runtime calls -------------------------------------------------

    /// codegen.zig:8206 `genFutureRuntimeCall`.
    pub(crate) fn gen_future_runtime_call(
        &mut self,
        plan: FutureRuntimeCallPlan,
        call: &CallExpr,
    ) -> Result<String> {
        if plan.is_ready() {
            if call.args.len() != 1 {
                return unsupported("future::ready arity", "codegen.zig:8206");
            }
            let value_reg = self.gen_expr(&call.args[0])?;
            let future_reg = self.gen_ready_future_i64(&value_reg)?;
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&value_reg)?;
            }
            return Ok(future_reg);
        }
        if plan.is_pending() {
            if !call.args.is_empty() || call.generics.len() != 1 {
                return unsupported("future::pending arity", "codegen.zig:8206");
            }
            return self.gen_pending_future();
        }
        if plan.is_defer_ready() {
            if call.args.len() != 1 || !call.generics.is_empty() {
                return unsupported("future::defer_ready arity", "codegen.zig:8206");
            }
            let value_reg = self.gen_expr(&call.args[0])?;
            let future_reg = self.gen_defer_ready_future_i64(&value_reg)?;
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&value_reg)?;
            }
            return Ok(future_reg);
        }
        if plan.is_join2() {
            if call.args.len() != 2 || !call.generics.is_empty() {
                return unsupported("future::join2 arity", "codegen.zig:8206");
            }
            let left_state = self.gen_expr(&call.args[0])?;
            let right_state = self.gen_expr(&call.args[1])?;
            return self.gen_join2_future(&left_state, &right_state);
        }
        if plan.is_select2() {
            if call.args.len() != 2 || !call.generics.is_empty() {
                return unsupported("future::select2 arity", "codegen.zig:8206");
            }
            let left_state = self.gen_expr(&call.args[0])?;
            let right_state = self.gen_expr(&call.args[1])?;
            return self.gen_select2_future(&left_state, &right_state);
        }
        if plan.is_pair_accessor() {
            if call.args.len() != 1 || !call.generics.is_empty() {
                return unsupported("future pair accessor arity", "codegen.zig:8206");
            }
            let pair_reg = self.gen_expr(&call.args[0])?;
            let value_reg = self.new_tmp();
            let macro_name = plan.pair_macro_name().unwrap();
            self.ins(&format!("EXPAND {macro_name} {value_reg}, {pair_reg}"));
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&pair_reg)?;
            }
            return Ok(value_reg);
        }
        if plan.is_either_accessor() {
            if call.args.len() != 1 || !call.generics.is_empty() {
                return unsupported("future either accessor arity", "codegen.zig:8206");
            }
            let either_reg = self.gen_expr(&call.args[0])?;
            let value_reg = self.new_tmp();
            let macro_name = plan.either_value_macro_name().unwrap();
            self.ins(&format!("EXPAND {macro_name} {value_reg}, {either_reg}"));
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&either_reg)?;
            }
            return Ok(value_reg);
        }
        unsupported("future runtime call kind", "codegen.zig:8206")
    }

    /// codegen.zig:8268 `genTaskRuntimeCall`.
    pub(crate) fn gen_task_runtime_call(
        &mut self,
        plan: TaskRuntimeCallPlan,
        call: &CallExpr,
    ) -> Result<String> {
        if plan.is_new() {
            if call.args.len() != 1 {
                return unsupported("task::new arity", "codegen.zig:8268");
            }
            let state_reg = self.gen_expr(&call.args[0])?;
            let ctx = self.new_tmp();
            let task = self.new_tmp();
            let future_obj = self.gen_future_object_for_state(&state_reg)?;
            self.ins(&format!("{ctx} = 0"));
            self.ins(&format!("EXPAND TASK_NEW {task}, {future_obj}, {ctx}"));
            // L4R2 Root A: TASK_NEW adopts the future object, so
            // its register is retired with the forget form at wrap
            // time — the same discipline gen_join2_future /
            // gen_select2_future apply to their FUTURE_*_NEW
            // operands, and gen_future_object_for_state applies to
            // the state operand. Without it the object register
            // stayed Active whenever the task itself was consumed
            // by a move (e.g. VEC_PUSH into an executor's queue),
            // because the task-release companion path in
            // emit_release only fires on a freeing release of the
            // task register. The consumed-bindings guard there
            // keeps this forget from double-retiring on the
            // release path.
            self.emit_forget_moved_value(&future_obj)?;
            self.emit_release(&ctx)?;
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&state_reg)?;
            }
            self.task_future_objects.insert(task.clone(), future_obj);
            return Ok(task);
        }
        if plan.is_poll() {
            if call.args.len() != 1 {
                return unsupported("task::poll arity", "codegen.zig:8268");
            }
            let task_reg = self.gen_expr(&call.args[0])?;
            let poll_reg = self.new_tmp();
            let tag_reg = self.new_tmp();
            let ready_reg = self.new_tmp();
            self.ins(&format!("EXPAND TASK_POLL {poll_reg}, {task_reg}"));
            self.ins(&format!("{tag_reg} = load {poll_reg}+Poll_tag as u64"));
            self.ins(&format!("{ready_reg} = eq {tag_reg}, Poll_READY"));
            self.emit_release(&tag_reg)?;
            self.emit_release(&poll_reg)?;
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&task_reg)?;
            }
            return Ok(ready_reg);
        }
        if plan.is_is_ready() {
            if call.args.len() != 1 {
                return unsupported("task::is_ready arity", "codegen.zig:8268");
            }
            let task_reg = self.gen_expr(&call.args[0])?;
            let ready_reg = self.new_tmp();
            self.ins(&format!("EXPAND TASK_IS_READY {ready_reg}, {task_reg}"));
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&task_reg)?;
            }
            return Ok(ready_reg);
        }
        if plan.is_result() {
            if call.args.len() != 1 {
                return unsupported("task::result arity", "codegen.zig:8268");
            }
            let task_reg = self.gen_expr(&call.args[0])?;
            let value_reg = self.new_tmp();
            self.ins(&format!("EXPAND TASK_RESULT {value_reg}, {task_reg}"));
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&task_reg)?;
            }
            return Ok(value_reg);
        }
        if plan.is_state() {
            if call.args.len() != 1 {
                return unsupported("task::state arity", "codegen.zig:8268");
            }
            let task_reg = self.gen_expr(&call.args[0])?;
            let state_reg = self.new_tmp();
            self.ins(&format!("EXPAND TASK_STATE {state_reg}, {task_reg}"));
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&task_reg)?;
            }
            return Ok(state_reg);
        }
        unsupported("task runtime call kind", "codegen.zig:8268")
    }

    /// codegen.zig:9408 `genReadyPoll`.
    fn gen_ready_poll(&mut self, value_reg: &str) -> Result<String> {
        let poll_reg = self.new_tmp();
        self.ins(&format!("EXPAND POLL_READY {poll_reg}, {value_reg}"));
        Ok(poll_reg)
    }

    /// codegen.zig:9414 `genPendingPoll`.
    fn gen_pending_poll(&mut self) -> Result<String> {
        let poll_reg = self.new_tmp();
        self.ins(&format!("EXPAND POLL_PENDING {poll_reg}"));
        Ok(poll_reg)
    }

    /// codegen.zig:9420 `genPollRuntimeCall`. `Ok(None)` when the
    /// call is not a poll runtime call.
    pub(crate) fn gen_poll_runtime_call(
        &mut self,
        call: &CallExpr,
    ) -> Result<Option<String>> {
        let plan: PollRuntimeCallPlan = match plan_poll_runtime_call(call) {
            Some(p) => p,
            None => return Ok(None),
        };
        if plan.is_ready() {
            if call.args.len() != 1 || !call.generics.is_empty() {
                return unsupported("poll::ready arity", "codegen.zig:9420");
            }
            let value_reg = self.gen_expr(&call.args[0])?;
            let poll_reg = self.gen_ready_poll(&value_reg)?;
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&value_reg)?;
            }
            return Ok(Some(poll_reg));
        }
        if plan.is_pending() {
            if !call.args.is_empty() || call.generics.len() != 1 {
                return unsupported("poll::pending arity", "codegen.zig:9420");
            }
            return Ok(Some(self.gen_pending_poll()?));
        }
        if plan.is_status_check() {
            if call.args.len() != 1 || !call.generics.is_empty() {
                return unsupported("poll status check arity", "codegen.zig:9420");
            }
            let poll_reg = self.gen_expr(&call.args[0])?;
            let out_reg = self.new_tmp();
            let macro_name = plan.poll_status_macro_name().unwrap();
            self.ins(&format!("EXPAND {macro_name} {out_reg}, {poll_reg}"));
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&poll_reg)?;
            }
            return Ok(Some(out_reg));
        }
        if plan.is_value() {
            if call.args.len() != 1 || !call.generics.is_empty() {
                return unsupported("poll::value arity", "codegen.zig:9420");
            }
            let poll_reg = self.gen_expr(&call.args[0])?;
            let value_reg = self.new_tmp();
            self.ins(&format!("EXPAND POLL_VALUE {value_reg}, {poll_reg}"));
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&poll_reg)?;
            }
            return Ok(Some(value_reg));
        }
        unsupported("poll runtime call kind", "codegen.zig:9420")
    }

    /// codegen.zig:9450 `genExecutorRuntimeCall`. `Ok(None)` when
    /// the call is not an executor runtime call.
    pub(crate) fn gen_executor_runtime_call(
        &mut self,
        call: &CallExpr,
    ) -> Result<Option<String>> {
        let plan: ExecutorRuntimeCallPlan = match plan_executor_runtime_call(call) {
            Some(p) => p,
            None => return Ok(None),
        };
        if plan.is_new() {
            if call.args.len() != 1 || !call.generics.is_empty() {
                return unsupported("executor::new arity", "codegen.zig:9450");
            }
            let tasks_ty = self
                .expr_type(&call.args[0])
                .ok_or(crate::TextError::MissingType {
                    context: "executor tasks type".to_string(),
                })?;
            let tasks_plan = match executor_task_buffer_plan(&tasks_ty) {
                Some(p) => p,
                None => {
                    return unsupported(
                        "executor task buffer shape",
                        "lowering_rules.zig:4662 (executorTaskBufferPlan)",
                    )
                }
            };
            let tasks_owner_reg = self.gen_expr(&call.args[0])?;
            let mut tasks_ptr_reg = tasks_owner_reg.clone();
            let mut release_tasks_ptr = false;
            let len_reg = self.new_tmp();
            let executor_reg = self.new_tmp();
            if tasks_plan.is_fixed_array() {
                self.emit_int_const(&len_reg, tasks_plan.fixed_len.unwrap() as i64);
                self.executor_task_counts
                    .insert(executor_reg.clone(), tasks_plan.fixed_len.unwrap());
            } else if tasks_plan.is_vec() {
                tasks_ptr_reg = self.new_tmp();
                release_tasks_ptr = true;
                self.ins(&format!(
                    "EXPAND VEC_AS_PTR {tasks_ptr_reg}, {tasks_owner_reg}"
                ));
                self.ins(&format!("EXPAND VEC_LEN {len_reg}, {tasks_owner_reg}"));
            } else {
                return unsupported("executor task buffer kind", "codegen.zig:9450");
            }
            self.ins(&format!(
                "EXPAND EXECUTOR_NEW {executor_reg}, {tasks_ptr_reg}, {len_reg}"
            ));
            self.emit_release(&len_reg)?;
            if release_tasks_ptr {
                self.emit_release(&tasks_ptr_reg)?;
            }
            return Ok(Some(executor_reg));
        }
        if plan.is_poll_one() {
            if call.args.len() != 2 || !call.generics.is_empty() {
                return unsupported("executor::poll_one arity", "codegen.zig:9450");
            }
            let executor_reg = self.gen_expr(&call.args[0])?;
            let index_reg = self.gen_expr(&call.args[1])?;
            let poll_reg = self.new_tmp();
            let tag_reg = self.new_tmp();
            let ready_reg = self.new_tmp();
            self.ins(&format!(
                "EXPAND EXECUTOR_POLL_ONE {poll_reg}, {executor_reg}, {index_reg}"
            ));
            self.ins(&format!("{tag_reg} = load {poll_reg}+Poll_tag as u64"));
            self.ins(&format!("{ready_reg} = eq {tag_reg}, Poll_READY"));
            self.emit_release(&tag_reg)?;
            self.emit_release(&poll_reg)?;
            if call_arg_needs_release(&call.args[1]) {
                self.emit_release(&index_reg)?;
            }
            if call_arg_needs_release(&call.args[0]) {
                self.emit_release(&executor_reg)?;
            }
            return Ok(Some(ready_reg));
        }
        if plan.is_poll_ready_count() {
            if call.args.len() != 1 || !call.generics.is_empty() {
                return unsupported("executor::poll_ready_count arity", "codegen.zig:9450");
            }
            let executor_reg = self.gen_expr(&call.args[0])?;
            let known_count = self
                .executor_task_counts
                .get(&self.resolve_binding_name(&executor_reg))
                .copied()
                .or_else(|| {
                    if let Node::Identifier(name) = &call.args[0] {
                        self.executor_task_counts
                            .get(&self.resolve_binding_name(name))
                            .copied()
                    } else {
                        None
                    }
                });
            match known_count {
                Some(task_count) => {
                    let count_reg = self.gen_executor_poll_ready_count_unrolled(
                        &executor_reg,
                        task_count,
                    )?;
                    if call_arg_needs_release(&call.args[0]) {
                        self.emit_release(&executor_reg)?;
                    }
                    return Ok(Some(count_reg));
                }
                None => {
                    let count_reg = self.new_tmp();
                    self.ins(&format!(
                        "EXPAND EXECUTOR_POLL_READY_COUNT {count_reg}, {executor_reg}"
                    ));
                    if call_arg_needs_release(&call.args[0]) {
                        self.emit_release(&executor_reg)?;
                    }
                    return Ok(Some(count_reg));
                }
            }
        }
        unsupported("executor runtime call kind", "codegen.zig:9450")
    }

    /// codegen.zig:9518 `genExecutorPollReadyCountUnrolled`.
    fn gen_executor_poll_ready_count_unrolled(
        &mut self,
        executor_reg: &str,
        task_count: usize,
    ) -> Result<String> {
        let tasks_reg = self.new_tmp();
        let count_slot = self.new_tmp();
        self.ins(&format!(
            "{tasks_reg} = load {executor_reg}+Executor_tasks as ptr"
        ));
        self.ins(&format!("{count_slot} = stack_alloc 8"));
        self.ins(&format!("store {count_slot}+0, 0 as u64"));
        for task_index in 0..task_count {
            let task_reg = self.new_tmp();
            let poll_reg = self.new_tmp();
            let tag_reg = self.new_tmp();
            let ready_reg = self.new_tmp();
            let ready_label = self.new_label("L_EXECUTOR_POLL_COUNT_READY");
            let next_label = self.new_label("L_EXECUTOR_POLL_COUNT_NEXT");
            self.ins(&format!(
                "{task_reg} = load {tasks_reg}+{} as ptr",
                task_index * 8
            ));
            self.ins(&format!("EXPAND TASK_POLL {poll_reg}, {task_reg}"));
            self.ins(&format!("{tag_reg} = load {poll_reg}+Poll_tag as u64"));
            self.ins(&format!("{ready_reg} = eq {tag_reg}, Poll_READY"));
            self.w(&format!("    br {ready_reg} -> {ready_label}, {next_label}\n\n"));
            self.wl(&format!("{ready_label}:"));
            let count_reg = self.new_tmp();
            let next_count_reg = self.new_tmp();
            self.ins(&format!("{count_reg} = load {count_slot}+0 as u64"));
            self.ins(&format!("{next_count_reg} = add {count_reg}, 1"));
            self.ins(&format!(
                "store {count_slot}+0, {next_count_reg} as u64"
            ));
            self.emit_release(&next_count_reg)?;
            self.emit_release(&count_reg)?;
            self.w(&format!("    jmp {next_label}\n\n"));
            self.wl(&format!("{next_label}:"));
            self.emit_release(&ready_reg)?;
            self.emit_release(&tag_reg)?;
            self.emit_release(&poll_reg)?;
            self.emit_release(&task_reg)?;
        }
        let out_count_reg = self.new_tmp();
        self.ins(&format!("{out_count_reg} = load {count_slot}+0 as u64"));
        self.emit_release(&tasks_reg)?;
        Ok(out_count_reg)
    }

    /// Dispatch used by the call-expression path (codegen.zig
    /// genExpr call arm order: poll, executor, future, then in the
    /// associated-target branch future/task). Returns the register
    /// when a runtime plan claimed the call.
    pub(crate) fn gen_runtime_call(&mut self, call: &CallExpr) -> Result<Option<String>> {
        if let Some(reg) = self.gen_poll_runtime_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_executor_runtime_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(plan) = plan_future_runtime_call(call) {
            return Ok(Some(self.gen_future_runtime_call(plan, call)?));
        }
        if let Some(plan) = plan_task_runtime_call(call) {
            return Ok(Some(self.gen_task_runtime_call(plan, call)?));
        }
        Ok(None)
    }

    // -- await (codegen.zig genExpr await_expr arm, 13253) -----------

    /// codegen.zig:1381 `emitAwaitPendingCleanups`.
    pub(crate) fn emit_await_pending_cleanups(&mut self, await_expr: &Node) -> Result<()> {
        if let Some(list) = self.check.await_cleanups(await_expr) {
            let names: Vec<String> = list.to_vec();
            self.emit_branch_scoped_cleanup_list(&names)?;
        }
        Ok(())
    }

    pub(crate) fn gen_await_expr(&mut self, expr: &Node) -> Result<String> {
        let aw = match expr {
            Node::AwaitExpr(a) => a,
            _ => unreachable!(),
        };
        let future_ty = self
            .expr_type(&aw.expr)
            .ok_or(crate::TextError::MissingType {
                context: "awaited future type".to_string(),
            })?;
        let future_reg = self.gen_expr(&aw.expr)?;
        let readiness_snapshot = self.future_readiness.clone();
        let plan = plan_await_future_with_readiness(
            &aw.expr,
            &future_ty,
            self.current_async_return_ty.as_ref(),
            Some(&readiness_snapshot),
        );
        if self.current_async && plan.pending_return_if_async {
            self.emit_await_pending_cleanups(expr)?;
            self.ins(&format!("return {future_reg}"));
            self.async_pending_return_emitted = true;
            return Ok(future_reg);
        }
        if plan.poll_once_if_statically_ready {
            let future_obj = self.gen_future_object_for_state(&future_reg)?;
            let ctx = self.new_tmp();
            let poll_reg = self.new_tmp();
            let out_reg = self.new_tmp();
            self.ins(&format!("{ctx} = 0"));
            self.ins(&format!(
                "EXPAND FUTURE_POLL {poll_reg}, {future_obj}, {ctx}"
            ));
            self.ins(&format!("EXPAND POLL_VALUE {out_reg}, {poll_reg}"));
            self.emit_release(&poll_reg)?;
            self.emit_release(&ctx)?;
            self.emit_release(&future_obj)?;
            return Ok(out_reg);
        }
        if self.current_async && plan.ready_pending_state_return_if_async {
            let is_custom = await_expr_future_is_custom(&aw.expr, &|name| {
                self.func_decl(name).map(|f| f.is_async).unwrap_or(false)
            });
            if is_custom {
                return self.gen_await_busypoll(&future_reg);
            }
            let state_reg = self.new_tmp();
            let pending_reg = self.new_tmp();
            let ready_label = self.new_label("L_AWAIT_READY");
            let pending_label = self.new_label("L_AWAIT_PENDING");
            self.ins(&format!(
                "{state_reg} = load {future_reg}+ReadyFuture_state as u64"
            ));
            self.ins(&format!("{pending_reg} = eq {state_reg}, 0"));
            self.w(&format!(
                "    br {pending_reg} -> {pending_label}, {ready_label}\n\n"
            ));
            self.wl(&format!("{pending_label}:"));
            self.emit_await_pending_cleanups(expr)?;
            self.ins(&format!("return {future_reg}"));
            self.wl(&format!("{ready_label}:"));
            self.emit_release(&state_reg)?;
            self.emit_release(&pending_reg)?;
            let out_reg = self.new_tmp();
            self.ins(&format!(
                "EXPAND FUTURE_READY_STATE_INTO_INNER {out_reg}, {future_reg}"
            ));
            self.emit_release(&future_reg)?;
            return Ok(out_reg);
        }
        if !self.current_async {
            // AWAIT-EXTRACT (corpus regression
            // test_unit_async_await_extract_sync_regression): in a
            // SYNC fn, awaiting a custom future (async-fn call,
            // join2/select2/defer_ready construction) must poll it
            // to Ready — the ready-state fast path below would read
            // the state machine's first word as a ready-state
            // value. The vendored Zig text codegen gates its
            // busypoll on the async context; the corpus file's
            // header documents that gating as the defect this
            // regression pins, so the sync context busypolls here.
            let is_custom = await_expr_future_is_custom(&aw.expr, &|name| {
                self.func_decl(name).map(|f| f.is_async).unwrap_or(false)
            });
            if is_custom {
                return self.gen_await_busypoll(&future_reg);
            }
        }
        if !plan.ready_state_inner {
            return unsupported("await of non-ready-state future", "codegen.zig:13253");
        }
        let out_reg = self.new_tmp();
        self.ins(&format!(
            "EXPAND FUTURE_READY_STATE_INTO_INNER {out_reg}, {future_reg}"
        ));
        self.emit_release(&future_reg)?;
        Ok(out_reg)
    }

    /// The busypoll await form (codegen.zig:13280, the custom
    /// branch of the ready/pending state-return path): wrap the
    /// state in a future object and FUTURE_POLL until Ready, then
    /// take the poll value.
    fn gen_await_busypoll(&mut self, future_reg: &str) -> Result<String> {
        let future_obj = self.gen_future_object_for_state(future_reg)?;
        let ctx = self.new_tmp();
        let poll_reg = self.new_tmp();
        let ready_reg = self.new_tmp();
        let out_reg = self.new_tmp();
        let loop_label = self.new_label("L_AWAIT_BUSYPOLL");
        let again_label = self.new_label("L_AWAIT_BUSYPOLL_AGAIN");
        let done_label = self.new_label("L_AWAIT_BUSYPOLL_DONE");
        self.ins(&format!("{ctx} = 0"));
        self.wl(&format!("{loop_label}:"));
        self.ins(&format!(
            "EXPAND FUTURE_POLL {poll_reg}, {future_obj}, {ctx}"
        ));
        self.ins(&format!("EXPAND POLL_IS_READY {ready_reg}, {poll_reg}"));
        self.w(&format!("    br {ready_reg} -> {done_label}, {again_label}\n\n"));
        self.wl(&format!("{again_label}:"));
        self.ins(&format!("!{poll_reg}"));
        self.ins(&format!("!{ready_reg}"));
        self.w(&format!("    jmp {loop_label}\n\n"));
        self.wl(&format!("{done_label}:"));
        self.ins(&format!("!{ready_reg}"));
        self.ins(&format!("EXPAND POLL_VALUE {out_reg}, {poll_reg}"));
        self.ins(&format!("!{poll_reg}"));
        self.ins(&format!("!{ctx}"));
        self.ins(&format!("!{future_obj}"));
        Ok(out_reg)
    }

    // -- program helpers (codegen.zig:6822 `emitFutureTaskHelpers`) --

    pub(crate) fn emit_future_task_helpers(&mut self) {
        self.w(
            "@const SLA_READY_FUTURE_VT = vtable { poll = @sla_future_ready_poll }\n\
             @const SLA_DEFER_READY_FUTURE_VT = vtable { poll = @sla_future_defer_ready_poll }\n\
             @const SLA_JOIN2_FUTURE_VT = vtable { poll = @sla_future_join2_poll }\n\
             @const SLA_SELECT2_FUTURE_VT = vtable { poll = @sla_future_select2_poll }\n\
             \n\
             @sla_future_ready_poll(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   EXPAND FUTURE_READY_SET_POLL_STATE out_poll_slot, data_slot\n\
             \x20   !data_slot\n\
             \x20   !ctx_slot\n\
             \x20   !out_poll_slot\n\
             \x20   return\n\
             \n\
             @sla_future_defer_ready_poll(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   defer_ready_stage = load data_slot+0 as u64\n\
             \x20   defer_ready_is_initial = eq defer_ready_stage, 0\n\
             \x20   br defer_ready_is_initial -> L_DEFER_READY_PENDING, L_DEFER_READY_CHECK_READY\n\
             L_DEFER_READY_PENDING:\n\
             \x20   store data_slot+0, 1 as u64\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   jmp L_DEFER_READY_DONE\n\
             L_DEFER_READY_CHECK_READY:\n\
             \x20   defer_ready_is_ready = eq defer_ready_stage, 1\n\
             \x20   br defer_ready_is_ready -> L_DEFER_READY_READY, L_DEFER_READY_EMPTY\n\
             L_DEFER_READY_READY:\n\
             \x20   defer_ready_value = load data_slot+8 as u64\n\
             \x20   store data_slot+0, 2 as u64\n\
             \x20   EXPAND POLL_SET_READY out_poll_slot, defer_ready_value\n\
             \x20   !defer_ready_value\n\
             \x20   !defer_ready_is_ready\n\
             \x20   jmp L_DEFER_READY_DONE\n\
             L_DEFER_READY_EMPTY:\n\
             \x20   EXPAND POLL_SET_PENDING out_poll_slot\n\
             \x20   !defer_ready_is_ready\n\
             L_DEFER_READY_DONE:\n\
             \x20   !defer_ready_is_initial\n\
             \x20   !defer_ready_stage\n\
             \x20   !data_slot\n\
             \x20   !ctx_slot\n\
             \x20   !out_poll_slot\n\
             \x20   return\n\
             \n\
             @sla_future_join2_poll(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   EXPAND FUTURE_JOIN2_STATE_POLL join2_poll_tmp, data_slot, ctx_slot\n\
             \x20   join2_poll_tag = load join2_poll_tmp+Poll_tag as u64\n\
             \x20   join2_poll_value = load join2_poll_tmp+Poll_value as u64\n\
             \x20   store out_poll_slot+Poll_tag, join2_poll_tag as u64\n\
             \x20   store out_poll_slot+Poll_value, join2_poll_value as u64\n\
             \x20   !join2_poll_value\n\
             \x20   !join2_poll_tag\n\
             \x20   !join2_poll_tmp\n\
             \x20   !data_slot\n\
             \x20   !ctx_slot\n\
             \x20   !out_poll_slot\n\
             \x20   return\n\
             \n\
             @sla_future_select2_poll(&data_slot: ptr, &ctx_slot: ptr, &out_poll_slot: ptr):\n\
             L_ENTRY:\n\
             \x20   EXPAND FUTURE_SELECT2_STATE_POLL select2_poll_tmp, data_slot, ctx_slot\n\
             \x20   select2_poll_tag = load select2_poll_tmp+Poll_tag as u64\n\
             \x20   select2_poll_value = load select2_poll_tmp+Poll_value as u64\n\
             \x20   store out_poll_slot+Poll_tag, select2_poll_tag as u64\n\
             \x20   store out_poll_slot+Poll_value, select2_poll_value as u64\n\
             \x20   !select2_poll_value\n\
             \x20   !select2_poll_tag\n\
             \x20   !select2_poll_tmp\n\
             \x20   !data_slot\n\
             \x20   !ctx_slot\n\
             \x20   !out_poll_slot\n\
             \x20   return\n\
             \n",
        );
    }
}

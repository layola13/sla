//! Function and test declaration emission, mirrored from
//! codegen.zig's `genFuncDeclNamed` (7429-7606) and `genTestDecl`
//! (7610-7700). The async continuation entry points checked at the
//! top of `genFuncDeclNamed` dispatch to the plan emitters in
//! async_plans_codegen*.rs (stage 6 async cluster); an async
//! function no plan claims falls through to the eager path, whose
//! returns wrap in a ready future (planAsyncFunctionReturn).

use sla_ast::{FuncDecl, Node, TestDecl, Type};
use sla_lowering::{
    abi_param_type_string, block_terminates, by_value_raw_pointer_param, is_borrow_like_type,
    is_void_type, plan_async_function_return, plan_async_general_await_continuation,
    plan_async_join2_await_continuation, plan_async_linear_await_continuation,
    plan_async_loop_await_continuation, plan_async_single_await_continuation,
    plan_async_two_await_continuation, shared_abi_param_prefix,
};

use crate::codegen::{sanitize_test_name, type_size, type_string, Codegen};
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// codegen.zig:7608 `genFuncDecl`.
    pub(crate) fn gen_func_decl(&mut self, f: &FuncDecl) -> Result<()> {
        let name = f.name.clone();
        self.gen_func_decl_named(&name, f)
    }

    /// codegen.zig:7429 `genFuncDeclNamed` (async context
    /// save/restore at 7430-7438).
    pub(crate) fn gen_func_decl_named(&mut self, name: &str, f: &FuncDecl) -> Result<()> {
        // RUST-STAGE10-CLONE-EXTRACT: the declared name, for the
        // M2 refinement set (`clone_extract_funcs`) — the SLICE_GET
        // writeback records the function it is generating. The
        // declared name (not the lowered/mangled emission name) is
        // what call sites spell.
        self.current_func_name = Some(f.name.clone());
        let prev_async = self.current_async;
        let prev_async_return_ty = self.current_async_return_ty.clone();
        let prev_pending = self.async_pending_return_emitted;
        self.current_async = f.is_async;
        self.current_async_return_ty = if f.is_async {
            Some((*f.ret_ty).clone())
        } else {
            None
        };
        self.async_pending_return_emitted = false;
        let result = self.gen_func_decl_named_inner(name, f);
        self.current_async = prev_async;
        self.current_async_return_ty = prev_async_return_ty;
        self.async_pending_return_emitted = prev_pending;
        result
    }

    fn gen_func_decl_named_inner(&mut self, name: &str, f: &FuncDecl) -> Result<()> {
        self.reset_function_state();
        // Async continuation plan dispatch (codegen.zig:7478-7495),
        // in Zig order: join2, two, linear, general, loop, single.
        if f.is_async {
            if let Some(plan) = plan_async_join2_await_continuation(f) {
                return self.gen_async_join2_await_func_decl_named(name, f, &plan);
            }
            if let Some(plan) = plan_async_two_await_continuation(f) {
                return self.gen_async_two_await_func_decl_named(name, f, &plan);
            }
            if let Some(plan) = plan_async_linear_await_continuation(f) {
                return self.gen_async_linear_await_func_decl_named(name, f, &plan);
            }
            if let Some(plan) = plan_async_general_await_continuation(f) {
                return self.gen_async_general_await_func_decl_named(name, f, &plan);
            }
            if let Some(plan) = plan_async_loop_await_continuation(f) {
                return self.gen_async_loop_await_func_decl_named(name, f, &plan);
            }
            if let Some(plan) = plan_async_single_await_continuation(f) {
                return self.gen_async_single_await_func_decl_named(name, f, &plan);
            }
        }
        self.current_ret_ty = Some((*f.ret_ty).clone());
        // Extern variadic batch (LINE-CENSUS-VARIADIC): record
        // the VaList parameter name for the va_start / va_end
        // bookkeeping below (Zig `current_variadic_list`).
        self.current_variadic_list = f
            .params
            .iter()
            .find(|p| matches!(&*p.ty, Type::Primitive(sla_ast::types::Primitive::VaList)))
            .map(|p| p.name.clone());
        self.repeated_let_bindings = sla_lowering::collect_repeated_let_bindings(&f.body);
        self.collect_assigned_bindings(&f.body);
        self.collect_addressable_bindings(&f.body);

        // Signature. (Methods arrive under a pre-mangled `name`
        // that differs from `f.name`; the overload member symbol
        // applies only to plain program functions, where the two
        // are the same.)
        let lowered_name = if name == f.name {
            self.decl_symbol(f)
        } else {
            self.lowered_func_symbol(name)
        };
        let mut sig = format!("@{lowered_name}(");
        let mut printed_params = 0usize;
        for p in f.params.iter() {
            // Extern variadic batch (LINE-CENSUS-VARIADIC,
            // codegen.zig genFuncDeclNamed): the VaList
            // parameter of a variadic definition is not an SA
            // parameter — the SA header carries the trailing
            // `...` marker and the body binds the name via
            // va_start. Emitting it as a plain `args: ptr`
            // parameter made every call with trailing arguments
            // mismatch the callee contract (CapabilityMismatch).
            if matches!(&*p.ty, Type::Primitive(sla_ast::types::Primitive::VaList)) {
                continue;
            }
            if printed_params > 0 {
                sig.push_str(", ");
            }
            printed_params += 1;
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
        if f.is_variadic {
            sig.push_str(", ...");
        }
        // codegen.zig:7506: planAsyncFunctionReturn — an async
        // function's ABI return is a raw pointer (the future state).
        let async_return_plan = plan_async_function_return(f);
        if is_void_type(&f.ret_ty) && !f.is_async {
            sig.push_str("):");
        } else if async_return_plan.abi_ret_is_ptr {
            sig.push_str(") -> ptr:");
        } else {
            sig.push_str(&format!(
                ") -> {}:",
                sla_lowering::abi_return_type_string(&f.ret_ty)
            ));
        }
        self.wl(&sig);
        self.wl("L_ENTRY:");

        // Extern variadic batch (LINE-CENSUS-VARIADIC): bind
        // the VaList name before anything else in the body
        // (codegen.zig emits `{list} = va_start` immediately
        // after L_ENTRY, ahead of the hoisted allocations).
        if let Some(list_name) = self.current_variadic_list.clone() {
            self.ins(&format!("{list_name} = va_start"));
        }

        // Hoisted loop allocations + for-loop counter slots.
        self.emit_hoisted_and_counter_slots(&f.body)?;

        // Parameter prologues.
        for p in &f.params {
            self.remember_local_binding_type(&p.name, &p.ty);
            self.param_bindings.insert(p.name.clone());
            // codegen.zig:10412: a parameter is a borrow parameter
            // when it carries the borrow prefix OR its type is a
            // borrow (the `&[T; N]` / `&T` annotation form).
            if p.is_borrow || matches!(&*p.ty, Type::Borrow(_)) {
                self.borrow_param_bindings.insert(p.name.clone());
            }
            // Extern variadic batch (LINE-CENSUS-VARIADIC): the
            // VaList parameter is bound by va_start above; it
            // takes no addressable storage or assigned-value
            // slot (the Zig prologue `continue` for the va_list
            // parameter).
            if matches!(&*p.ty, Type::Primitive(sla_ast::types::Primitive::VaList)) {
                continue;
            }
            // A borrowed ARRAY parameter needs no prologue at all:
            // Zig's slice materialization (codegen.zig:10453) fires
            // only when the borrowed type is a `Slice<T>` (its
            // element-type probe returns null for a fixed array),
            // so `&[i32; 3]` arrives as a bare pointer borrow and
            // the body indexes it directly (@sla__read_borrowed_
            // index_borrow in the Zig emission has no prologue).
            // The previous port rejected this shape as an
            // unsupported "borrowed slice parameter prologue".
            if !p.is_borrow && !p.is_move && self.binding_needs_addressable_storage(&p.name, &p.ty)
            {
                let raw_param = self.new_tmp();
                self.ins(&format!("{raw_param} = {}", p.name));
                self.stack_alloc_bindings.insert(p.name.clone());
                self.ins(&format!(
                    "{} = stack_alloc {}",
                    p.name,
                    type_size(&p.ty)
                ));
                self.ins(&format!(
                    "store {}+0, {raw_param} as {}",
                    p.name,
                    type_string(&p.ty)
                ));
                self.emit_release(&raw_param)?;
            } else if !p.is_borrow
                && !p.is_move
                && self.binding_needs_assigned_value_slot(&p.name, &p.ty)
            {
                let raw_param = self.new_tmp();
                self.ins(&format!("{raw_param} = {}", p.name));
                self.stack_alloc_bindings.insert(p.name.clone());
                self.assigned_value_slots.insert(p.name.clone());
                self.ins(&format!(
                    "{} = stack_alloc {}",
                    p.name,
                    type_size(&p.ty)
                ));
                self.ins(&format!(
                    "store {}+0, {raw_param} as {}",
                    p.name,
                    type_string(&p.ty)
                ));
            }
        }

        // Body: tail-expression return path or plain block
        // (codegen.zig:7569-7605, with the async pending-return
        // breaks and the ready-future wrap).
        let tail_expr_return = !is_void_type(&f.ret_ty)
            && !f.body.is_empty()
            && matches!(f.body.last(), Some(Node::ExprStmt(_)))
            && !block_terminates(&f.body);
        if tail_expr_return {
            let split = f.body.len() - 1;
            for stmt in &f.body[..split] {
                self.gen_stmt(stmt)?;
                if self.async_pending_return_emitted {
                    break;
                }
            }
            if !self.async_pending_return_emitted {
                let last = &f.body[split];
                let tail_expr = match last {
                    Node::ExprStmt(expr) => expr,
                    _ => unreachable!(),
                };
                // R2b (stage 6): a tail literal takes its layout from
                // the declared return type, not its own recorded type.
                let mut tail_reg = self.gen_expr_in_context(tail_expr, Some(&*f.ret_ty))?;
                if !self.async_pending_return_emitted {
                    if async_return_plan.wrap_ready_future {
                        tail_reg = self.gen_ready_future_i64(&tail_reg)?;
                    }
                    self.emit_function_tail_cleanups(last, tail_expr)?;
                    self.emit_va_end_if_variadic();
                    self.ins(&format!("return {tail_reg}"));
                }
            }
        } else {
            self.gen_block(&f.body)?;
        }

        if self.async_pending_return_emitted {
            // The generated pending await returned from this async
            // function (codegen.zig:7589).
        } else if async_return_plan.wrap_ready_future
            && !tail_expr_return
            && !block_terminates(&f.body)
        {
            let zero = self.new_tmp();
            self.ins(&format!("{zero} = 0"));
            let future = self.gen_ready_future_i64(&zero)?;
            self.ins(&format!("return {future}"));
        } else if is_void_type(&f.ret_ty) && !last_stmt_is_return(&f.body) {
            if f.body.is_empty() {
                // L4R2 Root V: an empty body has no last statement
                // for the checker to hang the fn-exit parameter
                // cleanups on (append_fn_exit_param_cleanups
                // early-returns), and gen_stmt's per-statement
                // cleanup emission never runs, so the parameters
                // were left Active at the bare return (MemoryLeak).
                // With no statements, no parameter can have been
                // consumed or borrowed from, so the fn-exit list is
                // exactly the parameter list; emit it through the
                // same lexical-cleanup primitive the recorded lists
                // use.
                for p in &f.params {
                    self.emit_lexical_cleanup_release(&p.name)?;
                }
            }
            self.emit_va_end_if_variadic();
            self.ins("return");
        }
        self.w("\n");
        Ok(())
    }

    /// codegen.zig:7610 `genTestDecl`. The body is generated exactly
    /// as given; build-mode test-body pruning happens upstream of
    /// codegen in the Zig pipeline (the build artifacts carry empty
    /// bodies), so a pruned program emits the stub shape seen in the
    /// goldens from this same code path.
    pub(crate) fn gen_test_decl(&mut self, t: &TestDecl) -> Result<()> {
        // RUST-STAGE10-CLONE-EXTRACT: tests are never callees, so
        // recording the test name in `clone_extract_funcs` (if a
        // test body cloned an extraction) is harmless.
        self.current_func_name = Some(t.name.clone());
        self.reset_function_state();
        self.repeated_let_bindings = sla_lowering::collect_repeated_let_bindings(&t.body);
        self.collect_assigned_bindings(&t.body);
        self.collect_addressable_bindings(&t.body);

        let mut header = String::from("@test");
        if t.is_ignored {
            header.push_str(" ignored");
        }
        if t.should_panic {
            header.push_str(" should_panic");
        }
        header.push_str(&format!(" \"{}\"():", sanitize_test_name(&t.name)));
        self.wl(&header);
        let entry_label = self.new_label("L_TEST_ENTRY");
        self.wl(&format!("{entry_label}:"));

        self.emit_hoisted_and_counter_slots(&t.body)?;
        self.gen_block(&t.body)?;
        self.ins("return");
        self.w("\n");
        Ok(())
    }

    /// Hoisted stack allocations (`stack_alloc 16`) and for-loop
    /// counter slots (`stack_alloc 8`), shared by func/test decls.
    fn emit_hoisted_and_counter_slots(&mut self, body: &[Node]) -> Result<()> {
        let mut hoisted: Vec<String> = Vec::new();
        self.collect_hoisted_allocs(body, &mut hoisted);
        self.hoisted_allocs = hoisted.clone();
        for h_name in &hoisted {
            self.ins(&format!("{h_name} = stack_alloc 16"));
        }
        let mut slots: Vec<String> = Vec::new();
        self.collect_loop_counter_slots(body, &mut slots);
        for slot_name in &slots {
            self.ins(&format!("{slot_name} = stack_alloc 8"));
            self.stack_alloc_bindings.insert(slot_name.clone());
        }
        Ok(())
    }

    /// codegen.zig:2553 `bindingNeedsAddressableStorage`.
    pub(crate) fn binding_needs_addressable_storage(&self, name: &str, ty: &Type) -> bool {
        sla_lowering::plan_borrowed_binding_storage(self.addressable_bindings.contains(name), ty)
            .materialize_stack_slot
    }

    /// codegen.zig:2557 `bindingNeedsAssignedValueSlot`.
    pub(crate) fn binding_needs_assigned_value_slot(&self, name: &str, ty: &Type) -> bool {
        self.assigned_bindings.contains(name)
            && !self.type_is_copy_value(ty)
            && self.check.type_is_shallow_copy_call_arg_value(ty)
    }
}

/// codegen.zig `lastStmtIsReturn`.
fn last_stmt_is_return(body: &[Node]) -> bool {
    matches!(body.last(), Some(Node::ReturnStmt(_)))
}

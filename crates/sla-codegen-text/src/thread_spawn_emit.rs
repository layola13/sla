//! `thread::spawn` emission half (RUST-S7-SPAWN-ASSOC): the
//! call site, the per-site `@ffi_wrapper sla_thread_spawn_{idx}`
//! / `@sla_thread_worker_{idx}` helper pair, and the worker
//! body generation. The recognition facts, the helper table and
//! the collection pre-pass live in thread_spawn.rs; the Zig
//! references are codegen.zig's `isThreadSpawnCall` call-site
//! arm and `emitThreadSpawnHelpers`.

use sla_ast::{CallExpr, Node, Type};
use sla_lowering::{is_void_type, node_key};

use crate::codegen::type_string;
use crate::thread_spawn::{thread_spawn_closure_literal, ThreadSpawnHelper};
use crate::{unsupported, Result};

impl<'c> crate::Codegen<'c> {
    // -- call site (codegen.zig `isThreadSpawnCall` arm) --

    /// The `thread::spawn` call-site emission: allocate and
    /// initialise the slot, transfer the captures into it, and
    /// either run the body inline (inline_join) or call the
    /// per-site spawn helper. The slot register is the
    /// JoinHandle value.
    pub(crate) fn gen_thread_spawn_call(&mut self, expr: &Node, call: &CallExpr) -> Result<String> {
        let key = node_key(expr);
        let Some((_, helper)) = self
            .thread_spawn_helpers
            .iter()
            .find(|(k, _)| *k == key)
        else {
            // No helper was registered: the argument is not a
            // nullary closure literal (the Zig lookup miss fails
            // generation the same way). Fail loudly, by shape.
            if thread_spawn_closure_literal(&call.args[0]).is_none() {
                return unsupported(
                    "thread::spawn with a non-literal closure argument",
                    "codegen.zig:8955 (threadSpawnHelperForExpr)",
                );
            }
            return unsupported(
                "thread::spawn closure with parameters",
                "codegen.zig:8955 (threadSpawnHelperForExpr)",
            );
        };
        // Clone the plan out of the table: generating the body
        // (inline path) borrows the codegen mutably throughout.
        let ThreadSpawnHelper {
            closure,
            ret_ty,
            captures,
            slot_size,
            inline_join,
            spawn_name,
            ..
        } = helper.clone();

        let slot = self.new_tmp();
        self.ins(&format!("{slot} = alloc {slot_size}"));
        self.ins(&format!("store {slot}+0, 0 as i32"));
        self.ins(&format!(
            "store {slot}+8, 0 as {}",
            type_string(&ret_ty)
        ));
        if inline_join {
            let value_reg = self.gen_expr(&closure.body)?;
            self.ins(&format!(
                "store {slot}+8, {value_reg} as {}",
                type_string(&ret_ty)
            ));
            if sla_lowering::expr_result_needs_release(&closure.body) {
                self.emit_release(&value_reg)?;
            }
            self.ins(&format!("store {slot}+0, -1 as i32"));
            self.inline_thread_handles.insert(slot.clone());
            return Ok(slot);
        }
        for capture in &captures {
            let capture_reg = self.resolve_binding_name(&capture.name);
            self.ins(&format!(
                "store {slot}+{}, {capture_reg} as ptr",
                capture.offset
            ));
            if capture.is_noncopy_payload {
                self.emit_forget_moved_value(&capture_reg)?;
            }
        }
        self.ins(&format!("call @{spawn_name}(*{slot})"));
        Ok(slot)
    }

    // -- helper emission (codegen.zig `emitThreadSpawnHelpers`) --

    /// Emit the vtables and the spawn/worker function pairs for
    /// every non-inline helper, in registration order, after all
    /// program functions (called from `generate_program`).
    pub(crate) fn emit_thread_spawn_helpers(&mut self) -> Result<()> {
        if self.thread_spawn_helpers.is_empty() {
            return Ok(());
        }
        let plans: Vec<ThreadSpawnHelper> = self
            .thread_spawn_helpers
            .iter()
            .map(|(_, h)| h.clone())
            .collect();
        for helper in &plans {
            if helper.inline_join {
                continue;
            }
            self.wl(&format!(
                "@const {} = vtable {{ call = @{} }}",
                helper.vtable_name, helper.worker_name
            ));
        }
        self.wl("");
        for helper in &plans {
            if helper.inline_join {
                continue;
            }
            self.wl(&format!("@ffi_wrapper {}(*slot: ptr):", helper.spawn_name));
            self.wl("L_ENTRY:");
            self.ins(&format!("worker_vt = &{}", helper.vtable_name));
            self.ins("worker_fn = load worker_vt+0 as ptr");
            self.ins("worker_raw = *worker_fn");
            self.ins("worker_safe = assume_safe worker_raw");
            self.ins("EXPAND THREAD_SPAWN handle, *worker_safe, *slot");
            self.ins("store slot+0, handle as i32");
            self.ins("!handle");
            self.ins("!worker_safe");
            self.ins("!worker_raw");
            self.ins("!worker_fn");
            self.ins("!worker_vt");
            self.ins("!slot");
            self.ins("return");
            self.wl("");
            self.wl(&format!("@{}(&slot: ptr) -> i32:", helper.worker_name));
            self.wl("L_ENTRY:");
            self.gen_thread_worker_body(helper)?;
        }
        Ok(())
    }

    /// The worker body: load the captures out of the slot, seed
    /// them as bindings, generate the closure body, and write
    /// the result back to `slot+8` (void bodies just return 0).
    fn gen_thread_worker_body(&mut self, helper: &ThreadSpawnHelper) -> Result<()> {
        self.reset_function_state();
        for capture in &helper.captures {
            let capture_reg = self.new_tmp();
            self.ins(&format!(
                "{capture_reg} = load slot+{} as ptr",
                capture.offset
            ));
            self.thread_capture_regs
                .insert(capture.name.clone(), capture_reg);
            if let Some(ty) = &capture.ty {
                let ty = ty.clone();
                self.remember_local_binding_type(&capture.name, &ty);
            }
        }
        let body = &helper.closure.body;
        if is_void_type(&helper.ret_ty) {
            // Void closure body: generate as a statement (a value
            // form would name a void call result).
            if matches!(&**body, Node::CallExpr(_)) {
                self.gen_call_stmt(body)?;
            } else {
                self.gen_expr(body)?;
            }
            self.thread_capture_regs.clear();
            self.ins("!slot");
            self.ins("return 0");
            self.wl("");
            return Ok(());
        }
        let value_reg = self.gen_expr(body)?;
        self.thread_capture_regs.clear();
        self.ins(&format!(
            "store slot+8, {value_reg} as {}",
            type_string(&helper.ret_ty)
        ));
        if matches!(helper.ret_ty, Type::Primitive(sla_ast::Primitive::I32)) {
            self.ins("!slot");
            self.ins(&format!("return {value_reg}"));
        } else {
            self.ins(&format!("!{value_reg}"));
            self.ins("!slot");
            self.ins("return 0");
        }
        self.wl("");
        Ok(())
    }
}

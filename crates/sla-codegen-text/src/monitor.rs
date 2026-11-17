//! `monitor(handle)` for the text lane (goroutine syntax
//! batch): the supervisor-surface builtin that subscribes to a
//! task's exit and yields the `Chan<(u64, i32)>` exit
//! notification channel — (watched handle, trap code), 0 for a
//! normal exit.
//!
//! In the Zig pipeline this call never reaches codegen as a
//! plain call: syntax_lowering.zig `emitMonitorCall`, gated on
//! the checker's `monitor_calls` side table, rewrites it into
//! a `sa_task_monitor` extern call over a zeroed u64 out-slot,
//! an rc check (panic 60921) and a value copy of the channel
//! handle out of the slot — the same materialisation form as
//! `Chan::new`. The text lane has no syntax-lowering stage, so
//! the same instruction sequence is emitted here directly, in
//! the lowering's order (the Chan len/cap arm in chan.rs is
//! the sibling shape).
//!
//! Identification mirrors the checker: check_call consults
//! the pre-associated bare-name block before user-function /
//! extern / macro resolution and its `monitor` verdict is
//! final, so any bare, callee-less, target-less `monitor`
//! call that survived checking is exactly this builtin (the
//! name+shape discipline chan.rs applies to Chan methods).
//! The checker has already verified the single argument is a
//! u64 goroutine handle (borrow spellings peel) and typed the
//! call `Chan<(u64, i32)>`.
//!
//! Register discipline follows the chan arms: the
//! `stack_alloc` out-slot is never explicitly released (the
//! host frees it at scope exit; an explicit `!` on it is a
//! StackEscape), and the result is a fresh load of the slot
//! word, never the slot itself.

use sla_ast::{CallExpr, Primitive, Type};

use crate::codegen::Codegen;
use crate::Result;

impl<'c> Codegen<'c> {
    /// Detect the `monitor(handle)` supervisor builtin: a bare
    /// call (no closure callee, no associated target) named
    /// `monitor` with exactly one argument.
    pub(crate) fn is_monitor_call(&self, call: &CallExpr) -> bool {
        call.func_name == "monitor"
            && call.callee.is_none()
            && call.associated_target.is_none()
            && call.args.len() == 1
    }

    /// Emit a `monitor(handle)` call in expression position;
    /// returns the register holding the materialised channel
    /// handle word (a u64 at the register level; the checker
    /// types the binding `Chan<(u64, i32)>`).
    pub(crate) fn gen_monitor_call(&mut self, call: &CallExpr) -> Result<String> {
        let arg = &call.args[0];
        // Handle value: a plain u64 register passes by copy
        // (the sa_task_join form). A borrow/pointer spelling —
        // the checker peels it in check_monitor_call — loads
        // the word out of the address first.
        let raw_reg = self.gen_expr(arg)?;
        let mut handle_reg = raw_reg.clone();
        let mut handle_temp: Option<String> = None;
        if let Some(ty) = self.expr_type(arg) {
            let peeled = sla_lowering::peel_borrow_pointer_type(&ty);
            if !matches!(ty, Type::Primitive(Primitive::U64))
                && matches!(peeled, Type::Primitive(Primitive::U64))
            {
                let loaded = self.new_tmp();
                self.ins(&format!("{loaded} = load {raw_reg}+0 as u64"));
                handle_reg = loaded.clone();
                handle_temp = Some(loaded);
            }
        }
        let cell = self.new_tmp();
        self.ins(&format!("{cell} = stack_alloc 8"));
        self.ins(&format!("store {cell}+0, 0 as u64"));
        let rc = self.new_tmp();
        self.ins(&format!(
            "{rc} = call @sa_task_monitor({handle_reg}, &{cell})"
        ));
        // Settle the handle registers now the call has taken
        // the value: a materialised load temp is dead, and a
        // disposable argument temp follows the ordinary
        // call-arg release discipline. A plain binding register
        // (the common `monitor(h)`) is Copy and stays live.
        if let Some(temp) = handle_temp {
            self.emit_release(&temp)?;
        } else if self.call_arg_result_temp_needs_release(arg, &raw_reg) {
            self.emit_release(&raw_reg)?;
        }
        self.emit_chan_rc_check(&rc, 60921)?;
        self.emit_release(&rc)?;
        let val = self.new_tmp();
        self.ins(&format!("{val} = load {cell}+0 as u64"));
        Ok(val)
    }
}

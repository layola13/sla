//! `go supervised` spawn emission (F-G2), mirrored from
//! syntax_lowering.zig `emitSpawn` (supervised arm): the lowering
//! rewrites the spawn into an extern call over a zeroed handle
//! slot — `sa_task_spawn_supervised(entry, arg, null, null,
//! &slot)` — an rc check (panic 60901, SPAWN_PANIC_CODE) and a
//! value copy of the handle out of the slot. The text lane has
//! no syntax-lowering stage, so the same instruction sequence is
//! emitted here directly, in the lowering's order (the F-G1
//! Chan::new arm in call.rs is the same pattern).
//!
//! Plain (unsupervised) `go` is NOT emitted here: statement
//! position keeps its scope-only pass-through and expression
//! position keeps the F-3 catch-all error, byte-identical to
//! before this line.
//!
//! Register discipline (F-G1/CHANNEW experience, applied to the
//! spawn site): an identifier argument whose register IS the
//! caller's binding is copied before the call when primitive
//! (the extern arg plan's value copy must not retire the
//! binding); the stack_alloc handle/dummy slots are never
//! explicitly `!`-released (the host raises StackEscape);
//! argument temps are retired after the call, in the order the
//! equivalent hand-written direct call retires them (arg, sink,
//! sink ctx, then the entry register is consumed with `^`).

use sla_ast::stmts::GoStmt;
use sla_ast::{Node, Type};

use crate::codegen::Codegen;
use crate::{Result, TextError};

impl<'c> Codegen<'c> {
    /// Statement position: spawn + `sa_task_detach(handle)`
    /// (Zig syntax_lowering.zig DEF-R1: a bare `go` statement
    /// detaches; the detach rc is an ordinary value binding,
    /// released at once here).
    pub(crate) fn gen_supervised_go_stmt(&mut self, go: &GoStmt) -> Result<()> {
        let handle = self.emit_supervised_spawn(go)?;
        let rc = self.new_tmp();
        self.ins(&format!("{rc} = call @sa_task_detach({handle})"));
        self.emit_release(&rc)?;
        self.emit_release(&handle)?;
        Ok(())
    }

    /// Expression position: the spawn's value is the join
    /// handle loaded out of the slot.
    pub(crate) fn gen_supervised_go_expr(&mut self, go: &GoStmt) -> Result<String> {
        self.emit_supervised_spawn(go)
    }

    fn emit_supervised_spawn(&mut self, go: &GoStmt) -> Result<String> {
        // The checker has already gated the operand's shape
        // (named void callee, arity 0 or 1); these arms are
        // defence in depth so an ungated tree fails loudly here
        // instead of emitting a malformed spawn.
        let Node::CallExpr(call) = &*go.call else {
            return Err(TextError::Unsupported {
                feature: "go supervised operand is not a call expression".to_string(),
                zig_ref: "syntax_lowering.zig:500 (emitSpawn)",
            });
        };
        if call.callee.is_some() || call.associated_target.is_some() {
            return Err(TextError::Unsupported {
                feature: "go supervised callee is not a plain named function".to_string(),
                zig_ref: "syntax_lowering.zig:500 (emitSpawn)",
            });
        }
        if call.args.len() > 1 {
            return Err(TextError::Unsupported {
                feature: format!(
                    "go supervised arity {} (expected 0 or 1)",
                    call.args.len()
                ),
                zig_ref: "syntax_lowering.zig:500 (emitSpawn)",
            });
        }

        // Entry: the callee's fn-ptr vtable address, passed to
        // the raw `*entry` parameter in deref form — the exact
        // shape a hand-written direct call takes.
        let entry = self.new_tmp();
        let vt = self.fn_ptr_vtable_name(&call.func_name);
        self.ins(&format!("{entry} = &{vt}"));

        // The single argument word (Zig: the arg node itself, or
        // a dummy zeroed slot's address for a zero-arg callee).
        let mut release_after: Vec<String> = Vec::new();
        let arg_display: String = if call.args.len() == 1 {
            let arg_node = &call.args[0];
            let reg = self.gen_expr(arg_node)?;
            if let Some(inner) = reg.strip_prefix('^') {
                // `^expr` payload: the checker keeps ownership
                // with the caller for go spawns (GoNoConsume), so
                // pass the value word and retire the temp after
                // the call like any other argument temp.
                let inner = inner.to_string();
                release_after.push(inner.clone());
                inner
            } else if let Node::Identifier(name) = arg_node {
                let resolved = self.resolve_binding_name(name);
                if reg == resolved || reg == *name {
                    match self.expr_type(arg_node) {
                        Some(ty) if matches!(ty, Type::Primitive(_)) => {
                            let copied = self.new_tmp();
                            self.emit_primitive_copy(&copied, &reg, &ty)?;
                            release_after.push(copied.clone());
                            copied
                        }
                        // A non-primitive binding register is
                        // passed under the extern arg plan's own
                        // materialisation; never release the
                        // caller's binding here.
                        _ => reg,
                    }
                } else {
                    release_after.push(reg.clone());
                    reg
                }
            } else {
                if sla_lowering::call_arg_needs_release(arg_node) {
                    release_after.push(reg.clone());
                }
                reg
            }
        } else {
            let dummy = self.new_tmp();
            self.ins(&format!("{dummy} = stack_alloc 8"));
            self.ins(&format!("store {dummy}+0, 0 as u64"));
            format!("&{dummy}")
        };

        // Zeroed handle slot + null trap sink / sink context
        // (corpus form: PTR_NULL()).
        let slot = self.new_tmp();
        self.ins(&format!("{slot} = stack_alloc 8"));
        self.ins(&format!("store {slot}+0, 0 as u64"));
        let sink = self.new_tmp();
        self.ins(&format!("EXPAND PTR_NULL {sink}"));
        let sink_ctx = self.new_tmp();
        self.ins(&format!("EXPAND PTR_NULL {sink_ctx}"));

        let rc = self.new_tmp();
        self.ins(&format!(
            "{rc} = call @sa_task_spawn_supervised(*{entry}, {arg_display}, *{sink}, *{sink_ctx}, &{slot})"
        ));
        for reg in &release_after {
            self.emit_release(reg)?;
        }
        self.emit_release(&sink)?;
        self.emit_release(&sink_ctx)?;
        self.emit_forget_moved_value(&entry)?;

        // Spawn rc gate (SPAWN_PANIC_CODE 60901).
        let cond = self.new_tmp();
        self.ins(&format!("{cond} = ne {rc}, 0"));
        self.emit_release(&rc)?;
        let fail_label = self.new_label("L_GO_SPAWN_FAIL");
        let ok_label = self.new_label("L_GO_SPAWN_OK");
        self.ins(&format!("br {cond} -> {fail_label}, {ok_label}"));
        self.wl(&format!("{fail_label}:"));
        self.ins("panic(60901)");
        self.wl(&format!("{ok_label}:"));
        self.emit_release(&cond)?;

        let val = self.new_tmp();
        self.ins(&format!("{val} = load {slot}+0 as u64"));
        Ok(val)
    }
}

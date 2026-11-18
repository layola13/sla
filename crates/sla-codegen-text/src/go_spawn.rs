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
//! Plain (unsupervised) `go` in expression position is
//! emitted here too (F-3): the same sequence with
//! `sa_task_spawn(entry, arg, &slot)` and no sink pair,
//! mirroring Zig syntax_lowering.zig `emitSpawn`
//! (detached=false). Statement position is the detached form
//! (emitSpawn detached=true, DEF-R1): spawn + `sa_task_detach`
//! + releases, exactly the supervised statement shape.
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
        let handle = self.emit_spawn(go, true)?;
        let rc = self.new_tmp();
        self.ins(&format!("{rc} = call @sa_task_detach({handle})"));
        self.emit_release(&rc)?;
        self.emit_release(&handle)?;
        Ok(())
    }

    /// Expression position: the spawn's value is the join
    /// handle loaded out of the slot.
    pub(crate) fn gen_supervised_go_expr(&mut self, go: &GoStmt) -> Result<String> {
        self.emit_spawn(go, true)
    }

    /// Expression position, plain `go` (F-3): the spawn's
    /// value is the join handle loaded out of the slot.
    pub(crate) fn gen_plain_go_expr(&mut self, go: &GoStmt) -> Result<String> {
        self.emit_spawn(go, false)
    }

    /// Statement position, plain `go`: spawn + `sa_task_detach(handle)`
    /// (Zig syntax_lowering.zig emitSpawn detached=true, DEF-R1: a
    /// bare `go` statement discards the handle, so the runtime is
    /// told to reap the completion record at exit). Same shape as
    /// the supervised statement form above.
    pub(crate) fn gen_plain_go_stmt(&mut self, go: &GoStmt) -> Result<()> {
        let handle = self.emit_spawn(go, false)?;
        let rc = self.new_tmp();
        self.ins(&format!("{rc} = call @sa_task_detach({handle})"));
        self.emit_release(&rc)?;
        self.emit_release(&handle)?;
        Ok(())
    }

    fn emit_spawn(&mut self, go: &GoStmt, supervised: bool) -> Result<String> {
        let kind = if supervised { "go supervised" } else { "go" };
        // The checker has already gated the operand's shape
        // (named void callee, arity 0 or 1); these arms are
        // defence in depth so an ungated tree fails loudly here
        // instead of emitting a malformed spawn.
        let Node::CallExpr(call) = &*go.call else {
            return Err(TextError::Unsupported {
                feature: format!("{kind} operand is not a call expression"),
                zig_ref: "syntax_lowering.zig:500 (emitSpawn)",
            });
        };
        if call.callee.is_some() || call.associated_target.is_some() {
            return Err(TextError::Unsupported {
                feature: format!("{kind} callee is not a plain named function"),
                zig_ref: "syntax_lowering.zig:500 (emitSpawn)",
            });
        }
        if call.args.len() > 1 {
            return Err(TextError::Unsupported {
                feature: format!(
                    "{kind} arity {} (expected 0 or 1)",
                    call.args.len()
                ),
                zig_ref: "syntax_lowering.zig:500 (emitSpawn)",
            });
        }

        // Entry: the callee's fn-ptr vtable address. The raw
        // `*entry` parameter of sa_task_spawn{,_supervised}
        // must receive the function pointer stored in the
        // vtable's call slot, not the vtable address itself:
        // passing `*` of the vtable-address register starts the
        // goroutine at a data address and the process faults
        // later at join/recv (LINE-SPAWN-ENTRY; proven by the
        // LINE-MONITOR SA-level mutation and by the Zig
        // emitter's SA for both the named-function form and
        // the local fn-ptr binding form, which both load the
        // call slot first). The load+cast chain is therefore
        // materialised just before the spawn call below, for
        // both entry shapes.
        //
        // F-G8: a named function has no runtime register, so its
        // vtable address is materialized into a fresh temp. A
        // local fn-ptr binding (e.g. supervise's `entry: fn(u64)`
        // parameter in sa_std/supervisor.sla) already HOLDS the
        // vtable address as its register value — Zig
        // syntax_lowering emitSpawn passes the bare identifier
        // through the ordinary extern-argument plan, and Zig
        // codegen's identifier arm returns the binding register
        // itself for non-function names. Minting
        // `SLA_FNPTR_VT_<binding>` here produced an undeclared
        // register (UnknownRegister), because program.rs only
        // emits vtable decls for FuncDecl names.
        let named_fn = self.func_decl(&call.func_name).is_some()
            || self.check.is_extern_func(&call.func_name);
        let (entry, entry_is_temp) = if named_fn {
            let entry = self.new_tmp();
            let vt = self.fn_ptr_vtable_name(&call.func_name);
            self.ins(&format!("{entry} = &{vt}"));
            (entry, true)
        } else {
            (self.resolve_binding_name(&call.func_name), false)
        };

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

        // Zeroed handle slot; supervised spawns also pass a
        // null trap sink / sink context pair (corpus form:
        // PTR_NULL()).
        let slot = self.new_tmp();
        self.ins(&format!("{slot} = stack_alloc 8"));
        self.ins(&format!("store {slot}+0, 0 as u64"));
        let mut sink: Option<String> = None;
        let mut sink_ctx: Option<String> = None;
        if supervised {
            let sk = self.new_tmp();
            self.ins(&format!("EXPAND PTR_NULL {sk}"));
            let sc = self.new_tmp();
            self.ins(&format!("EXPAND PTR_NULL {sc}"));
            sink = Some(sk);
            sink_ctx = Some(sc);
        }

        // Materialise the entry function pointer out of the
        // vtable call slot (offset 0, ptr width, cast to ptr -
        // the exact chain in the Zig emitter's SA and in the
        // LINE-MONITOR mutation): loaded = load {entry}+0 as
        // ptr; entry_fn = loaded as ptr; !loaded, then the
        // spawn takes *entry_fn and entry_fn is retired first
        // after the call. For a local fn-ptr binding the
        // binding register itself stays live (it is re-spawned
        // per loop iteration); only the loaded/cast temps are
        // retired here.
        let entry_loaded = self.new_tmp();
        self.ins(&format!("{entry_loaded} = load {entry}+0 as ptr"));
        let entry_fn = self.new_tmp();
        self.ins(&format!("{entry_fn} = {entry_loaded} as ptr"));
        self.emit_release(&entry_loaded)?;

        let rc = self.new_tmp();
        if supervised {
            let sk = sink.as_ref().expect("supervised sink");
            let sc = sink_ctx.as_ref().expect("supervised sink ctx");
            self.ins(&format!(
                "{rc} = call @sa_task_spawn_supervised(*{entry_fn}, {arg_display}, *{sk}, *{sc}, &{slot})"
            ));
        } else {
            self.ins(&format!(
                "{rc} = call @sa_task_spawn(*{entry_fn}, {arg_display}, &{slot})"
            ));
        }
        self.emit_release(&entry_fn)?;
        for reg in &release_after {
            self.emit_release(reg)?;
        }
        if let Some(sk) = &sink {
            self.emit_release(sk)?;
        }
        if let Some(sc) = &sink_ctx {
            self.emit_release(sc)?;
        }
        // Only the materialized vtable temp is forgotten; a
        // local fn-ptr binding stays live (supervise spawns it
        // once per loop iteration).
        if entry_is_temp {
            self.emit_forget_moved_value(&entry)?;
        }

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

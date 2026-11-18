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

use sla_ast::{FnPtrType, Literal, Node, Param, Primitive, Type, UserDefined};
use sla_lowering::{
    abi_call_arg_cap_kind, abi_param_needs_borrow_arg, by_value_raw_pointer_param,
    call_arg_is_copy_struct_value, call_arg_needs_release, is_borrow_like_type,
    is_panic_builtin_name, is_println_call, is_temporary_register_name,
    plan_call_arg_materialization, plan_refcell_call_arg_lifecycle,
    plan_stack_slot_identifier_call_arg_temp, root_identifier,
    value_arg_transfers_ownership_from_param, AbiCapKind, CallArgMaterializationInput,
    CallArgMaterializationKind, CallArgTarget,
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
    /// Value, AutoBorrow and ArrayToSliceBorrow arms are ported;
    /// every other plan kind belongs to a deferred family and
    /// fails loudly.
    pub(crate) fn gen_call_arg_from_materialization_plan(
        &mut self,
        arg: &Node,
        param: Option<&Param>,
        plan: &sla_lowering::CallArgMaterializationPlan,
    ) -> Result<LoweredCallArg> {
        match plan.kind {
            CallArgMaterializationKind::Value => {
                // R1b (stage-5a resume): Zig BM-D2 (codegen.zig
                // ~21843/21923/22009, method / known-function /
                // associated-target call paths): a string literal for
                // a String-typed parameter materializes as an owned
                // String handle, not the stack Slice view. A by-value
                // String parameter consumes its argument (`^s`
                // contract): the owned handle is MOVED in like a
                // format() result. Passing the slice plain traps
                // CapabilityMismatch; moving the slice traps
                // StackEscape ("stack allocation cannot be passed by
                // move"). A borrow parameter takes the owned handle
                // plain and releases it after the call.
                if let Node::Literal(Literal::StringVal(text)) = arg {
                    if let Some(p) = param {
                        if crate::codegen::is_format_string_type(&p.ty) {
                            let owned = self.gen_owned_string_literal(text)?;
                            if p.is_borrow {
                                return Ok(LoweredCallArg {
                                    reg: owned,
                                    release_after_call: true,
                                    release_reg: None,
                                    consume_reg: None,
                                });
                            }
                            let mv = format!("^{owned}");
                            return Ok(LoweredCallArg {
                                reg: mv.clone(),
                                release_after_call: false,
                                release_reg: None,
                                consume_reg: Some(mv),
                            });
                        }
                    }
                }
                // R2b (stage 6): a literal argument takes its
                // layout from the parameter's declared type.
                let raw_reg =
                    self.gen_expr_in_context(arg, param.map(|p| &*p.ty))?;
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
                    // Zig REG-BR-6 self-assignment move-out
                    // (codegen.zig ~12979): when this identifier
                    // argument IS the assignment target in the tail
                    // call chain, the call consumes the binding
                    // directly and the assignment rebinds the slot
                    // (gen_assign_stmt) — no clone. Otherwise, when
                    // the identifier must stay live for a later use
                    // (liveness.rs), clone the Vec/GrowVec and move
                    // the clone; the source binding stays live and
                    // owning. The consume_reg carries the ^-prefixed
                    // register (as in the Zig): its forget is a
                    // bookkeeping mark only, since the call operand's
                    // own `^` already consumed the value — a raw
                    // consume_reg here emitted a second `^reg` line
                    // and the host rejected it with UseAfterMove.
                    if let Node::Identifier(name) = arg {
                        if self.is_self_assign_move_out_arg(arg) {
                            self.claim_self_assign_move_out(arg);
                        } else if self.identifier_must_stay_live_for_later_use(name)
                            || self.identifier_used_in_later_sibling(name)
                        {
                            if let Some(ty) = self.expr_type(arg) {
                                if let Some((is_grow, elem)) =
                                    crate::place::vec_like_element_type(&ty)
                                {
                                    let cloned = if is_grow {
                                        self.gen_growvec_clone_value(&raw_reg, &elem)
                                    } else {
                                        self.gen_vec_clone_value(&raw_reg, &elem)
                                    };
                                    let mv = format!("^{cloned}");
                                    return Ok(LoweredCallArg {
                                        reg: mv.clone(),
                                        release_after_call: false,
                                        release_reg: None,
                                        consume_reg: Some(mv),
                                    });
                                }
                                // Stay-live deep clone: fires for a
                                // deep-cloneable value whose source
                                // binding is read again after the call
                                // (REG3-DEF-2 preserved-argument shape,
                                // test_unit_recursive_struct_clone),
                                // and for a deep-cloneable aggregate
                                // read by a later sibling (SPRINT-100
                                // L2: `merge(world, get_values(world))`
                                // — the clone transfers, the source
                                // binding stays live and owning).
                                if self.identifier_used_in_later_sibling(name)
                                    || (self.deep_clone_needed(&ty, 0)
                                        && self.uncloneable_type_name(&ty, 0).is_none())
                                {
                                    if let Some(cloned) =
                                        self.gen_deep_clone_value(&raw_reg, &ty)?
                                    {
                                        let mv = format!("^{cloned}");
                                        return Ok(LoweredCallArg {
                                            reg: mv.clone(),
                                            release_after_call: false,
                                            release_reg: None,
                                            consume_reg: Some(mv),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    // RUST-STAGE10-CLONE-EXTRACT (shape C): a
                    // projection argument (field/index read out of
                    // a live parent) passed by value aliases the
                    // parent's buffers — the callee frees them and
                    // the parent's exit cleanup frees them again
                    // (Zig genPlannedCallArg SCATTER-3 / DEF-PARALLEL:
                    // clone a non-identifier projection with a root
                    // identifier). Clone the extracted value so the
                    // callee owns a fresh copy; the source binding
                    // stays live and owning. Bare identifiers are
                    // moves (never cloned); a projection with no
                    // identifier root (call-result temp) is already
                    // freshly owned, so it moves as-is; map/set
                    // types keep the historical raw pass.
                    // SPRINT-100 L2: the clone attempt is no longer
                    // gated on deep_clone_needed — a POD struct field
                    // (SCATTER-3's {ptr, len} slice) needs the fresh
                    // record just as much: the callee drops the
                    // record it was given, so the aliasing load is
                    // freed under the still-live parent.
                    // gen_deep_clone_value declines plain-copyable
                    // types (None), which keeps scalar projections
                    // on the raw pass.
                    if !raw_reg.starts_with('&') && !raw_reg.starts_with('^') {
                        if let Some(arg_ty) = self.expr_type(arg) {
                            if root_identifier(arg).is_some()
                                && matches!(arg, Node::FieldExpr(_) | Node::IndexExpr(_))
                                && self.uncloneable_type_name(&arg_ty, 0).is_none()
                            {
                                if let Some(cloned) =
                                    self.gen_deep_clone_value(&raw_reg, &arg_ty)?
                                {
                                    // The source temp holds the
                                    // aliased word: retire it (a
                                    // temp has no binding type, so
                                    // this is a plain `!reg`, never
                                    // a field drop — the REG3-DEF-2
                                    // shape in extern_macro.rs).
                                    self.emit_release(&raw_reg)?;
                                    let mv = format!("^{cloned}");
                                    return Ok(LoweredCallArg {
                                        reg: mv.clone(),
                                        release_after_call: false,
                                        release_reg: None,
                                        consume_reg: Some(mv),
                                    });
                                }
                            }
                        }
                    }
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
                // A BorrowExpr argument already yields the borrow
                // operand itself (gen_borrow_expr returns the slot
                // address for index/field places, `&name` for
                // identifiers): prefixing another `&` addresses
                // the address register (CapabilityMismatch on
                // Vec<&T> pushes of &g[i]).
                if needs_borrow
                    && !raw_reg.starts_with('&')
                    && !matches!(arg, Node::BorrowExpr(_))
                {
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
                let raw_reg =
                    self.gen_expr_in_context(arg, param.map(|p| &*p.ty))?;
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
            CallArgMaterializationKind::ArrayToSliceBorrow => {
                self.gen_array_borrow_to_slice_arg(arg)
            }
            kind => Err(crate::TextError::Unsupported {
                feature: format!("call arg materialization kind {kind:?}"),
                zig_ref: "codegen.zig:9860 (genCallArgFromMaterializationPlan)",
            }),
        }
    }

    /// codegen.zig `genArrayBorrowToSliceArg`: materialize a
    /// slice view of the borrowed array and pass a borrow of the
    /// view slot; the array's base register is released after
    /// the call only when the borrowed expression's result needs
    /// release (the view itself is a stack slot, forgotten with
    /// the frame like Zig's).
    pub(crate) fn gen_array_borrow_to_slice_arg(
        &mut self,
        arg: &Node,
    ) -> Result<LoweredCallArg> {
        let slice_reg = self.new_tmp();
        let base_release_reg = self.gen_array_borrow_to_slice_into(&slice_reg, arg)?;
        Ok(LoweredCallArg {
            reg: format!("&{slice_reg}"),
            release_after_call: false,
            release_reg: base_release_reg,
            consume_reg: None,
        })
    }

    /// codegen.zig `genArrayBorrowToSliceInto`: emit the slice
    /// view of `arg`'s array operand into `target` (a stack
    /// Slice slot); returns the base source register when it
    /// must be released after the call, else None.
    fn gen_array_borrow_to_slice_into(
        &mut self,
        target: &str,
        arg: &Node,
    ) -> Result<Option<String>> {
        let Node::BorrowExpr(b) = arg else {
            return unsupported(
                "array-to-slice borrow of a non-borrow argument",
                "codegen.zig (genArrayBorrowToSliceInto)",
            );
        };
        let inner = &b.expr;
        let inner_ty = self.expr_type(inner).ok_or_else(|| {
            crate::TextError::MissingType {
                context: "array-to-slice borrow operand type".to_string(),
            }
        })?;
        let Type::Array(arr) = &inner_ty else {
            return unsupported(
                "array-to-slice borrow of a non-array operand",
                "codegen.zig (genArrayBorrowToSliceInto)",
            );
        };
        let arr_len = arr.len;
        let base_source_reg = self.gen_expr(inner)?;
        let base_reg = if let Node::Identifier(name) = &**inner {
            if self.global_const_bindings.contains(name) {
                let addr_reg = self.new_tmp();
                self.ins(&format!("{addr_reg} = &{base_source_reg}"));
                addr_reg
            } else {
                base_source_reg.clone()
            }
        } else {
            base_source_reg.clone()
        };
        let len_reg = self.new_tmp();
        self.emit_int_const(&len_reg, arr_len as i64);
        self.stack_alloc_bindings.insert(target.to_string());
        self.ins(&format!("{target} = stack_alloc Slice_SIZE"));
        self.ins(&format!(
            "EXPAND SLICE_NEW {target}, {base_reg}, {len_reg}"
        ));
        self.emit_release(&len_reg)?;
        if base_reg != base_source_reg {
            self.emit_release(&base_reg)?;
        }
        Ok(if sla_lowering::expr_result_needs_release(inner) {
            Some(base_source_reg)
        } else {
            None
        })
    }

    /// Shared argument planning for expr/statement calls to a known
    /// function: returns the display registers and the planned args.
    fn plan_known_call_args(
        &mut self,
        call: &sla_ast::CallExpr,
        params: &[Param],
    ) -> Result<(Vec<String>, Vec<LoweredCallArg>)> {
        let mut planned_args: Vec<LoweredCallArg> = Vec::new();
        // SPRINT-100 L2: sibling frame (Zig
        // `current_expr_later_nodes`) — while argument i is
        // planned, arguments i+1.. are the later siblings a
        // consuming argument must not move out from under.
        self.push_expr_later_frame(
            call.args.iter().map(|a| a as *const sla_ast::Node).collect(),
        );
        let mut plan_result: Result<()> = Ok(());
        for (i, arg) in call.args.iter().enumerate() {
            self.set_expr_later_frame_index(i);
            let param = params.get(i);
            match self.gen_planned_call_arg(arg, param, i == 0) {
                Ok(planned) => planned_args.push(planned),
                Err(e) => {
                    plan_result = Err(e);
                    break;
                }
            }
        }
        self.pop_expr_later_frame();
        plan_result?;
        let display: Vec<String> = planned_args.iter().map(|p| p.reg.clone()).collect();
        Ok((display, planned_args))
    }

    /// Resolve an associated call's target to a registered
    /// function declaration: user impl / overload methods live
    /// under the method-mangled key (`Target_func`), imported
    /// module functions under the module key (`target__func` or
    /// `target::func`); a unique tail match rooted at the target
    /// is the last resort (mirrors the SAB driver's tail rule).
    pub(crate) fn resolve_associated_decl(
        &self,
        call: &sla_ast::CallExpr,
    ) -> Option<(String, sla_ast::FuncDecl)> {
        let target = call.associated_target.as_deref()?;
        let func = call.func_name.as_str();
        let mangled = sla_lowering::mangle_method_name(target, func);
        for (key, d) in &self.assoc_index {
            if key == &mangled {
                return Some((key.clone(), d.clone()));
            }
        }
        let candidates = [
            mangled,
            format!("{target}__{func}"),
            format!("{target}::{func}"),
        ];
        for key in &candidates {
            if let Some(d) = self.func_decl(key) {
                return Some((key.clone(), d));
            }
        }
        if let Some((key, d)) = self.func_decl_tail_match(func) {
            if key.starts_with(target) {
                return Some((key, d));
            }
        }
        None
    }

    /// Receiver-typed method resolution for ordinary method
    /// calls (`recv.m(...)` keeps the bare name and the receiver
    /// as args[0]): the receiver's peeled type name keys the
    /// assoc index. Consulted only after the bare-name lookup
    /// misses, before the tail match.
    pub(crate) fn resolve_receiver_method_decl(
        &self,
        call: &sla_ast::CallExpr,
    ) -> Option<(String, sla_ast::FuncDecl)> {
        let first = call.args.first()?;
        let ty = self.expr_type(first)?;
        let mut cur = &ty;
        let name = loop {
            match cur {
                sla_ast::Type::Borrow(inner) | sla_ast::Type::Pointer(inner) => cur = inner,
                sla_ast::Type::UserDefined(u) => break u.name.as_str(),
                _ => return None,
            }
        };
        let mangled = sla_lowering::mangle_method_name(name, &call.func_name);
        for (key, d) in &self.assoc_index {
            if key == &mangled {
                return Some((key.clone(), d.clone()));
            }
        }
        None
    }

    /// codegen.zig `isMemForgetCall` arms (genCallStmt and the
    /// genExpr associated dispatch): `mem::forget(value)`
    /// evaluates the value, emits `EXPAND MEM_FORGET_U64 <reg>`
    /// plus a plain `!<reg>` retire — never the emitRelease drop
    /// cascade, the value is intentionally leaked, not dropped —
    /// and marks the argument's root binding consumed. The
    /// expression form returns a dummy register (the Zig returns
    /// its return_ty_sentinel; the surface Void precedent here).
    pub(crate) fn gen_mem_forget_call(
        &mut self,
        call: &sla_ast::CallExpr,
    ) -> Result<String> {
        if call.args.len() != 1 {
            return Err(crate::TextError::Unsupported {
                feature: format!(
                    "mem::forget arity {} (expected 1)",
                    call.args.len()
                ),
                zig_ref: "codegen.zig isMemForgetCall arm (genCallStmt/genExpr)",
            });
        }
        let value_reg = self.gen_expr(&call.args[0])?;
        self.ins(&format!("EXPAND MEM_FORGET_U64 {value_reg}"));
        self.ins(&format!("!{value_reg}"));
        if let Some(name) = root_identifier(&call.args[0]) {
            self.mark_consumed_binding(name);
        }
        Ok(self.new_tmp())
    }

    /// codegen.zig:14600 associated-call dispatch for the shapes
    /// the std surface does not claim: targeted Option/Result
    /// constructors (same EXPAND forms as the bare-name arms) and
    /// plain calls to a resolved declaration (impl methods,
    /// imported module functions). Returns None when neither
    /// applies, leaving the caller to fail loudly.
    fn gen_associated_call(
        &mut self,
        _expr: &Node,
        call: &sla_ast::CallExpr,
    ) -> Result<Option<String>> {
        let target = call.associated_target.as_deref().unwrap_or("");
        if is_mem_forget_call(call) {
            return Ok(Some(self.gen_mem_forget_call(call)?));
        }
        if matches!(target, "Option" | "Result") {
            match call.func_name.as_str() {
                "Some" | "Ok" | "Err" if call.args.len() == 1 => {
                    let expand_name = match call.func_name.as_str() {
                        "Some" => "OPTION_NEW_SOME",
                        "Ok" => "RESULT_NEW_OK",
                        _ => "RESULT_NEW_ERR",
                    };
                    let planned = self.gen_planned_call_arg(&call.args[0], None, false)?;
                    let result = self.new_tmp();
                    self.ins(&format!("EXPAND {expand_name} {result}, {}", planned.reg));
                    let planned_args = vec![planned];
                    self.emit_lowered_call_arg_cleanups(&planned_args, None)?;
                    return Ok(Some(result));
                }
                "None" if call.args.is_empty() => {
                    let result = self.new_tmp();
                    self.ins(&format!("EXPAND OPTION_NEW_NONE {result}"));
                    return Ok(Some(result));
                }
                _ => {}
            }
        }
        // Chan::new (F-G1): the goroutine-batch constructor. In
        // the Zig this shape never reaches codegen as an
        // associated call — syntax_lowering.zig `emitChanCall`
        // (.new arm) rewrites it to a `sa_chan_make` extern call
        // over a zeroed handle slot, an rc check (panic 60911)
        // and a value copy of the handle out of the slot. The
        // text lane has no syntax-lowering stage, so the same
        // instruction sequence is emitted here directly, in the
        // lowering's order.
        if target == "Chan" && call.func_name == "new" {
            if call.args.len() != 1 {
                return Err(crate::TextError::Unsupported {
                    feature: format!(
                        "Chan::new arity {} (expected 1)",
                        call.args.len()
                    ),
                    zig_ref: "syntax_lowering.zig emitChanCall (.new arm)",
                });
            }
            let cap_reg = self.gen_expr(&call.args[0])?;
            // The `as` cast below consumes its source register.
            // When the argument is an identifier whose register
            // IS the caller's binding, copy it first — the same
            // guard the cast_expr arm applies (codegen.zig:17018).
            // Every other argument shape already yields a
            // disposable temp, which the cast itself retires, so
            // no separate release of the source is emitted.
            let mut src_reg = cap_reg.clone();
            if let Node::Identifier(name) = &call.args[0] {
                if let Some(src_ty) = self.expr_type(&call.args[0]) {
                    if matches!(src_ty, Type::Primitive(_)) {
                        let resolved = self.resolve_binding_name(name);
                        if cap_reg == resolved || cap_reg == *name {
                            let copied = self.new_tmp();
                            self.emit_primitive_copy(&copied, &cap_reg, &src_ty)?;
                            src_reg = copied;
                        }
                    }
                }
            }
            let cap_u32 = self.new_tmp();
            self.ins(&format!("{cap_u32} = {src_reg} as u32"));
            let slot = self.new_tmp();
            self.ins(&format!("{slot} = stack_alloc 8"));
            self.ins(&format!("store {slot}+0, 0 as u64"));
            let rc = self.new_tmp();
            self.ins(&format!("{rc} = call @sa_chan_make({cap_u32}, &{slot})"));
            self.emit_release(&cap_u32)?;
            let cond = self.new_tmp();
            self.ins(&format!("{cond} = ne {rc}, 0"));
            self.emit_release(&rc)?;
            let fail_label = self.new_label("L_CHAN_MAKE_FAIL");
            let ok_label = self.new_label("L_CHAN_MAKE_OK");
            self.ins(&format!("br {cond} -> {fail_label}, {ok_label}"));
            self.wl(&format!("{fail_label}:"));
            self.ins("panic(60911)");
            self.wl(&format!("{ok_label}:"));
            self.emit_release(&cond)?;
            let val = self.new_tmp();
            self.ins(&format!("{val} = load {slot}+0 as u64"));
            // The stack_alloc slot itself is released implicitly
            // at scope exit — an explicit `!` on it is a host
            // StackEscape error.
            return Ok(Some(val));
        }
        if let Some((key, decl)) = self.resolve_associated_decl(call) {
            let (display, planned_args) = self.plan_known_call_args(call, &decl.params)?;
            let result = self.new_tmp();
            let lowered = self.lowered_func_symbol(&key);
            self.ins(&format!(
                "{result} = call @{lowered}({})",
                display.join(", ")
            ));
            self.emit_lowered_call_arg_cleanups(&planned_args, Some(&key))?;
            return Ok(Some(result));
        }
        Ok(None)
    }

    /// Indirect call through a fn-pointer-typed binding
    /// (RUST-FNPTR-DIRECT-CALL-FORM1). Zig codegen.zig `fn_ptr_calls`
    /// arm (sa_plugin_sla 87950412): the callee register holds the
    /// vtable address, so `load +0` reads the vtable's `call` slot
    /// and the call carries the fn-pointer type's signature when
    /// every component renders to an SA primitive. The 114 demo's
    /// `load cb+8` shape is the 16-byte object form (imported-macro
    /// args, genImportedMacroFnObjectArg), not the value form — a
    /// param/let/capture register never goes through `+8`.
    /// Returns the fn-pointer type and the register holding the
    /// value when `func_name` resolves to a FnPtr-typed local
    /// binding; None otherwise (caller falls through to the
    /// existing arms, so genuinely unknown names still EXPAND).
    fn fn_ptr_indirect_target(
        &self,
        func_name: &str,
    ) -> Option<(FnPtrType, String)> {
        // Zig checks thread_capture_regs first: a captured binding's
        // register is the worker slot-load temp, not the bare name;
        // its type stays registered under the bare name.
        if let Some(reg) = self.thread_capture_regs.get(func_name) {
            let fp = match self.local_binding_type_for_name(func_name)? {
                Type::FnPtr(fp) => fp,
                _ => return None,
            };
            return Some((fp, reg.clone()));
        }
        // Repeated-let aliasing renames a binding (e.g. `let run`
        // in two loops becomes tmp_9 / tmp_26) and the let
        // registers its type under the ALIASED name, so look the
        // type up by the raw name first, then by the resolved one.
        let reg = self.resolve_binding_name(func_name);
        let ty = self
            .local_binding_type_for_name(func_name)
            .or_else(|| self.local_binding_type_for_name(&reg))?;
        let fp = match ty {
            Type::FnPtr(fp) => fp,
            _ => return None,
        };
        Some((fp, reg))
    }

    /// Synthetic `Param`s for a fn-pointer type's parameters,
    /// mirroring Zig's inline construction in the `fn_ptr_calls`
    /// arm: borrow when the parameter type is a borrow; move when
    /// it is neither a by-value raw pointer nor Copy nor
    /// borrow-like — so a by-value owned argument is consumed by
    /// the callee exactly once, as in a direct call.
    fn fn_ptr_call_params(&self, fp: &FnPtrType) -> Vec<Param> {
        fp.params
            .iter()
            .map(|ty| {
                let mut param = Param {
                    name: String::new(),
                    ty: Box::new(ty.clone()),
                    is_borrow: matches!(ty, Type::Borrow(_)),
                    is_move: false,
                    fallible_sugar: false,
                    is_var: false,
                };
                param.is_move = !by_value_raw_pointer_param(&param)
                    && !self.type_is_copy_value(ty)
                    && !is_borrow_like_type(ty);
                param
            })
            .collect()
    }

    /// C2 sab_carried_indirect_sig: SA primitive name for one
    /// component of a carried fn-pointer signature. Mirrors Zig
    /// `fnPtrSigParamName` (bool params ride i1); a type with no
    /// SA primitive rendering yields None and the call carries
    /// no record (Zig's `catch null`). The return component is
    /// rendered by `fn_ptr_carried_sig` itself: a boolean return
    /// must read `u8`, the name the callee's own definition
    /// header carries (sla_lowering abi type name), because the
    /// host resolves a text-path `call_indirect` against the
    /// record as declared — an `i32` record against a `u8`
    /// callee corrupts the result value (LINE-FNPTR-REMAINDER;
    /// Zig's `fnPtrSigRetName` i32 mirrors the SAB backend's
    /// abi return type, whose record the host reads differently).
    fn fn_ptr_sig_param_name(ty: &Type) -> Option<&'static str> {
        match ty {
            Type::Primitive(p) => Some(match p {
                Primitive::I8 => "i8",
                Primitive::I16 => "i16",
                Primitive::I32 => "i32",
                Primitive::I64 | Primitive::Integer | Primitive::Isize => "i64",
                Primitive::U8 => "u8",
                Primitive::U16 => "u16",
                Primitive::U32 => "u32",
                Primitive::U64 | Primitive::Usize => "u64",
                Primitive::F32 => "f32",
                Primitive::F64 | Primitive::Float => "f64",
                Primitive::Boolean => "i1",
                Primitive::RawPtr => "ptr",
                Primitive::VoidType => "void",
                _ => return None,
            }),
            Type::Pointer(_) | Type::Borrow(_) => Some("ptr"),
            _ => None,
        }
    }

    fn fn_ptr_carried_sig(fp: &FnPtrType) -> Option<String> {
        let mut params = Vec::new();
        for ty in &fp.params {
            params.push(Self::fn_ptr_sig_param_name(ty)?);
        }
        let ret = if matches!(*fp.ret, Type::Primitive(Primitive::Boolean)) {
            "u8"
        } else {
            Self::fn_ptr_sig_param_name(&fp.ret)?
        };
        Some(format!("({}) -> {}", params.join(", "), ret))
    }

    /// Emit the indirect call itself: load the vtable's `call`
    /// slot, `call_indirect` (with the carried signature when
    /// renderable), release the loaded function pointer, then the
    /// planned argument cleanups — Zig's order (call-register
    /// release before cleanups). Returns the result register for
    /// non-void returns, None for void.
    fn emit_fn_ptr_indirect_call(
        &mut self,
        call: &sla_ast::CallExpr,
        fp: &FnPtrType,
        callee_reg: &str,
    ) -> Result<Option<String>> {
        let params = self.fn_ptr_call_params(fp);
        let (display, planned_args) = self.plan_known_call_args(call, &params)?;
        let fn_reg = self.new_tmp();
        self.ins(&format!("{fn_reg} = load {callee_reg}+0 as ptr"));
        let sig = Self::fn_ptr_carried_sig(fp)
            .map(|s| format!(" : {s}"))
            .unwrap_or_default();
        let args = display.join(", ");
        let result = if matches!(*fp.ret, Type::Primitive(Primitive::VoidType)) {
            self.ins(&format!("call_indirect {fn_reg}({args}){sig}"));
            None
        } else {
            let result = self.new_tmp();
            self.ins(&format!(
                "{result} = call_indirect {fn_reg}({args}){sig}"
            ));
            Some(result)
        };
        self.emit_release(&fn_reg)?;
        self.emit_lowered_call_arg_cleanups(&planned_args, None)?;
        Ok(result)
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
        // Future/task/executor/poll runtime calls (codegen.zig
        // genExpr call arm: genPollRuntimeCall / genExecutor-
        // RuntimeCall / genFutureRuntimeCall / genTaskRuntimeCall;
        // stage 6 async cluster).
        if let Some(reg) = self.gen_runtime_call(call)? {
            return Ok(reg);
        }
        // thread::spawn (codegen.zig `isThreadSpawnCall` arm;
        // thread_spawn.rs): the slot block is the JoinHandle.
        if crate::thread_spawn::is_thread_spawn_call(call) {
            return self.gen_thread_spawn_call(expr, call);
        }
        // Chan<T> method surface (chan.rs): send / recv /
        // destroy / close / len / cap emit the sa_chan_*
        // extern sequences directly (the lowering's
        // emitChanCall forms).
        if let Some((kind, payload_ty)) = self.chan_method(call) {
            return self.gen_chan_method_call(call, kind, &payload_ty);
        }
        // monitor(handle) (monitor.rs): the supervisor
        // builtin the checker claims in its pre-associated
        // block; emits the sa_task_monitor extern sequence
        // directly (the lowering's emitMonitorCall form).
        if self.is_monitor_call(call) {
            return self.gen_monitor_call(call);
        }
        if call.associated_target.is_some() {
            if let Some(reg) = self.gen_associated_call(expr, call)? {
                return Ok(reg);
            }
            return Err(crate::TextError::Unsupported {
                feature: format!(
                    "associated-target call target={:?} func={}",
                    call.associated_target, call.func_name
                ),
                zig_ref: "codegen.zig:14600 (associated call dispatch)",
            });
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
        if func_name == "str_eq" && call.args.len() == 2 {
            return self.gen_str_eq_call(call);
        }
        if let Some(view) = self.check.imported_macro(&func_name) {
            return self.gen_imported_macro_call_expr(call, &view);
        }
        if self.check.is_macro(&func_name) {
            return self.gen_user_macro_call(expr, call);
        }
        if let Some(sig) = self.check.extern_sig(&func_name) {
            return self.gen_extern_call(call, &sig);
        }

        // Fn-pointer value call: a callee name that resolves to a
        // FnPtr-typed local binding (param, let-local, closure
        // capture) calls through the vtable's `call` slot — never
        // the EXPAND catch-all. Placed before the known-function
        // lookup so a shadowing binding wins over a same-named
        // declared function, as the checker resolves the call to
        // the binding.
        if let Some((fp, callee_reg)) = self.fn_ptr_indirect_target(&func_name) {
            let result = self.emit_fn_ptr_indirect_call(call, &fp, &callee_reg)?;
            // A void fn-pointer in expression position has no
            // value; return a fresh placeholder, as gen_panic_call
            // does after a bare `panic(...)`.
            return Ok(result.unwrap_or_else(|| self.new_tmp()));
        }

        // collect::<String>() over an iterator chain
        // (codegen.zig:21752, via `stringCollectSource` /
        // `genStringCollect`): builds the String byte by byte.
        // Placed where the Zig cascade checks it — after the
        // fn-pointer path, before plain known-function
        // resolution; without it the chain degrades into the
        // EXPAND catch-all (`EXPAND iter` / `EXPAND collect`,
        // UndefinedMacro at the host).
        if let Some(source) = Self::string_collect_source(call) {
            return self.gen_string_collect(source);
        }

        // Plain known-function call; a `using` extension call
        // site keeps the bare name, so fall back to the unique
        // tail-segment match (SAB driver tail) before the
        // EXPAND catch-all.
        //
        // Overload sets first: when the checker recorded the
        // winning member's mangled symbol for this call, bind to
        // that member's declaration (the first-match lookup below
        // would bind every call to the first-declared member,
        // whose contract then mismatches the actual arguments).
        let mut symbol_override: Option<String> = None;
        let known = self
            .check
            .overload_call_symbol(expr)
            .and_then(|sym| {
                let decl = self
                    .func_index
                    .iter()
                    .filter(|(n, _)| n == &func_name)
                    .map(|(_, d)| d)
                    .find(|d| self.decl_symbol(d) == sym)?;
                symbol_override = Some(sym.to_string());
                Some((func_name.clone(), decl.clone()))
            })
            .or_else(|| {
                self.func_decl(&func_name)
                    .map(|d| (func_name.clone(), d))
                    .or_else(|| self.resolve_receiver_method_decl(call))
                    .or_else(|| self.func_decl_tail_match(&func_name))
            });
        if let Some((callee_name, decl)) = known {
            let (display, planned_args) = self.plan_known_call_args(call, &decl.params)?;
            let result = self.new_tmp();
            let lowered = symbol_override
                .unwrap_or_else(|| self.lowered_func_symbol(&callee_name));
            self.ins(&format!(
                "{result} = call @{lowered}({})",
                display.join(", ")
            ));
            self.emit_lowered_call_arg_cleanups(&planned_args, Some(&callee_name))?;
            // codegen.zig:15392: a call to an async fn with a
            // continuation plan registers the plan's vtable +
            // unknown readiness on the result register.
            self.register_async_call_result(&result, &decl);
            return Ok(result);
        }

        // F-B1 (`__slice_view` intrinsic, LINE-BORROW follow-up):
        // the generated sla_std wrappers (gen_wrap.py E_res form)
        // build a `&[u8]` view from a raw data pointer + length
        // with `__slice_view(ptr, len)`; the checker types it as
        // Borrow(Slice<u8>) (check_call.rs:159). Without this arm
        // the call falls into the EXPAND catch-all below and the
        // host rejects it with UndefinedMacro (`__slice_view` is
        // not a sa_std macro). Must precede the catch-all.
        if func_name == "__slice_view" {
            return self.gen_slice_view_call(call);
        }

        // Extern variadic batch (codegen.zig:15613): a
        // `va_next_*` read lowers to the native `va_arg`
        // instruction with the promoted read type. There is no
        // std macro by that name, so the catch-all's
        // `EXPAND va_next_i32` dies at the host with
        // UndefinedMacro (test_unit_variadic_define). The single
        // argument is the VaList binding; generating it yields
        // the binding's own register.
        if call.associated_target.is_none() {
            let va_prim = match func_name.as_str() {
                "va_next_i32" => Some("i32"),
                "va_next_i64" => Some("i64"),
                "va_next_u32" => Some("u32"),
                "va_next_u64" => Some("u64"),
                "va_next_f64" => Some("f64"),
                "va_next_ptr" => Some("ptr"),
                _ => None,
            };
            if let Some(prim) = va_prim {
                if call.args.len() != 1 {
                    return unsupported(
                        "va_next arity",
                        "codegen.zig:15613 (va_next lowering)",
                    );
                }
                let list_reg = self.gen_expr(&call.args[0])?;
                let reg = self.new_tmp();
                self.ins(&format!("{reg} = va_arg {list_reg}, {prim}"));
                return Ok(reg);
            }
        }

        if sla_lowering::is_stack_alloc_node(expr) {
            let size = self.stack_alloc_size_of(expr);
            let result = self.new_tmp();
            self.ins(&format!("{result} = stack_alloc {size}"));
            return Ok(result);
        }

        // DEF-deep-copy (codegen.zig:22695): the explicit `.clone()`
        // surface for value types -- String/Vec/GrowVec receivers
        // (deep, elements included) and user structs (field by field
        // through the record copy routine). A user-declared `clone`
        // method resolves through the known-callee path above, so
        // reaching here means the compiler-synthesised surface; the
        // bare-name catch-all below would emit `EXPAND clone`, a
        // macro that exists in no std tree (host UndefinedMacro).
        if call.associated_target.is_none()
            && call.func_name == "clone"
            && call.args.len() == 1
        {
            if let Some(recv_ty) = self.expr_type(&call.args[0]) {
                let cloneable = matches!(
                    &recv_ty,
                    Type::UserDefined(u)
                        if u.name == "String"
                            || u.name == "Vec"
                            || u.name == "GrowVec"
                            || crate::place::struct_decl_lookup(self.check, &u.name)
                                .is_some()
                );
                if cloneable {
                    let recv_reg = self.gen_expr(&call.args[0])?;
                    let cloned = match self.gen_deep_clone_value(&recv_reg, &recv_ty)? {
                        Some(reg) => reg,
                        // Plain-copyable closure: the loaded temp is
                        // already an independent copy.
                        None => recv_reg.clone(),
                    };
                    if cloned != recv_reg
                        && call_arg_needs_release(&call.args[0])
                    {
                        self.emit_release(&recv_reg)?;
                    }
                    return Ok(cloned);
                }
            }
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
        // Settle the temp args the expansion only reads (the Zig
        // macro-call site settle): a call-result temp passed to a
        // read-only macro -- `PTR_ADDR(rel_ptr(..))` is the family
        // shape -- otherwise stays Active at function exit
        // (MemoryLeak, LINE-REG3 S2). Macros whose body moves the
        // arg (the FS_READ_BUFFER_FREE class) retire it inside the
        // expansion and are the ported Zig exception.
        for (idx, (arg, reg)) in call.args.iter().zip(arg_regs.iter()).enumerate() {
            if call_arg_needs_release(arg)
                && !sla_lowering::imported_macro_direct_call_consumes_value_arg(&func_name, idx)
            {
                self.emit_release(reg)?;
            }
        }
        Ok(result)
    }

    /// codegen.zig:6680 `stringCollectSource`: the source node
    /// of a `collect::<String>()` terminal — the single argument
    /// of an `iter()` / `into_iter()` call, optionally under a
    /// `copied()` wrapper. Returns None for every other shape.
    fn string_collect_source<'n>(call: &'n sla_ast::CallExpr) -> Option<&'n Node> {
        if call.func_name != "collect" || call.args.len() != 1 || call.generics.len() != 1 {
            return None;
        }
        let is_string = matches!(
            &call.generics[0],
            Type::UserDefined(u) if crate::place::local_name(&u.name) == "String"
        );
        if !is_string {
            return None;
        }
        let iter_call = match &call.args[0] {
            Node::CallExpr(c) => c,
            _ => return None,
        };
        let is_iter = |c: &sla_ast::CallExpr| {
            (c.func_name == "iter" || c.func_name == "into_iter") && c.args.len() == 1
        };
        if is_iter(iter_call) {
            return Some(&iter_call.args[0]);
        }
        if iter_call.func_name == "copied" && iter_call.args.len() == 1 {
            if let Node::CallExpr(inner) = &iter_call.args[0] {
                if is_iter(inner) {
                    return Some(&inner.args[0]);
                }
            }
        }
        None
    }

    /// codegen.zig:6899 `genStringCollect`: build a String by
    /// pushing every byte of an array / Vec<u8> / &[u8] source
    /// into a fresh string buffer. The array arm unrolls; the
    /// Vec and slice arms loop over an index slot.
    pub(crate) fn gen_string_collect(&mut self, source: &Node) -> Result<String> {
        let source_ty = self.expr_type(source).ok_or_else(|| crate::TextError::MissingType {
            context: "collect::<String> source type".to_string(),
        })?;
        let peeled = sla_lowering::peel_borrow_pointer_type(&source_ty).clone();
        let array = match &peeled {
            Type::Array(arr) => Some(arr.clone()),
            _ => None,
        };
        let elem_ty = if let Some(arr) = &array {
            (*arr.elem).clone()
        } else if let Some(e) = crate::place::slice_element_type(&source_ty) {
            e
        } else if let Some((_, e)) = crate::place::vec_like_element_type(&source_ty) {
            e
        } else {
            return unsupported(
                "collect::<String> source shape",
                "codegen.zig:6899 (genStringCollect)",
            );
        };
        if crate::codegen::type_string(&elem_ty) != "u8" {
            return unsupported(
                "collect::<String> element type",
                "codegen.zig:6899 (genStringCollect)",
            );
        }

        let out_reg = self.new_tmp();
        self.ins(&format!("EXPAND STRING_BUF_NEW {out_reg}"));

        if let Some(arr) = &array {
            let elem_size = crate::codegen::type_size(&elem_ty);
            let base_source_reg = self.gen_expr(source)?;
            let mut base_reg = base_source_reg.clone();
            if let Node::Identifier(name) = source {
                if self.global_const_bindings.contains(name) {
                    let addr_reg = self.new_tmp();
                    self.ins(&format!("{addr_reg} = &{base_source_reg}"));
                    base_reg = addr_reg;
                }
            }
            for i in 0..arr.len {
                let off_reg = self.new_tmp();
                self.emit_int_const(&off_reg, (i * elem_size) as i64);
                let item_ptr = self.new_tmp();
                self.ins(&format!("{item_ptr} = ptr_add {base_reg}, {off_reg}"));
                let item_reg = self.new_tmp();
                self.ins(&format!("{item_reg} = load {item_ptr}+0 as u8"));
                self.ins(&format!(
                    "EXPAND STRING_BUF_PUSH_BYTE {out_reg}, {item_reg}"
                ));
                self.emit_release(&off_reg)?;
                self.emit_release(&item_ptr)?;
                self.emit_release(&item_reg)?;
            }
            if call_arg_needs_release(source) {
                self.emit_release(&base_reg)?;
            }
            if base_reg != base_source_reg {
                self.emit_release(&base_reg)?;
            }
            return Ok(out_reg);
        }

        if crate::place::vec_like_element_type(&source_ty).is_some() {
            let vec_reg = self.gen_expr(source)?;
            let len_reg = self.new_tmp();
            self.ins(&format!("{len_reg} = call @sa_vec_len(&{vec_reg})"));
            let data_reg = self.new_tmp();
            self.ins(&format!("{data_reg} = load {vec_reg}+Vec_ptr as ptr"));
            let idx_slot = self.new_tmp();
            self.ins(&format!("{idx_slot} = stack_alloc 8"));
            self.ins(&format!("store {idx_slot}+0, 0 as u64"));
            let head_label = self.new_label("L_VEC_STRING_COLLECT_HEAD");
            let body_label = self.new_label("L_VEC_STRING_COLLECT_BODY");
            let end_label = self.new_label("L_VEC_STRING_COLLECT_END");
            self.ins(&format!("jmp {head_label}"));
            self.wl("");

            self.wl(&format!("{head_label}:"));
            let idx_reg = self.new_tmp();
            self.ins(&format!("{idx_reg} = load {idx_slot}+0 as u64"));
            let more_reg = self.new_tmp();
            self.ins(&format!("{more_reg} = ult {idx_reg}, {len_reg}"));
            self.ins(&format!("br {more_reg} -> {body_label}, {end_label}"));
            self.wl("");

            self.wl(&format!("{body_label}:"));
            self.ins(&format!("!{more_reg}"));
            let item_ptr = self.new_tmp();
            self.ins(&format!("{item_ptr} = ptr_add {data_reg}, {idx_reg}"));
            let item_reg = self.new_tmp();
            self.ins(&format!("{item_reg} = load {item_ptr}+0 as u8"));
            self.ins(&format!(
                "EXPAND STRING_BUF_PUSH_BYTE {out_reg}, {item_reg}"
            ));
            let next_idx = self.new_tmp();
            self.ins(&format!("{next_idx} = add {idx_reg}, 1"));
            self.ins(&format!("store {idx_slot}+0, {next_idx} as u64"));
            self.emit_release(&item_ptr)?;
            self.emit_release(&item_reg)?;
            self.emit_release(&next_idx)?;
            self.emit_release(&idx_reg)?;
            self.ins(&format!("jmp {head_label}"));
            self.wl("");

            self.wl(&format!("{end_label}:"));
            self.ins(&format!("!{more_reg}"));
            self.emit_release(&idx_reg)?;
            self.emit_release(&len_reg)?;
            self.emit_release(&data_reg)?;
            if call_arg_needs_release(source) {
                self.emit_release(&vec_reg)?;
            }
            return Ok(out_reg);
        }

        let slice_reg = self.gen_expr(source)?;
        let base_ptr = self.new_tmp();
        self.ins(&format!("{base_ptr} = load {slice_reg}+Slice_ptr as ptr"));
        let len_reg = self.new_tmp();
        self.ins(&format!("{len_reg} = load {slice_reg}+Slice_len as u64"));
        let idx_slot = self.new_tmp();
        self.ins(&format!("{idx_slot} = stack_alloc 8"));
        self.ins(&format!("store {idx_slot}+0, 0 as u64"));
        let head_label = self.new_label("L_STRING_COLLECT_HEAD");
        let body_label = self.new_label("L_STRING_COLLECT_BODY");
        let end_label = self.new_label("L_STRING_COLLECT_END");
        self.ins(&format!("jmp {head_label}"));
        self.wl("");

        self.wl(&format!("{head_label}:"));
        let idx_reg = self.new_tmp();
        self.ins(&format!("{idx_reg} = load {idx_slot}+0 as u64"));
        let done_reg = self.new_tmp();
        self.ins(&format!("{done_reg} = eq {idx_reg}, {len_reg}"));
        self.ins(&format!("br {done_reg} -> {end_label}, {body_label}"));
        self.wl("");

        self.wl(&format!("{body_label}:"));
        self.ins(&format!("!{done_reg}"));
        let off_reg = self.new_tmp();
        self.ins(&format!("{off_reg} = ptr_add {base_ptr}, {idx_reg}"));
        let item_reg = self.new_tmp();
        self.ins(&format!("{item_reg} = load {off_reg}+0 as u8"));
        self.ins(&format!(
            "EXPAND STRING_BUF_PUSH_BYTE {out_reg}, {item_reg}"
        ));
        let next_idx = self.new_tmp();
        self.ins(&format!("{next_idx} = add {idx_reg}, 1"));
        self.ins(&format!("store {idx_slot}+0, {next_idx} as u64"));
        self.emit_release(&off_reg)?;
        self.emit_release(&item_reg)?;
        self.emit_release(&next_idx)?;
        self.emit_release(&idx_reg)?;
        self.ins(&format!("jmp {head_label}"));
        self.wl("");

        self.wl(&format!("{end_label}:"));
        self.ins(&format!("!{done_reg}"));
        self.emit_release(&idx_reg)?;
        self.emit_release(&base_ptr)?;
        self.emit_release(&len_reg)?;
        if call_arg_needs_release(source) {
            self.emit_release(&slice_reg)?;
        }
        Ok(out_reg)
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
                let len_reg = self.new_tmp();
                self.emit_int_const(&len_reg, len as i64);
                // codegen.zig:15514: a string panic lowers to
                // `EXPAND PANIC_MSG 103, *label, len_reg` — the
                // code is the literal 103 and the message is the
                // dereferenced label. The port emitted an
                // unassigned fresh temp as the code and `&label`
                // as the message; the host rejected the expanded
                // call with CapabilityMismatch
                // (test_unit_option_match, after the `!_` fix).
                self.ins(&format!("EXPAND PANIC_MSG 103, *{label}, {len_reg}"));
                self.emit_release(&len_reg)?;
                return Ok(self.new_tmp());
            }
            _ => {
                let reg = self.gen_expr(arg)?;
                self.ins(&format!("panic({reg})"));
            }
        }
        Ok(self.new_tmp())
    }

    /// Stage 4: the `str_eq` checker builtin (check_builtin.rs
    /// venue :8832). Both arguments are string-like; each is
    /// materialized as a slice register and compared with the
    /// std STR_EQ macro (byte-wise string equality from
    /// sa_std/string.sa). A `String` argument goes through
    /// STRING_BUF_AS_STR, mirroring the format-push path
    /// (format.rs emit_format_push_string_like); any other
    /// string-like argument already evaluates to a slice
    /// register (string literals included, codegen.rs
    /// gen_literal_value).
    ///
    /// R3 (stage-5a resume): this used to emit SLICE_EQ, whose
    /// SLICE_EQ_U64 body compares u64 words at stride 8 — it
    /// over-reads past the end of any byte string whose length
    /// is not a multiple of 8, so `str_eq(&s, "[a]")` failed
    /// while `len(s) == 3` passed. Zig emits STR_EQ here
    /// (codegen.zig:20085-20103).
    fn gen_str_eq_call(&mut self, call: &sla_ast::CallExpr) -> Result<String> {
        let left = self.gen_str_eq_slice(&call.args[0])?;
        let right = self.gen_str_eq_slice(&call.args[1])?;
        let result = self.new_tmp();
        self.ins(&format!("EXPAND STR_EQ {result}, {left}, {right}"));
        self.emit_release(&left)?;
        self.emit_release(&right)?;
        Ok(result)
    }

    fn gen_str_eq_slice(&mut self, arg: &Node) -> Result<String> {
        let ty = self.expr_type(arg);
        if let Some(ty) = &ty {
            if crate::codegen::is_format_string_type(ty) {
                let string_reg = self.gen_expr(arg)?;
                let slice_reg = self.new_tmp();
                self.ins(&format!(
                    "EXPAND STRING_BUF_AS_STR {slice_reg}, {string_reg}"
                ));
                if call_arg_needs_release(arg) {
                    self.emit_release(&string_reg)?;
                }
                return Ok(slice_reg);
            }
        }
        self.gen_expr(arg)
    }

    /// F-B1 (`__slice_view` intrinsic, LINE-BORROW follow-up):
    /// materialize the view the way Zig builds a slice from
    /// (ptr, len) — heap-allocate a {ptr, len} slot and fill it
    /// with SLICE_NEW (codegen.zig `genSliceViewCall`;
    /// sa_std/core/slice.sa). This is the same form the
    /// string-literal path (codegen.rs `gen_literal_value`)
    /// already emits, so no extra macro import is needed:
    /// SLICE_NEW resolves in the census env wherever a string
    /// literal does. The two arguments are planned as plain
    /// value args (Option/Result constructor precedent: no
    /// declared params exist for an intrinsic) and cleaned up
    /// by the shared call-arg machinery.
    ///
    /// F-B7 (LINE-STACKESCAPE-FIX): the cell must be `alloc`,
    /// not `stack_alloc` — the annotated wrapper returns it,
    /// and a stack allocation cannot escape its frame
    /// (StackEscape). The result is deliberately NOT registered
    /// in `stack_alloc_bindings`: it is a heap cell whose
    /// ownership transfers to the caller, and the stack-address
    /// treatment in block.rs / expr.rs / extern_macro.rs must
    /// not apply to it.
    fn gen_slice_view_call(&mut self, call: &sla_ast::CallExpr) -> Result<String> {
        if call.args.len() != 2 {
            return unsupported(
                "__slice_view arity",
                "check_call.rs:159 (__slice_view intrinsic takes ptr, len)",
            );
        }
        let ptr = self.gen_planned_call_arg(&call.args[0], None, false)?;
        let len = self.gen_planned_call_arg(&call.args[1], None, false)?;
        let result = self.new_tmp();
        self.ins(&format!("{result} = alloc Slice_SIZE"));
        self.ins(&format!(
            "EXPAND SLICE_NEW {result}, {}, {}",
            ptr.reg, len.reg
        ));
        let planned_args = vec![ptr, len];
        self.emit_lowered_call_arg_cleanups(&planned_args, None)?;
        Ok(result)
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
        // Statement-position runtime calls (stage 6 async
        // cluster): evaluate for effect and release the result
        // when the expression result needs it.
        if let Some(reg) = self.gen_runtime_call(call)? {
            if self.expr_result_reg_needs_release(expr) {
                self.emit_release(&reg)?;
            }
            return Ok(());
        }
        // Statement-position thread::spawn (thread_spawn.rs):
        // build the slot and release it when the statement's
        // result needs it, as the runtime-call branch above.
        if crate::thread_spawn::is_thread_spawn_call(call) {
            let reg = self.gen_thread_spawn_call(expr, call)?;
            if self.expr_result_reg_needs_release(expr) {
                self.emit_release(&reg)?;
            }
            return Ok(());
        }
        // Statement-position Chan<T> method call (chan.rs):
        // emit for effect and release the result when the
        // expression result needs it, as the runtime-call
        // branch above. destroy/close are void: their result
        // register is an undefined placeholder (the surface
        // Void precedent) and is never released.
        if let Some((kind, payload_ty)) = self.chan_method(call) {
            let reg = self.gen_chan_method_call(call, kind, &payload_ty)?;
            if !matches!(
                kind,
                crate::chan::ChanMethod::Destroy | crate::chan::ChanMethod::Close
            ) && self.expr_result_reg_needs_release(expr)
            {
                self.emit_release(&reg)?;
            }
            return Ok(());
        }
        // Statement-position mem::forget (gen_mem_forget_call):
        // void by Zig isVoidCall, dispatched before the method
        // resolution below, as in the Zig genCallStmt.
        if is_mem_forget_call(call) {
            self.gen_mem_forget_call(call)?;
            return Ok(());
        }
        // Statement-position associated call to a known void
        // function (thread::sleep, stage 6): emit the bare call
        // form, as the plain known-function arm below does.
        if call.associated_target.is_some() {
            let resolved = self
                .resolve_associated_decl(call)
                .or_else(|| self.resolve_receiver_method_decl(call));
            if let Some((key, decl)) = resolved {
                let (display, planned_args) = self.plan_known_call_args(call, &decl.params)?;
                let lowered = self.lowered_func_symbol(&key);
                self.ins(&format!("call @{lowered}({})", display.join(", ")));
                self.emit_lowered_call_arg_cleanups(&planned_args, Some(&key))?;
                return Ok(());
            }
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
            return self.gen_user_macro_call_stmt(expr, call);
        }
        if let Some(sig) = self.check.extern_sig(&func_name) {
            return self.gen_extern_call_stmt(expr, call, &sig);
        }
        // Fn-pointer value call in statement position (void form):
        // same arm as gen_expr_call; a non-void result is created
        // and released when the statement's result needs it, as the
        // runtime-call branch above does.
        if let Some((fp, callee_reg)) = self.fn_ptr_indirect_target(&func_name) {
            let result = self.emit_fn_ptr_indirect_call(call, &fp, &callee_reg)?;
            if let Some(reg) = result {
                if self.expr_result_reg_needs_release(expr) {
                    self.emit_release(&reg)?;
                }
            }
            return Ok(());
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
/// lowering_rules.zig `isMemForgetCall`: the `mem::forget`
/// associated call shape (target `mem`, func `forget`).
pub(crate) fn is_mem_forget_call(call: &sla_ast::CallExpr) -> bool {
    call.associated_target.as_deref() == Some("mem") && call.func_name == "forget"
}

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

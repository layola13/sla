//! Extern-contract calls and imported-macro calls, mirrored
//! from codegen.zig:3342 `genExternPayloadCall`, 13968
//! `genImportedMacroCall` and 13471 `genImportedMacroArg`.
//! Split out of call.rs for the per-file line cap.

use sla_ast::{Node, Param, Primitive, Type};
use sla_lowering::{ExternSigView, ImportedMacroView};

use crate::call::{abi_type_from_spelling, LoweredCallArg};
use crate::codegen::Codegen;
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// Prefix a display register with a capability marker unless
    /// it already carries one (Zig `abiPrefixedArg`).
    fn prefix_cap(reg: &str, cap: char) -> String {
        if reg.starts_with('&') || reg.starts_with('^') {
            reg.to_string()
        } else {
            format!("{cap}{reg}")
        }
    }

    /// Zig `abiRawPrefixedArg`: a raw-capability (`*`-marked)
    /// extern parameter keeps the same register; a borrow/move
    /// annotation on the operand is REPLACED by the raw `*`
    /// annotation the SA contract expects.
    fn raw_prefixed_arg(reg: &str) -> String {
        if let Some(rest) = reg.strip_prefix(['&', '^']) {
            format!("*{rest}")
        } else {
            Self::prefix_cap(reg, '*')
        }
    }

    /// Shared extern argument planning (Zig `genExternPayloadCall`
    /// arg loop): each argument is planned against the contract
    /// parameter (the ABI type becomes the AST param type, as in
    /// `externPtrParamAsAstParam`), then the contract capability
    /// prefix is applied to the display register (`abiCallArgPrefix`:
    /// borrow `&`, move `^`). Cleanups keep the unprefixed planned
    /// registers, as in the Zig cleanup split.
    fn plan_extern_call_args(
        &mut self,
        call: &sla_ast::CallExpr,
        sig: &ExternSigView,
    ) -> Result<(Vec<String>, Vec<LoweredCallArg>)> {
        let mut planned_args: Vec<LoweredCallArg> = Vec::new();
        let mut display: Vec<String> = Vec::new();
        for (i, arg) in call.args.iter().enumerate() {
            let view = sig.params.get(i);
            let param = view.map(|p| Param {
                name: p.name.clone(),
                ty: Box::new(abi_type_from_spelling(&p.abi_ty)),
                is_borrow: p.is_borrow,
                is_move: p.is_move,
                fallible_sugar: false,
                is_var: false,
            });
            let planned = self.gen_planned_call_arg(arg, param.as_ref(), false)?;
            let reg = match view {
                Some(p) if p.is_raw => Self::raw_prefixed_arg(&planned.reg),
                Some(p) if p.is_borrow => Self::prefix_cap(&planned.reg, '&'),
                Some(p) if p.is_move => Self::prefix_cap(&planned.reg, '^'),
                _ => planned.reg.clone(),
            };
            display.push(reg);
            planned_args.push(planned);
        }
        Ok((display, planned_args))
    }

    /// codegen.zig:3342 `genExternPayloadCall`. Before this arm
    /// existed, expression-position extern calls fell into the
    /// EXPAND catch-all below and the host rejected them with
    /// error[UndefinedMacro] (14 corpus files).
    pub(crate) fn gen_extern_call(
        &mut self,
        call: &sla_ast::CallExpr,
        sig: &ExternSigView,
    ) -> Result<String> {
        let (display, planned_args) = self.plan_extern_call_args(call, sig)?;
        let result = self.new_tmp();
        let lowered = self.lowered_func_symbol(&call.func_name);
        self.ins(&format!(
            "{result} = call @{lowered}({})",
            display.join(", ")
        ));
        self.emit_lowered_call_arg_cleanups(&planned_args, Some(&call.func_name))?;
        Ok(result)
    }

    /// Statement position (the DEF-10 shape): a void extern emits
    /// a bare call with the contract prefixes intact; a
    /// value-returning extern goes through the expression path and
    /// its discarded result temp is released like a discarded
    /// surface value. Before this arm existed, statement-position
    /// extern calls hit the gen_call_stmt fallthrough.
    pub(crate) fn gen_extern_call_stmt(
        &mut self,
        expr: &Node,
        call: &sla_ast::CallExpr,
        sig: &ExternSigView,
    ) -> Result<()> {
        let ret = sig
            .ret_abi
            .trim()
            .trim_start_matches(['&', '^', '*'])
            .trim_end_matches('!')
            .trim();
        if ret.is_empty() || ret == "void" {
            let (display, planned_args) = self.plan_extern_call_args(call, sig)?;
            let lowered = self.lowered_func_symbol(&call.func_name);
            self.ins(&format!("call @{lowered}({})", display.join(", ")));
            self.emit_lowered_call_arg_cleanups(&planned_args, Some(&call.func_name))?;
            return Ok(());
        }
        let reg = self.gen_extern_call(call, sig)?;
        if self.expr_result_reg_needs_release(expr) {
            self.emit_release(&reg)?;
        }
        Ok(())
    }

    /// One imported-macro operand (Zig `genImportedMacroArg`,
    /// core subset): address-slot operands take the argument's
    /// address — a stack-slot binding passes the slot itself, any
    /// other identifier its `&name`; borrowed operands take
    /// `&reg`; everything else is the plain value register.
    /// Non-identifier address-slot arguments stay loud.
    fn gen_imported_macro_arg(
        &mut self,
        arg: &Node,
        view: &ImportedMacroView,
        operand_index: usize,
    ) -> Result<String> {
        let bit = 1u64 << operand_index;
        if view.address_slot_arg_mask & bit != 0 {
            return match arg {
                Node::Identifier(name) => {
                    let resolved = self.resolve_binding_name(name);
                    // A pointer-typed binding already holds the
                    // address the macro expects (e.g. PTR_READ_U8's
                    // %ptr_reg, whose body loads through it): pass
                    // it plainly. Prefixing it produced `load &p+0`
                    // and a host UnknownRegister.
                    let arg_ty = self.expr_type(arg);
                    if matches!(
                        arg_ty,
                        Some(
                            Type::Pointer(_)
                                | Type::Borrow(_)
                                | Type::Primitive(Primitive::RawPtr)
                        )
                    ) {
                        return Ok(resolved);
                    }
                    if self.stack_alloc_bindings.contains(&resolved) {
                        return Ok(resolved);
                    }
                    // Pointer-represented composites (Slice,
                    // String, structs, arrays, tuples): the
                    // register already holds the value's address.
                    if matches!(
                        arg_ty,
                        Some(Type::UserDefined(_) | Type::Array(_) | Type::Tuple(_))
                    ) {
                        return Ok(resolved);
                    }
                    // A scalar in a plain register has no address:
                    // materialise a stack slot for it (Zig's
                    // MaterializeStackSlot action).
                    let ty = arg_ty.ok_or_else(|| crate::TextError::MissingType {
                        context: "macro address-slot argument type".to_string(),
                    })?;
                    let slot = self.new_tmp();
                    self.ins(&format!(
                        "{slot} = stack_alloc {}",
                        crate::codegen::type_size(&ty)
                    ));
                    self.ins(&format!(
                        "store {slot}+0, {resolved} as {}",
                        crate::codegen::type_string(&ty)
                    ));
                    Ok(slot)
                }
                // Composite values are pointer-represented in
                // this codegen (a string literal's register is
                // its slice image's stack slot, a Vec/String is
                // its handle), so a non-identifier address-slot
                // operand's generated register already is the
                // address the macro expects — the materialised
                // form of Zig's MaterializeStackSlot for these
                // shapes.
                _ => self.gen_expr(arg),
            };
        }
        // A borrow of a slot-backed or pointer-represented
        // identifier passes the binding register itself (Zig
        // planAddressOf identifier shape — the register already
        // IS the value's address). Macro operands are never
        // released by the caller emission, so the ptr_add temp
        // the generic borrow path materialises would leak at
        // function exit (PTR_BYTE_ADD(&big, 0) in
        // test_smoke1_net).
        if let Node::BorrowExpr(b) = arg {
            if let Node::Identifier(name) = &*b.expr {
                let resolved = self.resolve_binding_name(name);
                let pointer_represented = matches!(
                    self.expr_type(&b.expr),
                    Some(
                        Type::Array(_)
                            | Type::Tuple(_)
                            | Type::UserDefined(_)
                            | Type::Pointer(_)
                            | Type::Borrow(_)
                    )
                );
                if self.stack_alloc_bindings.contains(&resolved) || pointer_represented {
                    return Ok(resolved);
                }
            }
        }
        // Borrowed operands (borrowed_arg_mask) pass their plain
        // register: the mask records that the macro does not
        // consume the value (cleanup bookkeeping in the Zig
        // plan), it does NOT prefix the operand with `&` — doing
        // so substituted `&p` into macro bodies that use the
        // parameter as a load base (`load &p+0`, rejected by the
        // host with UnknownRegister; regression caught against
        // test_unit_ptr_byte_add_read_type_sa).
        self.gen_expr(arg)
    }

    /// Expression position (Zig `genImportedMacroCall`): the
    /// macro's single leading output is a fresh result register
    /// and the call's arguments fill the remaining operands.
    /// Multi-output macros are statement-only and stay loud here.
    pub(crate) fn gen_imported_macro_call_expr(
        &mut self,
        call: &sla_ast::CallExpr,
        view: &ImportedMacroView,
    ) -> Result<String> {
        if view.leading_outputs != 1 || call.args.len() + view.leading_outputs != view.arity {
            return unsupported(
                "imported macro call (multi-output / arity shape)",
                "codegen.zig:13968 (genImportedMacroCall)",
            );
        }
        let result = self.new_tmp();
        let mut operands = vec![result.clone()];
        for (i, arg) in call.args.iter().enumerate() {
            operands.push(self.gen_imported_macro_arg(arg, view, view.leading_outputs + i)?);
        }
        self.ins(&format!(
            "EXPAND {} {}",
            call.func_name,
            operands.join(", ")
        ));
        Ok(result)
    }

    /// Statement position: the leading outputs are the call's own
    /// first arguments (bindings the macro writes through); the
    /// remaining arguments are ordinary macro operands.
    pub(crate) fn gen_imported_macro_call_stmt(
        &mut self,
        call: &sla_ast::CallExpr,
        view: &ImportedMacroView,
    ) -> Result<()> {
        if call.args.len() != view.arity {
            return unsupported(
                "imported macro statement call (arity shape)",
                "codegen.zig:13968 (genImportedMacroCall)",
            );
        }
        let mut operands: Vec<String> = Vec::new();
        for (i, arg) in call.args.iter().enumerate() {
            if i < view.leading_outputs {
                match arg {
                    Node::Identifier(name) => operands.push(self.resolve_binding_name(name)),
                    _ => {
                        return unsupported(
                            "imported macro statement output argument",
                            "codegen.zig:13968 (genImportedMacroCall)",
                        )
                    }
                }
            } else {
                operands.push(self.gen_imported_macro_arg(arg, view, i)?);
            }
        }
        self.ins(&format!(
            "EXPAND {} {}",
            call.func_name,
            operands.join(", ")
        ));
        Ok(())
    }

}

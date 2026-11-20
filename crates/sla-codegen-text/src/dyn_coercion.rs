//! Dyn trait-object coercion and dispatch emission, mirrored
//! from codegen.zig (`genDynBorrowCoercionArg` :1089 and the
//! dyn-call arm of the call cascade :23158) and
//! lowering_rules.zig (`planDynDispatchReceiver`,
//! `dynMethodSlot` :6116).
//!
//! A `dyn Trait` value is a fat cell (`Dyn_SIZE` bytes from
//! sa_std/core/trait_object.sa): the data pointer at `+Dyn_data`
//! and the vtable pointer at `+Dyn_vtable`. A borrow of a
//! concrete value coerces by building a fresh fat cell pointing
//! at the value and at the impl's `@const VT_{Type}_{Trait}`
//! (vtable.rs). A method call on a dyn receiver loads the
//! function pointer from the vtable slot and calls it
//! indirectly with `&data` as the receiver argument.

use sla_ast::{CallExpr, Node, Type};
use sla_lowering::call_arg_needs_release;

use crate::codegen::Codegen;
use crate::{unsupported, Result};

/// lowering_rules.zig `DynDispatchReceiverPlan`, reduced to the
/// one distinction emission needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DynReceiverPlan {
    /// The receiver register already addresses the fat cell.
    Direct,
    /// The receiver is an `Rc<dyn Trait>` handle: `RC_GET`
    /// extracts the fat cell first.
    RcGet,
    /// The receiver is a `Box<dyn Trait>` handle: the fat cell
    /// is the box's payload word (`load +Box_value`), the same
    /// load the Box deref and drop paths use.
    BoxGet,
}

impl<'c> Codegen<'c> {
    /// lowering_rules.zig `planDynDispatchReceiver` plus the
    /// trait name each arm dispatches on: `Rc<dyn T>` unwraps
    /// through `RC_GET`; a direct dyn value dispatches in place;
    /// a `Box<dyn T>` handle holds the fat cell as its payload
    /// word, so the dispatch loads it first (`BoxGet`) — treating
    /// the handle itself as the fat cell read Dyn_vtable out of
    /// the box cell's second word (signal 11, F-DYN L3).
    pub(crate) fn dyn_receiver_trait(
        &self,
        recv_ty: &Type,
    ) -> Option<(String, DynReceiverPlan)> {
        if let Some(inner) = sla_lowering::rc_inner_type(recv_ty) {
            if let Some(trait_name) = sla_lowering::dyn_trait_name(inner) {
                return Some((trait_name.to_string(), DynReceiverPlan::RcGet));
            }
        }
        if let Some(trait_name) = sla_lowering::dyn_trait_name(recv_ty) {
            return Some((trait_name.to_string(), DynReceiverPlan::Direct));
        }
        if let Some(inner) = sla_lowering::box_inner_type(recv_ty) {
            if let Some(trait_name) = sla_lowering::dyn_trait_name(inner) {
                return Some((trait_name.to_string(), DynReceiverPlan::BoxGet));
            }
        }
        None
    }

    /// Whether this call is a dyn dispatch: a receiver call (no
    /// associated target) whose receiver type is a dyn trait
    /// object. Mirrors the Zig trigger `tc.dyn_call_traits.get(
    /// expr)`: the Rust checker records no such table (D-C21),
    /// so the emitter recomputes the same fact from the
    /// receiver's checked type — a `__dyn_<Trait>` receiver can
    /// never resolve through the inherent/trait assoc index.
    pub(crate) fn dyn_call_trait(&self, call: &CallExpr) -> Option<String> {
        if call.associated_target.is_some() || call.args.is_empty() {
            return None;
        }
        let recv_ty = self.expr_type(&call.args[0])?;
        let (trait_name, _) = self.dyn_receiver_trait(&recv_ty)?;
        Some(trait_name)
    }

    /// codegen.zig:1089 `genDynBorrowCoercionArg`: materialize
    /// the fat cell for a borrow argument whose parameter is a
    /// dyn trait object; returns the fat cell's register.
    pub(crate) fn gen_dyn_borrow_coercion_arg(
        &mut self,
        arg: &Node,
        trait_name: &str,
    ) -> Result<String> {
        let source_expr = match arg {
            Node::BorrowExpr(b) => &*b.expr,
            _ => arg,
        };
        let source_ty = self
            .expr_type(source_expr)
            .ok_or_else(|| crate::TextError::MissingType {
                context: "dyn borrow coercion source type".to_string(),
            })?;
        let source_reg = self.gen_expr(source_expr)?;
        let fat_reg = self.new_tmp();
        self.ins(&format!("{fat_reg} = alloc Dyn_SIZE"));
        if sla_lowering::dyn_trait_name(&source_ty).is_some() {
            // Dyn-to-dyn (upcast): copy the fat cell's two words.
            let data_reg = self.new_tmp();
            let vtable_reg = self.new_tmp();
            self.ins(&format!("{data_reg} = load {source_reg}+Dyn_data as ptr"));
            self.ins(&format!(
                "{vtable_reg} = load {source_reg}+Dyn_vtable as ptr"
            ));
            self.ins(&format!("store {fat_reg}+Dyn_data, {data_reg} as ptr"));
            self.ins(&format!(
                "store {fat_reg}+Dyn_vtable, {vtable_reg} as ptr"
            ));
        } else {
            let type_name = match sla_lowering::concrete_type_name(&source_ty) {
                Some(n) => n.to_string(),
                None => {
                    return unsupported(
                        "dyn borrow coercion of a non-user-defined source",
                        "codegen.zig:1089 (genDynBorrowCoercionArg)",
                    )
                }
            };
            let vt_name = Self::trait_vtable_name(trait_name, &type_name);
            self.ins(&format!("store {fat_reg}+Dyn_data, {source_reg} as ptr"));
            self.ins(&format!(
                "store {fat_reg}+Dyn_vtable, &{vt_name} as ptr"
            ));
            if call_arg_needs_release(source_expr) {
                self.emit_release(&source_reg)?;
            }
        }
        Ok(fat_reg)
    }

    /// codegen.zig:23158 dyn-call arm: dispatch a method call
    /// through the receiver's vtable.
    pub(crate) fn gen_dyn_dispatch_call(
        &mut self,
        expr: &Node,
        call: &CallExpr,
        trait_name: &str,
    ) -> Result<String> {
        let slot = match self.dyn_method_slot(trait_name, &call.func_name) {
            Some(s) => s,
            None => {
                return Err(crate::TextError::Unsupported {
                    feature: format!(
                        "dyn method `{}` has no vtable slot in trait `{trait_name}`",
                        call.func_name
                    ),
                    zig_ref: "codegen.zig:23158 (dyn dispatch)",
                })
            }
        };
        if call.args.is_empty() {
            return unsupported(
                "dyn dispatch without a receiver argument",
                "codegen.zig:23158 (dyn dispatch)",
            );
        }
        let recv_ty = self
            .expr_type(&call.args[0])
            .ok_or_else(|| crate::TextError::MissingType {
                context: "dyn dispatch receiver type".to_string(),
            })?;
        let recv_reg = self.gen_expr(&call.args[0])?;
        let (_, plan) = self
            .dyn_receiver_trait(&recv_ty)
            .ok_or_else(|| crate::TextError::MissingType {
                context: "dyn dispatch receiver plan".to_string(),
            })?;
        let dyn_reg = match plan {
            DynReceiverPlan::RcGet => {
                let reg = self.new_tmp();
                self.ins(&format!("EXPAND RC_GET {reg}, {recv_reg}"));
                reg
            }
            DynReceiverPlan::BoxGet => {
                let reg = self.new_tmp();
                self.ins(&format!("{reg} = load {recv_reg}+Box_value as ptr"));
                reg
            }
            DynReceiverPlan::Direct => recv_reg.clone(),
        };

        let mut arg_regs: Vec<String> = Vec::new();
        for arg in &call.args[1..] {
            arg_regs.push(self.gen_expr(arg)?);
        }

        let data_reg = self.new_tmp();
        let vtable_reg = self.new_tmp();
        let fn_reg = self.new_tmp();
        self.ins(&format!("{data_reg} = load {dyn_reg}+Dyn_data as ptr"));
        self.ins(&format!(
            "{vtable_reg} = load {dyn_reg}+Dyn_vtable as ptr"
        ));
        self.ins(&format!("{fn_reg} = load {vtable_reg}+{slot} as ptr"));

        let ret_ty = self.require_expr_type(expr, "dyn dispatch result")?;
        let returns_void = sla_lowering::is_void_type(&ret_ty);
        let mut operands = format!("&{data_reg}");
        for reg in &arg_regs {
            operands.push_str(&format!(", {reg}"));
        }
        let out_reg = if returns_void {
            self.ins(&format!("call_indirect {fn_reg}({operands})"));
            None
        } else {
            let out = self.new_tmp();
            self.ins(&format!("{out} = call_indirect {fn_reg}({operands})"));
            Some(out)
        };

        self.emit_release(&fn_reg)?;
        self.emit_release(&vtable_reg)?;
        self.emit_release(&data_reg)?;
        for (arg, reg) in call.args[1..].iter().zip(arg_regs.iter()) {
            if call_arg_needs_release(arg) {
                self.emit_release(reg)?;
            }
        }
        if dyn_reg != recv_reg {
            self.emit_release(&dyn_reg)?;
        }
        if call_arg_needs_release(&call.args[0]) {
            self.emit_release(&recv_reg)?;
        }
        // A void dyn call in expression position yields the same
        // fresh placeholder the plain void-call arm returns.
        Ok(out_reg.unwrap_or_else(|| self.new_tmp()))
    }
}

//! Expression emission, mirrored from codegen.zig's `genExpr`
//! (12824-). The slice paths (literal, identifier, scalar binary)
//! are ported in full; every other expression family fails loudly
//! with its Zig reference until its lane lands.

use sla_ast::{Literal, Node, Type};
use sla_lowering::{
    is_option_none_name, plan_scalar_binary_op, scalar_binary_op_name,
};

use crate::codegen::Codegen;
use crate::{unsupported, Result, TextError};

impl<'c> Codegen<'c> {
    /// codegen.zig:12824 `genExpr`.
    pub(crate) fn gen_expr(&mut self, expr: &Node) -> Result<String> {
        match expr {
            Node::Literal(lit) => self.gen_literal(lit),
            Node::Identifier(name) => self.gen_identifier_expr(expr, name),
            Node::BinaryExpr(_) => self.gen_binary_expr(expr),
            Node::IfExpr(_) => self.gen_if_expr(expr),
            Node::CallExpr(_) => self.gen_expr_call(expr),
            Node::MoveExpr(inner) => {
                // codegen.zig:13063 move_expr (verified verbatim):
                // evaluate the inner expression and prefix the
                // register with `^`.
                let inner_reg = self.gen_expr(&inner.expr)?;
                Ok(format!("^{inner_reg}"))
            }
            Node::GenericFuncRef(g) => {
                // A bare generic function reference lowers to its
                // vtable address when the checker typed it as fn_ptr;
                // anything else is outside the slice.
                if let Some(ty) = self.expr_type(expr) {
                    if matches!(ty, Type::FnPtr(_)) {
                        let vt = self.fn_ptr_vtable_name(&g.func_name);
                        let reg = self.new_tmp();
                        self.ins(&format!("{reg} = &{vt}"));
                        return Ok(reg);
                    }
                }
                unsupported(
                    "generic function reference (non-fn-ptr)",
                    "codegen.zig:12840 (generic_func_ref)",
                )
            }
            Node::DerefExpr(d) => {
                // codegen.zig:13068 deref_expr, plain load path
                // (verified verbatim): a move inner sheds its `^`;
                // Rc/Arc EXPAND paths are deferred.
                if let Some(Type::UserDefined(ud)) = self.expr_type(&d.expr) {
                    if ud.name == "Rc" || ud.name == "Arc" {
                        return unsupported(
                            "deref of Rc/Arc (RC_GET/ARC_GET family)",
                            "codegen.zig:13075 (rc/arc deref paths)",
                        );
                    }
                }
                let generated = self.gen_expr(&d.expr)?;
                let inner: String = if matches!(&*d.expr, Node::MoveExpr(_)) {
                    generated.strip_prefix('^').unwrap_or(&generated).to_string()
                } else {
                    generated
                };
                let deref_ty = self.require_expr_type(expr, "deref result type")?;
                let reg = self.new_tmp();
                self.ins(&format!(
                    "{reg} = load {inner}+0 as {}",
                    crate::codegen::type_string(&deref_ty)
                ));
                if sla_lowering::expr_result_needs_release(&d.expr) {
                    self.emit_release(&inner)?;
                }
                Ok(reg)
            }
            Node::UnsafeExpr(ue) => self.gen_block_tail_value_into_fresh(&ue.body),
            other => Err(TextError::Unsupported {
                feature: format!("expression kind {:?}", expr_kind_name(other)),
                zig_ref: "codegen.zig:12824 (genExpr)",
            }),
        }
    }

    fn gen_literal(&mut self, lit: &Literal) -> Result<String> {
        self.gen_literal_value(lit)
    }

    /// codegen.zig:12830 identifier expression: alias loads, global
    /// scalar constants, fn-ptr vtable addresses, plain names.
    fn gen_identifier_expr(&mut self, expr: &Node, name: &str) -> Result<String> {
        if is_option_none_name(name) {
            let result = self.new_tmp();
            self.ins(&format!("EXPAND OPTION_NEW_NONE {result}"));
            return Ok(result);
        }
        let resolved = self.resolve_binding_name(name);
        let expr_ty = self.expr_type(expr);

        // Addressable / assigned-slot bindings materialize via load.
        let needs_load = self.addressable_bindings.contains(name)
            && (self.stack_alloc_bindings.contains(&resolved)
                || matches!(expr_ty, Some(Type::Primitive(_))))
            || self.assigned_value_slots.contains(&resolved);
        if needs_load {
            let result = self.new_tmp();
            let ty = self.require_expr_type(expr, "identifier load")?;
            self.ins(&format!(
                "{result} = load {resolved}+0 as {}",
                crate::codegen::type_string(&ty)
            ));
            return Ok(result);
        }

        // Global scalar constants inline their literal value.
        if let Some(lit) = self.global_scalar_const(&resolved) {
            return self.gen_literal_value(&lit);
        }
        if self.global_const_bindings.contains(&resolved) {
            return Ok(resolved);
        }
        if self.assigned_value_slots.contains(&resolved) {
            let result = self.new_tmp();
            let ty = self.require_expr_type(expr, "identifier load")?;
            self.ins(&format!(
                "{result} = load {resolved}+0 as {}",
                crate::codegen::type_string(&ty)
            ));
            return Ok(result);
        }

        // Function name in fn-ptr position: vtable address.
        if self.func_decl(name).is_some() || self.check.is_extern_func(name) {
            if let Some(Type::FnPtr(_)) = expr_ty {
                let vt = self.fn_ptr_vtable_name(name);
                let result = self.new_tmp();
                self.ins(&format!("{result} = &{vt}"));
                return Ok(result);
            }
        }

        // Addressable primitive local: load from its slot.
        if self.addressable_bindings.contains(name) {
            let binding_ty = expr_ty.clone().or_else(|| self.local_binding_type_for_name(name));
            if matches!(binding_ty, Some(Type::Primitive(_))) {
                let ty = binding_ty.unwrap();
                let result = self.new_tmp();
                self.ins(&format!(
                    "{result} = load {resolved}+0 as {}",
                    crate::codegen::type_string(&ty)
                ));
                return Ok(result);
            }
        }

        Ok(resolved)
    }

    /// codegen.zig:12918 binary expression, scalar path. Resolved
    /// static calls and user-defined (struct) operator families are
    /// deferred and fail loudly.
    fn gen_binary_expr(&mut self, expr: &Node) -> Result<String> {
        let bin = match expr {
            Node::BinaryExpr(b) => b,
            _ => unreachable!(),
        };
        if self.check.resolved_call_symbol(expr).is_some() {
            return unsupported(
                "binary expr via resolved static call plan",
                "lowering_rules.zig:5819 (resolved static call plans)",
            );
        }
        let left_ty = self.expr_type(&bin.left);
        let right_ty = self.expr_type(&bin.right);
        if matches!(left_ty, Some(Type::UserDefined(_)))
            || matches!(right_ty, Some(Type::UserDefined(_)))
        {
            return unsupported(
                "binary expr over user-defined operands (struct op families)",
                "codegen.zig:12930 (struct eq/ord/arith emitters)",
            );
        }
        let l = self.gen_expr(&bin.left)?;
        let r = self.gen_expr(&bin.right)?;
        let result = self.new_tmp();
        let left_ty_or_infer = left_ty.clone().unwrap_or(Type::Infer);
        let right_ty_or_infer = right_ty.clone().unwrap_or(Type::Infer);
        let op_name = plan_scalar_binary_op(bin.op, &left_ty_or_infer, &right_ty_or_infer)
            .map(scalar_binary_op_name);
        let op_name = match op_name {
            Some(name) => name,
            None => {
                return Err(TextError::Unsupported {
                    feature: format!("scalar binary op {:?}", bin.op),
                    zig_ref: "lowering_rules.zig:5562 (planScalarBinaryOp returned null)",
                })
            }
        };
        self.ins(&format!("{result} = {op_name} {l}, {r}"));
        if self.expr_result_reg_needs_release(&bin.left) {
            self.emit_release(&l)?;
        }
        if self.expr_result_reg_needs_release(&bin.right) {
            self.emit_release(&r)?;
        }
        Ok(result)
    }

    /// Helper for the unsafe-block expression: tail value into a
    /// fresh register via genBlockTailValueInto.
    fn gen_block_tail_value_into_fresh(&mut self, block: &[Node]) -> Result<String> {
        let result = self.new_tmp();
        self.gen_block_tail_value_into(block, &result)?;
        Ok(result)
    }
}

fn expr_kind_name(node: &Node) -> &'static str {
    match node {
        Node::Literal(_) => "literal",
        Node::Identifier(_) => "identifier",
        Node::GenericFuncRef(_) => "generic_func_ref",
        Node::IfExpr(_) => "if_expr",
        Node::SwitchExpr(_) => "switch_expr",
        Node::MatchExpr(_) => "match_expr",
        Node::UnsafeExpr(_) => "unsafe_expr",
        Node::AwaitExpr(_) => "await_expr",
        Node::InlineAsmExpr(_) => "inline_asm_expr",
        Node::BinaryExpr(_) => "binary_expr",
        Node::CallExpr(_) => "call_expr",
        Node::ClosureLiteral(_) => "closure_literal",
        Node::BorrowExpr(_) => "borrow_expr",
        Node::MoveExpr(_) => "move_expr",
        Node::DerefExpr(_) => "deref_expr",
        Node::CastExpr(_) => "cast_expr",
        Node::FieldExpr(_) => "field_expr",
        Node::StructLiteral(_) => "struct_literal",
        Node::EnumLiteral(_) => "enum_literal",
        Node::TupleLiteral(_) => "tuple_literal",
        Node::ArrayLiteral(_) => "array_literal",
        Node::RepeatArrayLiteral(_) => "repeat_array_literal",
        Node::IndexExpr(_) => "index_expr",
        Node::SliceExpr(_) => "slice_expr",
        Node::TryExpr(_) => "try_expr",
        _ => "other",
    }
}

//! Expression emission, mirrored from codegen.zig's `genExpr`
//! (12824-). The slice paths (literal, identifier, scalar binary)
//! are ported in full; every other expression family fails loudly
//! with its Zig reference until its lane lands.

use sla_ast::{Literal, Node, Primitive, Type};
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
            Node::IndexExpr(idx) => {
                // Vec/GrowVec scalar-element reads
                // (codegen.zig `genVecIndexRead`): bounds check
                // with panic(32), then ptr arithmetic + load.
                // Other target shapes (arrays, slices, non-scalar
                // elements) stay on the loud path.
                let target_ty = self.expr_type(&idx.target).ok_or_else(|| {
                    TextError::MissingType {
                        context: "index target type".to_string(),
                    }
                })?;
                let (tname, elem) = match &target_ty {
                    Type::UserDefined(u) if u.name == "Vec" || u.name == "GrowVec" => {
                        (u.name.clone(), u.generics.first().cloned())
                    }
                    _ => {
                        return unsupported(
                            "index read on non-Vec target",
                            "codegen.zig:16020 (genVecIndexRead family)",
                        )
                    }
                };
                let elem = elem.ok_or_else(|| TextError::MissingType {
                    context: "vec element type".to_string(),
                })?;
                if !matches!(elem, Type::Primitive(_)) {
                    return unsupported(
                        "index read of non-scalar Vec element",
                        "codegen.zig:16020 (genVecIndexRead load variants)",
                    );
                }
                let (len_def, ptr_def) = if tname == "GrowVec" {
                    ("GrowVec_len", "GrowVec_ptr")
                } else {
                    ("Vec_len", "Vec_ptr")
                };
                let vec_reg = self.gen_expr(&idx.target)?;
                let vec_needs_release = self.expr_result_reg_needs_release(&idx.target);
                let index_reg = self.gen_expr(&idx.index)?;
                let len_reg = self.new_tmp();
                let in_bounds_reg = self.new_tmp();
                let hit_label = self.new_label("L_VEC_INDEX_OK");
                let miss_label = self.new_label("L_VEC_INDEX_OOB");
                self.ins(&format!("{len_reg} = load {vec_reg}+{len_def} as u64"));
                self.ins(&format!("{in_bounds_reg} = ult {index_reg}, {len_reg}"));
                self.ins(&format!(
                    "br {in_bounds_reg} -> {hit_label}, {miss_label}"
                ));
                self.ins(&format!("{miss_label}:"));
                self.ins("panic(32)");
                self.ins(&format!("{hit_label}:"));
                let data_reg = self.new_tmp();
                let offset_reg = self.new_tmp();
                let ptr_reg = self.new_tmp();
                self.ins(&format!("!{in_bounds_reg}"));
                self.ins(&format!("{data_reg} = load {vec_reg}+{ptr_def} as ptr"));
                self.ins(&format!(
                    "{offset_reg} = mul {index_reg}, {}",
                    crate::codegen::type_size(&elem).max(8)
                ));
                self.ins(&format!("{ptr_reg} = ptr_add {data_reg}, {offset_reg}"));
                let reg = self.new_tmp();
                self.ins(&format!(
                    "{reg} = load {ptr_reg}+0 as {}",
                    crate::codegen::type_string(&elem)
                ));
                self.emit_release(&ptr_reg)?;
                self.emit_release(&offset_reg)?;
                self.emit_release(&data_reg)?;
                self.emit_release(&len_reg)?;
                if sla_lowering::call_arg_needs_release(&idx.index) {
                    self.emit_release(&index_reg)?;
                }
                if vec_needs_release {
                    self.emit_release(&vec_reg)?;
                }
                Ok(reg)
            }
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
            Node::CastExpr(cast) => {
                // codegen.zig:17018 cast_expr: fn-ptr vtable load,
                // pointer-carrier identity, the non-consuming copy
                // guards for identifier / chained-cast sources,
                // then `{reg} = {src} as {ty}`.
                let inner = self.gen_expr(&cast.expr)?;
                let src_ty = self.require_expr_type(&cast.expr, "cast source type")?;
                if matches!(src_ty, Type::FnPtr(_)) {
                    let loaded = self.new_tmp();
                    self.ins(&format!(
                        "{loaded} = load {inner}+0 as {}",
                        crate::codegen::type_string(&cast.ty)
                    ));
                    if self.call_arg_result_temp_needs_release(&cast.expr, &inner) {
                        self.emit_release(&inner)?;
                    }
                    if matches!(&*cast.ty, Type::Primitive(Primitive::RawPtr)) {
                        let reg = self.new_tmp();
                        self.ins(&format!("{reg} = {loaded} as ptr"));
                        self.emit_release(&loaded)?;
                        return Ok(reg);
                    }
                    return Ok(loaded);
                }
                let carrier = |t: &Type| {
                    matches!(
                        t,
                        Type::Pointer(_)
                            | Type::Borrow(_)
                            | Type::Primitive(Primitive::RawPtr)
                    )
                };
                if carrier(&src_ty) && carrier(&cast.ty) {
                    return Ok(inner);
                }
                let mut src_reg = inner.clone();
                if let Node::Identifier(name) = &*cast.expr {
                    if matches!(src_ty, Type::Primitive(_)) {
                        let resolved = self.resolve_binding_name(name);
                        if inner == resolved || inner == *name {
                            let copied = self.new_tmp();
                            self.emit_primitive_copy(&copied, &inner, &src_ty)?;
                            src_reg = copied;
                        }
                    } else if (matches!(src_ty, Type::Pointer(_) | Type::Borrow(_))
                        && sla_lowering::is_any_integer_type(&cast.ty))
                    {
                        let copied = self.new_tmp();
                        self.ins(&format!("{copied} = ptr_add {inner}, 0"));
                        src_reg = copied;
                    }
                } else if matches!(&*cast.expr, Node::CastExpr(_))
                    && matches!(src_ty, Type::Primitive(_))
                {
                    let copied = self.new_tmp();
                    self.emit_primitive_copy(&copied, &inner, &src_ty)?;
                    src_reg = copied;
                }
                let reg = self.new_tmp();
                self.ins(&format!(
                    "{reg} = {src_reg} as {}",
                    crate::codegen::type_string(&cast.ty)
                ));
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

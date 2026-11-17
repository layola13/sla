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
            Node::MatchExpr(_) => self.gen_match_expr(expr),
            Node::CallExpr(_) => self.gen_expr_call(expr),
            Node::IndexExpr(idx) => {
                // Vec/GrowVec scalar-element reads
                // (codegen.zig `genVecIndexRead`): bounds check
                // with panic(32), then ptr arithmetic + load.
                // Plain arrays and slices go through the
                // genIndexAddress fallthrough (ptr arithmetic +
                // slot load, no bounds check in the Zig arm).
                let target_ty = self.expr_type(&idx.target).ok_or_else(|| {
                    TextError::MissingType {
                        context: "index target type".to_string(),
                    }
                })?;
                let peeled = sla_lowering::peel_borrow_pointer_type(&target_ty).clone();
                if let Type::UserDefined(u) = &peeled {
                    if (u.name == "HashMap" || u.name == "BTreeMap")
                        && u.generics.len() >= 2
                    {
                        let key_ty = u.generics[0].clone();
                        let value_ty = u.generics[1].clone();
                        return self.gen_map_index_read(
                            idx,
                            &key_ty,
                            &value_ty,
                            u.name == "BTreeMap",
                        );
                    }
                }
                if let Type::Array(arr) = &peeled {
                    let elem = (*arr.elem).clone();
                    return self.gen_array_index_read(expr, idx, &elem);
                }
                if let Type::UserDefined(u) = &peeled {
                    if u.name == "Slice" {
                        if let Some(elem) = u.generics.first().cloned() {
                            return self.gen_slice_index_read(idx, &elem);
                        }
                    }
                }
                let (tname, elem) = match &peeled {
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
                // Load variants (Zig genLoadSlotValue): scalar
                // elements load as their type; pointer-backed
                // elements (structs, nested collections, arrays)
                // load the stored word; an inline-POD element
                // lives in the slot itself, whose address is the
                // value. The load happens below at the slot
                // pointer, keyed on `elem`.
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
                // Vec slots hold the element word: scalars load
                // as their type, aggregates load their record
                // pointer. (Inline storage is the ARRAY
                // convention; a Vec slot never holds a struct
                // inline — VEC_PUSH stores the record pointer,
                // the CLASSB defect shape.)
                let reg = {
                    let reg = self.new_tmp();
                    let load_ty = if matches!(elem, Type::Primitive(_)) {
                        crate::codegen::type_string(&elem)
                    } else {
                        "ptr"
                    };
                    self.ins(&format!("{reg} = load {ptr_reg}+0 as {load_ty}"));
                    reg
                };
                self.emit_release(&ptr_reg)?;
                self.emit_release(&offset_reg)?;
                self.emit_release(&data_reg)?;
                self.emit_release(&len_reg)?;
                if self.expr_result_reg_needs_release(&idx.index) {
                    self.emit_release(&index_reg)?;
                }
                if vec_needs_release {
                    self.emit_release(&vec_reg)?;
                }
                Ok(reg)
            }
            Node::BorrowExpr(_) => self.gen_borrow_expr(expr),
            Node::FieldExpr(_) => self.gen_field_expr(expr),
            Node::StructLiteral(lit) => {
                let reg = self.new_tmp();
                self.gen_struct_literal_into(&reg, lit)?;
                Ok(reg)
            }
            Node::EnumLiteral(lit) => {
                let reg = self.new_tmp();
                self.gen_enum_literal_into(&reg, lit)?;
                Ok(reg)
            }
            Node::TupleLiteral(lit) => {
                let reg = self.new_tmp();
                self.gen_tuple_literal_into(&reg, lit)?;
                Ok(reg)
            }
            Node::ArrayLiteral(lit) => {
                let reg = self.new_tmp();
                self.gen_array_literal_into(&reg, expr, lit)?;
                Ok(reg)
            }
            Node::RepeatArrayLiteral(lit) => {
                let reg = self.new_tmp();
                self.gen_repeat_array_literal_into(&reg, expr, lit)?;
                Ok(reg)
            }
            Node::MoveExpr(inner) => {
                // codegen.zig:13063 move_expr (verified verbatim):
                // evaluate the inner expression and prefix the
                // register with `^`.
                let inner_reg = self.gen_expr(&inner.expr)?;
                Ok(format!("^{inner_reg}"))
            }
            Node::AwaitExpr(_) => {
                // codegen.zig:13253 await_expr (stage 6 async
                // cluster; async_runtime_codegen.rs).
                self.gen_await_expr(expr)
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
                // codegen.zig:13068 deref_expr (verified verbatim):
                // generate the inner expression first (a move inner
                // sheds its `^`), then the Rc/Arc EXPAND paths
                // (codegen.zig:13075-13086, keyed on
                // lowering_rules rcInnerType/arcInnerType — the
                // borrow/pointer peel, then `Name<T>` with exactly
                // one generic), then the plain load path below.
                let generated = self.gen_expr(&d.expr)?;
                let inner: String = if matches!(&*d.expr, Node::MoveExpr(_)) {
                    generated.strip_prefix('^').unwrap_or(&generated).to_string()
                } else {
                    generated
                };
                if let Some(inner_ty) = self.expr_type(&d.expr) {
                    let smart_macro = match sla_lowering::peel_borrow_pointer_type(&inner_ty) {
                        Type::UserDefined(u) if u.generics.len() == 1 => match u.name.as_str() {
                            "Rc" => Some("RC_GET"),
                            "Arc" => Some("ARC_GET"),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(macro_name) = smart_macro {
                        let reg = self.new_tmp();
                        self.ins(&format!("EXPAND {macro_name} {reg}, {inner}"));
                        if sla_lowering::expr_result_needs_release(&d.expr) {
                            self.emit_release(&inner)?;
                        }
                        return Ok(reg);
                    }
                }
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
            // F-G2: a supervised spawn in expression position
            // emits the spawn sequence (go_spawn.rs) and yields
            // the handle. F-3: a plain spawn in expression
            // position emits the sa_task_spawn sequence the
            // same way.
            Node::GoStmt(go) if go.supervised => self.gen_supervised_go_expr(go),
            Node::GoStmt(go) => self.gen_plain_go_expr(go),
            other => Err(TextError::Unsupported {
                feature: format!("expression kind {:?}", expr_kind_name(other)),
                zig_ref: "codegen.zig:12824 (genExpr)",
            }),
        }
    }

    /// Map index read (codegen.zig genExpr `.index_expr` map
    /// arms): HashMap probes by content for String keys
    /// (MAP_GET_STR) and by identity otherwise (MAP_GET); a miss
    /// is panic(404). BTreeMap uses BTREE_MAP_TRY_GET with its
    /// (found, value) output pair. The value cell's word loads
    /// as the value type.
    fn gen_map_index_read(
        &mut self,
        idx: &sla_ast::IndexExpr,
        key_ty: &Type,
        value_ty: &Type,
        is_btree: bool,
    ) -> Result<String> {
        let map_reg = self.gen_expr(&idx.target)?;
        // Zig genHashMapKeyReg (codegen.zig:19242): the key is
        // materialised against the receiver's key type — an
        // owned String handle for a literal in a String-keyed
        // map, a cached slice slot for a literal anywhere else.
        let lit_key = self.gen_hashmap_literal_key_reg(
            &idx.index,
            if is_btree { None } else { Some(key_ty) },
        )?;
        let (key_reg, key_owned_literal) = match lit_key {
            Some(kr) => (kr.reg, kr.owned_literal_string),
            None => (self.gen_expr(&idx.index)?, false),
        };
        let string_key = matches!(
            sla_lowering::peel_borrow_pointer_type(key_ty),
            Type::UserDefined(u) if u.name == "String"
        );
        let hit_label = self.new_label(if is_btree {
            "L_BTREE_MAP_INDEX_HIT"
        } else {
            "L_HASHMAP_INDEX_HIT"
        });
        let miss_label = self.new_label(if is_btree {
            "L_BTREE_MAP_INDEX_MISS"
        } else {
            "L_HASHMAP_INDEX_MISS"
        });
        let reg = if is_btree {
            let found = self.new_tmp();
            let raw_value = self.new_tmp();
            self.ins(&format!(
                "EXPAND BTREE_MAP_TRY_GET {found}, {raw_value}, {map_reg}, {key_reg}"
            ));
            self.ins(&format!("br {found} -> {hit_label}, {miss_label}"));
            self.ins(&format!("{miss_label}:"));
            self.ins(&format!("!{found}"));
            self.ins("panic(404)");
            self.ins(&format!("{hit_label}:"));
            self.ins(&format!("!{found}"));
            let reg = self.new_tmp();
            self.ins(&format!(
                "{reg} = {raw_value} as {}",
                crate::codegen::type_string(value_ty)
            ));
            self.emit_release(&raw_value)?;
            reg
        } else {
            let value_ptr = self.new_tmp();
            let found = self.new_tmp();
            let macro_name = if string_key { "MAP_GET_STR" } else { "MAP_GET" };
            self.ins(&format!(
                "EXPAND {macro_name} {value_ptr}, {map_reg}, {key_reg}"
            ));
            self.ins(&format!("{found} = ne {value_ptr}, 0"));
            self.ins(&format!("br {found} -> {hit_label}, {miss_label}"));
            self.ins(&format!("{miss_label}:"));
            self.ins(&format!("!{found}"));
            self.ins("panic(404)");
            self.ins(&format!("{hit_label}:"));
            self.ins(&format!("!{found}"));
            let reg = self.new_tmp();
            self.ins(&format!(
                "{reg} = load {value_ptr}+0 as {}",
                crate::codegen::type_string(value_ty)
            ));
            self.emit_release(&value_ptr)?;
            reg
        };
        if self.expr_result_reg_needs_release(&idx.index) {
            self.emit_release(&key_reg)?;
        }
        // An owned literal String key is released after the
        // probe (Zig keyRegIsOwnedLiteralString).
        if key_owned_literal {
            self.emit_release(&key_reg)?;
        }
        if self.expr_result_reg_needs_release(&idx.target) {
            self.emit_release(&map_reg)?;
        }
        Ok(reg)
    }

    /// codegen.zig:16815 `genIndexAddress` array fallthrough +
    /// `genLoadSlotValue` + `finishIndexAddress`: element address
    /// is base + index * typeSize(elem); scalar elements load,
    /// pointer-represented aggregate elements ARE the slot
    /// address. No bounds check in this Zig arm.
    fn gen_array_index_read(
        &mut self,
        expr: &Node,
        idx: &sla_ast::IndexExpr,
        elem: &Type,
    ) -> Result<String> {
        let base_reg = self.gen_expr(&idx.target)?;
        let base_needs_release = self.expr_result_reg_needs_release(&idx.target);
        let index_reg = self.gen_expr(&idx.index)?;
        let offset_reg = self.new_tmp();
        self.ins(&format!(
            "{offset_reg} = mul {index_reg}, {}",
            crate::codegen::type_size(elem)
        ));
        let ptr_reg = self.new_tmp();
        self.ins(&format!("{ptr_reg} = ptr_add {base_reg}, {offset_reg}"));
        self.emit_release(&offset_reg)?;
        if self.expr_result_reg_needs_release(&idx.index) {
            self.emit_release(&index_reg)?;
        }
        let value_ty = self.expr_type(expr).unwrap_or_else(|| elem.clone());
        // Pointer-backed elements (records, arrays, tuples,
        // handles) store their pointer AS the slot word: the
        // element value is the loaded word. Only an inline-POD
        // element lives in the slot itself, so its value is the
        // slot address.
        let reg = if matches!(
            value_ty,
            Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_)
        ) || !crate::place::inline_pod_element(self.check, &value_ty)
        {
            let reg = self.new_tmp();
            self.ins(&format!(
                "{reg} = load {ptr_reg}+0 as {}",
                crate::codegen::type_string(&value_ty)
            ));
            self.emit_release(&ptr_reg)?;
            reg
        } else {
            ptr_reg
        };
        if base_needs_release {
            self.emit_release(&base_reg)?;
        }
        Ok(reg)
    }

    /// codegen.zig:16815 `genIndexAddress` slice branch: the data
    /// pointer lives at the slice's `Slice_ptr` field.
    fn gen_slice_index_read(
        &mut self,
        idx: &sla_ast::IndexExpr,
        elem: &Type,
    ) -> Result<String> {
        let slice_reg = self.gen_expr(&idx.target)?;
        let slice_needs_release = self.expr_result_reg_needs_release(&idx.target);
        let base_reg = self.new_tmp();
        self.ins(&format!("{base_reg} = load {slice_reg}+Slice_ptr as ptr"));
        let index_reg = self.gen_expr(&idx.index)?;
        let offset_reg = self.new_tmp();
        self.ins(&format!(
            "{offset_reg} = mul {index_reg}, {}",
            crate::codegen::type_size(elem)
        ));
        let ptr_reg = self.new_tmp();
        self.ins(&format!("{ptr_reg} = ptr_add {base_reg}, {offset_reg}"));
        self.emit_release(&offset_reg)?;
        if self.expr_result_reg_needs_release(&idx.index) {
            self.emit_release(&index_reg)?;
        }
        let reg = if matches!(
            elem,
            Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_)
        ) || !crate::place::inline_pod_element(self.check, elem)
        {
            let reg = self.new_tmp();
            self.ins(&format!(
                "{reg} = load {ptr_reg}+0 as {}",
                crate::codegen::type_string(elem)
            ));
            self.emit_release(&ptr_reg)?;
            reg
        } else {
            ptr_reg
        };
        self.emit_release(&base_reg)?;
        if slice_needs_release {
            self.emit_release(&slice_reg)?;
        }
        Ok(reg)
    }

    fn gen_literal(&mut self, lit: &Literal) -> Result<String> {
        self.gen_literal_value(lit)
    }

    /// codegen.zig:12830 identifier expression: alias loads, global
    /// scalar constants, fn-ptr vtable addresses, plain names.
    fn gen_identifier_expr(&mut self, expr: &Node, name: &str) -> Result<String> {
        // Worker-body capture reads (codegen.zig
        // `thread_capture_regs`): inside a generated spawn
        // worker, a captured name reads as the slot load.
        if let Some(reg) = self.thread_capture_regs.get(name) {
            return Ok(reg.clone());
        }
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

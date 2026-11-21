//! Expression emission, mirrored from codegen.zig's `genExpr`
//! (12824-). The slice paths (literal, identifier, scalar binary)
//! are ported in full; every other expression family fails loudly
//! with its Zig reference until its lane lands.

use sla_ast::{Literal, Node, Primitive, Type};
use sla_lowering::{
    is_option_none_name, is_void_type, plan_scalar_binary_op, scalar_binary_op_name,
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
            Node::SwitchExpr(_) => self.gen_switch_expr(expr),
            Node::CallExpr(_) => self.gen_expr_call(expr),
            Node::IndexExpr(idx) => {
                // Vec/GrowVec scalar-element reads
                // (codegen.zig `genVecIndexRead`): bounds check
                // with panic(32), then ptr arithmetic + load.
                // Plain arrays and slices go through the
                // genIndexAddress fallthrough (ptr arithmetic +
                // slot load, no bounds check in the Zig arm).
                let target_ty = self.place_type_of_expr(&idx.target).ok_or_else(|| {
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
                // codegen.zig `genVecIndexRead` takes its owner
                // register from `genVecOwnerReceiver`: a field
                // target is projected through a borrow of the
                // loaded owner word, never through the base
                // binding's own register.
                let receiver = self.gen_vec_owner_receiver(&idx.target)?;
                let vec_reg = receiver.reg.clone();
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
                    crate::codegen::vec_element_slot_size(&elem)
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
                if let Some(release_reg) = &receiver.release_reg {
                    self.emit_release(release_reg)?;
                }
                if let Some(consume_reg) = &receiver.consume_reg {
                    self.emit_forget_moved_value(consume_reg)?;
                }
                Ok(reg)
            }
            Node::BorrowExpr(_) => self.gen_borrow_expr(expr),
            Node::FieldExpr(_) => self.gen_field_expr(expr),
            Node::StructLiteral(lit) => {
                let reg = self.new_tmp();
                self.gen_struct_literal_into(&reg, lit, expr)?;
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
                let inner_ty_opt = self.expr_type(&d.expr);
                if let Some(inner_ty) = &inner_ty_opt {
                    let smart_macro = match sla_lowering::peel_borrow_pointer_type(inner_ty) {
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
                // DEF-DEREF-BORROW-WRITE (Zig codegen.zig deref arm):
                // dereferencing a borrow to an aggregate yields the
                // pointee record address itself — a borrow value IS
                // the pointee address in the text backend's model.
                // Loading through it reinterprets the record's first
                // word as a pointer. Excluded: a pointee that is
                // itself a smart pointer (its value is the handle
                // cell address), and smart-pointer deref results.
                // The ptr_add materializes a disposable alias temp
                // so consumers never retire the borrow's register.
                fn smart_deref_type(ty: &Type) -> bool {
                    matches!(ty, Type::UserDefined(u)
                        if u.generics.len() == 1
                            && matches!(u.name.as_str(), "Box" | "Rc" | "Arc"))
                }
                if let Some(Type::Borrow(pointee)) = &inner_ty_opt {
                    if !smart_deref_type(pointee) {
                        let aggregate_result = match &deref_ty {
                            Type::UserDefined(_) => !smart_deref_type(&deref_ty),
                            Type::Tuple(_) | Type::Array(_) => true,
                            _ => false,
                        };
                        if aggregate_result {
                            let reg = self.new_tmp();
                            self.ins(&format!("{reg} = ptr_add {inner}, 0"));
                            return Ok(reg);
                        }
                    }
                }
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
                    // LINE-CENSUS-MEMLEAK: the guard copy
                    // supersedes the inner cast's result temp;
                    // retire the original (the FnPtr arm above
                    // applies the same predicate to its load
                    // source). Without this, chained casts
                    // (`signed as ptr as u64`) stranded every
                    // intermediate temp (MemoryLeak 1012,
                    // test_unit_s100_cast_int_ptr).
                    if self.call_arg_result_temp_needs_release(&cast.expr, &inner) {
                        self.emit_release(&inner)?;
                    }
                }
                let reg = self.new_tmp();
                self.ins(&format!(
                    "{reg} = {src_reg} as {}",
                    crate::codegen::type_string(&cast.ty)
                ));
                // LINE-CENSUS-MEMLEAK (corrected model): no
                // source settle here -- an `as` instruction
                // consumes its source register outright (the
                // vec-len cast at head leaves no retire and no
                // leak). The one stranded register in this
                // family, the chained-cast inner temp, is
                // retired in the guard-copy arm above, because
                // the copy reads it without consuming it.
                Ok(reg)
            }
            Node::UnsafeExpr(ue) => {
                // codegen.zig:20573: an unsafe block inside an
                // active user-macro inline is the macro's value
                // form (genUserMacroUnsafeValueInline).
                if self.macro_inline_depth > 0 {
                    if let Some(decl) = self.active_inline_macro.clone() {
                        return self.gen_user_macro_unsafe_value_inline(&decl, &ue.body);
                    }
                }
                // codegen.zig genExpr unsafe arm: a VOID-typed
                // unsafe block is generated as a plain block — its
                // tail need not be an expression (the out-param
                // writer shape `unsafe { *pp = value; }`). Only a
                // value-typed unsafe block goes through the
                // tail-value-into form. The previous unconditional
                // tail-value routing rejected every void unsafe
                // block whose tail was an assignment or a call
                // statement (test_unit_ptr_out_param). The dummy
                // register follows the surface Void precedent
                // (gen_mem_forget_call).
                let expr_ty = self.require_expr_type(expr, "unsafe block result")?;
                if is_void_type(&expr_ty) {
                    self.gen_block(&ue.body)?;
                    Ok(self.new_tmp())
                } else {
                    self.gen_block_tail_value_into_fresh(&ue.body)
                }
            }
            Node::TryExpr(t) => self.gen_try_expr(expr, t),
            // F-G2: a supervised spawn in expression position
            // emits the spawn sequence (go_spawn.rs) and yields
            // the handle. F-3: a plain spawn in expression
            // position emits the sa_task_spawn sequence the
            // same way.
            Node::GoStmt(go) if go.supervised => self.gen_supervised_go_expr(go),
            Node::GoStmt(go) => self.gen_plain_go_expr(go),
            Node::ClosureLiteral(_) => {
                // codegen.zig:13333: a bare closure literal in
                // value position is the zero placeholder; the
                // literal only takes effect through a call
                // (inline expansion) or a let/const binding.
                let reg = self.new_tmp();
                self.ins(&format!("{reg} = 0"));
                Ok(reg)
            }
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
        // Inline closure expansion (codegen.zig
        // `closure_param_regs`): a closure parameter name reads
        // as the argument register bound at the call site.
        if let Some((_, reg)) = self
            .closure_param_regs
            .iter()
            .rev()
            .find(|(n, _)| n == name)
        {
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

        // Overload member symbol in fn-ptr position
        // (`pick__ov_i32` as a value): the member's per-member
        // vtable, emitted per declaration under its mangled
        // member symbol (codegen.rs
        // emit_function_pointer_vtable_decl_for_decl).
        if let Some(Type::FnPtr(_)) = expr_ty {
            let want = format!("sla__{name}");
            let member_symbol = self
                .func_index
                .iter()
                .map(|(_, decl)| self.decl_symbol(decl))
                .find(|symbol| symbol == &want);
            if let Some(symbol) = member_symbol {
                let vt = format!("SLA_FNPTR_VT_{symbol}");
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
        if bin.op == sla_ast::BinaryOp::Spaceship {
            return self.gen_spaceship_expr(bin);
        }
        let left_ty = self.expr_type(&bin.left);
        let right_ty = self.expr_type(&bin.right);
        // The struct-op family applies to struct VALUES (and
        // borrows of them). A raw-pointer operand is an address:
        // its ==/!= and arithmetic are scalar ops even when the
        // pointee is a user-defined type — routing `*Opaque !=
        // *Opaque` here made gen_struct_binary_expr decline
        // (opaque decl) and the caller error out
        // (test_unit_ptr_out_param regression).
        let struct_family_operand = |ty: &Option<Type>| -> bool {
            match ty {
                Some(Type::Pointer(_)) => false,
                Some(t) => matches!(
                    sla_lowering::peel_borrow_pointer_type(t),
                    Type::UserDefined(_)
                ),
                None => false,
            }
        };
        if struct_family_operand(&left_ty) || struct_family_operand(&right_ty) {
            if let Some(reg) = self.gen_struct_binary_expr(bin, &left_ty, &right_ty)? {
                return Ok(reg);
            }
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

    /// codegen.zig:3849 `genSpaceshipExpr`: numeric operands
    /// compare through CMP_COMPARE_I64/U64 (floats through an
    /// fcmp branch chain) into a raw -1/0/1, then materialize
    /// the Ordering struct (alloc 8, store raw at +0). A
    /// same-struct pair compares field by field as scalars,
    /// storing the first decisive field's ordering.
    fn gen_spaceship_expr(&mut self, bin: &sla_ast::BinaryExpr) -> Result<String> {
        let left_ty = self
            .expr_type(&bin.left)
            .ok_or_else(|| TextError::MissingType {
                context: "spaceship left operand type".to_string(),
            })?;
        let right_ty = self
            .expr_type(&bin.right)
            .ok_or_else(|| TextError::MissingType {
                context: "spaceship right operand type".to_string(),
            })?;
        let l = self.gen_expr(&bin.left)?;
        let r = self.gen_expr(&bin.right)?;
        let result = if matches!(left_ty, Type::Primitive(_))
            && matches!(right_ty, Type::Primitive(_))
        {
            let raw = self.new_tmp();
            if sla_lowering::is_float_type(&left_ty) {
                let less = self.new_tmp();
                let greater = self.new_tmp();
                let less_label = self.new_label("L_SPACESHIP_FLOAT_LESS");
                let check_label = self.new_label("L_SPACESHIP_FLOAT_CHECK_GREATER");
                let greater_label = self.new_label("L_SPACESHIP_FLOAT_GREATER");
                let equal_label = self.new_label("L_SPACESHIP_FLOAT_EQUAL");
                let done_label = self.new_label("L_SPACESHIP_FLOAT_DONE");
                self.ins(&format!("{less} = fcmp_lt {l}, {r}"));
                self.w(&format!("    br {less} -> {less_label}, {check_label}\n\n"));
                self.wl(&format!("{less_label}:"));
                self.ins(&format!("{raw} = add 0, -1"));
                self.w(&format!("    jmp {done_label}\n\n"));
                self.wl(&format!("{check_label}:"));
                self.ins(&format!("{greater} = fcmp_gt {l}, {r}"));
                self.w(&format!(
                    "    br {greater} -> {greater_label}, {equal_label}\n\n"
                ));
                self.wl(&format!("{greater_label}:"));
                self.ins(&format!("{raw} = add 0, 1"));
                self.w(&format!("    jmp {done_label}\n\n"));
                self.wl(&format!("{equal_label}:"));
                self.ins(&format!("{raw} = add 0, 0"));
                self.wl(&format!("{done_label}:"));
            } else {
                let macro_name = if sla_lowering::is_unsigned_integer_type(&left_ty) {
                    "CMP_COMPARE_U64"
                } else {
                    "CMP_COMPARE_I64"
                };
                self.ins(&format!("EXPAND {macro_name} {raw}, {l}, {r}"));
            }
            let ord = self.new_tmp();
            self.ins(&format!("{ord} = alloc 8"));
            self.ins(&format!("store {ord}+0, {raw} as i64"));
            ord
        } else if let (Type::UserDefined(lu), Type::UserDefined(ru)) =
            (&left_ty, &right_ty)
        {
            if lu.name != ru.name {
                return unsupported(
                    "spaceship over different struct types",
                    "codegen.zig:3849 (genSpaceshipExpr same-struct plan)",
                );
            }
            let decl = self
                .check
                .struct_decl(&lu.name)
                .cloned()
                .ok_or_else(|| TextError::MissingType {
                    context: format!("spaceship struct `{}` declaration", lu.name),
                })?;
            let ord = self.new_tmp();
            self.ins(&format!("{ord} = alloc 8"));
            self.ins(&format!("store {ord}+0, 0 as i64"));
            let done_label = self.new_label("L_SPACESHIP_STRUCT_DONE");
            for field in &decl.fields {
                let (offset, field_ty) =
                    crate::layout::struct_field_layout(&decl, &field.name).ok_or_else(
                        || TextError::MissingType {
                            context: format!("spaceship field `{}` layout", field.name),
                        },
                    )?;
                if !matches!(field_ty, Type::Primitive(_)) {
                    return unsupported(
                        "spaceship over a struct with a non-scalar field",
                        "codegen.zig:3849 (genSpaceshipExpr scalar field loads)",
                    );
                }
                let lhs = self.new_tmp();
                let rhs = self.new_tmp();
                let less = self.new_tmp();
                let greater = self.new_tmp();
                let less_label = self.new_label("L_SPACESHIP_STRUCT_LESS");
                let check_label = self.new_label("L_SPACESHIP_STRUCT_CHECK_GREATER");
                let greater_label = self.new_label("L_SPACESHIP_STRUCT_GREATER");
                let next_label = self.new_label("L_SPACESHIP_STRUCT_NEXT");
                let unsigned = sla_lowering::is_unsigned_integer_type(&field_ty);
                let float = sla_lowering::is_float_type(&field_ty);
                let (less_op, greater_op) = if float {
                    ("fcmp_lt", "fcmp_gt")
                } else if unsigned {
                    ("ult", "ugt")
                } else {
                    ("slt", "sgt")
                };
                let ty_str = crate::codegen::type_string(&field_ty);
                self.ins(&format!("{lhs} = load {l}+{offset} as {ty_str}"));
                self.ins(&format!("{rhs} = load {r}+{offset} as {ty_str}"));
                self.ins(&format!("{less} = {less_op} {lhs}, {rhs}"));
                self.w(&format!("    br {less} -> {less_label}, {check_label}\n\n"));
                self.wl(&format!("{less_label}:"));
                self.ins(&format!("store {ord}+0, -1 as i64"));
                self.w(&format!("    jmp {done_label}\n\n"));
                self.wl(&format!("{check_label}:"));
                self.ins(&format!("{greater} = {greater_op} {lhs}, {rhs}"));
                self.w(&format!(
                    "    br {greater} -> {greater_label}, {next_label}\n\n"
                ));
                self.wl(&format!("{greater_label}:"));
                self.ins(&format!("store {ord}+0, 1 as i64"));
                self.w(&format!("    jmp {done_label}\n\n"));
                self.wl(&format!("{next_label}:"));
            }
            self.wl(&format!("{done_label}:"));
            ord
        } else {
            return unsupported(
                "spaceship over mixed operand kinds",
                "codegen.zig:3849 (genSpaceshipExpr plan)",
            );
        };
        if sla_lowering::call_arg_needs_release(&bin.left) {
            self.emit_release(&l)?;
        }
        if sla_lowering::call_arg_needs_release(&bin.right) {
            self.emit_release(&r)?;
        }
        Ok(result)
    }

    /// codegen.zig:12918 binary_expr user-defined operand
    /// families, in the Zig dispatch order: a checker-resolved
    /// @overload operator becomes a direct call to the mangled
    /// overload symbol; then field-wise struct equality
    /// (genStructEqualityExpr), derived-ord comparison
    /// (genStructOrdExpr) and field-wise struct arithmetic
    /// (genStructArithmeticExpr). Returns Ok(None) when no
    /// family applies (the caller keeps the loud unsupported).
    fn gen_struct_binary_expr(
        &mut self,
        bin: &sla_ast::BinaryExpr,
        left_ty: &Option<Type>,
        right_ty: &Option<Type>,
    ) -> Result<Option<String>> {
        let peeled = |t: &Option<Type>| {
            t.as_ref()
                .map(|ty| sla_lowering::peel_borrow_pointer_type(ty).clone())
        };
        let left_ty = peeled(left_ty);
        let right_ty = peeled(right_ty);
        let struct_decl_of = |t: &Option<Type>| -> Option<(String, sla_ast::StructDecl)> {
            match t {
                Some(Type::UserDefined(u)) => self
                    .check
                    .struct_decl(&u.name)
                    .map(|d| (u.name.clone(), d.clone())),
                _ => None,
            }
        };
        let left_struct = struct_decl_of(&left_ty);
        let right_struct = struct_decl_of(&right_ty);

        // @overload operator dispatch: the checker resolved the
        // operator to the unique overload whose parameters match
        // the operands exactly (check_binary.rs overload_symbol);
        // the body lives in the program under the mangled name
        // `{Type}_{op_name}` (program.rs OverloadDecl arm).
        let op_name = match bin.op {
            sla_ast::BinaryOp::Add => Some("op_add"),
            sla_ast::BinaryOp::Sub => Some("op_sub"),
            sla_ast::BinaryOp::Mul => Some("op_mul"),
            sla_ast::BinaryOp::Div => Some("op_div"),
            _ => None,
        };
        if let (Some(op_name), Some((lname, _))) = (op_name, &left_struct) {
            let mangled = sla_lowering::mangle_method_name(lname, op_name);
            let decl = self
                .assoc_index
                .iter()
                .find(|(n, d)| {
                    *n == mangled
                        && d.params.len() == 2
                        && left_ty.as_ref() == Some(&*d.params[0].ty)
                        && right_ty.as_ref() == Some(&*d.params[1].ty)
                })
                .map(|(_, d)| d.clone());
            if let Some(decl) = decl {
                let call = sla_ast::CallExpr {
                    func_name: mangled.clone(),
                    associated_target: None,
                    generics: Vec::new(),
                    args: vec![(*bin.left).clone(), (*bin.right).clone()],
                    callee: None,
                };
                let (display, planned_args) =
                    self.plan_known_call_args(&call, &decl.params)?;
                let result = self.new_tmp();
                let lowered = self.lowered_func_symbol(&mangled);
                self.ins(&format!(
                    "{result} = call @{lowered}({})",
                    display.join(", ")
                ));
                self.emit_lowered_call_arg_cleanups(&planned_args, Some(&mangled))?;
                return Ok(Some(result));
            }
        }

        let (Some((_, ldecl)), Some((_, rdecl))) = (&left_struct, &right_struct) else {
            // Struct-scalar arithmetic (mul by a scalar, 0 - x)
            // is part of the arithmetic family below.
            return self.gen_struct_scalar_arith(bin, &left_struct, &right_struct);
        };
        if ldecl.name != rdecl.name || ldecl.is_opaque || ldecl.is_union {
            return Ok(None);
        }
        let decl = ldecl;

        // genStructEqualityExpr: field-wise `eq` AND-chained.
        if matches!(bin.op, sla_ast::BinaryOp::Eq | sla_ast::BinaryOp::Ne) {
            let l = self.gen_expr(&bin.left)?;
            let r = self.gen_expr(&bin.right)?;
            let mut acc: Option<String> = None;
            for field in &decl.fields {
                let (offset, field_ty) =
                    crate::layout::struct_field_layout(decl, &field.name).ok_or_else(
                        || TextError::MissingType {
                            context: format!("struct eq field `{}` layout", field.name),
                        },
                    )?;
                let ty_str = crate::codegen::type_string(&field_ty);
                let lhs = self.new_tmp();
                let rhs = self.new_tmp();
                let eq_reg = self.new_tmp();
                self.ins(&format!("{lhs} = load {l}+{offset} as {ty_str}"));
                self.ins(&format!("{rhs} = load {r}+{offset} as {ty_str}"));
                self.ins(&format!("{eq_reg} = eq {lhs}, {rhs}"));
                acc = Some(match acc {
                    Some(prev) => {
                        let next = self.new_tmp();
                        self.ins(&format!("{next} = and {prev}, {eq_reg}"));
                        next
                    }
                    None => eq_reg,
                });
            }
            let result = self.new_tmp();
            match acc {
                Some(eq_all) => {
                    if bin.op == sla_ast::BinaryOp::Eq {
                        self.ins(&format!("{result} = or {eq_all}, 0"));
                    } else {
                        self.ins(&format!("{result} = ne {eq_all}, 1"));
                    }
                }
                None => {
                    let empty = if bin.op == sla_ast::BinaryOp::Eq { 1 } else { 0 };
                    self.ins(&format!("{result} = add 0, {empty}"));
                }
            }
            if sla_lowering::call_arg_needs_release(&bin.left) {
                self.emit_release(&l)?;
            }
            if sla_lowering::call_arg_needs_release(&bin.right) {
                self.emit_release(&r)?;
            }
            return Ok(Some(result));
        }

        // genStructOrdExpr: derived `ord`, lexicographic over
        // the fields with an equality-prefix chain.
        if matches!(
            bin.op,
            sla_ast::BinaryOp::Lt
                | sla_ast::BinaryOp::Le
                | sla_ast::BinaryOp::Gt
                | sla_ast::BinaryOp::Ge
        ) && decl.derives.iter().any(|d| d == "ord")
        {
            let l = self.gen_expr(&bin.left)?;
            let r = self.gen_expr(&bin.right)?;
            let cmp_op = match bin.op {
                sla_ast::BinaryOp::Lt | sla_ast::BinaryOp::Le => "slt",
                _ => "sgt",
            };
            let mut order_acc: Option<String> = None;
            let mut eq_prefix: Option<String> = None;
            for field in &decl.fields {
                let (offset, field_ty) =
                    crate::layout::struct_field_layout(decl, &field.name).ok_or_else(
                        || TextError::MissingType {
                            context: format!("struct ord field `{}` layout", field.name),
                        },
                    )?;
                let ty_str = crate::codegen::type_string(&field_ty);
                let lhs = self.new_tmp();
                let rhs = self.new_tmp();
                let cmp = self.new_tmp();
                let eq_reg = self.new_tmp();
                self.ins(&format!("{lhs} = load {l}+{offset} as {ty_str}"));
                self.ins(&format!("{rhs} = load {r}+{offset} as {ty_str}"));
                self.ins(&format!("{cmp} = {cmp_op} {lhs}, {rhs}"));
                self.ins(&format!("{eq_reg} = eq {lhs}, {rhs}"));
                let term = match &eq_prefix {
                    Some(prefix) => {
                        let t = self.new_tmp();
                        self.ins(&format!("{t} = and {prefix}, {cmp}"));
                        t
                    }
                    None => cmp,
                };
                order_acc = Some(match order_acc {
                    Some(prev) => {
                        let next = self.new_tmp();
                        self.ins(&format!("{next} = or {prev}, {term}"));
                        next
                    }
                    None => term,
                });
                eq_prefix = Some(match eq_prefix {
                    Some(prefix) => {
                        let next = self.new_tmp();
                        self.ins(&format!("{next} = and {prefix}, {eq_reg}"));
                        next
                    }
                    None => eq_reg,
                });
            }
            let result = self.new_tmp();
            match order_acc {
                Some(ordered) => {
                    if matches!(bin.op, sla_ast::BinaryOp::Le | sla_ast::BinaryOp::Ge) {
                        let eq_all = eq_prefix.ok_or_else(|| TextError::MissingType {
                            context: "struct ord equality prefix".to_string(),
                        })?;
                        self.ins(&format!("{result} = or {ordered}, {eq_all}"));
                    } else {
                        self.ins(&format!("{result} = or {ordered}, 0"));
                    }
                }
                None => {
                    let empty = if matches!(
                        bin.op,
                        sla_ast::BinaryOp::Le | sla_ast::BinaryOp::Ge
                    ) {
                        1
                    } else {
                        0
                    };
                    self.ins(&format!("{result} = add 0, {empty}"));
                }
            }
            if sla_lowering::call_arg_needs_release(&bin.left) {
                self.emit_release(&l)?;
            }
            if sla_lowering::call_arg_needs_release(&bin.right) {
                self.emit_release(&r)?;
            }
            return Ok(Some(result));
        }

        // genStructArithmeticExpr, same-struct add/sub arm:
        // field-wise numeric op into a fresh struct image.
        if matches!(bin.op, sla_ast::BinaryOp::Add | sla_ast::BinaryOp::Sub) {
            let op = if bin.op == sla_ast::BinaryOp::Add {
                "add"
            } else {
                "sub"
            };
            let l = self.gen_expr(&bin.left)?;
            let r = self.gen_expr(&bin.right)?;
            let result = self.new_tmp();
            self.ins(&format!(
                "{result} = alloc {}",
                crate::layout::struct_size(decl)
            ));
            for field in &decl.fields {
                if !sla_lowering::is_numeric_type(&field.ty) {
                    return Err(TextError::Unsupported {
                        feature: format!(
                            "struct arithmetic over non-numeric field `{}`",
                            field.name
                        ),
                        zig_ref: "codegen.zig:3590 (genStructArithmeticExpr)",
                    });
                }
                let (offset, field_ty) =
                    crate::layout::struct_field_layout(decl, &field.name).ok_or_else(
                        || TextError::MissingType {
                            context: format!(
                                "struct arith field `{}` layout",
                                field.name
                            ),
                        },
                    )?;
                let ty_str = crate::codegen::type_string(&field_ty);
                let lhs = self.new_tmp();
                let rhs = self.new_tmp();
                let value = self.new_tmp();
                self.ins(&format!("{lhs} = load {l}+{offset} as {ty_str}"));
                self.ins(&format!("{rhs} = load {r}+{offset} as {ty_str}"));
                self.ins(&format!("{value} = {op} {lhs}, {rhs}"));
                self.ins(&format!("store {result}+{offset}, {value} as {ty_str}"));
            }
            if sla_lowering::call_arg_needs_release(&bin.left) {
                self.emit_release(&l)?;
            }
            if sla_lowering::call_arg_needs_release(&bin.right) {
                self.emit_release(&r)?;
            }
            return Ok(Some(result));
        }

        Ok(None)
    }

    /// genStructArithmeticExpr's mixed arms: struct * scalar,
    /// scalar * struct, and `0 - struct` negation. Ok(None)
    /// when neither operand is a struct with a numeric partner.
    fn gen_struct_scalar_arith(
        &mut self,
        bin: &sla_ast::BinaryExpr,
        left_struct: &Option<(String, sla_ast::StructDecl)>,
        right_struct: &Option<(String, sla_ast::StructDecl)>,
    ) -> Result<Option<String>> {
        // struct * scalar / scalar * struct.
        if bin.op == sla_ast::BinaryOp::Mul {
            let (decl, struct_side_left) = match (left_struct, right_struct) {
                (Some((_, d)), None) => (d, true),
                (None, Some((_, d))) => (d, false),
                _ => return Ok(None),
            };
            if decl.is_opaque || decl.is_union {
                return Ok(None);
            }
            let (struct_node, scalar_node) = if struct_side_left {
                (&bin.left, &bin.right)
            } else {
                (&bin.right, &bin.left)
            };
            let struct_reg = self.gen_expr(struct_node)?;
            let scalar = self.gen_expr(scalar_node)?;
            let result = self.new_tmp();
            self.ins(&format!(
                "{result} = alloc {}",
                crate::layout::struct_size(decl)
            ));
            for field in &decl.fields {
                if !sla_lowering::is_numeric_type(&field.ty) {
                    return Err(TextError::Unsupported {
                        feature: format!(
                            "struct scalar-mul over non-numeric field `{}`",
                            field.name
                        ),
                        zig_ref: "codegen.zig:3590 (genStructArithmeticExpr)",
                    });
                }
                let (offset, field_ty) =
                    crate::layout::struct_field_layout(decl, &field.name).ok_or_else(
                        || TextError::MissingType {
                            context: format!(
                                "struct mul field `{}` layout",
                                field.name
                            ),
                        },
                    )?;
                let ty_str = crate::codegen::type_string(&field_ty);
                let lhs = self.new_tmp();
                let value = self.new_tmp();
                self.ins(&format!("{lhs} = load {struct_reg}+{offset} as {ty_str}"));
                self.ins(&format!("{value} = mul {lhs}, {scalar}"));
                self.ins(&format!("store {result}+{offset}, {value} as {ty_str}"));
            }
            if sla_lowering::call_arg_needs_release(&bin.left) {
                let reg = if struct_side_left { &struct_reg } else { &scalar };
                self.emit_release(reg)?;
            }
            if sla_lowering::call_arg_needs_release(&bin.right) {
                let reg = if struct_side_left { &scalar } else { &struct_reg };
                self.emit_release(reg)?;
            }
            return Ok(Some(result));
        }
        // 0 - struct negation.
        if bin.op == sla_ast::BinaryOp::Sub {
            if let (None, Some((_, decl))) = (left_struct, right_struct) {
                if decl.is_opaque || decl.is_union {
                    return Ok(None);
                }
                let is_zero = matches!(
                    &*bin.left,
                    Node::Literal(sla_ast::Literal::IntVal(v)) if *v == 0
                );
                if !is_zero {
                    return Ok(None);
                }
                let zero_reg = self.gen_expr(&bin.left)?;
                self.emit_release(&zero_reg)?;
                let r = self.gen_expr(&bin.right)?;
                let result = self.new_tmp();
                self.ins(&format!(
                    "{result} = alloc {}",
                    crate::layout::struct_size(decl)
                ));
                for field in &decl.fields {
                    if !sla_lowering::is_numeric_type(&field.ty) {
                        return Err(TextError::Unsupported {
                            feature: format!(
                                "struct negation over non-numeric field `{}`",
                                field.name
                            ),
                            zig_ref: "codegen.zig:3590 (genStructArithmeticExpr)",
                        });
                    }
                    let (offset, field_ty) = crate::layout::struct_field_layout(
                        decl,
                        &field.name,
                    )
                    .ok_or_else(|| TextError::MissingType {
                        context: format!("struct neg field `{}` layout", field.name),
                    })?;
                    let ty_str = crate::codegen::type_string(&field_ty);
                    let rhs = self.new_tmp();
                    let value = self.new_tmp();
                    self.ins(&format!("{rhs} = load {r}+{offset} as {ty_str}"));
                    self.ins(&format!("{value} = sub 0, {rhs}"));
                    self.ins(&format!(
                        "store {result}+{offset}, {value} as {ty_str}"
                    ));
                }
                if sla_lowering::call_arg_needs_release(&bin.right) {
                    self.emit_release(&r)?;
                }
                return Ok(Some(result));
            }
        }
        Ok(None)
    }

    /// codegen.zig:24137 `try_expr`: postfix `?` unwrapper. The
    /// error path emits the branch-scoped cleanups for the try
    /// node (inside a user-macro inline, the macro-call try
    /// cleanup list) and returns the whole inner value.
    fn gen_try_expr(&mut self, expr: &Node, try_expr: &sla_ast::TryExpr) -> Result<String> {
        let inner_ty = self
            .resolved_type_for_expr(&try_expr.expr)
            .ok_or_else(|| TextError::MissingType {
                context: "try operand type".to_string(),
            })?;
        // Option arm.
        if let Type::UserDefined(u) = &inner_ty {
            if u.name == "Option" && u.generics.len() == 1 {
                let inner_reg = self.gen_expr(&try_expr.expr)?;
                let is_some = self.new_tmp();
                self.ins(&format!("EXPAND OPTION_IS_SOME {is_some}, {inner_reg}"));
                let some_label = self.new_label("L_TRY_OPTION_SOME");
                let none_label = self.new_label("L_TRY_OPTION_NONE");
                self.w(&format!(
                    "    br {is_some} -> {some_label}, {none_label}\n\n"
                ));
                self.wl(&format!("{none_label}:"));
                self.ins(&format!("!{is_some}"));
                self.emit_branch_scoped_cleanup_for_node(expr)?;
                self.w(&format!("    return {inner_reg}\n\n"));
                self.wl(&format!("{some_label}:"));
                self.ins(&format!("!{is_some}"));
                let some_val = self.new_tmp();
                self.ins(&format!("EXPAND OPTION_GET {some_val}, {inner_reg}"));
                self.ins(&format!("!{inner_reg}"));
                return Ok(some_val);
            }
            if u.name == "Result" && !u.generics.is_empty() {
                let inner_reg = self.gen_expr(&try_expr.expr)?;
                let is_ok = self.new_tmp();
                self.ins(&format!("EXPAND RESULT_IS_OK {is_ok}, {inner_reg}"));
                let ok_label = self.new_label("L_TRY_RESULT_OK");
                let err_label = self.new_label("L_TRY_RESULT_ERR");
                self.w(&format!("    br {is_ok} -> {ok_label}, {err_label}\n\n"));
                self.wl(&format!("{err_label}:"));
                self.ins(&format!("!{is_ok}"));
                self.emit_branch_scoped_cleanup_for_node(expr)?;
                self.w(&format!("    return {inner_reg}\n\n"));
                self.wl(&format!("{ok_label}:"));
                self.ins(&format!("!{is_ok}"));
                let ok_val = self.new_tmp();
                self.ins(&format!("EXPAND RESULT_GET_OK {ok_val}, {inner_reg}"));
                self.ins(&format!("!{inner_reg}"));
                return Ok(ok_val);
            }
        }
        // User-defined result-struct arm.
        let result_ty = sla_lowering::peel_borrow_pointer_type(&inner_ty).clone();
        let type_name = match &result_ty {
            Type::UserDefined(u) => u.name.clone(),
            _ => {
                return Err(TextError::MissingType {
                    context: "try operand is not an Option/Result/result struct"
                        .to_string(),
                })
            }
        };
        let decl = self
            .check
            .struct_decl(&type_name)
            .cloned()
            .ok_or_else(|| TextError::MissingType {
                context: format!("try result struct `{type_name}` declaration"),
            })?;
        let (is_err_offset, is_err_ty) =
            crate::layout::struct_field_layout(&decl, "is_err").ok_or_else(|| {
                TextError::MissingType {
                    context: "try result struct `is_err` field".to_string(),
                }
            })?;
        let (value_offset, value_ty) =
            crate::layout::struct_field_layout(&decl, "value").ok_or_else(|| {
                TextError::MissingType {
                    context: "try result struct `value` field".to_string(),
                }
            })?;
        let inner_reg = self.gen_expr(&try_expr.expr)?;
        let is_err = self.new_tmp();
        self.ins(&format!(
            "{is_err} = load {inner_reg}+{is_err_offset} as {}",
            crate::codegen::type_string(&is_err_ty)
        ));
        let ok_label = self.new_label("L_TRY_OK");
        let err_label = self.new_label("L_TRY_ERR");
        self.w(&format!("    br {is_err} -> {err_label}, {ok_label}\n\n"));
        self.wl(&format!("{err_label}:"));
        self.ins(&format!("!{is_err}"));
        self.emit_branch_scoped_cleanup_for_node(expr)?;
        self.w(&format!("    return {inner_reg}\n\n"));
        self.wl(&format!("{ok_label}:"));
        self.ins(&format!("!{is_err}"));
        let ok_val = self.new_tmp();
        self.ins(&format!(
            "{ok_val} = load {inner_reg}+{value_offset} as {}",
            crate::codegen::type_string(&value_ty)
        ));
        self.ins(&format!("!{inner_reg}"));
        Ok(ok_val)
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

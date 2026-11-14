//! D-group `format_print` — the `println` family, ported for
//! CLI-D4 from the current-head Zig (`sab_codegen.zig`):
//! `genPrintlnCall` (13355), `emitPrintlnArg` (13274),
//! `emitPrintPrimitiveValue` (13253), `emitPrintSliceValue`
//! (13243), `emitPrintConstBytes` (13234), `emitPrintBytes`
//! (13230), over `lowering_rules.zig`'s `planPrintlnArg` (5231)
//! and `printPrimitiveFormat` (5218).
//!
//! Ported plans: the literal format-string walk (segments and
//! `{}` holes), string-literal hole arguments, the primitive
//! plan (with Zig's non-consuming `op.add value, raw, 0` copy),
//! and the borrowed/boxed-primitive plans (load, print,
//! release). The zero/one-arg historical paths (bare newline;
//! a single format-string/string-like value) follow Zig's
//! control flow exactly — but the two value plans themselves
//! (`format_string`, `string_like`) are NOT ported: they need
//! the `STRING_BUF_AS_STR` / `CSTR_LEN` fragment expansions plus
//! the string-binding materialization classification
//! (`lowering_rules.zig` #3/#5 contract), and they stop loudly
//! here with that coordinate instead of being approximated.
//! Any other unclassifiable argument is Zig's own final
//! `UnsupportedSabDirectFeature` from `emitPrintlnArg`.

use sla_ast::{CallExpr, Literal, Node, Primitive, Type};

use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::fragment::FragmentMachine;
use crate::fragment_imports::MachineStdDeps;
use crate::ir::{OpKind, Operand, PrimType};
use crate::surface_query::StdDepEnsurer;

/// Zig `PrintPrimitiveFormat` (lowering_rules.zig:5149).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrintFormat {
    SignedInt,
    UnsignedInt,
    Float,
    Boolean,
}

/// Zig `printPrimitiveFormat` (lowering_rules.zig:5218).
fn print_primitive_format(ty: &Type) -> Option<PrintFormat> {
    let Type::Primitive(p) = ty else {
        return None;
    };
    Some(match p {
        Primitive::I8 | Primitive::I16 | Primitive::I32 | Primitive::I64 | Primitive::Isize
        | Primitive::Integer => PrintFormat::SignedInt,
        Primitive::U8 | Primitive::U16 | Primitive::U32 | Primitive::U64 | Primitive::Usize => {
            PrintFormat::UnsignedInt
        }
        Primitive::F32 | Primitive::F64 | Primitive::Float => PrintFormat::Float,
        Primitive::Boolean => PrintFormat::Boolean,
        _ => return None,
    })
}

/// Zig `isFormatStringType` (lowering_rules.zig, peel first):
/// the peeled type is a user-defined `String`.
fn is_format_string_type(ty: &Type) -> bool {
    matches!(
        sla_lowering::peel_borrow_pointer_type(ty),
        Type::UserDefined(ud) if ud.name == "String"
    )
}

/// Zig `isStringLikeType`: peeled raw_ptr / array / `String`.
fn is_string_like_type(ty: &Type) -> bool {
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::Primitive(Primitive::RawPtr) => true,
        Type::Array(_) => true,
        Type::UserDefined(ud) => ud.name == "String",
        _ => false,
    }
}

/// Zig `borrowedPrimitiveType`: a borrow/pointer directly over
/// a primitive type.
fn borrowed_primitive_type(ty: &Type) -> Option<Type> {
    let inner = match ty {
        Type::Borrow(inner) | Type::Pointer(inner) => inner,
        _ => return None,
    };
    matches!(&**inner, Type::Primitive(_)).then(|| (**inner).clone())
}

/// Zig `boxInnerType` for the single-generic `Box<T>` shape.
fn box_inner_type(ty: &Type) -> Option<Type> {
    let Type::UserDefined(ud) = ty else {
        return None;
    };
    if ud.name != "Box" || ud.generics.len() != 1 {
        return None;
    }
    Some(ud.generics[0].clone())
}

/// Zig `PrintlnArgPlan` (lowering_rules.zig:5156) + `planPrintlnArg`.
enum PrintlnArgPlan {
    FormatString,
    StringLike,
    BorrowedPrimitive(Type),
    BoxedPrimitive(Type),
    Primitive(PrintFormat),
    Unsupported,
}

fn plan_println_arg(ty: Option<&Type>) -> PrintlnArgPlan {
    let Some(ty) = ty else {
        return PrintlnArgPlan::Unsupported;
    };
    if is_format_string_type(ty) {
        return PrintlnArgPlan::FormatString;
    }
    if is_string_like_type(ty) {
        return PrintlnArgPlan::StringLike;
    }
    if let Some(inner) = borrowed_primitive_type(ty) {
        return PrintlnArgPlan::BorrowedPrimitive(inner);
    }
    if let Some(inner) = box_inner_type(ty) {
        if print_primitive_format(&inner).is_some() {
            return PrintlnArgPlan::BoxedPrimitive(inner);
        }
    }
    if let Some(format) = print_primitive_format(ty) {
        return PrintlnArgPlan::Primitive(format);
    }
    PrintlnArgPlan::Unsupported
}

impl<'c> Driver<'c> {
    /// Zig `ensurePrintlnDeps` through the F machine (the
    /// driver's flattener is the CLI-D4 host provider in the
    /// standalone binary; the loud default in unit contexts).
    fn ensure_println_deps(&mut self) -> Result<()> {
        let mut ensurer = MachineStdDeps {
            machine: FragmentMachine::new(&mut self.state),
            flattener: &mut *self.flattener,
        };
        ensurer.ensure_println_deps()
    }

    /// Zig `emitCallBody` over the fragment machine (its
    /// operand shape is the Zig one: text in operands[0] when
    /// there is no destination).
    fn emit_call_body(&mut self, dst: Option<u32>, body: String) -> Result<()> {
        FragmentMachine::new(&mut self.state).emit_call_body(dst, body)
    }

    /// Zig `emitPrintBytes` (13230).
    fn emit_print_bytes(&mut self, ptr_name: &str, len_name: &str) -> Result<()> {
        self.emit_call_body(
            None,
            format!("@sa_print_bytes(&{ptr_name}, {len_name})"),
        )
    }

    /// Zig `emitPrintConstBytes` (13234): decode the literal
    /// text, intern a fresh `SLA_STR_{n}` utf8 const for the
    /// bytes, and print it with its decoded length.
    fn emit_print_const_bytes(&mut self, bytes_text: &str) -> Result<()> {
        let bytes = crate::literals::decode_string_literal_bytes(bytes_text.as_bytes())?;
        if bytes.is_empty() {
            return Ok(());
        }
        let len = bytes.len();
        let label = self.b().append_utf8_const(&bytes);
        self.emit_print_bytes(&label, &len.to_string())
    }

    /// Zig `emitPrintPrimitiveValue` (13253).
    fn emit_print_primitive_value(&mut self, value_reg: u32, format: PrintFormat) -> Result<()> {
        let value_name = self.b().symbol_name(value_reg).to_string();
        let fmt_buf = self.b().new_tmp();
        let fmt_body = match format {
            PrintFormat::SignedInt => format!("@sa_fmt_i64({value_name}, 10)"),
            PrintFormat::UnsignedInt => format!("@sa_fmt_u64({value_name}, 10)"),
            PrintFormat::Float => format!("@sa_fmt_f64({value_name}, 10)"),
            PrintFormat::Boolean => format!("@sa_fmt_bool({value_name})"),
        };
        self.emit_call_body(Some(fmt_buf), fmt_body)?;
        let fmt_buf_name = self.b().symbol_name(fmt_buf).to_string();
        let data_reg = self.b().new_tmp();
        self.emit_call_body(
            Some(data_reg),
            format!("@sa_fmt_buffer_data({fmt_buf_name})"),
        )?;
        let len_reg = self.b().new_tmp();
        self.emit_call_body(
            Some(len_reg),
            format!("@sa_fmt_buffer_len({fmt_buf_name})"),
        )?;
        let print_ptr = self.b().new_tmp();
        self.b().emit_assign_reg(print_ptr, data_reg);
        let print_ptr_name = self.b().symbol_name(print_ptr).to_string();
        let len_name = self.b().symbol_name(len_reg).to_string();
        self.emit_print_bytes(&print_ptr_name, &len_name)?;
        self.emit_call_body(
            None,
            format!("@sa_fmt_buffer_free(^{fmt_buf_name})"),
        )?;
        self.emit_release(print_ptr)?;
        self.emit_release(len_reg)?;
        Ok(())
    }

    /// Zig `emitPrintlnArg` (13274), ported plans only — see
    /// the module docs for the two loud stops.
    fn emit_println_arg(&mut self, arg: &Node) -> Result<()> {
        if let Node::Literal(Literal::StringVal(text)) = arg {
            let text = text.clone();
            return self.emit_print_const_bytes(&text);
        }
        let arg_ty = self.check.expr_type(arg).cloned();
        match plan_println_arg(arg_ty.as_ref()) {
            PrintlnArgPlan::FormatString => Err(unsupported(
                "println of a format-string (String) argument (STRING_BUF_AS_STR fragment + slice print)",
                "D surface (format_print)",
                "13282-13294",
            )),
            PrintlnArgPlan::StringLike => Err(unsupported(
                "println of a string-like argument (CSTR_LEN / slice print + string-binding materialization)",
                "D surface (format_print)",
                "13295-13310",
            )),
            PrintlnArgPlan::BorrowedPrimitive(inner) | PrintlnArgPlan::BoxedPrimitive(inner) => {
                let format = print_primitive_format(&inner).ok_or_else(|| {
                    unsupported(
                        "println of a borrowed/boxed non-formattable primitive",
                        "D surface (format_print)",
                        "13311-13330",
                    )
                })?;
                let ptr_reg = self.gen_expr(arg)?;
                let value_reg = self.b().new_tmp();
                self.emit_load(
                    value_reg,
                    ptr_reg,
                    0,
                    crate::layout::storage_prim_type(&inner)?,
                );
                self.emit_print_primitive_value(value_reg, format)?;
                self.emit_release(value_reg)?;
                self.release_expr_result_if_needed(arg, ptr_reg)?;
                Ok(())
            }
            PrintlnArgPlan::Primitive(format) => {
                let raw_reg = self.gen_expr(arg)?;
                // Zig's non-consuming copy idiom (see the long
                // comment at sab_codegen.zig:13339-13354): an
                // `op.add` by zero, never an assign (which the
                // verifier models as a move).
                let value_reg = self.b().new_tmp();
                self.b().emit_op(
                    value_reg,
                    OpKind::Add,
                    Operand::Reg(raw_reg),
                    Operand::ImmI64(0),
                );
                self.emit_print_primitive_value(value_reg, format)?;
                self.emit_release(value_reg)?;
                self.release_expr_result_if_needed(arg, raw_reg)?;
                Ok(())
            }
            PrintlnArgPlan::Unsupported => Err(unsupported(
                "println argument with no print plan (emitPrintlnArg final arm)",
                "D surface (format_print)",
                "13350-13352",
            )),
        }
    }

    /// Zig `genPrintlnCall` (13355).
    pub(crate) fn gen_println_call(&mut self, call: &CallExpr) -> Result<u32> {
        if call.associated_target.is_some() {
            return Err(unsupported(
                "println with an associated target",
                "D surface (format_print)",
                "13356",
            ));
        }
        self.ensure_println_deps()?;
        let fmt_literal = match call.args.first() {
            Some(Node::Literal(Literal::StringVal(text))) => Some(text.clone()),
            _ => None,
        };
        match fmt_literal {
            None => {
                // Zig #4b: a single string-like / format-string
                // argument prints its value; every other arity
                // keeps the historical bare-newline behavior.
                if call.args.len() == 1 {
                    let arg_ty = self.check.expr_type(&call.args[0]).cloned();
                    let plan = plan_println_arg(arg_ty.as_ref());
                    if matches!(
                        plan,
                        PrintlnArgPlan::FormatString | PrintlnArgPlan::StringLike
                    ) {
                        self.emit_println_arg(&call.args[0])?;
                        self.emit_print_const_bytes("\n")?;
                        let printed = self.b().new_tmp();
                        self.b().emit_assign_imm(printed, 0);
                        return Ok(printed);
                    }
                }
                self.emit_print_const_bytes("\n")?;
            }
            Some(fmt) => {
                let fmt_bytes = fmt.as_bytes();
                let mut arg_idx = 1usize;
                let mut i = 0usize;
                while i <= fmt_bytes.len() {
                    let start = i;
                    while i < fmt_bytes.len()
                        && !(fmt_bytes[i] == b'{'
                            && i + 1 < fmt_bytes.len()
                            && fmt_bytes[i + 1] == b'}')
                    {
                        i += 1;
                    }
                    if i > start {
                        self.emit_print_const_bytes(&fmt[start..i])?;
                    }
                    if i >= fmt_bytes.len() {
                        break;
                    }
                    if arg_idx >= call.args.len() {
                        break;
                    }
                    self.emit_println_arg(&call.args[arg_idx])?;
                    arg_idx += 1;
                    i += 2;
                }
                self.emit_print_const_bytes("\n")?;
            }
        }
        let sentinel = self.b().new_tmp();
        self.b().emit_assign_imm(sentinel, 0);
        Ok(sentinel)
    }
    // -- format() (Zig genFormatCall family, 13741-13960) --------

    /// Zig `ensureDebugFormatDeps`: the string_format.sa deps
    /// the format builder's fragments and `_into` calls need.
    fn ensure_debug_format_deps(&mut self) -> Result<()> {
        let mut ensurer = MachineStdDeps {
            machine: FragmentMachine::new(&mut self.state),
            flattener: &mut *self.flattener,
        };
        ensurer.ensure_std_deps(
            "sa_std/string_format.sa",
            &[
                "sa_vec_with_capacity".to_string(),
                "sa_vec_reserve".to_string(),
                "sa_vec_free".to_string(),
                "sa_mem_copy".to_string(),
                "sa_mem_set".to_string(),
                "sa_fmt_i64_into".to_string(),
                "sa_fmt_u64_into".to_string(),
                "sa_fmt_f64_into".to_string(),
                "sa_fmt_bool_into".to_string(),
            ],
        )
    }

    /// Zig `newFormatTag`: a fresh hygiene tag from the tmp
    /// counter.
    fn new_format_tag(&mut self) -> String {
        let n = self.b().new_tmp();
        format!("fmttag_{n}")
    }

    /// Zig `emitFormatPushConstBytes` (13741): intern the bytes
    /// as a utf8 const and push them into the format string.
    fn emit_format_push_const_bytes(&mut self, out_string: u32, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let label = self.b().append_utf8_const(bytes);
        let ptr_reg = self.b().new_tmp();
        let symbol_id = self.b().intern(&label);
        self.b().record_reg(symbol_id);
        self.emit_borrow_reg(ptr_reg, symbol_id, "read");
        let len_reg = self.b().new_tmp();
        self.b().emit_assign_imm(len_reg, bytes.len() as i64);
        let tag = self.new_format_tag();
        let out_name = self.b().symbol_name(out_string).to_string();
        let ptr_name = self.b().symbol_name(ptr_reg).to_string();
        let len_name = self.b().symbol_name(len_reg).to_string();
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
            &mut *self.flattener,
            "sa_std/string_format.sa",
            "FORMAT_PUSH_BYTES",
            &[tag, out_name, ptr_name, len_name],
        )?;
        self.emit_release(len_reg)?;
        self.emit_release(ptr_reg)?;
        Ok(())
    }

    /// Zig `emitFormatPushPrimitiveValue` (13854): format the
    /// value into a 64-byte stack scratch via the `sa_fmt_*_into`
    /// externs, then push the produced bytes.
    fn emit_format_push_primitive_value(
        &mut self,
        out_string: u32,
        value_reg: u32,
        suffix: &str,
    ) -> Result<()> {
        let buf = self.b().new_tmp();
        crate::release_prim::emit_stack_alloc(&mut self.state, buf, 64);
        let len_slot = self.b().new_tmp();
        crate::release_prim::emit_stack_alloc(&mut self.state, len_slot, 8);
        let value_name = self.b().symbol_name(value_reg).to_string();
        let buf_name = self.b().symbol_name(buf).to_string();
        let len_slot_name = self.b().symbol_name(len_slot).to_string();
        let ok = self.b().new_tmp();
        let body = match suffix {
            "I64" => format!("@sa_fmt_i64_into({value_name}, 10, {buf_name}, 64, &{len_slot_name})"),
            "U64" => format!("@sa_fmt_u64_into({value_name}, 10, {buf_name}, 64, &{len_slot_name})"),
            "F64" => format!("@sa_fmt_f64_into({value_name}, 6, {buf_name}, 64, &{len_slot_name})"),
            "BOOL" => format!("@sa_fmt_bool_into({value_name}, {buf_name}, 64, &{len_slot_name})"),
            _ => {
                return Err(unsupported(
                    "format argument with no debug-format suffix",
                    "D surface (format_print)",
                    "13854-13880",
                ))
            }
        };
        self.emit_call_body(Some(ok), body)?;
        let fmt_len = self.b().new_tmp();
        self.emit_load(fmt_len, len_slot, 0, PrimType::U64);
        let tag = self.new_format_tag();
        let out_name = self.b().symbol_name(out_string).to_string();
        let fmt_len_name = self.b().symbol_name(fmt_len).to_string();
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
            &mut *self.flattener,
            "sa_std/string_format.sa",
            "FORMAT_PUSH_BYTES",
            &[tag, out_name, buf_name, fmt_len_name],
        )?;
        self.emit_release(ok)?;
        self.emit_release(fmt_len)?;
        crate::release_meta::mark_non_owning_reg(&mut self.state, len_slot);
        self.emit_release(len_slot)?;
        crate::release_meta::mark_non_owning_reg(&mut self.state, buf);
        self.emit_release(buf)?;
        Ok(())
    }

    /// Zig `emitFormatPushStringLike` (13763).
    fn emit_format_push_string_like(
        &mut self,
        out_string: u32,
        arg: &Node,
        ty: &Type,
    ) -> Result<()> {
        let tag = self.new_format_tag();
        let out_name = self.b().symbol_name(out_string).to_string();
        if is_format_string_type(ty) {
            let owner_reg = self.gen_expr(arg)?;
            let slice_reg = self.b().new_tmp();
            let slice_name = self.b().symbol_name(slice_reg).to_string();
            let owner_name = self.b().symbol_name(owner_reg).to_string();
            FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
                &mut *self.flattener,
                "sa_std/string.sa",
                "STRING_BUF_AS_STR",
                &[slice_name.clone(), owner_name],
            )?;
            FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
                &mut *self.flattener,
                "sa_std/string_format.sa",
                "FORMAT_PUSH_SLICE",
                &[tag, out_name, slice_name],
            )?;
            self.emit_release(slice_reg)?;
            self.release_expr_result_if_needed(arg, owner_reg)?;
            return Ok(());
        }
        if let Node::Literal(Literal::StringVal(text)) = arg {
            let bytes = crate::literals::decode_string_literal_bytes(text.as_bytes())?;
            return self.emit_format_push_const_bytes(out_string, &bytes);
        }
        if matches!(ty, Type::Primitive(Primitive::RawPtr)) {
            let ptr_reg = self.gen_expr(arg)?;
            let len_reg = self.b().new_tmp();
            let len_name = self.b().symbol_name(len_reg).to_string();
            let ptr_name = self.b().symbol_name(ptr_reg).to_string();
            FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
                &mut *self.flattener,
                "sa_std/ffi.sa",
                "CSTR_LEN",
                &[len_name.clone(), ptr_name.clone()],
            )?;
            FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
                &mut *self.flattener,
                "sa_std/string_format.sa",
                "FORMAT_PUSH_BYTES",
                &[tag, out_name, ptr_name, len_name],
            )?;
            self.emit_release(len_reg)?;
            self.release_expr_result_if_needed(arg, ptr_reg)?;
            return Ok(());
        }
        let slice_reg = self.gen_expr(arg)?;
        let slice_name = self.b().symbol_name(slice_reg).to_string();
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
            &mut *self.flattener,
            "sa_std/string_format.sa",
            "FORMAT_PUSH_SLICE",
            &[tag, out_name, slice_name],
        )?;
        self.release_expr_result_if_needed(arg, slice_reg)?;
        Ok(())
    }

    /// Zig `emitFormatPushArg` (13800).
    fn emit_format_push_arg(&mut self, out_string: u32, arg: &Node) -> Result<()> {
        let ty = self
            .check
            .expr_type(arg)
            .cloned()
            .ok_or_else(|| Error::MissingType("format call argument".to_string()))?;
        if is_string_like_type(&ty) {
            return self.emit_format_push_string_like(out_string, arg, &ty);
        }
        let suffix = match &ty {
            Type::Primitive(p) => match p {
                Primitive::I8
                | Primitive::I16
                | Primitive::I32
                | Primitive::I64
                | Primitive::Isize => "I64",
                Primitive::U8
                | Primitive::U16
                | Primitive::U32
                | Primitive::U64
                | Primitive::Usize => "U64",
                Primitive::F32 | Primitive::F64 => "F64",
                Primitive::Boolean => "BOOL",
                _ => {
                    return Err(unsupported(
                        "format argument with no debug-format suffix",
                        "D surface (format_print)",
                        "13800-13810",
                    ))
                }
            },
            _ => {
                return Err(unsupported(
                    "format argument with no debug-format suffix",
                    "D surface (format_print)",
                    "13800-13810",
                ))
            }
        };
        let value_reg = self.gen_expr(arg)?;
        self.emit_format_push_primitive_value(out_string, value_reg, suffix)?;
        self.release_expr_result_if_needed(arg, value_reg)?;
        Ok(())
    }

    /// Zig `genFormatCall` (13812): build a String by walking
    /// the decoded format template, pushing literal runs and
    /// formatted arguments in order (`{{` / `}}` are escapes).
    pub(crate) fn gen_format_call(&mut self, call: &CallExpr) -> Result<u32> {
        let Some(Node::Literal(Literal::StringVal(text))) = call.args.first() else {
            return Err(unsupported(
                "format call without a string-literal template",
                "D surface (format_print)",
                "13812-13814",
            ));
        };
        let template = text.clone();
        self.ensure_debug_format_deps()?;
        let fmt = crate::literals::decode_string_literal_bytes(template.as_bytes())?;
        let out_string = self.b().new_tmp();
        let capacity = fmt.len() + call.args.len().saturating_sub(1) * 24;
        let out_name = self.b().symbol_name(out_string).to_string();
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment_with_literal_args(
            &mut *self.flattener,
            "sa_std/string_format.sa",
            "FORMAT_BEGIN",
            &[out_name, capacity.to_string()],
            &[false, true],
        )?;
        let mut literal: Vec<u8> = Vec::new();
        let mut arg_idx = 1usize;
        let mut i = 0usize;
        while i < fmt.len() {
            if fmt[i] == b'{' && i + 1 < fmt.len() {
                if fmt[i + 1] == b'{' {
                    literal.push(b'{');
                    i += 2;
                    continue;
                }
                if fmt[i + 1] == b'}' {
                    self.emit_format_push_const_bytes(out_string, &literal)?;
                    literal.clear();
                    if arg_idx >= call.args.len() {
                        return Err(unsupported(
                            "format template with more placeholders than arguments",
                            "D surface (format_print)",
                            "13840-13844",
                        ));
                    }
                    self.emit_format_push_arg(out_string, &call.args[arg_idx])?;
                    arg_idx += 1;
                    i += 2;
                    continue;
                }
            }
            if fmt[i] == b'}' && i + 1 < fmt.len() && fmt[i + 1] == b'}' {
                literal.push(b'}');
                i += 2;
                continue;
            }
            literal.push(fmt[i]);
            i += 1;
        }
        self.emit_format_push_const_bytes(out_string, &literal)?;
        if arg_idx != call.args.len() {
            return Err(unsupported(
                "format call with unused arguments",
                "D surface (format_print)",
                "13850-13852",
            ));
        }
        Ok(out_string)
    }
}


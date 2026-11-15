//! The `format(...)` builtin emitter, mirrored from
//! codegen.zig:11928 `genFormatCall` and its push helpers
//! (4574-4760). Split out of call.rs for the per-file line cap.

use sla_ast::{Literal, Node, Primitive, Type};

use crate::codegen::Codegen;
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// codegen.zig:11928 `genFormatCall` plus its push helpers
    /// (4574-4760): FORMAT_BEGIN with a capacity estimate, then
    /// per-segment FORMAT_PUSH_* expansions. `{{`/`}}` are brace
    /// escapes; a lone `{}` consumes the next argument.
    pub(crate) fn gen_format_call(&mut self, call: &sla_ast::CallExpr) -> Result<String> {
        let fmt = match call.args.first() {
            Some(Node::Literal(Literal::StringVal(s))) => s.clone(),
            _ => {
                return unsupported(
                    "format builtin with non-literal format string",
                    "codegen.zig:11928 (genFormatCall)",
                )
            }
        };
        let out_reg = self.new_tmp();
        let capacity =
            crate::codegen::escaped_string_byte_len(&fmt) + (call.args.len() - 1) * 24;
        self.ins(&format!("EXPAND FORMAT_BEGIN {out_reg}, {capacity}"));

        let bytes = fmt.as_bytes();
        let mut literal: Vec<u8> = Vec::new();
        let mut arg_idx = 1usize;
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'{' && i + 1 < bytes.len() {
                if bytes[i + 1] == b'{' {
                    literal.push(b'{');
                    i += 2;
                    continue;
                }
                if bytes[i + 1] == b'}' {
                    self.emit_format_push_const_bytes(&out_reg, &literal)?;
                    literal.clear();
                    if arg_idx >= call.args.len() {
                        return unsupported(
                            "format builtin: more holes than arguments",
                            "codegen.zig:11928 (genFormatCall)",
                        );
                    }
                    self.emit_format_push_arg(&out_reg, &call.args[arg_idx])?;
                    arg_idx += 1;
                    i += 2;
                    continue;
                }
            }
            if bytes[i] == b'}' && i + 1 < bytes.len() && bytes[i + 1] == b'}' {
                literal.push(b'}');
                i += 2;
                continue;
            }
            literal.push(bytes[i]);
            i += 1;
        }
        self.emit_format_push_const_bytes(&out_reg, &literal)?;
        if arg_idx != call.args.len() {
            return unsupported(
                "format builtin: fewer holes than arguments",
                "codegen.zig:11928 (genFormatCall)",
            );
        }
        Ok(out_reg)
    }

    /// codegen.zig:4574 `emitFormatPushConstBytes`.
    fn emit_format_push_const_bytes(&mut self, out_reg: &str, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(bytes);
        let label = self.new_string_const();
        self.ins(&format!("@const {label} = utf8:\"{text}\""));
        let ptr_reg = self.new_tmp();
        self.ins(&format!("{ptr_reg} = &{label}"));
        let tag = self.new_tmp();
        self.ins(&format!(
            "EXPAND FORMAT_PUSH_BYTES {tag}, {out_reg}, {ptr_reg}, {}",
            crate::codegen::escaped_string_byte_len(&text)
        ));
        self.emit_release(&ptr_reg)
    }

    /// codegen.zig:4672 `emitFormatPushArg`.
    fn emit_format_push_arg(&mut self, out_reg: &str, arg: &Node) -> Result<()> {
        let ty = self
            .expr_type(arg)
            .ok_or_else(|| crate::TextError::MissingType {
                context: "format argument type".to_string(),
            })?;
        if crate::codegen::is_string_like_type(&ty) {
            return self.emit_format_push_string_like(out_reg, arg, &ty);
        }
        let val_reg = self.gen_expr(arg)?;
        self.emit_format_push_value(out_reg, &val_reg, &ty)?;
        if sla_lowering::call_arg_needs_release(arg) {
            self.emit_release(&val_reg)?;
        }
        Ok(())
    }

    /// codegen.zig:4590 `emitFormatPushStringLike`.
    fn emit_format_push_string_like(
        &mut self,
        out_reg: &str,
        arg: &Node,
        ty: &Type,
    ) -> Result<()> {
        let tag = self.new_tmp();
        if crate::codegen::is_format_string_type(ty) {
            let string_reg = self.gen_expr(arg)?;
            let slice_reg = self.new_tmp();
            self.ins(&format!(
                "EXPAND STRING_BUF_AS_STR {slice_reg}, {string_reg}"
            ));
            self.ins(&format!(
                "EXPAND FORMAT_PUSH_SLICE {tag}, {out_reg}, {slice_reg}"
            ));
            self.emit_release(&slice_reg)?;
            if sla_lowering::call_arg_needs_release(arg) {
                self.emit_release(&string_reg)?;
            }
            return Ok(());
        }
        if let Node::Literal(Literal::StringVal(value)) = arg {
            let label = self.new_string_const();
            self.ins(&format!("@const {label} = utf8:\"{value}\""));
            let ptr_reg = self.new_tmp();
            self.ins(&format!("{ptr_reg} = &{label}"));
            self.ins(&format!(
                "EXPAND FORMAT_PUSH_BYTES {tag}, {out_reg}, {ptr_reg}, {}",
                crate::codegen::escaped_string_byte_len(value)
            ));
            self.emit_release(&ptr_reg)?;
            return Ok(());
        }
        if matches!(ty, Type::Primitive(Primitive::RawPtr)) {
            let ptr_reg = self.gen_expr(arg)?;
            let len_reg = self.new_tmp();
            self.ins(&format!("EXPAND CSTR_LEN {len_reg}, {ptr_reg}"));
            self.ins(&format!(
                "EXPAND FORMAT_PUSH_BYTES {tag}, {out_reg}, {ptr_reg}, {len_reg}"
            ));
            self.emit_release(&len_reg)?;
            if sla_lowering::call_arg_needs_release(arg) {
                self.emit_release(&ptr_reg)?;
            }
            return Ok(());
        }
        let slice_reg = self.gen_expr(arg)?;
        self.ins(&format!(
            "EXPAND FORMAT_PUSH_SLICE {tag}, {out_reg}, {slice_reg}"
        ));
        if sla_lowering::call_arg_needs_release(arg) {
            self.emit_release(&slice_reg)?;
        }
        Ok(())
    }

    /// codegen.zig:4747 `emitFormatPushValue` over
    /// `lowering_rules.debugFormatSuffix` (primitives only).
    fn emit_format_push_value(&mut self, out_reg: &str, val_reg: &str, ty: &Type) -> Result<()> {
        let suffix = match ty {
            Type::Primitive(p) => match p {
                Primitive::I8
                | Primitive::I16
                | Primitive::I32
                | Primitive::I64
                | Primitive::Isize
                | Primitive::Integer => "I64",
                Primitive::U8
                | Primitive::U16
                | Primitive::U32
                | Primitive::U64
                | Primitive::Usize => "U64",
                Primitive::F32 | Primitive::F64 | Primitive::Float => "F64",
                Primitive::Boolean => "BOOL",
                _ => {
                    return unsupported(
                        "format argument primitive (no debug suffix)",
                        "lowering_rules.zig:6690 (debugFormatSuffix)",
                    )
                }
            },
            _ => {
                return unsupported(
                    "format argument type (no debug suffix)",
                    "lowering_rules.zig:6690 (debugFormatSuffix)",
                )
            }
        };
        let tag = self.new_tmp();
        match suffix {
            "U64" => self.ins(&format!(
                "EXPAND FORMAT_PUSH_U64 {tag}, {out_reg}, {val_reg}"
            )),
            "F64" => self.ins(&format!(
                "EXPAND FORMAT_PUSH_F64 {tag}, {out_reg}, {val_reg}, 10"
            )),
            "BOOL" => self.ins(&format!(
                "EXPAND FORMAT_PUSH_BOOL {tag}, {out_reg}, {val_reg}"
            )),
            _ => self.ins(&format!(
                "EXPAND FORMAT_PUSH_I64 {tag}, {out_reg}, {val_reg}"
            )),
        }
        Ok(())
    }

}

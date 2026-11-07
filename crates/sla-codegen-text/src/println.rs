//! `println` emission, mirrored from codegen.zig's `emitPrintln`
//! (2127-2450) and `emitPrintlnNewline`. Behaviors preserved
//! exactly, including two suspicious ones (logged in the divergences
//! file): literal segments print with their RAW span length while
//! string-literal arguments print with the escaped byte length, and
//! an argument of unrecognized type falls back to printing exactly
//! one byte.

use sla_ast::{CallExpr, Literal, Node, Primitive, Type};
use sla_lowering::call_arg_needs_release;

use crate::codegen::{escaped_string_byte_len, Codegen};
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// codegen.zig:2127 `emitPrintln`.
    pub(crate) fn emit_println(&mut self, call: &CallExpr) -> Result<()> {
        if call.args.is_empty() {
            return self.emit_println_newline();
        }
        let fmt_text: &str = match &call.args[0] {
            Node::Literal(Literal::StringVal(text)) => text,
            _ => {
                if call.args.len() == 1 {
                    if let Some(ty) = self.expr_type(&call.args[0]) {
                        if crate::codegen::is_format_string_type(&ty)
                            || crate::codegen::is_string_like_type(&ty)
                        {
                            return unsupported(
                                "println of format-string/string value",
                                "codegen.zig:2140 (emitPrintln value paths)",
                            );
                        }
                    }
                }
                return self.emit_println_newline();
            }
        };
        let fmt_text = fmt_text.to_string();
        let bytes = fmt_text.as_bytes();
        let mut arg_index = 1usize;
        let mut start = 0usize;
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'}' {
                if i > start {
                    self.emit_print_segment(&fmt_text[start..i], i - start)?;
                }
                if arg_index < call.args.len() {
                    let arg = call.args[arg_index].clone();
                    self.emit_print_arg(&arg)?;
                    arg_index += 1;
                }
                i += 2;
                start = i;
            } else if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                i += 2;
            } else {
                i += 1;
            }
        }
        if bytes.len() > start {
            self.emit_print_segment(&fmt_text[start..], bytes.len() - start)?;
        }
        self.emit_println_newline()
    }

    /// Literal segment: the printed length is the raw span length.
    fn emit_print_segment(&mut self, segment: &str, raw_len: usize) -> Result<()> {
        let label = self.new_string_const();
        self.ins(&format!("@const {label} = utf8:\"{segment}\""));
        self.ins(&format!("call @sa_print_bytes(&{label}, {raw_len})"));
        Ok(())
    }

    /// One `{}` hole argument.
    fn emit_print_arg(&mut self, arg: &Node) -> Result<()> {
        if let Node::Literal(Literal::StringVal(text)) = arg {
            let text = text.clone();
            let label = self.new_string_const();
            self.ins(&format!("@const {label} = utf8:\"{text}\""));
            let len = escaped_string_byte_len(&text);
            self.ins(&format!("call @sa_print_bytes(&{label}, {len})"));
            return Ok(());
        }
        let arg_ty = self.expr_type(arg);
        match &arg_ty {
            Some(Type::Primitive(prim)) => {
                let fmt_fn = match prim {
                    Primitive::F32 | Primitive::F64 | Primitive::Float => "@sa_fmt_f64",
                    Primitive::Boolean => "@sa_fmt_bool",
                    _ => "@sa_fmt_u64",
                };
                let val = self.gen_expr(arg)?;
                let fmt_buf = self.new_tmp();
                let data = self.new_tmp();
                let len = self.new_tmp();
                if fmt_fn == "@sa_fmt_u64" {
                    self.ins(&format!("{fmt_buf} = call @sa_fmt_u64({val}, 10)"));
                } else {
                    self.ins(&format!("{fmt_buf} = call {fmt_fn}({val})"));
                }
                self.ins(&format!("{data} = call @sa_fmt_buffer_data({fmt_buf})"));
                self.ins(&format!("{len} = call @sa_fmt_buffer_len({fmt_buf})"));
                let ptr = self.new_tmp();
                self.ins(&format!("{ptr} = {data}"));
                self.ins(&format!("call @sa_print_bytes(&{ptr}, {len})"));
                self.ins(&format!("call @sa_fmt_buffer_free(^{fmt_buf})"));
                self.emit_release(&ptr)?;
                self.emit_release(&len)?;
                if call_arg_needs_release(arg) {
                    self.emit_release(&val)?;
                }
                Ok(())
            }
            Some(ty) if crate::codegen::is_format_string_type(ty) => unsupported(
                "println of String argument",
                "codegen.zig:2250 (format-string arg path)",
            ),
            Some(ty) if crate::codegen::is_string_like_type(ty) => unsupported(
                "println of string-like argument",
                "codegen.zig:2300 (string-like arg path)",
            ),
            Some(Type::Borrow(_)) => unsupported(
                "println of borrowed argument",
                "codegen.zig:2350 (borrow arg path)",
            ),
            _ => {
                // Fallback (mirrored): print exactly one byte.
                let val = self.gen_expr(arg)?;
                let ptr = self.new_tmp();
                self.ins(&format!("{ptr} = {val}"));
                self.ins(&format!("call @sa_print_bytes(&{ptr}, 1)"));
                Ok(())
            }
        }
    }

    /// codegen.zig `emitPrintlnNewline`.
    pub(crate) fn emit_println_newline(&mut self) -> Result<()> {
        let label = self.new_string_const();
        self.ins(&format!("@const {label} = utf8:\"\\n\""));
        self.ins(&format!("call @sa_print_bytes(&{label}, 1)"));
        Ok(())
    }
}

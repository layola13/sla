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
                // codegen.zig:2128 (#4b): a single string-like /
                // format-string argument (e.g. the
                // `println(format(...))` shape templates desugar
                // to) prints its value instead of a bare newline;
                // every other arity keeps the historical behavior.
                if call.args.len() == 1 {
                    if let Some(ty) = self.expr_type(&call.args[0]) {
                        if crate::codegen::is_format_string_type(&ty) {
                            self.emit_print_string_value(&call.args[0], true)?;
                            return self.emit_println_newline();
                        }
                        if crate::codegen::is_string_like_type(&ty) {
                            self.emit_print_string_value(&call.args[0], false)?;
                            return self.emit_println_newline();
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
                    // No clone: checker facts are keyed by node
                    // address, so the argument must stay the exact
                    // node from the program tree.
                    self.emit_print_arg(&call.args[arg_index])?;
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
            Some(ty) if crate::codegen::is_format_string_type(ty) => {
                self.emit_print_string_value(arg, true)
            }
            Some(ty) if crate::codegen::is_string_like_type(ty) => {
                self.emit_print_string_value(arg, false)
            }
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

    /// codegen.zig:2140/2250 string-value print sequence: the
    /// format-string form first views the String buffer as a
    /// string (`STRING_BUF_AS_STR`); both forms then print the
    /// value's bytes through `STRING_PTR` / `STRING_LEN` and
    /// `@sa_print_bytes`. No trailing newline here — the caller
    /// owns it.
    fn emit_print_string_value(&mut self, arg: &Node, format_string: bool) -> Result<()> {
        let value_reg = self.gen_expr(arg)?;
        let slice_reg = if format_string {
            let slice_reg = self.new_tmp();
            self.ins(&format!(
                "EXPAND STRING_BUF_AS_STR {slice_reg}, {value_reg}"
            ));
            slice_reg
        } else {
            value_reg.clone()
        };
        let ptr_reg = self.new_tmp();
        let len_reg = self.new_tmp();
        self.ins(&format!("EXPAND STRING_PTR {ptr_reg}, {slice_reg}"));
        self.ins(&format!("EXPAND STRING_LEN {len_reg}, {slice_reg}"));
        self.ins(&format!("call @sa_print_bytes(&{ptr_reg}, {len_reg})"));
        self.emit_release(&ptr_reg)?;
        self.emit_release(&len_reg)?;
        if format_string {
            self.emit_release(&slice_reg)?;
        }
        if call_arg_needs_release(arg) {
            self.emit_release(&value_reg)?;
        }
        Ok(())
    }

    /// codegen.zig `emitPrintlnNewline`.
    pub(crate) fn emit_println_newline(&mut self) -> Result<()> {
        let label = self.new_string_const();
        self.ins(&format!("@const {label} = utf8:\"\\n\""));
        self.ins(&format!("call @sa_print_bytes(&{label}, 1)"));
        Ok(())
    }
}

//! The emission trunk: a `ModuleBuilder` that mirrors the Zig
//! `Codegen`'s interning and primitive-emitter conventions, producing
//! the `Module` the encoder serializes.
//!
//! Conventions ported verbatim from sab_codegen.zig:
//! - one interning namespace for register names, function names,
//!   labels and const names (`Codegen.intern`, lines 1046-1053);
//! - temp names `tmp_{n}`, labels `{prefix}_{n}`, string consts
//!   `SLA_STR_{n}` (`newTmp` / `newLabel` / `newStringConst`);
//! - `makeInst`: `source_line = 0`, `expanded_line` = index the
//!   instruction will occupy (lines 1103-1106);
//! - decl instructions: operands `[Symbol(id), Func(id)]` where `id`
//!   is the interned function name, kind chosen by sig kind
//!   (`declInstForSig`, lines 6007-6021);
//! - string consts carry one trailing NUL byte and the literal text
//!   `utf8:"<escaped>"` (`appendUtf8Const`, lines 1229-1258).
//!
//! Only the trunk is ported in this slice. Statement/expression
//! generation (the `gen*` families in the decomposition map) lands in
//! later phases; anything not modeled here fails loudly through
//! `Error::UnsupportedSabDirectFeature`, never silently.

use std::collections::HashMap;

use crate::error::Result;
use crate::ir::*;
use crate::literals::{decode_string_literal_bytes, escape_utf8_literal_bytes};

/// Accumulates symbols, sigs, const decls and instructions in Zig
/// emission order.
#[derive(Debug, Default)]
pub struct ModuleBuilder {
    symbols: Vec<String>,
    symbol_ids: HashMap<String, u32>,
    function_sigs: Vec<FunctionSig>,
    const_decls: Vec<ConstDecl>,
    instructions: Vec<Instruction>,
    tmp_idx: u32,
    label_idx: u32,
    string_idx: u32,
    string_literal_consts: HashMap<String, String>,
    /// Regs recorded for the function currently being built
    /// (Zig `recordReg` / `finishFunctionRegs`).
    recorded_regs: Vec<u32>,
    current_sig: Option<usize>,
}

impl ModuleBuilder {
    pub fn new() -> Self {
        ModuleBuilder::default()
    }

    /// Intern a name in the shared namespace, returning its id
    /// (Zig `intern`).
    pub fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.symbol_ids.get(name) {
            return id;
        }
        let id = self.symbols.len() as u32;
        self.symbols.push(name.to_string());
        self.symbol_ids.insert(name.to_string(), id);
        id
    }

    /// Allocate a fresh temp register name and intern it
    /// (Zig `newTmp`).
    pub fn new_tmp(&mut self) -> u32 {
        let name = format!("tmp_{}", self.tmp_idx);
        self.tmp_idx += 1;
        self.intern(&name)
    }

    /// Allocate a fresh label name (Zig `newLabel`); the name is
    /// interned when the label is emitted, as in Zig.
    pub fn new_label(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.label_idx);
        self.label_idx += 1;
        name
    }

    fn make_inst(&self, kind: InstKind) -> Instruction {
        Instruction::new(kind, 0, self.instructions.len() as u32)
    }

    fn append_inst(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }

    /// Register a function sig and return its index. The sig's `id`
    /// is the interned name id, matching `declInstForSig`'s use of
    /// `intern(fsig.name)` for both the sig and its decl instruction.
    pub fn add_sig(&mut self, mut sig: FunctionSig) -> usize {
        sig.id = self.intern(&sig.name);
        sig.entry_inst_idx = self.instructions.len() as u32;
        self.function_sigs.push(sig);
        self.function_sigs.len() - 1
    }

    /// Begin emitting a function body: subsequent `record_reg` calls
    /// accumulate into this sig's `reg_ids` at `finish_function`.
    pub fn begin_function(&mut self, sig_index: usize) {
        self.current_sig = Some(sig_index);
        self.recorded_regs.clear();
    }

    /// Record a register as belonging to the current function
    /// (Zig `recordReg`).
    pub fn record_reg(&mut self, reg: u32) {
        if !self.recorded_regs.contains(&reg) {
            self.recorded_regs.push(reg);
        }
    }

    /// Close the current function, writing its recorded reg ids into
    /// the sig (Zig `finishFunctionRegs`).
    pub fn finish_function(&mut self) {
        if let Some(idx) = self.current_sig.take() {
            self.function_sigs[idx].reg_ids = self.recorded_regs.clone();
        }
        self.recorded_regs.clear();
    }

    /// Emit the decl instruction for a sig (Zig `appendDeclInst`).
    pub fn append_decl_inst(&mut self, sig_index: usize) {
        let sig = &self.function_sigs[sig_index];
        let kind = match sig.kind {
            FunctionKind::Normal => InstKind::FuncDecl,
            FunctionKind::FfiWrapper => InstKind::FfiWrapperDecl,
            FunctionKind::External => InstKind::ExternDecl,
            FunctionKind::Exported => InstKind::ExportDecl,
            FunctionKind::TestFunc => InstKind::TestDecl,
            // Current-head sci pairs the gpu sig kind with the
            // gpu_decl instruction kind (both appended last in
            // their enums); the plugin-side mapping in
            // declInstForSig is to be re-verified against the
            // current-head plugin tree in phase 2.
            FunctionKind::Gpu => InstKind::GpuDecl,
        };
        let id = sig.id;
        let mut inst = Instruction::new(kind, 0, self.instructions.len() as u32);
        inst.operands[0] = Operand::Symbol(id);
        inst.operands[1] = Operand::Func(id);
        self.append_inst(inst);
    }

    /// Append a utf8 const decl for `bytes`, with the trailing NUL
    /// the Zig codegen adds (Zig `appendUtf8Const`). Returns the
    /// const's label (`SLA_STR_{n}`).
    pub fn append_utf8_const(&mut self, bytes: &[u8]) -> String {
        let name = format!("SLA_STR_{}", self.string_idx);
        self.string_idx += 1;
        let mut term_bytes = bytes.to_vec();
        term_bytes.push(0);
        let literal_text = format!("utf8:\"{}\"", escape_utf8_literal_bytes(&term_bytes));
        self.const_decls.push(ConstDecl {
            source_line: 0,
            expanded_line: 0,
            upstream_loc: None,
            name: name.clone(),
            literal_text,
            value: ConstValue::Utf8(BytesLiteral {
                bytes: term_bytes,
                repeat: None,
            }),
        });
        self.intern(&name);
        name
    }

    /// Const label for a source string literal, deduplicated by the
    /// literal's source text (Zig `stringLiteralConstLabel`).
    /// `literal_source` is the text between the quotes, escapes intact.
    pub fn string_literal_const_label(&mut self, literal_source: &str) -> Result<String> {
        if let Some(label) = self.string_literal_consts.get(literal_source) {
            return Ok(label.clone());
        }
        let bytes = decode_string_literal_bytes(literal_source.as_bytes())?;
        let label = self.append_utf8_const(&bytes);
        self.string_literal_consts
            .insert(literal_source.to_string(), label.clone());
        Ok(label)
    }

    /// `label` instruction: operands `[Symbol(id), Label(id)]`
    /// (Zig `emitLabel`).
    pub fn emit_label(&mut self, name: &str) {
        let id = self.intern(name);
        let mut inst = self.make_inst(InstKind::Label);
        inst.operands[0] = Operand::Symbol(id);
        inst.operands[1] = Operand::Label(id);
        self.append_inst(inst);
    }

    /// Unconditional jump: operands `[Symbol(id), Label(id)]`
    /// (Zig `emitJmp`).
    pub fn emit_jmp(&mut self, label: &str) {
        let id = self.intern(label);
        let mut inst = self.make_inst(InstKind::Jmp);
        inst.operands[0] = Operand::Symbol(id);
        inst.operands[1] = Operand::Label(id);
        self.append_inst(inst);
    }

    /// Conditional branch (Zig `emitBranch`, lines 12537-12545).
    ///
    /// DIVERGENCE SAB-D13: the Zig original writes `then_label` into
    /// BOTH operands[1] and operands[2], and `else_label` into
    /// operands[3]. Every sci consumer reads only operands[1] (true)
    /// and operands[3] (false), so the duplicated slot is dead data
    /// today — but it is what the Zig encoder emits, so this port
    /// reproduces it byte-for-byte and registers the suspicion
    /// instead of silently "fixing" it.
    pub fn emit_branch(&mut self, cond: u32, then_label: &str, else_label: &str) {
        let then_id = self.intern(then_label);
        let else_id = self.intern(else_label);
        let mut inst = self.make_inst(InstKind::Br);
        inst.operands[0] = Operand::Reg(cond);
        inst.operands[1] = Operand::Label(then_id);
        inst.operands[2] = Operand::Label(then_id); // sic: mirrors Zig
        inst.operands[3] = Operand::Label(else_id);
        self.append_inst(inst);
    }

    /// Return, optionally with a value register (Zig `emitReturn`).
    pub fn emit_return(&mut self, value: Option<u32>) {
        let mut inst = self.make_inst(InstKind::Return);
        if let Some(reg) = value {
            inst.operands[0] = Operand::Reg(reg);
        }
        self.append_inst(inst);
    }

    /// `dst = imm` with a signed immediate (Zig `emitAssignImm`).
    pub fn emit_assign_imm(&mut self, dst: u32, value: i64) {
        self.record_reg(dst);
        let mut inst = self.make_inst(InstKind::Assign);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::ImmI64(value);
        self.append_inst(inst);
    }

    /// `dst = imm` choosing the unsigned encoding when the value's
    /// type is unsigned or the value needs it (Zig `emitAssignInt`:
    /// `imm_u64` iff `value >= 0 && (unsigned type || value >= 2^62)`,
    /// else `imm_i64`). `unsigned_type` stands in for the AST type
    /// query until sla-lowering feeds real types.
    pub fn emit_assign_int(&mut self, dst: u32, value: i64, unsigned_type: bool) {
        self.record_reg(dst);
        let mut inst = self.make_inst(InstKind::Assign);
        inst.operands[0] = Operand::Reg(dst);
        let needs_unsigned_leb = value >= (1i64 << 62);
        inst.operands[1] = if value >= 0 && (unsigned_type || needs_unsigned_leb) {
            Operand::ImmU64(value as u64)
        } else {
            Operand::ImmI64(value)
        };
        self.append_inst(inst);
    }

    /// `dst = float` (Zig `emitAssignFloat`).
    pub fn emit_assign_float(&mut self, dst: u32, value: f64) {
        self.record_reg(dst);
        let mut inst = self.make_inst(InstKind::Assign);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::ImmFloat(value);
        self.append_inst(inst);
    }

    /// `panic(<code>)`: operands `[Text("<code>")]`
    /// (Zig `emitPanicCode`).
    pub fn emit_panic_code(&mut self, code: i64) {
        let mut inst = self.make_inst(InstKind::Panic);
        inst.operands[0] = Operand::Text(code.to_string());
        self.append_inst(inst);
    }

    /// Binary/unary op: operands `[Reg(dst), lhs, rhs]` with
    /// `op_kind` set (Zig `emitOp`).
    pub fn emit_op(&mut self, dst: u32, op: OpKind, lhs: Operand, rhs: Operand) {
        self.record_reg(dst);
        let mut inst = self.make_inst(InstKind::Op);
        inst.op_kind = Some(op);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = lhs;
        inst.operands[2] = rhs;
        self.append_inst(inst);
    }

    /// A call in the codegen's text-body form: operands
    /// `[Reg(dst) | None, Text("@callee(args)")]` — the shape
    /// `emitCallBody` and the whole `gen*Call` family produce, where
    /// the argument list is rendered into the body text.
    pub fn emit_call(&mut self, dst: Option<u32>, body: &str) {
        if let Some(reg) = dst {
            self.record_reg(reg);
        }
        let mut inst = self.make_inst(InstKind::Call);
        if let Some(reg) = dst {
            inst.operands[0] = Operand::Reg(reg);
        }
        inst.operands[1] = Operand::Text(body.to_string());
        self.append_inst(inst);
    }

    /// Consume the builder into the finished module.
    pub fn finish(mut self) -> Module {
        self.finish_function();
        Module {
            symbols: self.symbols,
            function_sigs: self.function_sigs,
            const_decls: self.const_decls,
            instructions: self.instructions,
        }
    }
}

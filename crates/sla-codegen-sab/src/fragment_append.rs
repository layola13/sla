//! F group (append half) — cloning and appending decoded
//! modules into the host module: `cloneModuleFunctionSig`,
//! `cloneDecodedModuleFunctionSig`, `cloneModuleInstruction`,
//! `cloneDecodedModuleInstruction`,
//! `appendDecodedModuleFiltered`, `appendDecodedModuleConstDecls`
//! and `appendDecodedFunctionBody` of `sab_codegen.zig`
//! (current-head lines 5560–6150), plus the operand coercion and
//! panic-msg structuring they depend on.
//!
//! Clone conventions mirrored from Zig:
//! - a cloned sig's `id` is the host's *function-sig count* at
//!   clone time (main list only — the SAB-D17 boundary paths
//!   note: this family does not use the genFuncSig counter), and
//!   its `entry_inst_idx` is the host instruction count the
//!   caller passes in;
//! - cloned instructions lose their raw text (Zig sets
//!   `raw_text = ""`; raw text is unmodeled here, SAB-D29);
//! - `panic_msg` instructions whose first operand is a
//!   parenthesized `(code, msg, len)` text are restructured
//!   into three native-text operands
//!   (`structuredPanicMsgOperands`).

use std::collections::HashSet;

use crate::error::Result;
use crate::fragment::{
    call_instruction_body, call_target_name, clone_decoded_module_reg_ids,
    decoded_module_symbol_id_by_name, direct_call_target_name, unsupported_f,
    FragmentMachine,
};
use crate::ir::*;
use crate::types::DecodedModuleLocalRemap;

/// Zig `isStdMacroTemplateIntegerArg` (lowering_rules.zig
/// 4141–4153): optional minus, then at least one digit, digits
/// only.
pub fn is_std_macro_template_integer_arg(arg: &str) -> bool {
    if arg.is_empty() {
        return false;
    }
    let bytes = arg.as_bytes();
    let start = if bytes[0] == b'-' {
        if arg.len() == 1 {
            return false;
        }
        1
    } else {
        0
    };
    arg[start..].bytes().all(|ch| ch.is_ascii_digit())
}

/// Zig `stdMacroTemplateIntegerOperand` (sab_codegen.zig line
/// 5358): parse the arg as i64 — Zig raises a parse error for
/// out-of-range text, surfaced here as the F-group unsupported
/// error (the integer-arg shape check normally precedes it).
pub fn std_macro_template_integer_operand(arg: &str) -> Result<Operand> {
    arg.parse::<i64>()
        .map(Operand::ImmI64)
        .map_err(|_| unsupported_f("std macro template integer arg does not parse", "5358"))
}

/// Zig `structuredPanicMsgOperands`: `(code, msg, len)` — three
/// comma-separated parts inside parentheses, no fourth part —
/// becomes three native-text operands; anything else is `None`
/// (the instruction keeps its text operand).
pub fn structured_panic_msg_operands(text: &str) -> Option<[Operand; 3]> {
    let trimmed = text.trim_matches([' ', '\t', '\r', '\n']);
    if trimmed.len() < 2 || !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut parts = inner.split(',');
    let code_text = parts.next()?.trim_matches([' ', '\t', '\r', '\n']);
    let msg_text = parts.next()?.trim_matches([' ', '\t', '\r', '\n']);
    let len_text = parts.next()?.trim_matches([' ', '\t', '\r', '\n']);
    if parts.next().is_some() {
        return None;
    }
    Some([
        Operand::NativeText(code_text.to_string()),
        Operand::NativeText(msg_text.to_string()),
        Operand::NativeText(len_text.to_string()),
    ])
}

impl<'a> FragmentMachine<'a> {
    /// Zig `cloneModuleFunctionSig` (plain path): ids remapped
    /// through `remapModuleSymbol`; `id`/`entry_inst_idx` are
    /// set by the caller-side convention in
    /// [`Self::push_cloned_sig`].
    pub fn clone_module_function_sig(
        &mut self,
        symbols: &[String],
        source: &FunctionSig,
    ) -> Result<FunctionSig> {
        Ok(FunctionSig {
            id: 0,
            name: source.name.clone(),
            params: source.params.clone(),
            kind: source.kind,
            return_cap: source.return_cap,
            return_ty: source.return_ty,
            return_fallible: source.return_fallible,
            entry_inst_idx: 0,
            is_ffi_wrapper: source.is_ffi_wrapper,
            upstream_file: source.upstream_file.clone(),
            upstream_loc: source.upstream_loc.clone(),
            param_ids: self.remap_module_ids(symbols, &source.param_ids)?,
            reg_ids: self.remap_module_ids(symbols, &source.reg_ids)?,
            llvm_name: source.llvm_name.clone(),
            ignored: source.ignored,
            should_panic: source.should_panic,
            is_variadic: source.is_variadic,
        })
    }

    /// Zig `cloneDecodedModuleFunctionSig`: ids remapped through
    /// the local remap; the sig's register list is the required
    /// ids (params + source regs) closed over the remap's
    /// collection order (`cloneDecodedModuleRegIds`).
    pub fn clone_decoded_module_function_sig(
        &mut self,
        symbols: &[String],
        source: &FunctionSig,
        remap: &mut DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
    ) -> Result<FunctionSig> {
        let param_ids =
            self.remap_decoded_module_ids(symbols, &source.param_ids, remap, stable_names)?;
        let source_reg_ids =
            self.remap_decoded_module_ids(symbols, &source.reg_ids, remap, stable_names)?;
        let mut required_reg_ids = param_ids.clone();
        required_reg_ids.extend_from_slice(&source_reg_ids);
        Ok(FunctionSig {
            id: 0,
            name: source.name.clone(),
            params: source.params.clone(),
            kind: source.kind,
            return_cap: source.return_cap,
            return_ty: source.return_ty,
            return_fallible: source.return_fallible,
            entry_inst_idx: 0,
            is_ffi_wrapper: source.is_ffi_wrapper,
            upstream_file: source.upstream_file.clone(),
            upstream_loc: source.upstream_loc.clone(),
            param_ids,
            reg_ids: clone_decoded_module_reg_ids(&required_reg_ids, remap),
            llvm_name: source.llvm_name.clone(),
            ignored: source.ignored,
            should_panic: source.should_panic,
            is_variadic: source.is_variadic,
        })
    }

    /// Append a cloned sig to the host module, applying the
    /// clone family's id/entry convention: `id` = current
    /// function-sig count, `entry_inst_idx` = current
    /// instruction count. Returns the sig index.
    pub fn push_cloned_sig(&mut self, mut sig: FunctionSig) -> usize {
        sig.id = self.b().function_sigs_len() as u32;
        sig.entry_inst_idx = self.b().instructions_len() as u32;
        self.b().push_sig_verbatim(sig)
    }

    /// Zig `cloneModuleInstruction` (plain path).
    pub fn clone_module_instruction(
        &mut self,
        symbols: &[String],
        source: &Instruction,
    ) -> Result<Instruction> {
        let mut out = source.clone();
        for slot in out.operands.iter_mut() {
            let remapped = self.remap_module_operand(symbols, slot)?;
            *slot = remapped;
        }
        if out.kind == InstKind::PanicMsg {
            if let Operand::Text(text) = &out.operands[0] {
                if let Some(ops) = structured_panic_msg_operands(text) {
                    out.operands[0] = ops[0].clone();
                    out.operands[1] = ops[1].clone();
                    out.operands[2] = ops[2].clone();
                }
            }
        }
        Ok(out)
    }

    /// Zig `cloneDecodedModuleInstruction`: decoded remap on
    /// every operand, atomic texts and native names renamed,
    /// then the value-operand coercion.
    pub fn clone_decoded_module_instruction(
        &mut self,
        symbols: &[String],
        source: &Instruction,
        remap: &mut DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
    ) -> Result<Instruction> {
        let mut out = source.clone();
        out.atomic_expected_text = source
            .atomic_expected_text
            .as_ref()
            .map(|text| self.rename_decoded_module_local_text(symbols, text, remap));
        out.atomic_new_text = source
            .atomic_new_text
            .as_ref()
            .map(|text| self.rename_decoded_module_local_text(symbols, text, remap));
        out.native_reg_names = source
            .native_reg_names
            .iter()
            .map(|name| self.rename_decoded_module_local_text(symbols, name, remap))
            .collect();
        for (operand_idx, slot) in out.operands.iter_mut().enumerate() {
            let remapped = self.remap_decoded_module_operand(
                symbols,
                slot,
                source.kind,
                operand_idx,
                remap,
                stable_names,
            )?;
            *slot = remapped;
        }
        self.coerce_decoded_instruction_operands(&mut out, remap, stable_names);
        if out.kind == InstKind::PanicMsg {
            if let Operand::Text(text) = &out.operands[0] {
                if let Some(ops) = structured_panic_msg_operands(text) {
                    out.operands[0] = ops[0].clone();
                    out.operands[1] = ops[1].clone();
                    out.operands[2] = ops[2].clone();
                }
            }
        }
        Ok(out)
    }

    /// Zig `coerceDecodedValueOperand`: a text value operand
    /// that is an integer literal becomes an immediate; a stable
    /// name stays text (it resolves by name downstream); a
    /// remapped local name becomes the register operand.
    pub fn coerce_decoded_value_operand(
        &self,
        operand: &mut Operand,
        remap: &DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
    ) {
        let text = match operand {
            Operand::Text(text) => text.trim_matches([' ', '\t', '\r', '\n']),
            _ => return,
        };
        if is_std_macro_template_integer_arg(text) {
            if let Ok(coerced) = std_macro_template_integer_operand(text) {
                *operand = coerced;
            }
            return;
        }
        if stable_names.contains(text) {
            return;
        }
        if let Some(&reg) = remap.reg_name_ids.get(text) {
            *operand = Operand::Reg(reg);
        }
    }

    /// Zig `coerceDecodedInstructionOperands`: the value slots
    /// of store / assign / op / ptr_add / borrow / release.
    pub fn coerce_decoded_instruction_operands(
        &self,
        item: &mut Instruction,
        remap: &DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
    ) {
        match item.kind {
            InstKind::Store => {
                self.coerce_decoded_value_operand(&mut item.operands[2], remap, stable_names)
            }
            InstKind::Assign => {
                self.coerce_decoded_value_operand(&mut item.operands[1], remap, stable_names)
            }
            InstKind::Op | InstKind::PtrAdd => {
                self.coerce_decoded_value_operand(&mut item.operands[1], remap, stable_names);
                self.coerce_decoded_value_operand(&mut item.operands[2], remap, stable_names);
            }
            InstKind::Borrow => {
                self.coerce_decoded_value_operand(&mut item.operands[1], remap, stable_names)
            }
            InstKind::Release => {
                self.coerce_decoded_value_operand(&mut item.operands[0], remap, stable_names)
            }
            _ => {}
        }
    }

    /// Zig `recordInstructionRegs` + `recordCallBodyRegs`:
    /// register operands are recorded into the current function;
    /// a call body additionally records its direct callee and
    /// every already-seen register the body text names.
    pub fn record_instruction_regs(&mut self, item: &Instruction) {
        for operand in &item.operands {
            if let Operand::Reg(reg) = operand {
                self.b().record_reg(*reg);
            }
        }
        if let Some(body) = call_instruction_body(item) {
            let body = body.to_string();
            if let Some(callee) = direct_call_target_name(&body) {
                let callee = callee.to_string();
                let reg = crate::symbols::intern_stable(self.b(), &callee);
                self.b().record_reg(reg);
            }
            let mut i = 0;
            let bytes = body.as_bytes();
            while i < bytes.len() {
                while i < bytes.len() && !(bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                if start == i {
                    continue;
                }
                let name = &body[start..i];
                if let Some(reg) = self.b().symbol_id(name) {
                    if self.b().reg_seen(reg) {
                        self.b().record_reg(reg);
                    }
                }
            }
        }
    }

    /// Zig `ensureConstDeclCompatible`: a same-name const whose
    /// literal differs is `ConflictingConstDecl`. Zig compares
    /// `raw_text` as well; for decoded decls the raw text is the
    /// synthesized `@const {name} = {literal_text}`, so literal
    /// equality decides it (SAB-D30).
    pub fn ensure_const_decl_compatible(&mut self, decl: &ConstDecl) -> Result<()> {
        for existing in self.b().const_decls() {
            if existing.name != decl.name {
                continue;
            }
            if existing.literal_text == decl.literal_text {
                return Ok(());
            }
            return Err(crate::error::Error::ConflictingConstDecl(decl.name.clone()));
        }
        Ok(())
    }

    /// Zig `hasConstDecl`.
    pub fn has_const_decl(&mut self, name: &str) -> bool {
        self.b().const_decls().iter().any(|decl| decl.name == name)
    }

    /// Zig `appendDecodedModuleConstDecls`: intern *every*
    /// source symbol first, then append the consts (first wins;
    /// no compatibility check on this path — Zig's comment
    /// notes per-fragment template expansion re-appends the same
    /// std consts repeatedly). Cloned consts are re-based to
    /// source/expanded line 0.
    pub fn append_decoded_module_const_decls(&mut self, module: &Module) -> Result<()> {
        for name in &module.symbols {
            let name = name.clone();
            self.b().intern(&name);
        }
        for decl in &module.const_decls {
            let const_id = crate::symbols::intern_stable(self.b(), &decl.name.clone());
            self.b().record_reg(const_id);
            if self.has_const_decl(&decl.name) {
                continue;
            }
            let mut cloned = decl.clone();
            cloned.source_line = 0;
            cloned.expanded_line = 0;
            self.b().append_const_decl(cloned);
        }
        Ok(())
    }

    /// Zig `collectDecodedModuleDepClosure`: with no deps,
    /// every function in the module is selected; otherwise the
    /// seed deps plus the transitive closure of module-internal
    /// call targets reachable from selected non-extern bodies.
    pub fn collect_decoded_module_dep_closure(
        module: &Module,
        deps: &[String],
        selected: &mut HashSet<String>,
    ) {
        if deps.is_empty() {
            for fsig in &module.function_sigs {
                selected.insert(fsig.name.clone());
            }
            return;
        }
        for dep in deps {
            selected.insert(dep.clone());
        }
        let mut changed = true;
        while changed {
            changed = false;
            for (idx, fsig) in module.function_sigs.iter().enumerate() {
                if !selected.contains(&fsig.name) {
                    continue;
                }
                if fsig.kind == FunctionKind::External {
                    continue;
                }
                let start = fsig.entry_inst_idx as usize;
                if start >= module.instructions.len() {
                    continue;
                }
                let end = module_function_body_end(module, idx);
                for item in &module.instructions[start..end.min(module.instructions.len())] {
                    let body = match call_instruction_body(item) {
                        Some(body) => body,
                        None => continue,
                    };
                    let target = match call_target_name(body) {
                        Some(target) => target,
                        None => continue,
                    };
                    if !module_has_function_sig(module, target) {
                        continue;
                    }
                    if selected.insert(target.to_string()) {
                        changed = true;
                    }
                }
            }
        }
    }

    /// Zig `appendDecodedModuleFiltered`: append a decoded
    /// module's consts and the dep-selected functions into the
    /// host module. An extern declaration whose name is also
    /// *defined* in the same module is skipped (the definition
    /// must claim the name, or a dangling extern remains);
    /// names already in `included_imports` are skipped.
    pub fn append_decoded_module_filtered(
        &mut self,
        module: &Module,
        deps: &[String],
    ) -> Result<()> {
        let mut stable_names: HashSet<String> = HashSet::new();
        for fsig in &module.function_sigs {
            stable_names.insert(fsig.name.clone());
        }
        let mut const_names: HashSet<String> = HashSet::new();
        for decl in &module.const_decls {
            stable_names.insert(decl.name.clone());
            const_names.insert(decl.name.clone());
        }

        for decl in &module.const_decls {
            let const_id = crate::symbols::intern_stable(self.b(), &decl.name.clone());
            self.b().record_reg(const_id);
            self.ensure_const_decl_compatible(decl)?;
            if self.has_const_decl(&decl.name) {
                continue;
            }
            let mut cloned = decl.clone();
            cloned.source_line = 0;
            cloned.expanded_line = 0;
            self.b().append_const_decl(cloned);
        }

        let mut selected: HashSet<String> = HashSet::new();
        Self::collect_decoded_module_dep_closure(module, deps, &mut selected);

        let mut defined_names: HashSet<String> = HashSet::new();
        for fsig in &module.function_sigs {
            if fsig.kind != FunctionKind::External {
                defined_names.insert(fsig.name.clone());
            }
        }

        for (idx, fsig) in module.function_sigs.iter().enumerate() {
            if !selected.contains(&fsig.name) {
                continue;
            }
            if fsig.kind == FunctionKind::External && defined_names.contains(&fsig.name) {
                continue;
            }
            if self.state.program.included_imports.contains(&fsig.name) {
                continue;
            }
            self.state
                .program
                .included_imports
                .insert(fsig.name.clone());

            if fsig.kind == FunctionKind::External {
                let cloned = self.clone_module_function_sig(&module.symbols, fsig)?;
                let sig_index = self.push_cloned_sig(cloned);
                if fsig.kind == FunctionKind::TestFunc {
                    let sig = self.b().sig_clone(sig_index);
                    self.state.program.test_sigs.push(sig);
                }
                let name_id = self.b().intern(&fsig.name.clone());
                self.b().append_decl_inst_as(sig_index, name_id);
                continue;
            }

            let start = fsig.entry_inst_idx as usize;
            let end = module_function_body_end(module, idx);
            let mut function_stable_names = stable_names.clone();
            for param in &fsig.params {
                function_stable_names.insert(param.name.clone());
            }
            let mut local_remap = DecodedModuleLocalRemap::new();
            self.collect_decoded_module_local_remap(
                module,
                fsig,
                start,
                end,
                &mut local_remap,
                &function_stable_names,
                &const_names,
            )?;
            let cloned = self.clone_decoded_module_function_sig(
                &module.symbols,
                fsig,
                &mut local_remap,
                &function_stable_names,
            )?;
            let sig_index = self.push_cloned_sig(cloned);
            if fsig.kind == FunctionKind::TestFunc {
                let sig = self.b().sig_clone(sig_index);
                self.state.program.test_sigs.push(sig);
            }
            for item in &module.instructions[start..end] {
                let cloned = self.clone_decoded_module_instruction(
                    &module.symbols,
                    item,
                    &mut local_remap,
                    &function_stable_names,
                )?;
                self.b().append_instruction(cloned);
            }
        }
        Ok(())
    }

    /// Zig `appendDecodedFunctionBody`: splice one function
    /// body out of a decoded module into the current function —
    /// the decl instruction and the entry label are skipped, as
    /// is a trailing return (the caller emits its own).
    pub fn append_decoded_function_body(
        &mut self,
        module: &Module,
        func_name: &str,
    ) -> Result<()> {
        let (start, end) = function_body_bounds(module, func_name)
            .ok_or_else(|| unsupported_f("decoded module has no such function body", "5415"))?;
        let mut i = start;
        while i < end {
            let source = &module.instructions[i];
            if i == start
                && (source.kind == InstKind::FuncDecl || source.kind == InstKind::TestDecl)
            {
                i += 1;
                continue;
            }
            if i == start + 1 && source.kind == InstKind::Label {
                i += 1;
                continue;
            }
            if i + 1 == end && source.kind == InstKind::Return {
                i += 1;
                continue;
            }
            let cloned = self.clone_module_instruction(&module.symbols, source)?;
            self.record_instruction_regs(&cloned);
            self.b().append_instruction(cloned);
            i += 1;
        }
        Ok(())
    }
}

/// Zig `moduleHasFunctionSig`.
pub fn module_has_function_sig(module: &Module, name: &str) -> bool {
    module.function_sigs.iter().any(|fsig| fsig.name == name)
}

/// Zig `moduleFunctionBodyEnd`: the next sig's entry, or the
/// end of the instruction list.
pub fn module_function_body_end(module: &Module, sig_idx: usize) -> usize {
    if sig_idx + 1 < module.function_sigs.len() {
        module.function_sigs[sig_idx + 1].entry_inst_idx as usize
    } else {
        module.instructions.len()
    }
}

/// The `(start, end)` bounds of one function's body in a
/// decoded module (the search shared by Zig's three
/// `appendDecoded*FunctionBody` variants and
/// `templateFunctionBodyBounds`).
pub fn function_body_bounds(module: &Module, func_name: &str) -> Option<(usize, usize)> {
    for (idx, fsig) in module.function_sigs.iter().enumerate() {
        if fsig.name == func_name {
            let start = fsig.entry_inst_idx as usize;
            return Some((start, module_function_body_end(module, idx)));
        }
    }
    None
}

/// Referenced by `fragment.rs`'s text collection for symmetry
/// with Zig's helper set.
#[allow(dead_code)]
fn _symbol_id_anchor(symbols: &[String], name: &str) -> Option<u32> {
    decoded_module_symbol_id_by_name(symbols, name)
}

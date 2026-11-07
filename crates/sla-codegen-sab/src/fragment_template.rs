//! F group (template half) — the std macro *template*
//! machinery of `sab_codegen.zig` (current-head lines
//! 5248–5560 and 6164–6298): fragments flattened with
//! `__sla_macro_arg_{n}` placeholders instead of caller register
//! names, so the flattener cannot mistake caller temps for
//! fragment locals. At append time the placeholders are replaced
//! by the real caller arguments, while every other
//! fragment-internal name goes through the hygiene rename
//! (`fragment.rs`'s `fragmentRenamedName`).
//!
//! Two clone flavours exist in Zig and both are ported:
//! - *template* (`cloneTemplateInstruction`): placeholder
//!   substitution only, no hygiene rename — used for bodies
//!   whose symbols are already host-shaped;
//! - *fragment template* (`cloneFragmentTemplateInstruction`):
//!   substitution *then* rename, token-wise — the cached and
//!   fresh std macro fragment paths.
//!
//! The placeholder-index helpers recognise both the bare
//! `__sla_macro_arg_{n}` form and the hygiene-rewritten
//! `__frag{k}___sla_macro_arg_{n}` form a previous expansion
//! may have produced (`templateSymbolNameArgIndex`).

use crate::error::Result;
use crate::fragment::unsupported_f;
use crate::fragment::FragmentMachine;
use crate::fragment_append::{
    function_body_bounds, is_std_macro_template_integer_arg,
    std_macro_template_integer_operand, structured_panic_msg_operands,
};
use crate::ir::*;
use crate::types::StdMacroTemplate;

/// Zig `isStdMacroTemplateIdentArg` (lowering_rules.zig
/// 4155–4166).
pub fn is_std_macro_template_ident_arg(arg: &str) -> bool {
    if arg.is_empty() {
        return false;
    }
    arg.bytes().enumerate().all(|(idx, ch)| {
        let is_alpha = ch.is_ascii_alphabetic() || ch == b'_';
        let is_digit = ch.is_ascii_digit();
        if idx == 0 {
            is_alpha
        } else {
            is_alpha || is_digit
        }
    })
}

/// Zig `isStdMacroTemplateArgSafe` (lowering_rules.zig
/// 4168–4172): integer or identifier.
pub fn is_std_macro_template_arg_safe(arg: &str) -> bool {
    if arg.is_empty() {
        return false;
    }
    is_std_macro_template_integer_arg(arg) || is_std_macro_template_ident_arg(arg)
}

/// Zig `stdMacroTemplateArgsSafe` (sab_codegen.zig line 6176).
pub fn std_macro_template_args_safe(args: &[String]) -> bool {
    args.iter().all(|arg| is_std_macro_template_arg_safe(arg))
}

/// Zig `stdMacroPlaceholder` (line 5248).
pub fn std_macro_placeholder(index: usize) -> String {
    format!("__sla_macro_arg_{index}")
}

/// Zig `templatePlaceholderArgIndex`: the `n` of a bare
/// `__sla_macro_arg_{n}` name, if it is one.
pub fn template_placeholder_arg_index(name: &str) -> Option<usize> {
    const PREFIX: &str = "__sla_macro_arg_";
    if name.len() <= PREFIX.len() || !name.starts_with(PREFIX) {
        return None;
    }
    name[PREFIX.len()..].parse::<usize>().ok()
}

/// Zig `templateSymbolNameArgIndex`: bare placeholders, and
/// hygiene-rewritten `__frag{digits}_{placeholder}` names.
pub fn template_symbol_name_arg_index(name: &str) -> Option<usize> {
    if let Some(arg_idx) = template_placeholder_arg_index(name) {
        return Some(arg_idx);
    }
    const HYGIENE_PREFIX: &str = "__frag";
    if !name.starts_with(HYGIENE_PREFIX) {
        return None;
    }
    let bytes = name.as_bytes();
    let mut idx = HYGIENE_PREFIX.len();
    let digits_start = idx;
    while idx < name.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == digits_start || idx >= name.len() || bytes[idx] != b'_' {
        return None;
    }
    template_placeholder_arg_index(&name[idx + 1..])
}

/// Zig `templateSymbolArgIndex`: the placeholder index of a
/// source symbol, if it is a placeholder.
pub fn template_symbol_arg_index(symbols: &[String], old_id: u32) -> Option<usize> {
    let name = symbols.get(old_id as usize)?;
    template_symbol_name_arg_index(name)
}

impl<'a> FragmentMachine<'a> {
    /// Zig `replaceStdMacroPlaceholders` (line 5252): substitute
    /// every `__sla_macro_arg_{n}` in the text with `args[n]`.
    /// Texts without the marker pass through untouched (Zig
    /// checks first — the fast path is observable only in
    /// allocation, mirrored for fidelity of the code shape).
    pub fn replace_std_macro_placeholders(&self, text: &str, args: &[String]) -> String {
        if !text.contains("__sla_macro_arg_") {
            return text.to_string();
        }
        let mut current = text.to_string();
        for (idx, arg) in args.iter().enumerate() {
            let placeholder = std_macro_placeholder(idx);
            if !current.contains(&placeholder) {
                continue;
            }
            current = current.replace(&placeholder, arg);
        }
        current
    }

    /// Zig `replaceStdMacroPlaceholdersThenRename` (line 5268):
    /// token-wise — a token that *is* a (possibly
    /// hygiene-rewritten) placeholder becomes the caller arg
    /// verbatim; other tokens are placeholder-substituted and
    /// then hygiene-renamed. Delimiters are the fragment set
    /// (`,`, space, `&`, `^`, `(`, `)`).
    pub fn replace_std_macro_placeholders_then_rename(
        &mut self,
        text: &str,
        args: &[String],
    ) -> Result<String> {
        if self.state.fragment_rename.is_none() {
            return Ok(self.replace_std_macro_placeholders(text, args));
        }
        let mut out = String::new();
        let mut token = String::new();
        let flush = |machine: &mut FragmentMachine,
                         token: &mut String,
                         out: &mut String|
         -> Result<()> {
            if token.is_empty() {
                return Ok(());
            }
            if let Some(arg_idx) = template_symbol_name_arg_index(token) {
                if arg_idx >= args.len() {
                    return Err(unsupported_f(
                        "std macro template placeholder index out of range",
                        "5268-5298",
                    ));
                }
                out.push_str(&args[arg_idx]);
                token.clear();
                return Ok(());
            }
            let replaced = machine.replace_std_macro_placeholders(token, args);
            match machine.fragment_renamed_name(&replaced)? {
                Some(renamed) => out.push_str(&renamed),
                None => out.push_str(&replaced),
            }
            token.clear();
            Ok(())
        };
        for ch in text.chars() {
            let is_delim = matches!(ch, ',' | ' ' | '&' | '^' | '(' | ')');
            if is_delim {
                flush(self, &mut token, &mut out)?;
                out.push(ch);
            } else {
                token.push(ch);
            }
        }
        flush(self, &mut token, &mut out)?;
        Ok(out)
    }

    /// Zig `remapTemplateSymbol`: placeholder-substitute the
    /// source name, intern stable.
    pub fn remap_template_symbol(
        &mut self,
        symbols: &[String],
        old_id: u32,
        args: &[String],
    ) -> Result<u32> {
        let name = crate::fragment::decoded_module_symbol_name(symbols, old_id)?.to_string();
        let replaced = self.replace_std_macro_placeholders(&name, args);
        Ok(crate::symbols::intern_stable(self.b(), &replaced))
    }

    /// Zig `remapFragmentTemplateSymbol`: a placeholder symbol
    /// becomes the caller arg itself (interned stable); other
    /// names are substituted, hygiene-renamed when internal,
    /// and interned stable.
    pub fn remap_fragment_template_symbol(
        &mut self,
        symbols: &[String],
        old_id: u32,
        args: &[String],
    ) -> Result<u32> {
        if let Some(arg_idx) = template_symbol_arg_index(symbols, old_id) {
            if arg_idx >= args.len() {
                return Err(unsupported_f(
                    "std macro template placeholder index out of range",
                    "5300-5310",
                ));
            }
            return Ok(crate::symbols::intern_stable(
                self.b(),
                &args[arg_idx].clone(),
            ));
        }
        let name = crate::fragment::decoded_module_symbol_name(symbols, old_id)?.to_string();
        let replaced = self.replace_std_macro_placeholders(&name, args);
        if self.state.fragment_rename.is_some() {
            if let Some(renamed) = self.fragment_renamed_name(&replaced)? {
                return Ok(crate::symbols::intern_stable(self.b(), &renamed));
            }
        }
        Ok(crate::symbols::intern_stable(self.b(), &replaced))
    }

    /// Zig `remapTemplateOperand`: placeholder registers with
    /// integer args become immediates; everything else goes
    /// through `remapTemplateSymbol`.
    pub fn remap_template_operand(
        &mut self,
        symbols: &[String],
        operand: &Operand,
        args: &[String],
    ) -> Result<Operand> {
        Ok(match operand {
            Operand::Reg(old_id) => {
                if let Some(arg_idx) = template_symbol_arg_index(symbols, *old_id) {
                    if arg_idx < args.len()
                        && is_std_macro_template_integer_arg(&args[arg_idx])
                    {
                        return std_macro_template_integer_operand(&args[arg_idx]);
                    }
                }
                Operand::Reg(self.remap_template_symbol(symbols, *old_id, args)?)
            }
            Operand::Symbol(old_id) => {
                if let Some(arg_idx) = template_symbol_arg_index(symbols, *old_id) {
                    if arg_idx < args.len()
                        && is_std_macro_template_integer_arg(&args[arg_idx])
                    {
                        return std_macro_template_integer_operand(&args[arg_idx]);
                    }
                }
                Operand::Symbol(self.remap_template_symbol(symbols, *old_id, args)?)
            }
            Operand::Label(old_id) => {
                Operand::Label(self.remap_template_symbol(symbols, *old_id, args)?)
            }
            Operand::Func(old_id) => {
                Operand::Func(self.remap_template_symbol(symbols, *old_id, args)?)
            }
            Operand::Text(text) => {
                Operand::Text(self.replace_std_macro_placeholders(text, args))
            }
            Operand::NativeText(text) => {
                Operand::NativeText(self.replace_std_macro_placeholders(text, args))
            }
            other => other.clone(),
        })
    }

    /// Zig `remapFragmentTemplateOperand`: as the template
    /// operand remap, but through the fragment-template symbol
    /// remap and the rename-aware text rewrite.
    pub fn remap_fragment_template_operand(
        &mut self,
        symbols: &[String],
        operand: &Operand,
        args: &[String],
    ) -> Result<Operand> {
        Ok(match operand {
            Operand::Reg(old_id) => {
                if let Some(arg_idx) = template_symbol_arg_index(symbols, *old_id) {
                    if arg_idx < args.len()
                        && is_std_macro_template_integer_arg(&args[arg_idx])
                    {
                        return std_macro_template_integer_operand(&args[arg_idx]);
                    }
                }
                Operand::Reg(self.remap_fragment_template_symbol(symbols, *old_id, args)?)
            }
            Operand::Symbol(old_id) => {
                if let Some(arg_idx) = template_symbol_arg_index(symbols, *old_id) {
                    if arg_idx < args.len()
                        && is_std_macro_template_integer_arg(&args[arg_idx])
                    {
                        return std_macro_template_integer_operand(&args[arg_idx]);
                    }
                }
                Operand::Symbol(self.remap_fragment_template_symbol(symbols, *old_id, args)?)
            }
            Operand::Label(old_id) => {
                Operand::Label(self.remap_fragment_template_symbol(symbols, *old_id, args)?)
            }
            Operand::Func(old_id) => {
                Operand::Func(self.remap_fragment_template_symbol(symbols, *old_id, args)?)
            }
            Operand::Text(text) => {
                Operand::Text(self.replace_std_macro_placeholders_then_rename(text, args)?)
            }
            Operand::NativeText(text) => Operand::NativeText(
                self.replace_std_macro_placeholders_then_rename(text, args)?,
            ),
            other => other.clone(),
        })
    }

    /// Zig `coerceTemplateValueOperand`: a text value operand
    /// that is an integer becomes an immediate; one naming an
    /// already-seen register of the current function becomes
    /// that register.
    pub fn coerce_template_value_operand(&mut self, operand: &mut Operand) {
        let text = match operand {
            Operand::Text(text) => text.trim_matches([' ', '\t', '\r', '\n']).to_string(),
            _ => return,
        };
        if is_std_macro_template_integer_arg(&text) {
            if let Ok(coerced) = std_macro_template_integer_operand(&text) {
                *operand = coerced;
            }
            return;
        }
        if let Some(reg) = self.b().symbol_id(&text) {
            if self.b().reg_seen(reg) {
                *operand = Operand::Reg(reg);
            }
        }
    }

    /// Zig `coerceTemplateInstructionOperands`: same slot set
    /// as the decoded coercion.
    pub fn coerce_template_instruction_operands(&mut self, item: &mut Instruction) {
        match item.kind {
            InstKind::Store => self.coerce_template_value_operand(&mut item.operands[2]),
            InstKind::Assign => self.coerce_template_value_operand(&mut item.operands[1]),
            InstKind::Op | InstKind::PtrAdd => {
                self.coerce_template_value_operand(&mut item.operands[1]);
                self.coerce_template_value_operand(&mut item.operands[2]);
            }
            InstKind::Borrow => self.coerce_template_value_operand(&mut item.operands[1]),
            InstKind::Release => self.coerce_template_value_operand(&mut item.operands[0]),
            _ => {}
        }
    }

    /// Zig `cloneTemplateInstruction`.
    pub fn clone_template_instruction(
        &mut self,
        symbols: &[String],
        source: &Instruction,
        args: &[String],
    ) -> Result<Instruction> {
        let mut out = source.clone();
        out.atomic_expected_text = source
            .atomic_expected_text
            .as_ref()
            .map(|text| self.replace_std_macro_placeholders(text, args));
        out.atomic_new_text = source
            .atomic_new_text
            .as_ref()
            .map(|text| self.replace_std_macro_placeholders(text, args));
        out.native_reg_names = source
            .native_reg_names
            .iter()
            .map(|name| self.replace_std_macro_placeholders(name, args))
            .collect();
        for slot in out.operands.iter_mut() {
            let remapped = self.remap_template_operand(symbols, slot, args)?;
            *slot = remapped;
        }
        self.coerce_template_instruction_operands(&mut out);
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

    /// Zig `cloneFragmentTemplateInstruction`.
    pub fn clone_fragment_template_instruction(
        &mut self,
        symbols: &[String],
        source: &Instruction,
        args: &[String],
    ) -> Result<Instruction> {
        let mut out = source.clone();
        out.atomic_expected_text = match &source.atomic_expected_text {
            Some(text) => Some(self.replace_std_macro_placeholders_then_rename(text, args)?),
            None => None,
        };
        out.atomic_new_text = match &source.atomic_new_text {
            Some(text) => Some(self.replace_std_macro_placeholders_then_rename(text, args)?),
            None => None,
        };
        let mut names = Vec::with_capacity(source.native_reg_names.len());
        for name in &source.native_reg_names {
            names.push(self.replace_std_macro_placeholders_then_rename(name, args)?);
        }
        out.native_reg_names = names;
        for slot in out.operands.iter_mut() {
            let remapped = self.remap_fragment_template_operand(symbols, slot, args)?;
            *slot = remapped;
        }
        self.coerce_template_instruction_operands(&mut out);
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

    /// Zig `appendDecodedTemplateFunctionBody`: the template
    /// flavour of the body splice (same skip rules as the plain
    /// body append).
    pub fn append_decoded_template_function_body(
        &mut self,
        module: &Module,
        func_name: &str,
        args: &[String],
    ) -> Result<()> {
        let (start, end) = function_body_bounds(module, func_name).ok_or_else(|| {
            unsupported_f("decoded template module has no such function body", "5450")
        })?;
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
            let cloned = self.clone_template_instruction(&module.symbols, source, args)?;
            self.record_instruction_regs(&cloned);
            self.b().append_instruction(cloned);
            i += 1;
        }
        Ok(())
    }

    /// Zig `appendDecodedFragmentTemplateFunctionBody`: the
    /// fragment-template flavour of the body splice.
    pub fn append_decoded_fragment_template_function_body(
        &mut self,
        module: &Module,
        func_name: &str,
        args: &[String],
    ) -> Result<()> {
        let (start, end) = function_body_bounds(module, func_name).ok_or_else(|| {
            unsupported_f("decoded fragment module has no such function body", "5490")
        })?;
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
            let cloned =
                self.clone_fragment_template_instruction(&module.symbols, source, args)?;
            self.record_instruction_regs(&cloned);
            self.b().append_instruction(cloned);
            i += 1;
        }
        Ok(())
    }

    /// Zig `appendRenamedFragmentBody` (line 5149): run the
    /// plain body splice with a fresh hygiene rename map active
    /// (the previous map/args are restored afterwards, as Zig's
    /// save/restore does).
    pub fn append_renamed_fragment_body(
        &mut self,
        module: &Module,
        func_name: &str,
        args: &[String],
    ) -> Result<()> {
        let prev_rename = self.state.fragment_rename.take();
        let prev_args = self.state.fragment_rename_args.take();
        self.state.fragment_rename = Some(std::collections::HashMap::new());
        self.state.fragment_rename_args = Some(args.to_vec());
        let result = self.append_decoded_function_body(module, func_name);
        self.state.fragment_rename = prev_rename;
        self.state.fragment_rename_args = prev_args;
        result
    }

    /// Zig `appendRenamedTemplateFragmentBody` (line 5173): as
    /// above, with the fragment-template body splice.
    pub fn append_renamed_template_fragment_body(
        &mut self,
        module: &Module,
        func_name: &str,
        args: &[String],
    ) -> Result<()> {
        let prev_rename = self.state.fragment_rename.take();
        let prev_args = self.state.fragment_rename_args.take();
        self.state.fragment_rename = Some(std::collections::HashMap::new());
        self.state.fragment_rename_args = Some(args.to_vec());
        let result = self.append_decoded_fragment_template_function_body(module, func_name, args);
        self.state.fragment_rename = prev_rename;
        self.state.fragment_rename_args = prev_args;
        result
    }
}

/// Zig `templateFunctionBodyBounds` (line 6231).
pub fn template_function_body_bounds(
    template: &StdMacroTemplate,
    func_name: &str,
) -> Option<(usize, usize)> {
    function_body_bounds(&template.module, func_name)
}

/// Zig `stdMacroTemplateSupportsArgs` (line 6245): a template
/// whose body contains a call body cannot take non-ident,
/// non-integer args whose placeholder appears in atomic texts
/// or native reg names (those slots are not rewritten by the
/// template operand remap).
pub fn std_macro_template_supports_args(
    template: &StdMacroTemplate,
    args: &[String],
) -> Result<bool> {
    let bounds = match template_function_body_bounds(template, &template.func_name) {
        Some(bounds) => bounds,
        None => return Ok(false),
    };
    let (start, end) = bounds;
    for i in start..end {
        let source = &template.module.instructions[i];
        if crate::fragment::call_instruction_body(source).is_some() {
            return Ok(false);
        }
    }
    for (idx, arg) in args.iter().enumerate() {
        if is_std_macro_template_ident_arg(arg) || is_std_macro_template_integer_arg(arg) {
            continue;
        }
        let placeholder = std_macro_placeholder(idx);
        for i in start..end {
            let source = &template.module.instructions[i];
            if i == start
                && (source.kind == InstKind::FuncDecl || source.kind == InstKind::TestDecl)
            {
                continue;
            }
            if i == start + 1 && source.kind == InstKind::Label {
                continue;
            }
            if i + 1 == end && source.kind == InstKind::Return {
                continue;
            }
            if let Some(text) = &source.atomic_expected_text {
                if text.contains(&placeholder) {
                    return Ok(false);
                }
            }
            if let Some(text) = &source.atomic_new_text {
                if text.contains(&placeholder) {
                    return Ok(false);
                }
            }
            for text in &source.native_reg_names {
                if text.contains(&placeholder) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

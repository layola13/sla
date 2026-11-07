//! F group (core) — the decoded-module splicing machinery of
//! `sab_codegen.zig` (current-head lines 4817–5560): when a std
//! fragment or imported std module has been decoded from SAB
//! bytes (`decode.rs`), its instructions cannot be appended
//! verbatim — its symbol ids index *its own* pool. This module
//! remaps every id into the host module's namespace:
//!
//! - function names, globals and consts are interned *stable*
//!   (same name ⇒ same id, across all decoded modules);
//! - fragment-internal registers get hygienic fresh symbols
//!   (`__decoded_r_{n}_{old}_{name}`) recorded in a
//!   [`DecodedModuleLocalRemap`], labels get `__decoded_L_…`;
//! - text operands that embed register names are rewritten
//!   token-by-token so call bodies stay consistent with the
//!   remapped structured operands.
//!
//! All maps are lookup-only; the two order-bearing sequences
//! (`reg_order`, `extra_reg_ids`) are `Vec`s, as in Zig. The
//! machine borrows the whole [`CodegenState`]: the builder owns
//! the symbol/instruction tables, the program state owns
//! `included_imports` / `test_sigs`, and the F counters
//! (`decoded_module_rename_idx`, `fragment_rename*`) live beside
//! them, exactly as the fields sit on Zig's `Codegen`.

use std::collections::HashSet;

use crate::emit::ModuleBuilder;
use crate::error::{Error, Result};
use crate::ir::*;
use crate::state::CodegenState;
use crate::types::DecodedModuleLocalRemap;

pub(crate) fn unsupported_f(what: &str, zig_ref: &str) -> Error {
    Error::UnsupportedSabDirectFeature(format!(
        "{what} [group F fragments; Zig sab_codegen.zig:{zig_ref}]"
    ))
}

/// The F-group machine over the codegen state. One machine is
/// created per splice operation (the Zig original is methods on
/// `Codegen` itself; the borrow is the whole state because a
/// splice touches builder, registries and counters together).
pub struct FragmentMachine<'a> {
    pub state: &'a mut CodegenState,
}

impl<'a> FragmentMachine<'a> {
    pub fn new(state: &'a mut CodegenState) -> Self {
        FragmentMachine { state }
    }

    pub(crate) fn b(&mut self) -> &mut ModuleBuilder {
        &mut self.state.builder
    }

    /// Zig `internStable` via C3.
    fn intern_stable(&mut self, name: &str) -> u32 {
        crate::symbols::intern_stable(self.b(), name)
    }

    /// Zig `remapModuleSymbol` (line 4817): under an active
    /// fragment rename, internal names are uniquified first;
    /// everything is interned stable into the host namespace.
    pub fn remap_module_symbol(&mut self, symbols: &[String], old_id: u32) -> Result<u32> {
        let name = decoded_module_symbol_name(symbols, old_id)?.to_string();
        if self.state.fragment_rename.is_some() {
            let renamed = self.fragment_renamed_name(&name)?;
            if let Some(renamed) = renamed {
                return Ok(self.intern_stable(&renamed));
            }
        }
        Ok(self.intern_stable(&name))
    }

    /// Zig `fragmentRenamedName` (line 5123): caller arguments
    /// (by name) keep their identity; fragment-internal names
    /// (`tmp_*`, `__*`, `L_*`) get a `__frag{n}_{name}` rewrite,
    /// remembered in the active rename map. Returns `None` when
    /// the name is not fragment-internal.
    pub fn fragment_renamed_name(&mut self, name: &str) -> Result<Option<String>> {
        if let Some(args) = &self.state.fragment_rename_args {
            if args.iter().any(|arg| arg == name) {
                return Ok(None);
            }
        }
        let is_internal = sla_lowering::is_temporary_register_name(name)
            || name.starts_with("__")
            || name.starts_with("L_");
        if !is_internal {
            return Ok(None);
        }
        let rename = self
            .state
            .fragment_rename
            .as_mut()
            .expect("fragment_renamed_name called without an active rename map");
        if let Some(existing) = rename.get(name) {
            return Ok(Some(existing.clone()));
        }
        let unique = format!("__frag{}_{}", self.state.fragment_rename_idx, name);
        self.state.fragment_rename_idx += 1;
        rename.insert(name.to_string(), unique.clone());
        Ok(Some(unique))
    }

    /// Zig `newDecodedModuleLocalSymbol` (line 4837).
    fn new_decoded_module_local_symbol(&mut self, prefix: &str, old_id: u32) -> u32 {
        let name = format!(
            "__decoded_{}_{}_{}",
            prefix, self.state.decoded_module_rename_idx, old_id
        );
        self.state.decoded_module_rename_idx += 1;
        self.b().intern(&name)
    }

    /// Zig `newDecodedModuleRegSymbol` (line 4849).
    fn new_decoded_module_reg_symbol(&mut self, old_id: u32, old_name: &str) -> u32 {
        let name = format!(
            "__decoded_r_{}_{}_{}",
            self.state.decoded_module_rename_idx, old_id, old_name
        );
        self.state.decoded_module_rename_idx += 1;
        self.b().intern(&name)
    }

    /// Zig `ensureDecodedModuleRegId` (line 4861): the hygienic
    /// id for a fragment register, allocated on first sight and
    /// recorded in the remap's order/name tables.
    pub fn ensure_decoded_module_reg_id(
        &mut self,
        symbols: &[String],
        remap: &mut DecodedModuleLocalRemap,
        old_id: u32,
    ) -> Result<u32> {
        if let Some(&existing) = remap.stable_reg_ids.get(&old_id) {
            return Ok(existing);
        }
        if let Some(&existing) = remap.reg_ids.get(&old_id) {
            return Ok(existing);
        }
        let old_name = decoded_module_reg_symbol_name(symbols, remap, old_id)?.to_string();
        let new_id = self.new_decoded_module_reg_symbol(old_id, &old_name);
        remap.reg_ids.insert(old_id, new_id);
        remap.source_reg_symbol_ids.insert(old_id, new_id);
        remap.reg_order.push(old_id);
        let new_name = self.b().symbol_name(new_id).to_string();
        remap.reg_names.entry(old_name.clone()).or_insert(new_name);
        remap.reg_name_ids.entry(old_name).or_insert(new_id);
        Ok(new_id)
    }

    /// Zig `ensureDecodedModuleNamedRegId` (line 4884): a
    /// text-embedded name with no source symbol gets a stable id
    /// and joins the extra-reg list.
    pub fn ensure_decoded_module_named_reg_id(
        &mut self,
        remap: &mut DecodedModuleLocalRemap,
        name: &str,
    ) -> u32 {
        if let Some(&existing) = remap.reg_name_ids.get(name) {
            return existing;
        }
        let id = self.intern_stable(name);
        remap.extra_reg_ids.push(id);
        let key = self.b().symbol_name(id).to_string();
        remap.reg_name_ids.insert(key, id);
        id
    }

    /// Zig `ensureDecodedModuleLabelId` (line 4895).
    pub fn ensure_decoded_module_label_id(
        &mut self,
        remap: &mut DecodedModuleLocalRemap,
        old_id: u32,
    ) -> u32 {
        if let Some(&existing) = remap.label_ids.get(&old_id) {
            return existing;
        }
        let new_id = self.new_decoded_module_local_symbol("L", old_id);
        remap.label_ids.insert(old_id, new_id);
        new_id
    }

    /// Zig `remapDecodedModuleRegId` (line 4905): param/stable
    /// ids win, stable *names* intern stable, everything else is
    /// hygienic.
    pub fn remap_decoded_module_reg_id(
        &mut self,
        symbols: &[String],
        old_id: u32,
        remap: &mut DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
    ) -> Result<u32> {
        if let Some(&existing) = remap.stable_reg_ids.get(&old_id) {
            return Ok(existing);
        }
        let name = decoded_module_reg_symbol_name(symbols, remap, old_id)?.to_string();
        if stable_names.contains(&name) {
            return Ok(self.intern_stable(&name));
        }
        self.ensure_decoded_module_reg_id(symbols, remap, old_id)
    }

    /// Zig `remapDecodedModuleIds` (line 4917).
    pub fn remap_decoded_module_ids(
        &mut self,
        symbols: &[String],
        ids: &[u32],
        remap: &mut DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
    ) -> Result<Vec<u32>> {
        let mut out = Vec::with_capacity(ids.len());
        for &old_id in ids {
            out.push(self.remap_decoded_module_reg_id(symbols, old_id, remap, stable_names)?);
        }
        Ok(out)
    }

    /// Zig `remapModuleIds` (line 5561 of the plain path): every
    /// id through `remapModuleSymbol`.
    pub fn remap_module_ids(&mut self, symbols: &[String], ids: &[u32]) -> Result<Vec<u32>> {
        let mut out = Vec::with_capacity(ids.len());
        for &old_id in ids {
            out.push(self.remap_module_symbol(symbols, old_id)?);
        }
        Ok(out)
    }

    /// Zig `collectDecodedModuleRegId` (line 4931): stable const
    /// names alias their stable id in the remap (so const
    /// references stay comparable by id across decoded modules);
    /// other stable names are skipped — their references resolve
    /// by name downstream; all other names get hygienic ids.
    pub fn collect_decoded_module_reg_id(
        &mut self,
        symbols: &[String],
        old_id: u32,
        remap: &mut DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
        const_names: &HashSet<String>,
    ) -> Result<()> {
        let name = decoded_module_symbol_name(symbols, old_id)?.to_string();
        if stable_names.contains(&name) {
            if const_names.contains(&name) {
                if remap.stable_reg_ids.contains_key(&old_id)
                    || remap.reg_ids.contains_key(&old_id)
                {
                    return Ok(());
                }
                let stable_id = self.intern_stable(&name);
                remap.reg_ids.insert(old_id, stable_id);
                remap.reg_order.push(old_id);
                let stable_name = self.b().symbol_name(stable_id).to_string();
                remap.reg_names.entry(name.clone()).or_insert(stable_name);
                remap.reg_name_ids.entry(name).or_insert(stable_id);
            }
            return Ok(());
        }
        self.ensure_decoded_module_reg_id(symbols, remap, old_id)?;
        Ok(())
    }

    /// Zig `collectDecodedModuleTextRegs` (line 4971): scan a
    /// text operand's tokens and collect every token that names
    /// a fragment register. Delimiters are Zig's exact set for
    /// the decoded path (`,`, whitespace, `& ^ * ( )`).
    pub fn collect_decoded_module_text_regs(
        &mut self,
        symbols: &[String],
        text: &str,
        remap: &mut DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
        const_names: &HashSet<String>,
        allow_unknown: bool,
    ) -> Result<()> {
        fn flush(
            machine: &mut FragmentMachine,
            symbols: &[String],
            token: &str,
            remap: &mut DecodedModuleLocalRemap,
            stable_names: &HashSet<String>,
            const_names: &HashSet<String>,
            allow_unknown: bool,
        ) -> Result<()> {
            if !decoded_module_text_token_can_be_local_reg(token) {
                return Ok(());
            }
            if stable_names.contains(token) && !const_names.contains(token) {
                return Ok(());
            }
            if remap.reg_names.contains_key(token) {
                return Ok(());
            }
            if let Some(old_id) = decoded_module_symbol_id_by_name(symbols, token) {
                let operand_id =
                    decoded_module_operand_id_for_source_symbol(remap, old_id)
                        .unwrap_or(old_id);
                return machine.collect_decoded_module_reg_id(
                    symbols,
                    operand_id,
                    remap,
                    stable_names,
                    const_names,
                );
            }
            if allow_unknown {
                machine.ensure_decoded_module_named_reg_id(remap, token);
            }
            Ok(())
        }
        let mut token = String::new();
        for ch in text.chars() {
            let is_delim = matches!(ch, ',' | ' ' | '\t' | '\r' | '\n' | '&' | '^' | '*' | '(' | ')');
            if is_delim {
                flush(self, symbols, &token, remap, stable_names, const_names, allow_unknown)?;
                token.clear();
            } else {
                token.push(ch);
            }
        }
        flush(self, symbols, &token, remap, stable_names, const_names, allow_unknown)
    }

    /// Zig `collectDecodedModuleLocalRemap` (line 5018): seed
    /// the param ids as stable, then walk the body slice
    /// collecting register, label and text-embedded names.
    pub fn collect_decoded_module_local_remap(
        &mut self,
        module: &Module,
        fsig: &FunctionSig,
        start: usize,
        end: usize,
        remap: &mut DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
        const_names: &HashSet<String>,
    ) -> Result<()> {
        for (param_idx, &old_id) in fsig.param_ids.iter().enumerate() {
            if param_idx >= fsig.params.len() {
                break;
            }
            let stable_id = self.intern_stable(&fsig.params[param_idx].name.clone());
            remap.stable_reg_ids.insert(old_id, stable_id);
            remap.source_reg_symbol_ids.insert(old_id, stable_id);
            let key = self.b().symbol_name(stable_id).to_string();
            remap.reg_name_ids.insert(key, stable_id);
        }
        for item in &module.instructions[start..end] {
            for (operand_idx, operand) in item.operands.iter().enumerate() {
                match operand {
                    Operand::Reg(old_id) => self.collect_decoded_module_reg_id(
                        &module.symbols,
                        *old_id,
                        remap,
                        stable_names,
                        const_names,
                    )?,
                    Operand::Label(old_id) => {
                        self.ensure_decoded_module_label_id(remap, *old_id);
                    }
                    Operand::Symbol(old_id) => {
                        if decoded_module_symbol_operand_is_label(item.kind, operand_idx) {
                            self.ensure_decoded_module_label_id(remap, *old_id);
                        }
                    }
                    Operand::Text(text) => self.collect_decoded_module_text_regs(
                        &module.symbols,
                        text,
                        remap,
                        stable_names,
                        const_names,
                        true,
                    )?,
                    Operand::NativeText(text) => self.collect_decoded_module_text_regs(
                        &module.symbols,
                        text,
                        remap,
                        stable_names,
                        const_names,
                        false,
                    )?,
                    _ => {}
                }
            }
            if let Some(text) = &item.atomic_expected_text {
                self.collect_decoded_module_text_regs(
                    &module.symbols,
                    text,
                    remap,
                    stable_names,
                    const_names,
                    true,
                )?;
            }
            if let Some(text) = &item.atomic_new_text {
                self.collect_decoded_module_text_regs(
                    &module.symbols,
                    text,
                    remap,
                    stable_names,
                    const_names,
                    true,
                )?;
            }
            for name in &item.native_reg_names {
                self.collect_decoded_module_text_regs(
                    &module.symbols,
                    name,
                    remap,
                    stable_names,
                    const_names,
                    false,
                )?;
            }
        }
        Ok(())
    }

    /// Zig `renameDecodedModuleLocalText` (line 5071): rewrite
    /// the tokens of a text operand through the remap — by
    /// mapped name first, then by source-symbol id chain.
    pub fn rename_decoded_module_local_text(
        &mut self,
        symbols: &[String],
        text: &str,
        remap: &DecodedModuleLocalRemap,
    ) -> String {
        fn flush(
            machine: &FragmentMachine,
            symbols: &[String],
            token: &str,
            remap: &DecodedModuleLocalRemap,
            out: &mut String,
        ) {
            if token.is_empty() {
                return;
            }
            if let Some(mapped_name) = remap.reg_names.get(token) {
                out.push_str(mapped_name);
                return;
            }
            if let Some(old_id) = decoded_module_symbol_id_by_name(symbols, token) {
                let mapped_id = remap
                    .stable_reg_ids
                    .get(&old_id)
                    .or_else(|| remap.reg_ids.get(&old_id))
                    .or_else(|| remap.source_reg_symbol_ids.get(&old_id));
                if let Some(&id) = mapped_id {
                    if (id as usize) < machine.state.builder.symbols_len() {
                        out.push_str(machine.state.builder.symbol_name(id));
                        return;
                    }
                }
            }
            out.push_str(token);
        }
        let mut out = String::new();
        let mut token = String::new();
        for ch in text.chars() {
            let is_delim = matches!(ch, ',' | ' ' | '\t' | '\r' | '\n' | '&' | '^' | '*' | '(' | ')');
            if is_delim {
                flush(self, symbols, &token, remap, &mut out);
                token.clear();
                out.push(ch);
            } else {
                token.push(ch);
            }
        }
        flush(self, symbols, &token, remap, &mut out);
        out
    }

    /// Zig `remapDecodedModuleOperand` (line 5110 in the plain
    /// numbering of this port's reference tree; the decoded
    /// operand remap used by `cloneDecodedModuleInstruction`).
    pub fn remap_decoded_module_operand(
        &mut self,
        symbols: &[String],
        operand: &Operand,
        kind: InstKind,
        operand_idx: usize,
        remap: &mut DecodedModuleLocalRemap,
        stable_names: &HashSet<String>,
    ) -> Result<Operand> {
        Ok(match operand {
            Operand::Reg(old_id) => Operand::Reg(self.remap_decoded_module_reg_id(
                symbols,
                *old_id,
                remap,
                stable_names,
            )?),
            Operand::Symbol(old_id) => {
                if decoded_module_symbol_operand_is_label(kind, operand_idx) {
                    Operand::Symbol(self.ensure_decoded_module_label_id(remap, *old_id))
                } else {
                    Operand::Symbol(self.remap_module_symbol(symbols, *old_id)?)
                }
            }
            Operand::Label(old_id) => {
                Operand::Label(self.ensure_decoded_module_label_id(remap, *old_id))
            }
            Operand::Func(old_id) => Operand::Func(self.remap_module_symbol(symbols, *old_id)?),
            Operand::Text(text) => {
                Operand::Text(self.rename_decoded_module_local_text(symbols, text, remap))
            }
            Operand::NativeText(text) => {
                Operand::NativeText(self.rename_decoded_module_local_text(symbols, text, remap))
            }
            other => other.clone(),
        })
    }

    /// Zig `remapModuleOperand`: the plain (non-decoded) remap
    /// used by `cloneModuleInstruction` — every id through
    /// `remapModuleSymbol`, text through the fragment rename,
    /// native text verbatim.
    pub fn remap_module_operand(
        &mut self,
        symbols: &[String],
        operand: &Operand,
    ) -> Result<Operand> {
        Ok(match operand {
            Operand::Reg(old_id) => Operand::Reg(self.remap_module_symbol(symbols, *old_id)?),
            Operand::Symbol(old_id) => {
                Operand::Symbol(self.remap_module_symbol(symbols, *old_id)?)
            }
            Operand::Label(old_id) => {
                Operand::Label(self.remap_module_symbol(symbols, *old_id)?)
            }
            Operand::Func(old_id) => Operand::Func(self.remap_module_symbol(symbols, *old_id)?),
            Operand::Text(text) => Operand::Text(self.rename_fragment_text(text)?),
            Operand::NativeText(text) => Operand::NativeText(text.clone()),
            other => other.clone(),
        })
    }

    /// Zig `renameFragmentText`: rewrite fragment-internal
    /// tokens of a call-body text through the active rename map.
    /// Delimiters here are the *fragment* set (`,`, space, `&`,
    /// `^`, `(`, `)`) — narrower than the decoded set; mirrored
    /// exactly. Without an active rename the text passes through.
    pub fn rename_fragment_text(&mut self, text: &str) -> Result<String> {
        if self.state.fragment_rename.is_none() {
            return Ok(text.to_string());
        }
        let mut out = String::new();
        let mut token = String::new();
        for ch in text.chars() {
            let is_delim = matches!(ch, ',' | ' ' | '&' | '^' | '(' | ')');
            if is_delim {
                if !token.is_empty() {
                    match self.fragment_renamed_name(&token.clone())? {
                        Some(renamed) => out.push_str(&renamed),
                        None => out.push_str(&token),
                    }
                    token.clear();
                }
                out.push(ch);
            } else {
                token.push(ch);
            }
        }
        if !token.is_empty() {
            match self.fragment_renamed_name(&token.clone())? {
                Some(renamed) => out.push_str(&renamed),
                None => out.push_str(&token),
            }
        }
        Ok(out)
    }
}


/// The free name/lookup helpers live in `fragment_names.rs`;
/// re-exported here so the F modules share one surface.
pub use crate::fragment_names::{
    call_instruction_body, call_target_name, clone_decoded_module_reg_ids,
    decoded_module_operand_id_for_source_symbol, decoded_module_reg_symbol_name,
    decoded_module_symbol_id_by_name, decoded_module_symbol_name,
    decoded_module_symbol_operand_is_label, decoded_module_text_token_can_be_local_reg,
    direct_call_target_name,
};

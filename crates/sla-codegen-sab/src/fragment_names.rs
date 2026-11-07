//! F group (name helpers) — the free functions of the
//! decoded-module machinery: source-pool name lookups, the
//! text-token shape test, the remap's register-list closure,
//! and the call-body name scanners (`sab_codegen.zig`,
//! current-head lines 4825–4970 and 7214–7245).

use std::collections::HashSet;

use crate::error::Result;
use crate::fragment::unsupported_f;
use crate::ir::*;
use crate::types::DecodedModuleLocalRemap;

/// Zig `decodedModuleSymbolName` (line 4825): the source name of
/// a symbol id; out of range is `UnsupportedSabDirectFeature`.
pub fn decoded_module_symbol_name(symbols: &[String], old_id: u32) -> Result<&str> {
    symbols
        .get(old_id as usize)
        .map(|s| s.as_str())
        .ok_or_else(|| {
            unsupported_f(
                "decoded module symbol id out of range",
                "4825-4829",
            )
        })
}

/// Zig `decodedModuleRegSymbolName` (line 4844): today an alias
/// of `decodedModuleSymbolName` (the remap parameter is ignored
/// in Zig — mirrored, not "improved").
pub fn decoded_module_reg_symbol_name<'s>(
    symbols: &'s [String],
    _remap: &DecodedModuleLocalRemap,
    old_id: u32,
) -> Result<&'s str> {
    decoded_module_symbol_name(symbols, old_id)
}

/// Zig `decodedModuleSymbolOperandIsLabel` (line 4924): only
/// `label` / `jmp` instructions carry their target as operand 0
/// in symbol form.
pub fn decoded_module_symbol_operand_is_label(kind: InstKind, operand_idx: usize) -> bool {
    matches!(kind, InstKind::Label | InstKind::Jmp) && operand_idx == 0
}

/// Zig `decodedModuleSymbolIdByName` (line 4954): first match
/// wins (linear scan over the source pool).
pub fn decoded_module_symbol_id_by_name(symbols: &[String], name: &str) -> Option<u32> {
    symbols
        .iter()
        .position(|symbol_name| symbol_name == name)
        .map(|idx| idx as u32)
}

/// Zig `decodedModuleOperandIdForSourceSymbol` (line 4961).
pub fn decoded_module_operand_id_for_source_symbol(
    remap: &DecodedModuleLocalRemap,
    source_symbol_id: u32,
) -> Option<u32> {
    remap.source_reg_symbol_ids.get(&source_symbol_id).copied()
}

/// Zig `decodedModuleTextTokenCanBeLocalReg` (line 4965): a
/// token can name a local register iff it is identifier-shaped
/// and does not start with `@`.
pub fn decoded_module_text_token_can_be_local_reg(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let bytes = token.as_bytes();
    if bytes[0] == b'@' {
        return false;
    }
    if !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&ch| ch.is_ascii_alphanumeric() || ch == b'_')
}

/// Zig `cloneDecodedModuleRegIds` (line 4899 numbering of the
/// reference tree): the sig's register list — required ids
/// first, then the remap's collected ids in collection order,
/// then the extras; deduplicated by first occurrence. The
/// `seen` set is lookup-only; emission order is the Vec's.
pub fn clone_decoded_module_reg_ids(
    required_ids: &[u32],
    remap: &DecodedModuleLocalRemap,
) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    for &new_id in required_ids {
        if seen.insert(new_id) {
            out.push(new_id);
        }
    }
    for &old_id in &remap.reg_order {
        if let Some(&new_id) = remap.reg_ids.get(&old_id) {
            if seen.insert(new_id) {
                out.push(new_id);
            }
        }
    }
    for &new_id in &remap.extra_reg_ids {
        if seen.insert(new_id) {
            out.push(new_id);
        }
    }
    out
}

/// Zig `callInstructionBody` (line 7214): the text body of a
/// call / call_indirect instruction, from operand 1 first.
pub fn call_instruction_body(item: &Instruction) -> Option<&str> {
    if item.kind != InstKind::Call && item.kind != InstKind::CallIndirect {
        return None;
    }
    if let Operand::Text(text) = &item.operands[1] {
        return Some(text);
    }
    if let Operand::Text(text) = &item.operands[0] {
        return Some(text);
    }
    None
}

/// Zig `callTargetName` (line 7225): the `@name` a call body
/// starts with (after blank trimming), if any.
pub fn call_target_name(body: &str) -> Option<&str> {
    let trimmed = body.trim_start_matches([' ', '\t', '\r']);
    if trimmed.is_empty() || !trimmed.starts_with('@') {
        return None;
    }
    let mut end = 1;
    for (i, ch) in trimmed.char_indices() {
        if i == 0 {
            continue;
        }
        if ch.is_ascii_alphanumeric() || ch == '_' {
            end = i + ch.len_utf8();
        } else {
            break;
        }
    }
    if end == 1 {
        return None;
    }
    Some(&trimmed[1..end])
}

/// Zig `directCallTargetName` (line 7234): a call target
/// immediately followed by `(`.
pub fn direct_call_target_name(body: &str) -> Option<&str> {
    let trimmed = body.trim_start_matches([' ', '\t', '\r']);
    let target = call_target_name(trimmed)?;
    let after = trimmed[target.len() + 1..].trim_start_matches([' ', '\t', '\r']);
    if after.is_empty() || !after.starts_with('(') {
        return None;
    }
    Some(target)
}

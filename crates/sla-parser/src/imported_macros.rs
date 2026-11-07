//! Imported SA-text macro registration, mirrored from
//! `sa_plugin_sla/src/plugin_imported_macros.zig` (420 lines at the
//! box head 2cc12ea8; the local read-only tree carries the 288-line
//! ancestor — the assigned-output analysis and the cache-stamp
//! tests are new in the box head and are mirrored here).
//!
//! Zig's loader registers straight into the checker's
//! `imported_macros` table. Here the scan is pure: it produces
//! [`ImportedMacroRecord`]s against an ordered registry slice (the
//! plugin wiring layer registers them into the checker, whose
//! `ImportedMacroInfo` has no `assigned_output_mask` field — that
//! mask is carried in the record and dropped at the registration
//! boundary, registered as IMP-2, the same shape as INT2-2/INT2-3).
//! The registry keeps its records in a Vec in registration
//! order (the order every consumer observes) plus a name → slot
//! index used ONLY for lookups — the index is never iterated, so
//! no hash order reaches any output. (CLI-D4 timing fix: the
//! original port scanned the Vec linearly by name; Zig's checker
//! table is a hash map. With the std contract closure carrying
//! ~6k macros and ~6k `EXPAND` lines, the linear scans made
//! contract loading quadratic — the bulk of the 01_hello
//! 144 ms anomaly. Lookup complexity only; records, order, and
//! overwrite-in-place semantics are unchanged.)
//!
//! NOT ported: the `.sla-cache/macros/*.idx` disk memo (Wyhash key
//! + compiler stamp). It memoizes exactly this scan; skipping it
//! changes only speed. See IMP-6 in docs/divergences/import-expand.md.

/// One registered imported macro (Zig `type_checker.ImportedMacro`
/// plus its registration name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedMacroRecord {
    pub name: String,
    pub arity: usize,
    pub leading_outputs: usize,
    pub import_path: Option<String>,
    pub borrowed_arg_mask: u64,
    pub address_slot_arg_mask: u64,
    pub assigned_output_mask: u64,
    pub direct_callees: Vec<String>,
}

/// The ordered imported-macro registry (see the module docs):
/// records in registration order, name lookups through an index
/// that is never iterated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportedMacroRegistry {
    pub records: Vec<ImportedMacroRecord>,
    index: std::collections::HashMap<String, usize>,
}

impl ImportedMacroRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<&ImportedMacroRecord> {
        self.index.get(name).map(|&slot| &self.records[slot])
    }

    /// Zig `imported_macros.put`: a later definition overwrites
    /// the earlier one in place; a new name appends.
    pub fn put(&mut self, record: ImportedMacroRecord) {
        if let Some(&slot) = self.index.get(&record.name) {
            self.records[slot] = record;
        } else {
            self.index.insert(record.name.clone(), self.records.len());
            self.records.push(record);
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Zig `expandedSourceMayContainImportedMacros`.
pub fn expanded_source_may_contain_imported_macros(expanded_source: &str) -> bool {
    expanded_source.contains("[MACRO]")
}

/// Zig `macroParamName`: trim spaces/tabs/CR/commas, strip one `%`.
pub fn macro_param_name(raw: &str) -> &str {
    let mut param = raw.trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == ',');
    if param.starts_with('%') {
        param = &param[1..];
    }
    param
}

/// Zig `isLeadingOutputMacroParam`.
pub fn is_leading_output_macro_param(raw: &str) -> bool {
    let param = macro_param_name(raw);
    param.starts_with("out")
        || param == "nonnull_ptr"
        || param == "type_id"
        || param == "any_ref"
        || param == "cursor"
        || param == "take"
        || param == "repeat"
}

/// Zig `macroParamIndex`.
pub fn macro_param_index(param_names: &[String], name: &str) -> Option<usize> {
    param_names.iter().position(|p| p == name)
}

/// Zig `markBorrowedParam` (used for both masks, as in Zig).
pub fn mark_borrowed_param(mask: &mut u64, param_names: &[String], raw_name: &str) {
    let name = macro_param_name(raw_name);
    if let Some(idx) = macro_param_index(param_names, name) {
        if idx < 64 {
            *mask |= 1u64 << idx;
        }
    }
}

/// Zig `markDirectBorrowedMacroParams`: a literal `&%param` anywhere
/// in the line marks the parameter borrowed.
pub fn mark_direct_borrowed_macro_params(mask: &mut u64, param_names: &[String], line: &str) {
    for param in param_names {
        let needle = format!("&%{param}");
        if line.contains(&needle) {
            mark_borrowed_param(mask, param_names, param);
        }
    }
}

fn is_macro_register_name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'%'
}

/// Zig `markDirectAddressSlotMacroParams`: a `%param+` shape counts
/// only when the `%` starts a fresh register token (the D3 comment
/// in the Zig source: an interpolated synthetic register name that
/// merely contains `%param` is a different register).
pub fn mark_direct_address_slot_macro_params(mask: &mut u64, param_names: &[String], line: &str) {
    let bytes = line.as_bytes();
    for param in param_names {
        let needle = format!("%{param}+");
        let needle = needle.as_bytes();
        let mut search_from = 0usize;
        while let Some(hit) = find_subslice(&bytes[search_from..], needle) {
            let hit = search_from + hit;
            if hit == 0 || !is_macro_register_name_char(bytes[hit - 1]) {
                mark_borrowed_param(mask, param_names, param);
                break;
            }
            search_from = hit + 1;
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacroParamRefKind {
    Value,
    Deref,
    AddrTake,
}

/// Zig `classifyMacroParamToken`.
fn classify_macro_param_token(token: &str, param: &str) -> Option<MacroParamRefKind> {
    let mut rest = token;
    let mut took_addr = false;
    if rest.starts_with('&') {
        took_addr = true;
        rest = &rest[1..];
    }
    if !rest.starts_with('%') {
        return None;
    }
    rest = &rest[1..];
    if !rest.starts_with(param) {
        return None;
    }
    let after = &rest[param.len()..];
    if after.is_empty() {
        return Some(if took_addr {
            MacroParamRefKind::AddrTake
        } else {
            MacroParamRefKind::Value
        });
    }
    if after.starts_with('+') {
        return Some(MacroParamRefKind::Deref);
    }
    None
}

/// Zig `macroParamLineAssigns`: `%param = ...` (not `==`).
fn macro_param_line_assigns(line: &str, param: &str) -> bool {
    if !line.starts_with('%') {
        return false;
    }
    let mut rest = &line[1..];
    if !rest.starts_with(param) {
        return false;
    }
    rest = &rest[param.len()..];
    if rest.chars().next().is_some_and(|c| {
        c.is_ascii_alphanumeric() || c == '_' || c == '%'
    }) {
        return false;
    }
    rest = rest.trim_start_matches(|c| c == ' ' || c == '\t');
    rest.starts_with('=') && (rest.len() == 1 || !rest[1..].starts_with('='))
}

fn tokenize_any<'a>(text: &'a str, delims: &[char]) -> Vec<&'a str> {
    text.split(|c| delims.contains(&c))
        .filter(|t| !t.is_empty())
        .collect()
}

/// Zig `markAssignedOutputMacroParams`. `ref_state` is per-parameter
/// scan state: 0 unseen, 1 assigned before any memory reference,
/// 2 tainted (a deref/address-take preceded the first assignment).
pub fn mark_assigned_output_macro_params(
    registry: &ImportedMacroRegistry,
    param_names: &[String],
    ref_state: &mut [u8],
    line: &str,
) {
    let is_expand = line.starts_with("EXPAND");
    let mut expanded_assigned: u64 = 0;
    let mut expanded_memory: u64 = 0;
    if is_expand {
        let mut head = tokenize_any(&line["EXPAND".len()..], &[' ', '\t', ',', '(', ')']).into_iter();
        if let Some(expanded_name) = head.next() {
            if let Some(expanded) = registry.get(expanded_name) {
                expanded_assigned = expanded.assigned_output_mask;
                expanded_memory =
                    expanded.borrowed_arg_mask | expanded.address_slot_arg_mask;
            }
        }
    }
    for (idx, param) in param_names.iter().enumerate() {
        if ref_state[idx] == 2 {
            continue;
        }
        let lhs_assign = !is_expand && macro_param_line_assigns(line, param);
        let mut pending_assign = false;
        let tokens = tokenize_any(line, &[' ', '\t', ',', '(', ')']);
        for (token_idx, token) in tokens.iter().enumerate() {
            let kind = match classify_macro_param_token(token, param) {
                Some(k) => k,
                None => continue,
            };
            if lhs_assign && token_idx == 0 {
                pending_assign = true;
                continue;
            }
            if is_expand && token_idx >= 2 {
                let arg_pos = token_idx - 2;
                if arg_pos < 64 {
                    let bit = 1u64 << arg_pos;
                    if expanded_assigned & bit != 0 {
                        if ref_state[idx] == 0 {
                            ref_state[idx] = 1;
                        }
                        continue;
                    }
                    if expanded_memory & bit != 0 {
                        if ref_state[idx] == 0 {
                            ref_state[idx] = 2;
                        }
                        continue;
                    }
                }
            }
            match kind {
                MacroParamRefKind::Value => {}
                MacroParamRefKind::Deref | MacroParamRefKind::AddrTake => {
                    if ref_state[idx] == 0 {
                        ref_state[idx] = 2;
                    }
                }
            }
        }
        if pending_assign && ref_state[idx] == 0 {
            ref_state[idx] = 1;
        }
    }
}

/// Zig `markExpandedImportedMacroParamMasks`.
pub fn mark_expanded_imported_macro_param_masks(
    registry: &ImportedMacroRegistry,
    borrowed_mask: &mut u64,
    address_slot_mask: &mut u64,
    param_names: &[String],
    line: &str,
) {
    if !line.starts_with("EXPAND") {
        return;
    }
    let mut parts = tokenize_any(&line["EXPAND".len()..], &[' ', '\t', ',']).into_iter();
    let expanded_name = match parts.next() {
        Some(n) => n,
        None => return,
    };
    let expanded = match registry.get(expanded_name) {
        Some(e) => e,
        None => return,
    };
    for (arg_idx, raw_arg) in parts.enumerate() {
        if arg_idx >= 64 {
            continue;
        }
        let trimmed = raw_arg.trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == ',');
        if trimmed.is_empty() || !trimmed.starts_with('%') {
            continue;
        }
        let arg_bit = 1u64 << arg_idx;
        if expanded.borrowed_arg_mask & arg_bit != 0 {
            mark_borrowed_param(borrowed_mask, param_names, trimmed);
        }
        if expanded.address_slot_arg_mask & arg_bit != 0 {
            mark_borrowed_param(address_slot_mask, param_names, trimmed);
        }
    }
}

/// Zig `importedMacroCalleeName`.
pub fn imported_macro_callee_name(raw: &str) -> String {
    let trimmed = raw.trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == '\n' || c == '"');
    let without_at = trimmed.strip_prefix('@').unwrap_or(trimmed);
    let source_name = without_at.strip_prefix("sla__").unwrap_or(without_at);
    source_name.to_string()
}

/// Zig `appendUniqueDirectCallee`.
pub fn append_unique_direct_callee(callees: &mut Vec<String>, name: &str) {
    if callees.iter().any(|c| c == name) {
        return;
    }
    callees.push(name.to_string());
}

/// Zig `collectDirectSlaMacroCallees`.
pub fn collect_direct_sla_macro_callees(callees: &mut Vec<String>, line: &str) {
    let mut rest = line;
    while let Some(idx) = rest.find("call @") {
        let start = idx + "call @".len();
        let mut end = start;
        for (off, c) in rest[start..].char_indices() {
            if c.is_ascii_alphanumeric() || c == '_' || c == ':' {
                end = start + off + c.len_utf8();
            } else {
                break;
            }
        }
        if end > start {
            let name = imported_macro_callee_name(&rest[start..end]);
            append_unique_direct_callee(callees, &name);
        }
        rest = &rest[end..];
    }
}

/// Zig `appendExpandedImportedMacroDirectCallees`.
pub fn append_expanded_imported_macro_direct_callees(
    registry: &ImportedMacroRegistry,
    callees: &mut Vec<String>,
    line: &str,
) {
    if !line.starts_with("EXPAND") {
        return;
    }
    let mut parts = tokenize_any(&line["EXPAND".len()..], &[' ', '\t', ',']).into_iter();
    let expanded_name = match parts.next() {
        Some(n) => n,
        None => return,
    };
    if let Some(expanded) = registry.get(expanded_name) {
        for callee in &expanded.direct_callees {
            append_unique_direct_callee(callees, callee);
        }
    }
}

/// Zig `loadImportedMacrosFromExpandedSource`, minus the disk memo
/// (IMP-6). Scans `[MACRO]` blocks in source order; each finished
/// macro is registered into `registry` before the next is scanned,
/// so `EXPAND` lines resolve against earlier macros exactly as
/// against Zig's checker table. Returns the records defined by THIS
/// source (in definition order).
pub fn load_imported_macros_from_expanded_source(
    expanded_source: &str,
    import_path: Option<&str>,
    registry: &mut ImportedMacroRegistry,
) -> Vec<ImportedMacroRecord> {
    let mut defined = Vec::new();
    if !expanded_source_may_contain_imported_macros(expanded_source) {
        return defined;
    }
    let lines: Vec<&str> = expanded_source.split('\n').collect();
    let mut li = 0usize;
    while li < lines.len() {
        let line = lines[li].trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
        li += 1;
        if !line.starts_with("[MACRO]") {
            continue;
        }
        let mut parts = tokenize_any(&line["[MACRO]".len()..], &[' ', '\t']).into_iter();
        let raw_name = match parts.next() {
            Some(n) => n,
            None => continue,
        };
        let name = raw_name
            .trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == ',')
            .to_string();

        let mut param_names: Vec<String> = Vec::new();
        let mut leading_outputs = 0usize;
        let mut still_leading = true;
        for raw_param in parts {
            let param = macro_param_name(raw_param);
            if param.is_empty() {
                continue;
            }
            param_names.push(param.to_string());
            if still_leading && is_leading_output_macro_param(raw_param) {
                leading_outputs += 1;
            } else {
                still_leading = false;
            }
        }
        let arity = param_names.len();

        let mut borrowed_arg_mask: u64 = 0;
        let mut address_slot_arg_mask: u64 = 0;
        let mut ref_state = vec![0u8; param_names.len()];
        let mut direct_callees: Vec<String> = Vec::new();
        while li < lines.len() {
            let body_line = lines[li].trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
            li += 1;
            if body_line.starts_with("[END_MACRO]") {
                break;
            }
            mark_direct_borrowed_macro_params(&mut borrowed_arg_mask, &param_names, body_line);
            mark_direct_address_slot_macro_params(
                &mut address_slot_arg_mask,
                &param_names,
                body_line,
            );
            mark_expanded_imported_macro_param_masks(
                registry,
                &mut borrowed_arg_mask,
                &mut address_slot_arg_mask,
                &param_names,
                body_line,
            );
            mark_assigned_output_macro_params(registry, &param_names, &mut ref_state, body_line);
            collect_direct_sla_macro_callees(&mut direct_callees, body_line);
            append_expanded_imported_macro_direct_callees(registry, &mut direct_callees, body_line);
        }
        let mut assigned_output_mask: u64 = 0;
        for (idx, state) in ref_state.iter().enumerate() {
            if *state == 1 && idx < 64 {
                assigned_output_mask |= 1u64 << idx;
            }
        }
        let record = ImportedMacroRecord {
            name,
            arity,
            leading_outputs,
            import_path: import_path.map(str::to_string),
            borrowed_arg_mask,
            address_slot_arg_mask,
            assigned_output_mask,
            direct_callees,
        };
        registry.put(record.clone());
        defined.push(record);
    }
    defined
}

/// Zig `loadImportedMacros`: expand (source_expand::expand) then
/// load. An expansion failure aborts the load, as in Zig.
pub fn load_imported_macros(
    source: &str,
    import_path: Option<&str>,
    registry: &mut ImportedMacroRegistry,
) -> Result<Vec<ImportedMacroRecord>, crate::source_expand::SourceExpandError> {
    let expanded = crate::source_expand::expand(source)?;
    Ok(load_imported_macros_from_expanded_source(
        &expanded,
        import_path,
        registry,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(source: &str) -> Vec<ImportedMacroRecord> {
        let mut registry = ImportedMacroRegistry::new();
        let defined = load_imported_macros_from_expanded_source(source, Some("m.sa"), &mut registry);
        assert_eq!(defined.len(), registry.len());
        defined
    }

    #[test]
    fn arity_and_leading_outputs() {
        // Venue probe shapes (docs/probes): SET_PAIR has arity 3 and
        // TWO leading outputs; GET_ONE arity 2, one leading output.
        let recs = load("[MACRO] SET_PAIR %out_a, %out_b, %val\n    %out_a = copy %val\n    %out_b = copy %val\n[END_MACRO]\n[MACRO] GET_ONE %out, %val\n    %out = copy %val\n[END_MACRO]\n");
        assert_eq!(recs[0].name, "SET_PAIR");
        assert_eq!(recs[0].arity, 3);
        assert_eq!(recs[0].leading_outputs, 2);
        assert_eq!(recs[1].arity, 2);
        assert_eq!(recs[1].leading_outputs, 1);
        assert_eq!(recs[0].import_path.as_deref(), Some("m.sa"));
        // Both outputs are assigned before any memory reference.
        assert_eq!(recs[0].assigned_output_mask, 0b011);
        assert_eq!(recs[1].assigned_output_mask, 0b001);
        assert_eq!(recs[0].borrowed_arg_mask, 0);
    }

    #[test]
    fn borrowed_and_address_slot_masks() {
        let recs = load("[MACRO] M %out, %ptr, %val\n    store %ptr+4, %val\n    call @sa_mem_set(&%ptr, %val, 1)\n[END_MACRO]\n");
        // %ptr used as %ptr+ (address slot, bit 1) and &%ptr (borrowed, bit 1).
        assert_eq!(recs[0].address_slot_arg_mask, 0b010);
        assert_eq!(recs[0].borrowed_arg_mask, 0b010);
        // Direct callee: the `sla__` prefix rule does not apply, `@` stripped.
        assert_eq!(recs[0].direct_callees, vec!["sa_mem_set".to_string()]);
        // %ptr was address-referenced, so it is not an assigned output;
        // %out never assigned in this body.
        assert_eq!(recs[0].assigned_output_mask, 0);
    }

    #[test]
    fn address_slot_inside_synthetic_register_name_does_not_count() {
        // The D3 shape from the Zig comment: `%index` inside a longer
        // synthetic register name followed by `+Field` is NOT the
        // parameter used as an address base.
        let recs = load("[MACRO] M %out, %index\n    store __slot_%index+Data, 1\n[END_MACRO]\n");
        assert_eq!(recs[0].address_slot_arg_mask, 0);
    }

    #[test]
    fn expand_propagates_masks_and_callees() {
        let mut registry = ImportedMacroRegistry::new();
        load_imported_macros_from_expanded_source(
            "[MACRO] INNER %out, %ptr\n    store %ptr+0, 1\n[END_MACRO]\n",
            Some("inner.sa"),
            &mut registry,
        );
        let recs = load_imported_macros_from_expanded_source(
            "[MACRO] OUTER %out, %p\n    EXPAND INNER %out, %p\n[END_MACRO]\n",
            Some("outer.sa"),
            &mut registry,
        );
        // INNER's address-slot bit 1 maps onto OUTER's argument 1 (%p).
        assert_eq!(recs[0].address_slot_arg_mask, 0b010);
    }

    #[test]
    fn callee_name_strips_at_and_sla_prefix() {
        assert_eq!(imported_macro_callee_name("@sla__helper"), "helper");
        assert_eq!(imported_macro_callee_name("\"@plain\""), "plain");
        let recs = load("[MACRO] M %out\n    call @sla__helper()\n    call @sla__helper()\n[END_MACRO]\n");
        assert_eq!(recs[0].direct_callees, vec!["helper".to_string()]);
    }

    #[test]
    fn no_macro_source_registers_nothing() {
        let mut registry = ImportedMacroRegistry::new();
        let defined =
            load_imported_macros_from_expanded_source("fn main() {}\n", None, &mut registry);
        assert!(defined.is_empty());
        assert!(registry.is_empty());
    }

    #[test]
    fn leading_run_stops_at_first_non_output() {
        let recs = load("[MACRO] M %val, %out_x\n    %out_x = copy %val\n[END_MACRO]\n");
        assert_eq!(recs[0].leading_outputs, 0);
        assert_eq!(recs[0].arity, 2);
    }
}

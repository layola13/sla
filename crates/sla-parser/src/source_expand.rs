//! Source-level `@expand_tuple` expansion, mirrored from
//! `sa_plugin_sla/src/source_expand.zig` (468 lines at the box head
//! 2cc12ea8; the local read-only tree carries the 450-line ancestor).
//!
//! Zig entry points: `expand` (pure) and `expandForModulePath`
//! (same expansion behind a `.sla-cache/expand/` disk memo). The
//! disk memo is NOT ported: it is a pure cache of a deterministic
//! function (key = compiler stamp + module path + source), so
//! skipping it changes only speed, never output. Registered as
//! IMP-6 in docs/divergences/import-expand.md. The Wyhash key
//! construction also cannot be reproduced without a new dependency,
//! which the crate graph forbids on this lane.
//!
//! Everything else is mirrored byte-for-byte, including the quirks:
//!   * FB-045 (Rust-side fix, intentional divergence): the top-level
//!     scan is lexically aware — string literals and `//` comments
//!     are skipped, so directive-shaped text inside them is copied
//!     verbatim. Zig's raw scan matches `@expand_tuple` ANYWHERE in
//!     the byte stream (inside strings/comments too) and thereby
//!     rejects valid strings / silently rewrites their values;
//!     the Rust product line does not mirror that defect;
//!   * `@join` separators are the raw bytes between the quotes (no
//!     escape processing);
//!   * `$TYPE_PARAMS` is tried before `$TYPES` at the same spot;
//!   * `$ORD` is defined only for indices 0..=15;
//!   * brace matching skips `//` line comments and strings, paren
//!     matching skips strings only.

/// Mirror of Zig `SourceExpandError` (OutOfMemory has no Rust
/// analogue and is dropped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceExpandError {
    InvalidExpandTuple,
}

impl std::fmt::Display for SourceExpandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceExpandError::InvalidExpandTuple => write!(f, "error.InvalidExpandTuple"),
        }
    }
}

impl std::error::Error for SourceExpandError {}

type Result<T> = std::result::Result<T, SourceExpandError>;
use SourceExpandError::InvalidExpandTuple;

#[derive(Debug, Clone, Copy)]
struct Directive {
    args_start: usize,
    args_end: usize,
    body_start: usize,
    body_end: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy)]
struct TupleContext<'a> {
    arity: usize,
    type_prefix: &'a [u8],
    current_index: Option<usize>,
}

/// Zig `expand`.
pub fn expand(source: &str) -> Result<String> {
    let bytes = source.as_bytes();
    if !contains(bytes, b"@expand_tuple") {
        return Ok(source.to_string());
    }
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() * 2);
    let mut i = 0usize;
    while i < bytes.len() {
        // FB-045: skip string literals and `//` comments verbatim —
        // their bytes are data, never directives. (Unterminated
        // string: copy the rest verbatim; the parser diagnoses it.)
        if bytes[i] == b'"' {
            let end = skip_string(bytes, i).map(|close| close + 1).unwrap_or(bytes.len());
            out.extend_from_slice(&bytes[i..end]);
            i = end;
            continue;
        }
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            let end = skip_line(bytes, i + 2);
            out.extend_from_slice(&bytes[i..end]);
            i = end;
            continue;
        }
        if starts_with_at(bytes, i, b"@expand_tuple") {
            let directive = parse_directive(bytes, i, b"@expand_tuple")?;
            let spec = parse_expand_tuple_args(&bytes[directive.args_start..directive.args_end])?;
            let body = &bytes[directive.body_start..directive.body_end];
            let mut n = spec.min;
            loop {
                let ctx = TupleContext {
                    arity: n,
                    type_prefix: spec.type_prefix,
                    current_index: None,
                };
                let expanded = expand_template(body, ctx)?;
                out.extend_from_slice(&expanded);
                if expanded.is_empty() || expanded[expanded.len() - 1] != b'\n' {
                    out.push(b'\n');
                }
                if n == spec.max {
                    break;
                }
                n += 1;
            }
            i = directive.end;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// Zig `expandForModulePath`: identical output to [`expand`]; the
/// disk memo layer is omitted (see module docs, IMP-6). The module
/// path parameter exists for signature parity.
pub fn expand_for_module_path(_module_path: &str, source: &str) -> Result<String> {
    expand(source)
}

struct ExpandTupleSpec<'a> {
    min: usize,
    max: usize,
    type_prefix: &'a [u8],
}

/// Zig `parseExpandTupleArgs`: exactly three comma-separated parts.
fn parse_expand_tuple_args(args: &[u8]) -> Result<ExpandTupleSpec<'_>> {
    let mut parts = args.split(|&c| c == b',');
    let min_part = trim(parts.next().ok_or(InvalidExpandTuple)?);
    let max_part = trim(parts.next().ok_or(InvalidExpandTuple)?);
    let prefix_part = trim(parts.next().ok_or(InvalidExpandTuple)?);
    if parts.next().is_some() {
        return Err(InvalidExpandTuple);
    }
    if prefix_part.is_empty() || !is_ident_start(prefix_part[0]) {
        return Err(InvalidExpandTuple);
    }
    if prefix_part[1..].iter().any(|&c| !is_ident_continue(c)) {
        return Err(InvalidExpandTuple);
    }
    let min = parse_usize(min_part)?;
    let max = parse_usize(max_part)?;
    if min > max {
        return Err(InvalidExpandTuple);
    }
    Ok(ExpandTupleSpec {
        min,
        max,
        type_prefix: prefix_part,
    })
}

fn parse_usize(part: &[u8]) -> Result<usize> {
    let text = std::str::from_utf8(part).map_err(|_| InvalidExpandTuple)?;
    text.parse::<usize>().map_err(|_| InvalidExpandTuple)
}

/// Zig `expandTemplate` (recursive: `@each` / `@join` blocks).
fn expand_template(template: &[u8], ctx: TupleContext) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0usize;
    while i < template.len() {
        if starts_with_at(template, i, b"@each") {
            let directive = parse_directive(template, i, b"@each")?;
            let name = trim(&template[directive.args_start..directive.args_end]);
            if name != ctx.type_prefix {
                return Err(InvalidExpandTuple);
            }
            let body = &template[directive.body_start..directive.body_end];
            for idx in 0..ctx.arity {
                let child = TupleContext {
                    current_index: Some(idx),
                    ..ctx
                };
                let expanded = expand_template(body, child)?;
                out.extend_from_slice(&expanded);
            }
            i = directive.end;
            continue;
        }
        if starts_with_at(template, i, b"@join") {
            let directive = parse_directive(template, i, b"@join")?;
            let join = parse_join_args(&template[directive.args_start..directive.args_end])?;
            if join.name != ctx.type_prefix {
                return Err(InvalidExpandTuple);
            }
            let body = &template[directive.body_start..directive.body_end];
            for idx in 0..ctx.arity {
                if idx != 0 {
                    out.extend_from_slice(join.separator);
                }
                let child = TupleContext {
                    current_index: Some(idx),
                    ..ctx
                };
                let expanded = expand_template(body, child)?;
                out.extend_from_slice(trim(&expanded));
            }
            i = directive.end;
            continue;
        }
        if !append_placeholder(&mut out, template, &mut i, ctx)? {
            out.push(template[i]);
            i += 1;
        }
    }
    Ok(out)
}

/// Zig `appendPlaceholder`. Order matters and is Zig's: $TYPE_PARAMS
/// / $TYPES, $N, $I, $ORD, $<prefix>, $T.
fn append_placeholder(
    out: &mut Vec<u8>,
    text: &[u8],
    index: &mut usize,
    ctx: TupleContext,
) -> Result<bool> {
    let i = *index;
    if text[i] != b'$' {
        return Ok(false);
    }
    if starts_with_at(text, i, b"$TYPE_PARAMS") || starts_with_at(text, i, b"$TYPES") {
        for type_index in 0..ctx.arity {
            if type_index != 0 {
                out.extend_from_slice(b", ");
            }
            push_type_name(out, ctx.type_prefix, type_index);
        }
        *index += if starts_with_at(text, i, b"$TYPE_PARAMS") {
            "$TYPE_PARAMS".len()
        } else {
            "$TYPES".len()
        };
        return Ok(true);
    }
    if starts_with_at(text, i, b"$N") {
        out.extend_from_slice(ctx.arity.to_string().as_bytes());
        *index += 2;
        return Ok(true);
    }
    if starts_with_at(text, i, b"$I") {
        let idx = ctx.current_index.ok_or(InvalidExpandTuple)?;
        out.extend_from_slice(idx.to_string().as_bytes());
        *index += 2;
        return Ok(true);
    }
    if starts_with_at(text, i, b"$ORD") {
        let idx = ctx.current_index.ok_or(InvalidExpandTuple)?;
        out.extend_from_slice(ordinal_name(idx).ok_or(InvalidExpandTuple)?);
        *index += "$ORD".len();
        return Ok(true);
    }
    if starts_with_at(text, i + 1, ctx.type_prefix)
        && placeholder_boundary(text, i + 1 + ctx.type_prefix.len())
    {
        let type_index = ctx.current_index.ok_or(InvalidExpandTuple)?;
        push_type_name(out, ctx.type_prefix, type_index);
        *index += 1 + ctx.type_prefix.len();
        return Ok(true);
    }
    if starts_with_at(text, i, b"$T") {
        let type_index = ctx.current_index.ok_or(InvalidExpandTuple)?;
        push_type_name(out, ctx.type_prefix, type_index);
        *index += 2;
        return Ok(true);
    }
    Ok(false)
}

fn push_type_name(out: &mut Vec<u8>, prefix: &[u8], index: usize) {
    out.extend_from_slice(prefix);
    out.extend_from_slice(index.to_string().as_bytes());
}

struct JoinArgs<'a> {
    name: &'a [u8],
    separator: &'a [u8],
}

/// Zig `parseJoinArgs`.
fn parse_join_args(args: &[u8]) -> Result<JoinArgs<'_>> {
    let comma = args.iter().position(|&c| c == b',').ok_or(InvalidExpandTuple)?;
    let name = trim(&args[..comma]);
    let raw_sep = trim(&args[comma + 1..]);
    if raw_sep.len() < 2 || raw_sep[0] != b'"' || raw_sep[raw_sep.len() - 1] != b'"' {
        return Err(InvalidExpandTuple);
    }
    Ok(JoinArgs {
        name,
        separator: &raw_sep[1..raw_sep.len() - 1],
    })
}

/// Zig `parseDirective`.
fn parse_directive(source: &[u8], start: usize, name: &[u8]) -> Result<Directive> {
    let mut i = skip_whitespace(source, start + name.len());
    if i >= source.len() || source[i] != b'(' {
        return Err(InvalidExpandTuple);
    }
    let args_start = i + 1;
    let args_end = find_matching_paren(source, i).ok_or(InvalidExpandTuple)?;
    i = skip_whitespace(source, args_end + 1);
    if i >= source.len() || source[i] != b'{' {
        return Err(InvalidExpandTuple);
    }
    let body_end = find_matching_brace(source, i).ok_or(InvalidExpandTuple)?;
    Ok(Directive {
        args_start,
        args_end,
        body_start: i + 1,
        body_end,
        end: body_end + 1,
    })
}

/// Zig `findMatchingParen` (strings skipped, comments not).
fn find_matching_paren(source: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < source.len() {
        match source[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'"' => i = skip_string(source, i)?,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Zig `findMatchingBrace` (strings and `//` comments skipped).
fn find_matching_brace(source: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < source.len() {
        if source[i] == b'/' && i + 1 < source.len() && source[i + 1] == b'/' {
            i = skip_line(source, i + 2);
            continue;
        }
        match source[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'"' => i = skip_string(source, i)?,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Zig `skipString`: returns the index of the closing quote.
fn skip_string(source: &[u8], quote: usize) -> Option<usize> {
    let mut i = quote + 1;
    while i < source.len() {
        if source[i] == b'\\' {
            i += 1;
        } else if source[i] == b'"' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn skip_line(source: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < source.len() && source[i] != b'\n' {
        i += 1;
    }
    i
}

fn skip_whitespace(source: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < source.len() && source[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn trim(value: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = value.len();
    while start < end && matches!(value[start], b' ' | b'\t' | b'\r' | b'\n') {
        start += 1;
    }
    while end > start && matches!(value[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
        end -= 1;
    }
    &value[start..end]
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle)
}

fn starts_with_at(source: &[u8], index: usize, needle: &[u8]) -> bool {
    index + needle.len() <= source.len() && &source[index..index + needle.len()] == needle
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

fn placeholder_boundary(source: &[u8], index: usize) -> bool {
    index >= source.len() || !is_ident_continue(source[index])
}

/// Zig `ordinalName`: 0..=15 only.
fn ordinal_name(index: usize) -> Option<&'static [u8]> {
    Some(match index {
        0 => b"first",
        1 => b"second",
        2 => b"third",
        3 => b"fourth",
        4 => b"fifth",
        5 => b"sixth",
        6 => b"seventh",
        7 => b"eighth",
        8 => b"ninth",
        9 => b"tenth",
        10 => b"eleventh",
        11 => b"twelfth",
        12 => b"thirteenth",
        13 => b"fourteenth",
        14 => b"fifteenth",
        15 => b"sixteenth",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The Zig unit tests of source_expand.zig, ported
    // assertion-for-assertion (substring checks as in the original).

    #[test]
    fn expand_tuple_arity_template() {
        let source = "@expand_tuple(2, 3, T) {\nstruct Generated$N<$TYPES> {\n@each(T) {\n    value_$I: $T,\n}\n}\nfn make$N<@join(T, \", \") { $T }>(@join(T, \", \") { value_$I: $T }) -> i64 {\n    let sum = 0;\n@each(T) {\n    sum = sum + value_$I;\n}\n    return sum;\n}\n}\n";
        let expanded = expand(source).unwrap();
        assert!(expanded.contains("struct Generated2<T0, T1>"));
        assert!(expanded.contains("value_2: T2"));
        assert!(expanded.contains("fn make3<T0, T1, T2>"));
    }

    #[test]
    fn expand_without_tuple_directive_returns_copy() {
        let source = "fn main() -> i32 { return 1; }";
        assert_eq!(expand(source).unwrap(), source);
    }

    #[test]
    fn expand_tuple_ordinal_placeholder() {
        let source = "@expand_tuple(2, 3, T) {\nstruct Ordinal$N<$TYPES> {\n@each(T) {\n    $ORD: $T,\n}\n}\n}\n";
        let expanded = expand(source).unwrap();
        assert!(expanded.contains("first: T0"));
        assert!(expanded.contains("second: T1"));
        assert!(expanded.contains("third: T2"));
    }

    #[test]
    fn expand_tuple_current_type_placeholder_follows_prefix() {
        let source = "@expand_tuple(1, 2, P) {\nstruct ParamSet$N<@join(P, \", \") { $P }> {\n@each(P) {\n    p$I: $P,\n}\n}\n}\n";
        let expanded = expand(source).unwrap();
        assert!(expanded.contains("struct ParamSet1<P0>"));
        assert!(expanded.contains("p0: P0"));
        assert!(expanded.contains("struct ParamSet2<P0, P1>"));
        assert!(expanded.contains("p1: P1"));
    }

    #[test]
    fn expand_tuple_processes_multiple_top_level_directives() {
        let source = "@expand_tuple(2, 2, T) {\nstruct First$N<$TYPES> {}\n}\n@expand_tuple(3, 3, T) {\nstruct Second$N<$TYPES> {}\n}\n";
        let expanded = expand(source).unwrap();
        assert!(expanded.contains("struct First2<T0, T1>"));
        assert!(expanded.contains("struct Second3<T0, T1, T2>"));
        assert!(!expanded.contains("@expand_tuple"));
    }

    #[test]
    fn expand_for_module_path_is_deterministic() {
        // Zig's cache test, restated without the disk memo (IMP-6):
        // two calls over the same module path and source agree, and
        // both carry the expansion.
        let source = "@expand_tuple(1, 2, T) {\nfn f_$N() -> i32 { return $N; }\n}\n";
        let first = expand_for_module_path("unit_expand_cache_mod.sla", source).unwrap();
        let second = expand_for_module_path("unit_expand_cache_mod.sla", source).unwrap();
        assert!(first.contains("fn f_1"));
        assert!(first.contains("fn f_2"));
        assert_eq!(first, second);
    }

    #[test]
    fn expand_tuple_inside_string_or_comment_is_not_a_directive() {
        // FB-045 (Rust-side fix): the top-level scan is lexically
        // aware — directive-shaped text inside a string literal or
        // a `//` comment is copied verbatim, never expanded, never
        // an error. (Zig's raw scan rejects/rewrites these; the
        // Rust product line does not mirror that defect.)
        let a = "fn main() -> i32 { let s = \"@expand_tuple(x)\"; return 0; }\n";
        assert_eq!(expand(a), Ok(a.to_string()));
        let b = "fn main() -> i32 { let s = \"@expand_tuple(1, 1, T) { not a directive }\"; return 0; }\n";
        assert_eq!(expand(b), Ok(b.to_string()));
        let c = "// @expand_tuple(x)\nfn main() -> i32 { return 0; }\n";
        assert_eq!(expand(c), Ok(c.to_string()));
        // An escaped quote does not end the string early.
        let d = "fn f() -> i32 { let s = \"a\\\"@expand_tuple(x)\"; return 0; }\n";
        assert_eq!(expand(d), Ok(d.to_string()));
        // A real directive after a string still expands.
        let e = "fn f() -> i32 { let s = \"@expand_tuple\"; return 0; }\n@expand_tuple(1, 1, T) {\nfn g_$N() -> i32 { return $N; }\n}\n";
        let expanded = expand(e).unwrap();
        assert!(expanded.contains("\"@expand_tuple\""));
        assert!(expanded.contains("fn g_1"));
    }

    #[test]
    fn invalid_specs_are_loud() {
        // min > max (venue probe: rc=1, error.InvalidExpandTuple).
        assert_eq!(
            expand("@expand_tuple(3, 2, T) {\nfn f() {}\n}\n"),
            Err(InvalidExpandTuple)
        );
        // Four argument parts.
        assert_eq!(
            expand("@expand_tuple(1, 2, T, U) {\nfn f() {}\n}\n"),
            Err(InvalidExpandTuple)
        );
        // $I outside @each/@join has no current index.
        assert_eq!(
            expand("@expand_tuple(1, 1, T) {\nfn f_$I() {}\n}\n"),
            Err(InvalidExpandTuple)
        );
        // @each over a different prefix than the directive's.
        assert_eq!(
            expand("@expand_tuple(1, 1, T) {\n@each(P) {\nx\n}\n}\n"),
            Err(InvalidExpandTuple)
        );
    }
}

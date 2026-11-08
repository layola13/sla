//! `.sai` / `.sal` contract parsing, mirrored line-for-line from
//! `sa_plugin_sla/src/contract_parser.zig` (223 lines).
//!
//! This module is pure text -> declarations: no filesystem IO (the
//! loading orchestration lives in `contract_load`, the path
//! resolution in `imports`). The declarations produced here are the
//! "ast-layer contract declarations" of coordination ruling D-A2:
//! they flow through the plugin wiring layer, which converts them
//! into `sla_checker::ExternSig` registrations.
//!
//! Zig quirks mirrored on purpose (no improvements; see
//! docs/divergences/contracts.md):
//!   * parameter lists are split on EVERY ',' (`splitScalar`), so a
//!     parameter type containing a comma breaks exactly as in Zig;
//!   * a parameter is split on ':' and only the first two segments
//!     are read — further ':' segments are silently dropped;
//!   * the return type is the trimmed REST OF THE LINE after `->`;
//!   * a `#def` value that does not parse as i64 becomes 0
//!     (`catch 0` in `parseDefLine`).

/// One contract parameter (Zig `contract_parser.Param`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractParam {
    pub name: String,
    pub ty: String,
    pub is_borrow: bool,
    pub is_move: bool,
}

/// One `@extern` declaration (Zig `contract_parser.ExternalFunction`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFunction {
    pub name: String,
    pub params: Vec<ContractParam>,
    pub ret_ty: String,
    pub return_fallible: bool,
    /// R-3: the parameter list ends with a C-variadic `...`.
    pub is_variadic: bool,
}

/// One `#def` layout define (Zig `contract_parser.LayoutDefine`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutDefine {
    pub name: String,
    pub val_str: String,
    pub val_int: i64,
}

/// Mirror of the Zig contract-parser error set: `error.SaiParseError`
/// / `error.SalParseError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractParseError {
    SaiParseError,
    SalParseError,
}

impl std::fmt::Display for ContractParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractParseError::SaiParseError => write!(f, "SaiParseError"),
            ContractParseError::SalParseError => write!(f, "SalParseError"),
        }
    }
}

impl std::error::Error for ContractParseError {}

const TRIM_CHARS: &[char] = &[' ', '\t', '\r'];

fn trim(s: &str) -> &str {
    s.trim_matches(|c| TRIM_CHARS.contains(&c))
}

fn trim_space_tab(s: &str) -> &str {
    s.trim_matches(|c| c == ' ' || c == '\t')
}

/// Zig `ContractParser.parseSai`: collect one `ExternalFunction` per
/// line starting with `@extern`; blank lines and `//` comments are
/// skipped. Parsing an empty string yields an empty list (Zig's
/// `loadContracts` relies on this for the absent half of a pair).
pub fn parse_sai(content: &str) -> Result<Vec<ExternalFunction>, ContractParseError> {
    let mut functions = Vec::new();
    for raw_line in content.split('\n') {
        let line = trim(raw_line);
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("@extern") {
            functions.push(parse_extern_line(line)?);
        }
    }
    Ok(functions)
}

/// Zig `parseExternLine`. The reference line shape is:
/// `@extern sa_node_plugin_os_cpus(&out_ptr: ptr, &out_len: ptr) -> u32`
fn parse_extern_line(line: &str) -> Result<ExternalFunction, ContractParseError> {
    let bytes = line.as_bytes();
    let mut index = "@extern".len();
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    let name_start = index;
    while index < bytes.len() && bytes[index] != b'(' && bytes[index] != b' ' && bytes[index] != b'\t'
    {
        index += 1;
    }
    let name = &line[name_start..index];

    while index < bytes.len() && bytes[index] != b'(' {
        index += 1;
    }
    if index >= bytes.len() {
        return Err(ContractParseError::SaiParseError);
    }
    index += 1; // skip '('

    let params_start = index;
    let mut paren_count: usize = 1;
    while index < bytes.len() {
        if bytes[index] == b'(' {
            paren_count += 1;
        } else if bytes[index] == b')' {
            paren_count -= 1;
            if paren_count == 0 {
                break;
            }
        }
        index += 1;
    }
    if index >= bytes.len() {
        return Err(ContractParseError::SaiParseError);
    }
    let params_str = &line[params_start..index];
    index += 1; // skip ')'

    // Optional `-> ret` (a trailing `!` marks the return fallible).
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    let mut ret_ty: &str = "void";
    let mut return_fallible = false;
    if index + 2 <= bytes.len() && &line[index..index + 2] == "->" {
        index += 2;
        while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
            index += 1;
        }
        ret_ty = trim_space_tab(&line[index..]);
        if ret_ty.ends_with('!') {
            return_fallible = true;
            ret_ty = ret_ty[..ret_ty.len() - 1].trim_end_matches(|c| c == ' ' || c == '\t');
        }
    }

    // Parameter list: `splitScalar ','`, then per parameter
    // `splitScalar ':'` reading only name and type segments.
    let mut params = Vec::new();
    let mut is_variadic = false;
    for p_raw in params_str.split(',') {
        let p_trim = trim_space_tab(p_raw);
        if p_trim.is_empty() {
            continue;
        }
        // R-3: a bare `...` segment is the C-variadic marker, not
        // a name:type parameter.
        if p_trim == "..." {
            is_variadic = true;
            continue;
        }
        let mut colon_split = p_trim.split(':');
        let p_name_raw = trim_space_tab(
            colon_split
                .next()
                .ok_or(ContractParseError::SaiParseError)?,
        );
        let p_ty = trim_space_tab(
            colon_split
                .next()
                .ok_or(ContractParseError::SaiParseError)?,
        );
        let mut is_borrow = false;
        let mut is_move = false;
        let mut p_name = p_name_raw;
        if let Some(stripped) = p_name.strip_prefix('&') {
            is_borrow = true;
            p_name = stripped;
        } else if let Some(stripped) = p_name.strip_prefix('^') {
            is_move = true;
            p_name = stripped;
        }
        params.push(ContractParam {
            name: p_name.to_string(),
            ty: p_ty.to_string(),
            is_borrow,
            is_move,
        });
    }

    Ok(ExternalFunction {
        name: name.to_string(),
        params,
        ret_ty: ret_ty.to_string(),
        return_fallible,
        is_variadic,
    })
}

/// Zig `ContractParser.parseSal`: collect one `LayoutDefine` per line
/// starting with `#def`.
pub fn parse_sal(content: &str) -> Result<Vec<LayoutDefine>, ContractParseError> {
    let mut defines = Vec::new();
    for raw_line in content.split('\n') {
        let line = trim(raw_line);
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("#def") {
            defines.push(parse_def_line(line)?);
        }
    }
    Ok(defines)
}

/// Zig `parseDefLine`. Reference shapes: `#def RcBox_SIZE = 24`,
/// `#def RcBox_strong = +0`.
fn parse_def_line(line: &str) -> Result<LayoutDefine, ContractParseError> {
    let bytes = line.as_bytes();
    let mut index = "#def".len();
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    let name_start = index;
    while index < bytes.len() && bytes[index] != b' ' && bytes[index] != b'\t' && bytes[index] != b'='
    {
        index += 1;
    }
    let name = &line[name_start..index];

    while index < bytes.len() && bytes[index] != b'=' {
        index += 1;
    }
    if index >= bytes.len() {
        return Err(ContractParseError::SalParseError);
    }
    index += 1; // skip '='

    let val_str = trim_space_tab(&line[index..]);
    // Integer value, ignoring a '+' prefix; unparseable -> 0 (the
    // Zig `catch 0`, mirrored deliberately).
    let parse_target = val_str.strip_prefix('+').unwrap_or(val_str);
    let val_int: i64 = parse_target.parse().unwrap_or(0);

    Ok(LayoutDefine {
        name: name.to_string(),
        val_str: val_str.to_string(),
        val_int,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Zig unit test at the bottom of contract_parser.zig,
    /// ported assertion-for-assertion.
    #[test]
    fn zig_contract_parser_test_port() {
        let sai_content = "// Test interface\n\
            @extern sa_node_plugin_os_cpus(&out_ptr: ptr, &out_len: ptr) -> u32\n\
            @extern sa_node_plugin_free_buffer(ptr: ptr, len: u64) -> u32\n\
            @extern sa_time_sleep_ns(ns: u64) -> i32!\n";
        let sal_content = "#def RcBox_SIZE = 24\n\
            #def RcBox_strong = +0\n\
            #def RcBox_weak = +8\n";

        let functions = parse_sai(sai_content).unwrap();
        assert_eq!(functions.len(), 3);
        assert_eq!(functions[0].name, "sa_node_plugin_os_cpus");
        assert_eq!(functions[0].params.len(), 2);
        assert!(functions[0].params[0].is_borrow);
        assert_eq!(functions[0].params[0].name, "out_ptr");
        assert_eq!(functions[0].params[0].ty, "ptr");
        assert_eq!(functions[0].ret_ty, "u32");
        assert!(!functions[0].return_fallible);
        assert_eq!(functions[2].ret_ty, "i32");
        assert!(functions[2].return_fallible);

        let defines = parse_sal(sal_content).unwrap();
        assert_eq!(defines.len(), 3);
        assert_eq!(defines[0].name, "RcBox_SIZE");
        assert_eq!(defines[0].val_int, 24);
        assert_eq!(defines[1].name, "RcBox_strong");
        assert_eq!(defines[1].val_int, 0);
    }

    #[test]
    fn empty_content_parses_to_empty_lists() {
        // Zig loadContracts calls parseSai("") / parseSal("") for the
        // absent half of a pair; both must succeed empty.
        assert!(parse_sai("").unwrap().is_empty());
        assert!(parse_sal("").unwrap().is_empty());
    }

    #[test]
    fn move_param_and_default_void_return() {
        let fns = parse_sai("@extern sa_fmt_buffer_free(^buffer: u64) -> i32\n@extern sa_print_bytes(&msg: ptr, len: u64) -> void\n@extern no_ret(x: i32)\n").unwrap();
        assert_eq!(fns.len(), 3);
        assert!(fns[0].params[0].is_move);
        assert!(!fns[0].params[0].is_borrow);
        assert_eq!(fns[1].ret_ty, "void");
        assert!(fns[1].params[0].is_borrow);
        assert_eq!(fns[2].ret_ty, "void");
        assert!(!fns[2].return_fallible);
    }

    #[test]
    fn malformed_extern_line_is_sai_parse_error() {
        assert_eq!(
            parse_sai("@extern broken_line_without_parens\n"),
            Err(ContractParseError::SaiParseError)
        );
        assert_eq!(
            parse_sai("@extern f(x) -> void\n"),
            Err(ContractParseError::SaiParseError)
        );
    }

    #[test]
    fn def_without_equals_is_sal_parse_error_and_bad_int_is_zero() {
        assert_eq!(
            parse_sal("#def BROKEN\n"),
            Err(ContractParseError::SalParseError)
        );
        let defs = parse_sal("#def WEIRD = abc\n").unwrap();
        assert_eq!(defs[0].val_str, "abc");
        assert_eq!(defs[0].val_int, 0);
    }

    #[test]
    fn comma_inside_param_type_splits_like_zig() {
        // Documented Zig quirk (splitScalar on ','): a generic type
        // with a comma produces a second segment without ':', which
        // is a SaiParseError — mirrored, not repaired.
        assert_eq!(
            parse_sai("@extern f(x: Pair<i32, i32>) -> void\n"),
            Err(ContractParseError::SaiParseError)
        );
    }
}

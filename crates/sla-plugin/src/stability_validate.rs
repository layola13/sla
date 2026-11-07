//! Manifest validation, mirrored from `stability_metadata.zig`
//! `validateManifestText` and its helpers (venue head 6002b386).
//!
//! Zig validates over `std.json.Value`; the workspace has no JSON
//! dependency (a standing constraint — new Cargo edges belong to
//! the coordination line), so a small hand parser below produces
//! the same value shapes the validator reads. Object `get` follows
//! `std.json`'s StringArrayHashMap semantics: a later duplicate
//! key wins. Parse failures surface as `invalid JSON: <name>`,
//! with the name mapped onto the std.json scanner's error names
//! for the common failure classes (see `JsonError`).
//!
//! Control flow is mirrored statement-for-statement, including
//! which checks `continue` (skipping the label/artifact/evidence
//! counters) and which only record an error — the counters are
//! observable in both report writers.

/// Zig `ValidationReport` (minus the allocator).
#[derive(Debug, Default)]
pub struct ValidationReport {
    pub valid: bool,
    pub label_count: usize,
    pub artifact_count: usize,
    pub evidence_count: usize,
    pub errors: Vec<String>,
}

impl ValidationReport {
    fn new() -> ValidationReport {
        ValidationReport {
            valid: true,
            label_count: 0,
            artifact_count: 0,
            evidence_count: 0,
            errors: Vec::new(),
        }
    }

    fn add_error(&mut self, message: String) {
        self.valid = false;
        self.errors.push(message);
    }
}

/// Zig `validateManifestText`.
pub fn validate_manifest_text(text: &[u8]) -> ValidationReport {
    let mut report = ValidationReport::new();
    let value = match parse_json(text) {
        Ok(v) => v,
        Err(name) => {
            // Venue probes against the real plugin (14 malformed
            // shapes) show std.json's scanner surfaces exactly two
            // names here: UnexpectedEndOfInput when the input ends
            // mid-value, SyntaxError for every other malformation
            // (bad token, bad escape, control char, leading zero,
            // trailing comma/garbage, lone surrogate, ...). The
            // parser's finer internal classes collapse accordingly.
            let display = if name == "UnexpectedEndOfInput" {
                name
            } else {
                "SyntaxError"
            };
            report.add_error(format!("invalid JSON: {display}"));
            return report;
        }
    };
    validate_root(&mut report, &value);
    report
}

fn validate_root(report: &mut ValidationReport, value: &JsonValue) {
    let JsonValue::Object(_) = value else {
        report.add_error("root must be an object".to_string());
        return;
    };
    match value.get("schema_version") {
        Some(v) => {
            if !matches!(v, JsonValue::Integer(1)) {
                report.add_error("schema_version must be integer 1".to_string());
            }
        }
        None => report.add_error("missing required field: schema_version".to_string()),
    }
    if let Some(project) = value.get("project") {
        if !matches!(project, JsonValue::String(s) if !s.is_empty()) {
            report.add_error("project must be a non-empty string when present".to_string());
        }
    }

    let mut declared_labels: Vec<String> = Vec::new();
    match value.get("labels") {
        Some(labels) => validate_labels(report, labels, &mut declared_labels),
        None => report.add_error("missing required field: labels".to_string()),
    }
    match value.get("artifacts") {
        Some(artifacts) => validate_artifacts(report, artifacts, &declared_labels),
        None => report.add_error("missing required field: artifacts".to_string()),
    }
}

fn validate_labels(
    report: &mut ValidationReport,
    value: &JsonValue,
    declared_labels: &mut Vec<String>,
) {
    let JsonValue::Array(items) = value else {
        report.add_error("labels must be an array".to_string());
        return;
    };
    for (idx, entry) in items.iter().enumerate() {
        if !matches!(entry, JsonValue::Object(_)) {
            report.add_error(format!("labels[{idx}] must be an object"));
            continue;
        }
        let Some(name_value) = entry.get("name") else {
            report.add_error(format!("labels[{idx}] missing name"));
            continue;
        };
        let JsonValue::String(name) = name_value else {
            report.add_error(format!(
                "labels[{idx}].name must match [A-Za-z0-9][A-Za-z0-9_.-]*"
            ));
            continue;
        };
        if !is_valid_label_name(name) {
            report.add_error(format!(
                "labels[{idx}].name must match [A-Za-z0-9][A-Za-z0-9_.-]*"
            ));
            continue;
        }
        let name = name.clone();
        let Some(description_value) = entry.get("description") else {
            report.add_error(format!("labels[{idx}] missing description"));
            continue;
        };
        if !matches!(description_value, JsonValue::String(s) if !s.is_empty()) {
            report.add_error(format!(
                "labels[{idx}].description must be a non-empty string"
            ));
        }
        if declared_labels.iter().any(|n| n == &name) {
            report.add_error(format!("duplicate label: {name}"));
        } else {
            declared_labels.push(name);
            report.label_count += 1;
        }
    }
}

fn validate_artifacts(
    report: &mut ValidationReport,
    value: &JsonValue,
    declared_labels: &[String],
) {
    let JsonValue::Array(items) = value else {
        report.add_error("artifacts must be an array".to_string());
        return;
    };
    for (idx, entry) in items.iter().enumerate() {
        if !matches!(entry, JsonValue::Object(_)) {
            report.add_error(format!("artifacts[{idx}] must be an object"));
            continue;
        }
        let Some(path_value) = entry.get("path") else {
            report.add_error(format!("artifacts[{idx}] missing path"));
            continue;
        };
        if !matches!(path_value, JsonValue::String(p) if is_valid_artifact_path(p)) {
            report.add_error(format!(
                "artifacts[{idx}].path must be a non-empty relative path without '..' segments"
            ));
        }
        match entry.get("labels") {
            Some(labels) => validate_artifact_labels(report, labels, idx, declared_labels),
            None => report.add_error(format!("artifacts[{idx}] missing labels")),
        }
        if let Some(evidence) = entry.get("evidence") {
            validate_evidence(report, evidence, idx);
        }
        report.artifact_count += 1;
    }
}

fn validate_artifact_labels(
    report: &mut ValidationReport,
    value: &JsonValue,
    artifact_idx: usize,
    declared_labels: &[String],
) {
    let JsonValue::Array(items) = value else {
        report.add_error(format!("artifacts[{artifact_idx}].labels must be an array"));
        return;
    };
    for (label_idx, label_value) in items.iter().enumerate() {
        let JsonValue::String(label) = label_value else {
            report.add_error(format!(
                "artifacts[{artifact_idx}].labels[{label_idx}] must be a string"
            ));
            continue;
        };
        if !is_valid_label_name(label) {
            report.add_error(format!(
                "artifacts[{artifact_idx}].labels[{label_idx}] has invalid label name: {label}"
            ));
            continue;
        }
        if !declared_labels.iter().any(|n| n == label) {
            report.add_error(format!(
                "artifacts[{artifact_idx}].labels[{label_idx}] references undeclared label: {label}"
            ));
        }
    }
}

fn validate_evidence(report: &mut ValidationReport, value: &JsonValue, artifact_idx: usize) {
    let JsonValue::Array(items) = value else {
        report.add_error(format!(
            "artifacts[{artifact_idx}].evidence must be an array"
        ));
        return;
    };
    for (evidence_idx, entry) in items.iter().enumerate() {
        if !matches!(entry, JsonValue::Object(_)) {
            report.add_error(format!(
                "artifacts[{artifact_idx}].evidence[{evidence_idx}] must be an object"
            ));
            continue;
        }
        let Some(kind_value) = entry.get("kind") else {
            report.add_error(format!(
                "artifacts[{artifact_idx}].evidence[{evidence_idx}] missing kind"
            ));
            continue;
        };
        if !matches!(kind_value, JsonValue::String(s) if !s.is_empty()) {
            report.add_error(format!(
                "artifacts[{artifact_idx}].evidence[{evidence_idx}].kind must be a non-empty string"
            ));
        }
        let Some(status_value) = entry.get("status") else {
            report.add_error(format!(
                "artifacts[{artifact_idx}].evidence[{evidence_idx}] missing status"
            ));
            continue;
        };
        if !matches!(status_value, JsonValue::String(s) if is_valid_evidence_status(s)) {
            report.add_error(format!(
                "artifacts[{artifact_idx}].evidence[{evidence_idx}].status must be pass, fail, or unknown"
            ));
        }
        report.evidence_count += 1;
    }
}

fn is_valid_label_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.')
}

fn is_valid_artifact_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') {
        return false;
    }
    !path.split('/').any(|part| part == "..")
}

fn is_valid_evidence_status(status: &str) -> bool {
    matches!(status, "pass" | "fail" | "unknown")
}

// -- minimal JSON parser (std.json.Value shapes) ---------------------------

#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// Object member lookup; a later duplicate key wins (std.json
    /// StringArrayHashMap `put` semantics).
    fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs
                .iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v),
            _ => None,
        }
    }
}

/// The std.json scanner error NAME for a parse failure class.
type JsonError = &'static str;

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

fn parse_json(text: &[u8]) -> Result<JsonValue, JsonError> {
    let mut parser = JsonParser { bytes: text, pos: 0 };
    parser.skip_ws();
    let value = parser.parse_value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err("UnexpectedToken");
    }
    Ok(value)
}

impl<'a> JsonParser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        match self.peek() {
            None => Err("UnexpectedEndOfInput"),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(JsonValue::String(self.parse_string()?)),
            Some(b't') => self.parse_literal(b"true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal(b"false", JsonValue::Bool(false)),
            Some(b'n') => self.parse_literal(b"null", JsonValue::Null),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(_) => Err("UnexpectedToken"),
        }
    }

    fn parse_literal(
        &mut self,
        lit: &[u8],
        value: JsonValue,
    ) -> Result<JsonValue, JsonError> {
        if self.bytes.len() - self.pos < lit.len() {
            return Err("UnexpectedEndOfInput");
        }
        if &self.bytes[self.pos..self.pos + lit.len()] == lit {
            self.pos += lit.len();
            Ok(value)
        } else {
            Err("UnexpectedToken")
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        self.pos += 1; // '{'
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(if self.peek().is_none() {
                    "UnexpectedEndOfInput"
                } else {
                    "UnexpectedToken"
                });
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err("UnexpectedToken");
            }
            self.pos += 1;
            self.skip_ws();
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonValue::Object(pairs));
                }
                None => return Err("UnexpectedEndOfInput"),
                _ => return Err("UnexpectedToken"),
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            self.skip_ws();
            let value = self.parse_value()?;
            items.push(value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonValue::Array(items));
                }
                None => return Err("UnexpectedEndOfInput"),
                _ => return Err("UnexpectedToken"),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err("UnexpectedEndOfInput");
            };
            self.pos += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(esc) = self.peek() else {
                        return Err("UnexpectedEndOfInput");
                    };
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let cp = self.parse_hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                // High surrogate: expect a low one.
                                if self.peek() == Some(b'\\')
                                    && self.bytes.get(self.pos + 1) == Some(&b'u')
                                {
                                    self.pos += 2;
                                    let lo = self.parse_hex4()?;
                                    if !(0xDC00..0xE000).contains(&lo) {
                                        return Err("InvalidCharacter");
                                    }
                                    let c = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                    out.push(char::from_u32(c).ok_or("InvalidCharacter")?);
                                } else {
                                    return Err("InvalidCharacter");
                                }
                            } else if (0xDC00..0xE000).contains(&cp) {
                                return Err("InvalidCharacter");
                            } else {
                                out.push(char::from_u32(cp).ok_or("InvalidCharacter")?);
                            }
                        }
                        _ => return Err("InvalidCharacter"),
                    }
                }
                0x00..=0x1f => return Err("InvalidCharacter"),
                _ => {
                    // Copy the full UTF-8 sequence verbatim.
                    let start = self.pos - 1;
                    let width = utf8_width(c);
                    if width == 0 || self.pos - 1 + width > self.bytes.len() {
                        return Err("InvalidCharacter");
                    }
                    let end = start + width;
                    match std::str::from_utf8(&self.bytes[start..end]) {
                        Ok(s) => out.push_str(s),
                        Err(_) => return Err("InvalidCharacter"),
                    }
                    self.pos = end;
                }
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, JsonError> {
        if self.pos + 4 > self.bytes.len() {
            return Err("UnexpectedEndOfInput");
        }
        let mut value: u32 = 0;
        for i in 0..4 {
            let c = self.bytes[self.pos + i];
            let digit = match c {
                b'0'..=b'9' => (c - b'0') as u32,
                b'a'..=b'f' => (c - b'a' + 10) as u32,
                b'A'..=b'F' => (c - b'A' + 10) as u32,
                _ => return Err("InvalidCharacter"),
            };
            value = value * 16 + digit;
        }
        self.pos += 4;
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            None => return Err("UnexpectedEndOfInput"),
            _ => return Err("InvalidNumber"),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(if self.peek().is_none() {
                    "UnexpectedEndOfInput"
                } else {
                    "InvalidNumber"
                });
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(if self.peek().is_none() {
                    "UnexpectedEndOfInput"
                } else {
                    "InvalidNumber"
                });
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "InvalidNumber")?;
        if !is_float {
            if let Ok(v) = text.parse::<i64>() {
                return Ok(JsonValue::Integer(v));
            }
        }
        text.parse::<f64>()
            .map(JsonValue::Float)
            .map_err(|_| "InvalidNumber")
    }
}

fn utf8_width(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_MANIFEST: &str = r#"{
  "schema_version": 1,
  "project": "downstream-project",
  "labels": [
    { "name": "stable-demo", "description": "User-facing demo with repeatable verification evidence." },
    { "name": "verified-sa-backend", "description": "SA-text backend verification passed for this artifact." },
    { "name": "verified-sab-backend", "description": "Direct SAB verification passed for this artifact." },
    { "name": "experimental-parallel", "description": "Parallel behavior is experimental and not a parity claim." },
    { "name": "shape-only-reflect", "description": "Reflection covers shape metadata only." }
  ],
  "artifacts": [
    {
      "path": "lib/parallel.sla",
      "labels": ["verified-sab-backend", "experimental-parallel"],
      "evidence": [
        { "kind": "command", "status": "pass", "command": "SA_PLUGIN_DEV=1 sa sla test lib/parallel.sla --test-backend sab" }
      ]
    }
  ]
}"#;

    /// The Zig unit test in stability_metadata.zig, ported.
    #[test]
    fn valid_example_manifest() {
        let report = validate_manifest_text(EXAMPLE_MANIFEST.as_bytes());
        assert!(report.valid, "errors: {:?}", report.errors);
        assert_eq!(report.label_count, 5);
        assert_eq!(report.artifact_count, 1);
        assert_eq!(report.evidence_count, 1);
    }

    /// The second Zig unit test, ported (duplicate + undeclared +
    /// unsafe path + bad status => at least 3 errors, invalid).
    #[test]
    fn rejects_undeclared_duplicate_and_unsafe_path() {
        let manifest = r#"{
  "schema_version": 1,
  "labels": [
    { "name": "verified-sab-backend", "description": "one" },
    { "name": "verified-sab-backend", "description": "two" }
  ],
  "artifacts": [
    { "path": "../lib/demo.sla", "labels": ["missing-label"], "evidence": [{ "kind": "command", "status": "maybe" }] }
  ]
}"#;
        let report = validate_manifest_text(manifest.as_bytes());
        assert!(!report.valid);
        assert!(report.errors.len() >= 3, "errors: {:?}", report.errors);
    }

    #[test]
    fn invalid_json_is_a_single_prefixed_error() {
        let report = validate_manifest_text(b"{ not json");
        assert!(!report.valid);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].starts_with("invalid JSON: "));
    }

    #[test]
    fn float_schema_version_is_rejected() {
        let report = validate_manifest_text(
            br#"{"schema_version": 1.0, "labels": [], "artifacts": []}"#,
        );
        assert!(!report.valid);
        assert_eq!(report.errors, vec!["schema_version must be integer 1"]);
    }
}

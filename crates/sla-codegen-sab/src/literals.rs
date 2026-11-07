//! String-literal byte codec.
//!
//! Faithful port of the Zig `Codegen` helpers in sab_codegen.zig:
//! `escapedUtf8LiteralBytes` (lines 1208-1228) and
//! `decodeStringLiteralBytes` (lines 1182-1206), plus the decoded-length
//! pre-scan `decodedStringLiteralLen` (lines ~1140-1180) whose contract
//! the decoder here enforces directly.
//!
//! Escape table (write side): `\` -> `\\`, `"` -> `\"`, LF -> `\n`,
//! CR -> `\r`, TAB -> `\t`, NUL -> `\0`, other printable ASCII
//! (0x20..=0x7E) verbatim, everything else -> `\xHH` with uppercase hex.
//! The read side accepts exactly `\\`, `\"`, `\n`, `\r`, `\t`, `\0` and
//! `\xHH`; any other escape is `Error::InvalidStringLiteral`, as in Zig.

use crate::error::{Error, Result};

/// Escape raw bytes for inclusion in a `utf8:"..."` literal
/// (Zig `escapedUtf8LiteralBytes`).
pub fn escape_utf8_literal_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0 => out.push_str("\\0"),
            0x20..=0x7e => out.push(b as char),
            _ => {
                out.push_str(&format!("\\x{b:02X}"));
            }
        }
    }
    out
}

fn parse_hex_digit_pair(pair: &[u8]) -> Result<u8> {
    let hex = |c: u8| -> Result<u8> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(Error::InvalidStringLiteral(format!(
                "bad hex digit in string escape: {:?}",
                c as char
            ))),
        }
    };
    if pair.len() != 2 {
        return Err(Error::InvalidStringLiteral(
            "truncated \\x escape in string literal".to_string(),
        ));
    }
    Ok((hex(pair[0])? << 4) | hex(pair[1])?)
}

/// Decode the body of an SLA string literal (the text between the
/// quotes) to raw bytes (Zig `decodeStringLiteralBytes`).
pub fn decode_string_literal_bytes(value: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(value.len());
    let mut i = 0usize;
    while i < value.len() {
        let c = value[i];
        if c != b'\\' {
            out.push(c);
            i += 1;
            continue;
        }
        if i + 1 >= value.len() {
            return Err(Error::InvalidStringLiteral(
                "trailing backslash in string literal".to_string(),
            ));
        }
        match value[i + 1] {
            b'\\' => out.push(b'\\'),
            b'"' => out.push(b'"'),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'0' => out.push(0),
            b'x' => {
                if i + 3 >= value.len() {
                    return Err(Error::InvalidStringLiteral(
                        "truncated \\x escape in string literal".to_string(),
                    ));
                }
                out.push(parse_hex_digit_pair(&value[i + 2..i + 4])?);
                i += 4;
                continue;
            }
            other => {
                return Err(Error::InvalidStringLiteral(format!(
                    "unknown escape \\{} in string literal",
                    other as char
                )));
            }
        }
        i += 2;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_table() {
        assert_eq!(escape_utf8_literal_bytes(b"hello world\0"), "hello world\\0");
        assert_eq!(escape_utf8_literal_bytes(b"a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(escape_utf8_literal_bytes(&[0x01, 0x7f, 0xff]), "\\x01\\x7F\\xFF");
        assert_eq!(escape_utf8_literal_bytes(b"\n\r\t"), "\\n\\r\\t");
    }

    #[test]
    fn decode_known() {
        assert_eq!(decode_string_literal_bytes(b"hello").unwrap(), b"hello");
        assert_eq!(decode_string_literal_bytes(b"a\\0b").unwrap(), b"a\0b");
        assert_eq!(decode_string_literal_bytes(b"\\x41\\n").unwrap(), b"A\n");
    }

    #[test]
    fn decode_rejects_unknown_escape() {
        assert!(matches!(
            decode_string_literal_bytes(b"\\q"),
            Err(Error::InvalidStringLiteral(_))
        ));
        assert!(matches!(
            decode_string_literal_bytes(b"abc\\"),
            Err(Error::InvalidStringLiteral(_))
        ));
    }

    #[test]
    fn round_trip() {
        let raw: Vec<u8> = (0u16..=255).map(|b| b as u8).collect();
        let escaped = escape_utf8_literal_bytes(&raw);
        assert_eq!(decode_string_literal_bytes(escaped.as_bytes()).unwrap(), raw);
    }
}

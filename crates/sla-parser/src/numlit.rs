//! Numeric literal helpers, mirroring `stripNumericSeparators` and
//! `integerSuffixPrimitive` in `src/parser.zig`, plus the integer-literal
//! scan performed by `parsePrefixExpr`'s `int_literal` branch.

use sla_ast::exprs::{CastExpr, Literal};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};
use sla_lexer::Loc;

use crate::error::{ParseError, ParseErrorKind};

/// #11: copy a numeric literal lexeme sans `_` separators. The lexer only
/// keeps `_` between digits, so dropping every `_` is exact.
pub fn strip_numeric_separators(s: &str) -> String {
    s.chars().filter(|&c| c != '_').collect()
}

/// Mirror of Zig `integerSuffixPrimitive`: the type suffix after the digits
/// of an integer literal, if it names an integer primitive.
pub fn integer_suffix_primitive(suffix: &str) -> Option<Primitive> {
    Some(match suffix {
        "i8" => Primitive::I8,
        "i16" => Primitive::I16,
        "i32" => Primitive::I32,
        "i64" => Primitive::I64,
        "isize" => Primitive::Isize,
        "u8" => Primitive::U8,
        "u16" => Primitive::U16,
        "u32" => Primitive::U32,
        "u64" => Primitive::U64,
        "usize" => Primitive::Usize,
        _ => return None,
    })
}

/// Parse an integer literal lexeme exactly as the Zig parser does:
/// strip `_` separators, accept a `0x`/`0X` hex prefix, scan the digit run,
/// treat the remainder as a type suffix (unknown suffix => InvalidCharacter),
/// and wrap suffixed literals in a `cast_expr` to the suffix primitive.
/// `u64`/`usize` literals are parsed unsigned and bit-cast into `i64`.
pub fn parse_int_literal(lexeme: &str, loc: Loc) -> Result<Node, ParseError> {
    let invalid = || ParseError::new(ParseErrorKind::InvalidCharacter, loc.start, loc.end);
    let clean = strip_numeric_separators(lexeme);
    let bytes = clean.as_bytes();

    let (base, digits_start) = if clean.len() >= 2 && bytes[0] == b'0' && (bytes[1] == b'x' || bytes[1] == b'X') {
        (16u32, 2usize)
    } else {
        (10u32, 0usize)
    };

    let mut digit_len = digits_start;
    if base == 16 {
        while digit_len < bytes.len() && bytes[digit_len].is_ascii_hexdigit() {
            digit_len += 1;
        }
    } else {
        while digit_len < bytes.len() && bytes[digit_len].is_ascii_digit() {
            digit_len += 1;
        }
    }

    let suffix = &clean[digit_len..];
    let primitive = if !suffix.is_empty() {
        Some(integer_suffix_primitive(suffix).ok_or_else(invalid)?)
    } else {
        None
    };

    let digits = &clean[digits_start..digit_len];
    let unsigned_wide = matches!(primitive, Some(Primitive::U64) | Some(Primitive::Usize));
    let val: i64 = if unsigned_wide {
        let unsigned = u64::from_str_radix(digits, base).map_err(|_| invalid())?;
        unsigned as i64
    } else {
        i64::from_str_radix(digits, base).map_err(|_| invalid())?
    };

    let node = Node::Literal(Literal::IntVal(val));
    match primitive {
        Some(prim) => Ok(Node::CastExpr(CastExpr {
            expr: Box::new(node),
            ty: Box::new(Type::Primitive(prim)),
        })),
        None => Ok(node),
    }
}

/// Parse a float literal lexeme as the Zig parser does (separators stripped,
/// value parsed as `f64`).
pub fn parse_float_literal(lexeme: &str, loc: Loc) -> Result<Node, ParseError> {
    let clean = strip_numeric_separators(lexeme);
    let val: f64 = clean
        .parse::<f64>()
        .map_err(|_| ParseError::new(ParseErrorKind::InvalidCharacter, loc.start, loc.end))?;
    Ok(Node::Literal(Literal::FloatVal(val)))
}

//! Parse errors, mirroring `ParserError` in `src/parser.zig` of the Zig
//! plugin. Kind names keep the Zig spellings so diagnostics and divergence
//! notes can be diffed against the original by eye.
//!
//! Difference from Zig: the original threads a `last_expected` side channel
//! through the parser and renders diagnostics separately (`printDiagnostic`).
//! Here the expectation is carried on the error value itself, and every
//! error carries the byte span of the offending token.

use std::fmt;

/// Mirror of the Zig `ParserError` error set (same variant names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    SyntaxError,
    InlineStructNotAllowed,
    InlineImplNotAllowed,
    InlineMacroNotAllowed,
    ExternBlockRequiresExtern,
    ExpectedDeclaration,
    UnexpectedToken,
    UnexpectedInfixToken,
    InvalidCallTarget,
    UnexpectedTypeToken,
    OutOfMemory,
    Overflow,
    InvalidCharacter,
}

impl ParseErrorKind {
    pub fn name(self) -> &'static str {
        match self {
            ParseErrorKind::SyntaxError => "SyntaxError",
            ParseErrorKind::InlineStructNotAllowed => "InlineStructNotAllowed",
            ParseErrorKind::InlineImplNotAllowed => "InlineImplNotAllowed",
            ParseErrorKind::InlineMacroNotAllowed => "InlineMacroNotAllowed",
            ParseErrorKind::ExternBlockRequiresExtern => "ExternBlockRequiresExtern",
            ParseErrorKind::ExpectedDeclaration => "ExpectedDeclaration",
            ParseErrorKind::UnexpectedToken => "UnexpectedToken",
            ParseErrorKind::UnexpectedInfixToken => "UnexpectedInfixToken",
            ParseErrorKind::InvalidCallTarget => "InvalidCallTarget",
            ParseErrorKind::UnexpectedTypeToken => "UnexpectedTypeToken",
            ParseErrorKind::OutOfMemory => "OutOfMemory",
            ParseErrorKind::Overflow => "Overflow",
            ParseErrorKind::InvalidCharacter => "InvalidCharacter",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    /// Byte span of the token at which the error was raised.
    pub start: usize,
    pub end: usize,
    /// Human-readable expectation (the Zig `last_expected` channel), when the
    /// failing site recorded one.
    pub expected: Option<String>,
}

impl ParseError {
    pub fn new(kind: ParseErrorKind, start: usize, end: usize) -> Self {
        ParseError { kind, start, end, expected: None }
    }

    pub fn with_expected(kind: ParseErrorKind, start: usize, end: usize, expected: String) -> Self {
        ParseError { kind, start, end, expected: Some(expected) }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.expected {
            Some(exp) => write!(
                f,
                "{} at bytes {}..{} (expected {})",
                self.kind.name(),
                self.start,
                self.end,
                exp
            ),
            None => write!(f, "{} at bytes {}..{}", self.kind.name(), self.start, self.end),
        }
    }
}

impl std::error::Error for ParseError {}

//! Token types, mirroring `Token` / `Token.Loc` / `Token.Tag` in
//! `src/lexer.zig` of the Zig plugin.
//!
//! Variant mapping rule: Zig snake_case variant -> Rust CamelCase variant
//! (e.g. `keyword_struct` -> `KeywordStruct`, `int_literal` -> `IntLiteral`).
//! Declaration order matches the Zig enum so the two can be diffed by eye.

/// A single token: its tag plus a byte-offset span into the source buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub tag: Tag,
    pub loc: Loc,
}

/// Byte-offset span `[start, end)` in the source buffer (Zig `Token.Loc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loc {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
    Eof,
    Invalid,
    Identifier,

    // Literals
    IntLiteral,
    FloatLiteral,
    StringLiteral,
    // Template literal pieces: `head ${expr} tail`
    TemplateStart,  // opening backtick
    TemplateString, // raw literal chunk (escapes unprocessed)
    TemplateInterp, // ${
    TemplateEnd,    // closing backtick

    // Keywords
    KeywordStruct,
    KeywordUnion,
    KeywordEnum,
    KeywordTrait,
    KeywordOverload,
    KeywordDyn,
    KeywordImpl,
    KeywordMod,
    KeywordUsing,
    KeywordPub,
    KeywordExtern,
    KeywordAsync,
    KeywordAwait,
    KeywordUnsafe,
    KeywordAs,
    KeywordFn,
    KeywordIf,
    KeywordElse,
    KeywordMatch,
    KeywordSwitch,
    KeywordReturn,
    KeywordFor,
    KeywordWhile,
    KeywordBreak,
    KeywordContinue,
    KeywordIn,
    KeywordLet,
    KeywordConst,
    KeywordVar,
    KeywordInline,
    KeywordMacro,
    KeywordMut,
    KeywordType,

    // Symbols
    Plus,           // +
    PlusEqual,      // +=
    MinusEqual,     // -=
    AsteriskEqual,  // *=
    SlashEqual,     // /=
    PercentEqual,   // %=
    PipeEqual,      // |=
    AmpersandEqual, // &=
    Minus,          // -
    Asterisk,       // *
    Slash,          // /
    Percent,        // %
    Equal,          // =
    EqualEqual,     // ==
    BangEqual,      // !=
    LessEqual,      // <=
    GreaterEqual,   // >=
    Ampersand,      // &
    AmpAmp,         // &&
    Caret,          // ^
    Bang,           // !
    Pipe,           // |
    PipePipe,       // ||
    LessLess,       // <<
    GreaterGreater, // >>
    Spaceship,      // <=>
    Dot,            // .
    Comma,          // ,
    Semicolon,      // ;
    Colon,          // :
    DoubleColon,    // ::
    LParen,         // (
    RParen,         // )
    LBrace,         // {
    RBrace,         // }
    LBracket,       // [
    RBracket,       // ]
    LessThan,       // <
    GreaterThan,    // >
    Arrow,          // ->
    FatArrow,       // =>
    Range,          // ..
    QuestionMark,   // ?
    QuestionQuestion,       // ??
    QuestionQuestionEqual,  // ??=
    QuestionDot,            // ?.
    At,                     // @
}

//! The lexer proper, a line-by-line port of `Lexer` in the Zig plugin's
//! `src/lexer.zig`.
//!
//! Deliberate, documented deviations from the Zig original (see
//! `docs/divergences/lexer-ast.md` for the full register):
//!  * Line comments restart scanning with a loop; Zig recurses via
//!    `return self.next()`. Token output is identical.
//!  * Where Zig can run `index` one byte past the end of the buffer (a `\`
//!    as the very last byte of an unterminated string or template chunk),
//!    producing `loc.end > buffer.len`, this port clamps `index` to the
//!    buffer length. The emitted tag (`Invalid`) is unchanged.

use crate::keyword::check_keyword;
use crate::token::{Loc, Tag, Token};

pub struct Lexer<'a> {
    buffer: &'a [u8],
    index: usize,
    /// True while scanning template-literal text (between `` ` `` and `${` or `` ` ``).
    in_template: bool,
    /// Open `${` depth while lexing an interpolation (0 at template level).
    /// `{`/`}` adjust it only when > 0, so plain blocks are untouched.
    interp_depth: u16,
    /// Saved interp depths for nested templates (cap 8, else invalid).
    tmpl_stack: [u16; 8],
    tmpl_sp: u8,
}

impl<'a> Lexer<'a> {
    pub fn new(buffer: &'a [u8]) -> Lexer<'a> {
        Lexer {
            buffer,
            index: 0,
            in_template: false,
            interp_depth: 0,
            tmpl_stack: [0; 8],
            tmpl_sp: 0,
        }
    }

    /// The full source buffer (Zig exposes `buffer` as a public field).
    pub fn buffer(&self) -> &'a [u8] {
        self.buffer
    }

    /// Source text of a token, sliced by its location.
    pub fn token_text(&self, tok: Token) -> &'a [u8] {
        &self.buffer[tok.loc.start..tok.loc.end]
    }

    fn token(&self, tag: Tag, start: usize) -> Token {
        Token {
            tag,
            loc: Loc {
                start,
                end: self.index,
            },
        }
    }

    fn invalid_at(&self, start: usize) -> Token {
        self.token(Tag::Invalid, start)
    }

    fn at_byte(&self, offset: usize) -> Option<u8> {
        self.buffer.get(self.index + offset).copied()
    }

    /// Scans one template-literal piece. Called with `in_template` set;
    /// whitespace is significant here. Returns `TemplateString` chunks,
    /// `TemplateInterp` (`${`), or `TemplateEnd` (closing backtick).
    fn next_template(&mut self) -> Token {
        // `${` opener.
        if self.at_byte(0) == Some(b'$') && self.at_byte(1) == Some(b'{') {
            let start = self.index;
            self.index += 2;
            self.in_template = false;
            self.interp_depth = 1;
            return self.token(Tag::TemplateInterp, start);
        }
        // Closing backtick.
        if self.at_byte(0) == Some(b'`') {
            let start = self.index;
            self.index += 1;
            if self.tmpl_sp == 0 {
                return self.invalid_at(start);
            }
            self.tmpl_sp -= 1;
            self.interp_depth = self.tmpl_stack[self.tmpl_sp as usize];
            self.in_template = false;
            return self.token(Tag::TemplateEnd, start);
        }
        // Literal chunk up to (not consuming) `${`, backtick, or EOF.
        let start = self.index;
        while self.index < self.buffer.len() {
            let c = self.buffer[self.index];
            if c == b'\\' {
                // Zig steps 2 here and can land one past the buffer end;
                // clamped (see crate docs / divergence register D-L2).
                self.index = (self.index + 2).min(self.buffer.len());
                continue;
            }
            if c == b'`' {
                break;
            }
            if c == b'$' && self.at_byte(1) == Some(b'{') {
                break;
            }
            self.index += 1;
        }
        if self.index >= self.buffer.len() {
            return self.invalid_at(start);
        }
        self.token(Tag::TemplateString, start)
    }

    pub fn next(&mut self) -> Token {
        // The Zig original restarts `next()` recursively after a line
        // comment; a loop over the scan body is output-equivalent.
        loop {
            // Template-literal text is significant whitespace: bypass skipping.
            if self.in_template {
                return self.next_template();
            }
            self.skip_whitespace();

            if self.index >= self.buffer.len() {
                return Token {
                    tag: Tag::Eof,
                    loc: Loc {
                        start: self.index,
                        end: self.index,
                    },
                };
            }

            let start = self.index;
            let c = self.buffer[self.index];
            self.index += 1;

            match c {
                b'@' => return self.token(Tag::At, start),
                b'?' => {
                    if self.at_byte(0) == Some(b'?') {
                        // `??=` (but not `??==`, which is `??` + `==`).
                        if self.at_byte(1) == Some(b'=')
                            && (self.at_byte(2).is_none() || self.at_byte(2) != Some(b'='))
                        {
                            self.index += 2;
                            return self.token(Tag::QuestionQuestionEqual, start);
                        }
                        self.index += 1;
                        return self.token(Tag::QuestionQuestion, start);
                    }
                    // `?.` is the optional chain, but NOT when a digit follows
                    // (TS rule: `a?.5:0` stays ternary + number).
                    if self.at_byte(0) == Some(b'.')
                        && (self.at_byte(1).is_none() || !self.at_byte(1).unwrap().is_ascii_digit())
                    {
                        self.index += 1;
                        return self.token(Tag::QuestionDot, start);
                    }
                    return self.token(Tag::QuestionMark, start);
                }
                b'+' => {
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::PlusEqual, start);
                    }
                    return self.token(Tag::Plus, start);
                }
                b'-' => {
                    if self.at_byte(0) == Some(b'>') {
                        self.index += 1;
                        return self.token(Tag::Arrow, start);
                    }
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::MinusEqual, start);
                    }
                    return self.token(Tag::Minus, start);
                }
                b'*' => {
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::AsteriskEqual, start);
                    }
                    return self.token(Tag::Asterisk, start);
                }
                b'/' => {
                    if self.at_byte(0) == Some(b'/') {
                        // Line comment, skip till newline, then rescan.
                        self.index += 1;
                        while self.index < self.buffer.len() && self.buffer[self.index] != b'\n' {
                            self.index += 1;
                        }
                        continue;
                    }
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::SlashEqual, start);
                    }
                    return self.token(Tag::Slash, start);
                }
                b'%' => {
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::PercentEqual, start);
                    }
                    return self.token(Tag::Percent, start);
                }
                b'=' => {
                    if self.at_byte(0) == Some(b'>') {
                        self.index += 1;
                        return self.token(Tag::FatArrow, start);
                    }
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::EqualEqual, start);
                    }
                    return self.token(Tag::Equal, start);
                }
                b'&' => {
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::AmpersandEqual, start);
                    }
                    if self.at_byte(0) == Some(b'&') {
                        self.index += 1;
                        return self.token(Tag::AmpAmp, start);
                    }
                    return self.token(Tag::Ampersand, start);
                }
                b'^' => return self.token(Tag::Caret, start),
                b'|' => {
                    if self.at_byte(0) == Some(b'|') {
                        self.index += 1;
                        return self.token(Tag::PipePipe, start);
                    }
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::PipeEqual, start);
                    }
                    return self.token(Tag::Pipe, start);
                }
                b'!' => {
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::BangEqual, start);
                    }
                    return self.token(Tag::Bang, start);
                }
                b'.' => {
                    if self.at_byte(0) == Some(b'.') && self.at_byte(1) == Some(b'.') {
                        self.index += 2;
                        return self.token(Tag::DotDotDot, start);
                    }
                    if self.at_byte(0) == Some(b'.') {
                        self.index += 1;
                        return self.token(Tag::Range, start);
                    }
                    return self.token(Tag::Dot, start);
                }
                b',' => return self.token(Tag::Comma, start),
                b';' => return self.token(Tag::Semicolon, start),
                b':' => {
                    if self.at_byte(0) == Some(b':') {
                        self.index += 1;
                        return self.token(Tag::DoubleColon, start);
                    }
                    return self.token(Tag::Colon, start);
                }
                b'(' => return self.token(Tag::LParen, start),
                b')' => return self.token(Tag::RParen, start),
                b'{' => {
                    if self.interp_depth > 0 {
                        self.interp_depth += 1;
                    }
                    return self.token(Tag::LBrace, start);
                }
                b'}' => {
                    if self.interp_depth > 0 {
                        self.interp_depth -= 1;
                        if self.interp_depth == 0 {
                            self.in_template = true;
                        }
                    }
                    return self.token(Tag::RBrace, start);
                }
                b'[' => return self.token(Tag::LBracket, start),
                b']' => return self.token(Tag::RBracket, start),
                b'<' => {
                    if self.at_byte(0) == Some(b'<') {
                        self.index += 1;
                        return self.token(Tag::LessLess, start);
                    }
                    if self.at_byte(0) == Some(b'=') && self.at_byte(1) == Some(b'>') {
                        self.index += 2;
                        return self.token(Tag::Spaceship, start);
                    }
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::LessEqual, start);
                    }
                    return self.token(Tag::LessThan, start);
                }
                b'>' => {
                    if self.at_byte(0) == Some(b'>') {
                        self.index += 1;
                        return self.token(Tag::GreaterGreater, start);
                    }
                    if self.at_byte(0) == Some(b'=') {
                        self.index += 1;
                        return self.token(Tag::GreaterEqual, start);
                    }
                    return self.token(Tag::GreaterThan, start);
                }
                b'"' => {
                    while self.index < self.buffer.len() && self.buffer[self.index] != b'"' {
                        if self.buffer[self.index] == b'\\' {
                            self.index += 1;
                        }
                        self.index += 1;
                        // Zig lets the escape step push `index` one past the
                        // buffer end here; clamped (divergence D-L2).
                        if self.index > self.buffer.len() {
                            self.index = self.buffer.len();
                        }
                    }
                    if self.index < self.buffer.len() {
                        self.index += 1; // skip closing quote
                        return self.token(Tag::StringLiteral, start);
                    }
                    return self.invalid_at(start);
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                    while self.index < self.buffer.len() {
                        let next_c = self.buffer[self.index];
                        if !next_c.is_ascii_alphanumeric() && next_c != b'_' {
                            break;
                        }
                        self.index += 1;
                    }
                    let tag = check_keyword(&self.buffer[start..self.index]);
                    return self.token(tag, start);
                }
                b'0'..=b'9' => {
                    return self.next_number(start, c);
                }
                b'`' => {
                    // Template open (possibly nested inside an interpolation).
                    if (self.tmpl_sp as usize) >= self.tmpl_stack.len() {
                        return self.invalid_at(start);
                    }
                    self.tmpl_stack[self.tmpl_sp as usize] = self.interp_depth;
                    self.tmpl_sp += 1;
                    self.interp_depth = 0;
                    self.in_template = true;
                    return self.token(Tag::TemplateStart, start);
                }
                _ => return self.invalid_at(start),
            }
        }
    }

    /// Number scanning (the `0`..=`9` arm of the Zig `next`).
    fn next_number(&mut self, start: usize, first: u8) -> Token {
        if first == b'0' && (self.at_byte(0) == Some(b'x') || self.at_byte(0) == Some(b'X')) {
            self.index += 1;
            // `_` separators allowed between hex digits (`0xFF_FF`).
            while self.index < self.buffer.len() {
                let hex_c = self.buffer[self.index];
                if hex_c.is_ascii_hexdigit() {
                    self.index += 1;
                    continue;
                }
                if hex_c == b'_'
                    && self
                        .at_byte(1)
                        .map_or(false, |n| n.is_ascii_hexdigit())
                {
                    self.index += 1;
                    continue;
                }
                break;
            }
            // Suffix gobble: alphabetic run, then digit run (e.g. `0xFFu32`).
            while self
                .at_byte(0)
                .map_or(false, |n| n.is_ascii_alphabetic())
            {
                self.index += 1;
            }
            while self.at_byte(0).map_or(false, |n| n.is_ascii_digit()) {
                self.index += 1;
            }
            return self.token(Tag::IntLiteral, start);
        }

        let mut is_float = false;
        while self.index < self.buffer.len() {
            let next_c = self.buffer[self.index];
            if next_c == b'.' {
                // A range operator `..` or a float dot `.`?
                if self.at_byte(1) == Some(b'.') {
                    break; // Range operator, stop parsing number
                }
                if !self.at_byte(1).map_or(false, |n| n.is_ascii_digit()) {
                    break;
                }
                is_float = true;
                self.index += 1;
            } else if next_c == b'_' {
                // `_` separators allowed between digits (`1_000`, `1_000.5`,
                // `1.5_0`); a trailing/lone `_` keeps the historical split
                // (`1` + identifier) behavior.
                if !self.at_byte(1).map_or(false, |n| n.is_ascii_digit()) {
                    break;
                }
                self.index += 1;
            } else if next_c.is_ascii_digit() {
                self.index += 1;
            } else {
                break;
            }
        }
        if !is_float {
            // Integer suffix gobble (e.g. `123i32` is one token).
            while self
                .at_byte(0)
                .map_or(false, |n| n.is_ascii_alphanumeric())
            {
                self.index += 1;
            }
        }
        let tag = if is_float {
            Tag::FloatLiteral
        } else {
            Tag::IntLiteral
        };
        self.token(tag, start)
    }

    fn skip_whitespace(&mut self) {
        while self.index < self.buffer.len() {
            let c = self.buffer[self.index];
            if c != b' ' && c != b'\t' && c != b'\n' && c != b'\r' {
                break;
            }
            self.index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::Tag::*;

    fn tags(source: &str) -> Vec<Tag> {
        let mut lexer = Lexer::new(source.as_bytes());
        let mut out = Vec::new();
        loop {
            let tok = lexer.next();
            out.push(tok.tag);
            if tok.tag == Tag::Eof {
                return out;
            }
        }
    }

    /// Port of the Zig test "basic lexing".
    #[test]
    fn basic_lexing() {
        let mut l = Lexer::new(b"struct Option<T> { has_value: bool, value: T }");
        assert_eq!(l.next().tag, KeywordStruct);
        assert_eq!(l.next().tag, Identifier);
        assert_eq!(l.next().tag, LessThan);
        assert_eq!(l.next().tag, Identifier);
        assert_eq!(l.next().tag, GreaterThan);
        assert_eq!(l.next().tag, LBrace);
        assert_eq!(l.next().tag, Identifier);
        assert_eq!(l.next().tag, Colon);
        assert_eq!(l.next().tag, Identifier);
        assert_eq!(l.next().tag, Comma);
        assert_eq!(l.next().tag, Identifier);
        assert_eq!(l.next().tag, Colon);
        assert_eq!(l.next().tag, Identifier);
        assert_eq!(l.next().tag, RBrace);
        assert_eq!(l.next().tag, Eof);
    }

    /// Port of the Zig test "numeric separators lex as single literal".
    #[test]
    fn numeric_separators() {
        let mut l = Lexer::new(b"1_000 7_000.5 0xFF_FF 1_000..2");
        let mut tok = l.next();
        assert_eq!(tok.tag, IntLiteral);
        assert_eq!(l.token_text(tok), b"1_000");
        tok = l.next();
        assert_eq!(tok.tag, FloatLiteral);
        tok = l.next();
        assert_eq!(tok.tag, IntLiteral);
        assert_eq!(l.token_text(tok), b"0xFF_FF");
        tok = l.next();
        assert_eq!(tok.tag, IntLiteral);
        assert_eq!(l.token_text(tok), b"1_000");

        // Trailing/lone `_` keeps the historical split (`1` + identifier).
        let mut m = Lexer::new(b"1_ ");
        tok = m.next();
        assert_eq!(tok.tag, IntLiteral);
        assert_eq!(m.token_text(tok), b"1");
        tok = m.next();
        assert_eq!(tok.tag, Identifier);
    }

    /// Template literals: chunks, interpolation with a nested block, end.
    #[test]
    fn template_pieces() {
        let got = tags("`a ${ { 1 } } b`");
        assert_eq!(
            got,
            vec![
                TemplateStart,
                TemplateString,
                TemplateInterp,
                LBrace,
                IntLiteral,
                RBrace,
                RBrace,
                TemplateString,
                TemplateEnd,
                Eof,
            ]
        );
    }

    /// Zig quirk, kept: float suffixes are NOT gobbled (`1.5f32` splits),
    /// while integer suffixes are (`32i32` is a single int literal).
    #[test]
    fn suffix_asymmetry() {
        assert_eq!(tags("32i32"), vec![IntLiteral, Eof]);
        assert_eq!(tags("1.5f32"), vec![FloatLiteral, Identifier, Eof]);
    }

    /// Every coordinator fixture must lex with no `Invalid` token and end
    /// at `Eof`. Fixtures live at the workspace root (`../../fixtures`
    /// relative to this crate's manifest).
    #[test]
    fn fixtures_lex_clean() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures");
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .expect("fixtures dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |x| x == "sla"))
            .collect();
        names.sort();
        assert!(!names.is_empty(), "no fixtures found in {dir}");
        for path in names {
            let source = std::fs::read(&path).expect("read fixture");
            let mut lexer = Lexer::new(&source);
            loop {
                let tok = lexer.next();
                assert_ne!(
                    tok.tag,
                    Tag::Invalid,
                    "invalid token at {}..{} in {}",
                    tok.loc.start,
                    tok.loc.end,
                    path.display()
                );
                if tok.tag == Tag::Eof {
                    break;
                }
            }
        }
    }
}

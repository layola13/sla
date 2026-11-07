//! Parser core: cursor machinery, name registries and shared helpers.
//!
//! The Zig original (`src/parser.zig`) drives its lexer incrementally and
//! snapshots `(lex, tok)` pairs by value. This port pre-scans the whole
//! token stream up front (the Rust `Lexer` is not `Copy` and exposes no
//! state snapshot), so a snapshot is just a token index; the token stream
//! observed is identical because the lexer is deterministic and pure.
//! See `docs/divergences/parser.md` D-P (b) entries.

use std::collections::HashSet;

use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::{Type, UserDefined};
use sla_lexer::{Lexer, Loc, Tag, Token};

use crate::error::{ParseError, ParseErrorKind};

/// Mirror of the Zig `Parser.Options` fields that gate body parsing.
/// (The Zig `prescan_sla_import_types` option drives a filesystem scan of
/// `.sla` imports from inside the parser; that IO layer is intentionally
/// not reproduced here — see `docs/divergences/parser.md`.)
#[derive(Debug, Clone)]
pub struct ParserOptions {
    pub parse_function_bodies: bool,
    pub function_body_names: Option<HashSet<String>>,
    pub parse_macro_bodies: bool,
    pub macro_body_names: Option<HashSet<String>>,
    pub parse_test_bodies: bool,
}

impl Default for ParserOptions {
    fn default() -> Self {
        ParserOptions {
            parse_function_bodies: true,
            function_body_names: None,
            parse_macro_bodies: true,
            macro_body_names: None,
            parse_test_bodies: true,
        }
    }
}

pub struct Parser<'a> {
    pub(crate) source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    known_types: Vec<String>,
    known_enums: Vec<String>,
    known_modules: Vec<String>,
    current_impl_target: Option<Box<Type>>,
    synth_counter: usize,
    function_body_spans: Vec<(usize, usize)>,
    options: ParserOptions,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Parser<'a> {
        Parser::with_options(source, ParserOptions::default())
    }

    pub fn with_options(source: &'a str, options: ParserOptions) -> Parser<'a> {
        let mut lexer = Lexer::new(source.as_bytes());
        let mut tokens = Vec::new();
        loop {
            let tok = lexer.next();
            let is_eof = tok.tag == Tag::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Parser {
            source,
            tokens,
            pos: 0,
            known_types: Vec::new(),
            known_enums: Vec::new(),
            known_modules: Vec::new(),
            current_impl_target: None,
            synth_counter: 0,
            function_body_spans: Vec::new(),
            options,
        }
    }

    // -- cursor ------------------------------------------------------------

    /// The current token (Zig `self.tok`).
    pub fn tok(&self) -> Token {
        self.tokens[self.pos]
    }

    /// Tag of the current token (Zig `self.peek()`).
    pub fn peek(&self) -> Tag {
        self.tok().tag
    }

    /// Token at an absolute stream index; `Eof` past the end.
    pub fn token_at(&self, idx: usize) -> Token {
        match self.tokens.get(idx) {
            Some(t) => *t,
            None => Token {
                tag: Tag::Eof,
                loc: Loc { start: self.source.len(), end: self.source.len() },
            },
        }
    }

    /// Index of the current token; snapshots are taken/restored in these
    /// units (Zig snapshots the `(lex, tok)` pair).
    pub fn snapshot(&self) -> usize {
        self.pos
    }

    pub fn restore(&mut self, snap: usize) {
        self.pos = snap;
    }

    pub fn advance(&mut self) {
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
    }

    /// Zig `match(tag)`: consume the current token iff it has `tag`.
    pub fn eat(&mut self, tag: Tag) -> bool {
        if self.peek() == tag {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Zig `expect(tag)`: consume or raise `SyntaxError` with the tag named
    /// as the expectation (the Zig `last_expected` channel).
    pub fn expect(&mut self, tag: Tag) -> Result<(), ParseError> {
        if self.eat(tag) {
            Ok(())
        } else {
            Err(self.err_expected(ParseErrorKind::SyntaxError, tag_name(tag).to_string()))
        }
    }

    pub fn expect_identifier(&mut self) -> Result<String, ParseError> {
        if self.peek() == Tag::Identifier {
            let text = self.cur_text().to_string();
            self.advance();
            Ok(text)
        } else {
            Err(self.err_expected(ParseErrorKind::SyntaxError, "identifier".to_string()))
        }
    }

    /// Zig `expectGenericClose`: a `>` closes one generic level; a `>>`
    /// token is rewritten in place into a virtual `>` (start + 1) without
    /// advancing, so the next close consumes the other half.
    pub fn expect_generic_close(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            Tag::GreaterThan => {
                self.advance();
                Ok(())
            }
            Tag::GreaterGreater => {
                let tok = self.tok();
                self.tokens[self.pos] = Token {
                    tag: Tag::GreaterThan,
                    loc: Loc { start: tok.loc.start + 1, end: tok.loc.end },
                };
                Ok(())
            }
            _ => Err(self.err_expected(
                ParseErrorKind::SyntaxError,
                "generic closing bracket".to_string(),
            )),
        }
    }

    // -- source text ---------------------------------------------------------

    /// Source slice for a token (Zig `lex.buffer[tok.loc.start..tok.loc.end]`).
    pub fn text(&self, tok: Token) -> &'a str {
        let end = tok.loc.end.min(self.source.len());
        let start = tok.loc.start.min(end);
        &self.source[start..end]
    }

    pub fn cur_text(&self) -> &'a str {
        self.text(self.tok())
    }

    // -- errors --------------------------------------------------------------

    pub fn err_here(&self, kind: ParseErrorKind) -> ParseError {
        let tok = self.tok();
        ParseError::new(kind, tok.loc.start, tok.loc.end)
    }

    pub fn err_expected(&self, kind: ParseErrorKind, expected: String) -> ParseError {
        let tok = self.tok();
        ParseError::with_expected(kind, tok.loc.start, tok.loc.end, expected)
    }

    // -- name registries -------------------------------------------------------

    pub fn is_known_type_name(&self, name: &str) -> bool {
        self.known_types.iter().any(|n| n == name)
            || (name == "Self" && self.current_impl_target.is_some())
    }

    pub fn is_known_enum_name(&self, name: &str) -> bool {
        self.known_enums.iter().any(|n| n == name)
    }

    pub fn is_known_module_name(&self, name: &str) -> bool {
        self.known_modules.iter().any(|n| n == name)
    }

    pub fn record_known_type(&mut self, name: &str) {
        if !self.known_types.iter().any(|n| n == name) {
            self.known_types.push(name.to_string());
        }
    }

    pub fn record_known_enum(&mut self, name: &str) {
        if !self.known_enums.iter().any(|n| n == name) {
            self.known_enums.push(name.to_string());
        }
    }

    /// Zig records the basename stem of every `.sla` import path as a known
    /// module (used by the `a::b(...)` module-call disambiguation).
    pub fn record_import_module_name(&mut self, path: &str) {
        if !path.ends_with(".sla") {
            return;
        }
        let base = path.rsplit('/').next().unwrap_or(path);
        let stem = &base[..base.len() - ".sla".len()];
        if !stem.is_empty() && !self.known_modules.iter().any(|n| n == stem) {
            self.known_modules.push(stem.to_string());
        }
    }

    /// Mirror of Zig `seedKnownTypeNames`: the import prescan's output
    /// channel. The harness seeds names learned from imported `.sla` files
    /// here because this port does no filesystem IO while parsing.
    pub fn seed_known_type_names(&mut self, types: &[&str], enums: &[&str]) {
        for name in types {
            self.record_known_type(name);
        }
        for name in enums {
            self.record_known_enum(name);
        }
    }

    pub fn known_type_names(&self) -> &[String] {
        &self.known_types
    }

    pub fn known_enum_names(&self) -> &[String] {
        &self.known_enums
    }

    pub fn known_module_names(&self) -> &[String] {
        &self.known_modules
    }

    pub fn set_current_impl_target(&mut self, ty: Option<Box<Type>>) {
        self.current_impl_target = ty;
    }

    pub fn current_impl_target(&self) -> Option<&Type> {
        self.current_impl_target.as_deref()
    }

    // -- synthesized names -----------------------------------------------------

    /// Zig's shared `synth_counter` temp names (`__chainK`, `__qcN`).
    pub fn synth_temp(&mut self, prefix: &str) -> String {
        let name = format!("{}{}", prefix, self.synth_counter);
        self.synth_counter += 1;
        name
    }

    // -- body spans / skipping ---------------------------------------------------

    /// Byte span of the `{ ... }` block starting at the current token,
    /// computed over the token stream (the Zig `peekBlockSpan` re-lexes the
    /// source; the token stream is the same one). Errors `SyntaxError`
    /// when the current token is not `{` or the block never closes.
    pub fn block_span(&self) -> Result<(usize, usize), ParseError> {
        if self.peek() != Tag::LBrace {
            return Err(self.err_expected(ParseErrorKind::SyntaxError, "l_brace".to_string()));
        }
        let start = self.tok().loc.start;
        let mut depth: usize = 0;
        let mut idx = self.pos;
        loop {
            let tok = self.token_at(idx);
            match tok.tag {
                Tag::LBrace => depth += 1,
                Tag::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok((start, tok.loc.end));
                    }
                }
                Tag::Eof => return Err(self.err_here(ParseErrorKind::SyntaxError)),
                _ => {}
            }
            idx += 1;
        }
    }

    /// Consume the `{ ... }` block at the current token without parsing it
    /// (Zig `skipBlockSpan`); returns its byte span.
    pub fn skip_block(&mut self) -> Result<(usize, usize), ParseError> {
        let span = self.block_span()?;
        let mut depth: usize = 0;
        loop {
            let tok = self.tok();
            match tok.tag {
                Tag::LBrace => depth += 1,
                Tag::RBrace => {
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        return Ok(span);
                    }
                    continue;
                }
                Tag::Eof => return Err(self.err_here(ParseErrorKind::SyntaxError)),
                _ => {}
            }
            self.advance();
        }
    }

    pub fn should_parse_fn_body(&self, name: &str) -> bool {
        self.options.parse_function_bodies
            && self.options.function_body_names.as_ref().map_or(true, |set| set.contains(name))
    }

    pub fn should_parse_macro_body(&self, name: &str) -> bool {
        self.options.parse_macro_bodies
            && self.options.macro_body_names.as_ref().map_or(true, |set| set.contains(name))
    }

    pub fn should_parse_test_body(&self) -> bool {
        self.options.parse_test_bodies
    }

    /// Function body spans in parse order (Zig keeps a pointer-keyed side
    /// map; `Vec<Node>` storage has no stable node addresses, so the
    /// association is by parse order — see divergences D-P (b)).
    pub fn record_function_body_span(&mut self, span: (usize, usize)) {
        self.function_body_spans.push(span);
    }

    pub fn function_body_spans(&self) -> &[(usize, usize)] {
        &self.function_body_spans
    }

    // -- shared node constructors ---------------------------------------------

    pub fn make_user_defined_type(name: &str, generics: Vec<Type>) -> Type {
        Type::UserDefined(UserDefined { name: name.to_string(), generics })
    }

    pub fn make_identifier(name: &str) -> Node {
        Node::Identifier(name.to_string())
    }

    pub fn make_call(func_name: &str, args: Vec<Node>) -> Node {
        Node::CallExpr(CallExpr {
            func_name: func_name.to_string(),
            associated_target: None,
            generics: Vec::new(),
            args,
            callee: None,
        })
    }
}

/// Zig `@tagName` spelling of a token tag (used for `expected` strings).
pub fn tag_name(tag: Tag) -> &'static str {
    match tag {
        Tag::Eof => "eof",
        Tag::Invalid => "invalid",
        Tag::Identifier => "identifier",
        Tag::IntLiteral => "int_literal",
        Tag::FloatLiteral => "float_literal",
        Tag::StringLiteral => "string_literal",
        Tag::TemplateStart => "template_start",
        Tag::TemplateString => "template_string",
        Tag::TemplateInterp => "template_interp",
        Tag::TemplateEnd => "template_end",
        Tag::KeywordStruct => "keyword_struct",
        Tag::KeywordUnion => "keyword_union",
        Tag::KeywordEnum => "keyword_enum",
        Tag::KeywordTrait => "keyword_trait",
        Tag::KeywordOverload => "keyword_overload",
        Tag::KeywordDyn => "keyword_dyn",
        Tag::KeywordImpl => "keyword_impl",
        Tag::KeywordMod => "keyword_mod",
        Tag::KeywordUsing => "keyword_using",
        Tag::KeywordPub => "keyword_pub",
        Tag::KeywordExtern => "keyword_extern",
        Tag::KeywordAsync => "keyword_async",
        Tag::KeywordAwait => "keyword_await",
        Tag::KeywordUnsafe => "keyword_unsafe",
        Tag::KeywordAs => "keyword_as",
        Tag::KeywordFn => "keyword_fn",
        Tag::KeywordIf => "keyword_if",
        Tag::KeywordElse => "keyword_else",
        Tag::KeywordMatch => "keyword_match",
        Tag::KeywordSwitch => "keyword_switch",
        Tag::KeywordReturn => "keyword_return",
        Tag::KeywordFor => "keyword_for",
        Tag::KeywordWhile => "keyword_while",
        Tag::KeywordBreak => "keyword_break",
        Tag::KeywordContinue => "keyword_continue",
        Tag::KeywordIn => "keyword_in",
        Tag::KeywordLet => "keyword_let",
        Tag::KeywordConst => "keyword_const",
        Tag::KeywordVar => "keyword_var",
        Tag::KeywordInline => "keyword_inline",
        Tag::KeywordMacro => "keyword_macro",
        Tag::KeywordMut => "keyword_mut",
        Tag::KeywordType => "keyword_type",
        Tag::Plus => "plus",
        Tag::PlusEqual => "plus_equal",
        Tag::MinusEqual => "minus_equal",
        Tag::AsteriskEqual => "asterisk_equal",
        Tag::SlashEqual => "slash_equal",
        Tag::PercentEqual => "percent_equal",
        Tag::PipeEqual => "pipe_equal",
        Tag::AmpersandEqual => "ampersand_equal",
        Tag::Minus => "minus",
        Tag::Asterisk => "asterisk",
        Tag::Slash => "slash",
        Tag::Percent => "percent",
        Tag::Equal => "equal",
        Tag::EqualEqual => "equal_equal",
        Tag::BangEqual => "bang_equal",
        Tag::LessEqual => "less_equal",
        Tag::GreaterEqual => "greater_equal",
        Tag::Ampersand => "ampersand",
        Tag::AmpAmp => "amp_amp",
        Tag::Caret => "caret",
        Tag::Bang => "bang",
        Tag::Pipe => "pipe",
        Tag::PipePipe => "pipe_pipe",
        Tag::LessLess => "less_less",
        Tag::GreaterGreater => "greater_greater",
        Tag::Spaceship => "spaceship",
        Tag::Dot => "dot",
        Tag::Comma => "comma",
        Tag::Semicolon => "semicolon",
        Tag::Colon => "colon",
        Tag::DoubleColon => "double_colon",
        Tag::LParen => "l_paren",
        Tag::RParen => "r_paren",
        Tag::LBrace => "l_brace",
        Tag::RBrace => "r_brace",
        Tag::LBracket => "l_bracket",
        Tag::RBracket => "r_bracket",
        Tag::LessThan => "less_than",
        Tag::GreaterThan => "greater_than",
        Tag::Arrow => "arrow",
        Tag::FatArrow => "fat_arrow",
        Tag::Range => "range",
        Tag::QuestionMark => "question_mark",
        Tag::QuestionQuestion => "question_question",
        Tag::QuestionQuestionEqual => "question_question_equal",
        Tag::QuestionDot => "question_dot",
        Tag::At => "at",
    }
}

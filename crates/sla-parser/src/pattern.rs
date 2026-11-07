//! Pattern parsing, mirroring `parseEnumPattern` / `parseLetPattern` /
//! `parseIfLetChain` in `src/parser.zig`.
//!
//! The two entry points differ exactly as in Zig: `parse_enum_pattern`
//! (match arms, if-let in the original's strict form) rejects a bare name
//! that is not `Some`/`None`/`Ok`/`Err`, while `parse_let_pattern` (let-else
//! and while-let) accepts any bare name as a variant of the default enum.

use sla_ast::exprs::{EnumPattern, IfLetCond};
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;

impl<'a> Parser<'a> {
    /// Zig `parseEnumPattern`: `Name::Variant{bindings}` / `(bindings)`,
    /// or a bare `Some`/`None` (enum `Option`) / `Ok`/`Err` (enum `Result`).
    pub fn parse_enum_pattern(&mut self) -> Result<EnumPattern, ParseError> {
        let first = self.expect_identifier()?;
        let (enum_name, variant_name) = if self.eat(Tag::DoubleColon) {
            let variant = self.expect_identifier()?;
            (first, variant)
        } else if first == "Ok" || first == "Err" {
            ("Result".to_string(), first)
        } else if first == "Some" || first == "None" {
            ("Option".to_string(), first)
        } else {
            return Err(self.err_here(ParseErrorKind::UnexpectedToken));
        };
        let bindings = self.parse_pattern_bindings()?;
        Ok(EnumPattern { enum_name, variant_name, bindings })
    }

    /// Zig `parseLetPattern`: like `parse_enum_pattern`, but a bare name
    /// that is not `Ok`/`Err` defaults to a variant of enum `Option`
    /// instead of being rejected.
    pub fn parse_let_pattern(&mut self) -> Result<EnumPattern, ParseError> {
        let first = self.expect_identifier()?;
        let (enum_name, variant_name) = if self.eat(Tag::DoubleColon) {
            let variant = self.expect_identifier()?;
            (first, variant)
        } else if first == "Ok" || first == "Err" {
            ("Result".to_string(), first)
        } else {
            ("Option".to_string(), first)
        };
        let bindings = self.parse_pattern_bindings()?;
        Ok(EnumPattern { enum_name, variant_name, bindings })
    }

    /// `{a, b}` (comma optional after each) or `(a, b)` (comma required
    /// between bindings) following a pattern head; empty when absent.
    fn parse_pattern_bindings(&mut self) -> Result<Vec<String>, ParseError> {
        let mut bindings = Vec::new();
        if self.eat(Tag::LBrace) {
            loop {
                if self.eat(Tag::RBrace) {
                    break;
                }
                bindings.push(self.expect_identifier()?);
                self.eat(Tag::Comma);
            }
        } else if self.eat(Tag::LParen) {
            loop {
                if self.eat(Tag::RParen) {
                    break;
                }
                bindings.push(self.expect_identifier()?);
                if !self.eat(Tag::Comma) {
                    self.expect(Tag::RParen)?;
                    break;
                }
            }
        }
        Ok(bindings)
    }

    /// Zig `parseIfLetChain`: one or more `let <pattern> = <expr>` joined
    /// by `&&`; each value parses at the precedence of `&&` (2).
    pub fn parse_if_let_chain(&mut self) -> Result<Vec<IfLetCond>, ParseError> {
        let mut chain = Vec::new();
        loop {
            self.expect(Tag::KeywordLet)?;
            let pattern = self.parse_let_pattern()?;
            self.expect(Tag::Equal)?;
            let value = self.parse_expr(2)?;
            chain.push(IfLetCond { pattern, value: Box::new(value) });
            if !self.eat(Tag::AmpAmp) {
                break;
            }
            if self.peek() != Tag::KeywordLet {
                return Err(self.err_here(ParseErrorKind::UnexpectedToken));
            }
        }
        Ok(chain)
    }
}

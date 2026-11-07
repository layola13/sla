//! Generic-lookahead probes and the "tail" parsers they gate, mirroring
//! `looksLikeGenericStructLiteralTail` / `looksLikeGenericCallTail` /
//! `looksLikeGenericFunctionRefTail` / `looksLikeGenericMethodCallTail`
//! and the struct/enum literal + associated/module call parsers of
//! `src/parser.zig`.
//!
//! Probe convention (as in Zig): the `<` has already been consumed and
//! scanning starts at the current token with angle depth 1.

use sla_ast::exprs::{
    CallExpr, EnumLiteral, EnumLiteralField, GenericFuncRef, StructLiteral, StructLiteralField,
};
use sla_ast::node::Node;
use sla_ast::types::Type;
use sla_lexer::Tag;

use crate::error::ParseError;
use crate::parser::Parser;

/// What must follow the matching `>` for a probe to succeed.
#[derive(Clone, Copy)]
enum EndPred {
    LBrace,
    LParen,
    RefBoundary,
}

fn end_pred_matches(pred: EndPred, tag: Tag) -> bool {
    match pred {
        EndPred::LBrace => tag == Tag::LBrace,
        EndPred::LParen => tag == Tag::LParen,
        EndPred::RefBoundary => generic_function_ref_boundary(tag),
    }
}

/// Tokens that, at angle depth 1, prove the `<` was not a generic open
/// (Zig's boundary set in the `looksLike*` scanners).
fn generic_boundary(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::Semicolon
            | Tag::LBrace
            | Tag::RBrace
            | Tag::RParen
            | Tag::RBracket
            | Tag::Equal
            | Tag::Plus
            | Tag::Minus
            | Tag::Asterisk
            | Tag::Slash
            | Tag::Percent
            | Tag::PlusEqual
            | Tag::MinusEqual
            | Tag::AsteriskEqual
            | Tag::SlashEqual
            | Tag::PercentEqual
            | Tag::AmpAmp
            | Tag::PipePipe
            | Tag::Ampersand
            | Tag::Pipe
            | Tag::Caret
            | Tag::LessLess
            | Tag::LessEqual
            | Tag::GreaterEqual
            | Tag::Spaceship
            | Tag::EqualEqual
            | Tag::BangEqual
            | Tag::Arrow
            | Tag::FatArrow
            | Tag::Range
            | Tag::QuestionMark
    )
}

/// Tokens allowed immediately after a generic function reference
/// (`foo::<T>` used as a value).
fn generic_function_ref_boundary(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::Comma | Tag::Semicolon | Tag::RParen | Tag::RBrace | Tag::RBracket | Tag::Eof
    )
}

impl<'a> Parser<'a> {
    /// Shared angle-bracket scan behind all `looksLike*` probes: starts at
    /// token index `start` (first token after `<`) with depth 1.
    fn scan_generic_tail(&self, start: usize, pred: EndPred) -> bool {
        let mut depth: usize = 1;
        let mut idx = start;
        loop {
            let tok = self.token_at(idx);
            if tok.tag == Tag::Eof {
                return false;
            }
            if depth == 1 && generic_boundary(tok.tag) {
                return false;
            }
            match tok.tag {
                Tag::LessThan => depth += 1,
                Tag::GreaterThan => {
                    depth -= 1;
                    if depth == 0 {
                        return end_pred_matches(pred, self.token_at(idx + 1).tag);
                    }
                }
                Tag::GreaterGreater => {
                    if depth <= 2 {
                        return end_pred_matches(pred, self.token_at(idx + 1).tag);
                    }
                    depth -= 2;
                }
                _ => {}
            }
            idx += 1;
        }
    }

    pub fn looks_like_struct_literal_tail(&self) -> bool {
        self.scan_generic_tail(self.snapshot(), EndPred::LBrace)
    }

    pub fn looks_like_generic_call_tail(&self) -> bool {
        self.scan_generic_tail(self.snapshot(), EndPred::LParen)
    }

    pub fn looks_like_generic_function_ref_tail(&self) -> bool {
        self.scan_generic_tail(self.snapshot(), EndPred::RefBoundary)
    }

    /// Method-call variant: the current token must be the `<` itself;
    /// scanning starts one token later.
    pub fn looks_like_generic_method_call_tail(&self) -> bool {
        if self.peek() != Tag::LessThan {
            return false;
        }
        self.scan_generic_tail(self.snapshot() + 1, EndPred::LParen)
    }

    // -- generic argument lists (the `<` is already consumed) ---------------

    fn parse_generic_type_args(&mut self) -> Result<Vec<Type>, ParseError> {
        let mut generics = Vec::new();
        loop {
            generics.push(self.parse_type()?);
            if !self.eat(Tag::Comma) {
                break;
            }
        }
        self.expect_generic_close()?;
        Ok(generics)
    }

    /// `name<T, ...>(args)` with `<` already consumed.
    pub fn parse_generic_call_tail(&mut self, func_name: String) -> Result<Node, ParseError> {
        let generics = self.parse_generic_type_args()?;
        let args = self.parse_call_args()?;
        Ok(Node::CallExpr(CallExpr {
            func_name,
            associated_target: None,
            generics,
            args,
            callee: None,
        }))
    }

    /// `name<T, ...>` used as a value, with `<` already consumed.
    pub fn parse_generic_function_ref_tail(&mut self, func_name: String) -> Result<Node, ParseError> {
        let generics = self.parse_generic_type_args()?;
        Ok(Node::GenericFuncRef(GenericFuncRef { func_name, generics }))
    }

    /// `name<T, ...> { ... }` struct literal, with `<` already consumed.
    pub fn parse_generic_struct_literal_tail(&mut self, name: String) -> Result<Node, ParseError> {
        let generics = self.parse_generic_type_args()?;
        let ty = Parser::make_user_defined_type(&name, generics);
        self.parse_struct_literal(ty)
    }

    // -- calls -----------------------------------------------------------------

    /// `(expr, ...)` argument list; the `(` is consumed here.
    pub fn parse_call_args(&mut self) -> Result<Vec<Node>, ParseError> {
        self.expect(Tag::LParen)?;
        let mut args = Vec::new();
        if self.eat(Tag::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr(0)?);
            if self.eat(Tag::Comma) {
                if self.eat(Tag::RParen) {
                    break;
                }
                continue;
            }
            self.expect(Tag::RParen)?;
            break;
        }
        Ok(args)
    }

    // -- literals ----------------------------------------------------------------

    /// `Ty { name: expr, ..., ..update }` with the type already known and
    /// the current token at `{`.
    pub fn parse_struct_literal(&mut self, ty: Type) -> Result<Node, ParseError> {
        self.expect(Tag::LBrace)?;
        let mut fields = Vec::new();
        let mut update_expr = None;
        loop {
            if self.eat(Tag::RBrace) {
                break;
            }
            if self.eat(Tag::Range) {
                update_expr = Some(Box::new(self.parse_expr(0)?));
                self.eat(Tag::Comma);
                self.expect(Tag::RBrace)?;
                break;
            }
            let field_name = self.expect_identifier()?;
            self.expect(Tag::Colon)?;
            let value = self.parse_expr(0)?;
            fields.push(StructLiteralField { name: field_name, value });
            self.eat(Tag::Comma);
        }
        Ok(Node::StructLiteral(StructLiteral {
            ty: Box::new(ty),
            fields,
            update_expr,
        }))
    }

    /// `Enum::Variant{...}` / `Enum::Variant(...)`; the current token is
    /// the `::` after the enum name.
    pub fn parse_enum_literal(&mut self, enum_name: String) -> Result<Node, ParseError> {
        self.expect(Tag::DoubleColon)?;
        let variant_name = self.expect_identifier()?;
        let mut fields = Vec::new();
        if self.eat(Tag::LBrace) {
            loop {
                if self.eat(Tag::RBrace) {
                    break;
                }
                let field_name = self.expect_identifier()?;
                self.expect(Tag::Colon)?;
                let value = self.parse_expr(0)?;
                fields.push(EnumLiteralField { name: field_name, value });
                self.eat(Tag::Comma);
            }
        } else if self.eat(Tag::LParen) {
            loop {
                if self.eat(Tag::RParen) {
                    break;
                }
                let value = self.parse_expr(0)?;
                fields.push(EnumLiteralField {
                    name: fields.len().to_string(),
                    value,
                });
                if !self.eat(Tag::Comma) {
                    self.expect(Tag::RParen)?;
                    break;
                }
            }
        }
        Ok(Node::EnumLiteral(EnumLiteral {
            enum_name,
            variant_name,
            fields,
        }))
    }

    // -- :: tails ------------------------------------------------------------------

    /// `Target::method(args)`; the current token is the `::`.
    pub fn parse_associated_call(&mut self, target: String) -> Result<Node, ParseError> {
        self.expect(Tag::DoubleColon)?;
        let func_name = self.expect_identifier()?;
        let args = self.parse_call_args()?;
        Ok(Node::CallExpr(CallExpr {
            func_name,
            associated_target: Some(target),
            generics: Vec::new(),
            args,
            callee: None,
        }))
    }

    /// `a::b::name(args)` / `a::name::<T>(args)`; the current token is the
    /// first `::`. Path parts join with `__` into the call's func_name.
    pub fn parse_module_call(&mut self, first: String) -> Result<Node, ParseError> {
        let mut parts = vec![first];
        let mut generics = Vec::new();
        loop {
            if !self.eat(Tag::DoubleColon) {
                break;
            }
            if self.eat(Tag::LessThan) {
                generics = self.parse_generic_type_args()?;
                break;
            }
            parts.push(self.expect_identifier()?);
        }
        let args = self.parse_call_args()?;
        Ok(Node::CallExpr(CallExpr {
            func_name: parts.join("__"),
            associated_target: None,
            generics,
            args,
            callee: None,
        }))
    }
}

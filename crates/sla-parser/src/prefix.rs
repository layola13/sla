//! Prefix expression parsing, mirroring `parsePrefixExpr` and the
//! expression forms it dispatches to (`parseIfExpr`, `parseSwitchExpr`,
//! `parseMatchExpr`, `parseClosureLiteral`, `parseInlineAsmExpr`) in
//! `src/parser.zig`.
//!
//! Desugars applied here, exactly as in Zig:
//!   `-x`  -> `0 - x` at operand precedence 6 (so `-a + b` is `-(a + b)`)
//!   `!x`  -> `x == false` with the operand at precedence 8
//!   `&x` / `^x` / `*x` / `await x` wrap the precedence-6 operand.

use sla_ast::decls::Param;
use sla_ast::exprs::{
    ArrayLiteral, AwaitExpr, BinaryExpr, BinaryOp, Case, ClosureLiteral, IfExpr, InlineAsmExpr,
    InlineAsmOperand, Literal, MatchCase, MatchExpr, RepeatArrayLiteral, SwitchExpr, TupleLiteral,
    UnsafeExpr,
};
use sla_ast::node::Node;
use sla_ast::types::Type;
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::numlit;
use crate::parser::Parser;
use crate::types::string_literal_inner;

impl<'a> Parser<'a> {
    /// Zig `parsePrefixExpr`.
    pub fn parse_prefix_expr(&mut self) -> Result<Node, ParseError> {
        let tag = self.peek();
        match tag {
            Tag::IntLiteral => {
                let tok = self.tok();
                let lexeme = self.cur_text().to_string();
                self.advance();
                numlit::parse_int_literal(&lexeme, tok.loc)
            }
            Tag::FloatLiteral => {
                let tok = self.tok();
                let lexeme = self.cur_text().to_string();
                self.advance();
                numlit::parse_float_literal(&lexeme, tok.loc)
            }
            Tag::StringLiteral => {
                let raw = self.cur_text().to_string();
                self.advance();
                Ok(Node::Literal(Literal::StringVal(
                    string_literal_inner(&raw).to_string(),
                )))
            }
            Tag::TemplateStart => Err(self.err_expected(
                ParseErrorKind::SyntaxError,
                "template literals are not covered by the sla-parser slice yet".to_string(),
            )),
            Tag::Identifier => self.parse_identifier_prefix(),
            Tag::Ampersand => {
                self.advance();
                let inner = self.parse_expr(6)?;
                Ok(Node::BorrowExpr(sla_ast::exprs::BorrowExpr {
                    expr: Box::new(inner),
                }))
            }
            Tag::Caret => {
                self.advance();
                let inner = self.parse_expr(6)?;
                Ok(Node::MoveExpr(sla_ast::exprs::MoveExpr {
                    expr: Box::new(inner),
                }))
            }
            Tag::Asterisk => {
                self.advance();
                let inner = self.parse_expr(6)?;
                Ok(Node::DerefExpr(sla_ast::exprs::DerefExpr {
                    expr: Box::new(inner),
                }))
            }
            Tag::Minus => {
                self.advance();
                let operand = self.parse_expr(6)?;
                Ok(Node::BinaryExpr(BinaryExpr {
                    op: BinaryOp::Sub,
                    left: Box::new(Node::Literal(Literal::IntVal(0))),
                    right: Box::new(operand),
                }))
            }
            Tag::Bang => {
                self.advance();
                let operand = self.parse_expr(8)?;
                Ok(Node::BinaryExpr(BinaryExpr {
                    op: BinaryOp::Eq,
                    left: Box::new(operand),
                    right: Box::new(Node::Literal(Literal::BoolVal(false))),
                }))
            }
            Tag::KeywordIf => self.parse_if_expr(),
            Tag::KeywordSwitch => self.parse_switch_expr(),
            Tag::KeywordMatch => self.parse_match_expr(),
            Tag::KeywordUnsafe => {
                self.advance();
                let body = self.parse_block()?;
                Ok(Node::UnsafeExpr(UnsafeExpr { body }))
            }
            Tag::KeywordAwait => {
                self.advance();
                let inner = self.parse_expr(6)?;
                Ok(Node::AwaitExpr(AwaitExpr {
                    expr: Box::new(inner),
                }))
            }
            Tag::Pipe | Tag::PipePipe => self.parse_closure_literal(),
            Tag::LParen => {
                self.advance();
                if self.eat(Tag::RParen) {
                    return Ok(Node::TupleLiteral(TupleLiteral { elements: Vec::new() }));
                }
                let first = self.parse_expr(0)?;
                if self.eat(Tag::Comma) {
                    let mut elements = vec![first];
                    loop {
                        if self.eat(Tag::RParen) {
                            break;
                        }
                        elements.push(self.parse_expr(0)?);
                        if !self.eat(Tag::Comma) {
                            self.expect(Tag::RParen)?;
                            break;
                        }
                    }
                    Ok(Node::TupleLiteral(TupleLiteral { elements }))
                } else {
                    self.expect(Tag::RParen)?;
                    Ok(first)
                }
            }
            Tag::LBracket => {
                self.advance();
                if self.eat(Tag::RBracket) {
                    return Ok(Node::ArrayLiteral(ArrayLiteral { elements: Vec::new() }));
                }
                let first = self.parse_expr(0)?;
                if self.eat(Tag::Semicolon) {
                    // `[value; len]` repeat literal.
                    if self.peek() != Tag::IntLiteral {
                        return Err(self.err_here(ParseErrorKind::InvalidCharacter));
                    }
                    let raw = self.cur_text().to_string();
                    let clean = numlit::strip_numeric_separators(&raw);
                    let len: usize = clean
                        .parse::<usize>()
                        .map_err(|_| self.err_here(ParseErrorKind::InvalidCharacter))?;
                    self.advance();
                    self.expect(Tag::RBracket)?;
                    Ok(Node::RepeatArrayLiteral(RepeatArrayLiteral {
                        value: Box::new(first),
                        len,
                    }))
                } else {
                    let mut elements = vec![first];
                    loop {
                        if self.eat(Tag::RBracket) {
                            break;
                        }
                        self.expect(Tag::Comma)?;
                        if self.eat(Tag::RBracket) {
                            break;
                        }
                        elements.push(self.parse_expr(0)?);
                    }
                    Ok(Node::ArrayLiteral(ArrayLiteral { elements }))
                }
            }
            _ => Err(self.err_here(ParseErrorKind::UnexpectedToken)),
        }
    }

    /// Identifier-led prefix forms: `asm!`, the `::` decision tree
    /// (enum literal / associated call / module call / `Ordering::X`),
    /// struct literals, and the `true` / `false` literals.
    fn parse_identifier_prefix(&mut self) -> Result<Node, ParseError> {
        let name = self.cur_text().to_string();
        self.advance();

        if name == "asm" && self.eat(Tag::Bang) {
            return self.parse_inline_asm_expr();
        }

        if self.peek() == Tag::DoubleColon {
            if self.is_known_enum_name(&name) {
                return self.parse_enum_literal(name);
            }
            if self.is_known_module_name(&name) {
                return self.parse_module_call(name);
            }
            // Lookahead over the token stream decides between a module
            // call (`a::b::f(...)`), an associated call (`a::f(...)`),
            // the `Ordering::Variant` value form, and the default:
            // an enum literal.
            let next1 = self.token_at(self.snapshot() + 1);
            if next1.tag == Tag::Identifier {
                let next2 = self.token_at(self.snapshot() + 2);
                if next2.tag == Tag::DoubleColon {
                    return self.parse_module_call(name);
                }
                if next2.tag == Tag::LParen {
                    return self.parse_associated_call(name);
                }
                if name == "Ordering" {
                    self.advance(); // `::`
                    let assoc = self.expect_identifier()?;
                    return Ok(Parser::make_identifier(&format!("Ordering::{}", assoc)));
                }
            }
            return self.parse_enum_literal(name);
        }

        if self.peek() == Tag::LBrace && self.is_known_type_name(&name) {
            let ty = Parser::make_user_defined_type(&name, Vec::new());
            return self.parse_struct_literal(ty);
        }

        match name.as_str() {
            "true" => Ok(Node::Literal(Literal::BoolVal(true))),
            "false" => Ok(Node::Literal(Literal::BoolVal(false))),
            _ => Ok(Node::Identifier(name)),
        }
    }

    /// Zig `parseIfExpr`, including if-let chains and the `else if`
    /// normalization (the nested if becomes a single expr-statement).
    pub fn parse_if_expr(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordIf)?;
        let (cond, let_chain) = if self.peek() == Tag::KeywordLet {
            let chain = self.parse_if_let_chain()?;
            let cond = (*chain[0].value).clone();
            (cond, Some(chain))
        } else {
            (self.parse_expr(0)?, None)
        };
        let then_block = self.parse_block()?;
        let else_block = if self.eat(Tag::KeywordElse) {
            if self.peek() == Tag::KeywordIf {
                let nested = self.parse_if_expr()?;
                Some(vec![Node::ExprStmt(Box::new(nested))])
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        Ok(Node::IfExpr(IfExpr {
            cond: Box::new(cond),
            let_chain,
            then_block,
            else_block,
        }))
    }

    /// Zig `parseSwitchExpr`: `switch val { expr => block-or-stmt, ... }`.
    pub fn parse_switch_expr(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordSwitch)?;
        let val = self.parse_expr(0)?;
        self.expect(Tag::LBrace)?;
        let mut cases = Vec::new();
        loop {
            if self.eat(Tag::RBrace) {
                break;
            }
            let pattern = self.parse_expr(0)?;
            self.expect(Tag::FatArrow)?;
            let body = if self.peek() == Tag::LBrace {
                self.parse_block()?
            } else if is_stmt_keyword(self.peek()) {
                vec![self.parse_stmt()?]
            } else {
                let arm = self.parse_expr(0)?;
                vec![Node::ExprStmt(Box::new(arm))]
            };
            cases.push(Case {
                pattern: Box::new(pattern),
                body,
            });
            self.eat(Tag::Comma);
        }
        Ok(Node::SwitchExpr(SwitchExpr {
            val: Box::new(val),
            cases,
        }))
    }

    /// Zig `parseMatchExpr`: `match val { Pattern (if guard)? => block, ... }`.
    pub fn parse_match_expr(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordMatch)?;
        let val = self.parse_expr(0)?;
        self.expect(Tag::LBrace)?;
        let mut cases = Vec::new();
        loop {
            if self.eat(Tag::RBrace) {
                break;
            }
            let pattern = self.parse_enum_pattern()?;
            let guard = if self.eat(Tag::KeywordIf) {
                Some(Box::new(self.parse_expr(0)?))
            } else {
                None
            };
            self.expect(Tag::FatArrow)?;
            let body = self.parse_block()?;
            cases.push(MatchCase {
                pattern,
                guard,
                body,
            });
            self.eat(Tag::Comma);
        }
        Ok(Node::MatchExpr(MatchExpr {
            val: Box::new(val),
            cases,
        }))
    }

    /// Zig `parseClosureLiteral`: `|| expr` or `|name (: type)?, ...| expr`;
    /// untyped parameters get the `Infer` type.
    pub fn parse_closure_literal(&mut self) -> Result<Node, ParseError> {
        let mut params: Vec<Param> = Vec::new();
        if !self.eat(Tag::PipePipe) {
            self.expect(Tag::Pipe)?;
            loop {
                if self.eat(Tag::Pipe) {
                    break;
                }
                let param_name = self.expect_identifier()?;
                let ty = if self.eat(Tag::Colon) {
                    self.parse_type()?
                } else {
                    Type::Infer
                };
                params.push(Param {
                    name: param_name,
                    ty: Box::new(ty),
                    is_borrow: false,
                    is_move: false,
                });
                if !self.eat(Tag::Comma) {
                    self.expect(Tag::Pipe)?;
                    break;
                }
            }
        }
        let body = self.parse_expr(0)?;
        Ok(Node::ClosureLiteral(ClosureLiteral {
            params,
            body: Box::new(body),
        }))
    }

    /// Zig `parseInlineAsmExpr` after `asm!`: `("template", dir("reg") var, ...)`.
    /// The direction identifier (`in`/`out`/...) is consumed and dropped;
    /// the operand's `constraint` takes the string literal's contents —
    /// see divergences D-P (b) for the unverified field assignment.
    pub fn parse_inline_asm_expr(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::LParen)?;
        if self.peek() != Tag::StringLiteral {
            return Err(self.err_expected(
                ParseErrorKind::SyntaxError,
                "string_literal".to_string(),
            ));
        }
        let raw = self.cur_text().to_string();
        self.advance();
        let template = string_literal_inner(&raw).to_string();
        let mut operands = Vec::new();
        while self.eat(Tag::Comma) {
            self.expect_identifier()?; // direction keyword
            self.expect(Tag::LParen)?;
            if self.peek() != Tag::StringLiteral {
                return Err(self.err_expected(
                    ParseErrorKind::SyntaxError,
                    "string_literal".to_string(),
                ));
            }
            let raw_constraint = self.cur_text().to_string();
            self.advance();
            let constraint = string_literal_inner(&raw_constraint).to_string();
            self.expect(Tag::RParen)?;
            let var_name = self.expect_identifier()?;
            operands.push(InlineAsmOperand {
                constraint,
                var_name,
            });
        }
        self.expect(Tag::RParen)?;
        Ok(Node::InlineAsmExpr(InlineAsmExpr {
            template,
            operands,
        }))
    }
}

/// Statement keywords that may lead a single-statement switch arm.
fn is_stmt_keyword(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::KeywordLet
            | Tag::KeywordConst
            | Tag::KeywordVar
            | Tag::KeywordReturn
            | Tag::KeywordFor
            | Tag::KeywordWhile
            | Tag::KeywordBreak
            | Tag::KeywordContinue
            | Tag::KeywordUsing
    )
}

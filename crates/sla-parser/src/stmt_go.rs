//! R-3 goroutine batch statement parsing: `go`, `defer` and
//! `select` statements.
//!
//! `go` and `defer` take a single call-shaped operand followed
//! by `;`. Whether the operand really is a call (and a legal
//! spawn target) is the checker's business — parse keeps any
//! expression so the checker can name the shape fault
//! (DeferNonCall / GoClosureCallee / GoMethodCallee).
//!
//! `select` arms: `case let (v, closed) = ch.recv() { ... }`,
//! `case ch.recv() { ... }` and `default { ... }`. `case` and
//! `default` are contextual identifiers (not lexer keywords):
//! they only carry arm meaning directly inside a select block.

use sla_ast::node::Node;
use sla_ast::stmts::{DeferStmt, GoStmt, SelectArm, SelectStmt};
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;

impl<'a> Parser<'a> {
    pub fn parse_go_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordGo)?;
        let call = self.parse_expr(0)?;
        self.expect(Tag::Semicolon)?;
        Ok(Node::GoStmt(GoStmt {
            call: Box::new(call),
        }))
    }

    pub fn parse_defer_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordDefer)?;
        let call = self.parse_expr(0)?;
        self.expect(Tag::Semicolon)?;
        Ok(Node::DeferStmt(DeferStmt {
            call: Box::new(call),
        }))
    }

    pub fn parse_select_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordSelect)?;
        self.expect(Tag::LBrace)?;
        let mut arms = Vec::new();
        while self.peek() != Tag::RBrace && self.peek() != Tag::Eof {
            if self.peek() == Tag::Identifier && self.cur_text() == "default" {
                self.advance();
                let body = self.parse_block()?;
                arms.push(SelectArm {
                    pattern: None,
                    recv: None,
                    body,
                    is_default: true,
                });
                continue;
            }
            if !(self.peek() == Tag::Identifier && self.cur_text() == "case") {
                return Err(self.err_expected(
                    ParseErrorKind::SyntaxError,
                    "select arm must start with `case` or `default`".to_string(),
                ));
            }
            self.advance(); // `case`
            let mut pattern = None;
            if self.eat(Tag::KeywordLet) {
                if self.eat(Tag::LParen) {
                    let v = self.expect_identifier()?;
                    self.expect(Tag::Comma)?;
                    let closed = self.expect_identifier()?;
                    self.expect(Tag::RParen)?;
                    pattern = Some((v, Some(closed)));
                } else {
                    let v = self.expect_identifier()?;
                    pattern = Some((v, None));
                }
                self.expect(Tag::Equal)?;
            }
            let recv = self.parse_expr(0)?;
            let body = self.parse_block()?;
            arms.push(SelectArm {
                pattern,
                recv: Some(Box::new(recv)),
                body,
                is_default: false,
            });
        }
        self.expect(Tag::RBrace)?;
        // A trailing `;` after a select statement is tolerated in
        // the corpus (`select { ... };` inside if branches is not,
        // per house rules — the plain statement form eats one).
        Ok(Node::SelectStmt(SelectStmt { arms }))
    }
}

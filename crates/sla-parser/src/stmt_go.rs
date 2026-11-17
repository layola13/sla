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
//! `case ch.recv() { ... }`, `default { ... }` and
//! `timeout(expr) { ... }`. `case`, `default` and `timeout`
//! are contextual identifiers (not lexer keywords): they only
//! carry arm meaning directly inside a select block.

use sla_ast::node::Node;
use sla_ast::stmts::{DeferStmt, GoStmt, SelectArm, SelectStmt, SelectTimeout};
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;

impl<'a> Parser<'a> {
    /// F-G2 contextual `supervised` marker (Zig parser.zig
    /// parseGoExpr): directly after `go`, the identifier
    /// `supervised` is consumed as the supervised-spawn flag
    /// only when the token after it is another identifier (the
    /// callee). `go supervised(...)` therefore stays a plain
    /// spawn of a function named `supervised`.
    pub(crate) fn eat_contextual_supervised(&mut self) -> bool {
        if self.peek() != Tag::Identifier || self.cur_text() != "supervised" {
            return false;
        }
        if self.peek2() != Tag::Identifier {
            return false;
        }
        self.advance();
        true
    }

    pub fn parse_go_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordGo)?;
        let supervised = self.eat_contextual_supervised();
        let call = self.parse_expr(0)?;
        self.expect(Tag::Semicolon)?;
        Ok(Node::GoStmt(GoStmt {
            supervised,
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
        let mut timeout = None;
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
            if self.peek() == Tag::Identifier && self.cur_text() == "timeout" {
                // Zig parseSelectStmt: at most one timeout arm,
                // `timeout` `(` expr `)` block.
                if timeout.is_some() {
                    return Err(self.err_expected(
                        ParseErrorKind::SyntaxError,
                        "duplicate select timeout arm".to_string(),
                    ));
                }
                self.advance();
                self.expect(Tag::LParen)?;
                let expr = self.parse_expr(0)?;
                self.expect(Tag::RParen)?;
                let body = self.parse_block()?;
                timeout = Some(SelectTimeout {
                    expr: Box::new(expr),
                    body,
                });
                continue;
            }
            if !(self.peek() == Tag::Identifier && self.cur_text() == "case") {
                return Err(self.err_expected(
                    ParseErrorKind::SyntaxError,
                    "select arm must start with `case`, `default` or `timeout`".to_string(),
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
        Ok(Node::SelectStmt(SelectStmt { arms, timeout }))
    }
}

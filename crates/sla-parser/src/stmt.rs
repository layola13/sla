//! Statement parsing, mirroring `parseBlock` / `parseStmt` and the
//! statement parsers of `src/parser.zig`, including the assignment
//! desugars (chained `=`, compound `op=`, and `??=`).

use sla_ast::exprs::{BinaryExpr, BinaryOp};
use sla_ast::node::Node;
use sla_ast::stmts::{
    AssignStmt, BlockStmt, BreakStmt, ContinueStmt, ForStmt, LetDestructureStmt, LetElseStmt,
    LetStmt, ConstStmt, ReleaseStmt, ReturnStmt, VarStmt, WhileStmt,
};
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;

impl<'a> Parser<'a> {
    /// Zig `parseBlock`: `{ stmt* }`.
    pub fn parse_block(&mut self) -> Result<Vec<Node>, ParseError> {
        self.expect(Tag::LBrace)?;
        let mut stmts = Vec::new();
        while self.peek() != Tag::RBrace && self.peek() != Tag::Eof {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(Tag::RBrace)?;
        Ok(stmts)
    }

    /// Zig `parseStmt`.
    pub fn parse_stmt(&mut self) -> Result<Node, ParseError> {
        match self.peek() {
            Tag::KeywordUsing => self.parse_using_decl(),
            Tag::KeywordLet => self.parse_let_stmt(),
            Tag::KeywordConst => self.parse_const_stmt(),
            Tag::KeywordVar => self.parse_var_stmt(),
            Tag::KeywordReturn => self.parse_return_stmt(),
            Tag::KeywordFor => self.parse_for_stmt(),
            Tag::KeywordWhile => self.parse_while_stmt(),
            Tag::KeywordBreak => {
                self.advance();
                self.expect(Tag::Semicolon)?;
                Ok(Node::BreakStmt(BreakStmt))
            }
            Tag::KeywordContinue => {
                self.advance();
                self.expect(Tag::Semicolon)?;
                Ok(Node::ContinueStmt(ContinueStmt))
            }
            Tag::Bang => self.parse_release_stmt(),
            Tag::LBrace => {
                let body = self.parse_block()?;
                Ok(Node::BlockStmt(BlockStmt { body }))
            }
            _ => self.parse_expr_stmt(),
        }
    }

    /// Expression-led statements: plain expression statements, `=`
    /// assignment (with the multi-target chain desugar), compound
    /// assignment, and `??=`.
    fn parse_expr_stmt(&mut self) -> Result<Node, ParseError> {
        // `name:` at statement level would be a loop label; the Zig
        // parser rejects those loudly with guidance.
        if self.peek() == Tag::Identifier && self.token_at(self.snapshot() + 1).tag == Tag::Colon {
            return Err(self.err_expected(
                ParseErrorKind::SyntaxError,
                "loop labels are not supported".to_string(),
            ));
        }
        let expr = self.parse_expr(0)?;
        let stmt = if self.eat(Tag::Equal) {
            // Assignment; further `=` links make it a chain.
            let mut targets = vec![expr];
            let value = loop {
                let next = self.parse_expr(0)?;
                if self.eat(Tag::Equal) {
                    targets.push(next);
                } else {
                    break next;
                }
            };
            if targets.len() == 1 {
                Node::AssignStmt(AssignStmt {
                    target: Box::new(targets.into_iter().next().unwrap()),
                    value: Box::new(value),
                })
            } else {
                // `a = b = v` -> `let __chainK = v; b = __chainK; a = __chainK;`
                // (assignments emitted in reverse target order).
                let tmp = self.synth_temp("__chain");
                let mut body = vec![Node::LetStmt(LetStmt {
                    name: tmp.clone(),
                    ty: None,
                    value: Box::new(value),
                })];
                for target in targets.into_iter().rev() {
                    body.push(Node::AssignStmt(AssignStmt {
                        target: Box::new(target),
                        value: Box::new(Parser::make_identifier(&tmp)),
                    }));
                }
                Node::BlockStmt(BlockStmt { body })
            }
        } else if let Some(op) = compound_assign_op(self.peek()) {
            self.advance();
            let rhs = self.parse_expr(0)?;
            Node::AssignStmt(AssignStmt {
                target: Box::new(expr.clone()),
                value: Box::new(Node::BinaryExpr(BinaryExpr {
                    op,
                    left: Box::new(expr),
                    right: Box::new(rhs),
                })),
            })
        } else if self.eat(Tag::QuestionQuestionEqual) {
            // `a ??= v` -> `a = Some(unwrap_or(a, v))`
            let rhs = self.parse_expr(0)?;
            let inner = Parser::make_call("unwrap_or", vec![expr.clone(), rhs]);
            let wrapped = Parser::make_call("Some", vec![inner]);
            Node::AssignStmt(AssignStmt {
                target: Box::new(expr),
                value: Box::new(wrapped),
            })
        } else {
            Node::ExprStmt(Box::new(expr))
        };
        // The trailing `;` is optional only immediately before `}`.
        if !self.eat(Tag::Semicolon) && self.peek() != Tag::RBrace {
            return Err(self.err_expected(ParseErrorKind::SyntaxError, "semicolon".to_string()));
        }
        Ok(stmt)
    }

    /// Zig `parseLetStmt`, including let-else (pattern gate), tuple
    /// destructure and slice destructure forms.
    pub fn parse_let_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordLet)?;
        // Legacy `let mut` is rejected by the Zig parser.
        if self.peek() == Tag::KeywordMut {
            return Err(self.err_here(ParseErrorKind::UnexpectedToken));
        }
        // Let-else gate: a leading pattern name (`Some`/`None`/`Option`/
        // `Ok`/`Err`/`Result`, or a known enum name) routes to the
        // pattern form, exactly as in Zig.
        if self.peek() == Tag::Identifier {
            let first = self.cur_text().to_string();
            if matches!(first.as_str(), "Some" | "None" | "Option" | "Ok" | "Err" | "Result")
                || self.is_known_enum_name(&first)
            {
                let pattern = self.parse_let_pattern()?;
                self.expect(Tag::Equal)?;
                let value = self.parse_expr(0)?;
                self.expect(Tag::KeywordElse)?;
                let else_block = self.parse_block()?;
                self.expect(Tag::Semicolon)?;
                return Ok(Node::LetElseStmt(LetElseStmt {
                    pattern,
                    value: Box::new(value),
                    else_block,
                }));
            }
        }
        if self.eat(Tag::LParen) {
            let mut names = Vec::new();
            loop {
                if self.eat(Tag::RParen) {
                    break;
                }
                names.push(self.expect_identifier()?);
                if !self.eat(Tag::Comma) {
                    self.expect(Tag::RParen)?;
                    break;
                }
            }
            self.expect(Tag::Equal)?;
            let value = self.parse_expr(0)?;
            self.expect(Tag::Semicolon)?;
            return Ok(Node::LetDestructureStmt(LetDestructureStmt {
                names,
                value: Box::new(value),
                rest_name: None,
                rest_alias: None,
                is_slice: false,
            }));
        }
        if self.eat(Tag::LBracket) {
            let mut names = Vec::new();
            let mut rest_name = None;
            let mut rest_alias = None;
            loop {
                if self.eat(Tag::RBracket) {
                    break;
                }
                if self.eat(Tag::Range) {
                    rest_name = Some(self.expect_identifier()?);
                    if self.eat(Tag::KeywordAs) {
                        rest_alias = Some(self.expect_identifier()?);
                    }
                    self.expect(Tag::RBracket)?;
                    break;
                }
                names.push(self.expect_identifier()?);
                self.eat(Tag::Comma);
            }
            self.expect(Tag::Equal)?;
            let value = self.parse_expr(0)?;
            self.expect(Tag::Semicolon)?;
            return Ok(Node::LetDestructureStmt(LetDestructureStmt {
                names,
                value: Box::new(value),
                rest_name,
                rest_alias,
                is_slice: true,
            }));
        }
        let name = self.expect_identifier()?;
        let ty = if self.eat(Tag::Colon) {
            Some(Box::new(self.parse_type()?))
        } else {
            None
        };
        self.expect(Tag::Equal)?;
        let value = self.parse_expr(0)?;
        self.expect(Tag::Semicolon)?;
        Ok(Node::LetStmt(LetStmt { name, ty, value: Box::new(value) }))
    }

    /// Zig `parseConstStmt` (also the shape of a top-level `const` decl).
    pub fn parse_const_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordConst)?;
        let name = self.expect_identifier()?;
        let ty = if self.eat(Tag::Colon) {
            Some(Box::new(self.parse_type()?))
        } else {
            None
        };
        self.expect(Tag::Equal)?;
        let value = self.parse_expr(0)?;
        self.expect(Tag::Semicolon)?;
        Ok(Node::ConstStmt(ConstStmt { name, ty, value: Box::new(value) }))
    }

    /// Zig `parseVarStmt`: the type annotation is required and there is
    /// no initializer.
    pub fn parse_var_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordVar)?;
        let name = self.expect_identifier()?;
        self.expect(Tag::Colon)?;
        let ty = self.parse_type()?;
        self.expect(Tag::Semicolon)?;
        Ok(Node::VarStmt(VarStmt { name, ty: Box::new(ty) }))
    }

    /// Zig `parseReturnStmt`.
    pub fn parse_return_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordReturn)?;
        if self.eat(Tag::Semicolon) {
            return Ok(Node::ReturnStmt(ReturnStmt { value: None }));
        }
        let value = self.parse_expr(0)?;
        self.expect(Tag::Semicolon)?;
        Ok(Node::ReturnStmt(ReturnStmt { value: Some(Box::new(value)) }))
    }

    /// Zig `parseForStmt`: `for name in expr (.. expr)? block`.
    pub fn parse_for_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordFor)?;
        let var_name = self.expect_identifier()?;
        self.expect(Tag::KeywordIn)?;
        let start = self.parse_expr(0)?;
        let end = if self.eat(Tag::Range) {
            Some(Box::new(self.parse_expr(0)?))
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(Node::ForStmt(ForStmt {
            var_name,
            start: Box::new(start),
            end,
            body,
        }))
    }

    /// Zig `parseWhileStmt`, including the while-let form.
    pub fn parse_while_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordWhile)?;
        let (cond, let_pattern) = if self.eat(Tag::KeywordLet) {
            let pattern = self.parse_let_pattern()?;
            self.expect(Tag::Equal)?;
            let cond = self.parse_expr(0)?;
            (cond, Some(pattern))
        } else {
            (self.parse_expr(0)?, None)
        };
        let body = self.parse_block()?;
        Ok(Node::WhileStmt(WhileStmt {
            cond: Box::new(cond),
            let_pattern,
            body,
        }))
    }

    /// Zig `parseReleaseStmt`: `!name;`.
    pub fn parse_release_stmt(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::Bang)?;
        let var_name = self.expect_identifier()?;
        self.expect(Tag::Semicolon)?;
        Ok(Node::ReleaseStmt(ReleaseStmt { var_name }))
    }
}

/// Compound assignment operators and their binary-op desugar targets.
/// (There is no `^=` token in the lexer, hence no `BitXor` entry.)
fn compound_assign_op(tag: Tag) -> Option<BinaryOp> {
    Some(match tag {
        Tag::PlusEqual => BinaryOp::Add,
        Tag::MinusEqual => BinaryOp::Sub,
        Tag::AsteriskEqual => BinaryOp::Mul,
        Tag::SlashEqual => BinaryOp::Div,
        Tag::PercentEqual => BinaryOp::Mod,
        Tag::PipeEqual => BinaryOp::BitOr,
        Tag::AmpersandEqual => BinaryOp::BitAnd,
        _ => return None,
    })
}

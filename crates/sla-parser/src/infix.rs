//! Infix expression parsing, mirroring `parseInfixExpr` in
//! `src/parser.zig`. The operator token is consumed on entry; binary
//! right-hand sides parse at the operator's own precedence, which is
//! what makes the binary operators left-associative.

use sla_ast::exprs::{
    AwaitExpr, BinaryExpr, BinaryOp, CallExpr, CastExpr, FieldExpr, IfExpr, IndexExpr, SliceExpr,
    TryExpr,
};
use sla_ast::node::Node;
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::expr::token_starts_prefix_expr;
use crate::parser::Parser;

impl<'a> Parser<'a> {
    /// Zig `parseInfixExpr`.
    pub fn parse_infix_expr(&mut self, left: Node, precedence: u8) -> Result<Node, ParseError> {
        let tag = self.peek();
        self.advance();
        match tag {
            Tag::Dot => self.parse_dot_infix(left),
            Tag::Bang => {
                // Macro-style call `name!(args)`. NOTE: `parse_expr`'s
                // postfix-`!` interception consumes every `!` before this
                // branch can be reached from the main loop; kept for
                // structural parity with Zig (see divergences D-P (c)).
                if self.peek() != Tag::LParen {
                    return Err(self.err_here(ParseErrorKind::UnexpectedInfixToken));
                }
                match left {
                    Node::Identifier(name) => Ok(Parser::make_call(&name, self.parse_call_args()?)),
                    _ => Err(self.err_here(ParseErrorKind::InvalidCallTarget)),
                }
            }
            Tag::LParen => match left {
                Node::Identifier(name) => {
                    let args = self.parse_call_args_after_open()?;
                    Ok(Node::CallExpr(CallExpr {
                        func_name: name,
                        associated_target: None,
                        generics: Vec::new(),
                        args,
                        callee: None,
                    }))
                }
                Node::ClosureLiteral(closure) => {
                    let args = self.parse_call_args_after_open()?;
                    Ok(Node::CallExpr(CallExpr {
                        func_name: String::new(),
                        associated_target: None,
                        generics: Vec::new(),
                        args,
                        callee: Some(Box::new(Node::ClosureLiteral(closure))),
                    }))
                }
                _ => Err(self.err_here(ParseErrorKind::InvalidCallTarget)),
            },
            Tag::LBracket => {
                // Zig parses the start expression unconditionally, so
                // `a[..b]` (no start) fails in the prefix parser on `..`.
                let index = self.parse_expr(0)?;
                if self.eat(Tag::Range) {
                    let end = self.parse_expr(0)?;
                    self.expect(Tag::RBracket)?;
                    Ok(Node::SliceExpr(SliceExpr {
                        target: Box::new(left),
                        start: Box::new(index),
                        end: Box::new(end),
                    }))
                } else {
                    self.expect(Tag::RBracket)?;
                    Ok(Node::IndexExpr(IndexExpr {
                        target: Box::new(left),
                        index: Box::new(index),
                    }))
                }
            }
            Tag::KeywordAs => {
                let ty = self.parse_type()?;
                Ok(Node::CastExpr(CastExpr {
                    expr: Box::new(left),
                    ty: Box::new(ty),
                }))
            }
            Tag::QuestionMark => {
                // Ternary if the following tokens form one (snapshot is
                // taken AFTER the `?`, as in Zig); otherwise postfix try.
                let saved = self.snapshot();
                if token_starts_prefix_expr(self.peek()) {
                    match self.parse_ternary_tail() {
                        Ok((then_node, else_node)) => {
                            return Ok(Node::IfExpr(IfExpr {
                                cond: Box::new(left),
                                let_chain: None,
                                then_block: vec![Node::ExprStmt(Box::new(then_node))],
                                else_block: Some(vec![Node::ExprStmt(Box::new(else_node))]),
                            }));
                        }
                        Err(_) => self.restore(saved),
                    }
                }
                Ok(Node::TryExpr(TryExpr {
                    expr: Box::new(left),
                }))
            }
            Tag::LessThan => {
                // Generic tails were pre-probed by `parse_expr`; re-check
                // on the identifier before falling back to comparison.
                if let Node::Identifier(name) = &left {
                    let name = name.clone();
                    if self.looks_like_struct_literal_tail() {
                        return self.parse_generic_struct_literal_tail(name);
                    }
                    if self.looks_like_generic_call_tail() {
                        return self.parse_generic_call_tail(name);
                    }
                    if self.looks_like_generic_function_ref_tail() {
                        return self.parse_generic_function_ref_tail(name);
                    }
                }
                let rhs = self.parse_expr(precedence)?;
                Ok(Node::BinaryExpr(BinaryExpr {
                    op: BinaryOp::Lt,
                    left: Box::new(left),
                    right: Box::new(rhs),
                }))
            }
            _ => {
                let op = binary_op_for(tag)
                    .ok_or_else(|| self.err_here(ParseErrorKind::UnexpectedInfixToken))?;
                let rhs = self.parse_expr(precedence)?;
                Ok(Node::BinaryExpr(BinaryExpr {
                    op,
                    left: Box::new(left),
                    right: Box::new(rhs),
                }))
            }
        }
    }

    /// `.field`, `.method(args)`, `.method::<T>(args)`, `.await`.
    /// The `.` is already consumed.
    fn parse_dot_infix(&mut self, left: Node) -> Result<Node, ParseError> {
        if self.eat(Tag::KeywordAwait) {
            return Ok(Node::AwaitExpr(AwaitExpr {
                expr: Box::new(left),
            }));
        }
        let field_tok = self.tok();
        if field_tok.tag != Tag::Identifier && field_tok.tag != Tag::IntLiteral {
            return Err(self.err_here(ParseErrorKind::UnexpectedToken));
        }
        let field_name = self.text(field_tok).to_string();
        self.advance();
        if self.peek() == Tag::LParen {
            let mut args = vec![left];
            args.extend(self.parse_call_args()?);
            return Ok(Node::CallExpr(CallExpr {
                func_name: field_name,
                associated_target: None,
                generics: Vec::new(),
                args,
                callee: None,
            }));
        }
        if self.looks_like_generic_method_call_tail() {
            self.advance(); // `<`
            let mut generics = Vec::new();
            loop {
                generics.push(self.parse_type()?);
                if !self.eat(Tag::Comma) {
                    break;
                }
            }
            self.expect_generic_close()?;
            let mut args = vec![left];
            args.extend(self.parse_call_args()?);
            return Ok(Node::CallExpr(CallExpr {
                func_name: field_name,
                associated_target: None,
                generics,
                args,
                callee: None,
            }));
        }
        Ok(Node::FieldExpr(FieldExpr {
            expr: Box::new(left),
            field_name,
        }))
    }

    /// Argument list when the `(` was already consumed as the infix
    /// operator (the plain-call branch above).
    fn parse_call_args_after_open(&mut self) -> Result<Vec<Node>, ParseError> {
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
}

/// Binary operator tokens and their `BinaryOp` (`<` is handled by its own
/// branch because of the generic tails, but is listed for completeness).
fn binary_op_for(tag: Tag) -> Option<BinaryOp> {
    Some(match tag {
        Tag::Plus => BinaryOp::Add,
        Tag::Minus => BinaryOp::Sub,
        Tag::Asterisk => BinaryOp::Mul,
        Tag::Slash => BinaryOp::Div,
        Tag::Percent => BinaryOp::Mod,
        Tag::Ampersand => BinaryOp::BitAnd,
        Tag::Pipe => BinaryOp::BitOr,
        Tag::Caret => BinaryOp::BitXor,
        Tag::LessLess => BinaryOp::Shl,
        Tag::GreaterGreater => BinaryOp::Shr,
        Tag::EqualEqual => BinaryOp::Eq,
        Tag::BangEqual => BinaryOp::Ne,
        Tag::LessThan => BinaryOp::Lt,
        Tag::LessEqual => BinaryOp::Le,
        Tag::GreaterThan => BinaryOp::Gt,
        Tag::GreaterEqual => BinaryOp::Ge,
        Tag::Spaceship => BinaryOp::Spaceship,
        Tag::AmpAmp => BinaryOp::LogicalAnd,
        Tag::PipePipe => BinaryOp::LogicalOr,
        _ => return None,
    })
}

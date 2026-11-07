//! The Pratt expression loop, mirroring `parseExpr` and
//! `getInfixPrecedence` in `src/parser.zig`, including the loop-level
//! interceptions in their original order: ternary `?:` attempt, `??`
//! desugar, postfix `!` desugar, `?.` optional-chain desugar, and the
//! generic-`<` precedence override.

use sla_ast::exprs::{CallExpr, EnumPattern, FieldExpr, IfExpr, IfLetCond};
use sla_ast::node::Node;
use sla_lexer::Tag;

use crate::error::ParseError;
use crate::parser::Parser;

/// Zig `getInfixPrecedence`. Note the deliberate quirks: `|` binds as
/// loosely as `||`, `^` as loosely as `&&`, and `as` sits above `*`.
pub fn infix_precedence(tag: Tag) -> u8 {
    match tag {
        Tag::PipePipe | Tag::Pipe | Tag::QuestionQuestion | Tag::QuestionMark => 1,
        Tag::AmpAmp | Tag::Caret => 2,
        Tag::Ampersand => 3,
        Tag::EqualEqual | Tag::BangEqual | Tag::Spaceship => 4,
        Tag::LessThan | Tag::GreaterThan | Tag::LessEqual | Tag::GreaterEqual => 5,
        Tag::LessLess | Tag::GreaterGreater => 6,
        Tag::Plus | Tag::Minus => 7,
        Tag::Asterisk | Tag::Slash | Tag::Percent => 8,
        Tag::KeywordAs => 9,
        Tag::Dot | Tag::Bang | Tag::LParen | Tag::LBracket => 10,
        _ => 0,
    }
}

/// Zig `tokenStartsPrefixExpr`.
pub fn token_starts_prefix_expr(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::IntLiteral
            | Tag::FloatLiteral
            | Tag::StringLiteral
            | Tag::TemplateStart
            | Tag::Identifier
            | Tag::Ampersand
            | Tag::Caret
            | Tag::Asterisk
            | Tag::Minus
            | Tag::Bang
            | Tag::KeywordIf
            | Tag::KeywordSwitch
            | Tag::KeywordMatch
            | Tag::KeywordUnsafe
            | Tag::KeywordAwait
            | Tag::Pipe
            | Tag::PipePipe
            | Tag::LParen
            | Tag::LBracket
    )
}

impl<'a> Parser<'a> {
    /// Zig `parseExpr`.
    pub fn parse_expr(&mut self, precedence: u8) -> Result<Node, ParseError> {
        let mut left = self.parse_prefix_expr()?;
        loop {
            // Ternary attempt (only at the lowest level): speculative —
            // any failure restores the cursor and falls through, exactly
            // as in Zig (which swallows the attempt's error).
            if self.peek() == Tag::QuestionMark && precedence < 1 {
                let saved = self.snapshot();
                self.advance();
                if token_starts_prefix_expr(self.peek()) {
                    match self.parse_ternary_tail() {
                        Ok((then_node, else_node)) => {
                            left = Node::IfExpr(IfExpr {
                                cond: Box::new(left),
                                let_chain: None,
                                then_block: vec![Node::ExprStmt(Box::new(then_node))],
                                else_block: Some(vec![Node::ExprStmt(Box::new(else_node))]),
                            });
                            continue;
                        }
                        Err(_) => self.restore(saved),
                    }
                } else {
                    self.restore(saved);
                }
            }

            // `a ?? b` -> `unwrap_or(a, b)` (lowest level only).
            if self.peek() == Tag::QuestionQuestion && precedence < 1 {
                self.advance();
                let rhs = self.parse_expr(1)?;
                left = Parser::make_call("unwrap_or", vec![left, rhs]);
                continue;
            }

            // Postfix `a!` -> `unwrap(a)` (any level). This interception
            // runs before the infix dispatch, which makes the `.bang`
            // branch of `parse_infix_expr` unreachable from this loop —
            // see divergences D-P (c).
            if self.peek() == Tag::Bang {
                self.advance();
                left = Parser::make_call("unwrap", vec![left]);
                continue;
            }

            // `a?.field` / `a?.method(args)` optional-chain desugar:
            // `if let Some(__qcN) = a { wrapped(__qcN) } else { None }`.
            if self.peek() == Tag::QuestionDot {
                self.advance();
                let field_tok = self.tok();
                if field_tok.tag != Tag::Identifier && field_tok.tag != Tag::IntLiteral {
                    return Err(self.err_here(crate::error::ParseErrorKind::UnexpectedToken));
                }
                let field_name = self.text(field_tok).to_string();
                self.advance();
                let tmp = self.synth_temp("__qc");
                let pattern = EnumPattern {
                    enum_name: "Option".to_string(),
                    variant_name: "Some".to_string(),
                    bindings: vec![tmp.clone()],
                };
                let cond = IfLetCond {
                    pattern,
                    value: Box::new(left.clone()),
                };
                let tmp_node = Parser::make_identifier(&tmp);
                let wrapped = if self.peek() == Tag::LParen {
                    let mut args = vec![tmp_node];
                    args.extend(self.parse_call_args()?);
                    Node::CallExpr(CallExpr {
                        func_name: field_name,
                        associated_target: None,
                        generics: Vec::new(),
                        args,
                        callee: None,
                    })
                } else {
                    Node::FieldExpr(FieldExpr {
                        expr: Box::new(tmp_node),
                        field_name,
                    })
                };
                left = Node::IfExpr(IfExpr {
                    cond: Box::new(left),
                    let_chain: Some(vec![cond]),
                    then_block: vec![Node::ExprStmt(Box::new(wrapped))],
                    else_block: Some(vec![Node::ExprStmt(Box::new(Parser::make_identifier("None")))]),
                });
                continue;
            }

            // A `<` after an identifier may open a generic struct literal,
            // generic call, or generic function reference instead of a
            // comparison: probe, and if a tail matches, force the call
            // precedence so `parse_infix_expr` takes the tail branch.
            let mut op_prec = infix_precedence(self.peek());
            if self.peek() == Tag::LessThan {
                if let Node::Identifier(_) = left {
                    let saved = self.snapshot();
                    self.advance();
                    let generic = self.looks_like_struct_literal_tail()
                        || self.looks_like_generic_call_tail()
                        || self.looks_like_generic_function_ref_tail();
                    self.restore(saved);
                    if generic {
                        op_prec = 10;
                    }
                }
            }
            if op_prec <= precedence {
                break;
            }
            left = self.parse_infix_expr(left, op_prec)?;
        }
        Ok(left)
    }

    /// `<then-expr> : <else-expr>` after a consumed `?` (ternary form).
    pub(crate) fn parse_ternary_tail(&mut self) -> Result<(Node, Node), ParseError> {
        let then_node = self.parse_expr(0)?;
        self.expect(Tag::Colon)?;
        let else_node = self.parse_expr(0)?;
        Ok((then_node, else_node))
    }
}

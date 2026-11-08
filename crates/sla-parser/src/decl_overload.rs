//! R-3 overload slice: `@overload Type { fn <op>(...) ... }`
//! block parsing, split out of decl.rs for the 700-line file
//! cap. The bare `overload` (no `@`) form is rejected by the
//! caller's `at_decl_starts_decl` gate and never reaches here;
//! its negative corpus twin is judged by that rejection.

use sla_ast::decls::OverloadDecl;
use sla_ast::exprs::BinaryOp;
use sla_ast::node::Node;
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;

impl<'a> Parser<'a> {
    /// `@overload Type { fn +(...) ... }`: the target type, then
    /// a brace block of operator methods. Each method's operator
    /// token becomes `FuncDecl.operator` (the checker keys the
    /// registration symbol on it).
    pub fn parse_overload_decl(&mut self) -> Result<Node, ParseError> {
        self.advance(); // consume `overload`
        let target_ty = self.parse_type()?;
        self.expect(Tag::LBrace)?;
        let prev_target = self
            .current_impl_target()
            .map(|t| Box::new(t.clone()));
        self.set_current_impl_target(Some(Box::new(target_ty.clone())));
        let mut methods = Vec::new();
        let mut result: Result<(), ParseError> = Ok(());
        while self.peek() != Tag::RBrace && self.peek() != Tag::Eof {
            match self.parse_overload_method(&target_ty) {
                Ok(m) => {
                    methods.push(m);
                    self.eat(Tag::Semicolon);
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        self.set_current_impl_target(prev_target);
        result?;
        self.expect(Tag::RBrace)?;
        Ok(Node::OverloadDecl(OverloadDecl {
            target_ty: Box::new(target_ty),
            methods,
        }))
    }

    /// One operator method: `fn <op>(params) (-> ret)? body`.
    /// Parameters reuse the impl-method parameter shapes
    /// (a first parameter named `self` takes the target type).
    fn parse_overload_method(
        &mut self,
        target_ty: &sla_ast::types::Type,
    ) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordFn)?;
        let (op, op_name) = match self.peek() {
            Tag::Plus => (BinaryOp::Add, "+".to_string()),
            Tag::Minus => (BinaryOp::Sub, "-".to_string()),
            Tag::Asterisk => (BinaryOp::Mul, "*".to_string()),
            Tag::Slash => (BinaryOp::Div, "/".to_string()),
            Tag::Percent => (BinaryOp::Mod, "%".to_string()),
            Tag::EqualEqual => (BinaryOp::Eq, "==".to_string()),
            Tag::BangEqual => (BinaryOp::Ne, "!=".to_string()),
            Tag::LessThan => (BinaryOp::Lt, "<".to_string()),
            Tag::LessEqual => (BinaryOp::Le, "<=".to_string()),
            Tag::GreaterThan => (BinaryOp::Gt, ">".to_string()),
            Tag::GreaterEqual => (BinaryOp::Ge, ">=".to_string()),
            Tag::Ampersand => (BinaryOp::BitAnd, "&".to_string()),
            Tag::Pipe => (BinaryOp::BitOr, "|".to_string()),
            Tag::Caret => (BinaryOp::BitXor, "^".to_string()),
            _ => {
                return Err(self.err_here(
                    ParseErrorKind::ExpectedDeclaration,
                ))
            }
        };
        self.advance();
        self.expect(Tag::LParen)?;
        let mut params: Vec<sla_ast::decls::Param> = Vec::new();
        if self.peek() != Tag::RParen && self.peek() != Tag::Eof {
            let first_is_borrow = self.eat(Tag::Ampersand);
            let first_is_move = !first_is_borrow && self.eat(Tag::Caret);
            // An explicitly typed `self: Type` first parameter is
            // legal in overload blocks (unlike impl methods, whose
            // bare `self` is untyped): strip the annotation when
            // the name is `self` AND a colon+type follows.
            let first_name = self.expect_identifier()?;
            if first_name == "self" && self.peek() == Tag::Colon {
                self.advance();
                let _annotated = self.parse_type()?;
                params.push(sla_ast::decls::Param {
                    name: "self".to_string(),
                    ty: Box::new(target_ty.clone()),
                    is_borrow: first_is_borrow,
                    is_move: first_is_move,
                    fallible_sugar: false,
                    is_var: false,
                });
                if self.eat(Tag::Comma) {
                    while self.peek() != Tag::RParen
                        && self.peek() != Tag::Eof
                    {
                        params.push(self.parse_method_param()?);
                        if !self.eat(Tag::Comma) {
                            break;
                        }
                    }
                }
                self.expect(Tag::RParen)?;
                let ret_ty = if self.eat(Tag::Arrow) {
                    self.parse_type()?
                } else {
                    sla_ast::types::Type::Primitive(
                        sla_ast::types::Primitive::VoidType,
                    )
                };
                let body = {
                    let span = self.block_span()?;
                    self.record_function_body_span(span);
                    if self.should_parse_fn_body(&op_name) {
                        self.parse_block()?
                    } else {
                        self.skip_block()?;
                        Vec::new()
                    }
                };
                return Ok(Node::FuncDecl(sla_ast::decls::FuncDecl {
                    name: op_name,
                    is_pub: false,
                    is_extern: false,
                    abi: None,
                    no_mangle: false,
                    is_decl_only: false,
                    generics: Vec::new(),
                    params,
                    ret_ty: Box::new(ret_ty),
                    body,
                    is_inline: false,
                    is_async: false,
                    operator: Some(op),
                    is_arena: false,
                    ret_fallible_sugar: false,
                    ret_borrow_origin: None,
                    is_variadic: false,
                }));
            }
            if first_name == "self" {
                params.push(sla_ast::decls::Param {
                    name: "self".to_string(),
                    ty: Box::new(target_ty.clone()),
                    is_borrow: first_is_borrow,
                    is_move: first_is_move,
                    fallible_sugar: false,
                    is_var: false,
                });
                if self.eat(Tag::Comma) {
                    while self.peek() != Tag::RParen
                        && self.peek() != Tag::Eof
                    {
                        params.push(self.parse_method_param()?);
                        if !self.eat(Tag::Comma) {
                            break;
                        }
                    }
                }
            } else {
                self.expect(Tag::Colon)?;
                let first_ty = self.parse_type()?;
                params.push(sla_ast::decls::Param {
                    name: first_name,
                    ty: Box::new(first_ty),
                    is_borrow: first_is_borrow,
                    is_move: first_is_move,
                    fallible_sugar: false,
                    is_var: false,
                });
                while self.eat(Tag::Comma) {
                    if self.peek() == Tag::RParen
                        || self.peek() == Tag::Eof
                    {
                        break;
                    }
                    params.push(self.parse_method_param()?);
                }
            }
        }
        self.expect(Tag::RParen)?;
        let ret_ty = if self.eat(Tag::Arrow) {
            self.parse_type()?
        } else {
            sla_ast::types::Type::Primitive(
                sla_ast::types::Primitive::VoidType,
            )
        };
        let body = {
            let span = self.block_span()?;
            self.record_function_body_span(span);
            if self.should_parse_fn_body(&op_name) {
                self.parse_block()?
            } else {
                self.skip_block()?;
                Vec::new()
            }
        };
        Ok(Node::FuncDecl(sla_ast::decls::FuncDecl {
            name: op_name,
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: Vec::new(),
            params,
            ret_ty: Box::new(ret_ty),
            body,
            is_inline: false,
            is_async: false,
            operator: Some(op),
            is_arena: false,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
            is_variadic: false,
        }))
    }
}

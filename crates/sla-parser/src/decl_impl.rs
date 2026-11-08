//! R-3 impl slice: `impl` declaration parsing, split out of
//! decl.rs for the 700-line file cap. Mirrors Zig `parseImplDecl`
//! / `parseMethodDecl` (parser.zig:976 / :1042).

use sla_ast::decls::{FuncDecl, ImplDecl};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;

impl<'a> Parser<'a> {
    /// Zig `parseImplDecl` (parser.zig:976): `impl Type { ... }` or
    /// `impl Trait for Type { ... }`. Methods parse under
    /// `current_impl_target = target` (restored after), with
    /// inline/async modifiers rejected as in Zig.
    pub fn parse_impl_decl(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordImpl)?;
        let first_ty = self.parse_type()?;
        let mut trait_name: Option<String> = None;
        let mut target_ty = first_ty;
        if self.eat(Tag::KeywordFor) {
            match &target_ty {
                Type::UserDefined(ud) => trait_name = Some(ud.name.clone()),
                _ => {
                    return Err(self.err_here(
                        ParseErrorKind::UnexpectedTypeToken,
                    ))
                }
            }
            target_ty = self.parse_type()?;
        }
        self.expect(Tag::LBrace)?;
        let prev_target = self
            .current_impl_target()
            .map(|t| Box::new(t.clone()));
        self.set_current_impl_target(Some(Box::new(target_ty.clone())));
        let mut methods = Vec::new();
        let mut result: Result<(), ParseError> = Ok(());
        while self.peek() != Tag::RBrace && self.peek() != Tag::Eof {
            let is_inline = self.eat(Tag::KeywordInline);
            let is_async = self.eat(Tag::KeywordAsync);
            if is_inline || is_async {
                result = Err(self.err_here(ParseErrorKind::ExpectedDeclaration));
                break;
            }
            match self.parse_method_decl(&target_ty) {
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
        Ok(Node::ImplDecl(ImplDecl {
            trait_name,
            target_ty: Box::new(target_ty),
            methods,
        }))
    }

    /// Zig `parseMethodDecl` (parser.zig:1042): a method inside an
    /// impl block. The first parameter named `self` takes the impl
    /// target type (no colon/type follows); a first parameter with
    /// any other name is typed normally. Zig keys the parse-vs-skip
    /// body decision on the impl target (shouldParseMethodBody);
    /// this port has no such registry, so method bodies are always
    /// parsed — a documented divergence risk if a corpus method body
    /// uses a surface the fn-body path rejects (the census is the
    /// arbiter).
    pub fn parse_method_decl(
        &mut self,
        target_ty: &Type,
    ) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordFn)?;
        let name = self.expect_identifier()?;
        let generics = self.parse_generic_params()?;
        self.expect(Tag::LParen)?;
        let mut params: Vec<sla_ast::decls::Param> = Vec::new();
        if self.peek() != Tag::RParen && self.peek() != Tag::Eof {
            let first_is_borrow = self.eat(Tag::Ampersand);
            let first_is_move =
                !first_is_borrow && self.eat(Tag::Caret);
            let first_name = self.expect_identifier()?;
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
                    if self.peek() == Tag::RParen || self.peek() == Tag::Eof
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
            Type::Primitive(Primitive::VoidType)
        };
        let body = {
            let span = self.block_span()?;
            self.record_function_body_span(span);
            if self.should_parse_fn_body(&name) {
                self.parse_block()?
            } else {
                self.skip_block()?;
                Vec::new()
            }
        };
        Ok(Node::FuncDecl(FuncDecl {
            name,
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics,
            params,
            ret_ty: Box::new(ret_ty),
            body,
            is_inline: false,
            is_async: false,
            operator: None,
            is_arena: false,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
            is_variadic: false,
        }))
    }

    /// One non-first method parameter (Zig parseMethodDecl's inner
    /// loops share this exact shape; also used by the trait slice).
    pub fn parse_method_param(
        &mut self,
    ) -> Result<sla_ast::decls::Param, ParseError> {
        let is_borrow = self.eat(Tag::Ampersand);
        let is_move = !is_borrow && self.eat(Tag::Caret);
        let name = self.expect_identifier()?;
        self.expect(Tag::Colon)?;
        let ty = self.parse_type()?;
        Ok(sla_ast::decls::Param {
            name,
            ty: Box::new(ty),
            is_borrow,
            is_move,
            fallible_sugar: false,
            is_var: false,
        })
    }
}

//! R-3 trait slice: `trait` declaration parsing, split out of
//! decl.rs for the 700-line file cap. Mirrors Zig `parseTraitDecl`
//! (parser.zig:887).

use sla_ast::decls::{TraitDecl, TraitMethod};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};
use sla_lexer::Tag;

use crate::error::ParseError;
use crate::parser::Parser;

impl<'a> Parser<'a> {
    /// Zig `parseTraitDecl` (parser.zig:887): `trait Name (: Super
    /// (+ Super)*)? { fn sig; ... }`. Signatures only — each method
    /// ends with `;`. Inside the block, `current_impl_target` is the
    /// `Self` user-defined type (restored after), so `Self` resolves
    /// as a known type name in signatures.
    pub fn parse_trait_decl(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordTrait)?;
        let name = self.expect_identifier()?;
        let mut supertraits = Vec::new();
        if self.eat(Tag::Colon) {
            loop {
                supertraits.push(self.expect_identifier()?);
                if !self.eat(Tag::Plus) {
                    break;
                }
            }
        }
        self.expect(Tag::LBrace)?;
        let self_ty = Type::UserDefined(sla_ast::types::UserDefined {
            name: "Self".to_string(),
            generics: Vec::new(),
        });
        let prev_target = self
            .current_impl_target()
            .map(|t| Box::new(t.clone()));
        self.set_current_impl_target(Some(Box::new(self_ty)));
        let mut methods = Vec::new();
        let mut result: Result<(), ParseError> = Ok(());
        while self.peek() != Tag::RBrace && self.peek() != Tag::Eof {
            match self.parse_trait_method() {
                Ok(m) => methods.push(m),
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        self.set_current_impl_target(prev_target);
        result?;
        self.expect(Tag::RBrace)?;
        Ok(Node::TraitDecl(TraitDecl {
            name,
            supertraits,
            methods,
        }))
    }

    /// One trait method signature (Zig parseTraitDecl's inner loop):
    /// same parameter grammar as impl methods, `self` taking the
    /// `Self` type, terminated by `;`.
    fn parse_trait_method(&mut self) -> Result<TraitMethod, ParseError> {
        self.expect(Tag::KeywordFn)?;
        let name = self.expect_identifier()?;
        let _ = self.parse_generic_params()?;
        self.expect(Tag::LParen)?;
        let self_ty = Type::UserDefined(sla_ast::types::UserDefined {
            name: "Self".to_string(),
            generics: Vec::new(),
        });
        let mut params: Vec<sla_ast::decls::Param> = Vec::new();
        if self.peek() != Tag::RParen && self.peek() != Tag::Eof {
            let first_is_borrow = self.eat(Tag::Ampersand);
            let first_is_move =
                !first_is_borrow && self.eat(Tag::Caret);
            let first_name = self.expect_identifier()?;
            if first_name == "self" {
                params.push(sla_ast::decls::Param {
                    name: "self".to_string(),
                    ty: Box::new(self_ty),
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
        self.expect(Tag::Semicolon)?;
        Ok(TraitMethod {
            name,
            params,
            ret_ty: Box::new(ret_ty),
        })
    }
}

//! Type declarations, mirroring `parseStructDecl` / `parseUnionDecl` /
//! `parseEnumDecl` / `parseTypeAliasDecl` in `src/parser.zig`.

use sla_ast::decls::{EnumDecl, EnumVariant, Field, StructDecl, TypeAliasComponent, TypeAliasDecl};
use sla_ast::node::Node;
use sla_ast::types::Type;
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::numlit::strip_numeric_separators;
use crate::parser::Parser;

impl<'a> Parser<'a> {
    /// Zig `parseStructDecl` / `parseUnionDecl` (keyword still current).
    pub fn parse_aggregate_decl(&mut self, is_union: bool, derives: Vec<String>) -> Result<Node, ParseError> {
        self.advance(); // struct / union keyword
        self.parse_aggregate_decl_rest(is_union, derives)
    }

    /// Aggregate declaration after the keyword (also used after `@derive`).
    pub fn parse_aggregate_decl_rest(&mut self, is_union: bool, derives: Vec<String>) -> Result<Node, ParseError> {
        let name = self.expect_identifier()?;
        let generics = self.parse_generic_params()?;
        if self.eat(Tag::Semicolon) {
            // Opaque declaration: `struct Foo;`
            return Ok(Node::StructDecl(StructDecl {
                name,
                derives,
                generics,
                fields: Vec::new(),
                is_union,
                is_opaque: true,
            }));
        }
        let fields = if self.eat(Tag::LParen) {
            // Tuple fields, auto-named "0", "1", ...
            let mut fields = Vec::new();
            loop {
                if self.eat(Tag::RParen) {
                    break;
                }
                let ty = self.parse_type()?;
                fields.push(Field { name: fields.len().to_string(), ty: Box::new(ty) });
                if !self.eat(Tag::Comma) {
                    self.expect(Tag::RParen)?;
                    break;
                }
            }
            fields
        } else {
            self.expect(Tag::LBrace)?;
            self.parse_braced_fields()?
        };
        Ok(Node::StructDecl(StructDecl {
            name,
            derives,
            generics,
            fields,
            is_union,
            is_opaque: false,
        }))
    }

    /// `{ name: type, ... }` field list; the `{` is already consumed and
    /// the closing `}` is consumed here. Commas are optional per field,
    /// as in Zig.
    pub fn parse_braced_fields(&mut self) -> Result<Vec<Field>, ParseError> {
        let mut fields = Vec::new();
        loop {
            if self.eat(Tag::RBrace) {
                break;
            }
            let field_name = self.expect_identifier()?;
            self.expect(Tag::Colon)?;
            let ty = self.parse_type()?;
            fields.push(Field { name: field_name, ty: Box::new(ty) });
            self.eat(Tag::Comma);
        }
        Ok(fields)
    }

    /// Zig `parseEnumDecl`, including its parse-time discriminant checks:
    /// an explicit discriminant on a payload variant is rejected, and the
    /// effective discriminants (auto-increment from 0) must be unique.
    pub fn parse_enum_decl(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordEnum)?;
        let name = self.expect_identifier()?;
        let generics = self.parse_generic_params()?;
        self.expect(Tag::LBrace)?;
        let mut variants: Vec<EnumVariant> = Vec::new();
        loop {
            if self.eat(Tag::RBrace) {
                break;
            }
            let variant_name = self.expect_identifier()?;
            let discriminant = if self.eat(Tag::Equal) {
                Some(self.parse_enum_discriminant()?)
            } else {
                None
            };
            let fields = if self.eat(Tag::LBrace) {
                self.parse_braced_fields()?
            } else if self.eat(Tag::LParen) {
                let mut fields = Vec::new();
                loop {
                    if self.eat(Tag::RParen) {
                        break;
                    }
                    let ty = self.parse_type()?;
                    fields.push(Field { name: fields.len().to_string(), ty: Box::new(ty) });
                    if !self.eat(Tag::Comma) {
                        self.expect(Tag::RParen)?;
                        break;
                    }
                }
                fields
            } else {
                Vec::new()
            };
            variants.push(EnumVariant { name: variant_name, fields, discriminant });
            self.eat(Tag::Comma);
        }
        // Parse-time discriminant validation (see the Zig original).
        let mut seen: Vec<i64> = Vec::new();
        let mut current: i64 = 0;
        for variant in &variants {
            if variant.discriminant.is_some() && !variant.fields.is_empty() {
                return Err(self.err_here(ParseErrorKind::UnexpectedToken));
            }
            let effective = variant.discriminant.unwrap_or(current);
            if seen.contains(&effective) {
                return Err(self.err_here(ParseErrorKind::UnexpectedToken));
            }
            seen.push(effective);
            current = effective + 1;
        }
        Ok(Node::EnumDecl(EnumDecl { name, generics, variants }))
    }

    /// `[-]123` after `=` in an enum variant; a trailing type suffix on
    /// the literal is ignored, as in Zig.
    fn parse_enum_discriminant(&mut self) -> Result<i64, ParseError> {
        let negative = self.eat(Tag::Minus);
        if self.peek() != Tag::IntLiteral {
            return Err(self.err_here(ParseErrorKind::InvalidCharacter));
        }
        let raw = self.cur_text().to_string();
        self.advance();
        let clean = strip_numeric_separators(&raw);
        let (base, digits_start) = if clean.len() >= 2
            && clean.as_bytes()[0] == b'0'
            && (clean.as_bytes()[1] == b'x' || clean.as_bytes()[1] == b'X')
        {
            (16u32, 2usize)
        } else {
            (10u32, 0usize)
        };
        let bytes = clean.as_bytes();
        let mut end = digits_start;
        if base == 16 {
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
        } else {
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
        }
        if end == digits_start {
            return Err(self.err_here(ParseErrorKind::InvalidCharacter));
        }
        let value = i64::from_str_radix(&clean[digits_start..end], base)
            .map_err(|_| self.err_here(ParseErrorKind::InvalidCharacter))?;
        Ok(if negative { -value } else { value })
    }

    /// Zig `parseTypeAliasDecl`: `type Name = comp (& comp)* ;` where a
    /// component is a type or an inline `{ fields }` struct.
    pub fn parse_type_alias_decl(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordType)?;
        let name = self.expect_identifier()?;
        self.expect(Tag::Equal)?;
        let mut components: Vec<TypeAliasComponent> = Vec::new();
        loop {
            if self.eat(Tag::LBrace) {
                let fields = self.parse_braced_fields()?;
                components.push(TypeAliasComponent::InlineStruct(fields));
            } else {
                let ty: Type = self.parse_type()?;
                components.push(TypeAliasComponent::Ty(Box::new(ty)));
            }
            if !self.eat(Tag::Ampersand) {
                break;
            }
        }
        self.expect(Tag::Semicolon)?;
        Ok(Node::TypeAliasDecl(TypeAliasDecl { name, components }))
    }
}

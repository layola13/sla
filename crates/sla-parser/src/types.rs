//! Type parsing, mirroring `parseType` / `parseTypeAtom` /
//! `parseFunctionType` in `src/parser.zig`. The Zig grammar has no postfix
//! type operators beyond what the atom itself handles, so `parse_type`
//! is exactly `parse_type_atom`.

use sla_ast::types::{ArrayType, FnPtrType, Primitive, TupleType, Type};
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::numlit::strip_numeric_separators;
use crate::parser::Parser;

impl<'a> Parser<'a> {
    pub fn parse_type(&mut self) -> Result<Type, ParseError> {
        self.parse_type_atom()
    }

    fn parse_type_atom(&mut self) -> Result<Type, ParseError> {
        let tag = self.peek();
        let text = self.cur_text().to_string();
        self.advance();
        match tag {
            Tag::Identifier => self.finish_named_type(&text),
            Tag::KeywordDyn => {
                // `dyn Trait` lowers to a synthetic user-defined name.
                let name = self.expect_identifier()?;
                Ok(Parser::make_user_defined_type(&format!("__dyn_{}", name), Vec::new()))
            }
            Tag::KeywordExtern => {
                let abi = self.parse_optional_extern_abi()?;
                self.expect(Tag::KeywordFn)?;
                self.parse_function_type(abi)
            }
            Tag::KeywordFn => self.parse_function_type(None),
            Tag::Ampersand => Ok(Type::Borrow(Box::new(self.parse_type()?))),
            Tag::Asterisk => Ok(Type::Pointer(Box::new(self.parse_type()?))),
            Tag::LBracket => {
                let elem = self.parse_type()?;
                if self.eat(Tag::Semicolon) {
                    // `[T; N]`: N is a bare integer literal (separators
                    // stripped; the whole cleaned lexeme must parse).
                    if self.peek() != Tag::IntLiteral {
                        return Err(self.err_here(ParseErrorKind::InvalidCharacter));
                    }
                    let raw = self.cur_text().to_string();
                    let clean = strip_numeric_separators(&raw);
                    let len: usize = clean
                        .parse::<usize>()
                        .map_err(|_| self.err_here(ParseErrorKind::InvalidCharacter))?;
                    self.advance();
                    self.expect(Tag::RBracket)?;
                    Ok(Type::Array(ArrayType { elem: Box::new(elem), len }))
                } else {
                    // `[T]` is the slice type, spelled as a generic.
                    self.expect(Tag::RBracket)?;
                    Ok(Parser::make_user_defined_type("Slice", vec![elem]))
                }
            }
            Tag::LParen => {
                let first = self.parse_type()?;
                if self.eat(Tag::Comma) {
                    let mut elems = vec![first];
                    loop {
                        if self.eat(Tag::RParen) {
                            break;
                        }
                        elems.push(self.parse_type()?);
                        if !self.eat(Tag::Comma) {
                            self.expect(Tag::RParen)?;
                            break;
                        }
                    }
                    Ok(Type::Tuple(TupleType { elems }))
                } else {
                    self.expect(Tag::RParen)?;
                    Ok(first)
                }
            }
            _ => Err(self.err_here(ParseErrorKind::UnexpectedTypeToken)),
        }
    }

    /// After a leading identifier in type position: primitive mapping,
    /// `Self`, and the optional `<...>` generic argument list.
    fn finish_named_type(&mut self, name: &str) -> Result<Type, ParseError> {
        if name == "Self" {
            return match self.current_impl_target() {
                Some(ty) => Ok(ty.clone()),
                None => Err(self.err_here(ParseErrorKind::UnexpectedTypeToken)),
            };
        }
        if let Some(prim) = primitive_by_name(name) {
            return Ok(Type::Primitive(prim));
        }
        let mut generics: Vec<Type> = Vec::new();
        if self.eat(Tag::LessThan) {
            loop {
                generics.push(self.parse_type()?);
                if !self.eat(Tag::Comma) {
                    break;
                }
            }
            self.expect_generic_close()?;
        }
        // `future<T>` is a dedicated Type variant, not a generic.
        if name == "future" {
            if generics.len() != 1 {
                return Err(self.err_here(ParseErrorKind::UnexpectedTypeToken));
            }
            let inner = generics.into_iter().next().unwrap();
            return Ok(Type::Future(Box::new(inner)));
        }
        Ok(Parser::make_user_defined_type(name, generics))
    }

    /// `fn(T, ...) -> R` in type position (Zig `parseFunctionType`).
    pub fn parse_function_type(&mut self, abi: Option<String>) -> Result<Type, ParseError> {
        self.expect(Tag::LParen)?;
        let mut params: Vec<Type> = Vec::new();
        if !self.eat(Tag::RParen) {
            loop {
                params.push(self.parse_type()?);
                if !self.eat(Tag::Comma) {
                    self.expect(Tag::RParen)?;
                    break;
                }
                if self.eat(Tag::RParen) {
                    break;
                }
            }
        }
        let ret = if self.eat(Tag::Arrow) {
            self.parse_type()?
        } else {
            Type::Primitive(Primitive::VoidType)
        };
        Ok(Type::FnPtr(FnPtrType { abi, params, ret: Box::new(ret) }))
    }

    /// Optional `extern "abi"`: the string literal after `extern`, if any.
    /// Used by declaration prefixes and by `extern fn` types.
    pub fn parse_optional_extern_abi(&mut self) -> Result<Option<String>, ParseError> {
        if self.peek() == Tag::StringLiteral {
            let raw = self.cur_text().to_string();
            self.advance();
            Ok(Some(string_literal_inner(&raw).to_string()))
        } else {
            Ok(None)
        }
    }
}

/// Zig stores a string literal's value as the raw bytes between the
/// quotes (escapes are NOT processed at parse time).
pub fn string_literal_inner(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        &raw[1..raw.len() - 1]
    } else {
        raw
    }
}

/// Primitive spellings in type position, including the legacy aliases
/// (`int` -> i64, `float` -> f64, `bool`, `void`, `ptr`).
pub fn primitive_by_name(name: &str) -> Option<Primitive> {
    Some(match name {
        "i8" => Primitive::I8,
        "i16" => Primitive::I16,
        "i32" => Primitive::I32,
        "i64" => Primitive::I64,
        "isize" => Primitive::Isize,
        "u8" => Primitive::U8,
        "u16" => Primitive::U16,
        "u32" => Primitive::U32,
        "u64" => Primitive::U64,
        "usize" => Primitive::Usize,
        "f32" => Primitive::F32,
        "f64" => Primitive::F64,
        "int" => Primitive::I64,
        "float" => Primitive::F64,
        "bool" => Primitive::Boolean,
        "void" => Primitive::VoidType,
        "ptr" => Primitive::RawPtr,
        _ => return None,
    })
}

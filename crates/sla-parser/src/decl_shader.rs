//! R-3 (J-1): the shader sublanguage parser (`@sublanguage`
//! registration and `shader Name { ... }` declarations).
//! Stage bodies are captured as raw source spans; the typed
//! validation lives in sla-checker check_shader.

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;
use sla_ast::node::Node;
use sla_lexer::Tag;

impl<'a> Parser<'a> {
// -- R-3 (J-1) sublanguage / shader --------------------------------

/// Parser state: sublanguage kinds registered so far in
/// this file (`@sublanguage shader;`).
pub fn sublanguage_registered(&self, name: &str) -> bool {
    self.registered_sublanguages.iter().any(|n| n == name)
}

pub fn parse_sublanguage_decl(&mut self) -> Result<Node, ParseError> {
    let name = self.expect_identifier()?;
    self.expect(Tag::Semicolon)?;
    self.registered_sublanguages.push(name.clone());
    Ok(Node::SublanguageDecl(sla_ast::SublanguageDecl { name }))
}

/// `shader Name { ... }` — only reachable when the shader
/// sublanguage is registered (parse_decl routes here).
/// Members are parsed structurally; stage bodies are kept
/// as raw source spans for the shader checker.
pub fn parse_shader_decl(&mut self) -> Result<Node, ParseError> {
    // current token: `shader`
    self.advance();
    let name = self.expect_identifier()?;
    self.expect(Tag::LBrace)?;
    let mut shader = sla_ast::ShaderDecl {
        name,
        priority: None,
        globals: Vec::new(),
        inputs: Vec::new(),
        params: Vec::new(),
        consts: Vec::new(),
        vars: Vec::new(),
        stages: Vec::new(),
    };
    loop {
        match self.peek() {
            Tag::RBrace => {
                self.advance();
                break;
            }
            Tag::KeywordVar => {
                self.advance();
                let f = self.parse_shader_field()?;
                shader.vars.push(f);
            }
            Tag::At => {
                self.advance();
                let qual = if self.peek() == Tag::KeywordConst {
                    self.advance();
                    "const".to_string()
                } else {
                    self.expect_identifier()?
                };
                match qual.as_str() {
                    "global" => {
                        // `@global struct Name { f: T; ... }`
                        if self.peek() == Tag::KeywordStruct
                            || self.cur_text() == "struct"
                        {
                            self.advance();
                        }
                        let gname = self.expect_identifier()?;
                        self.expect(Tag::LBrace)?;
                        let mut fields = Vec::new();
                        loop {
                            if self.eat(Tag::RBrace) {
                                break;
                            }
                            let fname = self.expect_identifier()?;
                            self.expect(Tag::Colon)?;
                            let fty = self.expect_identifier()?;
                            self.expect(Tag::Semicolon)?;
                            fields.push(sla_ast::ShaderField {
                                name: fname,
                                ty: fty,
                            });
                        }
                        self.eat(Tag::Semicolon);
                        shader.globals.push(sla_ast::ShaderGlobalStruct {
                            name: gname,
                            fields,
                        });
                    }
                    "input" => {
                        let f = self.parse_shader_field()?;
                        shader.inputs.push(f);
                    }
                    "param" => {
                        let f = self.parse_shader_field()?;
                        shader.params.push(f);
                    }
                    "const" => {
                        let cname = self.expect_identifier()?;
                        self.expect(Tag::Colon)?;
                        let cty = self.expect_identifier()?;
                        self.expect(Tag::Equal)?;
                        let value = self.cur_text().to_string();
                        self.advance();
                        self.expect(Tag::Semicolon)?;
                        shader.consts.push(sla_ast::ShaderConst {
                            name: cname,
                            ty: cty,
                            value,
                        });
                    }
                    _ => {
                        return Err(self.err_here(ParseErrorKind::SyntaxError));
                    }
                }
            }
            Tag::Identifier => {
                let word = self.cur_text().to_string();
                match word.as_str() {
                    "priority" => {
                        self.advance();
                        let text = self.cur_text().to_string();
                        shader.priority =
                            text.parse::<i64>().ok();
                        self.advance();
                        self.expect(Tag::Semicolon)?;
                    }
                    "var" => {
                        self.advance();
                        let f = self.parse_shader_field()?;
                        shader.vars.push(f);
                    }
                    "vertex" | "fragment" | "init" => {
                        self.advance();
                        let raw = self.capture_braced_source()?;
                        shader.stages.push((word, raw));
                    }
                    _ => {
                        return Err(
                            self.err_here(ParseErrorKind::SyntaxError)
                        );
                    }
                }
            }
            _ => {
                return Err(self.err_here(ParseErrorKind::SyntaxError));
            }
        }
    }
    Ok(Node::ShaderDecl(shader))
}

pub fn parse_shader_field(&mut self) -> Result<sla_ast::ShaderField, ParseError> {
    let name = self.expect_identifier()?;
    self.expect(Tag::Colon)?;
    let ty = self.expect_identifier()?;
    self.expect(Tag::Semicolon)?;
    Ok(sla_ast::ShaderField { name, ty })
}

/// The current token must be `{`; consume through the
/// matching `}` and return the raw source BETWEEN the
/// braces.
pub fn capture_braced_source(&mut self) -> Result<String, ParseError> {
    let open = self.tok();
    if open.tag != Tag::LBrace {
        return Err(self.err_here(ParseErrorKind::SyntaxError));
    }
    let start = open.loc.end;
    let mut depth = 0usize;
    let mut end = self.source.len();
    loop {
        let t = self.tok();
        match t.tag {
            Tag::LBrace => depth += 1,
            Tag::RBrace => {
                depth -= 1;
                if depth == 0 {
                    end = t.loc.start;
                    self.advance();
                    break;
                }
            }
            Tag::Eof => {
                return Err(self.err_here(ParseErrorKind::SyntaxError));
            }
            _ => {}
        }
        self.advance();
    }
    Ok(self.source[start..end].to_string())
}
}

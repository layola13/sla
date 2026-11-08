//! Declaration parsing: `parseProgram`, `parseDecl`, `parseAtDecl`,
//! function/macro/import/using/test declarations, extern blocks and
//! modules, mirroring the declaration half of `src/parser.zig`.
//!
//! Slice scope: `@overload` declarations are NOT covered yet and
//! fail loudly (see `docs/divergences/parser.md`); `impl` and
//! `trait` declarations are covered by the R-3 impl/trait slices.

use sla_ast::decls::{
    FuncDecl, ImportDecl, MacroDecl, Program, TestDecl, UsingDecl,
};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};
use sla_lexer::Tag;

use crate::error::{ParseError, ParseErrorKind};
use crate::parser::Parser;
use crate::types::string_literal_inner;

/// Declaration modifiers collected by the prefix scan in `parseDecl`
/// (order is significant: `pub`, `extern`, `inline`, `async`, each at
/// most once, exactly as in Zig).
#[derive(Debug, Clone, Default)]
pub struct DeclMods {
    pub is_pub: bool,
    pub is_extern: bool,
    pub abi: Option<String>,
    pub is_inline: bool,
    pub is_async: bool,
    pub no_mangle: bool,
}

impl<'a> Parser<'a> {
    /// Zig `parseProgram`: declarations until `eof`, each followed by an
    /// optional `;`. Extern blocks flatten into the program; module
    /// declarations append their (name-mangled) members directly.
    pub fn parse_program(&mut self) -> Result<Node, ParseError> {
        let mut decls: Vec<Node> = Vec::new();
        while self.peek() != Tag::Eof {
            match self.peek() {
                Tag::KeywordUsing => {
                    decls.push(self.parse_using_decl()?);
                }
                Tag::KeywordMod => {
                    self.parse_module_decl_into(&mut decls, None)?;
                }
                _ => {
                    let decl = self.parse_decl()?;
                    match decl {
                        Node::Program(program) => {
                            for sub in program.decls {
                                self.register_declared_names(&sub);
                                decls.push(sub);
                            }
                        }
                        other => {
                            self.register_declared_names(&other);
                            decls.push(other);
                        }
                    }
                }
            }
            self.eat(Tag::Semicolon);
        }
        Ok(Node::Program(Program { decls }))
    }

    /// Registration side effects of a finished declaration (Zig records
    /// known type/enum names as declarations complete).
    pub(crate) fn register_declared_names(&mut self, node: &Node) {
        match node {
            Node::StructDecl(decl) => self.record_known_type(&decl.name),
            Node::TypeAliasDecl(decl) => self.record_known_type(&decl.name),
            Node::EnumDecl(decl) => {
                self.record_known_type(&decl.name);
                self.record_known_enum(&decl.name);
            }
            Node::ImplDecl(decl) => {
                if let Type::UserDefined(user) = &*decl.target_ty {
                    self.record_known_type(&user.name);
                }
            }
            _ => {}
        }
    }

    /// Zig `parseDecl`.
    pub fn parse_decl(&mut self) -> Result<Node, ParseError> {
        if self.peek() == Tag::At {
            return self.parse_at_decl();
        }
        // R-3 (J-1): a bare `shader` declaration is only a
        // declaration when the shader sublanguage was
        // registered earlier in this file; otherwise it is not
        // recognised at all (ExpectedDeclaration).
        if self.peek() == Tag::Identifier
            && self.cur_text() == "shader"
            && self.sublanguage_registered("shader")
        {
            return self.parse_shader_decl();
        }
        let mut mods = DeclMods::default();
        mods.is_pub = self.eat(Tag::KeywordPub);
        if self.eat(Tag::KeywordExtern) {
            mods.is_extern = true;
            mods.abi = self.parse_optional_extern_abi()?;
        }
        mods.is_inline = self.eat(Tag::KeywordInline);
        mods.is_async = self.eat(Tag::KeywordAsync);
        match self.peek() {
            Tag::KeywordStruct => {
                if mods.is_inline {
                    return Err(self.err_here(ParseErrorKind::InlineStructNotAllowed));
                }
                self.parse_aggregate_decl(false, Vec::new())
            }
            Tag::KeywordUnion => {
                if mods.is_inline {
                    return Err(self.err_here(ParseErrorKind::InlineStructNotAllowed));
                }
                self.parse_aggregate_decl(true, Vec::new())
            }
            Tag::KeywordEnum => self.parse_enum_decl(),
            Tag::KeywordType => self.parse_type_alias_decl(),
            Tag::LBrace => {
                if !mods.is_extern {
                    return Err(self.err_here(ParseErrorKind::ExternBlockRequiresExtern));
                }
                self.parse_extern_block(mods.abi.clone())
            }
            Tag::KeywordImpl => {
                if mods.is_inline {
                    return Err(self.err_here(ParseErrorKind::InlineImplNotAllowed));
                }
                self.parse_impl_decl()
            }
            Tag::KeywordTrait => self.parse_trait_decl(),
            Tag::KeywordFn => self.parse_func_decl(&mods, false),
            Tag::KeywordConst => self.parse_const_stmt(),
            Tag::KeywordMacro => {
                if mods.is_inline {
                    return Err(self.err_here(ParseErrorKind::InlineMacroNotAllowed));
                }
                self.parse_macro_decl()
            }
            Tag::KeywordUsing => self.parse_using_decl(),
            _ => Err(self.err_here(ParseErrorKind::ExpectedDeclaration)),
        }
    }

    /// Zig `startsDecl` gate for `@`-prefixed declarations: the
    /// token after `@` must name a known @-declaration (Zig's
    /// fixed set plus this port's own additions: `arena`,
    /// `check_hook`, `sublanguage`). Anything else means the
    /// `@` does not start a declaration at all.
    pub fn at_decl_starts_decl(&mut self) -> bool {
        if self.peek() != Tag::At {
            return false;
        }
        self.advance();
        if self.peek() == Tag::KeywordOverload {
            self.advance();
            return true;
        }
        if self.peek() != Tag::Identifier {
            return false;
        }
        let name = self.cur_text().to_string();
        matches!(
            name.as_str(),
            "test"
                | "import"
                | "arena"
                | "check_hook"
                | "sublanguage"
                | "no_mangle"
                | "derive"
                | "intrinsic"
               
        )
        // NOTE: Zig also admits imported-macro names and
        // @expand_tuple template names here; this port has no
        // equivalent registry query yet (source_expand handles
        // @expand_tuple at the source level, before parsing),
        // so those two arms are deferred — registered in
        // RESULT as a known gate gap.
    }

    /// Zig `parseAtDecl`: `@test`, `@import`, `@no_mangle`, `@derive`.
    /// Zig `parseAtDecl`: `@test`, `@import`, `@no_mangle`, `@derive`.

    /// Zig `parseAtDecl`: `@test`, `@import`, `@no_mangle`, `@derive`.
    /// Zig `parseAtDecl`: `@test`, `@import`, `@no_mangle`, `@derive`.
    /// Zig `parseAtDecl`: `@test`, `@import`, `@no_mangle`, `@derive`.
    /// Zig `parseAtDecl`: `@test`, `@import`, `@no_mangle`, `@derive`.
    /// Zig `parseAtDecl`: `@test`, `@import`, `@no_mangle`, `@derive`.
    /// Zig `parseAtDecl`: `@test`, `@import`, `@no_mangle`, `@derive`.
    pub fn parse_at_decl(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::At)?;
        if self.peek() == Tag::KeywordOverload {
            // R-3 overload slice: `@overload Type { fn <op> ... }`.
            return self.parse_overload_decl();
        }
        let name = self.expect_identifier()?;
        match name.as_str() {
            "check_hook" => {
                // R-3 (J-5): `@check_hook <name>;` attaches a
                // registered read-only hook; the checker validates
                // the name and applies the audit.
                let hook = self.expect_identifier()?;
                self.expect(Tag::Semicolon)?;
                Ok(Node::CheckHookDecl(sla_ast::CheckHookDecl {
                    name: hook,
                }))
            }
            "sublanguage" => self.parse_sublanguage_decl(),
            "test" => self.parse_test_decl(),
            "import" => self.parse_import_decl(),
            "no_mangle" => {
                // Zig order: pub, extern (+abi), inline, async, then `fn`
                // (anything else is ExpectedDeclaration).
                let mut mods = DeclMods { no_mangle: true, ..DeclMods::default() };
                mods.is_pub = self.eat(Tag::KeywordPub);
                if self.eat(Tag::KeywordExtern) {
                    mods.is_extern = true;
                    mods.abi = self.parse_optional_extern_abi()?;
                }
                mods.is_inline = self.eat(Tag::KeywordInline);
                mods.is_async = self.eat(Tag::KeywordAsync);
                if self.peek() != Tag::KeywordFn {
                    return Err(self.err_here(ParseErrorKind::ExpectedDeclaration));
                }
                self.advance();
                self.parse_func_decl_rest(&mods, false)
            }
            "arena" => {
                // R-3: `@arena fn` sugar (F2-A C1, Jev U1 = C) — the
                // whole function body is one arena domain.
                let mut mods = DeclMods::default();
                mods.is_pub = self.eat(Tag::KeywordPub);
                mods.is_inline = self.eat(Tag::KeywordInline);
                mods.is_async = self.eat(Tag::KeywordAsync);
                if self.peek() != Tag::KeywordFn {
                    return Err(self.err_here(ParseErrorKind::ExpectedDeclaration));
                }
                self.advance();
                let mut node = self.parse_func_decl_rest(&mods, false)?;
                if let Node::FuncDecl(f) = &mut node {
                    f.is_arena = true;
                }
                Ok(node)
            }
            "derive" => {
                self.expect(Tag::LParen)?;
                let mut derives = Vec::new();
                loop {
                    if self.eat(Tag::RParen) {
                        break;
                    }
                    derives.push(self.expect_identifier()?);
                    if !self.eat(Tag::Comma) {
                        self.expect(Tag::RParen)?;
                        break;
                    }
                }
                // Zig: an optional `pub`, then `struct` is required
                // (unions are rejected with ExpectedDeclaration).
                self.eat(Tag::KeywordPub);
                if self.peek() != Tag::KeywordStruct {
                    return Err(self.err_here(ParseErrorKind::ExpectedDeclaration));
                }
                self.advance();
                self.parse_aggregate_decl_rest(false, derives)
            }
            _ => Err(self.err_here(ParseErrorKind::SyntaxError)),
        }
    }

    fn not_covered(&self, what: &str) -> ParseError {
        self.err_expected(
            ParseErrorKind::SyntaxError,
            format!("{} are not covered by the sla-parser slice yet", what),
        )
    }

    // -- functions ---------------------------------------------------------

    /// Zig `parseGenericParams`: `<T, U: Bound>` — bounds are a single
    /// identifier and are discarded, exactly as in Zig.
    pub fn parse_generic_params(&mut self) -> Result<Vec<String>, ParseError> {
        let mut generics = Vec::new();
        if !self.eat(Tag::LessThan) {
            return Ok(generics);
        }
        loop {
            if self.peek() == Tag::GreaterThan || self.peek() == Tag::GreaterGreater {
                self.expect_generic_close()?;
                break;
            }
            generics.push(self.expect_identifier()?);
            if self.eat(Tag::Colon) {
                self.expect_identifier()?;
            }
            if !self.eat(Tag::Comma) {
                self.expect_generic_close()?;
                break;
            }
        }
        Ok(generics)
    }

    /// Zig `parseFuncDecl` (the `fn` keyword is still current).
    pub fn parse_func_decl(&mut self, mods: &DeclMods, is_decl_only: bool) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordFn)?;
        self.parse_func_decl_rest(mods, is_decl_only)
    }

    /// Function declaration after the `fn` keyword has been consumed.
    pub fn parse_func_decl_rest(&mut self, mods: &DeclMods, is_decl_only: bool) -> Result<Node, ParseError> {
        let name = self.expect_identifier()?;
        let generics = self.parse_generic_params()?;
        self.expect(Tag::LParen)?;
        let mut params = Vec::new();
        let mut is_variadic_decl = false;
        loop {
            if self.eat(Tag::RParen) {
                break;
            }
            // R-3 (extern variadic): a trailing `...` marker.
            if self.peek() == Tag::DotDotDot {
                self.advance();
                is_variadic_decl = true;
                self.expect(Tag::RParen)?;
                break;
            }
            // R-3 (named variadic): `args: ...` — a named C
            // va_list parameter (the body reads it via va_next_*).
            if self.peek() == Tag::Identifier && self.peek2() == Tag::Colon {
                let save = self.snapshot();
                let candidate = self.cur_text().to_string();
                self.advance();
                self.advance();
                if self.peek() == Tag::DotDotDot {
                    self.advance();
                    params.push(sla_ast::decls::Param {
                        name: candidate,
                        ty: Box::new(Type::Primitive(Primitive::VaList)),
                        is_borrow: false,
                        is_move: false,
                        fallible_sugar: false,
                        is_var: false,
                    });
                    is_variadic_decl = true;
                    self.eat(Tag::Comma);
                    self.expect(Tag::RParen)?;
                    break;
                }
                self.restore(save);
            }
            let is_var = self.eat(Tag::KeywordVar);
            let is_borrow = self.eat(Tag::Ampersand);
            let is_move = !is_borrow && self.eat(Tag::Caret);
            let param_name = self.expect_identifier()?;
            self.expect(Tag::Colon)?;
            let ty = self.parse_type()?;
            // R-3 (A_min scope limit): `T!` in parameter position
            // parses (so the checker can name it) and is rejected
            // there with FallibleSugarParam — the sugar exists only
            // in return position.
            let fallible_sugar = self.eat(Tag::Bang);
            if self.peek() == Tag::Equal {
                return Err(self.err_expected(
                    ParseErrorKind::SyntaxError,
                    "parameter default values are not supported".to_string(),
                ));
            }
            params.push(sla_ast::decls::Param {
                name: param_name,
                ty: Box::new(ty),
                is_borrow,
                is_move,
                fallible_sugar,
                is_var,
            });
            if !self.eat(Tag::Comma) {
                self.expect(Tag::RParen)?;
                break;
            }
        }
        // R-3: return-position suffixes — the A_min fallible
        // sugar `-> T!` (desugars to `Result<T, i32>`, extern
        // declarations and ordinary functions alike per the
        // corpus) and the borrow-origin annotation `-> &[T][name]`
        // (recorded verbatim; the origin gates live in the
        // checker). Both suffixes attach AFTER the return type.
        let mut ret_fallible_sugar = false;
        let mut ret_borrow_origin: Option<String> = None;
        let ret_ty = if self.eat(Tag::Arrow) {
            let base = self.parse_type()?;
            if self.eat(Tag::Bang) {
                ret_fallible_sugar = true;
                Type::UserDefined(sla_ast::types::UserDefined {
                    name: "Result".to_string(),
                    generics: vec![
                        base,
                        Type::Primitive(Primitive::I32),
                    ],
                })
            } else {
                if self.eat(Tag::LBracket) {
                    ret_borrow_origin =
                        Some(self.expect_identifier()?);
                    self.expect(Tag::RBracket)?;
                }
                base
            }
        } else {
            Type::Primitive(Primitive::VoidType)
        };
        let body = if is_decl_only {
            Vec::new()
        } else {
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
            is_pub: mods.is_pub,
            is_extern: mods.is_extern,
            abi: mods.abi.clone(),
            no_mangle: mods.no_mangle,
            is_decl_only,
            generics,
            params,
            ret_ty: Box::new(ret_ty),
            body,
            is_inline: mods.is_inline,
            is_async: mods.is_async,
            operator: None,
            is_arena: false,
            ret_fallible_sugar,
            ret_borrow_origin,
            is_variadic: is_variadic_decl,
        }))
    }

    // -- macro ---------------------------------------------------------------

    /// Zig `parseMacroDecl`.
    pub fn parse_macro_decl(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordMacro)?;
        let name = self.expect_identifier()?;
        self.expect(Tag::LParen)?;
        let mut params = Vec::new();
        let mut param_tys = Vec::new();
        loop {
            if self.eat(Tag::RParen) {
                break;
            }
            params.push(self.expect_identifier()?);
            // R-3 (J-3): a parameter may declare a type
            // (`name: Type`); untyped parameters stay `None`.
            param_tys.push(if self.eat(Tag::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            });
            if !self.eat(Tag::Comma) {
                self.expect(Tag::RParen)?;
                break;
            }
        }
        let body = if self.should_parse_macro_body(&name) {
            self.parse_block()?
        } else {
            self.skip_block()?;
            Vec::new()
        };
        Ok(Node::MacroDecl(MacroDecl {
            name,
            params,
            param_tys,
            body,
        }))
    }

    // -- import / using --------------------------------------------------------

    /// Zig `parseImportDeclAfterAt`: `@import "path"` or a bare
    /// token-sequence path (no parentheses), then an optional `;`.
    /// For `.sla` paths the module name is recorded; the Zig original
    /// additionally prescans the imported file from disk here — that IO
    /// layer is out of this port's scope (harness seeds the names via
    /// `Parser::seed_known_type_names` instead; see divergences).
    pub fn parse_import_decl(&mut self) -> Result<Node, ParseError> {
        let path = if self.peek() == Tag::StringLiteral {
            let raw = self.cur_text().to_string();
            self.advance();
            let bytes = raw.as_bytes();
            if bytes.len() < 2 || bytes[0] != b'"' || bytes[bytes.len() - 1] != b'"' {
                return Err(self.err_here(ParseErrorKind::SyntaxError));
            }
            string_literal_inner(&raw).to_string()
        } else {
            self.parse_path_token_sequence()?
        };
        self.eat(Tag::Semicolon);
        self.record_import_module_name(&path);
        Ok(Node::ImportDecl(ImportDecl { path }))
    }

    /// Zig `parseUsingDecl`.
    pub fn parse_using_decl(&mut self) -> Result<Node, ParseError> {
        self.expect(Tag::KeywordUsing)?;
        let path = self.parse_path_token_sequence()?;
        self.eat(Tag::Semicolon);
        Ok(Node::UsingDecl(UsingDecl { path }))
    }

    /// Zig `parseImportPathTokenSequence`: concatenated lexemes of the
    /// path token set (identifier, `*`, `/`, `.`, `..`, `-`, `::`);
    /// an empty sequence is a `SyntaxError`, as in Zig.
    fn parse_path_token_sequence(&mut self) -> Result<String, ParseError> {
        let mut path = String::new();
        loop {
            match self.peek() {
                Tag::Identifier
                | Tag::Asterisk
                | Tag::Slash
                | Tag::Dot
                | Tag::Range
                | Tag::Minus
                | Tag::DoubleColon => {
                    path.push_str(self.cur_text());
                    self.advance();
                }
                _ => break,
            }
        }
        if path.is_empty() {
            return Err(self.err_here(ParseErrorKind::SyntaxError));
        }
        Ok(path)
    }

    // -- @test -----------------------------------------------------------------

    /// Zig `parseTestDecl` after `@test`: optional `ignored` /
    /// `should_panic` modifier identifiers, a string name, `()`, a block.
    pub fn parse_test_decl(&mut self) -> Result<Node, ParseError> {
        let mut is_ignored = false;
        let mut should_panic = false;
        while self.peek() == Tag::Identifier {
            match self.cur_text() {
                "ignored" => is_ignored = true,
                "should_panic" => should_panic = true,
                _ => break,
            }
            self.advance();
        }
        if self.peek() != Tag::StringLiteral {
            return Err(self.err_expected(ParseErrorKind::SyntaxError, "string_literal".to_string()));
        }
        let raw = self.cur_text().to_string();
        self.advance();
        let raw_bytes = raw.as_bytes();
        if raw_bytes.len() < 2 || raw_bytes[0] != b'"' || raw_bytes[raw_bytes.len() - 1] != b'"' {
            return Err(self.err_here(ParseErrorKind::SyntaxError));
        }
        let name = string_literal_inner(&raw).to_string();
        self.expect(Tag::LParen)?;
        self.expect(Tag::RParen)?;
        let body = if self.should_parse_test_body() {
            self.parse_block()?
        } else {
            self.skip_block()?;
            Vec::new()
        };
        Ok(Node::TestDecl(TestDecl { name, is_ignored, should_panic, body }))
    }

    // -- extern block ------------------------------------------------------------

    /// Zig `parseExternBlock`: `{ fn ...; ... }` producing a `Program`
    /// node of decl-only functions (flattened by `parse_program`).
    pub fn parse_extern_block(&mut self, abi: Option<String>) -> Result<Node, ParseError> {
        self.expect(Tag::LBrace)?;
        let mut decls = Vec::new();
        while self.peek() != Tag::RBrace && self.peek() != Tag::Eof {
            let mut mods = DeclMods {
                is_extern: true,
                abi: abi.clone(),
                ..DeclMods::default()
            };
            mods.is_pub = self.eat(Tag::KeywordPub);
            decls.push(self.parse_func_decl(&mods, true)?);
            self.eat(Tag::Semicolon);
        }
        self.expect(Tag::RBrace)?;
        Ok(Node::Program(Program { decls }))
    }

    // -- modules -------------------------------------------------------------------

    /// Zig `parseModuleDecl`: `mod name { ... }` whose function and
    /// type-alias members are appended to `decls` with names mangled to
    /// `prefix__name` (nested modules extend the prefix).
    pub fn parse_module_decl_into(
        &mut self,
        decls: &mut Vec<Node>,
        parent_prefix: Option<&str>,
    ) -> Result<(), ParseError> {
        self.expect(Tag::KeywordMod)?;
        let mod_name = self.expect_identifier()?;
        let prefix = match parent_prefix {
            Some(parent) => format!("{}__{}", parent, mod_name),
            None => mod_name,
        };
        self.expect(Tag::LBrace)?;
        while self.peek() != Tag::RBrace && self.peek() != Tag::Eof {
            if self.peek() == Tag::KeywordMod {
                self.parse_module_decl_into(decls, Some(&prefix))?;
            } else if self.peek() == Tag::KeywordType {
                let mut node = self.parse_type_alias_decl()?;
                if let Node::TypeAliasDecl(decl) = &mut node {
                    decl.name = format!("{}__{}", prefix, decl.name);
                }
                self.register_declared_names(&node);
                decls.push(node);
            } else {
                let mut mods = DeclMods::default();
                mods.is_pub = self.eat(Tag::KeywordPub);
                mods.is_inline = self.eat(Tag::KeywordInline);
                mods.is_async = self.eat(Tag::KeywordAsync);
                if self.peek() != Tag::KeywordFn {
                    return Err(self.err_here(ParseErrorKind::ExpectedDeclaration));
                }
                let mut node = self.parse_func_decl(&mods, false)?;
                if let Node::FuncDecl(decl) = &mut node {
                    decl.name = format!("{}__{}", prefix, decl.name);
                }
                decls.push(node);
            }
            self.eat(Tag::Semicolon);
        }
        self.expect(Tag::RBrace)?;
        Ok(())
    }
}


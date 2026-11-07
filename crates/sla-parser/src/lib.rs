//! sla-parser: recursive-descent / Pratt parser mirrored from
//! `sa_plugin_sla/src/parser.zig` (4,478 lines).
//!
//! The parser consumes source text, drives `sla_lexer` over it, and
//! produces the `sla_ast` node tree. Grammar divergences from the Zig
//! original are registered in `docs/divergences/parser.md` — nothing is
//! silently harmonized.
//!
//! Module map (each file mirrors the named region of parser.zig):
//!   parser  — cursor machinery, name registries, body-span handling
//!   decl    — parseProgram / parseDecl / @-decls / fn / macro / import /
//!             using / @test / extern blocks / modules
//!   decl_types — struct / union / enum / type-alias declarations
//!   stmt    — blocks and statements, assignment desugars
//!   expr    — the Pratt loop and its desugar interceptions
//!   prefix  — prefix expressions, if/switch/match, closures, asm
//!   infix   — infix expressions (binary, dot, call, index, cast, `?`)
//!   tails   — generic-lookahead probes and literal/call tails
//!   types   — parseType
//!   pattern — enum patterns, let patterns, if-let chains
//!   numlit  — numeric literal scanning helpers
//!   error   — ParseError (mirror of the Zig ParserError set)
//!   contract — .sai/.sal contract parsing (contract_parser.zig)
//!   imports  — import path resolution (plugin_imports.zig); the
//!              crate's one IO module, kept apart from the parser
//!   contract_load — loadImportedContracts orchestration
//!              (plugin_import_expand.zig slice)

pub mod contract;
pub mod contract_load;
pub mod decl;
pub mod decl_types;
pub mod error;
pub mod expr;
pub mod imports;
pub mod infix;
pub mod numlit;
pub mod parser;
pub mod pattern;
pub mod prefix;
pub mod stmt;
pub mod tails;
pub mod types;

pub use contract::{ExternalFunction, LayoutDefine};
pub use contract_load::{load_imported_contracts, ContractLoadError, LoadReport};
pub use error::{ParseError, ParseErrorKind};
pub use imports::ResolvedImport;
pub use parser::{Parser, ParserOptions};

use sla_ast::node::Node;

/// Parse a complete SLA source file into its `Program` node
/// (Zig `Parser.parseProgram`).
pub fn parse_program(source: &str) -> Result<Node, ParseError> {
    Parser::new(source).parse_program()
}

/// `parse_program` with explicit body-parsing options.
pub fn parse_program_with_options(
    source: &str,
    options: ParserOptions,
) -> Result<Node, ParseError> {
    Parser::with_options(source, options).parse_program()
}

//! sla-lexer: tokenizer mirrored from sa_plugin_sla/src/lexer.zig.
//!
//! Crate wiring (this file) is coordinator-owned; module internals are
//! owned by the lexer/ast lane. See docs/COORDINATION.md.

pub mod keyword;
pub mod lexer;
pub mod token;

pub use lexer::Lexer;
pub use token::{Loc, Tag, Token};

//! sla-ast: AST node definitions mirrored from sa_plugin_sla/src/ast.zig.
//!
//! Crate wiring (this file) is coordinator-owned; module internals are
//! owned by the lexer/ast lane. See docs/COORDINATION.md.

pub mod decls;
pub mod exprs;
pub mod node;
pub mod stmts;
pub mod types;

pub use decls::Program;
pub use node::Node;
pub use types::Type;

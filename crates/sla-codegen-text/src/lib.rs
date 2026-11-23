//! sla-codegen-text: SA text codegen mirrored from codegen.zig
//! (16,288 lines). Lane D scope (vertical slice): the emission paths
//! the fixture programs exercise are ported faithfully — byte-identical
//! output against the Zig plugin is the acceptance line — and every
//! path outside the slice fails loudly with [`TextError::Unsupported`]
//! instead of silently approximating. See
//! `docs/divergences/lowering-text.md`.
//!
//! Module map (codegen.zig is one struct; the Rust impl keeps one
//! `Codegen` struct and splits its methods by emission domain):
//! - `codegen`  — state, counters, name resolution, small emitters
//! - `program`  — `generate()`: imports, macro prelude, vtables, order
//! - `func`     — function and test declaration emission
//! - `block`    — blocks, tail-value stores, loop-local cleanup
//! - `collect`  — assigned/addressable/hoisted binding pre-passes
//! - `stmt`     — statement emission
//! - `stmt_while` — the while statement (plain + pattern arms)
//! - `expr`     — expression emission (literals, identifiers, binary)
//! - `expr_if`  — the if expression (plain + let-chain arms)
//! - `call`     — call expressions and call statements
//! - `println`  — the println emitter
//! - `release`  — release/cleanup emission and branch-state restore
//! - `place`    — place-address emission (index/field addresses)

mod async_plans_codegen;
mod async_plans_codegen2;
mod async_runtime_codegen;
mod async_scalar;
mod block;
mod call;
mod chan;
mod select;
mod clone;
mod closure;
mod call_plan;
mod codegen;
mod copy;
mod drop;
mod family_macros;
mod liveness;
mod surface;
mod thread_spawn;
mod thread_spawn_emit;
mod collect;
mod destructure;
mod dyn_coercion;
mod expr;
mod expr_if;
mod expr_match;
mod expr_switch;
mod extern_decl;
mod extern_macro;
mod format;
mod func;
mod go_spawn;
mod agg_literal;
mod into;
mod iter_window;
mod layout;
mod map;
mod min_imports;
mod monitor;
mod place;
mod println;
mod program;
mod release;
mod stmt;
mod stmt_while;
mod user_macro;
mod vec_lift;
mod vec_stack;
mod vtable;

pub use codegen::Codegen;
pub use sla_lowering::LoweringError;

use sla_ast::Node;
use sla_lowering::CheckInfo;

/// Codegen failure: an unported emission path (loud, with the Zig
/// reference), a missing checker fact the Zig original would have
/// errored on, or a lowering error propagated unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextError {
    Unsupported {
        feature: String,
        zig_ref: &'static str,
    },
    MissingType {
        context: String,
    },
    Lowering(LoweringError),
}

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextError::Unsupported { feature, zig_ref } => write!(
                f,
                "sla-codegen-text: unsupported emission `{feature}` (Zig reference: {zig_ref})"
            ),
            TextError::MissingType { context } => {
                write!(f, "sla-codegen-text: checker type missing for {context}")
            }
            TextError::Lowering(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for TextError {}

impl From<LoweringError> for TextError {
    fn from(err: LoweringError) -> Self {
        TextError::Lowering(err)
    }
}

pub type Result<T> = std::result::Result<T, TextError>;

/// Loud-failure constructor for unported emission paths.
pub fn unsupported<T>(feature: &'static str, zig_ref: &'static str) -> Result<T> {
    Err(TextError::Unsupported {
        feature: feature.to_string(),
        zig_ref,
    })
}

/// Generate SA text for a checked program. Mirrors
/// `Codegen.generate` (codegen.zig:7032).
pub fn generate(program: &Node, check: &dyn CheckInfo) -> Result<String> {
    let mut codegen = Codegen::new(check);
    codegen.generate_program(program)
}

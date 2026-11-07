//! sla-codegen-sab: SAB binary codegen, the Rust differential port of
//! sa_plugin_sla's `sab_codegen.zig` (20,024 lines).
//!
//! Phase 1 ported:
//! - the SAB v4 encoder — a faithful port of sci `sab.zig`'s write
//!   path (`encode.rs`, `pool.rs`, `leb128.rs`, `ir.rs`);
//! - the emission trunk — interning, naming, decl instructions,
//!   primitive instruction emitters and string-const handling
//!   (`emit.rs`, `literals.rs`);
//! - a hand-lowered structural model of the simplest coordinator
//!   fixture (`fixture01.rs`) exercising both.
//!
//! Phase 2 (this slice) adds the B-group state and driver:
//! - `types.rs` (B2): the Zig auxiliary structures (lines 66–293);
//! - `state.rs` (B3): the `Codegen` field set, split into
//!   ownership-clear sub-states per SAB-D09 (lines 294–534);
//! - `driver.rs` + `driver_gen.rs` (B4): the `generate`
//!   orchestration (lines 535–632) and the statement/expression
//!   generation for the fixture01-level subset — user functions,
//!   inter-function calls, literals, scalar ops, `if`/`while`.
//!
//! The full decomposition of the Zig file into <=700-line modules
//! is in `docs/sab-decomposition.md`; suspicious constructs found
//! while reading it are registered in `docs/divergences/sab.md` and
//! are reproduced, never silently harmonized.
//!
//! Everything outside the B4 subset — std surface expansion
//! (D group: `println`, `str_eq`, string materialization), macro
//! and fragment machinery (F/G), async (H), closures and impls
//! (I) — fails loudly through `Error::UnsupportedSabDirectFeature`
//! with its group attribution. SAB never falls back to the text
//! backend.

pub mod driver;
pub mod driver_gen;
pub mod emit;
pub mod encode;
pub mod error;
pub mod fixture01;
pub mod ir;
pub mod leb128;
pub mod literals;
pub mod pool;
pub mod state;
pub mod types;

pub use emit::ModuleBuilder;
pub use encode::encode_module;
pub use error::{Error, Result};
pub use ir::Module;

/// Serialize a fully built module to SAB v4 bytes.
pub fn generate(module: &Module) -> Result<Vec<u8>> {
    encode_module(module)
}

/// AST-level entry point, mirroring the Zig top-level
/// `generate(allocator, tc, program) -> []u8`.
///
/// Drives the program through the B4 orchestration (`driver.rs`)
/// on the checked-program surface sla-lowering exposes
/// (`&Node` + `&dyn CheckInfo`, the same shape sla-codegen-text
/// uses) and encodes the emitted module. Constructs outside the
/// fixture01-level subset fail loudly — see the crate docs.
pub fn generate_from_program(
    program: &sla_ast::Node,
    check: &dyn sla_lowering::CheckInfo,
) -> Result<Vec<u8>> {
    driver::generate_from_program(program, check)
}

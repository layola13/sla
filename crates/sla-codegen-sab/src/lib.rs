//! sla-codegen-sab: SAB binary codegen, the Rust differential port of
//! sa_plugin_sla's `sab_codegen.zig` (20,024 lines).
//!
//! Phase 1 (this slice) ports:
//! - the SAB v4 encoder — a faithful port of sci `sab.zig`'s write
//!   path (`encode.rs`, `pool.rs`, `leb128.rs`, `ir.rs`);
//! - the emission trunk — interning, naming, decl instructions,
//!   primitive instruction emitters and string-const handling
//!   (`emit.rs`, `literals.rs`);
//! - a hand-lowered structural model of the simplest coordinator
//!   fixture (`fixture01.rs`) exercising both.
//!
//! The full decomposition of the Zig file into <=700-line modules is
//! in `docs/sab-decomposition.md`; suspicious constructs found while
//! reading it are registered in `docs/divergences/sab.md` and are
//! reproduced, never silently harmonized.
//!
//! Statement/expression generation (the `gen*` families) arrives in
//! later phases along the decomposition map. Until then, AST-level
//! entry points fail loudly with
//! `Error::UnsupportedSabDirectFeature` — SAB never falls back to the
//! text backend.

pub mod emit;
pub mod encode;
pub mod error;
pub mod fixture01;
pub mod ir;
pub mod leb128;
pub mod literals;
pub mod pool;

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
/// Not yet wired: AST-driven generation needs the checked program plus
/// sla-lowering's facts (the `CheckInfo` surface). Until that wiring
/// exists this returns a loud `UnsupportedSabDirectFeature` rather
/// than an empty or text-routed result.
///
/// Wiring note (coordinator, 2026-11-07): the signature previously
/// took the coordinator's placeholder `sla_lowering::Lowering`, which
/// the lowering lane has since replaced with its real API; the stub
/// now consumes that real surface (`&Node` + `&dyn CheckInfo`, the
/// same shape sla-codegen-text uses). Body unchanged.
pub fn generate_from_program(
    _program: &sla_ast::Node,
    _check: &dyn sla_lowering::CheckInfo,
) -> Result<Vec<u8>> {
    Err(Error::UnsupportedSabDirectFeature(
        "AST -> SAB direct generation awaits the sla-lowering pipeline (phase 2)".to_string(),
    ))
}

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
//! Phase 3 (SAB-D slice) adds the D-group std surface
//! infrastructure:
//! - `surface_load.rs` (D1): surface-file location (the
//!   multi-root fallback chains, loudly logged per SAB-D02) and
//!   rule parsing (lines 1273–1589);
//! - `layout.rs` (D2): prim/storage types and tuple/array/struct
//!   layout adapters over sla-lowering (lines 1590–1706,
//!   1776–1782);
//! - `copy_facts.rs` (D3): Copy-derive / Copy-value /
//!   shallow-copy facts with Zig's derive cache (lines
//!   1783–1883);
//! - `surface_query.rs` (D4): rule lookup, argument rendering,
//!   macro-name expansion, and the dep-ensuring surface (lines
//!   1884–1997);
//! - `surface_preload.rs` + `surface_scan.rs` (D5): dep
//!   preload traversal, future/thread usage scans, and
//!   `stackAllocSize` (lines 1998–2317, 2419).
//! The driver's step 2 (surface load + preload) is wired to
//! D1/D5; dep ensuring queues into `pending_std_deps` (SAB-D26)
//! because the F-group fragment append is not yet ported, and
//! surface *calls* still stop loudly at their D-group call
//! sites. See docs/phase3/sab-d.md.
//!
//! Phase 3 (SAB-C slice) adds the C-group pre-scan and symbols:
//! - `prescan_consts.rs` (C1): global scalar/array const
//!   collection and folding (lines 633–854);
//! - `prescan_bindings.rs` (C2): assigned/borrowed binding
//!   collection, repeated-let preparation, the register ledger
//!   (lines 855–934, 2954–3056);
//! - `symbols.rs` (C3): interning, binding registers, lowered /
//!   mangled names, trait and fn-ptr vtable const emission
//!   (lines 935–1102).
//! The driver's C-group call points (pre-scan step, trait vtable
//! step, extern suppression, binding registers, lowered symbols)
//! are wired to these modules; see docs/phase3/sab-c.md.
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

pub mod copy_facts;
pub mod driver;
pub mod driver_gen;
pub mod emit;
pub mod encode;
pub mod error;
pub mod fixture01;
pub mod ir;
pub mod layout;
pub mod leb128;
pub mod literals;
pub mod pool;
pub mod prescan_bindings;
pub mod prescan_consts;
pub mod state;
pub mod surface_load;
pub mod surface_preload;
pub mod surface_query;
pub mod surface_scan;
pub mod symbols;
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

/// [`generate_from_program`] with explicit D1 surface roots
/// (tests and harnesses point the fallback chains at fixtures
/// instead of the process environment).
pub fn generate_from_program_with_roots(
    program: &sla_ast::Node,
    check: &dyn sla_lowering::CheckInfo,
    roots: surface_load::SurfaceRoots,
) -> Result<Vec<u8>> {
    driver::generate_from_program_with_roots(program, check, roots)
}

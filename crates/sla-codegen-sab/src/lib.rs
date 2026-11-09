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
//! D1/D5; dep ensuring ran queue-only (SAB-D26) until the
//! F group landed. See docs/phase3/sab-d.md.
//!
//! Phase 4 (SAB-F slice) adds the F-group fragment machinery:
//! - `decode.rs` / `decode_sigs.rs` / `decode_inst.rs` (F1/F2):
//!   the SAB byte decoder — sci `sab.zig`'s `decodeModule` read
//!   path, the entry point every fragment splice starts from;
//! - `fragment.rs` / `fragment_names.rs`: the decoded-module
//!   remap — stable interning for globals, hygienic
//!   `__decoded_r_…` registers, label remap, text rewriting;
//! - `fragment_append.rs`: sig/instruction cloning, dep-closure
//!   filtered appends, const appends, body splices;
//! - `fragment_template.rs`: the `__sla_macro_arg_{n}`
//!   placeholder templates and hygiene-renamed fragment bodies;
//! - `fragment_imports.rs`: cached std import modules and
//!   macro templates, dep append/flush, and the production
//!   `SurfaceEmitter` / `StdDepEnsurer` implementations.
//! The one boundary this crate cannot supply is sci's
//! `flattenStdSnippet` (SA-text flattener + verifier), behind
//! the `StdSnippetFlattener` trait; the default provider stops
//! loudly there (SAB-D31). Driver step 2 now appends preload
//! deps immediately outside function bodies, exactly as Zig's
//! `ensureStdDeps` does (SAB-D26 resolved), so a program that
//! reaches for baked std surfaces dies at that F boundary —
//! one level deeper than the old D-group call-site stop.
//!
//! Phase 4 (SAB-G slice) adds the G-group macro-expansion and
//! call-site materialization machinery:
//! - `macro_plan.rs`: the ArgRole classification — imported-macro
//!   call plans, per-argument lowering actions and address-of
//!   shapes (lowering_rules.zig:101/500/528/570);
//! - `macro_ctx.rs`: `MacroExpansionContext` mechanics —
//!   argument/local lookup, hygienic naming, scope mark/restore
//!   (sab_codegen.zig:8867–8935, 9584);
//! - `callsite_args.rs`: `SabLoweredCallArg` and the call-site
//!   buffer materialization family — value / leading-output /
//!   materialized-slot / address-expression arguments and the
//!   per-argument dispatch (sab_codegen.zig:14452–14689, 15960);
//! - `macro_expand.rs`: the expansion orchestration
//!   (`genImportedMacroCall`, sab_codegen.zig:15072) over the
//!   F-group fragment machine and the direct-macro seam.
//! The mechanisms are not yet wired into the driver's `gen_call`
//! (that surface wiring is the F/driver side); they fail through
//! host seams, never silently. See the G-group report.
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
//! Phase 4 (SAB-I slice) adds the I-group aggregate / closure
//! family:
//! - `agg_emit.rs`: the memory emission primitives and stored-
//!   value release helpers the family consumes (Zig 4417–4800,
//!   4240–4340), implemented over the trunk's public instruction
//!   append so the trunk file itself needs no I-group edits;
//! - `aggregate.rs`: struct/union literals, field plans and
//!   transfer planning, `genCopyValue` / shallow-copy / deep-
//!   clone dispatch (Zig 7913, 21172–22239, 7663);
//! - `aggregate_seq.rs`: tuple/array/repeat-array/enum literals
//!   (Zig 21472, 22133–22400, 10983);
//! - `closures.rs`: inline and let-bound closures, closure
//!   calls, `genMove`, and impl/overload decl dispatch
//!   (Zig 4009, 7314–7360, 8156–8180, 14283, 19351–19412).
//! Escaped (thread-spawn) closures stay loudly unported; see
//! the I-group REPORT for the death-point coordinates.
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

pub mod agg_emit;
pub mod aggregate;
pub mod aggregate_copy;
pub mod aggregate_seq;
pub mod branch_state;
pub mod callsite_args;
pub mod captures;
pub mod closures;
pub mod copy_facts;
pub mod decode;
pub mod decode_inst;
pub mod decode_sigs;
pub mod driver;
pub mod driver_gen;
pub mod driver_types;
pub mod gen_print;
pub mod gen_surface_fn;
pub mod gen_struct;
pub mod gen_stmt;
pub mod gen_surface_call;
pub mod emit;
pub mod encode;
pub mod error;
pub mod fixture01;
pub mod fragment;
pub mod fragment_append;
pub mod fragment_imports;
pub mod fragment_names;
pub mod fragment_template;
pub mod ir;
pub mod layout;
pub mod leb128;
pub mod literals;
pub mod locals;
pub mod macro_ctx;
pub mod macro_expand;
pub mod macro_plan;
pub mod opkinds;
pub mod pool;
pub mod prescan_bindings;
pub mod release_drop;
pub mod release_emit;
pub mod release_locals;
pub mod release_meta;
pub mod release_plans;
pub mod release_prim;
pub mod prescan_consts;
pub mod state;
pub mod surface_load;
pub mod surface_preload;
pub mod surface_query;
pub mod surface_scan;
pub mod symbols;
pub mod types;

pub use decode_sigs::{decode_function_sigs_only, decode_module, decode_test_function_sigs_only};
pub use emit::ModuleBuilder;
pub use encode::encode_module;
pub use error::{Error, Result};
pub use fragment::FragmentMachine;
pub use fragment_imports::{MachineStdDeps, MachineSurfaceEmitter, NoSnippetFlattener, StdSnippetFlattener};
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

/// [`generate_from_program_with_roots`] with an explicit F-group
/// snippet flattener (the sci flattener boundary; the default
/// entry points use [`NoSnippetFlattener`], which stops loudly
/// at that boundary whenever a program actually reaches it).
pub fn generate_from_program_with_flattener(
    program: &sla_ast::Node,
    check: &dyn sla_lowering::CheckInfo,
    roots: surface_load::SurfaceRoots,
    flattener: Box<dyn StdSnippetFlattener>,
) -> Result<Vec<u8>> {
    driver::generate_from_program_with_flattener(program, check, roots, flattener)
}

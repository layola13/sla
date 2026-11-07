//! sla-oracle: dev-only differential harness (six-house verdict 2026-11-07,
//! mandatory item 5). Nobody depends on this crate.
//!
//! Duties:
//! - run the Rust pipeline over fixtures/ and byte-compare against the Zig
//!   plugin's expected outputs (fixtures/expected/);
//! - emit a NORMALIZED BOUNDARY DUMP at every crate boundary so a divergence
//!   can be attributed to one stage instead of only showing up as a final
//!   byte diff: tokens / AST / checked annotations / lowering trace /
//!   reachable set / per-function text+SAB bytes;
//! - staged timing (SLA_RS_PROFILE=1 prints per-stage ms to stderr).
//!
//! Usage: sla-oracle <input.sla> [-o out.sa] [--dump-boundaries <dir>]

fn main() {
    eprintln!("sla-oracle: pipeline not yet integrated (coordinator skeleton)");
    std::process::exit(2);
}

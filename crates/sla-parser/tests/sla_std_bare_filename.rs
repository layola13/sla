//! CLI-D6 regression: the mainstream sla_std thin-wrapper import
//! must resolve when the importing file is named by a BARE filename
//! (no directory component) — the exact shape of the phase-4 CLI
//! parity case `check_sla_std` (`sla check p_sla_std.sla` run with
//! the case directory as cwd, the sla_std tree two ancestors up
//! under a `sa_plugin_sla/` intermediate root, mirroring the parity
//! script's symlink layout).
//!
//! Root cause pinned by this test: the ancestor-root walk in
//! `imports::read_import_from_ancestor_roots` starts at
//! `real_path(base_dir)`; for a bare filename the parent is the
//! empty path, whose canonicalization fails, so the walk started
//! (and ended) at "" and never reached the cwd's ancestors. Direct
//! `.sai` imports survived only because `SA_STD_DIR` rescues them;
//! sla_std has no such env in the parity shape.
//!
//! This file contains exactly ONE test because it changes the
//! process cwd; it lives in its own integration-test binary so the
//! cwd change cannot race other tests.

use sla_parser::contract_load::load_imported_contracts;
use std::path::PathBuf;

const FMT_SAI: &str = "@extern sa_fmt_i64(value: i64, base: u32) -> u64\n\
                       @extern sa_fmt_buffer_len(buffer: u64) -> u64\n\
                       @extern sa_mem_free(^ptr: ptr) -> void\n";

const WRAP_FMT_SLA: &str = "@import \"sa_std/fmt.sai\"\n\
    fn sla_fmt_i64(value: i64, base: u32) -> u64 { return sa_fmt_i64(value, base); }\n\
    fn sla_fmt_buffer_len(buffer: u64) -> u64 { return sa_fmt_buffer_len(buffer); }\n";

#[test]
fn sla_std_wrap_resolves_from_bare_filename_source() {
    let root = std::env::temp_dir().join(format!(
        "sla_std_bare_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let write = |rel: &str, content: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    };
    write("sa_std/fmt.sai", FMT_SAI);
    write("sa_plugin_sla/sla_std/wrap/fmt.sla", WRAP_FMT_SLA);
    write(
        "check/case/p_sla_std.sla",
        "@import \"sla_std/wrap/fmt.sla\"\nfn main() -> i32 { return 0; }\n",
    );

    let case_dir: PathBuf = root.join("check").join("case");
    let prev_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&case_dir).unwrap();
    let result = load_imported_contracts(
        &["sla_std/wrap/fmt.sla".to_string()],
        std::path::Path::new("p_sla_std.sla"),
    );
    std::env::set_current_dir(&prev_cwd).unwrap();
    let _ = std::fs::remove_dir_all(&root);

    let report = result.expect(
        "sla_std wrap import from a bare filename must resolve via ancestor roots",
    );
    // The wrapper's re-exported extern closure, nested-first order.
    assert_eq!(
        report.extern_names(),
        vec!["sa_fmt_i64", "sa_fmt_buffer_len", "sa_mem_free"]
    );
    assert_eq!(report.visited.len(), 2);
    assert!(
        report.visited[0].ends_with("sla_std/wrap/fmt.sla"),
        "visited[0] must be the wrap file: {}",
        report.visited[0]
    );
    assert!(
        report.visited[1].ends_with("sa_std/fmt.sai"),
        "visited[1] must be the nested .sai: {}",
        report.visited[1]
    );
    assert_eq!(report.sla_imports.len(), 1);
}

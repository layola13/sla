//! CLI-D6 dual-form pipeline proof: the same fmt surface checked
//! through the Rust pipeline in BOTH import forms —
//!   (a) mainstream thin wrapper: `@import "sla_std/wrap/fmt.sla"`,
//!       the program calls the wrapper `sla_fmt_i64`;
//!   (b) direct contract: `@import "sa_std/fmt.sai"`, the program
//!       calls the extern `sa_fmt_i64` directly (compatibility
//!       form, kept working per the 2026-11-07 14:40 correction).
//! Both must type-check, and both must register the identical
//! extern closure. The wrapper case composes the two wirings that
//! meet at CLI-D6: `contract_load` (root resolution through the
//! shared `imports` ancestor walk + the nested scan that follows
//! the wrapper body's re-import of the `.sai`) and `sla_expand`
//! (include-all body expansion, decl-only on the check path and
//! raw for the build-shaped path).

use sla_ast::node::Node;
use sla_checker::Checker;
use sla_plugin::contracts::load_imported_contracts_into;
use sla_plugin::sla_expand::{expand_sla_imports, SlaImportExpansionOptions};
use std::path::PathBuf;

struct TempTree(PathBuf);

impl TempTree {
    fn new(tag: &str) -> TempTree {
        let dir = std::env::temp_dir().join(format!(
            "sla_plugin_d6_{}_{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempTree(dir)
    }
    fn write(&self, rel: &str, content: &str) {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn parse(source: &str) -> sla_ast::Program {
    let node = sla_parser::parse_program(source).expect("fixture-shaped source must parse");
    let Node::Program(program) = node else {
        panic!("parser root must be a Program");
    };
    program
}

const FMT_SAI: &str = "@extern sa_fmt_i64(value: i64, base: u32) -> u64\n\
                       @extern sa_fmt_buffer_len(buffer: u64) -> u64\n\
                       @extern sa_mem_free(^ptr: ptr) -> void\n";

// The real generated wrapper shape (sai-wrap-pilot `full/wrap/fmt.sla`,
// trimmed to the functions this fixture exercises).
const WRAP_FMT_SLA: &str = "@import \"sa_std/fmt.sai\"\n\
    fn sla_fmt_i64(value: i64, base: u32) -> u64 { return sa_fmt_i64(value, base); }\n\
    fn sla_fmt_buffer_len(buffer: u64) -> u64 { return sa_fmt_buffer_len(buffer); }\n";

const EXPECTED_EXTERNS: [&str; 3] = ["sa_fmt_i64", "sa_fmt_buffer_len", "sa_mem_free"];

#[test]
fn sla_std_wrap_form_checks_through_the_pipeline() {
    let tree = TempTree::new("wrap");
    tree.write("sa_std/fmt.sai", FMT_SAI);
    tree.write("sla_std/wrap/fmt.sla", WRAP_FMT_SLA);
    // The original CLI-D6 probe shape (parity corpus p_sla_std.sla).
    let source = "@import \"sla_std/wrap/fmt.sla\"\n\
        fn main() -> i32 {\n\
        \x20   let h = sla_fmt_i64(255, 16);\n\
        \x20   if h == 0 { return 1; };\n\
        \x20   return 0;\n\
        }\n";
    tree.write("main.sla", source);
    let program = parse(source);

    // Contract side: root resolution + wrapper-body re-import scan.
    let mut checker = Checker::new();
    let report = load_imported_contracts_into(&mut checker, &program, &tree.0.join("main.sla"))
        .expect("sla_std wrap contract load must succeed");
    assert_eq!(report.extern_names(), EXPECTED_EXTERNS);
    assert_eq!(report.visited.len(), 2);
    assert!(report.visited[0].ends_with("sla_std/wrap/fmt.sla"));
    assert!(report.visited[1].ends_with("sa_std/fmt.sai"));

    // Check path: include-all expansion, imported bodies decl-only.
    let expanded = expand_sla_imports(
        &program,
        &tree.0.join("main.sla"),
        &SlaImportExpansionOptions {
            imported_bodies_decl_only: true,
            include_all_imported_decls: true,
            ..Default::default()
        },
    )
    .expect("sla_std wrap expansion must succeed");
    checker
        .check_program(&expanded.program)
        .expect("wrap-form program must check with the wrapper decl-only in scope");

    // Build-shaped path: raw expansion keeps the wrapper bodies,
    // which call the registered externs.
    let raw = expand_sla_imports(
        &program,
        &tree.0.join("main.sla"),
        &SlaImportExpansionOptions {
            include_all_imported_decls: true,
            ..Default::default()
        },
    )
    .expect("raw sla_std wrap expansion must succeed");
    let mut raw_checker = Checker::new();
    load_imported_contracts_into(&mut raw_checker, &program, &tree.0.join("main.sla"))
        .expect("contract load must succeed on the raw path too");
    raw_checker
        .check_program(&raw.program)
        .expect("wrap-form program must check with wrapper bodies in scope");
}

#[test]
fn sa_std_direct_form_checks_through_the_pipeline() {
    let tree = TempTree::new("direct");
    tree.write("sa_std/fmt.sai", FMT_SAI);
    let source = "@import \"sa_std/fmt.sai\"\n\
        fn main() -> i32 {\n\
        \x20   let h = sa_fmt_i64(255, 16);\n\
        \x20   if h == 0 { return 1; };\n\
        \x20   return 0;\n\
        }\n";
    tree.write("main.sla", source);
    let program = parse(source);
    let mut checker = Checker::new();
    let report = load_imported_contracts_into(&mut checker, &program, &tree.0.join("main.sla"))
        .expect("direct .sai contract load must succeed");
    // Identical extern closure to the wrapper form.
    assert_eq!(report.extern_names(), EXPECTED_EXTERNS);
    assert_eq!(report.visited.len(), 1);
    checker
        .check_program(&program)
        .expect("direct-form program must check");
}

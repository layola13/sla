//! INT-2 pipeline proof for the contract segment.
//!
//! These tests run the real chain — parse a source file, load its
//! imported contracts through `sla_plugin::contracts`, register
//! them into a fresh `Checker`, then check — against a temp-dir
//! contract tree shaped like the fixtures' `sa_std` imports.
//!
//! They pin both halves of the INT-2 accounting:
//!   * the contract segment WORKS: a direct call to a contract
//!     extern (`sa_print_bytes`) is rejected before the load and
//!     accepted after it (the Zig plugin accepts it too — venue
//!     probe p3, docs/probes/int2/);
//!   * the remaining fixture blocker is NOT in this segment:
//!     `println` still fails after a complete contract load, with
//!     the checker's unresolved-call diagnostic, because in Zig
//!     `println` is a checker name-special (type_checker.zig
//!     isPrintlnCall arm), not a contract — INT2-1.

use sla_ast::node::Node;
use sla_checker::Checker;
use sla_plugin::contracts::load_imported_contracts_into;
use std::path::PathBuf;

struct TempTree(PathBuf);

impl TempTree {
    fn new(tag: &str) -> TempTree {
        let dir = std::env::temp_dir().join(format!(
            "sla_plugin_contracts_{}_{}",
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

const PRINT_SAI: &str = "// Legacy print contract.\n@extern sa_print_bytes(&msg: ptr, len: u64) -> void\n";

fn parse(source: &str) -> sla_ast::Program {
    let node = sla_parser::parse_program(source).expect("fixture-shaped source must parse");
    let Node::Program(program) = node else {
        panic!("parser root must be a Program");
    };
    program
}

#[test]
fn direct_extern_call_unblocked_by_contract_load() {
    let tree = TempTree::new("direct");
    tree.write("sub/print.sai", PRINT_SAI);
    let source = "@import \"sub/print.sai\"\n\nfn main() -> i32 {\n    sa_print_bytes(\"hi\", 2);\n    return 0;\n}\n";
    tree.write("main.sla", source);
    let program = parse(source);

    // Control: without the contract load the call is unresolved.
    let mut bare = Checker::new();
    let err = bare
        .check_program(&program)
        .expect_err("extern call must fail before contracts load");
    assert!(
        err.message.contains("sa_print_bytes"),
        "control diagnostic should name the call: {}",
        err.message
    );

    // With the load (plugin wiring -> register_extern_func) it passes.
    let mut checker = Checker::new();
    let report = load_imported_contracts_into(&mut checker, &program, &tree.0.join("main.sla"))
        .expect("contract load must succeed");
    assert_eq!(report.extern_names(), vec!["sa_print_bytes"]);
    assert!(checker.extern_funcs.contains_key("sa_print_bytes"));
    checker
        .check_program(&program)
        .expect("extern call must pass after contracts load");
}

#[test]
fn println_resolves_through_checker_builtin_arm() {
    let tree = TempTree::new("println");
    tree.write("sub/print.sai", PRINT_SAI);
    let source = "@import \"sub/print.sai\"\n\nfn main() -> i32 {\n    println(\"hi\");\n    return 0;\n}\n";
    tree.write("main.sla", source);
    let program = parse(source);

    let mut checker = Checker::new();
    let report = load_imported_contracts_into(&mut checker, &program, &tree.0.join("main.sla"))
        .expect("contract load itself must succeed");
    assert_eq!(report.extern_names(), vec!["sa_print_bytes"]);

    // INT2-1 closed (INT21 lane, phase 3): println is not a contract
    // extern — the registry above rightly contains only
    // sa_print_bytes — it is a checker name-special arm in Zig
    // (isPrintlnCall, any arity, unshadowable), now mirrored in
    // sla-checker's check_builtin. This test formerly pinned the
    // blocker (`Undefined call: println`, D-C18 wording); it now
    // pins the closed state: the same fully-loaded program checks
    // clean, with println resolved by the arm, not the registry.
    checker
        .check_program(&program)
        .expect("println must now check via the builtin arm (INT2-1 closed)");
}

#[test]
fn nested_contract_closure_registers_in_scan_first_order() {
    let tree = TempTree::new("nested");
    tree.write("sub/fmt.sai", "@extern sa_fmt_i64(value: i64, base: u32) -> u64\n");
    tree.write(
        "sub/print.sai",
        "@import \"fmt.sai\"\n@extern sa_print_bytes(&msg: ptr, len: u64) -> void\n",
    );
    let source = "@import \"sub/print.sai\"\n\nfn main() -> i32 {\n    sa_print_bytes(\"hi\", 2);\n    return 0;\n}\n";
    tree.write("main.sla", source);
    let program = parse(source);

    let mut checker = Checker::new();
    let report = load_imported_contracts_into(&mut checker, &program, &tree.0.join("main.sla"))
        .expect("nested contract load must succeed");
    // Zig scans nested imports before loading a file's own
    // contracts: fmt's extern lands first.
    assert_eq!(report.extern_names(), vec!["sa_fmt_i64", "sa_print_bytes"]);
    assert!(checker.extern_funcs.contains_key("sa_fmt_i64"));
    checker
        .check_program(&program)
        .expect("program must check with the full closure registered");
}

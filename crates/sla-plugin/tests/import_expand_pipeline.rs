//! Phase-4 pipeline proofs for the import/expansion surface:
//! imported macros (INT2-4) and `.sla` inline expansion
//! (include-all mode). Every expectation is pinned to a venue
//! probe against the real plugin at box head 2cc12ea8
//! (docs/probes/imp/PROBES.md):
//!   * mac_one_expr / mac_bad — imported macro arity verdicts and
//!     the exact InvalidArgsCount message shape;
//!   * inline_broken_check — an imported function whose body would
//!     fail checking passes `sa sla check` (decl-only stubs);
//!   * nest_check — a nested `.sai` extern called from an imported
//!     `.sla` body checks clean on the full (non-decl-only) path.

use sla_ast::node::Node;
use sla_checker::Checker;
use sla_plugin::contracts::load_imported_contracts_into;
use sla_plugin::sla_expand::{expand_sla_imports, SlaImportExpansionOptions};
use std::path::PathBuf;

struct TempTree(PathBuf);

impl TempTree {
    fn new(tag: &str) -> TempTree {
        let dir = std::env::temp_dir().join(format!(
            "sla_plugin_imp_{}_{}",
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

const ONE_SA: &str = "[MACRO] GET_ONE %out, %val\n    %out = copy %val\n[END_MACRO]\n";

#[test]
fn imported_macro_call_checks_after_contract_load() {
    let tree = TempTree::new("macro");
    tree.write("one.sa", ONE_SA);
    let source = "@import \"one.sa\"\nfn main() -> i32 {\n    let a = 0;\n    GET_ONE(a, 5);\n    return a;\n}\n";
    tree.write("main.sla", source);
    let program = parse(source);

    // Control: without the load the macro is unresolved.
    let mut bare = Checker::new();
    assert!(bare.check_program(&program).is_err());

    let mut checker = Checker::new();
    let report = load_imported_contracts_into(&mut checker, &program, &tree.0.join("main.sla"))
        .expect("contract load must succeed");
    assert_eq!(report.imported_macro_registry.len(), 1);
    let record = &report.imported_macro_registry[0];
    assert_eq!(record.name, "GET_ONE");
    assert_eq!(record.arity, 2);
    assert_eq!(record.leading_outputs, 1);
    assert!(checker.imported_macros.contains_key("GET_ONE"));
    checker
        .check_program(&program)
        .expect("macro call with exact arity must check after registration");
}

#[test]
fn imported_macro_wrong_arity_matches_venue_message() {
    let tree = TempTree::new("macrobad");
    tree.write("one.sa", ONE_SA);
    // Venue probe mac_bad shape: SET_PAIR(1, 2) against arity 3
    // failed with "expects 3 args or 2 args when using expression
    // output". Here: GET_ONE with 3 args against arity 2.
    let source = "@import \"one.sa\"\nfn main() -> i32 {\n    let a = 0;\n    GET_ONE(a, 5, 9);\n    return a;\n}\n";
    tree.write("main.sla", source);
    let program = parse(source);
    let mut checker = Checker::new();
    load_imported_contracts_into(&mut checker, &program, &tree.0.join("main.sla"))
        .expect("contract load must succeed");
    let err = checker
        .check_program(&program)
        .expect_err("3 args against arity 2 must fail");
    assert!(
        err.message.contains(
            "macro GET_ONE expects 2 args or 1 args when using expression output, got 3"
        ),
        "diagnostic must match the venue shape: {}",
        err.message
    );
}

#[test]
fn include_all_decl_only_expansion_then_check_matches_venue() {
    // Venue probe inline_broken_check (rc=0): the imported
    // `broken_unused` body returns a string for i32; on the check
    // path the body never reaches the checker.
    let tree = TempTree::new("broken");
    tree.write(
        "lib.sla",
        "fn helper() -> i32 { return 40; }\nfn broken_unused() -> i32 { return \"str\"; }\n",
    );
    let source = "@import \"lib.sla\"\nfn main() -> i32 { return helper() + 2; }\n";
    tree.write("main.sla", source);
    let program = parse(source);
    let expanded = expand_sla_imports(
        &program,
        &tree.0.join("main.sla"),
        &SlaImportExpansionOptions {
            imported_bodies_decl_only: true,
            include_all_imported_decls: true,
            ..Default::default()
        },
    )
    .expect("include-all expansion must succeed");
    let mut checker = Checker::new();
    checker
        .check_program(&expanded.program)
        .expect("check-path shape must pass, as on the venue");
}

#[test]
fn raw_expansion_plus_contract_load_checks_nested_extern() {
    // Venue probe nest_check / nest_build (rc=0): lib.sla calls an
    // extern from its own nested .sai import. Raw expansion keeps
    // bodies; the contract closure (which scans through .sla
    // files) registers the extern.
    let tree = TempTree::new("nest");
    tree.write("helper.sai", "@extern ext_answer() -> i32\n");
    tree.write(
        "lib.sla",
        "@import \"helper.sai\"\nfn helper() -> i32 { return ext_answer(); }\n",
    );
    let source = "@import \"lib.sla\"\nfn main() -> i32 { return helper(); }\n";
    tree.write("main.sla", source);
    let program = parse(source);
    let expanded = expand_sla_imports(
        &program,
        &tree.0.join("main.sla"),
        &SlaImportExpansionOptions {
            include_all_imported_decls: true,
            ..Default::default()
        },
    )
    .expect("raw include-all expansion must succeed");
    let mut checker = Checker::new();
    let report = load_imported_contracts_into(&mut checker, &program, &tree.0.join("main.sla"))
        .expect("contract load must succeed");
    assert_eq!(report.extern_names(), vec!["ext_answer"]);
    checker
        .check_program(&expanded.program)
        .expect("nested extern call must check with the closure registered");
}

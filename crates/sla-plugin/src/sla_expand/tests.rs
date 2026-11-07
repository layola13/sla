use super::*;
use std::path::PathBuf;

struct TempTree(PathBuf);

impl TempTree {
    fn new(tag: &str) -> TempTree {
        let dir = std::env::temp_dir().join(format!(
            "sla_expand_{}_{}",
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

fn parse(source: &str) -> Program {
    let node = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = node else {
        panic!("parser root must be a Program");
    };
    program
}

fn func_names(program: &Program) -> Vec<String> {
    program
        .decls
        .iter()
        .filter_map(|d| match d {
            Node::FuncDecl(f) => Some(f.name.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn selective_mode_is_loud_not_silent() {
    let program = parse("fn main() -> i32 { return 0; }\n");
    let err = expand_sla_imports(
        &program,
        Path::new("/nonexistent/main.sla"),
        &SlaImportExpansionOptions::default(),
    )
    .unwrap_err();
    assert!(matches!(err, SlaExpandError::SelectiveUnwired(_)));
}

#[test]
fn include_all_decl_only_matches_check_path_shape() {
    // Venue probes inline_* / nest_*: check mode sees imported
    // functions as decl-only stubs, children first, tests dropped.
    let tree = TempTree::new("checkshape");
    tree.write(
        "lib.sla",
        "fn helper() -> i32 { return 40; }\nfn unused_helper() -> i32 { return 1; }\n@test \"lib test\"() { }\n",
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
    .unwrap();
    assert_eq!(
        func_names(&expanded.program),
        vec!["helper", "unused_helper", "main"]
    );
    for decl in &expanded.program.decls {
        if let Node::FuncDecl(f) = decl {
            if f.name != "main" {
                assert!(f.is_decl_only, "{} must be decl-only", f.name);
                assert!(f.body.is_empty());
            }
        }
        assert!(!matches!(decl, Node::TestDecl(_)));
    }
    assert_eq!(expanded.primary.len(), expanded.program.decls.len());
    assert!(expanded.contract_imports.is_empty());
    assert_eq!(expanded.ordered_modules.len(), 1);
}

#[test]
fn include_all_raw_keeps_bodies_and_non_sla_root_imports() {
    let tree = TempTree::new("raw");
    tree.write("sub/print.sai", "@extern sa_print_bytes(&msg: ptr, len: u64) -> void\n");
    tree.write("lib.sla", "fn helper() -> i32 { return 40; }\n");
    let source = "@import \"lib.sla\"\n@import \"sub/print.sai\"\nfn main() -> i32 { return helper(); }\n";
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
    .unwrap();
    assert_eq!(func_names(&expanded.program), vec!["helper", "main"]);
    // The root .sai import survives as an import declaration and
    // is collected for contract loading.
    assert!(expanded.program.decls.iter().any(|d| matches!(
        d,
        Node::ImportDecl(i) if i.path.ends_with("print.sai")
    )));
    assert_eq!(expanded.contract_imports.len(), 1);
    // Raw mode keeps the imported body.
    let helper = expanded
        .program
        .decls
        .iter()
        .find_map(|d| match d {
            Node::FuncDecl(f) if f.name == "helper" => Some(f),
            _ => None,
        })
        .unwrap();
    assert!(!helper.is_decl_only);
    assert!(!helper.body.is_empty());
}

#[test]
fn bodies_mode_retains_module_contract_imports_decl_only_drops_them() {
    // Phase-6 census R-2: in bodies mode the kept module bodies
    // call the module's own .sai contracts, so the import must be
    // retained (declaration + contract_imports). In decl-only
    // mode the same import is dropped (check-path shape, pinned
    // by the venue probes in the module docs).
    let tree = TempTree::new("bodymode");
    tree.write("defs.sai", "@extern sa_fixture_ping(x: u64) -> u64\n");
    tree.write(
        "lib.sla",
        "@import \"defs.sai\"\nfn lib_answer() -> u64 { return sa_fixture_ping(40); }\n",
    );
    let source = "@import \"lib.sla\"\nfn main() -> i32 { return 0; }\n";
    tree.write("main.sla", source);
    let program = parse(source);

    let bodies = expand_sla_imports(
        &program,
        &tree.0.join("main.sla"),
        &SlaImportExpansionOptions {
            imported_bodies_decl_only: false,
            include_all_imported_decls: true,
            ..Default::default()
        },
    )
    .unwrap();
    let lib_fn = bodies
        .program
        .decls
        .iter()
        .find_map(|d| match d {
            Node::FuncDecl(f) if f.name == "lib_answer" => Some(f),
            _ => None,
        })
        .expect("module function must be present in bodies mode");
    assert!(!lib_fn.is_decl_only);
    assert!(!lib_fn.body.is_empty());
    assert!(
        bodies
            .contract_imports
            .iter()
            .any(|r| r.path.ends_with("defs.sai")),
        "bodies mode must collect the module's .sai contract import"
    );
    assert!(bodies.program.decls.iter().any(|d| matches!(
        d,
        Node::ImportDecl(i) if i.path.ends_with("defs.sai")
    )));

    let decl_only = expand_sla_imports(
        &program,
        &tree.0.join("main.sla"),
        &SlaImportExpansionOptions {
            imported_bodies_decl_only: true,
            include_all_imported_decls: true,
            ..Default::default()
        },
    )
    .unwrap();
    let stub = decl_only
        .program
        .decls
        .iter()
        .find_map(|d| match d {
            Node::FuncDecl(f) if f.name == "lib_answer" => Some(f),
            _ => None,
        })
        .expect("module function must be present as a stub in decl-only mode");
    assert!(stub.is_decl_only);
    assert!(stub.body.is_empty());
    assert!(
        !decl_only
            .contract_imports
            .iter()
            .any(|r| r.path.ends_with("defs.sai")),
        "decl-only mode must NOT retain the module's .sai contract import"
    );
}

#[test]
fn bodies_mode_falls_back_to_decl_only_for_unparseable_module_bodies() {
    // A module whose function BODIES use a parser surface the
    // Rust parser has not ported (here a `go` statement, the
    // sa_std/executor.sla shape) parses decl-only but not with
    // bodies. Bodies-mode expansion must include it decl-only
    // and RECORD the fallback instead of failing the whole
    // expansion; other modules keep their bodies.
    let tree = TempTree::new("bodyfallback");
    tree.write(
        "gapped.sla",
        "fn worker() { }\nfn lib_go() -> i32 { go worker(); return 1; }\n",
    );
    tree.write("plain.sla", "fn lib_plain() -> i32 { return 3; }\n");
    let source = "@import \"gapped.sla\"\n@import \"plain.sla\"\nfn main() -> i32 { return 0; }\n";
    tree.write("main.sla", source);
    let program = parse(source);
    let expanded = expand_sla_imports(
        &program,
        &tree.0.join("main.sla"),
        &SlaImportExpansionOptions {
            imported_bodies_decl_only: false,
            include_all_imported_decls: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(expanded.body_fallback_modules.len(), 1);
    assert!(expanded.body_fallback_modules[0].ends_with("gapped.sla"));
    let plain = expanded
        .program
        .decls
        .iter()
        .find_map(|d| match d {
            Node::FuncDecl(f) if f.name == "lib_plain" => Some(f),
            _ => None,
        })
        .expect("plain module function must be present");
    assert!(!plain.is_decl_only);
    assert!(!plain.body.is_empty());
}

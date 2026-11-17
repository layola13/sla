    use super::*;
    use std::path::PathBuf;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(tag: &str) -> TempTree {
            let dir = std::env::temp_dir().join(format!(
                "sla_cli_pipeline_{}_{}",
                tag,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempTree(dir)
        }
        fn write(&self, rel: &str, content: &str) -> String {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            path.to_string_lossy().into_owned()
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Phase-6 census R-2 minimal repro at the front-end level:
    /// a single-@test file whose body uses the std Vec surface
    /// AND calls an imported module function. At the baseline
    /// the TestCodegen front-end rejected this shape with
    /// `Undefined call: new` / `Undefined call: lib_add` (the
    /// Check kind passed it vacuously, because test bodies are
    /// not parsed there). Both kinds must now succeed.
    #[test]
    fn r2_test_frontend_accepts_std_surface_and_module_calls() {
        let tree = TempTree::new("r2repro");
        tree.write(
            "lib.sla",
            "fn lib_add(a: i32, b: i32) -> i32 { return a + b; }\n",
        );
        let main = tree.write(
            "main.sla",
            "@import \"lib.sla\"\n@test \"t\"() {\n    let v = Vec::new();\n    v.push(lib_add(1, 2));\n    if (len(v) as u64) != 1 { panic(1); };\n}\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let frontend = run_frontend(
            &main,
            FrontendKind::TestCodegen,
            None,
            &mut sink,
        );
        assert!(
            frontend.is_some(),
            "TestCodegen front-end must accept the R-2 repro; output: {}",
            String::from_utf8_lossy(&sink)
        );
        let mut sink2: Vec<u8> = Vec::new();
        let checked = run_frontend(&main, FrontendKind::Check, None, &mut sink2);
        assert!(checked.is_some());
    }

    /// The same repro through SA text codegen: the emitted test
    /// program must carry the std-surface EXPAND arms (baseline:
    /// codegen rejected every associated/method surface call as
    /// an unported family).
    #[test]
    fn r2_test_codegen_emits_std_surface_expands() {
        let tree = TempTree::new("r2codegen");
        let main = tree.write(
            "main.sla",
            "@test \"t\"() {\n    let v = Vec::new();\n    v.push(40);\n    if len(v) != 1 { panic(1); }\n}\n",
        );
        let out_path = tree.0.join("main.test.sa").to_string_lossy().into_owned();
        let mut sink: Vec<u8> = Vec::new();
        let text = compile_to_sa_string(
            &main,
            FrontendKind::TestCodegen,
            &CompileOptions::test_codegen(None),
            &out_path,
            &mut sink,
        );
        let text = text.unwrap_or_else(|| {
            panic!(
                "test codegen must succeed; output: {}",
                String::from_utf8_lossy(&sink)
            )
        });
        assert!(text.contains("EXPAND VEC_NEW"), "{text}");
        assert!(text.contains("EXPAND VEC_PUSH"), "{text}");
        assert!(text.contains("EXPAND VEC_LEN"), "{text}");
    }

    /// Module-expand line: the Check front-end runs the
    /// decl-only include-all expansion (the combination Zig's
    /// `sla check` runs). A plain function body calling an
    /// imported module function by bare name was
    /// `Undefined call: lib_add` at the baseline, because Check
    /// never expanded. (The R-2 test above could not see this:
    /// its call sits in a test body, which Check does not
    /// parse.)
    #[test]
    fn check_frontend_expands_imported_module_decls() {
        let tree = TempTree::new("checkexpand");
        tree.write(
            "lib.sla",
            "fn lib_add(a: i32, b: i32) -> i32 { return a + b; }\n",
        );
        let main = tree.write(
            "main.sla",
            "@import \"lib.sla\"\nfn use_it() -> i32 { return lib_add(1, 2); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let checked = run_frontend(&main, FrontendKind::Check, None, &mut sink);
        assert!(
            checked.is_some(),
            "Check front-end must accept a bare call into an imported module; output: {}",
            String::from_utf8_lossy(&sink)
        );
    }

    /// IMP-4: the same call through the `namespace__name`
    /// alias resolves only via the imported-signature
    /// registration — the expanded program declares `lib_add`,
    /// never `lib__lib_add`, so nothing else can resolve it.
    #[test]
    fn check_frontend_registers_imported_alias_signatures() {
        let tree = TempTree::new("checkalias");
        tree.write(
            "lib.sla",
            "fn lib_add(a: i32, b: i32) -> i32 { return a + b; }\n",
        );
        let main = tree.write(
            "main.sla",
            "@import \"lib.sla\"\nfn use_it() -> i32 { return lib__lib_add(1, 2); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let checked = run_frontend(&main, FrontendKind::Check, None, &mut sink);
        assert!(
            checked.is_some(),
            "Check front-end must resolve the namespace__name alias call; output: {}",
            String::from_utf8_lossy(&sink)
        );
    }

    /// Expansion carries imported type + impl declarations
    /// too: an associated call on a struct whose impl lives in
    /// the imported module checks on the Check path.
    #[test]
    fn check_frontend_accepts_imported_impl_assoc_call() {
        let tree = TempTree::new("checkimpl");
        tree.write(
            "lib.sla",
            "struct Thing { x: i32 }\nimpl Thing {\n    fn twice(x: i32) -> i32 { return x * 2; }\n}\n",
        );
        let main = tree.write(
            "main.sla",
            "@import \"lib.sla\"\nfn use_it() -> i32 { return Thing::twice(21); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let checked = run_frontend(&main, FrontendKind::Check, None, &mut sink);
        assert!(
            checked.is_some(),
            "Check front-end must accept an associated call into an imported impl; output: {}",
            String::from_utf8_lossy(&sink)
        );
    }


    /// A1 line (2026-11-14): the Compile front-end must run
    /// the bodies-kept include-all expansion like the other
    /// kinds. At the baseline it ran NO expansion at all, so
    /// a plain function body calling an imported module
    /// function — a bare free-function call, or an
    /// associated call of the `Instant::now()` shape —
    /// fell to `Undefined call: <name>` at check_program,
    /// and no Compile program could use the .sla std
    /// surface (the A1 cluster: 50 frontier names sharing
    /// that one signature). The Check-kind siblings above
    /// passed vacuously for these shapes; Compile was the
    /// red one. Both calls below must now be accepted.
    #[test]
    fn compile_frontend_expands_imported_module_bodies() {
        let tree = TempTree::new("a1compile");
        tree.write(
            "lib.sla",
            "struct Clock { ns: u64 }\nimpl Clock {\n    fn now() -> Clock { return Clock { ns: 7 }; }\n    fn raw_ns(self) -> u64 { return self.ns; }\n}\nfn lib_add(a: i32, b: i32) -> i32 { return a + b; }\n",
        );
        let main = tree.write(
            "main.sla",
            "@import \"lib.sla\"\nfn use_it() -> i32 { return lib_add(1, 2); }\nfn use_now() -> u64 { let c = Clock::now(); return c.raw_ns(); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let frontend = run_frontend(&main, FrontendKind::Compile, None, &mut sink);
        assert!(
            frontend.is_some(),
            "Compile front-end must accept calls into imported module bodies; output: {}",
            String::from_utf8_lossy(&sink)
        );
    }

    // -- LINE-AUTOMATCH-IMPL ------------------------------------------------
    //
    // Auto-match std imports: a name nothing defines is resolved
    // against sla_std/automatch_index.tsv; a unique provider is
    // imported through the ordinary machinery, an ambiguous one
    // is a loud error, and local definitions / explicit imports
    // always win. The mini std trees below keep the tests
    // independent of the real std surface.

    /// A one-module std tree: the index maps `mini_answer` to
    /// sla_std/wrap/mini.sla, which declares it.
    fn write_mini_std(tree: &TempTree) {
        tree.write(
            "sla_std/automatch_index.tsv",
            "# mini index\nmini_answer\tfn\tsla_std/wrap/mini.sla\n",
        );
        tree.write(
            "sla_std/wrap/mini.sla",
            "fn mini_answer() -> i32 { return 42; }\n",
        );
    }

    #[test]
    fn automatch_unique_provider_imports_without_explicit_import() {
        // Pass-after: with the index present, the call checks
        // with no @import anywhere, the note names the matched
        // name and its module, and the index file joins the
        // freshness closure.
        let tree = TempTree::new("automatch_ok");
        write_mini_std(&tree);
        let main = tree.write(
            "main.sla",
            "fn main() -> i32 { return mini_answer(); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let frontend = run_frontend(&main, FrontendKind::Check, None, &mut sink);
        let frontend = frontend.unwrap_or_else(|| {
            panic!(
                "auto-matched check must succeed; output: {}",
                String::from_utf8_lossy(&sink)
            )
        });
        let out = String::from_utf8_lossy(&sink).into_owned();
        assert!(
            out.contains("auto-matched 'mini_answer'"),
            "note must name the matched name; output: {out}"
        );
        assert!(
            out.contains("sla_std/wrap/mini.sla"),
            "note must name the providing module; output: {out}"
        );
        assert!(
            frontend
                .closure
                .iter()
                .any(|p| p.ends_with("automatch_index.tsv")),
            "index must join the freshness closure: {:?}",
            frontend.closure
        );

        // Fail-before: the identical tree WITHOUT the index
        // keeps the ordinary undefined-call failure.
        let tree2 = TempTree::new("automatch_off");
        tree2.write(
            "sla_std/wrap/mini.sla",
            "fn mini_answer() -> i32 { return 42; }\n",
        );
        let main2 = tree2.write(
            "main.sla",
            "fn main() -> i32 { return mini_answer(); }\n",
        );
        let mut sink2: Vec<u8> = Vec::new();
        let frontend2 = run_frontend(&main2, FrontendKind::Check, None, &mut sink2);
        assert!(
            frontend2.is_none(),
            "without the index the call must stay undefined"
        );
        assert!(
            String::from_utf8_lossy(&sink2).contains("Undefined call: mini_answer"),
            "baseline error must be the ordinary undefined call; output: {}",
            String::from_utf8_lossy(&sink2)
        );
    }

    #[test]
    fn automatch_test_codegen_path() {
        let tree = TempTree::new("automatch_test");
        write_mini_std(&tree);
        let main = tree.write(
            "main.sla",
            "@test \"t\"() {\n    if mini_answer() != 42 { panic(1); };\n}\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let frontend = run_frontend(&main, FrontendKind::TestCodegen, None, &mut sink);
        assert!(
            frontend.is_some(),
            "auto-match must fire on the test-codegen path too; output: {}",
            String::from_utf8_lossy(&sink)
        );
    }

    #[test]
    fn automatch_compile_kind_emits_matched_module() {
        let tree = TempTree::new("automatch_compile");
        write_mini_std(&tree);
        let main = tree.write(
            "main.sla",
            "fn main() -> i32 { return mini_answer(); }\n",
        );
        let out_path = tree.0.join("main.sa").to_string_lossy().into_owned();
        let mut sink: Vec<u8> = Vec::new();
        let text = compile_to_sa_string(
            &main,
            FrontendKind::Compile,
            &CompileOptions::defaults(),
            &out_path,
            &mut sink,
        );
        let text = text.unwrap_or_else(|| {
            panic!(
                "compile via auto-match must succeed; output: {}",
                String::from_utf8_lossy(&sink)
            )
        });
        assert!(
            text.contains("mini_answer"),
            "emitted SA must carry the matched module's function:\n{text}"
        );
    }

    #[test]
    fn automatch_ambiguous_name_fails_loudly_with_candidates() {
        let tree = TempTree::new("automatch_amb");
        tree.write(
            "sla_std/automatch_index.tsv",
            "dup_name\tfn\tsla_std/wrap/a.sla\ndup_name\tfn\tsla_std/wrap/b.sla\n",
        );
        tree.write(
            "sla_std/wrap/a.sla",
            "fn dup_name() -> i32 { return 1; }\n",
        );
        tree.write(
            "sla_std/wrap/b.sla",
            "fn dup_name() -> i32 { return 2; }\n",
        );
        let main = tree.write(
            "main.sla",
            "fn main() -> i32 { return dup_name(); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let frontend = run_frontend(&main, FrontendKind::Check, None, &mut sink);
        assert!(
            frontend.is_none(),
            "an ambiguous auto-match must not guess a module"
        );
        let out = String::from_utf8_lossy(&sink).into_owned();
        assert!(
            out.contains("AutoMatch Error") && out.contains("'dup_name'"),
            "the error must name the ambiguous name; output: {out}"
        );
        assert!(
            out.contains("sla_std/wrap/a.sla") && out.contains("sla_std/wrap/b.sla"),
            "the error must list both candidate modules; output: {out}"
        );
    }

    #[test]
    fn automatch_local_definition_wins() {
        let tree = TempTree::new("automatch_local");
        tree.write(
            "sla_std/automatch_index.tsv",
            "mini_answer\tfn\tsla_std/wrap/mini.sla\n",
        );
        // sla_std/wrap/mini.sla is deliberately ABSENT: any
        // import attempt would fail loudly, so success proves
        // the local definition won and nothing was imported.
        let main = tree.write(
            "main.sla",
            "fn mini_answer() -> i32 { return 7; }\nfn main() -> i32 { return mini_answer(); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let frontend = run_frontend(&main, FrontendKind::Check, None, &mut sink);
        assert!(
            frontend.is_some(),
            "local definition must win; output: {}",
            String::from_utf8_lossy(&sink)
        );
        assert!(
            !String::from_utf8_lossy(&sink).contains("auto-matched"),
            "no auto-match note may print when the local definition wins"
        );
    }

    #[test]
    fn automatch_explicit_import_wins() {
        let tree = TempTree::new("automatch_explicit");
        tree.write(
            "sla_std/automatch_index.tsv",
            "mini_answer\tfn\tsla_std/wrap/mini.sla\n",
        );
        // The index's module is absent from the tree; the
        // explicit import of lib.sla provides the name, so
        // success proves auto-match stayed out of the way.
        tree.write("lib.sla", "fn mini_answer() -> i32 { return 9; }\n");
        let main = tree.write(
            "main.sla",
            "@import \"lib.sla\"\nfn main() -> i32 { return mini_answer(); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let frontend = run_frontend(&main, FrontendKind::Check, None, &mut sink);
        assert!(
            frontend.is_some(),
            "explicit import must win; output: {}",
            String::from_utf8_lossy(&sink)
        );
        assert!(
            !String::from_utf8_lossy(&sink).contains("auto-matched"),
            "no auto-match note may print when an explicit import provides the name"
        );
    }

    #[test]
    fn automatch_imports_macro_module() {
        // SA macros live in the same index (kind=macro): a call
        // to an imported-macro name with no @import pulls the
        // defining .sa file into the contract load.
        let tree = TempTree::new("automatch_macro");
        tree.write(
            "sla_std/automatch_index.tsv",
            "MINI_PTR\tmacro\tsa_std/mini.sa\n",
        );
        tree.write(
            "sa_std/mini.sa",
            "[MACRO] MINI_PTR %out_ptr, %slice_reg\n    %out_ptr = copy %slice_reg\n[END_MACRO]\n",
        );
        let main = tree.write(
            "main.sla",
            "fn take(p: ptr) -> i32 { return 0; }\nfn main() -> i32 { return take(MINI_PTR(\"x\")); }\n",
        );
        let mut sink: Vec<u8> = Vec::new();
        let frontend = run_frontend(&main, FrontendKind::Check, None, &mut sink);
        assert!(
            frontend.is_some(),
            "macro auto-match must succeed; output: {}",
            String::from_utf8_lossy(&sink)
        );
        assert!(
            String::from_utf8_lossy(&sink).contains("auto-matched 'MINI_PTR'"),
            "note must name the matched macro; output: {}",
            String::from_utf8_lossy(&sink)
        );
    }

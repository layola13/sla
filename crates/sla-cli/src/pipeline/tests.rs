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

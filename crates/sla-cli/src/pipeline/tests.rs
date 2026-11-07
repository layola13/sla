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

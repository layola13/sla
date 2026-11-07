    use super::*;

    #[test]
    fn import_snippet_source_is_the_zig_lone_import_line() {
        assert_eq!(
            import_snippet_source("sa_std/io/print.sai"),
            "@import \"sa_std/io/print.sai\"\n"
        );
    }

    #[test]
    fn missing_host_is_a_loud_error_not_a_fallback() {
        let mut provider = SaHostFlattener::new("/nonexistent/sa-d4-test".to_string());
        match provider.flatten_and_decode("@x() -> void:\nL_ENTRY:\n    return\n") {
            Err(Error::UnsupportedSabDirectFeature(msg)) => {
                assert!(msg.contains("CLI-D4 sa-host flattener"), "{msg}");
                assert!(msg.contains("SAB-D31"), "{msg}");
            }
            other => panic!("expected loud host-seam error, got {other:?}"),
        }
        match provider.flatten_import_module("sa_std/io/print.sai") {
            Err(Error::UnsupportedSabDirectFeature(msg)) => {
                assert!(msg.contains("sa_std/io/print.sai"), "{msg}");
            }
            other => panic!("expected loud host-seam error, got {other:?}"),
        }
    }

    #[test]
    fn fragment_shape_reads_the_machine_snippet() {
        let source = "@import \"sa_std/vec.sa\"\n@__sla_macro_template_3() -> void:\nL_ENTRY:\n    EXPAND VEC_LEN __sla_macro_arg_0, __sla_macro_arg_1\n    return\n";
        let shape = FragmentShape::from_source(source).expect("shape");
        assert_eq!(shape.function_name, "__sla_macro_template_3");
        assert_eq!(
            shape.placeholders,
            vec!["__sla_macro_arg_0".to_string(), "__sla_macro_arg_1".to_string()]
        );
        let contract = FragmentContract::seeded(&shape);
        let header = contract.header();
        assert!(header.starts_with("#fragment 1\n#fragment-function: __sla_macro_template_3\n"), "{header}");
        assert!(header.contains("#fragment-reads: __sla_macro_arg_1\n"), "{header}");
        assert!(header.contains("#fragment-owns: __sla_macro_arg_0\n"), "{header}");
        assert!(header.contains("#fragment-returns: __sla_macro_arg_0\n"), "{header}");
        assert!(header.contains("#fragment-caller: sla-rust"), "{header}");
    }

    #[test]
    fn contract_refinement_transcribes_host_facts() {
        let shape = FragmentShape {
            function_name: "__sla_macro_fragment_0".to_string(),
            placeholders: vec![
                "__sla_macro_arg_0".to_string(),
                "__sla_macro_arg_1".to_string(),
            ],
        };
        let mut contract = FragmentContract::seeded(&shape);
        // VEC_AS_PTR shape: the expansion defines one extra
        // register, named by the host on the first round.
        assert!(contract.apply_violation(
            "undeclared_own",
            Some("__vec_view___sla_macro_arg_0")
        ));
        assert!(!contract.apply_violation(
            "undeclared_own",
            Some("__vec_view___sla_macro_arg_0")
        ));
        assert!(contract.apply_violation("unused_read", Some("__sla_macro_arg_1")));
        // Program-surface kinds are not set facts.
        assert!(!contract.apply_violation("program_entry", None));
        assert!(!contract.apply_violation("entry_not_found", Some("x")));
    }

    #[test]
    fn safrag_unwrap_reads_the_container_layout() {
        // Hand-built container per sci sab_fragment.encodeContainer:
        // magic, fmt 1.0, sab 5.0, 32-byte sha, caller, function,
        // reads [r1], owns [r0], returns [r0], payload.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(SAFRAG_MAGIC);
        bytes.extend_from_slice(&[1, 0, 5, 0]);
        bytes.extend_from_slice(&[7u8; 32]);
        let name = |bytes: &mut Vec<u8>, s: &str| {
            bytes.push(s.len() as u8);
            bytes.extend_from_slice(s.as_bytes());
        };
        name(&mut bytes, "sla-rust 0.1.0");
        name(&mut bytes, "__sla_macro_fragment_0");
        bytes.push(1);
        name(&mut bytes, "__sla_macro_arg_1");
        bytes.push(1);
        name(&mut bytes, "__sla_macro_arg_0");
        bytes.push(1);
        name(&mut bytes, "__sla_macro_arg_0");
        bytes.push(3);
        bytes.extend_from_slice(b"SAB");
        let payload = decode_safrag_payload(&bytes).expect("unwrap");
        assert_eq!(payload, b"SAB");
        // Bad magic and bad version are named, not reinterpreted.
        assert!(decode_safrag_payload(b"SAB\0xxxx").is_err());
        let mut bad = bytes.clone();
        bad[SAFRAG_MAGIC.len()] = 2;
        let err = decode_safrag_payload(&bad).unwrap_err();
        assert!(err.contains("UnsupportedFragmentVersion"), "{err}");
    }

    #[test]
    fn violation_json_parses_kind_and_register() {
        let stdout = "{\"status\":\"error\",\"error\":{\"name\":\"FragmentContractViolation\",\"kind\":\"undeclared_own\",\"register\":\"tmp_3\",\"message\":\"...\"}}\n";
        let (kind, register) = parse_fragment_violation(stdout).expect("violation");
        assert_eq!(kind, "undeclared_own");
        assert_eq!(register.as_deref(), Some("tmp_3"));
        let stdout_null = "{\"status\":\"error\",\"error\":{\"name\":\"FragmentContractViolation\",\"kind\":\"entry_not_found\",\"register\":null,\"message\":\"...\"}}\n";
        let (kind, register) = parse_fragment_violation(stdout_null).expect("violation");
        assert_eq!(kind, "entry_not_found");
        assert_eq!(register, None);
        // A header parse error is not a contract violation.
        let stdout_parse = "{\"status\":\"error\",\"error\":{\"name\":\"FragmentMissingField\",\"message\":\"...\"}}\n";
        assert!(parse_fragment_violation(stdout_parse).is_none());
    }

    /// Live-host round trip, run where a real `sa` is resolvable
    /// (the box): the print contract's lone extern must come back
    /// as a decoded import-module view. Skipped loudly in the
    /// test log — never silently — when no host exists.
    #[test]
    fn live_host_flattens_print_contract() {
        let sa = crate::delegate::resolve_sa_executable();
        if !std::path::Path::new(&sa).exists() && sa == "sa" {
            eprintln!("live_host_flattens_print_contract: no `sa` on PATH, host round trip not exercised");
            return;
        }
        let mut provider = SaHostFlattener::new(sa);
        let module = provider
            .flatten_import_module("sa_std/io/print.sai")
            .expect("print contract flattens through the host");
        assert_eq!(module.import_path, "sa_std/io/print.sai");
        assert!(
            module.function_sigs.iter().any(|s| s.name == "sa_print_bytes"),
            "extern sig present: {:?}",
            module.function_sigs.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        // Second call is served by the provider cache and agrees.
        let again = provider
            .flatten_import_module("sa_std/io/print.sai")
            .expect("cached import module");
        assert_eq!(again, module);
    }

    /// Live-host fragment round trip: a std macro template
    /// snippet of the shape the verified route refuses (an out
    /// placeholder live at the wrapper's return) must now come
    /// back as a decoded module through `sa encode-fragment`.
    /// Skipped loudly when no host exists.
    #[test]
    fn live_host_encodes_vec_len_fragment() {
        let sa = crate::delegate::resolve_sa_executable();
        if !std::path::Path::new(&sa).exists() && sa == "sa" {
            eprintln!("live_host_encodes_vec_len_fragment: no `sa` on PATH, host round trip not exercised");
            return;
        }
        let mut provider = SaHostFlattener::new(sa);
        let source = "@import \"sa_std/vec.sa\"\n@__sla_macro_template_0() -> void:\nL_ENTRY:\n    EXPAND VEC_LEN __sla_macro_arg_0, __sla_macro_arg_1\n    return\n";
        let module = provider
            .flatten_and_decode(source)
            .expect("VEC_LEN template fragment encodes through the host");
        assert!(
            module
                .function_sigs
                .iter()
                .any(|s| s.name == "__sla_macro_template_0"),
            "wrapper sig present: {:?}",
            module.function_sigs.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

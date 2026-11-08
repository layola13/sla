//! SAB-F end-to-end tests: the F-group fragment machinery
//! (decode → remap → splice → import machine) and the driver's
//! new death-point coordinates.
//!
//! Claims pinned here:
//! - with the default (un-wired) snippet flattener, a program
//!   whose preload discovers std deps now dies at driver step 2
//!   in the F group — `ensureStdDeps` → `appendStdDepsNow` →
//!   `cachedStdImportModule` → `flattenStdSnippet` (SAB-D31) —
//!   not at the old gen_call D-group stop;
//! - with a working flattener, the same program passes step 2
//!   (the deps are really appended) and — since CLI-D4 ported
//!   the gen_call `println` literal path (group D format_print,
//!   gen_print.rs) — generates end to end;
//! - the decoded-module splice hygienically renames fragment
//!   registers/labels and keeps stable names stable;
//! - the cached-template fragment path substitutes
//!   `__sla_macro_arg_{n}` placeholders with caller args and
//!   hygiene-renames fragment-internal temps.

use sla_ast::*;
use sla_codegen_sab::fragment::FragmentMachine;
use sla_codegen_sab::ir::*;
use sla_codegen_sab::state::CodegenState;
use sla_codegen_sab::surface_load::SurfaceRoots;
use sla_codegen_sab::types::StdImportModule;
use sla_codegen_sab::{
    generate_from_program_with_flattener, generate_from_program_with_roots, Error,
    StdSnippetFlattener,
};
use sla_lowering::{node_key, MapCheckInfo};

/// A flattener that serves hand-built decoded modules: import
/// modules by path, and one fixed template module for any
/// snippet source. It stands in for sci's flattener+verifier
/// exactly at the SAB-D31 boundary.
struct FakeFlattener;

fn defined_fn_module(names: &[&str]) -> StdImportModule {
    // One defined function per name: decl, entry label, return.
    let mut symbols: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    symbols.push("L_ENTRY".to_string());
    let label_id = (names.len()) as u32;
    let mut sigs = Vec::new();
    let mut insts = Vec::new();
    for (i, name) in names.iter().enumerate() {
        sigs.push(FunctionSig::new(i as u32, name, PrimType::Void, (i * 3) as u32));
        let mut decl = Instruction::new(InstKind::FuncDecl, 0, (i * 3) as u32);
        decl.operands[0] = Operand::Symbol(i as u32);
        decl.operands[1] = Operand::Func(i as u32);
        insts.push(decl);
        let mut label = Instruction::new(InstKind::Label, 0, (i * 3 + 1) as u32);
        label.operands[0] = Operand::Symbol(label_id);
        label.operands[1] = Operand::Label(label_id);
        insts.push(label);
        insts.push(Instruction::new(InstKind::Return, 0, (i * 3 + 2) as u32));
    }
    StdImportModule {
        import_path: String::new(),
        symbols,
        function_sigs: sigs,
        const_decls: Vec::new(),
        instructions: insts,
    }
}

impl StdSnippetFlattener for FakeFlattener {
    fn flatten_import_module(&mut self, import_path: &str) -> Result<StdImportModule, Error> {
        let mut module = match import_path {
            "sa_std/io/print.sai" => defined_fn_module(&["sa_print_bytes"]),
            "sa_std/fmt.sai" => defined_fn_module(&[
                "sa_fmt_i64",
                "sa_fmt_u64",
                "sa_fmt_f64",
                "sa_fmt_bool",
                "sa_fmt_buffer_data",
                "sa_fmt_buffer_len",
                "sa_fmt_buffer_free",
            ]),
            _ => {
                return Err(Error::UnsupportedSabDirectFeature(format!(
                    "fake flattener has no module for {import_path}"
                )))
            }
        };
        module.import_path = import_path.to_string();
        Ok(module)
    }

    fn flatten_and_decode(&mut self, source: &str) -> Result<Module, Error> {
        // Template module for `__sla_macro_template_0` /
        // `__sla_macro_fragment_0`: body assigns the placeholder
        // into a fragment temp, then the temp into `out`.
        assert!(
            source.contains("__sla_macro_arg_0"),
            "snippet carries the placeholder: {source}"
        );
        let func_name = if source.contains("__sla_macro_template_0") {
            "__sla_macro_template_0"
        } else {
            "__sla_macro_fragment_0"
        };
        let symbols: Vec<String> = vec![
            func_name.into(),
            "__sla_macro_arg_0".into(),
            "tmp_0".into(),
            "L_ENTRY".into(),
            "out".into(),
        ];
        let mut sig = FunctionSig::new(0, func_name, PrimType::Void, 0);
        sig.reg_ids = vec![1, 2];
        let mut decl = Instruction::new(InstKind::FuncDecl, 0, 0);
        decl.operands[0] = Operand::Symbol(0);
        decl.operands[1] = Operand::Func(0);
        let mut label = Instruction::new(InstKind::Label, 0, 1);
        label.operands[0] = Operand::Symbol(3);
        label.operands[1] = Operand::Label(3);
        let mut assign_arg = Instruction::new(InstKind::Assign, 0, 2);
        assign_arg.operands[0] = Operand::Reg(2);
        assign_arg.operands[1] = Operand::Reg(1);
        let mut assign_out = Instruction::new(InstKind::Assign, 0, 3);
        assign_out.operands[0] = Operand::Reg(4);
        assign_out.operands[1] = Operand::Reg(2);
        let ret = Instruction::new(InstKind::Return, 0, 4);
        Ok(Module {
            symbols,
            function_sigs: vec![sig],
            const_decls: Vec::new(),
            instructions: vec![decl, label, assign_arg, assign_out, ret],
        })
    }
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn func(name: &str, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        is_variadic: false,
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params: vec![],
        ret_ty: Box::new(i32_ty()),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
    })
}

fn println_program() -> Node {
    Node::Program(Program {
        decls: vec![func(
            "main",
            vec![
                Node::ExprStmt(Box::new(Node::CallExpr(CallExpr {
                    func_name: "println".to_string(),
                    associated_target: None,
                    generics: Vec::new(),
                    args: vec![Node::Literal(Literal::StringVal("hello".into()))],
                    callee: None,
                }))),
                Node::ReturnStmt(ReturnStmt {
                    value: Some(Box::new(Node::Literal(Literal::IntVal(0)))),
                }),
            ],
        )],
    })
}

fn type_all_i32(program: &Node) -> MapCheckInfo {
    fn walk(node: &Node, info: &mut MapCheckInfo) {
        info.expr_types.insert(node_key(node), i32_ty());
        match node {
            Node::Program(p) => p.decls.iter().for_each(|d| walk(d, info)),
            Node::FuncDecl(f) => f.body.iter().for_each(|s| walk(s, info)),
            Node::ExprStmt(e) => walk(e, info),
            Node::ReturnStmt(r) => {
                if let Some(v) = &r.value {
                    walk(v, info);
                }
            }
            Node::CallExpr(c) => c.args.iter().for_each(|a| walk(a, info)),
            _ => {}
        }
    }
    let mut info = MapCheckInfo::new();
    walk(program, &mut info);
    info
}

fn empty_roots() -> SurfaceRoots {
    SurfaceRoots {
        cwd: std::env::temp_dir(),
        ..Default::default()
    }
}

#[test]
fn f_println_dies_at_step2_flatten_boundary_by_default() {
    let program = println_program();
    let check = type_all_i32(&program);
    match generate_from_program_with_roots(&program, &check, empty_roots()) {
        Err(Error::UnsupportedSabDirectFeature(msg)) => {
            assert!(msg.contains("flattenStdSnippet"), "F boundary named: {msg}");
            assert!(msg.contains("group F"), "death point is group F: {msg}");
        }
        other => panic!("expected loud F-group stop at step 2, got {other:?}"),
    }
}

#[test]
fn f_println_with_flattener_passes_step2_and_generates() {
    // CLI-D4 flip: the D-group format_print arm is ported for
    // the literal path (gen_print.rs), so with a working
    // flattener the println program now generates end to end:
    // step-2 deps appended, the literal printed through
    // @sa_print_bytes call bodies, and the module encodes.
    let program = println_program();
    let check = type_all_i32(&program);
    let bytes = generate_from_program_with_flattener(
        &program,
        &check,
        empty_roots(),
        Box::new(FakeFlattener),
    )
    .expect("println literal program generates with a flattener");
    assert!(!bytes.is_empty(), "SAB bytes produced");
    let module = sla_codegen_sab::decode_module(&bytes).expect("generated SAB decodes");
    let bodies: Vec<String> = module
        .instructions
        .iter()
        .flat_map(|inst| inst.operands.iter())
        .filter_map(|op| match op {
            Operand::Text(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        bodies.iter().any(|b| b.contains("@sa_print_bytes(&SLA_STR_")),
        "print call bodies present: {bodies:?}"
    );
}

#[test]
fn f_filtered_append_hygienically_renames_fragment_locals() {
    // A decoded std module: sa_vec_new assigns an immediate into
    // its internal tmp_0 and returns it.
    let symbols: Vec<String> = vec![
        "sa_vec_new".into(),
        "data".into(),
        "tmp_0".into(),
        "L_ENTRY".into(),
    ];
    let mut sig = FunctionSig::new(0, "sa_vec_new", PrimType::I64, 0);
    sig.reg_ids = vec![2];
    let mut decl = Instruction::new(InstKind::FuncDecl, 0, 0);
    decl.operands[0] = Operand::Symbol(0);
    decl.operands[1] = Operand::Func(0);
    let mut label = Instruction::new(InstKind::Label, 0, 1);
    label.operands[0] = Operand::Symbol(3);
    label.operands[1] = Operand::Label(3);
    let mut assign = Instruction::new(InstKind::Assign, 0, 2);
    assign.operands[0] = Operand::Reg(2);
    assign.operands[1] = Operand::ImmI64(7);
    let mut ret = Instruction::new(InstKind::Return, 0, 3);
    ret.operands[0] = Operand::Reg(2);
    let module = Module {
        symbols,
        function_sigs: vec![sig],
        const_decls: Vec::new(),
        instructions: vec![decl, label, assign, ret],
    };

    let mut state = CodegenState::new();
    {
        let mut machine = FragmentMachine::new(&mut state);
        machine
            .append_decoded_module_filtered(&module, &["sa_vec_new".to_string()])
            .expect("filtered append runs");
    }
    assert!(state.program.included_imports.contains("sa_vec_new"));
    let built = state.builder.finish();
    let names: Vec<&str> = built.symbols.iter().map(|s| s.as_str()).collect();
    // The fragment's tmp_0 must NOT be interned under its own
    // name (it would alias a host tmp_0); it got a hygienic
    // __decoded_r_ name embedding the rename counter, old id
    // and old name.
    assert!(
        names.iter().any(|n| *n == "__decoded_r_1_2_tmp_0"),
        "hygienic register present: {names:?}"
    );
    assert!(!names.contains(&"tmp_0"), "no aliasing intern: {names:?}");
    // (The label was collected before the register, so the
    // rename counter gave it slot 0 and the register slot 1 —
    // Zig's operand-order collection, mirrored.)
    assert_eq!(built.function_sigs.len(), 1);
    assert_eq!(built.function_sigs[0].name, "sa_vec_new");
    // The appended sig's reg list carries the hygienic id.
    let hygienic_id = built
        .symbols
        .iter()
        .position(|n| n == "__decoded_r_1_2_tmp_0")
        .unwrap() as u32;
    assert!(built.function_sigs[0].reg_ids.contains(&hygienic_id));
    // And the whole module still encodes.
    sla_codegen_sab::generate(&built).expect("spliced module encodes");
}

#[test]
fn f_cached_template_fragment_substitutes_and_renames() {
    let mut state = CodegenState::new();
    let mut fake = FakeFlattener;
    let before = {
        let mut machine = FragmentMachine::new(&mut state);
        machine
            .emit_std_macro_fragment(
                &mut fake,
                "sa_std/vec.sa",
                "VEC_PUSH",
                &["recv".to_string()],
            )
            .expect("cached template fragment emits");
        state.builder.instructions_len()
    };
    assert_eq!(before, 2, "decl/label/return skipped, two assigns spliced");
    let built = state.builder.finish();
    let names: Vec<&str> = built.symbols.iter().map(|s| s.as_str()).collect();
    assert!(names.contains(&"recv"), "caller arg interned: {names:?}");
    assert!(
        names.iter().any(|n| *n == "__frag0_tmp_0"),
        "fragment temp hygiene-renamed: {names:?}"
    );
    // `tmp_0` does appear in the symbol table — Zig's
    // appendDecodedModuleConstDecls interns every source
    // symbol before appending consts — but no spliced
    // instruction may *reference* it: references go through
    // the hygiene rename.
    let tmp0_id = built.symbols.iter().position(|n| n == "tmp_0").unwrap() as u32;
    assert!(
        built
            .instructions
            .iter()
            .all(|inst| !inst.operands.contains(&Operand::Reg(tmp0_id))),
        "no instruction references the un-renamed tmp_0"
    );
    // First spliced assign: __frag0_tmp_0 = recv (the placeholder
    // became the caller register).
    let recv_id = built.symbols.iter().position(|n| n == "recv").unwrap() as u32;
    let tmp_id = built
        .symbols
        .iter()
        .position(|n| n == "__frag0_tmp_0")
        .unwrap() as u32;
    assert_eq!(built.instructions[0].operands[0], Operand::Reg(tmp_id));
    assert_eq!(built.instructions[0].operands[1], Operand::Reg(recv_id));
    assert_eq!(built.instructions[1].operands[1], Operand::Reg(tmp_id));
}

#[test]
fn f_flush_pending_deps_appends_through_import_machine() {
    let mut state = CodegenState::new();
    state.frame.in_function_body = true;
    let mut fake = FakeFlattener;
    {
        let mut machine = FragmentMachine::new(&mut state);
        // Inside a function body the dep queues (Zig's branch)...
        machine
            .ensure_std_deps(
                &mut fake,
                "sa_std/io/print.sai",
                &["sa_print_bytes".to_string()],
            )
            .expect("queueing ensure runs");
        assert_eq!(machine.state.program.pending_std_deps.len(), 1);
        // ...and the function-end flush appends it for real.
        machine
            .flush_pending_std_deps(&mut fake)
            .expect("flush appends");
    }
    assert!(state.program.pending_std_deps.is_empty());
    assert!(state.program.included_imports.contains("sa_print_bytes"));
    let built = state.builder.finish();
    assert_eq!(built.function_sigs.len(), 1);
    assert_eq!(built.function_sigs[0].name, "sa_print_bytes");
}

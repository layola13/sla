//! SAB-D end-to-end tests: the D-group wiring in the driver
//! (step 2 surface load + preload) and the D modules' joint
//! behaviour against fixture roots.
//!
//! Claims pinned here:
//! - step 2 really loads rules through D1's fallback chain
//!   (a fixture `std_surface.sla_meta` is found and parsed) and
//!   really preloads deps through D5/D4 (the pending queue fills
//!   in Zig's order);
//! - loading rules does not change emitted bytes for a program
//!   that uses no surface (D wiring is emission-neutral until a
//!   surface call site consumes it);
//! - a missing surface file is the SAB-D02 zero-rule outcome:
//!   generation proceeds, the walk is loudly logged;
//! - a `println` program dies at its precise dead point —
//!   since phase 4 (SAB-F) that is driver step 2's dep append
//!   hitting the sci flatten boundary in the F group (the old
//!   gen_call D-group stop is now behind a working flattener).

use std::collections::HashSet;
use std::path::PathBuf;

use sla_ast::*;
use sla_codegen_sab::surface_load::{
    load_std_surface_rules, parse_std_surface_rules, resolve_sa_std_root, SurfaceRoots,
    SA_STD_REQUIRED_FILES,
};
use sla_codegen_sab::surface_preload::SurfacePreloader;
use sla_codegen_sab::surface_query::PendingStdDeps;
use sla_codegen_sab::types::PendingStdDep;
use sla_codegen_sab::{generate_from_program_with_roots, Error};
use sla_lowering::{node_key, MapCheckInfo};

/// Verbatim excerpt of the current-head `std_surface.sla_meta`
/// (one line per kind family), the same excerpt D1's unit tests
/// pin.
const META_EXCERPT: &str = "\
# Sla std surface lowering metadata.
associated Vec new sa_std/vec.sa VEC_NEW out deps=sa_vec_new
associated Box new sa_std/core/box.sa BOX_NEW out,value consume_value
method Vec push sa_std/vec.sa VEC_PUSH receiver,value,elem_size deps=sa_vec_push,sa_mem_copy
function len Vec sa_std/vec.sa VEC_LEN out,receiver deps=sa_vec_len
fallible_method Vec remove sa_std/vec.sa VEC_REMOVE ok,out,receiver,index,elem_size deps=sa_vec_try_remove panic=86
index Vec sa_std/vec.sa VEC_GET_TYPED_{elem_ty} out,receiver,index,elem_size
index_address Vec sa_std/vec.sa VEC_GET_MUT_PTR_U64 out,receiver,index
index_assign Vec sa_std/vec.sa VEC_SET_TYPED receiver,index,value,elem_size,elem_ty
constructor Some Option sa_std/core/option.sa OPTION_NEW_SOME out,value
constructor None Option sa_std/core/option.sa OPTION_NEW_NONE out
";

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

fn ret(v: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(v)),
    })
}

fn int(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn call(name: &str, target: Option<&str>, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: target.map(|t| t.to_string()),
        generics: Vec::new(),
        args,
        callee: None,
    })
}

/// Type every node in the finished tree as i32 (same discipline
/// as driver_e2e.rs: type after building, while the tree is alive).
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

fn plain_program() -> Node {
    Node::Program(Program {
        decls: vec![func("main", vec![ret(int(42))])],
    })
}

fn println_program() -> Node {
    Node::Program(Program {
        decls: vec![func(
            "main",
            vec![
                Node::ExprStmt(Box::new(call(
                    "println",
                    None,
                    vec![Node::Literal(Literal::StringVal("hello".into()))],
                ))),
                ret(int(0)),
            ],
        )],
    })
}

/// A fixture directory holding `std_surface.sla_meta`, plus the
/// roots that find it at stage 1 (`SLA_STD_DIR`).
fn fixture_roots(tag: &str, with_meta: bool) -> (PathBuf, SurfaceRoots) {
    let dir = std::env::temp_dir().join(format!("sab-d-e2e-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    if with_meta {
        std::fs::write(dir.join("std_surface.sla_meta"), META_EXCERPT).unwrap();
    }
    let roots = SurfaceRoots {
        sla_std_dir: Some(dir.display().to_string()),
        cwd: dir.clone(),
        ..Default::default()
    };
    (dir, roots)
}

fn empty_roots(tag: &str) -> (PathBuf, SurfaceRoots) {
    fixture_roots(tag, false)
}

#[test]
fn d_step2_loads_rules_and_output_is_emission_neutral() {
    let (meta_dir, meta_roots) = fixture_roots("meta", true);
    let (empty_dir, no_roots) = empty_roots("empty");
    let program = plain_program();
    let check = type_all_i32(&program);
    let with_rules =
        generate_from_program_with_roots(&program, &check, meta_roots).expect("generates");
    let without =
        generate_from_program_with_roots(&program, &check, no_roots).expect("generates");
    assert_eq!(
        with_rules, without,
        "loaded-but-unused surface rules must not change emitted bytes"
    );
    std::fs::remove_dir_all(&meta_dir).ok();
    std::fs::remove_dir_all(&empty_dir).ok();
}

#[test]
fn d_step2_zero_rules_outcome_is_logged() {
    let (dir, roots) = empty_roots("zero");
    let mut rules = Vec::new();
    let log = load_std_surface_rules(&mut rules, &roots).expect("load runs");
    assert!(rules.is_empty(), "missing file means zero rules (SAB-D02)");
    assert!(
        log.attempts.iter().any(|a| a.contains("zero rules")),
        "the walk is loudly logged: {:?}",
        log.attempts
    );
    assert!(log.landed_path.is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn d1_loads_fixture_meta_through_chain() {
    let (dir, roots) = fixture_roots("load", true);
    let mut rules = Vec::new();
    let log = load_std_surface_rules(&mut rules, &roots).expect("load runs");
    assert_eq!(rules.len(), 10);
    assert!(
        log.landed_path.as_deref().unwrap().ends_with("std_surface.sla_meta"),
        "landed on the fixture file: {:?}",
        log.landed_path
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn d_preload_queues_println_and_associated_deps_in_order() {
    let rules = parse_std_surface_rules(META_EXCERPT).unwrap();
    let program = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![
                Node::ExprStmt(Box::new(call(
                    "println",
                    None,
                    vec![Node::Literal(Literal::StringVal("hi".into()))],
                ))),
                Node::ExprStmt(Box::new(call("new", Some("Vec"), vec![]))),
            ],
        )],
    });
    let check = type_all_i32(&program);
    let mut included: HashSet<String> = HashSet::new();
    let mut pending: Vec<PendingStdDep> = Vec::new();
    {
        let mut ensurer = PendingStdDeps::new(&mut included, &mut pending);
        let preloader = SurfacePreloader::new(&check, &rules);
        preloader
            .preload_std_surface_deps(&mut ensurer, &program)
            .expect("preload runs");
    }
    let deps: Vec<(&str, &str)> = pending
        .iter()
        .map(|p| (p.import_path.as_str(), p.dep.as_str()))
        .collect();
    assert_eq!(deps[0], ("sa_std/io/print.sai", "sa_print_bytes"));
    assert!(deps.contains(&("sa_std/fmt.sai", "sa_fmt_buffer_free")));
    assert_eq!(deps.last().unwrap(), &("sa_std/vec.sa", "sa_vec_new"));
}

#[test]
fn d_println_dies_at_gen_call_surface_expansion() {
    let (dir, roots) = fixture_roots("dead", true);
    let program = println_program();
    let check = type_all_i32(&program);
    match generate_from_program_with_roots(&program, &check, roots) {
        Err(Error::UnsupportedSabDirectFeature(msg)) => {
            // Phase 4 (SAB-F): the death point moved deeper —
            // preload's dep ensuring now appends through the
            // F-group import machine and stops at the sci
            // flatten boundary (SAB-D31), before gen_call.
            assert!(msg.contains("flattenStdSnippet"), "dead point names the F boundary: {msg}");
            assert!(msg.contains("group F"), "dead point is group F: {msg}");
        }
        other => panic!("expected loud F-group refusal at step 2, got {other:?}"),
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn d_resolve_sa_std_root_against_fixture() {
    let dir = std::env::temp_dir().join(format!("sab-d-root-{}", std::process::id()));
    let root = dir.join("sa_std");
    for rel in SA_STD_REQUIRED_FILES {
        let path = rel
            .split('/')
            .fold(root.clone(), |acc, part| acc.join(part));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "").unwrap();
    }
    let roots = SurfaceRoots {
        cwd: dir.clone(),
        ..Default::default()
    };
    let (resolved, log) = resolve_sa_std_root(&roots).expect("fixture root resolves");
    assert_eq!(resolved, root.display().to_string());
    assert_eq!(log.landed_root.as_deref(), Some(resolved.as_str()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn d_future_scan_gates_async_program_at_step3() {
    let (_dir, roots) = empty_roots("async");
    let mut program = plain_program();
    if let Node::Program(p) = &mut program {
        if let Node::FuncDecl(f) = &mut p.decls[0] {
            f.is_async = true;
        }
    }
    let check = type_all_i32(&program);
    match generate_from_program_with_roots(&program, &check, roots) {
        Err(Error::UnsupportedSabDirectFeature(msg)) => {
            assert!(msg.contains("group H"), "step-3 gate is group H: {msg}");
        }
        other => panic!("expected loud H-group refusal at step 3, got {other:?}"),
    }
    std::fs::remove_dir_all(&_dir).ok();
}

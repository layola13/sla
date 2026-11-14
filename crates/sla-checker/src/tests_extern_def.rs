//! Stage-5 (Rust SAB surface line) checker regressions.
//!
//! Three defects fixed on this line, each pinned here at the
//! checker level (the codegen/host halves are covered by the
//! shared corpus file tests/test_unit_variadic_define.sla):
//!
//! 1. `check_program` pass 3 skipped the bodies of ALL extern
//!    FuncDecls, so an `extern "C"` DEFINITION's body had no
//!    type checking and no recorded expression types at all
//!    (codegen then failed with MissingType on the first cast
//!    or binary op). A definition (non-empty body) must be
//!    checked like any other function.
//! 2. The extern return-type spelling round trip was lossy:
//!    `register_source_extern` spells types via `type_desc`
//!    tags ("raw_ptr" / "boolean" / "void_type"), but
//!    `extern_ret_type` only accepted the .sai author
//!    vocabulary ("ptr" / "bool" / "void"), silently producing
//!    Type::Infer for source-extern `-> ptr` returns.
//! 3. `block_exit_cleanups` skipped raw-pointer bindings, so
//!    a let-bound raw pointer stayed Active at block exit and
//!    the host verifier reported MemoryLeak; the exit cleanup
//!    list must include them (register retirement frees
//!    nothing for a raw pointer).

use crate::checker::{extern_ret_type, Checker};
use crate::diag::Diagnostic;
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

fn check_full(source: &str) -> Result<Checker, Diagnostic> {
    let root = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    checker.check_program(program)?;
    Ok(checker)
}

#[test]
fn extern_def_body_type_error_is_reported() {
    // Pre-fix this checked clean: the body was never visited.
    let source = "extern \"C\" fn bad(x: i32) -> i32 {\n    return true;\n}\n";
    let result = check_full(source);
    assert!(result.is_err(), "extern definition body must be type-checked");
}

#[test]
fn extern_def_body_records_expr_types() {
    // A program consisting of only an extern definition:
    // pre-fix it contributed zero recorded expression types.
    let source = "extern \"C\" fn add3(a: i32, b: i32, c: i32) -> i64 {\n    return a as i64 + b as i64 + c as i64;\n}\n";
    let checker = check_full(source).expect("valid extern definition must check");
    assert!(
        !checker.expr_types.is_empty(),
        "extern definition body must record expression types"
    );
}

#[test]
fn extern_ret_spelling_accepts_type_desc_vocabulary() {
    assert_eq!(
        extern_ret_type("raw_ptr"),
        Type::Primitive(Primitive::RawPtr)
    );
    assert_eq!(
        extern_ret_type("boolean"),
        Type::Primitive(Primitive::Boolean)
    );
    assert_eq!(
        extern_ret_type("void_type"),
        Type::Primitive(Primitive::VoidType)
    );
    // The .sai author vocabulary keeps working.
    assert_eq!(extern_ret_type("ptr"), Type::Primitive(Primitive::RawPtr));
    assert_eq!(extern_ret_type("bool"), Type::Primitive(Primitive::Boolean));
    assert_eq!(extern_ret_type("void"), Type::Primitive(Primitive::VoidType));
}

#[test]
fn source_extern_ptr_call_type_is_not_infer() {
    // The unsafe block's value type is the call's return type;
    // pre-fix it was Infer for a source-extern `-> ptr`.
    let source = "extern \"C\" fn idptr(tag: i32, p: ptr) -> ptr {\n    return p;\n}\nfn probe(cell: ptr) -> ptr {\n    let got = unsafe { idptr(0, cell) };\n    return got;\n}\n";
    let checker = check_full(source).expect("probe program must check");
    let has_rawptr = checker
        .expr_types
        .values()
        .any(|ty| matches!(ty, Type::Primitive(Primitive::RawPtr)));
    assert!(has_rawptr, "call/unsafe types must include RawPtr");
    let has_infer = checker
        .expr_types
        .values()
        .any(|ty| matches!(ty, Type::Infer));
    assert!(!has_infer, "no recorded type may be Infer here");
}

#[test]
fn block_exit_cleanups_include_rawptr_bindings() {
    let source = "fn idptr2(tag: i32, p: ptr) -> ptr {\n    return p;\n}\nfn probe(cell: ptr) {\n    let got = idptr2(0, cell);\n}\n";
    let checker = check_full(source).expect("probe program must check");
    let names: Vec<&String> = checker.cleanups.values().flatten().collect();
    assert!(
        names.iter().any(|n| n.as_str() == "got"),
        "block-exit cleanups must retire the raw-pointer binding `got`: {names:?}"
    );
}

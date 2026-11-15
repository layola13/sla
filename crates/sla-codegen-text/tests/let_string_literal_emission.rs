//! Regression (RUST-TEXT-EMITTER stage 5a resume, R1):
//! `let x: String = "literal"` must materialize an owned String
//! handle instead of renaming the stack-allocated Slice view into
//! the binding (host `error[StackEscape]` at the rename, e.g.
//! `test_unit_reg1_dns_client_a` `fx = tmp_242`).
//!
//! Pre-fix, `generate` returns the rename form with no owned-string
//! materialization; post-fix it emits
//! `STRING_BUF_FROM_UTF8_UNCHECKED` (Zig `genOwnedStringLiteral`,
//! codegen.zig:14295, via the genLetStmt arms :14940/:14964/:15009).
//! The un-annotated form (`let doc = "lit"`, RawPtr type) keeps its
//! existing `add tmp, 0` copy and is covered by the second test.

use sla_ast::{FuncDecl, LetStmt, Literal, Node, Primitive, Program, ReturnStmt, Type};
use sla_lowering::MapCheckInfo;

fn string_ty() -> Type {
    Type::UserDefined(sla_ast::UserDefined {
        name: "String".to_string(),
        generics: Vec::new(),
    })
}

fn raw_ptr_ty() -> Type {
    Type::Primitive(Primitive::RawPtr)
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

fn func_with_let(name: &str, ty: Option<Type>) -> Node {
    Node::Program(Program {
        decls: vec![Node::FuncDecl(FuncDecl {
            name: "f".to_string(),
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: Vec::new(),
            params: Vec::new(),
            ret_ty: Box::new(void_ty()),
            body: vec![
                Node::LetStmt(LetStmt {
                    name: name.to_string(),
                    ty: ty.map(Box::new),
                    value: Box::new(Node::Literal(Literal::StringVal("hi".to_string()))),
                }),
                Node::ReturnStmt(ReturnStmt { value: None }),
            ],
            is_inline: false,
            is_async: false,
            operator: None,
            is_arena: false,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
            is_variadic: false,
        })],
    })
}

#[test]
fn string_typed_let_of_literal_emits_owned_string() {
    let program = func_with_let("x", Some(string_ty()));
    let mut check = MapCheckInfo::new();
    if let Node::Program(prog) = &program {
        if let Node::FuncDecl(f) = &prog.decls[0] {
            if let Node::LetStmt(l) = &f.body[0] {
                check.add_expr_type(&l.value, raw_ptr_ty());
            }
        }
    }
    let out = sla_codegen_text::generate(&program, &check)
        .expect("String-typed let of a literal must emit on the text backend");
    assert!(
        out.contains("STRING_BUF_FROM_UTF8_UNCHECKED"),
        "expected owned-String materialization, got:\n{out}"
    );
    // The owned handle (not a stack Slice view) is what lands in `x`:
    // either the plain rename or the slot store of the owned tmp.
    let owned = out
        .lines()
        .find_map(|l| {
            let t = l.trim();
            t.strip_prefix("EXPAND STRING_BUF_FROM_UTF8_UNCHECKED ")
                .and_then(|rest| rest.split([',', ' ']).next())
                .map(str::to_string)
        })
        .expect("owned-String tmp must be named in the emission");
    assert!(
        out.contains(&format!("x = {owned}"))
            || out.contains(&format!("store x+0, {owned} as String")),
        "expected the owned handle {owned} to land in `x`, got:\n{out}"
    );
}

#[test]
fn unannotated_let_of_literal_keeps_existing_copy_form() {
    // No annotation: the literal's type is RawPtr, so the generic
    // path keeps its `add tmp, 0` copy — the R1 arm must not fire.
    let program = func_with_let("doc", None);
    let mut check = MapCheckInfo::new();
    if let Node::Program(prog) = &program {
        if let Node::FuncDecl(f) = &prog.decls[0] {
            if let Node::LetStmt(l) = &f.body[0] {
                check.add_expr_type(&l.value, raw_ptr_ty());
            }
        }
    }
    let out = sla_codegen_text::generate(&program, &check)
        .expect("un-annotated let of a literal must emit on the text backend");
    assert!(
        !out.contains("STRING_BUF_FROM_UTF8_UNCHECKED"),
        "un-annotated literal let must not take the owned-String arm, got:\n{out}"
    );
    assert!(
        out.contains("doc = add "),
        "expected the existing copy form for the un-annotated let, got:\n{out}"
    );
}

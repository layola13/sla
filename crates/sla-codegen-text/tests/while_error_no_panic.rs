//! Regression (RUST-TEXT-EMITTER-1): an erroring while body must
//! surface its real emission error, never a codegen panic.
//!
//! `gen_while_stmt` pushed a loop-body local scope and, on the
//! pre-fix code, returned a body error with `?` before popping
//! it. The leaked scope entry (block depth 0) was then
//! decremented by the enclosing block's leave-tracking —
//! `attempt to subtract with overflow` at block.rs:191 — which
//! masked the real error on 14 corpus files. The fix pops the
//! scope on every path and emits the back-edge cleanups while
//! the scope is still live (Zig's `defer` discipline).
//!
//! On the pre-fix code this test panics; on the fixed code
//! `generate` returns the body's error.

use sla_ast::{
    FuncDecl, InlineAsmExpr, LetStmt, Literal, Node, Primitive, Program, Type, WhileStmt,
};
use sla_lowering::MapCheckInfo;

#[test]
fn erroring_while_body_returns_error_not_panic() {
    let cond = Node::Literal(Literal::BoolVal(true));
    let asm_expr = Node::InlineAsmExpr(InlineAsmExpr {
        template: "nop".to_string(),
        operands: Vec::new(),
    });
    let body = vec![Node::WhileStmt(WhileStmt {
        cond: Box::new(cond),
        let_pattern: None,
        body: vec![Node::LetStmt(LetStmt {
            name: "x".to_string(),
            ty: None,
            value: Box::new(asm_expr),
        })],
    })];
    let program = Node::Program(Program {
        decls: vec![Node::FuncDecl(FuncDecl {
            name: "f".to_string(),
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: Vec::new(),
            params: Vec::new(),
            ret_ty: Box::new(Type::Primitive(Primitive::VoidType)),
            body,
            is_inline: false,
            is_async: false,
            operator: None,
            is_arena: false,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
            is_variadic: false,
        })],
    });

    // Checker facts, registered on the in-place nodes (the
    // MapCheckInfo keys facts by node identity): the condition
    // is boolean; the let's value has a (placeholder scalar)
    // type so gen_let_stmt walks its arms and reaches the
    // inline-asm arm, which is the unsupported shape that
    // errors INSIDE the while body block. (Closure literals used
    // to play this role, but inline closure expansion now supports
    // them, so the test uses inline asm instead.)
    let mut check = MapCheckInfo::new();
    if let Node::Program(prog) = &program {
        if let Node::FuncDecl(f) = &prog.decls[0] {
            if let Node::WhileStmt(w) = &f.body[0] {
                check.add_expr_type(&w.cond, Type::Primitive(Primitive::Boolean));
                if let Node::LetStmt(l) = &w.body[0] {
                    check.add_expr_type(&l.value, Type::Primitive(Primitive::I32));
                }
            }
        }
    }

    let result = sla_codegen_text::generate(&program, &check);
    match &result {
        Err(sla_codegen_text::TextError::Unsupported { feature, .. }) => {
            assert!(
                feature.contains("inline_asm"),
                "expected the inline-asm error from the while body, got: {feature}"
            );
        }
        other => panic!("expected the body's loud Unsupported error, got: {other:?}"),
    }
}

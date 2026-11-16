//! Stage 10 regression tests for RUST-S9-BARE-CLONE-CALL.
//!
//! A `thread::spawn` worker body is generated from the spawn
//! helper's CLONED closure AST (`thread_spawn.rs` stores
//! `closure.clone()`), so every node-address-keyed checker fact
//! inside the body misses — including the receiver type of
//! `barrier.clone()`. The stage-9 surface arms classified the
//! receiver purely through the checker fact, declined, and the
//! call fell to the bare-name EXPAND catch-all, synthesising
//! `EXPAND clone tmp, recv` for a macro that exists in no std
//! tree (UndefinedMacro at assembly). The worker seeds each
//! capture as a typed local binding; the receiver classification
//! now falls back to that binding type for identifier receivers.
//!
//! These tests simulate the fact loss directly: the receiver
//! identifier gets NO checker expr-type registration, only the
//! parameter prologue's binding type (exactly the worker's
//! situation). On the base emitter both tests fail with the
//! bare-name EXPAND.

use sla_ast::*;
use sla_lowering::MapCheckInfo;

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
}

fn arc_ty(inner: Type) -> Type {
    ud("Arc", vec![inner])
}

fn rc_ty(inner: Type) -> Type {
    ud("Rc", vec![inner])
}

fn param(name: &str, ty: Type) -> Param {
    Param {
        name: name.to_string(),
        ty: Box::new(ty),
        is_borrow: false,
        is_move: false,
        fallible_sugar: false,
        is_var: false,
    }
}

fn func(name: &str, params: Vec<Param>, ret_ty: Type, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params,
        ret_ty: Box::new(ret_ty),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_variadic: false,
    })
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn call(func_name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: func_name.to_string(),
        associated_target: None,
        generics: Vec::new(),
        args,
        callee: None,
    })
}

fn ret(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(value)),
    })
}

/// fn f(a: Arc<i64>) -> Arc<i64> { return a.clone(); }
/// with NO checker fact on the receiver identifier — the
/// worker-body situation.
fn factless_clone_program(handle_ty: Type) -> (Node, Node) {
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("a", handle_ty.clone())],
            handle_ty,
            vec![ret(call("clone", vec![ident("a")]))],
        )],
    });
    let Node::Program(p) = &program else {
        panic!("root must be a Program node");
    };
    let Node::FuncDecl(f) = &p.decls[0] else {
        panic!("decl 0 must be a function");
    };
    let Node::ReturnStmt(r) = &f.body[0] else {
        panic!("body 0 must be a return statement");
    };
    let value = r.value.as_deref().expect("return must carry a value").clone();
    (program, value)
}

#[test]
fn arc_clone_without_receiver_fact_uses_binding_type() {
    let (program, value) = factless_clone_program(arc_ty(i64_ty()));
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("clone on a typed binding must emit without a receiver fact");
    let _ = value;
    assert!(
        out.contains("EXPAND ARC_CLONE "),
        "expected ARC_CLONE via the binding-type fallback, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND clone "),
        "must not emit the bare-name EXPAND catch-all, got:\n{out}"
    );
}

#[test]
fn rc_clone_without_receiver_fact_uses_binding_type() {
    let (program, _value) = factless_clone_program(rc_ty(i64_ty()));
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("clone on a typed binding must emit without a receiver fact");
    assert!(
        out.contains("EXPAND RC_CLONE "),
        "expected RC_CLONE via the binding-type fallback, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND clone "),
        "must not emit the bare-name EXPAND catch-all, got:\n{out}"
    );
}

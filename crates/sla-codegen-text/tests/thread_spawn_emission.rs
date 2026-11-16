//! Regression (RUST-TEXT-EMITTER stage 8, RUST-S7-SPAWN-ASSOC):
//! `thread::spawn(^|| body)` must lower through the spawn
//! helper machinery (codegen.zig `threadSpawnHelperForExpr` /
//! `emitThreadSpawnHelpers` / the `isThreadSpawnCall` call-site
//! arm): a slot block in the caller (handle at +0, result at
//! +8, captures from +16), a per-site `@ffi_wrapper
//! sla_thread_spawn_{idx}` running `EXPAND THREAD_SPAWN`, and a
//! `@sla_thread_worker_{idx}` that loads the captures out of the
//! slot and writes the body's value back to `slot+8`. Any
//! registered helper also pulls `@import "sa_std/thread.sa"`.
//!
//! Pre-fix, `generate` rejected every spawn with
//! `unsupported emission associated-target call
//! target=Some("thread") func=spawn`; the non-literal-argument
//! control stays a loud error both pre- and post-fix.

use sla_ast::{
    CallExpr, ClosureLiteral, FuncDecl, LetStmt, Literal, MoveExpr, Node, Param, Primitive,
    Program, ReturnStmt, Type, UserDefined,
};
use sla_lowering::MapCheckInfo;

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

fn join_handle_ty(inner: Type) -> Type {
    Type::UserDefined(UserDefined {
        name: "JoinHandle".to_string(),
        generics: vec![inner],
    })
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

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
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

fn spawn_call(arg: Node) -> Node {
    Node::CallExpr(CallExpr {
        func_name: "spawn".to_string(),
        associated_target: Some("thread".to_string()),
        generics: Vec::new(),
        args: vec![arg],
        callee: None,
    })
}

fn closure(body: Node) -> Node {
    Node::MoveExpr(MoveExpr {
        expr: Box::new(Node::ClosureLiteral(ClosureLiteral {
            params: Vec::new(),
            body: Box::new(body),
        })),
    })
}

fn let_stmt(name: &str, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: None,
        value: Box::new(value),
    })
}

fn ret(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(value)),
    })
}

/// fn work(x: i64) -> i32 { return 7; }
fn work_decl() -> Node {
    func("work", vec![param("x", i64_ty())], i32_ty(), vec![ret(int_lit(7))])
}

/// Navigate to decl `idx`'s body statement `stmt` (test programs
/// are built so the interesting nodes sit at known positions).
fn stmt_of<'p>(program: &'p Node, idx: usize, stmt: usize) -> &'p Node {
    let Node::Program(p) = program else {
        panic!("root must be a Program node");
    };
    let Node::FuncDecl(f) = &p.decls[idx] else {
        panic!("decl {idx} must be a function");
    };
    &f.body[stmt]
}

/// The spawn call node inside `let h = thread::spawn(...)`.
fn spawn_call_of<'p>(program: &'p Node, idx: usize, stmt: usize) -> &'p Node {
    let Node::LetStmt(l) = stmt_of(program, idx, stmt) else {
        panic!("statement must be a let");
    };
    &l.value
}

/// The closure body inside the spawn call at (idx, stmt).
fn closure_body_of<'p>(program: &'p Node, idx: usize, stmt: usize) -> &'p Node {
    let Node::CallExpr(c) = spawn_call_of(program, idx, stmt) else {
        panic!("let value must be the spawn call");
    };
    let Node::MoveExpr(m) = &c.args[0] else {
        panic!("spawn argument must be a move-wrapped closure");
    };
    let Node::ClosureLiteral(lit) = &*m.expr else {
        panic!("spawn argument must be a closure literal");
    };
    &lit.body
}

#[test]
fn spawn_with_param_capture_emits_helper_pair() {
    // fn f(cap: i64) -> i32 {
    //   let h = thread::spawn(^|| work(cap));
    //   return 0;
    // }
    let program = Node::Program(Program {
        decls: vec![
            work_decl(),
            func(
                "f",
                vec![param("cap", i64_ty())],
                i32_ty(),
                vec![
                    let_stmt("h", spawn_call(closure(call("work", vec![ident("cap")])))),
                    ret(int_lit(0)),
                ],
            ),
        ],
    });
    let mut check = MapCheckInfo::new();
    check.add_expr_type(spawn_call_of(&program, 1, 0), join_handle_ty(i32_ty()));
    let body = closure_body_of(&program, 1, 0);
    check.add_expr_type(body, i32_ty());
    let Node::CallExpr(body_call) = body else {
        panic!("closure body must be a call");
    };
    check.add_expr_type(&body_call.args[0], i64_ty());

    let out = sla_codegen_text::generate(&program, &check)
        .expect("thread::spawn must emit on the text backend");
    assert!(
        out.contains("@import \"sa_std/thread.sa\""),
        "a registered spawn helper must import sa_std/thread.sa, got:\n{out}"
    );
    assert!(
        out.contains("@const SLA_THREAD_VT_0 = vtable { call = @sla_thread_worker_0 }"),
        "expected the worker vtable, got:\n{out}"
    );
    assert!(
        out.contains("@ffi_wrapper sla_thread_spawn_0(*slot: ptr):"),
        "expected the spawn ffi wrapper, got:\n{out}"
    );
    assert!(
        out.contains("EXPAND THREAD_SPAWN handle, *worker_safe, *slot"),
        "expected the THREAD_SPAWN expansion, got:\n{out}"
    );
    assert!(
        out.contains("@sla_thread_worker_0(&slot: ptr) -> i32:"),
        "expected the worker function, got:\n{out}"
    );
    // One capture: slot = 16 header bytes + 8 for the capture.
    assert!(
        out.contains("= alloc 24"),
        "expected a 24-byte spawn slot, got:\n{out}"
    );
    assert!(
        out.contains("+16, cap as ptr"),
        "expected the capture store at slot+16, got:\n{out}"
    );
    assert!(
        out.contains("= load slot+16 as ptr"),
        "expected the worker capture load at slot+16, got:\n{out}"
    );
    assert!(
        out.contains("call @sla_thread_spawn_0(*"),
        "expected the call site to invoke the spawn helper, got:\n{out}"
    );
    assert!(
        out.contains("store slot+8, "),
        "expected the worker to write its result to slot+8, got:\n{out}"
    );
}

#[test]
fn spawn_with_let_capture_emits_helper_pair() {
    // fn f() -> i32 {
    //   let cap = 41;
    //   let h = thread::spawn(^|| work(cap));
    //   return 0;
    // }
    let program = Node::Program(Program {
        decls: vec![
            work_decl(),
            func(
                "f",
                Vec::new(),
                i32_ty(),
                vec![
                    let_stmt("cap", int_lit(41)),
                    let_stmt("h", spawn_call(closure(call("work", vec![ident("cap")])))),
                    ret(int_lit(0)),
                ],
            ),
        ],
    });
    let mut check = MapCheckInfo::new();
    check.add_expr_type(spawn_call_of(&program, 1, 1), join_handle_ty(i32_ty()));
    let body = closure_body_of(&program, 1, 1);
    check.add_expr_type(body, i32_ty());
    let Node::CallExpr(body_call) = body else {
        panic!("closure body must be a call");
    };
    check.add_expr_type(&body_call.args[0], i64_ty());
    let Node::LetStmt(cap_let) = stmt_of(&program, 1, 0) else {
        panic!("statement 0 must be the cap let");
    };
    check.add_expr_type(&cap_let.value, i64_ty());

    let out = sla_codegen_text::generate(&program, &check)
        .expect("thread::spawn over a let capture must emit on the text backend");
    assert!(
        out.contains("@ffi_wrapper sla_thread_spawn_0(*slot: ptr):"),
        "expected the spawn ffi wrapper, got:\n{out}"
    );
    assert!(
        out.contains("+16, cap as ptr"),
        "expected the let capture store at slot+16, got:\n{out}"
    );
    assert!(
        out.contains("= load slot+16 as ptr"),
        "expected the worker capture load at slot+16, got:\n{out}"
    );
}

#[test]
fn spawn_void_worker_returns_zero_without_result_store() {
    // fn ping() -> void {}
    // fn g() -> i32 {
    //   let h = thread::spawn(^|| ping());
    //   return 0;
    // }
    let program = Node::Program(Program {
        decls: vec![
            func("ping", Vec::new(), void_ty(), Vec::new()),
            func(
                "g",
                Vec::new(),
                i32_ty(),
                vec![
                    let_stmt("h", spawn_call(closure(call("ping", Vec::new())))),
                    ret(int_lit(0)),
                ],
            ),
        ],
    });
    let mut check = MapCheckInfo::new();
    check.add_expr_type(spawn_call_of(&program, 1, 0), join_handle_ty(void_ty()));
    check.add_expr_type(closure_body_of(&program, 1, 0), void_ty());

    let out = sla_codegen_text::generate(&program, &check)
        .expect("a void thread::spawn must emit on the text backend");
    assert!(
        out.contains("@sla_thread_worker_0(&slot: ptr) -> i32:"),
        "expected the worker function, got:\n{out}"
    );
    // No captures: the slot is exactly the 16-byte header.
    assert!(
        out.contains("= alloc 16"),
        "expected a 16-byte spawn slot, got:\n{out}"
    );
    let worker_pos = out
        .find("@sla_thread_worker_0(&slot: ptr) -> i32:")
        .expect("worker present");
    let worker = &out[worker_pos..];
    assert!(
        worker.contains("return 0"),
        "a void worker must return 0, got:\n{out}"
    );
    assert!(
        !worker.contains("store slot+8, tmp"),
        "a void worker must not store a result value, got:\n{out}"
    );
}

#[test]
fn spawn_with_non_literal_argument_stays_loud() {
    // fn f(h2: ptr) -> i32 {
    //   let h = thread::spawn(h2);
    //   return 0;
    // }
    // No closure literal: no helper is registered and generation
    // must fail loudly (control: errors pre- and post-fix).
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("h2", Type::Primitive(Primitive::RawPtr))],
            i32_ty(),
            vec![
                let_stmt("h", spawn_call(ident("h2"))),
                ret(int_lit(0)),
            ],
        )],
    });
    let mut check = MapCheckInfo::new();
    check.add_expr_type(spawn_call_of(&program, 0, 0), join_handle_ty(i32_ty()));
    let result = sla_codegen_text::generate(&program, &check);
    assert!(
        result.is_err(),
        "thread::spawn with a non-literal closure must stay a loud error, got:\n{:?}",
        result.map(|_| "<ok>")
    );
}

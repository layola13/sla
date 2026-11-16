//! Regression (RUST-TEXT-EMITTER stage 9, RUST-S7-EXPAND-MACRO-
//! FAMILY): the receiver methods below used to fall through to
//! the bare-name EXPAND catch-all in call.rs, synthesising
//! `EXPAND clone/strong_count/fetch_add/lock/join/RESULT_UNWRAP`
//! for macros that exist in no std tree (grep-verified in the
//! pinned sci, current sci main, and sla_std). Each arm now
//! mirrors the Zig TEXT emission (R122 B_mirror_zig):
//!
//! * `Rc`/`Arc::clone` -> copy + in-place `EXPAND RC_CLONE` /
//!   `EXPAND ARC_CLONE` (codegen.zig:21657-21686; rc.sa:20).
//! * `Rc`/`Arc::strong_count`/`weak_count` -> fresh-src copy +
//!   `EXPAND RC_STRONG_COUNT` / `ARC_STRONG_COUNT` / `*_WEAK_*`
//!   (codegen.zig:21687-21726, DEF-ARC-DROP F1).
//! * `AtomicI32`/`AtomicUsize::load`/`fetch_add` (+ `AtomicPtr`
//!   load) -> `EXPAND ATOMIC_*_LOAD` / `ATOMIC_*_FETCH_ADD`
//!   with the ordering lowered to its lowercase token
//!   (codegen.zig:21406-21500; atomicOrderingToken 6686).
//! * `Mutex::lock` -> statement `EXPAND MUTEX_LOCK`, guard =
//!   `ptr_add recv, Mutex_data`, wrapped in `RESULT_NEW_OK`
//!   (codegen.zig:21569-21590).
//! * `JoinHandle::join` -> inline branch code, pthread arm via
//!   `EXPAND THREAD_JOIN_STATUS` + `EXPAND THREAD_DROP`,
//!   24-byte Result in both arms (codegen.zig:21030-21180).
//! * `Result::unwrap` -> inline branch code (`L_RESULT_UNWRAP_*`,
//!   `panic(17)` on Err); `RESULT_UNWRAP` has no [MACRO]
//!   definition anywhere (result.sa defines only
//!   `RESULT_UNWRAP_OR*`) (codegen.zig:21340-21380).
//!
//! Pre-fix, every test below fails: the emission contains the
//! bare-name EXPAND (or, for `Result::unwrap`, the bogus
//! `EXPAND RESULT_UNWRAP`) instead of the Zig form. The
//! `Option::unwrap` control keeps its real macro
//! (`OPTION_UNWRAP`, option.sa:489) and passes pre- and
//! post-fix.

use sla_ast::{
    CallExpr, FuncDecl, LetStmt, Literal, Node, Param, Primitive, Program, ReturnStmt, Type,
    UserDefined,
};
use sla_lowering::MapCheckInfo;

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn u64_ty() -> Type {
    Type::Primitive(Primitive::U64)
}

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

fn rc_ty(inner: Type) -> Type {
    ud("Rc", vec![inner])
}

fn arc_ty(inner: Type) -> Type {
    ud("Arc", vec![inner])
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

/// The return statement's value node of decl `idx`.
fn return_value_of<'p>(program: &'p Node, idx: usize) -> &'p Node {
    let Node::Program(p) = program else {
        panic!("root must be a Program node");
    };
    let Node::FuncDecl(f) = &p.decls[idx] else {
        panic!("decl {idx} must be a function");
    };
    let Node::ReturnStmt(r) = &f.body[0] else {
        panic!("body 0 must be a return statement");
    };
    r.value.as_deref().expect("return must carry a value")
}

/// fn f(a: Rc<i64>) -> Rc<i64> { return a.clone(); }
/// (and the Arc twin).
fn clone_program(handle_ty: Type) -> Node {
    Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("a", handle_ty.clone())],
            handle_ty,
            vec![ret(call("clone", vec![ident("a")]))],
        )],
    })
}

#[test]
fn rc_clone_emits_rc_clone() {
    let program = clone_program(rc_ty(i64_ty()));
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], rc_ty(i64_ty()));
    check.add_expr_type(value, rc_ty(i64_ty()));
    let out = sla_codegen_text::generate(&program, &check)
        .expect("Rc::clone must emit on the text backend");
    assert!(
        out.contains("EXPAND RC_CLONE "),
        "expected the in-place RC_CLONE projection, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND clone "),
        "an Rc clone must not emit the bare-name EXPAND, got:\n{out}"
    );
}

#[test]
fn arc_clone_emits_arc_clone() {
    let program = clone_program(arc_ty(i64_ty()));
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], arc_ty(i64_ty()));
    check.add_expr_type(value, arc_ty(i64_ty()));
    let out = sla_codegen_text::generate(&program, &check)
        .expect("Arc::clone must emit on the text backend");
    assert!(
        out.contains("EXPAND ARC_CLONE "),
        "expected the in-place ARC_CLONE projection, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND clone "),
        "an Arc clone must not emit the bare-name EXPAND, got:\n{out}"
    );
}

#[test]
fn rc_strong_count_emits_rc_strong_count() {
    // fn f(a: Rc<i64>) -> u64 { return a.strong_count(); }
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("a", rc_ty(i64_ty()))],
            u64_ty(),
            vec![ret(call("strong_count", vec![ident("a")]))],
        )],
    });
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], rc_ty(i64_ty()));
    check.add_expr_type(value, u64_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("Rc::strong_count must emit on the text backend");
    assert!(
        out.contains("EXPAND RC_STRONG_COUNT "),
        "expected the RC_STRONG_COUNT projection, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND strong_count "),
        "a strong_count must not emit the bare-name EXPAND, got:\n{out}"
    );
}

#[test]
fn arc_weak_count_emits_arc_weak_count() {
    // fn f(a: Arc<i64>) -> u64 { return a.weak_count(); }
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("a", arc_ty(i64_ty()))],
            u64_ty(),
            vec![ret(call("weak_count", vec![ident("a")]))],
        )],
    });
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], arc_ty(i64_ty()));
    check.add_expr_type(value, u64_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("Arc::weak_count must emit on the text backend");
    assert!(
        out.contains("EXPAND ARC_WEAK_COUNT "),
        "expected the ARC_WEAK_COUNT projection, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND weak_count "),
        "a weak_count must not emit the bare-name EXPAND, got:\n{out}"
    );
}

#[test]
fn atomic_usize_fetch_add_emits_atomic_usize_fetch_add() {
    // fn f(c: AtomicUsize) -> u64 {
    //     return c.fetch_add(1, Ordering::SeqCst);
    // }
    let atomic_ty = ud("AtomicUsize", Vec::new());
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("c", atomic_ty.clone())],
            u64_ty(),
            vec![ret(call(
                "fetch_add",
                vec![ident("c"), int_lit(1), ident("Ordering::SeqCst")],
            ))],
        )],
    });
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], atomic_ty);
    check.add_expr_type(value, u64_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("AtomicUsize::fetch_add must emit on the text backend");
    assert!(
        out.contains("EXPAND ATOMIC_USIZE_FETCH_ADD "),
        "expected the ATOMIC_USIZE_FETCH_ADD projection, got:\n{out}"
    );
    assert!(
        out.contains("seq_cst"),
        "the ordering must lower to its lowercase token, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND fetch_add "),
        "a fetch_add must not emit the bare-name EXPAND, got:\n{out}"
    );
    assert!(
        !out.contains("Ordering::SeqCst"),
        "the ordering identifier must not leak through verbatim, got:\n{out}"
    );
}

#[test]
fn atomic_i32_load_emits_atomic_i32_load() {
    // fn f(c: AtomicI32) -> i32 { return c.load(Ordering::Acquire); }
    let atomic_ty = ud("AtomicI32", Vec::new());
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("c", atomic_ty.clone())],
            i32_ty(),
            vec![ret(call(
                "load",
                vec![ident("c"), ident("Ordering::Acquire")],
            ))],
        )],
    });
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], atomic_ty);
    check.add_expr_type(value, i32_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("AtomicI32::load must emit on the text backend");
    assert!(
        out.contains("EXPAND ATOMIC_I32_LOAD "),
        "expected the ATOMIC_I32_LOAD projection, got:\n{out}"
    );
    assert!(
        out.contains("acquire"),
        "the ordering must lower to its lowercase token, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND load "),
        "a load must not emit the bare-name EXPAND, got:\n{out}"
    );
}

#[test]
fn mutex_lock_emits_mutex_lock_and_ok_result() {
    // fn f(m: Mutex<i32>) -> i32 {
    //     let g = m.lock();
    //     return 0;
    // }
    // The lock Result itself is never unwrapped here, so no
    // guard-unlock emission is expected; the arm's shape is
    // what this test pins (the unlock path is exercised by the
    // async_thread_pool CLI gate).
    let mutex_ty = ud("Mutex", vec![i32_ty()]);
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("m", mutex_ty.clone())],
            i32_ty(),
            vec![
                let_stmt("g", call("lock", vec![ident("m")])),
                ret(int_lit(0)),
            ],
        )],
    });
    let mut check = MapCheckInfo::new();
    let Node::Program(p) = &program else {
        panic!("root must be a Program node");
    };
    let Node::FuncDecl(f) = &p.decls[0] else {
        panic!("decl 0 must be a function");
    };
    let Node::LetStmt(l) = &f.body[0] else {
        panic!("body 0 must be a let");
    };
    check.add_expr_type(&l.value, ud("Result", vec![ud("MutexGuard", vec![i32_ty()]), i32_ty()]));
    let Node::CallExpr(c) = &*l.value else {
        panic!("let value must be a call");
    };
    check.add_expr_type(&c.args[0], mutex_ty);
    let out = sla_codegen_text::generate(&program, &check)
        .expect("Mutex::lock must emit on the text backend");
    assert!(
        out.contains("EXPAND MUTEX_LOCK "),
        "expected the MUTEX_LOCK statement, got:\n{out}"
    );
    assert!(
        out.contains("ptr_add "),
        "expected the guard pointer (ptr_add Mutex_data), got:\n{out}"
    );
    assert!(
        out.contains("Mutex_data"),
        "the guard must point at the mutex data slot, got:\n{out}"
    );
    assert!(
        out.contains("EXPAND RESULT_NEW_OK "),
        "expected the lock Result to wrap the guard, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND lock "),
        "a lock must not emit the bare-name EXPAND, got:\n{out}"
    );
}

#[test]
fn result_unwrap_emits_inline_branch() {
    // fn f(r: Result<i64, i64>) -> i64 { return r.unwrap(); }
    let result_ty = ud("Result", vec![i64_ty(), i64_ty()]);
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("r", result_ty.clone())],
            i64_ty(),
            vec![ret(call("unwrap", vec![ident("r")]))],
        )],
    });
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], result_ty);
    check.add_expr_type(value, i64_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("Result::unwrap must emit on the text backend");
    assert!(
        out.contains("L_RESULT_UNWRAP_OK"),
        "expected the inline unwrap branch, got:\n{out}"
    );
    assert!(
        out.contains("panic(17)"),
        "the Err arm must panic, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND RESULT_UNWRAP "),
        "an unwrap must not emit the bogus RESULT_UNWRAP EXPAND, got:\n{out}"
    );
}

#[test]
fn join_emits_pthread_join() {
    // fn f(h: JoinHandle<i64>) -> Result<i64, i64> {
    //     return h.join();
    // }
    let handle_ty = ud("JoinHandle", vec![i64_ty()]);
    let result_ty = ud("Result", vec![i64_ty(), i64_ty()]);
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("h", handle_ty.clone())],
            result_ty.clone(),
            vec![ret(call("join", vec![ident("h")]))],
        )],
    });
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], handle_ty);
    check.add_expr_type(value, result_ty);
    let out = sla_codegen_text::generate(&program, &check)
        .expect("JoinHandle::join must emit on the text backend");
    assert!(
        out.contains("EXPAND THREAD_JOIN_STATUS "),
        "expected the pthread join arm, got:\n{out}"
    );
    assert!(
        out.contains("L_THREAD_JOIN_OK"),
        "expected the join branch labels, got:\n{out}"
    );
    assert!(
        out.contains("= alloc 24"),
        "expected the 24-byte Result build, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND join "),
        "a join must not emit the bare-name EXPAND, got:\n{out}"
    );
}

#[test]
fn option_unwrap_still_expands_control() {
    // fn f(o: Option<i64>) -> i64 { return o.unwrap(); }
    // Control: Option::unwrap keeps its real macro
    // (OPTION_UNWRAP, option.sa:489) and passes pre- and
    // post-fix; only Result::unwrap was bogus.
    let option_ty = ud("Option", vec![i64_ty()]);
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("o", option_ty.clone())],
            i64_ty(),
            vec![ret(call("unwrap", vec![ident("o")]))],
        )],
    });
    let mut check = MapCheckInfo::new();
    let value = return_value_of(&program, 0);
    let Node::CallExpr(c) = value else {
        panic!("return value must be a call");
    };
    check.add_expr_type(&c.args[0], option_ty);
    check.add_expr_type(value, i64_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("Option::unwrap must emit on the text backend");
    assert!(
        out.contains("EXPAND OPTION_UNWRAP "),
        "Option::unwrap must keep its real macro, got:\n{out}"
    );
}

//! (xxxvi) regression: `println("{}", v)` of a signed integer must
//! format through `@sa_fmt_i64`, not `@sa_fmt_u64`.
//!
//! Fail-before: the emitted SA text calls `@sa_fmt_u64` for the i64
//! hole (2 calls) and never `@sa_fmt_i64`; end to end, -3 prints as
//! 18446744073709551613 (2^64-3).
//! Pass-after: exactly one `@sa_fmt_i64` call (the i64 hole) and one
//! `@sa_fmt_u64` call (the u64 control); the seed prints -3.
//!
//! Placement: drop this file at
//! `crates/sla-codegen-text/tests/println_i64_signed.rs` and run
//! `cargo test -p sla-codegen-text --test println_i64_signed`.
//! It uses only public APIs (sla_ast, sla_lowering::MapCheckInfo,
//! sla_codegen_text::generate) — no golden file, no `sa` binary.
//!
//! NOTE: landing this fix also flips the signed-println calls in the
//! fixture-parity goldens (02_arith: 4, 03_if_else: 2,
//! 04_while_sum: 1, 07_functions: 2 — all `@sa_fmt_u64` today).
//! Those goldens were produced by the Zig plugin on 2026-11-07,
//! ten days BEFORE the Zig DEF-DEMOS-SIGNED-FMT fix (2026-11-17),
//! so they pin the old buggy behaviour. Regenerate them from the
//! current Zig plugin (see fixtures/PROVENANCE.md) in the same
//! landing; until then fixture_02/03/04/07 parity will fail
//! against the fixed codegen. That is the goldens being stale,
//! not the fix being wrong.

use sla_ast::{
    BinaryExpr, BinaryOp, CallExpr, FuncDecl, ImportDecl, LetStmt, Literal, Node, Param,
    Primitive, Program, ReturnStmt, TestDecl, Type,
};
use sla_lowering::MapCheckInfo;

fn i32t() -> Type {
    Type::Primitive(Primitive::I32)
}
fn i64t() -> Type {
    Type::Primitive(Primitive::I64)
}
fn u64t() -> Type {
    Type::Primitive(Primitive::U64)
}
fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}
fn int(value: i64) -> Node {
    Node::Literal(Literal::IntVal(value))
}
fn string(value: &str) -> Node {
    Node::Literal(Literal::StringVal(value.to_string()))
}
fn call(name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: None,
        generics: Vec::new(),
        args,
        callee: None,
    })
}
fn println_stmt(args: Vec<Node>) -> Node {
    Node::ExprStmt(Box::new(call("println", args)))
}
fn bin(op: BinaryOp, left: Node, right: Node) -> Node {
    Node::BinaryExpr(BinaryExpr {
        op,
        left: Box::new(left),
        right: Box::new(right),
    })
}
/// Unary minus as the parser desugars it: `0 - x`.
fn neg(expr: Node) -> Node {
    bin(BinaryOp::Sub, int(0), expr)
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
        params: Vec::new(),
        ret_ty: Box::new(i32t()),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
    })
}

#[test]
fn println_i64_uses_signed_formatter() {
    // fn main() -> i32 {
    //     let a = 0 - 3;   // i64, value -3
    //     let b = 0 - 3;   // u64 control, value 2^64-3
    //     println("{}", a);
    //     println("{}", b);
    //     return 0;
    // }
    let prog = Node::Program(Program {
        decls: vec![
            Node::ImportDecl(ImportDecl {
                path: "sa_std/io/print.sai".to_string(),
            }),
            Node::ImportDecl(ImportDecl {
                path: "sa_std/fmt.sai".to_string(),
            }),
            func(
                "main",
                vec![
                    let_stmt("a", neg(int(3))),
                    let_stmt("b", neg(int(3))),
                    println_stmt(vec![string("{}"), ident("a")]),
                    println_stmt(vec![string("{}"), ident("b")]),
                    ret(int(0)),
                ],
            ),
            Node::TestDecl(TestDecl {
                name: "xxxvi println i64".to_string(),
                is_ignored: false,
                should_panic: false,
                body: Vec::new(),
            }),
        ],
    });

    // Checker facts, keyed by node address in the finished tree
    // (same navigation pattern as fixture f02's cleanup override).
    let mut info = MapCheckInfo::new();
    if let Node::Program(p) = &prog {
        let main = match &p.decls[2] {
            Node::FuncDecl(f) => f,
            _ => panic!("decls[2] is not main"),
        };
        // body[0] `let a = 0 - 3`: value bin-expr i64, literals i32.
        if let Node::LetStmt(l) = &main.body[0] {
            if let Node::BinaryExpr(b) = l.value.as_ref() {
                info.add_expr_type(l.value.as_ref(), i64t());
                info.add_expr_type(b.left.as_ref(), i32t());
                info.add_expr_type(b.right.as_ref(), i32t());
            } else {
                panic!("body[0] value is not a bin expr");
            }
        }
        // body[1] `let b = 0 - 3`: value bin-expr u64, literals i32.
        if let Node::LetStmt(l) = &main.body[1] {
            if let Node::BinaryExpr(b) = l.value.as_ref() {
                info.add_expr_type(l.value.as_ref(), u64t());
                info.add_expr_type(b.left.as_ref(), i32t());
                info.add_expr_type(b.right.as_ref(), i32t());
            } else {
                panic!("body[1] value is not a bin expr");
            }
        }
        // body[2] println("{}", a): arg ident i64.
        if let Node::ExprStmt(s) = &main.body[2] {
            if let Node::CallExpr(c) = s.as_ref() {
                info.add_expr_type(&c.args[1], i64t());
            } else {
                panic!("body[2] is not a println call");
            }
        }
        // body[3] println("{}", b): arg ident u64.
        if let Node::ExprStmt(s) = &main.body[3] {
            if let Node::CallExpr(c) = s.as_ref() {
                info.add_expr_type(&c.args[1], u64t());
            } else {
                panic!("body[3] is not a println call");
            }
        }
        // body[4] return 0.
        if let Node::ReturnStmt(r) = &main.body[4] {
            if let Some(v) = r.value.as_ref() {
                info.add_expr_type(v, i32t());
            }
        }
    }

    let out = sla_codegen_text::generate(&prog, &info)
        .unwrap_or_else(|err| panic!("generate failed: {err}"));
    let i64_calls = out.matches("call @sa_fmt_i64(").count();
    let u64_calls = out.matches("call @sa_fmt_u64(").count();
    assert_eq!(
        i64_calls, 1,
        "expected exactly one @sa_fmt_i64 call (the i64 hole), got {i64_calls}:\n{out}"
    );
    assert_eq!(
        u64_calls, 1,
        "expected exactly one @sa_fmt_u64 call (the u64 control), got {u64_calls}:\n{out}"
    );
}

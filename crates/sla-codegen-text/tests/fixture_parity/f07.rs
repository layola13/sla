//! Fixture 07 (functions): recursive `fact` whose if expression is
//! void (both branches return), `add` with return cleanups [b, a],
//! branch returns in `fact` each carrying cleanup [n]. The void if
//! also proves the discarded-temp counter consumption (D-C5).

use sla_ast::{BinaryOp, Node};
use sla_lowering::MapCheckInfo;

use super::{
    bin, call, expr_stmt, func, ident, if_expr, import, int, println_stmt, program,
    register_expr_types, ret, string, test_decl, voidt,
};

pub fn build() -> (Node, MapCheckInfo) {
    let fact_if = if_expr(
        bin(BinaryOp::Le, ident("n"), int(1)),
        vec![ret(int(1))],
        vec![ret(bin(
            BinaryOp::Mul,
            ident("n"),
            call("fact", vec![bin(BinaryOp::Sub, ident("n"), int(1))]),
        ))],
    );
    let fact_body = vec![expr_stmt(fact_if)];
    let add_body = vec![ret(bin(BinaryOp::Add, ident("a"), ident("b")))];
    let main_body = vec![
        println_stmt(vec![string("{}"), call("fact", vec![int(5)])]),
        println_stmt(vec![string("{}"), call("add", vec![int(40), int(2)])]),
        ret(int(0)),
    ];
    let prog = program(vec![
        import("sa_std/io/print.sai"),
        import("sa_std/fmt.sai"),
        func("fact", &["n"], fact_body),
        func("add", &["a", "b"], add_body),
        func("main", &[], main_body),
        test_decl("basic 07 functions recursion"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    if let Node::Program(p) = &prog {
        // fact is decls[2]; its body[0] is the if expr statement.
        if let Node::FuncDecl(f) = &p.decls[2] {
            if let Node::ExprStmt(if_node) = &f.body[0] {
                // The if is void (both branches return).
                info.add_expr_type(if_node, voidt());
                if let Node::IfExpr(i) = &**if_node {
                    info.add_cleanups(&i.then_block[0], vec!["n".into()]);
                    if let Some(else_block) = &i.else_block {
                        info.add_cleanups(&else_block[0], vec!["n".into()]);
                    }
                }
            }
        }
        // add is decls[3]; its body[0] return carries [b, a].
        if let Node::FuncDecl(f) = &p.decls[3] {
            info.add_cleanups(&f.body[0], vec!["b".into(), "a".into()]);
        }
    }
    (prog, info)
}

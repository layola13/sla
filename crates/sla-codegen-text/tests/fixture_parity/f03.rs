//! Fixture 03 (if/else): `max` is a tail if expression over two
//! identifier branches; no cleanups anywhere.

use sla_ast::{BinaryOp, Node};
use sla_lowering::MapCheckInfo;

use super::{
    bin, call, expr_stmt, func, ident, if_expr, import, int, neg, println_stmt, program,
    register_expr_types, ret, string, test_decl,
};

pub fn build() -> (Node, MapCheckInfo) {
    let max_body = vec![expr_stmt(if_expr(
        bin(BinaryOp::Gt, ident("a"), ident("b")),
        vec![expr_stmt(ident("a"))],
        vec![expr_stmt(ident("b"))],
    ))];
    let main_body = vec![
        println_stmt(vec![
            string("{}"),
            call("max", vec![int(7), int(42)]),
        ]),
        println_stmt(vec![
            string("{}"),
            call("max", vec![neg(int(3)), neg(int(9))]),
        ]),
        ret(int(0)),
    ];
    let prog = program(vec![
        import("sa_std/io/print.sai"),
        import("sa_std/fmt.sai"),
        func("max", &["a", "b"], max_body),
        func("main", &[], main_body),
        test_decl("basic 03 max branches"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    (prog, info)
}

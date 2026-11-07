//! Fixture 04 (while sum): typed lets, a plain while loop with two
//! identifier assignments; the return statement carries the
//! checker cleanup list [i, limit].

use sla_ast::{BinaryOp, Node};
use sla_lowering::MapCheckInfo;

use super::{
    assign, bin, call, func, ident, import, int, let_typed, println_stmt, program,
    register_expr_types, ret, string, test_decl, while_stmt,
};

pub fn build() -> (Node, MapCheckInfo) {
    let while_body = vec![
        assign("t", bin(BinaryOp::Add, ident("t"), ident("i"))),
        assign("i", bin(BinaryOp::Add, ident("i"), int(1))),
    ];
    let sum_body = vec![
        let_typed("t", int(0)),
        let_typed("i", int(0)),
        while_stmt(bin(BinaryOp::Lt, ident("i"), ident("limit")), while_body),
        ret(ident("t")),
    ];
    let main_body = vec![
        println_stmt(vec![string("{}"), call("while_sum", vec![int(10)])]),
        ret(int(0)),
    ];
    let prog = program(vec![
        import("sa_std/io/print.sai"),
        import("sa_std/fmt.sai"),
        func("while_sum", &["limit"], sum_body),
        func("main", &[], main_body),
        test_decl("basic 04 while sum"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    // Cleanups on the while_sum return statement: decls[2], body[3].
    if let Node::Program(p) = &prog {
        if let Node::FuncDecl(f) = &p.decls[2] {
            info.add_cleanups(&f.body[3], vec!["i".into(), "limit".into()]);
        }
    }
    (prog, info)
}

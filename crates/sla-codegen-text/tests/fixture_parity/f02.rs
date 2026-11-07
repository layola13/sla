//! Fixture 02 (arithmetic): untyped lets over binary expressions;
//! the return statement of `arith_values` carries the checker
//! cleanup list [b, a, c, d] (proven by the golden's release order).

use sla_ast::{BinaryOp, Node};
use sla_lowering::MapCheckInfo;

use super::{
    bin, func, ident, import, int, let_stmt, neg, println_stmt, program, register_expr_types, ret,
    string, test_decl,
};

fn arith_exprs() -> Vec<Node> {
    vec![
        bin(BinaryOp::Add, int(2), bin(BinaryOp::Mul, int(3), int(4))),
        bin(
            BinaryOp::Div,
            bin(BinaryOp::Sub, int(20), int(8)),
            int(3),
        ),
        bin(BinaryOp::Mod, int(17), int(5)),
        neg(bin(BinaryOp::Add, int(2), int(3))),
    ]
}

pub fn build() -> (Node, MapCheckInfo) {
    let exprs = arith_exprs();
    let mut arith_body = vec![
        let_stmt("a", exprs[0].clone()),
        let_stmt("b", exprs[1].clone()),
        let_stmt("c", exprs[2].clone()),
        let_stmt("d", exprs[3].clone()),
    ];
    // return a + b * 10 + c * 100 + d;  (left associative)
    let sum = bin(
        BinaryOp::Add,
        bin(
            BinaryOp::Add,
            bin(
                BinaryOp::Add,
                ident("a"),
                bin(BinaryOp::Mul, ident("b"), int(10)),
            ),
            bin(BinaryOp::Mul, ident("c"), int(100)),
        ),
        ident("d"),
    );
    arith_body.push(ret(sum));

    let main_exprs = arith_exprs();
    let main_body = vec![
        println_stmt(vec![string("{}"), main_exprs[0].clone()]),
        println_stmt(vec![string("{}"), main_exprs[1].clone()]),
        println_stmt(vec![string("{}"), main_exprs[2].clone()]),
        println_stmt(vec![string("{}"), main_exprs[3].clone()]),
        ret(int(0)),
    ];

    let prog = program(vec![
        import("sa_std/io/print.sai"),
        import("sa_std/fmt.sai"),
        func("arith_values", &[], arith_body),
        func("main", &[], main_body),
        test_decl("basic 02 arith precedence"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    // Cleanups on the arith_values return statement: decls[2] is
    // arith_values, its body[4] is the return.
    if let Node::Program(p) = &prog {
        if let Node::FuncDecl(f) = &p.decls[2] {
            info.add_cleanups(
                &f.body[4],
                vec!["b".into(), "a".into(), "c".into(), "d".into()],
            );
        }
    }
    (prog, info)
}

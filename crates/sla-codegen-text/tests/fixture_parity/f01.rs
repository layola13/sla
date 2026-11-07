//! Fixture 01 (hello world): three source imports, a println of a
//! plain string, no checker cleanups anywhere.

use sla_ast::Node;
use sla_lowering::MapCheckInfo;

use super::{func, import, int, println_stmt, program, register_expr_types, ret, string, test_decl};

pub fn build() -> (Node, MapCheckInfo) {
    let prog = program(vec![
        import("sa_std/io/print.sai"),
        import("sa_std/fmt.sai"),
        import("sa_std/string.sa"),
        func(
            "main",
            &[],
            vec![println_stmt(vec![string("hello world")]), ret(int(0))],
        ),
        test_decl("basic 01 hello world"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    (prog, info)
}

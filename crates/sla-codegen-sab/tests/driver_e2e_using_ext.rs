//! Regression (stage-3 `using` extension fix): a module function
//! lifted by the .sla import expansion carries a module-prefixed
//! declaration name (`vec_ext__len_plus`), while a `using`
//! extension call site keeps the bare name (`len_plus`). The
//! plain static-call tail must resolve the bare name to the
//! unique funcs-table key with that tail segment instead of
//! halting `call to unknown or non-user function`.
//!
//! The corpus shapes are
//! sa_plugin_sla/tests/test_unit_using_ext_{std_vec,borrow_auto,
//! borrow_explicit}.sla (method syntax on Vec receivers); this
//! miniature exercises the same tail resolution with scalar
//! functions so it runs without the std surface or a host
//! flattener.

use sla_ast::*;
use sla_codegen_sab::generate_from_program;
use sla_lowering::{node_key, MapCheckInfo};

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn param(name: &str) -> Param {
    Param {
        is_var: false,
        name: name.to_string(),
        ty: Box::new(i32_ty()),
        is_borrow: false,
        is_move: false,
        fallible_sugar: false,
    }
}

fn func(name: &str, params: Vec<Param>, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        is_variadic: false,
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params,
        ret_ty: Box::new(i32_ty()),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
    })
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn int(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn bin(op: BinaryOp, l: Node, r: Node) -> Node {
    Node::BinaryExpr(BinaryExpr {
        op,
        left: Box::new(l),
        right: Box::new(r),
    })
}

fn ret(v: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(v)),
    })
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

/// Type every node in the finished tree as i32 in the check info
/// (same discipline as driver_e2e.rs: keys are taken while the
/// tree is alive and unmodified).
fn type_all_i32(program: &Node) -> MapCheckInfo {
    fn walk(node: &Node, info: &mut MapCheckInfo) {
        info.expr_types.insert(node_key(node), i32_ty());
        match node {
            Node::Program(p) => p.decls.iter().for_each(|d| walk(d, info)),
            Node::FuncDecl(f) => f.body.iter().for_each(|s| walk(s, info)),
            Node::TestDecl(t) => t.body.iter().for_each(|s| walk(s, info)),
            Node::LetStmt(l) => walk(&l.value, info),
            Node::ConstStmt(c) => walk(&c.value, info),
            Node::AssignStmt(a) => {
                walk(&a.target, info);
                walk(&a.value, info);
            }
            Node::ExprStmt(e) => walk(e, info),
            Node::ReturnStmt(r) => {
                if let Some(v) = &r.value {
                    walk(v, info);
                }
            }
            Node::BlockStmt(b) => b.body.iter().for_each(|s| walk(s, info)),
            Node::WhileStmt(w) => {
                walk(&w.cond, info);
                w.body.iter().for_each(|s| walk(s, info));
            }
            Node::BinaryExpr(b) => {
                walk(&b.left, info);
                walk(&b.right, info);
            }
            Node::CallExpr(c) => c.args.iter().for_each(|a| walk(a, info)),
            Node::IfExpr(i) => {
                walk(&i.cond, info);
                i.then_block.iter().for_each(|s| walk(s, info));
                if let Some(e) = &i.else_block {
                    e.iter().for_each(|s| walk(s, info));
                }
            }
            _ => {}
        }
    }
    let mut info = MapCheckInfo::new();
    walk(program, &mut info);
    info
}

#[test]
fn bare_call_resolves_to_module_prefixed_decl() {
    // fn vec_ext__len_plus(x: i32) -> i32 { return x + 100; }
    // fn main() -> i32 { return len_plus(3); }
    let program = Node::Program(Program {
        decls: vec![
            func(
                "vec_ext__len_plus",
                vec![param("x")],
                vec![ret(bin(BinaryOp::Add, ident("x"), int(100)))],
            ),
            func("main", vec![], vec![ret(call("len_plus", vec![int(3)]))]),
        ],
    });
    let check = type_all_i32(&program);
    let bytes = generate_from_program(&program, &check)
        .expect("bare `using`-style call must resolve to the module-prefixed decl");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("sla__vec_ext__len_plus"),
        "the emitted module must reference the mangled callee symbol"
    );
}

#[test]
fn bare_call_with_two_tail_candidates_still_halts() {
    // Two modules exporting the same bare name: the tail match is
    // ambiguous and the call must keep the loud stop rather than
    // guessing a callee.
    let program = Node::Program(Program {
        decls: vec![
            func(
                "alpha__dup",
                vec![param("x")],
                vec![ret(ident("x"))],
            ),
            func(
                "beta__dup",
                vec![param("x")],
                vec![ret(ident("x"))],
            ),
            func("main", vec![], vec![ret(call("dup", vec![int(1)]))]),
        ],
    });
    let check = type_all_i32(&program);
    assert!(
        generate_from_program(&program, &check).is_err(),
        "an ambiguous bare call must not resolve by guessing"
    );
}

//! Byte-parity harness: hand-built fixture ASTs + checker facts,
//! emitted through `sla_codegen_text::generate` and compared
//! byte-for-byte against the coordinator's golden files in
//! `fixtures/expected/` (produced by the Zig plugin).
//!
//! The checker facts mirror what the real checker would record:
//! expression types are registered by a structural walk (binary and
//! call expressions are i32, comparisons boolean, if expressions i32
//! unless a fixture overrides), and lexical cleanup lists are
//! attached to the exact return statements the Zig goldens prove
//! them on. Test-decl bodies are empty, matching the build-mode
//! programs the goldens were generated from (test-body pruning
//! happens upstream of codegen in the Zig pipeline).

use sla_ast::{
    AssignStmt, BinaryExpr, BinaryOp, CallExpr, FuncDecl, IfExpr, ImportDecl, LetStmt, Literal,
    Node, Param, Primitive, Program, ReturnStmt, TestDecl, Type, WhileStmt,
};
use sla_lowering::MapCheckInfo;

#[path = "fixture_parity/f01.rs"]
mod f01;
#[path = "fixture_parity/f02.rs"]
mod f02;
#[path = "fixture_parity/f03.rs"]
mod f03;
#[path = "fixture_parity/f04.rs"]
mod f04;
#[path = "fixture_parity/f07.rs"]
mod f07;

// -- small constructors -------------------------------------------------

pub fn i32t() -> Type {
    Type::Primitive(Primitive::I32)
}

pub fn voidt() -> Type {
    Type::Primitive(Primitive::VoidType)
}

pub fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

pub fn int(value: i64) -> Node {
    Node::Literal(Literal::IntVal(value))
}

pub fn string(value: &str) -> Node {
    Node::Literal(Literal::StringVal(value.to_string()))
}

pub fn bin(op: BinaryOp, left: Node, right: Node) -> Node {
    Node::BinaryExpr(BinaryExpr {
        op,
        left: Box::new(left),
        right: Box::new(right),
    })
}

/// Unary minus as the parser desugars it: `0 - x`.
pub fn neg(expr: Node) -> Node {
    bin(BinaryOp::Sub, int(0), expr)
}

pub fn call(name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: None,
        generics: Vec::new(),
        args,
        callee: None,
    })
}

pub fn println_stmt(args: Vec<Node>) -> Node {
    Node::ExprStmt(Box::new(call("println", args)))
}

pub fn let_stmt(name: &str, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: None,
        value: Box::new(value),
    })
}

pub fn let_typed(name: &str, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: Some(Box::new(i32t())),
        value: Box::new(value),
    })
}

pub fn ret(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(value)),
    })
}

pub fn expr_stmt(expr: Node) -> Node {
    Node::ExprStmt(Box::new(expr))
}

pub fn assign(target: &str, value: Node) -> Node {
    Node::AssignStmt(AssignStmt {
        target: Box::new(ident(target)),
        value: Box::new(value),
    })
}

pub fn func(name: &str, params: &[&str], body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params: params
            .iter()
            .map(|name| Param {
                name: name.to_string(),
                ty: Box::new(i32t()),
                is_borrow: false,
                is_move: false,
            })
            .collect(),
        ret_ty: Box::new(i32t()),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
    })
}

pub fn import(path: &str) -> Node {
    Node::ImportDecl(ImportDecl {
        path: path.to_string(),
    })
}

/// Test decl with an empty body, as in the build-mode program the
/// goldens were generated from.
pub fn test_decl(name: &str) -> Node {
    Node::TestDecl(TestDecl {
        name: name.to_string(),
        is_ignored: false,
        should_panic: false,
        body: Vec::new(),
    })
}

pub fn program(decls: Vec<Node>) -> Node {
    Node::Program(Program { decls })
}

pub fn if_expr(cond: Node, then_block: Vec<Node>, else_block: Vec<Node>) -> Node {
    Node::IfExpr(IfExpr {
        cond: Box::new(cond),
        let_chain: None,
        then_block,
        else_block: Some(else_block),
    })
}

pub fn while_stmt(cond: Node, body: Vec<Node>) -> Node {
    Node::WhileStmt(WhileStmt {
        cond: Box::new(cond),
        let_pattern: None,
        body,
    })
}

// -- checker-fact registration -------------------------------------------

fn is_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
    )
}

/// Structural type registration over the finished tree: every binary
/// expression is i32 (comparisons boolean), every call i32, every if
/// expression i32 (fixtures override where the golden proves void).
pub fn register_expr_types(info: &mut MapCheckInfo, node: &Node) {
    match node {
        Node::Program(p) => {
            for decl in &p.decls {
                register_expr_types(info, decl);
            }
        }
        Node::FuncDecl(f) => {
            for stmt in &f.body {
                register_expr_types(info, stmt);
            }
        }
        Node::TestDecl(t) => {
            for stmt in &t.body {
                register_expr_types(info, stmt);
            }
        }
        Node::LetStmt(l) => register_expr_types(info, &l.value),
        Node::ConstStmt(c) => register_expr_types(info, &c.value),
        Node::AssignStmt(a) => {
            register_expr_types(info, &a.target);
            register_expr_types(info, &a.value);
        }
        Node::ExprStmt(e) => register_expr_types(info, e),
        Node::ReturnStmt(r) => {
            if let Some(value) = &r.value {
                register_expr_types(info, value);
            }
        }
        Node::WhileStmt(w) => {
            register_expr_types(info, &w.cond);
            for stmt in &w.body {
                register_expr_types(info, stmt);
            }
        }
        Node::BlockStmt(b) => {
            for stmt in &b.body {
                register_expr_types(info, stmt);
            }
        }
        Node::BinaryExpr(b) => {
            let ty = if is_comparison(b.op) {
                Type::Primitive(Primitive::Boolean)
            } else {
                i32t()
            };
            info.add_expr_type(node, ty);
            register_expr_types(info, &b.left);
            register_expr_types(info, &b.right);
        }
        Node::CallExpr(c) => {
            info.add_expr_type(node, i32t());
            for arg in &c.args {
                register_expr_types(info, arg);
            }
        }
        Node::IfExpr(i) => {
            info.add_expr_type(node, i32t());
            register_expr_types(info, &i.cond);
            for stmt in &i.then_block {
                register_expr_types(info, stmt);
            }
            if let Some(else_block) = &i.else_block {
                for stmt in else_block {
                    register_expr_types(info, stmt);
                }
            }
        }
        Node::Literal(Literal::IntVal(_)) => {
            info.add_expr_type(node, i32t());
        }
        Node::Literal(Literal::FloatVal(_)) => {
            info.add_expr_type(node, Type::Primitive(Primitive::F64));
        }
        Node::Literal(Literal::BoolVal(_)) => {
            info.add_expr_type(node, Type::Primitive(Primitive::Boolean));
        }
        Node::Identifier(_) => {
            // Every fixture identifier is an i32 binding.
            info.add_expr_type(node, i32t());
        }
        Node::MoveExpr(m) => register_expr_types(info, &m.expr),
        Node::BorrowExpr(b) => register_expr_types(info, &b.expr),
        Node::DerefExpr(d) => register_expr_types(info, &d.expr),
        Node::CastExpr(c) => register_expr_types(info, &c.expr),
        _ => {}
    }
}

// -- assertion -------------------------------------------------------------

pub fn check_fixture(name: &str, program: &Node, info: &MapCheckInfo) {
    let out = sla_codegen_text::generate(program, info)
        .unwrap_or_else(|err| panic!("{name}: generate failed: {err}"));
    let expected_path = format!(
        "{}/../../fixtures/expected/{name}.sa",
        env!("CARGO_MANIFEST_DIR")
    );
    let expected = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|err| panic!("cannot read golden {expected_path}: {err}"));
    if out != expected {
        let out_lines: Vec<&str> = out.lines().collect();
        let exp_lines: Vec<&str> = expected.lines().collect();
        let mut first_diff = 0usize;
        while first_diff < out_lines.len().max(exp_lines.len()) {
            let o = out_lines.get(first_diff).copied().unwrap_or("<missing>");
            let e = exp_lines.get(first_diff).copied().unwrap_or("<missing>");
            if o != e {
                break;
            }
            first_diff += 1;
        }
        panic!(
            "{name}: byte mismatch at line {}:\n  ours:   {:?}\n  golden: {:?}",
            first_diff + 1,
            out_lines.get(first_diff).copied().unwrap_or("<missing>"),
            exp_lines.get(first_diff).copied().unwrap_or("<missing>"),
        );
    }
}

#[test]
fn fixture_01_hello() {
    let (program, info) = f01::build();
    check_fixture("01_hello", &program, &info);
}

#[test]
fn fixture_02_arith() {
    let (program, info) = f02::build();
    check_fixture("02_arith", &program, &info);
}

#[test]
fn fixture_03_if_else() {
    let (program, info) = f03::build();
    check_fixture("03_if_else", &program, &info);
}

#[test]
fn fixture_04_while_sum() {
    let (program, info) = f04::build();
    check_fixture("04_while_sum", &program, &info);
}

#[test]
fn fixture_07_functions() {
    let (program, info) = f07::build();
    check_fixture("07_functions", &program, &info);
}

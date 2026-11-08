//! Fixture-driven structural tests: the five vertical-slice fixtures must
//! parse, and their ASTs must have the shapes the Zig parser produces
//! (including its desugars). Negative tests pin the loud-error behavior.

use sla_ast::decls::{FuncDecl, Program};
use sla_ast::exprs::{BinaryOp, Literal};
use sla_ast::node::Node;
use sla_parser::{parse_program, ParseErrorKind, Parser};

fn fixture(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/");
    std::fs::read_to_string(format!("{}{}", path, name)).expect("fixture must exist")
}

fn parse_ok(source: &str) -> Program {
    match parse_program(source) {
        Ok(Node::Program(program)) => program,
        Ok(other) => panic!("expected a program node, got {:?}", other),
        Err(err) => panic!("expected a successful parse, got {}", err),
    }
}

fn find_fn<'p>(program: &'p Program, name: &str) -> &'p FuncDecl {
    program
        .decls
        .iter()
        .find_map(|decl| match decl {
            Node::FuncDecl(func) if func.name == name => Some(func),
            _ => None,
        })
        .unwrap_or_else(|| panic!("function {} not found", name))
}

fn int_lit(node: &Node) -> i64 {
    match node {
        Node::Literal(Literal::IntVal(value)) => *value,
        other => panic!("expected an int literal, got {:?}", other),
    }
}

#[test]
fn fixture_01_hello() {
    let program = parse_ok(&fixture("01_hello.sla"));
    // 3 imports + main + @test
    assert_eq!(program.decls.len(), 5);
    assert!(matches!(program.decls[0], Node::ImportDecl(_)));
    let main = find_fn(&program, "main");
    assert_eq!(main.body.len(), 2);
    match &main.body[0] {
        Node::ExprStmt(expr) => match &**expr {
            Node::CallExpr(call) => {
                assert_eq!(call.func_name, "println");
                assert_eq!(call.args.len(), 1);
                match &call.args[0] {
                    Node::Literal(Literal::StringVal(text)) => {
                        assert_eq!(text, "hello world")
                    }
                    other => panic!("expected string literal arg, got {:?}", other),
                }
            }
            other => panic!("expected a call, got {:?}", other),
        },
        other => panic!("expected an expr statement, got {:?}", other),
    }
    // The @test body is one if statement whose condition is the prefix-`!`
    // desugar: `str_eq(...) == false`.
    let test_decl = program
        .decls
        .iter()
        .find_map(|decl| match decl {
            Node::TestDecl(test) => Some(test),
            _ => None,
        })
        .expect("@test decl present");
    assert_eq!(test_decl.name, "basic 01 hello world");
    assert_eq!(test_decl.body.len(), 1);
    match &test_decl.body[0] {
        Node::ExprStmt(expr) => match &**expr {
            Node::IfExpr(if_expr) => match &*if_expr.cond {
                Node::BinaryExpr(bin) => {
                    assert_eq!(bin.op, BinaryOp::Eq);
                    assert!(matches!(&*bin.right, Node::Literal(Literal::BoolVal(false))));
                }
                other => panic!("expected the ! desugar, got {:?}", other),
            },
            other => panic!("expected an if expr, got {:?}", other),
        },
        other => panic!("expected an expr statement, got {:?}", other),
    }
}

#[test]
fn fixture_02_arith() {
    let program = parse_ok(&fixture("02_arith.sla"));
    assert_eq!(program.decls.len(), 5);
    let arith = find_fn(&program, "arith_values");
    // `let d = -(2 + 3);` must be the unary-minus desugar `0 - (2 + 3)`.
    match &arith.body[3] {
        Node::LetStmt(let_stmt) => {
            assert_eq!(let_stmt.name, "d");
            match &*let_stmt.value {
                Node::BinaryExpr(bin) => {
                    assert_eq!(bin.op, BinaryOp::Sub);
                    assert_eq!(int_lit(&bin.left), 0);
                    match &*bin.right {
                        Node::BinaryExpr(inner) => {
                            assert_eq!(inner.op, BinaryOp::Add);
                            assert_eq!(int_lit(&inner.left), 2);
                            assert_eq!(int_lit(&inner.right), 3);
                        }
                        other => panic!("expected (2 + 3), got {:?}", other),
                    }
                }
                other => panic!("expected the minus desugar, got {:?}", other),
            }
        }
        other => panic!("expected a let statement, got {:?}", other),
    }
    // `let a = 2 + 3 * 4;` keeps * above +.
    match &arith.body[0] {
        Node::LetStmt(let_stmt) => match &*let_stmt.value {
            Node::BinaryExpr(bin) => {
                assert_eq!(bin.op, BinaryOp::Add);
                assert!(matches!(
                    &*bin.right,
                    Node::BinaryExpr(inner) if inner.op == BinaryOp::Mul
                ));
            }
            other => panic!("expected a binary expr, got {:?}", other),
        },
        other => panic!("expected a let statement, got {:?}", other),
    }
}

#[test]
fn fixture_03_if_else() {
    let program = parse_ok(&fixture("03_if_else.sla"));
    let max = find_fn(&program, "max");
    assert_eq!(max.params.len(), 2);
    // The body is a single tail if-expression with both branches and no
    // trailing semicolon before `}`.
    assert_eq!(max.body.len(), 1);
    match &max.body[0] {
        Node::ExprStmt(expr) => match &**expr {
            Node::IfExpr(if_expr) => {
                assert!(matches!(
                    &*if_expr.cond,
                    Node::BinaryExpr(bin) if bin.op == BinaryOp::Gt
                ));
                assert_eq!(if_expr.then_block.len(), 1);
                let else_block = if_expr.else_block.as_ref().expect("else block present");
                assert_eq!(else_block.len(), 1);
            }
            other => panic!("expected an if expr, got {:?}", other),
        },
        other => panic!("expected an expr statement, got {:?}", other),
    }
}

#[test]
fn fixture_04_while_sum() {
    let program = parse_ok(&fixture("04_while_sum.sla"));
    let while_sum = find_fn(&program, "while_sum");
    assert_eq!(while_sum.body.len(), 4);
    match &while_sum.body[2] {
        Node::WhileStmt(while_stmt) => {
            assert!(while_stmt.let_pattern.is_none());
            assert!(matches!(
                &*while_stmt.cond,
                Node::BinaryExpr(bin) if bin.op == BinaryOp::Lt
            ));
            assert_eq!(while_stmt.body.len(), 2);
            assert!(matches!(while_stmt.body[0], Node::AssignStmt(_)));
            assert!(matches!(while_stmt.body[1], Node::AssignStmt(_)));
        }
        other => panic!("expected a while statement, got {:?}", other),
    }
}

#[test]
fn fixture_07_functions() {
    let program = parse_ok(&fixture("07_functions.sla"));
    let fact = find_fn(&program, "fact");
    assert_eq!(fact.body.len(), 1);
    match &fact.body[0] {
        Node::ExprStmt(expr) => match &**expr {
            Node::IfExpr(if_expr) => {
                assert!(matches!(
                    &*if_expr.cond,
                    Node::BinaryExpr(bin) if bin.op == BinaryOp::Le
                ));
                assert!(matches!(if_expr.then_block[0], Node::ReturnStmt(_)));
                let else_block = if_expr.else_block.as_ref().expect("else block present");
                match &else_block[0] {
                    Node::ReturnStmt(ret) => {
                        let value = ret.value.as_ref().expect("return value");
                        assert!(matches!(
                            &**value,
                            Node::BinaryExpr(bin) if bin.op == BinaryOp::Mul
                        ));
                    }
                    other => panic!("expected a return, got {:?}", other),
                }
            }
            other => panic!("expected an if expr, got {:?}", other),
        },
        other => panic!("expected an expr statement, got {:?}", other),
    }
    let add = find_fn(&program, "add");
    assert_eq!(add.params.len(), 2);
}

#[test]
fn suffixed_int_literal_becomes_cast() {
    let program = parse_ok("fn f() -> i32 { let x = 32i32; return x; }");
    let f = find_fn(&program, "f");
    match &f.body[0] {
        Node::LetStmt(let_stmt) => {
            assert!(matches!(&*let_stmt.value, Node::CastExpr(_)));
        }
        other => panic!("expected a let statement, got {:?}", other),
    }
}

#[test]
fn struct_literal_needs_known_type() {
    // With the type seeded (as the import prescan would), `Point { ... }`
    // is a struct literal; unseeded it is not.
    let source = "struct Point { x: i32, y: i32 }\nfn f() -> i32 { let p = Point { x: 1, y: 2 }; return p.x; }";
    let program = parse_ok(source);
    let f = find_fn(&program, "f");
    match &f.body[0] {
        Node::LetStmt(let_stmt) => match &*let_stmt.value {
            Node::StructLiteral(lit) => {
                assert_eq!(lit.fields.len(), 2);
                assert_eq!(lit.fields[0].name, "x");
            }
            other => panic!("expected a struct literal, got {:?}", other),
        },
        other => panic!("expected a let statement, got {:?}", other),
    }
    // Field access on the next statement.
    assert!(matches!(f.body[1], Node::ReturnStmt(_)));
}

#[test]
fn parser_registers_imported_module_names() {
    let mut parser = Parser::new("@import \"geom.sla\"\nfn f() -> i32 { return geom::origin(); }");
    let program = parser.parse_program().expect("parse");
    assert_eq!(parser.known_module_names(), &["geom".to_string()]);
    match program {
        Node::Program(program) => {
            let f = find_fn(&program, "f");
            match &f.body[0] {
                Node::ReturnStmt(ret) => {
                    let value = ret.value.as_ref().expect("return value");
                    match &**value {
                        Node::CallExpr(call) => assert_eq!(call.func_name, "geom__origin"),
                        other => panic!("expected a module call, got {:?}", other),
                    }
                }
                other => panic!("expected a return, got {:?}", other),
            }
        }
        other => panic!("expected a program, got {:?}", other),
    }
}

#[test]
fn rejects_legacy_let_mut() {
    let err = parse_program("fn f() { let mut x = 1; }").expect_err("must reject let mut");
    assert_eq!(err.kind, ParseErrorKind::UnexpectedToken);
}

#[test]
fn rejects_unterminated_block() {
    let err = parse_program("fn f() {").expect_err("must reject unterminated block");
    assert_eq!(err.kind, ParseErrorKind::SyntaxError);
}

#[test]
fn rejects_missing_return_semicolon() {
    let err = parse_program("fn f() -> i32 { return 1 }").expect_err("must reject");
    assert_eq!(err.kind, ParseErrorKind::SyntaxError);
}

#[test]
fn rejects_uncovered_constructs_loudly() {
    // R-3: trait declarations are now covered (decl_trait.rs) —
    // the trait form parses; template literals are covered too
    // (decl_template.rs desugar). The loudly-rejected set keeps
    // the unknown @-decl.
    let ok = parse_ok("trait T { fn m(); }");
    assert_eq!(ok.decls.len(), 1);
    let err = parse_program("@foo \"x\"").expect_err("unknown @-decl must fail");
    assert_eq!(err.kind, ParseErrorKind::SyntaxError);
    let ok = parse_ok("fn f() { let x = `tpl`; }");
    assert_eq!(ok.decls.len(), 1);
}

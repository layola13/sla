//! CLI-D5R regression tests: the driver's SAB release emission
//! for while-shaped functions and panic-guard tests.
//!
//! Before D5R the driver emitted no releases at all (SAB-D19
//! item 1): the CLI-D5 repro (`while_sum` + two panic-guard
//! tests) passed the text backend but `sa test`'s strict SAB
//! verifier reported MemoryLeak (tmp_13 full run / tmp_7 with
//! `--filter alpha` — the test fns' call-result temps, consumed
//! by a comparison op that does not consume its operands).
//! These tests pin the wired consumption points on the decoded
//! instruction stream:
//! - a binary op releases its non-local operand temps right
//!   after the op;
//! - the while condition temp is released at the head of both
//!   branch successors (body and cond_false);
//! - the return statement runs the checker's cleanup list
//!   (exit locals/params, in list order) before the return,
//!   and never releases the returned binding itself.

use sla_ast::*;
use sla_codegen_sab::decode_module;
use sla_codegen_sab::generate_from_program;
use sla_codegen_sab::ir::{InstKind, Operand};
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

/// Type every node in the finished tree as i32 (same harness
/// discipline as driver_e2e's `type_all_i32`).
fn type_all_i32(program: &Node) -> MapCheckInfo {
    fn walk(node: &Node, info: &mut MapCheckInfo) {
        info.expr_types.insert(node_key(node), i32_ty());
        match node {
            Node::Program(p) => p.decls.iter().for_each(|d| walk(d, info)),
            Node::FuncDecl(f) => f.body.iter().for_each(|s| walk(s, info)),
            Node::TestDecl(t) => t.body.iter().for_each(|s| walk(s, info)),
            Node::LetStmt(l) => walk(&l.value, info),
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

/// The CLI-D5 repro's helper, verbatim shape:
/// `fn while_sum(limit: i32) -> i32 { let t=0; let i=0;
/// while i < limit { t = t + i; i = i + 1; } return t; }`
fn while_sum_program() -> Node {
    let func_node = func(
        "while_sum",
        vec![param("limit")],
        vec![
            Node::LetStmt(LetStmt {
                name: "t".into(),
                ty: Some(Box::new(i32_ty())),
                value: Box::new(int(0)),
            }),
            Node::LetStmt(LetStmt {
                name: "i".into(),
                ty: Some(Box::new(i32_ty())),
                value: Box::new(int(0)),
            }),
            Node::WhileStmt(WhileStmt {
                cond: Box::new(bin(BinaryOp::Lt, ident("i"), ident("limit"))),
                let_pattern: None,
                body: vec![
                    Node::AssignStmt(AssignStmt {
                        target: Box::new(ident("t")),
                        value: Box::new(bin(BinaryOp::Add, ident("t"), ident("i"))),
                    }),
                    Node::AssignStmt(AssignStmt {
                        target: Box::new(ident("i")),
                        value: Box::new(bin(BinaryOp::Add, ident("i"), int(1))),
                    }),
                ],
            }),
            ret(ident("t")),
        ],
    );
    Node::Program(Program {
        decls: vec![func_node],
    })
}

fn reg_of(module: &sla_codegen_sab::ir::Module, name: &str) -> u32 {
    module
        .symbols
        .iter()
        .position(|s| s == name)
        .map(|p| p as u32)
        .unwrap_or_else(|| panic!("symbol {name} interned"))
}

fn release_positions(
    module: &sla_codegen_sab::ir::Module,
    reg: u32,
) -> Vec<usize> {
    module
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, inst)| {
            inst.kind == InstKind::Release && inst.operands[0] == Operand::Reg(reg)
        })
        .map(|(idx, _)| idx)
        .collect()
}

#[test]
fn while_sum_sab_releases_cond_literal_and_exit_cleanups() {
    let program = while_sum_program();
    let mut check = type_all_i32(&program);
    // The real checker's return-exit list for `return t` in
    // this shape, in its recorded order (result root `t`
    // excluded by the checker itself). The cleanup key is the
    // address-based key of the return node *inside* the built
    // tree, so it must be borrowed from the tree itself.
    let ret_stmt = match &program {
        Node::Program(p) => match &p.decls[0] {
            Node::FuncDecl(f) => &f.body[3],
            other => panic!("decl 0 is the while_sum fn, got {other:?}"),
        },
        other => panic!("top-level program node, got {other:?}"),
    };
    check.add_cleanups(ret_stmt, vec!["i".to_string(), "limit".to_string()]);
    let bytes = generate_from_program(&program, &check).expect("driver generates while_sum");
    let module = decode_module(&bytes).expect("generated SAB decodes");

    // The branch instruction's condition register is a temp;
    // it must be released exactly twice: as the first
    // instruction of the body block and of the cond_false
    // block (one release per path, as in stock SAB).
    let br_idx = module
        .instructions
        .iter()
        .position(|inst| inst.kind == InstKind::Br)
        .expect("while emits a branch");
    let cond_reg = match &module.instructions[br_idx].operands[0] {
        Operand::Reg(r) => *r,
        other => panic!("branch cond operand is a register, got {other:?}"),
    };
    let cond_releases = release_positions(&module, cond_reg);
    assert_eq!(cond_releases.len(), 2, "cond temp released on both successors");
    // Each release directly follows its successor's label.
    for pos in &cond_releases {
        assert_eq!(
            module.instructions[pos - 1].kind,
            InstKind::Label,
            "cond release heads its successor block"
        );
    }

    // Exit cleanups: the last three instructions are
    // release(i), release(param limit), return(t) — in the
    // checker list's order, immediately before the return.
    let n = module.instructions.len();
    let ret_inst = &module.instructions[n - 1];
    assert_eq!(ret_inst.kind, InstKind::Return);
    let t_reg = reg_of(&module, "t");
    assert_eq!(ret_inst.operands[0], Operand::Reg(t_reg));
    let i_reg = reg_of(&module, "i");
    let limit_reg = module
        .symbols
        .iter()
        .position(|s| s == "sla__while_sum__param_0_limit")
        .map(|p| p as u32)
        .expect("scoped param symbol interned");
    assert_eq!(module.instructions[n - 3].kind, InstKind::Release);
    assert_eq!(module.instructions[n - 3].operands[0], Operand::Reg(i_reg));
    assert_eq!(module.instructions[n - 2].kind, InstKind::Release);
    assert_eq!(module.instructions[n - 2].operands[0], Operand::Reg(limit_reg));

    // The returned binding is never released, and the literal
    // operand temp of `i + 1` is released right after its op:
    // total releases = 2 cond + 1 literal + 2 exit cleanups.
    assert!(release_positions(&module, t_reg).is_empty());
    let total_releases = module
        .instructions
        .iter()
        .filter(|inst| inst.kind == InstKind::Release)
        .count();
    assert_eq!(total_releases, 5);
}

#[test]
fn panic_guard_test_releases_call_result_after_comparison() {
    // `@test "alpha while sum"() { if while_sum(10) != 45 { panic(401); }; }`
    // — the exact register the host verifier leaked (tmp_7 in
    // the filtered repro) is the call result consumed by `!=`.
    let program = Node::Program(Program {
        decls: vec![
            func("while_sum", vec![param("limit")], vec![ret(int(0))]),
            Node::TestDecl(TestDecl {
                name: "alpha while sum".into(),
                is_ignored: false,
                should_panic: false,
                body: vec![Node::ExprStmt(Box::new(Node::IfExpr(IfExpr {
                    cond: Box::new(bin(
                        BinaryOp::Ne,
                        call("while_sum", vec![int(10)]),
                        int(45),
                    )),
                    let_chain: None,
                    then_block: vec![Node::ExprStmt(Box::new(call("panic", vec![int(401)])))],
                    else_block: None,
                })))],
            }),
        ],
    });
    let check = type_all_i32(&program);
    let bytes = generate_from_program(&program, &check).expect("driver generates guard test");
    let module = decode_module(&bytes).expect("generated SAB decodes");

    // Find the comparison op in the test function: the two
    // instructions right after it must release its operand
    // temps (call result first, then the literal), in operand
    // order — the stock SAB pattern for this shape.
    let op_idx = module
        .instructions
        .iter()
        .rposition(|inst| inst.kind == InstKind::Op)
        .expect("comparison op emitted");
    let op = &module.instructions[op_idx];
    let lhs = match &op.operands[1] {
        Operand::Reg(r) => *r,
        other => panic!("op lhs is a register, got {other:?}"),
    };
    let rhs = match &op.operands[2] {
        Operand::Reg(r) => *r,
        other => panic!("op rhs is a register, got {other:?}"),
    };
    assert_eq!(module.instructions[op_idx + 1].kind, InstKind::Release);
    assert_eq!(module.instructions[op_idx + 1].operands[0], Operand::Reg(lhs));
    assert_eq!(module.instructions[op_idx + 2].kind, InstKind::Release);
    assert_eq!(module.instructions[op_idx + 2].operands[0], Operand::Reg(rhs));
}

//! H group end-to-end tests: structured statements and
//! expressions (for-range, if-value, cast, unsafe, inline asm,
//! using) — AST -> driver -> SAB bytes, compared byte-for-byte
//! against hand-built models through the phase-1 trunk, following
//! the `driver_e2e.rs` discipline. The loud-boundary tests pin the
//! shapes that stay unimplemented (collection for, untyped try).

use sla_ast::*;
use sla_codegen_sab::emit::ModuleBuilder;
use sla_codegen_sab::generate_from_program;
use sla_codegen_sab::ir::{FunctionSig, InstKind, Instruction, OpKind, Operand, PrimType};
use sla_codegen_sab::Error;
use sla_lowering::{node_key, MapCheckInfo};

// -- AST builders ---------------------------------------------------

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
}

fn func(name: &str, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params: Vec::new(),
        ret_ty: Box::new(i32_ty()),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
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

fn let_stmt(name: &str, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: None,
        value: Box::new(value),
    })
}

fn expr_stmt(e: Node) -> Node {
    Node::ExprStmt(Box::new(e))
}

/// Type every node i32, except cast nodes, which take their target
/// type. Built trees are typed after construction (address keys).
fn type_tree(program: &Node) -> MapCheckInfo {
    fn walk(node: &Node, info: &mut MapCheckInfo) {
        let ty = match node {
            Node::CastExpr(c) => (*c.ty).clone(),
            _ => i32_ty(),
        };
        info.expr_types.insert(node_key(node), ty);
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
            Node::ForStmt(f) => {
                walk(&f.start, info);
                if let Some(e) = &f.end {
                    walk(e, info);
                }
                f.body.iter().for_each(|s| walk(s, info));
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
            Node::CastExpr(c) => walk(&c.expr, info),
            Node::UnsafeExpr(u) => u.body.iter().for_each(|s| walk(s, info)),
            Node::TryExpr(t) => walk(&t.expr, info),
            _ => {}
        }
    }
    let mut info = MapCheckInfo::new();
    walk(program, &mut info);
    info
}

// -- model helpers ----------------------------------------------------

fn push_inst(b: &mut ModuleBuilder, kind: InstKind, operands: [Operand; 4]) {
    let mut inst = Instruction::new(kind, 0, b.instructions_len() as u32);
    inst.operands = operands;
    b.append_instruction(inst);
}

fn model_main(b: &mut ModuleBuilder) -> usize {
    let sig = b.add_sig(FunctionSig::new(0, "main", PrimType::I32, 0));
    b.begin_function(sig);
    b.append_decl_inst(sig);
    sig
}

// -- for (numeric range) ----------------------------------------------

/// fn main() -> i32 { let s = 0; for i in 0..3 { s = s + i; } return s; }
fn program_for_range() -> Node {
    Node::Program(Program {
        decls: vec![func(
            "main",
            vec![
                let_stmt("s", int(0)),
                Node::ForStmt(ForStmt {
                    var_name: "i".to_string(),
                    start: Box::new(int(0)),
                    end: Some(Box::new(int(3))),
                    body: vec![Node::AssignStmt(AssignStmt {
                        target: Box::new(ident("s")),
                        value: Box::new(bin(BinaryOp::Add, ident("s"), ident("i"))),
                    })],
                }),
                ret(ident("s")),
            ],
        )],
    })
}

fn model_for_range() -> Vec<u8> {
    let mut b = ModuleBuilder::new();
    model_main(&mut b);
    // let s = 0
    let t0 = b.new_tmp();
    b.emit_assign_int(t0, 0, false);
    let s = b.intern("s");
    b.emit_assign_reg(s, t0);
    // counter slot + start/end
    let counter = b.new_tmp();
    b.record_reg(counter);
    push_inst(
        &mut b,
        InstKind::StackAlloc,
        [Operand::Reg(counter), Operand::ImmU64(8), Operand::None, Operand::None],
    );
    let start = b.new_tmp();
    b.emit_assign_int(start, 0, false);
    let end = b.new_tmp();
    b.emit_assign_int(end, 3, false);
    push_inst(
        &mut b,
        InstKind::Store,
        [
            Operand::Reg(counter),
            Operand::ImmU64(0),
            Operand::Reg(start),
            Operand::Ty(PrimType::I64 as u32),
        ],
    );
    let head = b.new_label("L_FOR_HEAD");
    let body = b.new_label("L_FOR_BODY");
    let cont = b.new_label("L_FOR_CONTINUE");
    let cond_false = b.new_label("L_FOR_COND_FALSE");
    let _cleanup = b.new_label("L_FOR_BREAK_CLEANUP");
    let exit = b.new_label("L_FOR_EXIT");
    b.emit_jmp(&head);
    b.emit_label(&head);
    let index = b.new_tmp();
    b.record_reg(index);
    push_inst(
        &mut b,
        InstKind::Load,
        [
            Operand::Reg(index),
            Operand::Reg(counter),
            Operand::ImmU64(0),
            Operand::Ty(PrimType::I64 as u32),
        ],
    );
    let cond = b.new_tmp();
    b.emit_op(cond, OpKind::Slt, Operand::Reg(index), Operand::Reg(end));
    b.emit_branch(cond, &body, &cond_false);
    b.emit_label(&body);
    let loop_value = b.new_tmp();
    b.emit_op(loop_value, OpKind::Add, Operand::Reg(index), Operand::ImmI64(0));
    // s = s + i
    let sum = b.new_tmp();
    b.emit_op(sum, OpKind::Add, Operand::Reg(s), Operand::Reg(loop_value));
    b.emit_assign_reg(s, sum);
    b.emit_jmp(&cont);
    b.emit_label(&cont);
    let next = b.new_tmp();
    b.emit_op(next, OpKind::Add, Operand::Reg(index), Operand::ImmI64(1));
    push_inst(
        &mut b,
        InstKind::Store,
        [
            Operand::Reg(counter),
            Operand::ImmU64(0),
            Operand::Reg(next),
            Operand::Ty(PrimType::I64 as u32),
        ],
    );
    b.emit_jmp(&head);
    b.emit_label(&cond_false);
    b.emit_jmp(&exit);
    b.emit_label(&exit);
    b.emit_return(Some(s));
    let module = b.finish();
    sla_codegen_sab::generate(&module).unwrap()
}

#[test]
fn for_range_matches_hand_model() {
    let program = program_for_range();
    let info = type_tree(&program);
    let bytes = generate_from_program(&program, &info).expect("for-range must generate");
    assert_eq!(bytes, model_for_range());
    // Determinism: a second run is byte-identical.
    let again = generate_from_program(&program, &info).unwrap();
    assert_eq!(bytes, again);
}

#[test]
fn for_over_collection_stays_loud() {
    let program = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![
                Node::ForStmt(ForStmt {
                    var_name: "i".to_string(),
                    start: Box::new(ident("xs")),
                    end: None,
                    body: Vec::new(),
                }),
                ret(int(0)),
            ],
        )],
    });
    let info = type_tree(&program);
    let err = generate_from_program(&program, &info).unwrap_err();
    match err {
        Error::UnsupportedSabDirectFeature(msg) => {
            assert!(msg.contains("genForOverArray"), "unexpected message: {msg}");
        }
        other => panic!("expected loud unsupported, got {other:?}"),
    }
}

// -- if (value form) -----------------------------------------------------

/// fn main() -> i32 { let x = if 1 < 2 { 10 } else { 20 }; return x; }
fn program_if_value() -> Node {
    let ife = Node::IfExpr(IfExpr {
        cond: Box::new(bin(BinaryOp::Lt, int(1), int(2))),
        let_chain: None,
        then_block: vec![expr_stmt(int(10))],
        else_block: Some(vec![expr_stmt(int(20))]),
    });
    Node::Program(Program {
        decls: vec![func("main", vec![let_stmt("x", ife), ret(ident("x"))])],
    })
}

fn model_if_value() -> Vec<u8> {
    let mut b = ModuleBuilder::new();
    model_main(&mut b);
    let lhs = b.new_tmp();
    b.emit_assign_int(lhs, 1, false);
    let rhs = b.new_tmp();
    b.emit_assign_int(rhs, 2, false);
    let cond = b.new_tmp();
    b.emit_op(cond, OpKind::Slt, Operand::Reg(lhs), Operand::Reg(rhs));
    // CLI-D5R: the comparison's literal operand temps are
    // released right after the op (the op does not consume
    // them) — the hand model gains exactly these two releases.
    push_inst(
        &mut b,
        InstKind::Release,
        [Operand::Reg(lhs), Operand::None, Operand::None, Operand::None],
    );
    push_inst(
        &mut b,
        InstKind::Release,
        [Operand::Reg(rhs), Operand::None, Operand::None, Operand::None],
    );
    let slot = b.new_tmp();
    b.record_reg(slot);
    push_inst(
        &mut b,
        InstKind::Alloc,
        [Operand::Reg(slot), Operand::ImmU64(4), Operand::None, Operand::None],
    );
    let then_label = b.new_label("L_THEN");
    let else_label = b.new_label("L_ELSE");
    let merge_label = b.new_label("L_MERGE");
    b.emit_branch(cond, &then_label, &else_label);
    b.emit_label(&then_label);
    let tv = b.new_tmp();
    b.emit_assign_int(tv, 10, false);
    push_inst(
        &mut b,
        InstKind::Store,
        [
            Operand::Reg(slot),
            Operand::ImmU64(0),
            Operand::Reg(tv),
            Operand::Ty(PrimType::I32 as u32),
        ],
    );
    b.emit_jmp(&merge_label);
    b.emit_label(&else_label);
    let ev = b.new_tmp();
    b.emit_assign_int(ev, 20, false);
    push_inst(
        &mut b,
        InstKind::Store,
        [
            Operand::Reg(slot),
            Operand::ImmU64(0),
            Operand::Reg(ev),
            Operand::Ty(PrimType::I32 as u32),
        ],
    );
    b.emit_jmp(&merge_label);
    b.emit_label(&merge_label);
    let result = b.new_tmp();
    b.record_reg(result);
    push_inst(
        &mut b,
        InstKind::Load,
        [
            Operand::Reg(result),
            Operand::Reg(slot),
            Operand::ImmU64(0),
            Operand::Ty(PrimType::I32 as u32),
        ],
    );
    let x = b.intern("x");
    b.emit_assign_reg(x, result);
    b.emit_return(Some(x));
    let module = b.finish();
    sla_codegen_sab::generate(&module).unwrap()
}

#[test]
fn if_value_matches_hand_model() {
    let program = program_if_value();
    let info = type_tree(&program);
    let bytes = generate_from_program(&program, &info).expect("if-value must generate");
    assert_eq!(bytes, model_if_value());
}

// -- cast -----------------------------------------------------------------

/// fn main() -> i32 { let a = 5; return (a as i64) as i32; }
fn program_cast() -> Node {
    let inner = Node::CastExpr(CastExpr {
        expr: Box::new(ident("a")),
        ty: Box::new(i64_ty()),
    });
    let outer = Node::CastExpr(CastExpr {
        expr: Box::new(inner),
        ty: Box::new(i32_ty()),
    });
    Node::Program(Program {
        decls: vec![func("main", vec![let_stmt("a", int(5)), ret(outer)])],
    })
}

fn model_cast() -> Vec<u8> {
    let mut b = ModuleBuilder::new();
    model_main(&mut b);
    let t0 = b.new_tmp();
    b.emit_assign_int(t0, 5, false);
    let a = b.intern("a");
    b.emit_assign_reg(a, t0);
    // a as i64: sext with Ty(i64) operand
    let wide = b.new_tmp();
    b.record_reg(wide);
    let mut inst = Instruction::new(InstKind::Op, 0, b.instructions_len() as u32);
    inst.op_kind = Some(OpKind::Sext);
    inst.operands = [
        Operand::Reg(wide),
        Operand::Reg(a),
        Operand::Ty(PrimType::I64 as u32),
        Operand::None,
    ];
    b.append_instruction(inst);
    // (a as i64) as i32: trunc with Ty(i32) operand
    let back = b.new_tmp();
    b.record_reg(back);
    let mut inst = Instruction::new(InstKind::Op, 0, b.instructions_len() as u32);
    inst.op_kind = Some(OpKind::Trunc);
    inst.operands = [
        Operand::Reg(back),
        Operand::Reg(wide),
        Operand::Ty(PrimType::I32 as u32),
        Operand::None,
    ];
    b.append_instruction(inst);
    b.emit_return(Some(back));
    let module = b.finish();
    sla_codegen_sab::generate(&module).unwrap()
}

#[test]
fn cast_chain_matches_hand_model() {
    let program = program_cast();
    let info = type_tree(&program);
    let bytes = generate_from_program(&program, &info).expect("cast chain must generate");
    assert_eq!(bytes, model_cast());
}

// -- unsafe (value form) ----------------------------------------------------

/// fn main() -> i32 { let x = unsafe { 42 }; return x; }
fn program_unsafe_value() -> Node {
    let u = Node::UnsafeExpr(UnsafeExpr {
        body: vec![expr_stmt(int(42))],
    });
    Node::Program(Program {
        decls: vec![func("main", vec![let_stmt("x", u), ret(ident("x"))])],
    })
}

fn model_unsafe_value() -> Vec<u8> {
    let mut b = ModuleBuilder::new();
    model_main(&mut b);
    let slot = b.new_tmp();
    b.record_reg(slot);
    push_inst(
        &mut b,
        InstKind::Alloc,
        [Operand::Reg(slot), Operand::ImmU64(4), Operand::None, Operand::None],
    );
    let v = b.new_tmp();
    b.emit_assign_int(v, 42, false);
    push_inst(
        &mut b,
        InstKind::Store,
        [
            Operand::Reg(slot),
            Operand::ImmU64(0),
            Operand::Reg(v),
            Operand::Ty(PrimType::I32 as u32),
        ],
    );
    let result = b.new_tmp();
    b.record_reg(result);
    push_inst(
        &mut b,
        InstKind::Load,
        [
            Operand::Reg(result),
            Operand::Reg(slot),
            Operand::ImmU64(0),
            Operand::Ty(PrimType::I32 as u32),
        ],
    );
    let x = b.intern("x");
    b.emit_assign_reg(x, result);
    b.emit_return(Some(x));
    let module = b.finish();
    sla_codegen_sab::generate(&module).unwrap()
}

#[test]
fn unsafe_value_matches_hand_model() {
    let program = program_unsafe_value();
    let info = type_tree(&program);
    let bytes = generate_from_program(&program, &info).expect("unsafe value must generate");
    assert_eq!(bytes, model_unsafe_value());
}

// -- inline asm + using + try boundary ---------------------------------------

#[test]
fn inline_asm_yields_zero_sentinel() {
    let asm = Node::InlineAsmExpr(InlineAsmExpr {
        template: "nop".to_string(),
        operands: Vec::new(),
    });
    let program = Node::Program(Program {
        decls: vec![func("main", vec![ret(asm)])],
    });
    let info = type_tree(&program);
    let bytes = generate_from_program(&program, &info).expect("inline asm must generate");
    let mut b = ModuleBuilder::new();
    model_main(&mut b);
    let sentinel = b.new_tmp();
    b.record_reg(sentinel);
    b.emit_assign_imm(sentinel, 0);
    b.emit_return(Some(sentinel));
    let module = b.finish();
    assert_eq!(bytes, sla_codegen_sab::generate(&module).unwrap());
}

#[test]
fn using_statement_emits_nothing() {
    let program = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![
                Node::UsingDecl(UsingDecl {
                    path: "std::fmt".to_string(),
                }),
                ret(int(3)),
            ],
        )],
    });
    let info = type_tree(&program);
    let bytes = generate_from_program(&program, &info).expect("using stmt must generate");
    let mut b = ModuleBuilder::new();
    model_main(&mut b);
    let v = b.new_tmp();
    b.emit_assign_int(v, 3, false);
    b.emit_return(Some(v));
    let module = b.finish();
    assert_eq!(bytes, sla_codegen_sab::generate(&module).unwrap());
}

#[test]
fn try_without_operand_type_is_missing_type() {
    let try_node = Node::TryExpr(TryExpr {
        expr: Box::new(int(5)),
    });
    let program = Node::Program(Program {
        decls: vec![func("main", vec![ret(try_node)])],
    });
    // No expr types recorded at all: gen_try must fail MissingType,
    // not silently fabricate one (SAB-D12 discipline).
    let info = MapCheckInfo::new();
    let err = generate_from_program(&program, &info).unwrap_err();
    assert!(matches!(err, Error::MissingType(_)), "got {err:?}");
}

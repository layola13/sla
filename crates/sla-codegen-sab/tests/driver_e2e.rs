//! B4 end-to-end tests: AST -> driver -> SAB bytes.
//!
//! The parity claim is the phase-1 one, extended to the driver:
//! for a fixture01-level program (user functions, inter-function
//! calls, literals, scalar ops, if/while forms — the non-std
//! surface part of fixture01's shape), the driver's output must be
//! byte-identical to a hand-built model of the same program built
//! through the phase-1 emission trunk (`ModuleBuilder`), and its
//! sha256 is pinned below.
//!
//! Std-surface programs (the real fixture01: `println` / `str_eq`)
//! must fail loudly with a D-group attribution — the driver never
//! emits a fake surface expansion and never falls back to text.

use sla_ast::*;
use sla_codegen_sab::emit::ModuleBuilder;
use sla_codegen_sab::generate_from_program;
use sla_codegen_sab::ir::{CapPrefix, FunctionSig, OpKind, Operand, ParamSpec, PrimType};
use sla_codegen_sab::Error;
use sla_lowering::{node_key, MapCheckInfo};

// -- tiny SHA-256 (test-only; the crate has no hash dependency) ---

fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

// -- AST builders ---------------------------------------------------

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn param(name: &str) -> Param {
    Param {
        name: name.to_string(),
        ty: Box::new(i32_ty()),
        is_borrow: false,
        is_move: false,
    }
}

fn func(name: &str, params: Vec<Param>, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
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
    Node::ReturnStmt(ReturnStmt { value: Some(Box::new(v)) })
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

/// Type every node in the finished tree as i32 in the check info.
/// The trees are built first and typed afterwards, so the
/// address-based node keys are the final ones (the same discipline
/// the real checker uses: keys are taken while the tree is alive
/// and unmodified).
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

/// Program P1: two user functions, an inter-function call, literals
/// and a scalar add — fixture01's non-surface shape in miniature.
///
/// ```sla
/// fn add(a: i32, b: i32) -> i32 { return a + b; }
/// fn main() -> i32 { return add(40, 2); }
/// ```
fn program_p1() -> Node {
    Node::Program(Program {
        decls: vec![
            func(
                "add",
                vec![param("a"), param("b")],
                vec![ret(bin(BinaryOp::Add, ident("a"), ident("b")))],
            ),
            func("main", vec![], vec![ret(call("add", vec![int(40), int(2)]))]),
        ],
    })
}

/// The hand model of P1, built through the phase-1 trunk with the
/// driver's documented conventions (lowered symbols, scoped param
/// registers, trunk interning order).
fn model_p1() -> Vec<u8> {
    let mut b = ModuleBuilder::new();
    // fn sla__add(a, b) -> i32 { return a + b; }
    let add = b.add_sig(FunctionSig::new(0, "sla__add", PrimType::I32, 0));
    b.begin_function(add);
    let pa = b.intern("sla__add__param_0_a");
    let pb = b.intern("sla__add__param_1_b");
    b.set_sig_params(
        add,
        vec![
            ParamSpec { name: "a".into(), ty: PrimType::I32, cap: CapPrefix::ByValue },
            ParamSpec { name: "b".into(), ty: PrimType::I32, cap: CapPrefix::ByValue },
        ],
        vec![pa, pb],
    );
    b.record_reg(pa);
    b.record_reg(pb);
    b.append_decl_inst(add);
    // SAB-D17: add is the first sig emitted, so its counter id (0)
    // coincides with its intern id; no override needed.
    let sum = b.new_tmp();
    b.emit_op(sum, OpKind::Add, Operand::Reg(pa), Operand::Reg(pb));
    b.emit_return(Some(sum));
    b.finish_function();
    // fn main() -> i32 { return add(40, 2); }
    let main = b.add_sig(FunctionSig::new(0, "main", PrimType::I32, 0));
    b.begin_function(main);
    b.append_decl_inst(main);
    // SAB-D17 dual track: the decl operands above carry main's
    // intern id (4), but the sig's own id is the Zig emission
    // counter — the second sig emitted, hence 1.
    b.set_sig_id(main, 1);
    let arg0 = b.new_tmp();
    b.emit_assign_int(arg0, 40, false);
    let arg1 = b.new_tmp();
    b.emit_assign_int(arg1, 2, false);
    let dst = b.new_tmp();
    b.emit_call(Some(dst), "@sla__add(tmp_1, tmp_2)");
    b.emit_return(Some(dst));
    b.finish_function();
    sla_codegen_sab::generate(&b.finish()).expect("encode model")
}

#[test]
fn sha256_self_check() {
    // FIPS 180-4 example vector for "abc".
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn driver_p1_matches_hand_model_byte_for_byte() {
    let program = program_p1();
    let check = type_all_i32(&program);
    let bytes = generate_from_program(&program, &check).expect("driver generates P1");
    let model = model_p1();
    assert_eq!(bytes, model, "driver output must equal the hand model");
    assert_eq!(bytes.len(), MODEL_P1_LEN);
    assert_eq!(sha256_hex(&bytes), MODEL_P1_SHA256);
}

/// Program P2: while loop + statement-if with returns + a compound
/// return expression — the fixture01-level control-flow shapes.
///
/// ```sla
/// fn while_sum(limit: i32) -> i32 {
///     let t: i32 = 0; let i: i32 = 0;
///     while i < limit { t = t + i; i = i + 1; }
///     return t;
/// }
/// fn classify(n: i32) -> i32 {
///     if n > 10 { return 1; } else { return 2; };
/// }
/// fn main() -> i32 { return while_sum(10) + classify(3); }
/// ```
fn program_p2() -> Node {
    let while_sum = func(
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
    let classify = func(
        "classify",
        vec![param("n")],
        vec![Node::ExprStmt(Box::new(Node::IfExpr(IfExpr {
            cond: Box::new(bin(BinaryOp::Gt, ident("n"), int(10))),
            let_chain: None,
            then_block: vec![ret(int(1))],
            else_block: Some(vec![ret(int(2))]),
        })))],
    );
    let main = func(
        "main",
        vec![],
        vec![ret(bin(
            BinaryOp::Add,
            call("while_sum", vec![int(10)]),
            call("classify", vec![int(3)]),
        ))],
    );
    Node::Program(Program {
        decls: vec![while_sum, classify, main],
    })
}

#[test]
fn driver_p2_control_flow_end_to_end() {
    let program = program_p2();
    let check = type_all_i32(&program);
    let bytes = generate_from_program(&program, &check).expect("driver generates P2");
    // Deterministic: a second run over the same tree is identical.
    let again = generate_from_program(&program, &check).expect("driver regenerates P2");
    assert_eq!(bytes, again);
    assert_eq!(&bytes[0..4], b"SAB\0");
    assert_eq!(bytes.len(), MODEL_P2_LEN);
    assert_eq!(sha256_hex(&bytes), MODEL_P2_SHA256);
}

/// Program P3: a test decl between two functions — the SAB-D17
/// counter's jump case. Zig `genTestDecl` appends the test sig to
/// BOTH `function_sigs` and `test_sigs`, so the emission counter
/// (`function_sigs.len + test_sigs.len` at `genFuncSig` entry)
/// advances by 2 for one test: the sig ids here are add=0,
/// test=1, main=3 (2 is skipped). Verified against a real Zig
/// SAB product for the same shape (func/test/func): sig ids
/// 0/1/3, decl operands at the intern ids — see
/// docs/divergences/sab.md SAB-D17.
///
/// ```sla
/// fn add(a: i32, b: i32) -> i32 { return a + b; }
/// @test "t one"() {}
/// fn main() -> i32 { return add(40, 2); }
/// ```
fn program_p3() -> Node {
    Node::Program(Program {
        decls: vec![
            func(
                "add",
                vec![param("a"), param("b")],
                vec![ret(bin(BinaryOp::Add, ident("a"), ident("b")))],
            ),
            Node::TestDecl(TestDecl {
                name: "t one".into(),
                is_ignored: false,
                should_panic: false,
                body: vec![],
            }),
            func("main", vec![], vec![ret(call("add", vec![int(40), int(2)]))]),
        ],
    })
}

/// The hand model of P3, mirroring the driver's conventions plus
/// the SAB-D17 counter ids (0 / 1 / 3) applied after each decl
/// instruction (whose operands keep the intern ids).
fn model_p3() -> Vec<u8> {
    let mut b = ModuleBuilder::new();
    // fn sla__add(a, b) -> i32 { return a + b; }  (counter id 0)
    let add = b.add_sig(FunctionSig::new(0, "sla__add", PrimType::I32, 0));
    b.begin_function(add);
    let pa = b.intern("sla__add__param_0_a");
    let pb = b.intern("sla__add__param_1_b");
    b.set_sig_params(
        add,
        vec![
            ParamSpec { name: "a".into(), ty: PrimType::I32, cap: CapPrefix::ByValue },
            ParamSpec { name: "b".into(), ty: PrimType::I32, cap: CapPrefix::ByValue },
        ],
        vec![pa, pb],
    );
    b.record_reg(pa);
    b.record_reg(pb);
    b.append_decl_inst(add);
    let sum = b.new_tmp();
    b.emit_op(sum, OpKind::Add, Operand::Reg(pa), Operand::Reg(pb));
    b.emit_return(Some(sum));
    b.finish_function();
    // @test "t one"() {}  (counter id 1)
    let mut test_sig = FunctionSig::new(0, "\"t one\"", PrimType::Void, 0);
    test_sig.kind = sla_codegen_sab::ir::FunctionKind::TestFunc;
    let test = b.add_sig(test_sig);
    b.begin_function(test);
    b.append_decl_inst(test);
    b.set_sig_id(test, 1);
    let entry = b.new_label("L_TEST_ENTRY");
    b.emit_label(&entry);
    b.emit_return(None);
    b.finish_function();
    // fn main() -> i32 { return add(40, 2); }  (counter id 3: the
    // test counted twice — once per list it was appended to)
    let main = b.add_sig(FunctionSig::new(0, "main", PrimType::I32, 0));
    b.begin_function(main);
    b.append_decl_inst(main);
    b.set_sig_id(main, 3);
    let arg0 = b.new_tmp();
    b.emit_assign_int(arg0, 40, false);
    let arg1 = b.new_tmp();
    b.emit_assign_int(arg1, 2, false);
    let dst = b.new_tmp();
    b.emit_call(Some(dst), "@sla__add(tmp_1, tmp_2)");
    b.emit_return(Some(dst));
    b.finish_function();
    sla_codegen_sab::generate(&b.finish()).expect("encode model")
}

#[test]
fn driver_p3_sig_id_counter_skips_after_test() {
    let program = program_p3();
    let check = type_all_i32(&program);
    let bytes = generate_from_program(&program, &check).expect("driver generates P3");
    let model = model_p3();
    assert_eq!(bytes, model, "driver output must equal the dual-track hand model");
    assert_eq!(bytes.len(), MODEL_P3_LEN);
    assert_eq!(sha256_hex(&bytes), MODEL_P3_SHA256);
}

/// The real fixture01 shape: imports, `println` in main, `str_eq`
/// in the test. Both are D-group surface expansions — the driver
/// must refuse loudly (D attribution), never fake the expansion.
#[test]
fn driver_fixture01_std_surface_is_loudly_uncovered() {
    let program = Node::Program(Program {
        decls: vec![
            Node::ImportDecl(ImportDecl { path: "sa_std/io/print.sai".into() }),
            Node::ImportDecl(ImportDecl { path: "sa_std/fmt.sai".into() }),
            Node::ImportDecl(ImportDecl { path: "sa_std/string.sa".into() }),
            func(
                "main",
                vec![],
                vec![
                    Node::ExprStmt(Box::new(call(
                        "println",
                        vec![Node::Literal(Literal::StringVal("hello world".into()))],
                    ))),
                    ret(int(0)),
                ],
            ),
        ],
    });
    let check = type_all_i32(&program);
    match generate_from_program(&program, &check) {
        Err(Error::UnsupportedSabDirectFeature(msg)) => {
            assert!(msg.contains("println"), "attribution names println: {msg}");
            assert!(msg.contains("group D"), "attribution names group D: {msg}");
        }
        other => panic!("expected loud D-group refusal, got {other:?}"),
    }
}

/// `str_eq` on its own (the fixture01 test-decl shape) is likewise
/// D-group surface, and a string literal binding is D/B6 surface
/// materialization — both loud.
#[test]
fn driver_str_eq_and_string_let_are_loudly_uncovered() {
    let str_eq_prog = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![],
            vec![ret(call(
                "str_eq",
                vec![
                    Node::Literal(Literal::StringVal("a".into())),
                    Node::Literal(Literal::StringVal("a".into())),
                ],
            ))],
        )],
    });
    let check = type_all_i32(&str_eq_prog);
    match generate_from_program(&str_eq_prog, &check) {
        Err(Error::UnsupportedSabDirectFeature(msg)) => {
            assert!(msg.contains("str_eq"), "{msg}");
            assert!(msg.contains("group D"), "{msg}");
        }
        other => panic!("expected loud D-group refusal, got {other:?}"),
    }

    let string_let = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![],
            vec![
                Node::LetStmt(LetStmt {
                    name: "s".into(),
                    ty: None,
                    value: Box::new(Node::Literal(Literal::StringVal("a".into()))),
                }),
                ret(int(0)),
            ],
        )],
    });
    let check = type_all_i32(&string_let);
    match generate_from_program(&string_let, &check) {
        Err(Error::UnsupportedSabDirectFeature(msg)) => {
            assert!(msg.contains("group D"), "{msg}");
        }
        other => panic!("expected loud D-group refusal, got {other:?}"),
    }
}

/// A test decl with a panic guard over a user call — the fixture01
/// test shape minus its std surface — generates end to end.
#[test]
fn driver_test_decl_with_panic_guard() {
    let program = Node::Program(Program {
        decls: vec![
            func("add", vec![param("a"), param("b")], vec![ret(bin(BinaryOp::Add, ident("a"), ident("b")))]),
            Node::TestDecl(TestDecl {
                name: "add works".into(),
                is_ignored: false,
                should_panic: false,
                body: vec![Node::ExprStmt(Box::new(Node::IfExpr(IfExpr {
                    cond: Box::new(bin(
                        BinaryOp::Ne,
                        call("add", vec![int(40), int(2)]),
                        int(42),
                    )),
                    let_chain: None,
                    then_block: vec![Node::ExprStmt(Box::new(call("panic", vec![int(101)])))],
                    else_block: None,
                })))],
            }),
        ],
    });
    let check = type_all_i32(&program);
    let bytes = generate_from_program(&program, &check).expect("driver generates test decl program");
    assert_eq!(&bytes[0..4], b"SAB\0");
    assert_eq!(sha256_hex(&bytes), MODEL_TEST_SHA256);
}

// Pinned after the first venue run (see the phase-2 report).
// SAB-D17 re-pin (2026-11-07), field-level diffs vs the old
// intern-id pins (decl operands — the intern ids — are unchanged
// in every case; lengths unchanged, all ids single-byte LEB):
// - P1 (was e92ab628…): main sig.id 4 -> 1; add 0 -> 0.
// - P2 (was a1b04fd6…): sla__classify sig.id 14 -> 1, main
//   sig.id 23 -> 2; sla__while_sum 0 -> 0.
// - TEST (was 8220127f…): the test sig.id 4 -> 1; add 0 -> 0.
// - P3 is new with the mirror (sig ids 0/1/3, decl operands
//   0/4/6 — the same shape the real Zig product shows).
const MODEL_P1_LEN: usize = 334;
const MODEL_P1_SHA256: &str = "2ab9401221af90e79d2d278992bd87b54c856a59804e4e7a880a8a853455d148";
const MODEL_P2_LEN: usize = 1184;
const MODEL_P2_SHA256: &str = "93e9cc6ace723d9ad5595a10acd9367a76962b76e33fabc862dd5c02aa7b106a";
const MODEL_P3_LEN: usize = 433;
const MODEL_P3_SHA256: &str = "ddcc33f7656899c1c4e183e4e5dc6dfda7a533f1f351a0fb7012eaebc9126b96";
const MODEL_TEST_SHA256: &str = "490503408db5dab65bd140bffc2f0646836b0d23890646f26194c3def4cea733";

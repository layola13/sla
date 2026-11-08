//! B4 end-to-end tests, part 2: C-group (pre-scan) wiring through
//! the driver — split from driver_e2e.rs by the phase-3 coordinator
//! at integration time purely for the 700-line file rule; test
//! names and bodies are unchanged (SAB-C lane, phase 3). The AST
//! builder helpers below are the same shared block as
//! driver_e2e.rs, duplicated per test-target crate.

#![allow(dead_code)]

use sla_ast::*;
use sla_codegen_sab::generate_from_program;
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

// -- Phase 3 (SAB-C): C-group wiring through the driver ---------

fn const_stmt(name: &str, value: Node) -> Node {
    Node::ConstStmt(ConstStmt {
        name: name.to_string(),
        ty: None,
        value: Box::new(value),
    })
}

/// C1 wired: alias + binary-folded top-level consts resolve at
/// pre-scan, so a program returning the folded const emits exactly
/// the bytes of the same program returning the literal.
#[test]
fn driver_global_const_fold_end_to_end() {
    let program = Node::Program(Program {
        decls: vec![
            const_stmt("BASE", int(40)),
            const_stmt("ALIAS", ident("BASE")),
            const_stmt(
                "SUM",
                bin(BinaryOp::Add, ident("ALIAS"), int(2)),
            ),
            func("main", vec![], vec![ret(ident("SUM"))]),
        ],
    });
    let check = type_all_i32(&program);
    let folded = generate_from_program(&program, &check).expect("const fold program generates");

    let direct = Node::Program(Program {
        decls: vec![func("main", vec![], vec![ret(int(42))])],
    });
    let check2 = type_all_i32(&direct);
    let direct_bytes = generate_from_program(&direct, &check2).expect("direct program generates");
    assert_eq!(folded, direct_bytes);
}

/// C1 wired: a checker-typed global array const materializes as a
/// `SLA_GCONST_<name>` hex const decl at pre-scan time; without
/// the array type fact it does not (Zig `orelse continue`).
#[test]
fn driver_global_array_const_materializes() {
    let arr_value = Node::ArrayLiteral(ArrayLiteral {
        elements: vec![int(1), int(2)],
    });
    let program = Node::Program(Program {
        decls: vec![
            const_stmt("ARR", arr_value),
            func("main", vec![], vec![ret(int(1))]),
        ],
    });
    let mut check = MapCheckInfo::new();
    if let Node::Program(p) = &program {
        if let Node::ConstStmt(c) = &p.decls[0] {
            check.add_expr_type(
                &c.value,
                Type::Array(ArrayType {
                    elem: Box::new(i32_ty()),
                    len: 2,
                }),
            );
        }
    }
    let bytes = generate_from_program(&program, &check).expect("array const program generates");
    let contains = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).any(|w| w == needle);
    assert!(contains(&bytes, b"SLA_GCONST_ARR"));
    // The pooled literal text: one backslash per byte, exactly as
    // in the real plugin's product (sci's managedPrint collapses
    // the doubled backslash of the source format string; venue
    // byte-verified, see prescan_consts.rs docs).
    assert!(contains(&bytes, br"hex:\x01\x00\x00\x00\x02\x00\x00\x00"));

    let untyped = MapCheckInfo::new();
    let bytes2 = generate_from_program(&program, &untyped).expect("untyped program generates");
    assert!(!contains(&bytes2, b"SLA_GCONST_ARR"));
}

/// Section-level parity with the REAL Zig plugin's product for
/// the C1 array-const probe program (venue, current-head plugin
/// `sa sla sab build`, 2026-11-07):
///   const TABLE: [i32; 3] = [1, 2, 16909060];
///   fn main() -> i32 { return 1; }
/// Real product: 225 bytes, sha256 a114ec0329fa.... Section diff
/// against this driver's output (197 bytes, pinned below):
/// signature section identical (19 B), const section identical
/// (21 B — the C1 array const decl matches the real product
/// byte-for-byte), string pool differs only by the `L_ENTRY`
/// entry (+8 B) and the instruction section by exactly the
/// `L_ENTRY` label instruction (+20 B) — the already-registered
/// SAB-D19 item ② (the B4 subset emits no `L_ENTRY`; Zig
/// genFuncDeclNamed emits it for every function at line 6218).
#[test]
fn driver_real_product_array_const_parity() {
    let arr_value = Node::ArrayLiteral(ArrayLiteral {
        elements: vec![int(1), int(2), int(16909060)],
    });
    let program = Node::Program(Program {
        decls: vec![
            const_stmt("TABLE", arr_value),
            func("main", vec![], vec![ret(int(1))]),
        ],
    });
    let mut check = type_all_i32(&program);
    if let Node::Program(p) = &program {
        if let Node::ConstStmt(c) = &p.decls[0] {
            check.add_expr_type(
                &c.value,
                Type::Array(ArrayType {
                    elem: Box::new(i32_ty()),
                    len: 3,
                }),
            );
        }
    }
    let bytes = generate_from_program(&program, &check).expect("array parity program generates");
    // The driver's own bytes (real product minus the L_ENTRY
    // label, per the section diff in this test's docs).
    assert_eq!(bytes.len(), 197);
    assert_eq!(
        sha256_hex(&bytes),
        "01862628010d57f3a45f5bba60d1d7046251943fa5136a05a678133e84b4c12a"
    );
}

/// C3 wired: a decl-only extern prototype whose name has a
/// concrete definition in the same program emits nothing
/// (`hasConcreteFunctionSymbol`), so the output equals the program
/// without the prototype.
#[test]
fn driver_extern_prototype_with_concrete_twin_is_suppressed() {
    let prototype = Node::FuncDecl(FuncDecl {
        is_variadic: false,
        name: "dup".to_string(),
        is_pub: false,
        is_extern: true,
        abi: None,
        no_mangle: false,
        is_decl_only: true,
        generics: Vec::new(),
        params: vec![],
        ret_ty: Box::new(i32_ty()),
        body: vec![],
        is_inline: false,
        is_async: false,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
    });
    let with_proto = Node::Program(Program {
        decls: vec![
            prototype,
            func("dup", vec![], vec![ret(int(3))]),
            func("main", vec![], vec![ret(call("dup", vec![]))]),
        ],
    });
    let without_proto = Node::Program(Program {
        decls: vec![
            func("dup", vec![], vec![ret(int(3))]),
            func("main", vec![], vec![ret(call("dup", vec![]))]),
        ],
    });
    let check = type_all_i32(&with_proto);
    let a = generate_from_program(&with_proto, &check).expect("prototype program generates");
    let check2 = type_all_i32(&without_proto);
    let b = generate_from_program(&without_proto, &check2).expect("plain program generates");
    assert_eq!(a, b);
}

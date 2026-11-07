//! Encoder golden tests.
//!
//! The minimal-module expectation below was derived by hand from the
//! Zig write path in sci `sab.zig` (field order and optional-field
//! encodings), independently of this crate's encoder implementation:
//! if the two disagree, at least one derivation is wrong and the
//! failure output shows both byte strings.

use sla_codegen_sab::encode::encode_module;
use sla_codegen_sab::fixture01::fixture01_module;
use sla_codegen_sab::ir::*;
use sla_codegen_sab::leb128::read_uleb128;
use sla_codegen_sab::pool::build_pool;

fn minimal_module() -> Module {
    let mut func_decl = Instruction::new(InstKind::FuncDecl, 0, 0);
    func_decl.operands[0] = Operand::Symbol(0);
    func_decl.operands[1] = Operand::Func(0);
    let ret = Instruction::new(InstKind::Return, 0, 1);
    Module {
        symbols: vec!["f".to_string()],
        function_sigs: vec![FunctionSig::new(0, "f", PrimType::Void, 0)],
        const_decls: Vec::new(),
        instructions: vec![func_decl, ret],
    }
}

/// Hand-derived expected bytes for `minimal_module` (75 bytes total).
fn minimal_expected() -> Vec<u8> {
    let mut v = Vec::new();
    // Header: magic, major 4, minor 0, section count 4.
    v.extend_from_slice(b"SAB\0");
    v.extend_from_slice(&[4, 0, 4]);
    // Section 1 (pool): payload = count 1, len 1, "f".
    v.extend_from_slice(&[1, 3, 1, 1, b'f']);
    // Section 2 (sigs): payload len 17 = count 1 + one 16-byte sig
    // record whose fields are all zero (id 0, name pool 0, kind
    // normal, no return cap, return ty void, all flags 0, no
    // optionals, zero params / param ids / reg ids).
    v.extend_from_slice(&[2, 17, 1]);
    v.extend_from_slice(&[0u8; 16]);
    // Section 3 (consts): payload = count 0.
    v.extend_from_slice(&[3, 1, 0]);
    // Section 4 (instructions): payload len 39 = count 2 + inst0 (20)
    // + inst1 (18).
    v.extend_from_slice(&[4, 39, 2]);
    // inst0: func_decl, lines 0/0, no op_kind; operands
    // [symbol 0, func 0, none, none]; ten zero flag/optional bytes.
    v.extend_from_slice(&[26, 0, 0, 0, 2, 0, 4, 0, 0, 0]);
    v.extend_from_slice(&[0u8; 10]);
    // inst1: return, lines 0/1, no op_kind; four none operands;
    // ten zero flag/optional bytes.
    v.extend_from_slice(&[24, 0, 1, 0, 0, 0, 0, 0]);
    v.extend_from_slice(&[0u8; 10]);
    v
}

#[test]
fn minimal_module_matches_hand_derived_bytes() {
    let bytes = encode_module(&minimal_module()).expect("encode minimal module");
    assert_eq!(bytes.len(), 75);
    assert_eq!(bytes, minimal_expected());
}

/// Walk the four sections of an encoded module, returning
/// (section id, payload) pairs. Panics on malformed framing.
fn sections(bytes: &[u8]) -> Vec<(u64, &[u8])> {
    assert_eq!(&bytes[0..4], b"SAB\0");
    assert_eq!(bytes[4], 4, "major version");
    assert_eq!(bytes[5], 0, "minor version");
    let mut pos = 6usize;
    let count = read_uleb128(bytes, &mut pos).expect("section count");
    assert_eq!(count, 4);
    let mut out = Vec::new();
    for _ in 0..count {
        let id = read_uleb128(bytes, &mut pos).expect("section id");
        let len = read_uleb128(bytes, &mut pos).expect("section len") as usize;
        out.push((id, &bytes[pos..pos + len]));
        pos += len;
    }
    assert_eq!(pos, bytes.len(), "sections must consume the module exactly");
    out
}

fn decode_pool(payload: &[u8]) -> Vec<String> {
    let mut pos = 0usize;
    let count = read_uleb128(payload, &mut pos).expect("pool count");
    let mut entries = Vec::new();
    for _ in 0..count {
        let len = read_uleb128(payload, &mut pos).expect("pool entry len") as usize;
        entries.push(String::from_utf8(payload[pos..pos + len].to_vec()).unwrap());
        pos += len;
    }
    entries
}

#[test]
fn fixture01_pool_order_and_structure() {
    let module = fixture01_module();

    // Symbol interning order produced by the builder.
    assert_eq!(
        module.symbols,
        ["main", "SLA_STR_0", "tmp_0", "test_hello_world", "tmp_1", "then_0", "else_1"]
    );

    // Pool = symbols, then instruction texts in order, then sig/const
    // texts (all duplicates of earlier entries except the literal).
    let pool = build_pool(&module);
    assert_eq!(
        pool.entries(),
        [
            "main",
            "SLA_STR_0",
            "tmp_0",
            "test_hello_world",
            "tmp_1",
            "then_0",
            "else_1",
            "@println(SLA_STR_0)",
            "@str_eq(SLA_STR_0, SLA_STR_0)",
            "101",
            "utf8:\"hello world\\0\"",
        ]
    );

    // The string const carries the trailing NUL, per appendUtf8Const.
    assert_eq!(module.const_decls.len(), 1);
    let decl = &module.const_decls[0];
    assert_eq!(decl.name, "SLA_STR_0");
    match &decl.value {
        ConstValue::Utf8(bytes) => assert_eq!(bytes.bytes, b"hello world\0"),
        other => panic!("expected utf8 const, got {other:?}"),
    }

    // Instruction shape.
    let kinds: Vec<InstKind> = module.instructions.iter().map(|i| i.kind).collect();
    assert_eq!(
        kinds,
        [
            InstKind::FuncDecl,
            InstKind::Call,
            InstKind::Assign,
            InstKind::Return,
            InstKind::TestDecl,
            InstKind::Call,
            InstKind::Br,
            InstKind::Label,
            InstKind::Panic,
            InstKind::Label,
            InstKind::Return,
        ]
    );

    // SAB-D13: the br instruction duplicates the then-label into
    // operands[2], exactly as the Zig emitBranch does today.
    let br = &module.instructions[6];
    assert_eq!(br.operands[0], Operand::Reg(4));
    assert_eq!(br.operands[1], Operand::Label(5));
    assert_eq!(br.operands[2], Operand::Label(5));
    assert_eq!(br.operands[3], Operand::Label(6));

    // Sigs: ids are interned name ids; entry points are decl indexes;
    // main recorded its return temp.
    assert_eq!(module.function_sigs[0].id, 0);
    assert_eq!(module.function_sigs[0].entry_inst_idx, 0);
    assert_eq!(module.function_sigs[0].reg_ids, [2]);
    assert_eq!(module.function_sigs[1].kind, FunctionKind::TestFunc);
    assert_eq!(module.function_sigs[1].entry_inst_idx, 4);
}

#[test]
fn fixture01_encodes_with_well_formed_sections() {
    let module = fixture01_module();
    let bytes = encode_module(&module).expect("encode fixture01 model");
    let secs = sections(&bytes);
    let ids: Vec<u64> = secs.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [1, 2, 3, 4]);
    let pool = decode_pool(secs[0].1);
    assert_eq!(pool.len(), 11);
    assert_eq!(pool[7], "@println(SLA_STR_0)");
    assert_eq!(pool[10], "utf8:\"hello world\\0\"");
    // Encoding is deterministic.
    let again = encode_module(&module).expect("re-encode");
    assert_eq!(bytes, again);
}

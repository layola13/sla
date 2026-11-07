//! F2 (instruction half) — the instruction-section reader of
//! sci `sab.zig`'s read path: `readOperand` (lines 357–387) and
//! `readInstructions` (lines 880–945), plus the ordinal decoders
//! for the instruction-level enums.
//!
//! Wire order per instruction (mirrors `writeInstructions`):
//! kind byte, source/expanded line, optional op kind, four
//! operands, the v2/v3 raw-text slot (dropped: raw text is not
//! modeled, SAB-D29), the atomic value type, the three atomic
//! optionals, and — only under full metadata (major >= 3) — the
//! atomic texts, native reg names, package identity, package
//! hash and upstream loc.

use crate::decode::{
    atomic_ordering_from_u8, atomic_rmw_op_from_u8, read_optional_alloc_text,
    read_optional_enum, read_optional_hash, read_optional_pooled_text,
    read_optional_upstream_loc, read_pooled_text, Cursor,
};
use crate::error::{Error, Result};
use crate::ir::*;

fn sab_err(name: &str) -> Error {
    Error::InvalidModule(format!("{name} [sci sab.zig decode]"))
}

/// Ordinal decoder for `InstKind` (instruction.zig order,
/// including the tail-appended variadic/gpu kinds, SAB-D01).
pub fn inst_kind_from_u8(v: u8) -> Option<InstKind> {
    Some(match v {
        0 => InstKind::Alloc,
        1 => InstKind::StackAlloc,
        2 => InstKind::Load,
        3 => InstKind::Store,
        4 => InstKind::AtomicLoad,
        5 => InstKind::AtomicStore,
        6 => InstKind::Cmpxchg,
        7 => InstKind::AtomicRmw,
        8 => InstKind::Fence,
        9 => InstKind::Borrow,
        10 => InstKind::Move,
        11 => InstKind::Release,
        12 => InstKind::Assign,
        13 => InstKind::Op,
        14 => InstKind::PtrAdd,
        15 => InstKind::Jmp,
        16 => InstKind::Br,
        17 => InstKind::BrNull,
        18 => InstKind::Call,
        19 => InstKind::CallIndirect,
        20 => InstKind::Try,
        21 => InstKind::EarlyReturn,
        22 => InstKind::Panic,
        23 => InstKind::PanicMsg,
        24 => InstKind::Return,
        25 => InstKind::Take,
        26 => InstKind::FuncDecl,
        27 => InstKind::FfiWrapperDecl,
        28 => InstKind::ExternDecl,
        29 => InstKind::ExportDecl,
        30 => InstKind::TestDecl,
        31 => InstKind::Label,
        32 => InstKind::RawCast,
        33 => InstKind::AssumeSafe,
        34 => InstKind::AssumeBorrow,
        35 => InstKind::Native,
        36 => InstKind::VaStart,
        37 => InstKind::VaArg,
        38 => InstKind::VaEnd,
        39 => InstKind::GpuDecl,
        _ => return None,
    })
}

/// Ordinal decoder for `OpKind` (instruction.zig order).
pub fn op_kind_from_u8(v: u8) -> Option<OpKind> {
    Some(match v {
        0 => OpKind::Add,
        1 => OpKind::Sub,
        2 => OpKind::Mul,
        3 => OpKind::Sdiv,
        4 => OpKind::Udiv,
        5 => OpKind::Srem,
        6 => OpKind::Urem,
        7 => OpKind::Neg,
        8 => OpKind::And,
        9 => OpKind::Or,
        10 => OpKind::Xor,
        11 => OpKind::Shl,
        12 => OpKind::Lshr,
        13 => OpKind::Ashr,
        14 => OpKind::Not,
        15 => OpKind::Eq,
        16 => OpKind::Ne,
        17 => OpKind::Slt,
        18 => OpKind::Sle,
        19 => OpKind::Sgt,
        20 => OpKind::Sge,
        21 => OpKind::Ult,
        22 => OpKind::Ule,
        23 => OpKind::Ugt,
        24 => OpKind::Uge,
        25 => OpKind::Div,
        26 => OpKind::Rem,
        27 => OpKind::Gt,
        28 => OpKind::Lt,
        29 => OpKind::Shr,
        30 => OpKind::Fadd,
        31 => OpKind::Fsub,
        32 => OpKind::Fmul,
        33 => OpKind::Fdiv,
        34 => OpKind::Fneg,
        35 => OpKind::FcmpEq,
        36 => OpKind::FcmpNe,
        37 => OpKind::FcmpLt,
        38 => OpKind::FcmpLe,
        39 => OpKind::FcmpGt,
        40 => OpKind::FcmpGe,
        41 => OpKind::Trunc,
        42 => OpKind::Zext,
        43 => OpKind::Sext,
        44 => OpKind::Fptosi,
        45 => OpKind::Sitofp,
        46 => OpKind::Uitofp,
        47 => OpKind::Fptrunc,
        48 => OpKind::Fpext,
        49 => OpKind::Bitcast,
        50 => OpKind::AddV128,
        51 => OpKind::SubV128,
        52 => OpKind::MulV128,
        53 => OpKind::ShuffleV128,
        54 => OpKind::ExtractLane,
        55 => OpKind::InsertLane,
        _ => return None,
    })
}

/// Ordinal decoder for `OpCode` (instruction.zig order).
pub fn op_code_from_u8(v: u8) -> Option<OpCode> {
    Some(match v {
        0 => OpCode::Add,
        1 => OpCode::Sub,
        2 => OpCode::Mul,
        3 => OpCode::Div,
        4 => OpCode::Sdiv,
        5 => OpCode::Udiv,
        6 => OpCode::Rem,
        7 => OpCode::Srem,
        8 => OpCode::Urem,
        9 => OpCode::Gt,
        10 => OpCode::Lt,
        11 => OpCode::Sgt,
        12 => OpCode::Slt,
        13 => OpCode::Sge,
        14 => OpCode::Sle,
        15 => OpCode::Ugt,
        16 => OpCode::Ult,
        17 => OpCode::Uge,
        18 => OpCode::Ule,
        19 => OpCode::Eq,
        20 => OpCode::Ne,
        21 => OpCode::And,
        22 => OpCode::Or,
        23 => OpCode::Shl,
        24 => OpCode::Shr,
        _ => return None,
    })
}

/// Zig `readOperand` (lines 357–387): tag byte, then the payload
/// in the tag's encoding. Text payloads are pool ids resolved
/// against the module's symbols.
pub fn read_operand(symbols: &[String], cursor: &mut Cursor) -> Result<Operand> {
    Ok(match cursor.read_byte()? {
        0 => Operand::None,
        1 => Operand::Reg(crate::decode::decode_uleb128(cursor)? as u32),
        2 => Operand::Symbol(crate::decode::decode_uleb128(cursor)? as u32),
        3 => Operand::Label(crate::decode::decode_uleb128(cursor)? as u32),
        4 => Operand::Func(crate::decode::decode_uleb128(cursor)? as u32),
        5 => Operand::ImmI64(crate::decode::decode_sleb128(cursor)?),
        6 => Operand::ImmU64(crate::decode::decode_uleb128(cursor)?),
        7 => Operand::ImmInt(crate::decode::decode_sleb128(cursor)? as i128),
        8 => {
            let raw = cursor.read_slice(8)?;
            let mut buf = [0u8; 8];
            buf.copy_from_slice(raw);
            Operand::ImmFloat(f64::from_bits(u64::from_le_bytes(buf)))
        }
        9 => Operand::OpCode(
            op_code_from_u8(cursor.read_byte()?).ok_or_else(|| sab_err("InvalidTag"))?,
        ),
        10 => Operand::CapPrefix(
            crate::decode::cap_prefix_from_u8(cursor.read_byte()?)
                .ok_or_else(|| sab_err("InvalidTag"))?,
        ),
        11 => Operand::Offset(crate::decode::decode_uleb128(cursor)? as u32),
        12 => Operand::Ty(crate::decode::decode_uleb128(cursor)? as u32),
        13 => Operand::Text(read_pooled_text(
            symbols,
            crate::decode::decode_uleb128(cursor)?,
        )?),
        14 => Operand::NativeText(read_pooled_text(
            symbols,
            crate::decode::decode_uleb128(cursor)?,
        )?),
        _ => return Err(sab_err("InvalidTag")),
    })
}

/// Zig `readInstructions` (lines 880–945). The expanded line is
/// the wire value, never the decode index (Zig comment at
/// 890–891: SA text is the semantic standard).
pub fn read_instructions(
    symbols: &[String],
    payload: &[u8],
    has_raw_text: bool,
    has_full_metadata: bool,
) -> Result<Vec<Instruction>> {
    let mut cursor = Cursor::new(payload);
    let count = crate::decode::decode_uleb128(&mut cursor)?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let kind = inst_kind_from_u8(cursor.read_byte()?)
            .ok_or_else(|| sab_err("InvalidTag"))?;
        let mut item = Instruction::new(
            kind,
            crate::decode::decode_uleb128(&mut cursor)? as u32,
            crate::decode::decode_uleb128(&mut cursor)? as u32,
        );
        item.op_kind = read_optional_enum(&mut cursor, op_kind_from_u8)?;
        for slot in item.operands.iter_mut() {
            *slot = read_operand(symbols, &mut cursor)?;
        }
        if has_raw_text {
            let raw_present = cursor.read_byte()?;
            if raw_present == 1 {
                // Consumed and dropped: raw_text is not modeled
                // in this port (SAB-D29).
                read_pooled_text(symbols, crate::decode::decode_uleb128(&mut cursor)?)?;
            } else if raw_present != 0 {
                return Err(sab_err("InvalidTag"));
            }
        }
        // FB-039: Zig reads the presence byte with
        // `(byte) == 1` — any other value (2..=255) silently
        // means "absent", unlike the strictly validated optional
        // fields around it. Mirrored verbatim.
        item.atomic_value_ty = if cursor.read_byte()? == 1 {
            Some(crate::decode::decode_uleb128(&mut cursor)? as u32)
        } else {
            None
        };
        item.atomic_ordering = read_optional_enum(&mut cursor, atomic_ordering_from_u8)?;
        item.atomic_second_ordering =
            read_optional_enum(&mut cursor, atomic_ordering_from_u8)?;
        item.atomic_rmw_op = read_optional_enum(&mut cursor, atomic_rmw_op_from_u8)?;
        if has_full_metadata {
            item.atomic_expected_text = read_optional_pooled_text(symbols, &mut cursor)?;
            item.atomic_new_text = read_optional_pooled_text(symbols, &mut cursor)?;
            let native_count = crate::decode::decode_uleb128(&mut cursor)?;
            let mut names = Vec::with_capacity(native_count as usize);
            for _ in 0..native_count {
                names.push(read_pooled_text(
                    symbols,
                    crate::decode::decode_uleb128(&mut cursor)?,
                )?);
            }
            item.native_reg_names = names;
            item.package_identity = read_optional_alloc_text(symbols, &mut cursor)?;
            item.package_source_sha256 = read_optional_hash(&mut cursor)?;
            item.upstream_loc = read_optional_upstream_loc(symbols, &mut cursor)?;
        }
        // Zig's `synthesize_debug_text` step writes only into
        // the unmodeled raw_text; skipped (SAB-D29).
        out.push(item);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::encode_module;

    #[test]
    fn operand_tags_round_trip_through_module() {
        let mut inst = Instruction::new(InstKind::Op, 3, 5);
        inst.op_kind = Some(OpKind::Add);
        inst.operands = [
            Operand::Reg(1),
            Operand::ImmI64(-42),
            Operand::ImmFloat(2.5),
            Operand::OpCode(OpCode::Shr),
        ];
        let module = Module {
            symbols: vec!["main".into(), "value".into()],
            function_sigs: vec![],
            const_decls: vec![],
            instructions: vec![inst],
        };
        let bytes = encode_module(&module).unwrap();
        let decoded = crate::decode_module(&bytes).unwrap();
        let got = &decoded.instructions[0];
        assert_eq!(got.kind, InstKind::Op);
        assert_eq!(got.source_line, 3);
        assert_eq!(got.expanded_line, 5);
        assert_eq!(got.op_kind, Some(OpKind::Add));
        assert_eq!(got.operands, module.instructions[0].operands);
    }

    #[test]
    fn invalid_kind_byte_is_invalid_tag() {
        // Hand-build a minimal stream: pool empty, one instruction
        // with kind byte 200.
        let mut payload = vec![1u8]; // instruction count
        payload.push(200);
        let err = read_instructions(&[], &payload, false, true).unwrap_err();
        assert!(err.to_string().contains("InvalidTag"), "{err}");
    }
}

//! The SAB v4 serializer: `Module` -> bytes.
//!
//! Faithful port of the write path of sci `sab.zig`:
//! `encodeProgramWithConsts` (assembly + section order),
//! `writeSymbolPool`, `writeFunctionSigs`, `writeConstDecls`,
//! `writeInstructions`, `writeInstruction`, `writeFunctionSig`,
//! `writeConstDecl`, `writeConstValue`, `writeBytesLiteral`,
//! `writeOperand`, and the `writeOptional*` helpers.
//!
//! Wire layout reminder: header = magic `SAB\0`, major 4, minor 0,
//! uleb128 section count (4); then sections in id order
//! 1 = symbol pool, 2 = function sigs, 3 = const decls,
//! 4 = instructions; each section is uleb(id) + uleb(len) + payload.
//!
//! Byte-parity criterion for this port: for the same logical module
//! (same symbol order, sigs, const decls and instructions), the output
//! is byte-identical to the Zig encoder's. Pool construction order is
//! part of the criterion and lives in `pool.rs`.

use crate::error::{Error, Result};
use crate::ir::*;
use crate::leb128::{write_sleb128, write_uleb128};
use crate::pool::{build_pool, StringPool};

const SAB_MAGIC: &[u8; 4] = b"SAB\0";
const SAB_MAJOR: u8 = 4;
const SAB_MINOR: u8 = 0;

fn pool_id(pool: &StringPool, text: &str) -> Result<u32> {
    pool.id_of(text)
        .ok_or_else(|| Error::InvalidModule(format!("string missing from pool: {text:?}")))
}

fn write_section(out: &mut Vec<u8>, id: u64, payload: &[u8]) {
    write_uleb128(out, id);
    write_uleb128(out, payload.len() as u64);
    out.extend_from_slice(payload);
}

fn write_optional_pool_text(out: &mut Vec<u8>, pool: &StringPool, text: &Option<String>) -> Result<()> {
    match text {
        Some(value) => {
            out.push(1);
            write_uleb128(out, pool_id(pool, value)? as u64);
        }
        None => out.push(0),
    }
    Ok(())
}

fn write_optional_enum<T: Copy>(out: &mut Vec<u8>, value: &Option<T>, ordinal: fn(T) -> u8) {
    match value {
        Some(v) => {
            out.push(1);
            out.push(ordinal(*v));
        }
        None => out.push(0),
    }
}

fn write_optional_upstream_loc(
    out: &mut Vec<u8>,
    pool: &StringPool,
    loc: &Option<UpstreamLoc>,
) -> Result<()> {
    match loc {
        Some(value) => {
            out.push(1);
            write_uleb128(out, pool_id(pool, &value.file)? as u64);
            write_uleb128(out, value.line as u64);
            write_uleb128(out, value.col as u64);
        }
        None => out.push(0),
    }
    Ok(())
}

/// Signed LEB128 for the full i128 range (the Zig `imm_int` payload is
/// an i128; values inside i64 range encode identically to
/// `write_sleb128`).
fn write_sleb128_i128(out: &mut Vec<u8>, value: i128) {
    let mut rest = value;
    loop {
        let mut byte = (rest & 0x7f) as u8;
        rest >>= 7;
        let sign_bit_set = byte & 0x40 != 0;
        let done = (rest == 0 && !sign_bit_set) || (rest == -1 && sign_bit_set);
        if !done {
            byte |= 0x80;
        }
        out.push(byte);
        if done {
            break;
        }
    }
}

fn write_operand(out: &mut Vec<u8>, pool: &StringPool, operand: &Operand) -> Result<()> {
    out.push(operand.tag());
    match operand {
        Operand::None => {}
        Operand::Reg(v) | Operand::Symbol(v) | Operand::Label(v) | Operand::Func(v) => {
            write_uleb128(out, *v as u64);
        }
        Operand::ImmI64(v) => write_sleb128(out, *v),
        Operand::ImmU64(v) => write_uleb128(out, *v),
        Operand::ImmInt(v) => write_sleb128_i128(out, *v),
        Operand::ImmFloat(v) => out.extend_from_slice(&v.to_bits().to_le_bytes()),
        Operand::OpCode(v) => out.push(*v as u8),
        Operand::CapPrefix(v) => out.push(*v as u8),
        Operand::Offset(v) => write_uleb128(out, *v as u64),
        Operand::Ty(v) => write_uleb128(out, *v as u64),
        Operand::Text(text) | Operand::NativeText(text) => {
            write_uleb128(out, pool_id(pool, text)? as u64);
        }
    }
    Ok(())
}

fn write_instruction(out: &mut Vec<u8>, pool: &StringPool, inst: &Instruction) -> Result<()> {
    out.push(inst.kind as u8);
    write_uleb128(out, inst.source_line as u64);
    write_uleb128(out, inst.expanded_line as u64);
    match &inst.op_kind {
        Some(op) => {
            out.push(1);
            out.push(*op as u8);
        }
        None => out.push(0),
    }
    for operand in &inst.operands {
        write_operand(out, pool, operand)?;
    }
    match &inst.atomic_value_ty {
        Some(ty) => {
            out.push(1);
            write_uleb128(out, *ty as u64);
        }
        None => out.push(0),
    }
    write_optional_enum(out, &inst.atomic_ordering, |v| v as u8);
    write_optional_enum(out, &inst.atomic_second_ordering, |v| v as u8);
    write_optional_enum(out, &inst.atomic_rmw_op, |v| v as u8);
    write_optional_pool_text(out, pool, &inst.atomic_expected_text)?;
    write_optional_pool_text(out, pool, &inst.atomic_new_text)?;
    write_uleb128(out, inst.native_reg_names.len() as u64);
    for name in &inst.native_reg_names {
        write_uleb128(out, pool_id(pool, name)? as u64);
    }
    write_optional_pool_text(out, pool, &inst.package_identity)?;
    match &inst.package_source_sha256 {
        Some(hash) => {
            out.push(1);
            out.extend_from_slice(hash);
        }
        None => out.push(0),
    }
    write_optional_upstream_loc(out, pool, &inst.upstream_loc)?;
    Ok(())
}

fn write_function_sig(out: &mut Vec<u8>, pool: &StringPool, sig: &FunctionSig) -> Result<()> {
    write_uleb128(out, sig.id as u64);
    write_uleb128(out, pool_id(pool, &sig.name)? as u64);
    out.push(sig.kind as u8);
    match &sig.return_cap {
        Some(cap) => {
            out.push(1);
            out.push(*cap as u8);
        }
        None => out.push(0),
    }
    out.push(sig.return_ty as u8);
    out.push(sig.return_fallible as u8);
    write_uleb128(out, sig.entry_inst_idx as u64);
    out.push(sig.is_ffi_wrapper as u8);
    out.push(sig.ignored as u8);
    out.push(sig.should_panic as u8);
    write_optional_pool_text(out, pool, &sig.upstream_file)?;
    write_optional_upstream_loc(out, pool, &sig.upstream_loc)?;
    write_optional_pool_text(out, pool, &sig.llvm_name)?;
    write_uleb128(out, sig.params.len() as u64);
    for param in &sig.params {
        write_uleb128(out, pool_id(pool, &param.name)? as u64);
        out.push(param.ty as u8);
        out.push(param.cap as u8);
    }
    write_uleb128(out, sig.param_ids.len() as u64);
    for id in &sig.param_ids {
        write_uleb128(out, *id as u64);
    }
    write_uleb128(out, sig.reg_ids.len() as u64);
    for id in &sig.reg_ids {
        write_uleb128(out, *id as u64);
    }
    Ok(())
}

fn write_bytes_literal(out: &mut Vec<u8>, literal: &BytesLiteral) {
    write_uleb128(out, literal.bytes.len() as u64);
    out.extend_from_slice(&literal.bytes);
    match &literal.repeat {
        Some((count, byte)) => {
            out.push(1);
            write_uleb128(out, *count as u64);
            out.push(*byte);
        }
        None => out.push(0),
    }
}

fn write_const_value(out: &mut Vec<u8>, pool: &StringPool, value: &ConstValue) -> Result<()> {
    out.push(value.tag());
    match value {
        ConstValue::Hex(bytes) | ConstValue::Utf8(bytes) | ConstValue::Repeat(bytes) => {
            write_bytes_literal(out, bytes);
        }
        ConstValue::Struct(fields) => {
            write_uleb128(out, fields.len() as u64);
            for field in fields {
                write_uleb128(out, pool_id(pool, &field.name)? as u64);
                write_uleb128(out, field.size as u64);
                write_const_value(out, pool, &field.value)?;
            }
        }
        ConstValue::VTable(slots) => {
            write_uleb128(out, slots.len() as u64);
            for slot in slots {
                write_uleb128(out, pool_id(pool, &slot.name)? as u64);
                write_uleb128(out, pool_id(pool, &slot.func_name)? as u64);
            }
        }
    }
    Ok(())
}

fn write_const_decl(out: &mut Vec<u8>, pool: &StringPool, decl: &ConstDecl) -> Result<()> {
    write_uleb128(out, decl.source_line as u64);
    write_uleb128(out, decl.expanded_line as u64);
    write_uleb128(out, pool_id(pool, &decl.name)? as u64);
    write_uleb128(out, pool_id(pool, &decl.literal_text)? as u64);
    write_optional_upstream_loc(out, pool, &decl.upstream_loc)?;
    write_const_value(out, pool, &decl.value)?;
    Ok(())
}

/// Serialize `module` to SAB v4 bytes, byte-identical to the Zig
/// `sab.encodeProgramWithConsts` for the same logical module.
pub fn encode_module(module: &Module) -> Result<Vec<u8>> {
    let pool = build_pool(module);

    let mut pool_payload = Vec::new();
    write_uleb128(&mut pool_payload, pool.len() as u64);
    for entry in pool.entries() {
        write_uleb128(&mut pool_payload, entry.len() as u64);
        pool_payload.extend_from_slice(entry.as_bytes());
    }

    let mut sigs_payload = Vec::new();
    write_uleb128(&mut sigs_payload, module.function_sigs.len() as u64);
    for sig in &module.function_sigs {
        write_function_sig(&mut sigs_payload, &pool, sig)?;
    }

    let mut consts_payload = Vec::new();
    write_uleb128(&mut consts_payload, module.const_decls.len() as u64);
    for decl in &module.const_decls {
        write_const_decl(&mut consts_payload, &pool, decl)?;
    }

    let mut insts_payload = Vec::new();
    write_uleb128(&mut insts_payload, module.instructions.len() as u64);
    for inst in &module.instructions {
        write_instruction(&mut insts_payload, &pool, inst)?;
    }

    let mut out = Vec::new();
    out.extend_from_slice(SAB_MAGIC);
    out.push(SAB_MAJOR);
    out.push(SAB_MINOR);
    write_uleb128(&mut out, 4);
    write_section(&mut out, 1, &pool_payload);
    write_section(&mut out, 2, &sigs_payload);
    write_section(&mut out, 3, &consts_payload);
    write_section(&mut out, 4, &insts_payload);
    Ok(out)
}

//! F1/F2 — the SAB byte decoder: a faithful port of sci
//! `sab.zig`'s read path (`decodeModule`, lines 1008–1041, and its
//! section readers, lines 56–1007 of the current-head sci tree).
//!
//! Zig's `decodeModule` is the entry point of the whole F group:
//! std macro fragments and imported std modules reach the codegen
//! as SAB bytes (produced by the snippet flattener + encoder), and
//! every fragment append starts by decoding them back into a
//! module — symbols (macros/imports surface), function sigs
//! (signatures), const decls (resident globals) and instructions
//! (resident bodies).
//!
//! Porting notes:
//! - Zig's decoded `Module.symbols` are *borrowed* slices of the
//!   input bytes (`owns_symbol_text = false`); this port clones
//!   them into owned `String`s. Ownership form only — no decoded
//!   value differs.
//! - Zig's `Instruction.raw_text` / `ConstDecl.raw_text` are not
//!   modeled in this crate's IR (see `ir.rs`): the v2/v3 raw-text
//!   slot is consumed and dropped, the const raw text (which Zig
//!   re-synthesizes as `@const {name} = {literal_text}`) is not
//!   stored, and the v<4 debug-text synthesis
//!   (`synthesizeRawText`) is skipped — it writes only into
//!   `raw_text`. Registered as SAB-D29.
//! - Error names are Zig's verbatim (`TruncatedSab`,
//!   `InvalidSabMagic`, `UnsupportedSabVersion`, `Leb128Overflow`,
//!   `InvalidTag`, `MissingInstructionSection`), carried in
//!   `Error::InvalidModule` — the encoder-side variant this crate
//!   already uses for sci `sab.zig` failures.
//! - Two lax-byte quirks of the Zig readers are mirrored exactly
//!   and registered as feedback candidates (FB-039/FB-040): the
//!   boolean fields of a function sig and the instructions'
//!   `atomic_value_ty` presence byte accept *any* non-1 byte as
//!   "absent/false" without the `InvalidTag` validation the
//!   neighbouring optional fields perform.

use crate::error::{Error, Result};
use crate::ir::*;

/// Zig `sab.zig` `magic` / `version_major`.
pub const SAB_MAGIC: &[u8; 4] = b"SAB\x00";
pub const SAB_VERSION_MAJOR: u8 = 4;

fn sab_err(name: &str) -> Error {
    Error::InvalidModule(format!("{name} [sci sab.zig decode]"))
}

/// Zig `Cursor` (sab.zig lines 56–76): a byte slice plus index;
/// every overrun is `TruncatedSab`.
#[derive(Debug, Clone, Copy)]
pub struct Cursor<'a> {
    pub bytes: &'a [u8],
    pub index: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Cursor { bytes, index: 0 }
    }

    /// Zig `Cursor.readByte`.
    pub fn read_byte(&mut self) -> Result<u8> {
        if self.index >= self.bytes.len() {
            return Err(sab_err("TruncatedSab"));
        }
        let byte = self.bytes[self.index];
        self.index += 1;
        Ok(byte)
    }

    /// Zig `Cursor.readSlice`.
    pub fn read_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        if len > self.bytes.len() - self.index {
            return Err(sab_err("TruncatedSab"));
        }
        let start = self.index;
        self.index += len;
        Ok(&self.bytes[start..self.index])
    }
}

/// Zig `decodeUleb128` (sab.zig lines 86–97): the overflow check
/// fires when the shift *before* an increment has reached 63, so
/// at most ten bytes are consumed and high bits shifted past 63
/// are silently dropped, exactly as in Zig.
pub fn decode_uleb128(cursor: &mut Cursor) -> Result<u64> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = cursor.read_byte()?;
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        if shift >= 63 {
            return Err(sab_err("Leb128Overflow"));
        }
        shift += 7;
    }
}

/// Zig `decodeSleb128` (sab.zig lines 111–127).
pub fn decode_sleb128(cursor: &mut Cursor) -> Result<i64> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    let mut byte: u8;
    loop {
        byte = cursor.read_byte()?;
        result |= ((byte & 0x7f) as u64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break;
        }
        if shift >= 63 {
            return Err(sab_err("Leb128Overflow"));
        }
    }
    let mut result = result as i64;
    if shift < 64 && (byte & 0x40) != 0 {
        result |= -(1i64 << shift);
    }
    Ok(result)
}

/// Zig `readStringPool` / `readStringPoolBorrowed` (lines
/// 142–176): count, then per string a length and its bytes. The
/// two Zig variants differ only in ownership (dupe vs borrow);
/// this port always owns.
pub fn read_string_pool(payload: &[u8]) -> Result<Vec<String>> {
    let mut cursor = Cursor::new(payload);
    let count = decode_uleb128(&mut cursor)?;
    let mut symbols = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let len = decode_uleb128(&mut cursor)?;
        let raw = cursor.read_slice(len as usize)?;
        symbols.push(String::from_utf8_lossy(raw).into_owned());
    }
    Ok(symbols)
}

/// Zig `readPooledText` (lines 251–262): the pooled string at
/// `id`; out-of-range ids are `InvalidTag`.
pub fn read_pooled_text(symbols: &[String], id: u64) -> Result<String> {
    let idx = id as usize;
    if id > usize::MAX as u64 || idx >= symbols.len() {
        return Err(sab_err("InvalidTag"));
    }
    Ok(symbols[idx].clone())
}

/// Zig `readSymbolName` (lines 674–679): same lookup as
/// `readPooledText`, used for sig/const names.
pub fn read_symbol_name(symbols: &[String], id: u64) -> Result<String> {
    read_pooled_text(symbols, id)
}

/// Zig `readOptionalPooledText` (lines 294–299).
pub fn read_optional_pooled_text(
    symbols: &[String],
    cursor: &mut Cursor,
) -> Result<Option<String>> {
    let present = cursor.read_byte()?;
    if present == 0 {
        return Ok(None);
    }
    if present != 1 {
        return Err(sab_err("InvalidTag"));
    }
    Ok(Some(read_pooled_text(symbols, decode_uleb128(cursor)?)?))
}

/// Zig `readOptionalAllocText` (lines 301–306): identical wire
/// shape to `readOptionalPooledText` in this port (the Zig split
/// is ownership bookkeeping only).
pub fn read_optional_alloc_text(
    symbols: &[String],
    cursor: &mut Cursor,
) -> Result<Option<String>> {
    read_optional_pooled_text(symbols, cursor)
}

/// Zig `skipOptionalPoolText` (lines 308–313).
pub fn skip_optional_pool_text(cursor: &mut Cursor) -> Result<()> {
    let present = cursor.read_byte()?;
    if present == 0 {
        return Ok(());
    }
    if present != 1 {
        return Err(sab_err("InvalidTag"));
    }
    decode_uleb128(cursor)?;
    Ok(())
}

/// Zig `readOptionalHash` (lines 315–322).
pub fn read_optional_hash(cursor: &mut Cursor) -> Result<Option<[u8; 32]>> {
    let present = cursor.read_byte()?;
    if present == 0 {
        return Ok(None);
    }
    if present != 1 {
        return Err(sab_err("InvalidTag"));
    }
    let raw = cursor.read_slice(32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(raw);
    Ok(Some(out))
}

/// Zig `readOptionalPooledUpstreamLoc` /
/// `readOptionalAllocUpstreamLoc` (lines 324–349).
pub fn read_optional_upstream_loc(
    symbols: &[String],
    cursor: &mut Cursor,
) -> Result<Option<UpstreamLoc>> {
    let present = cursor.read_byte()?;
    if present == 0 {
        return Ok(None);
    }
    if present != 1 {
        return Err(sab_err("InvalidTag"));
    }
    Ok(Some(UpstreamLoc {
        file: read_pooled_text(symbols, decode_uleb128(cursor)?)?,
        line: decode_uleb128(cursor)? as u32,
        col: decode_uleb128(cursor)? as u32,
    }))
}

/// Zig `skipOptionalUpstreamLoc` (lines 351–355).
pub fn skip_optional_upstream_loc(cursor: &mut Cursor) -> Result<()> {
    let present = cursor.read_byte()?;
    if present == 0 {
        return Ok(());
    }
    if present != 1 {
        return Err(sab_err("InvalidTag"));
    }
    decode_uleb128(cursor)?;
    decode_uleb128(cursor)?;
    decode_uleb128(cursor)?;
    Ok(())
}

/// Zig `readOptionalEnum` (lines 182–187) for the small enums,
/// via their ordinal decoders.
pub fn read_optional_enum<T>(
    cursor: &mut Cursor,
    from_byte: fn(u8) -> Option<T>,
) -> Result<Option<T>> {
    let present = cursor.read_byte()?;
    if present == 0 {
        return Ok(None);
    }
    if present != 1 {
        return Err(sab_err("InvalidTag"));
    }
    Ok(Some(from_byte(cursor.read_byte()?).ok_or_else(|| sab_err("InvalidTag"))?))
}

/// Ordinal decoder for `CapPrefix` (instruction.zig order).
pub fn cap_prefix_from_u8(v: u8) -> Option<CapPrefix> {
    Some(match v {
        0 => CapPrefix::ByValue,
        1 => CapPrefix::Borrow,
        2 => CapPrefix::Move,
        3 => CapPrefix::Raw,
        _ => return None,
    })
}

/// Ordinal decoder for `PrimType` (instruction.zig order).
pub fn prim_type_from_u8(v: u8) -> Option<PrimType> {
    Some(match v {
        0 => PrimType::Void,
        1 => PrimType::I1,
        2 => PrimType::I8,
        3 => PrimType::I16,
        4 => PrimType::I32,
        5 => PrimType::I64,
        6 => PrimType::U8,
        7 => PrimType::U16,
        8 => PrimType::U32,
        9 => PrimType::U64,
        10 => PrimType::F32,
        11 => PrimType::F64,
        12 => PrimType::Ptr,
        13 => PrimType::BlobHandle,
        14 => PrimType::V128,
        _ => return None,
    })
}

/// Ordinal decoder for `FunctionKind` (signature.zig order).
pub fn function_kind_from_u8(v: u8) -> Option<FunctionKind> {
    Some(match v {
        0 => FunctionKind::Normal,
        1 => FunctionKind::FfiWrapper,
        2 => FunctionKind::External,
        3 => FunctionKind::Exported,
        4 => FunctionKind::TestFunc,
        5 => FunctionKind::Gpu,
        _ => return None,
    })
}

/// Ordinal decoder for `AtomicOrdering` (atomic.zig order).
pub fn atomic_ordering_from_u8(v: u8) -> Option<AtomicOrdering> {
    Some(match v {
        0 => AtomicOrdering::Relaxed,
        1 => AtomicOrdering::Acquire,
        2 => AtomicOrdering::Release,
        3 => AtomicOrdering::AcqRel,
        4 => AtomicOrdering::SeqCst,
        _ => return None,
    })
}

/// Ordinal decoder for `AtomicRmwOp` (atomic.zig order).
pub fn atomic_rmw_op_from_u8(v: u8) -> Option<AtomicRmwOp> {
    Some(match v {
        0 => AtomicRmwOp::Add,
        1 => AtomicRmwOp::Sub,
        2 => AtomicRmwOp::And,
        3 => AtomicRmwOp::Or,
        4 => AtomicRmwOp::Xor,
        5 => AtomicRmwOp::Xchg,
        6 => AtomicRmwOp::Min,
        7 => AtomicRmwOp::Max,
        8 => AtomicRmwOp::Umin,
        9 => AtomicRmwOp::Umax,
        _ => return None,
    })
}

/// Zig `readBytesLiteral` (lines 512–528). The Zig value also
/// carries the literal kind as a field; this crate's
/// `BytesLiteral` drops it (the kind is the `ConstValue` variant).
fn read_bytes_literal(cursor: &mut Cursor) -> Result<BytesLiteral> {
    let len = decode_uleb128(cursor)?;
    let bytes = cursor.read_slice(len as usize)?.to_vec();
    let repeat_present = cursor.read_byte()?;
    if repeat_present == 0 {
        return Ok(BytesLiteral { bytes, repeat: None });
    }
    if repeat_present != 1 {
        return Err(sab_err("InvalidTag"));
    }
    let count = decode_uleb128(cursor)?;
    let byte = cursor.read_byte()?;
    Ok(BytesLiteral {
        bytes,
        repeat: Some((count, byte)),
    })
}

/// Zig `readConstValue` (lines 553–605), recursive.
pub fn read_const_value(symbols: &[String], cursor: &mut Cursor) -> Result<ConstValue> {
    let tag = cursor.read_byte()?;
    Ok(match tag {
        0 => ConstValue::Hex(read_bytes_literal(cursor)?),
        1 => ConstValue::Utf8(read_bytes_literal(cursor)?),
        2 => ConstValue::Repeat(read_bytes_literal(cursor)?),
        3 => {
            let count = decode_uleb128(cursor)?;
            let mut fields = Vec::with_capacity(count as usize);
            for _ in 0..count {
                fields.push(StructField {
                    name: read_symbol_name(symbols, decode_uleb128(cursor)?)?,
                    size: decode_uleb128(cursor)? as u32,
                    value: read_const_value(symbols, cursor)?,
                });
            }
            ConstValue::Struct(fields)
        }
        4 => {
            let count = decode_uleb128(cursor)?;
            let mut slots = Vec::with_capacity(count as usize);
            for _ in 0..count {
                slots.push(VTableSlot {
                    name: read_symbol_name(symbols, decode_uleb128(cursor)?)?,
                    func_name: read_symbol_name(symbols, decode_uleb128(cursor)?)?,
                });
            }
            ConstValue::VTable(slots)
        }
        _ => return Err(sab_err("InvalidTag")),
    })
}

/// Zig `readConstDecls` (lines 607–672). With `has_metadata`
/// false (SAB major < 3) the upstream loc is absent from the
/// wire and stays `None`.
pub fn read_const_decls(
    symbols: &[String],
    payload: &[u8],
    has_metadata: bool,
) -> Result<Vec<ConstDecl>> {
    let mut cursor = Cursor::new(payload);
    let count = decode_uleb128(&mut cursor)?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let source_line = decode_uleb128(&mut cursor)? as u32;
        let expanded_line = decode_uleb128(&mut cursor)? as u32;
        let name = read_symbol_name(symbols, decode_uleb128(&mut cursor)?)?;
        let literal_text = read_symbol_name(symbols, decode_uleb128(&mut cursor)?)?;
        // Zig synthesizes `raw_text = "@const {name} = {literal}"`
        // here; raw_text is not modeled in this port (SAB-D29).
        let upstream_loc = if has_metadata {
            read_optional_upstream_loc(symbols, &mut cursor)?
        } else {
            None
        };
        let value = read_const_value(symbols, &mut cursor)?;
        out.push(ConstDecl {
            source_line,
            expanded_line,
            upstream_loc,
            name,
            literal_text,
            value,
        });
    }
    Ok(out)
}


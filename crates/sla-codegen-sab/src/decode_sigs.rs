//! F2 (signature half) — the function-sig readers and the
//! public decode entry points of sci `sab.zig`'s read path:
//! `readFunctionSigs` / `readTestFunctionSigs` (lines 681–878),
//! the section scanner, and `decodeModule` /
//! `decodeFunctionSigsOnly` / `decodeTestFunctionSigsOnly`
//! (lines 1008–1107). The cursor/pool/const readers live in
//! `decode.rs`; the instruction reader in `decode_inst.rs`.

use crate::decode::{
    cap_prefix_from_u8, decode_uleb128, function_kind_from_u8, prim_type_from_u8,
    read_const_decls, read_optional_alloc_text, read_optional_upstream_loc,
    read_string_pool, read_symbol_name, skip_optional_pool_text, skip_optional_upstream_loc,
    Cursor, SAB_MAGIC, SAB_VERSION_MAJOR,
};
use crate::error::{Error, Result};
use crate::ir::*;

fn sab_err(name: &str) -> Error {
    Error::InvalidModule(format!("{name} [sci sab.zig decode]"))
}

/// The fixed head of one sig record, shared by `readFunctionSigs`
/// and `readTestFunctionSigs` (the two Zig readers duplicate it
/// verbatim).
struct SigHead {
    id: u32,
    name_id: u64,
    kind: FunctionKind,
    return_cap: Option<CapPrefix>,
    return_ty: PrimType,
    return_fallible: bool,
    entry_inst_idx: u32,
    is_ffi_wrapper: bool,
    ignored: bool,
    should_panic: bool,
    is_variadic: bool,
}

fn read_sig_head(cursor: &mut Cursor) -> Result<SigHead> {
    let id = decode_uleb128(cursor)? as u32;
    let name_id = decode_uleb128(cursor)?;
    let kind = function_kind_from_u8(cursor.read_byte()?)
        .ok_or_else(|| sab_err("InvalidTag"))?;
    let return_cap_present = cursor.read_byte()?;
    let return_cap = if return_cap_present == 1 {
        Some(cap_prefix_from_u8(cursor.read_byte()?).ok_or_else(|| sab_err("InvalidTag"))?)
    } else if return_cap_present == 0 {
        None
    } else {
        return Err(sab_err("InvalidTag"));
    };
    let return_ty =
        prim_type_from_u8(cursor.read_byte()?).ok_or_else(|| sab_err("InvalidTag"))?;
    // FB-040: the four boolean bytes below are read with Zig's
    // `(byte) == 1` laxity — any other value, not just 0, means
    // false, with no InvalidTag. Mirrored verbatim.
    let return_fallible = cursor.read_byte()? == 1;
    let entry_inst_idx = decode_uleb128(cursor)? as u32;
    let is_ffi_wrapper = cursor.read_byte()? == 1;
    let ignored = cursor.read_byte()? == 1;
    let should_panic = cursor.read_byte()? == 1;
    let is_variadic = cursor.read_byte()? == 1;
    Ok(SigHead {
        id,
        name_id,
        kind,
        return_cap,
        return_ty,
        return_fallible,
        entry_inst_idx,
        is_ffi_wrapper,
        ignored,
        should_panic,
        is_variadic,
    })
}

fn read_sig_params(
    symbols: &[String],
    cursor: &mut Cursor,
) -> Result<Vec<ParamSpec>> {
    let count = decode_uleb128(cursor)?;
    let mut params = Vec::with_capacity(count as usize);
    for _ in 0..count {
        params.push(ParamSpec {
            name: read_symbol_name(symbols, decode_uleb128(cursor)?)?,
            ty: prim_type_from_u8(cursor.read_byte()?)
                .ok_or_else(|| sab_err("InvalidTag"))?,
            cap: cap_prefix_from_u8(cursor.read_byte()?)
                .ok_or_else(|| sab_err("InvalidTag"))?,
        });
    }
    Ok(params)
}

fn read_id_list(cursor: &mut Cursor) -> Result<Vec<u32>> {
    let count = decode_uleb128(cursor)?;
    let mut ids = Vec::with_capacity(count as usize);
    for _ in 0..count {
        ids.push(decode_uleb128(cursor)? as u32);
    }
    Ok(ids)
}

/// Zig `readFunctionSigs` (lines 681–770).
pub fn read_function_sigs(
    symbols: &[String],
    payload: &[u8],
    has_metadata: bool,
) -> Result<Vec<FunctionSig>> {
    let mut cursor = Cursor::new(payload);
    let count = decode_uleb128(&mut cursor)?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let head = read_sig_head(&mut cursor)?;
        let (upstream_file, upstream_loc, llvm_name) = if has_metadata {
            (
                read_optional_alloc_text(symbols, &mut cursor)?,
                read_optional_upstream_loc(symbols, &mut cursor)?,
                read_optional_alloc_text(symbols, &mut cursor)?,
            )
        } else {
            (None, None, None)
        };
        let params = read_sig_params(symbols, &mut cursor)?;
        let param_ids = read_id_list(&mut cursor)?;
        let reg_ids = read_id_list(&mut cursor)?;
        out.push(FunctionSig {
            id: head.id,
            name: read_symbol_name(symbols, head.name_id)?,
            params,
            kind: head.kind,
            return_cap: head.return_cap,
            return_ty: head.return_ty,
            return_fallible: head.return_fallible,
            entry_inst_idx: head.entry_inst_idx,
            is_ffi_wrapper: head.is_ffi_wrapper,
            upstream_file,
            upstream_loc,
            param_ids,
            reg_ids,
            llvm_name,
            ignored: head.ignored,
            should_panic: head.should_panic,
            is_variadic: head.is_variadic,
        });
    }
    Ok(out)
}

/// Zig `readTestFunctionSigs` (lines 772–878): walks every sig
/// record but keeps only `test_func` ones, with params and id
/// lists *skipped* on the wire (kept sigs get empty lists) and
/// metadata read only for test sigs (skipped for the rest).
pub fn read_test_function_sigs(
    symbols: &[String],
    payload: &[u8],
    has_metadata: bool,
) -> Result<Vec<FunctionSig>> {
    let mut cursor = Cursor::new(payload);
    let count = decode_uleb128(&mut cursor)?;
    let mut out = Vec::new();
    for _ in 0..count {
        let head = read_sig_head(&mut cursor)?;
        let is_test = head.kind == FunctionKind::TestFunc;
        let (upstream_file, upstream_loc, llvm_name) = if has_metadata {
            if is_test {
                (
                    read_optional_alloc_text(symbols, &mut cursor)?,
                    read_optional_upstream_loc(symbols, &mut cursor)?,
                    read_optional_alloc_text(symbols, &mut cursor)?,
                )
            } else {
                skip_optional_pool_text(&mut cursor)?;
                skip_optional_upstream_loc(&mut cursor)?;
                skip_optional_pool_text(&mut cursor)?;
                (None, None, None)
            }
        } else {
            (None, None, None)
        };
        // Params and id lists are consumed but discarded for
        // every record, test or not.
        let param_count = decode_uleb128(&mut cursor)?;
        for _ in 0..param_count {
            decode_uleb128(&mut cursor)?;
            cursor.read_byte()?;
            cursor.read_byte()?;
        }
        let param_id_count = decode_uleb128(&mut cursor)?;
        for _ in 0..param_id_count {
            decode_uleb128(&mut cursor)?;
        }
        let reg_id_count = decode_uleb128(&mut cursor)?;
        for _ in 0..reg_id_count {
            decode_uleb128(&mut cursor)?;
        }
        if !is_test {
            continue;
        }
        out.push(FunctionSig {
            id: head.id,
            name: read_symbol_name(symbols, head.name_id)?,
            params: Vec::new(),
            kind: head.kind,
            return_cap: head.return_cap,
            return_ty: head.return_ty,
            return_fallible: head.return_fallible,
            entry_inst_idx: head.entry_inst_idx,
            is_ffi_wrapper: head.is_ffi_wrapper,
            upstream_file,
            upstream_loc,
            param_ids: Vec::new(),
            reg_ids: Vec::new(),
            llvm_name,
            ignored: head.ignored,
            should_panic: head.should_panic,
            is_variadic: head.is_variadic,
        });
    }
    Ok(out)
}

/// The section frame of a SAB byte stream, shared by the three
/// public decode entry points: validates magic + version and
/// hands each section payload to `visit` in stream order (Zig
/// applies the sections in order; a repeated section id simply
/// overwrites the earlier value — mirrored in `decode_module`).
fn scan_sections<'a>(
    bytes: &'a [u8],
    mut visit: impl FnMut(u64, &'a [u8], bool) -> Result<()>,
) -> Result<()> {
    if bytes.len() < SAB_MAGIC.len() + 2 {
        return Err(sab_err("TruncatedSab"));
    }
    if &bytes[0..SAB_MAGIC.len()] != SAB_MAGIC {
        return Err(sab_err("InvalidSabMagic"));
    }
    let mut cursor = Cursor {
        bytes,
        index: SAB_MAGIC.len(),
    };
    let major = cursor.read_byte()?;
    let _minor = cursor.read_byte()?;
    if major != 1 && major != 2 && major != 3 && major != SAB_VERSION_MAJOR {
        return Err(sab_err("UnsupportedSabVersion"));
    }
    let has_full_metadata = major >= 3;
    let section_count = decode_uleb128(&mut cursor)?;
    for _ in 0..section_count {
        let id = decode_uleb128(&mut cursor)?;
        let len = decode_uleb128(&mut cursor)?;
        let payload = cursor.read_slice(len as usize)?;
        visit(id, payload, has_full_metadata)?;
    }
    Ok(())
}

/// Zig `decodeModule` (lines 1008–1041): the full F-group entry
/// point. Section ids: 1 symbol pool, 2 function sigs, 3 const
/// decls, 4 instructions; unknown ids are skipped. A stream
/// without an instruction section is `MissingInstructionSection`.
pub fn decode_module(bytes: &[u8]) -> Result<Module> {
    // Version facts the section readers need (Zig computes them
    // from the major byte before the loop): raw text slots exist
    // on majors 2–3, full metadata on major >= 3.
    let major = if bytes.len() >= SAB_MAGIC.len() + 1 && &bytes[0..4] == SAB_MAGIC {
        bytes[SAB_MAGIC.len()]
    } else {
        0
    };
    let has_raw_text = (2..4).contains(&major);
    let mut symbols: Vec<String> = Vec::new();
    let mut function_sigs: Vec<FunctionSig> = Vec::new();
    let mut const_decls: Vec<ConstDecl> = Vec::new();
    let mut instructions: Option<Vec<Instruction>> = None;
    scan_sections(bytes, |id, payload, has_full_metadata| {
        match id {
            1 => symbols = read_string_pool(payload)?,
            2 => function_sigs = read_function_sigs(&symbols, payload, has_full_metadata)?,
            3 => const_decls = read_const_decls(&symbols, payload, has_full_metadata)?,
            4 => {
                instructions = Some(crate::decode_inst::read_instructions(
                    &symbols,
                    payload,
                    has_raw_text,
                    has_full_metadata,
                )?);
            }
            _ => {}
        }
        Ok(())
    })?;
    let instructions = instructions.ok_or_else(|| sab_err("MissingInstructionSection"))?;
    Ok(Module {
        symbols,
        function_sigs,
        const_decls,
        instructions,
    })
}

/// Zig `decodeFunctionSigsOnly` (lines 1043–1074).
pub fn decode_function_sigs_only(bytes: &[u8]) -> Result<Vec<FunctionSig>> {
    let mut symbols: Vec<String> = Vec::new();
    let mut sig_payload: Option<&[u8]> = None;
    let mut has_full_metadata = false;
    scan_sections(bytes, |id, payload, metadata| {
        if id == 1 {
            symbols = read_string_pool(payload)?;
        }
        if id == 2 {
            sig_payload = Some(payload);
            has_full_metadata = metadata;
        }
        Ok(())
    })?;
    match sig_payload {
        Some(payload) => read_function_sigs(&symbols, payload, has_full_metadata),
        None => Ok(Vec::new()),
    }
}

/// Zig `decodeTestFunctionSigsOnly` (lines 1076–1107).
pub fn decode_test_function_sigs_only(bytes: &[u8]) -> Result<Vec<FunctionSig>> {
    let mut symbols: Vec<String> = Vec::new();
    let mut sig_payload: Option<&[u8]> = None;
    let mut has_full_metadata = false;
    scan_sections(bytes, |id, payload, metadata| {
        if id == 1 {
            symbols = read_string_pool(payload)?;
        }
        if id == 2 {
            sig_payload = Some(payload);
            has_full_metadata = metadata;
        }
        Ok(())
    })?;
    match sig_payload {
        Some(payload) => read_test_function_sigs(&symbols, payload, has_full_metadata),
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::encode_module;

    #[test]
    fn uleb_sleb_decode_edges() {
        let mut c = Cursor::new(&[0xac, 0x02]);
        assert_eq!(decode_uleb128(&mut c).unwrap(), 300);
        let mut c = Cursor::new(&[0xc7, 0x9f, 0x7f]);
        assert_eq!(crate::decode::decode_sleb128(&mut c).unwrap(), -12_345);
        let mut c = Cursor::new(&[0x7f]);
        assert_eq!(crate::decode::decode_sleb128(&mut c).unwrap(), -1);
        // Truncation and overflow are Zig's named errors.
        let mut c = Cursor::new(&[0x80]);
        assert!(decode_uleb128(&mut c)
            .unwrap_err()
            .to_string()
            .contains("TruncatedSab"));
        let mut c = Cursor::new(&[0xff; 11]);
        assert!(decode_uleb128(&mut c)
            .unwrap_err()
            .to_string()
            .contains("Leb128Overflow"));
    }

    #[test]
    fn decode_rejects_bad_magic_and_version() {
        assert!(decode_module(b"XXXX\x04\x00\x00")
            .unwrap_err()
            .to_string()
            .contains("InvalidSabMagic"));
        assert!(decode_module(b"SAB\x00\x09\x00\x00")
            .unwrap_err()
            .to_string()
            .contains("UnsupportedSabVersion"));
        // Valid header, no sections at all: no instructions.
        assert!(decode_module(b"SAB\x00\x04\x00\x00")
            .unwrap_err()
            .to_string()
            .contains("MissingInstructionSection"));
    }

    #[test]
    fn encode_decode_encode_round_trip_is_byte_identical() {
        let module = crate::fixture01::fixture01_module();
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        // The decoded symbol table is the encoder's pool (a
        // superset ordering of the builder's symbols); re-encoding
        // the decoded module must reproduce the original bytes
        // exactly — the F group's core invariant.
        let reencoded = encode_module(&decoded).unwrap();
        assert_eq!(bytes, reencoded);
        assert_eq!(decoded.instructions.len(), module.instructions.len());
        assert_eq!(decoded.function_sigs.len(), module.function_sigs.len());
        for (want, got) in module.instructions.iter().zip(&decoded.instructions) {
            assert_eq!(want.kind, got.kind);
            assert_eq!(want.operands, got.operands);
        }
    }
}

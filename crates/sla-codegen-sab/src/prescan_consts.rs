//! C1 — top-level global const pre-scan, mirroring
//! `sab_codegen.zig` lines 633–854:
//! - `collectGlobalScalarConsts` (633–687): scalar const collection
//!   with the alias fixpoint and the binary-fold fixpoint;
//! - `collectGlobalArrayConsts` + `emitGlobalArrayConst`
//!   (689–772): array consts become `SLA_GCONST_<name>` hex const
//!   decls at pre-scan time (they are *emitted*, not just recorded);
//! - `scalarConstantNodeFor` (774–783) and
//!   `foldTopLevelBinaryConst` (785–835): the folding oracle.
//!
//! Zig quirks reproduced, not "fixed":
//! - scalar collection only accepts int/float/bool literals
//!   directly; string consts never enter the scalar map
//!   (`scalarConstantNodeFor` returns null for them too);
//! - the fold loop's inner alias loop runs to its own fixpoint
//!   before the outer fold loop re-scans — the two-phase structure
//!   below is the Zig control flow, verbatim;
//! - int fold overflow *declines* the fold (returns null), it is
//!   not an error — see the checked-\&-cleared note in
//!   docs/divergences/sab.md. Division/remainder by zero likewise
//!   decline. (`checked_div`/`checked_rem` also decline the
//!   `i64::MIN / -1` overflow, where Zig's unguarded `@divTrunc`
//!   traps the whole compiler with SIGFPE on both backends —
//!   real-product verified, registered as SAB-D25 / FB-025; the
//!   port declines the fold instead of reproducing the trap.)
//! - `emitGlobalArrayConst` silently registers *nothing* when the
//!   value is neither an array literal nor a repeat literal
//!   (Zig `else => return` at line 769): no decl, no map entry,
//!   no error. Elements that are not int/bool literals contribute
//!   only zero padding up to the element stride.
//! - the hex `literal_text` is built per byte from the Zig
//!   format string `"\\\\x{x:0>2}"` (as written in the source),
//!   but sci's `sio.managedPrint` collapses the doubled backslash,
//!   so the emitted pool text carries ONE backslash per byte
//!   (`hex:\x01\x00...`). Verified byte-for-byte against the real
//!   plugin's product on the venue (see the checked-&-cleared
//!   entry in docs/divergences/sab.md); a naive std.fmt reading of
//!   the source would double the backslash — this port's first
//!   version did exactly that and the product probe caught it.

use std::collections::HashMap;

use sla_ast::{ArrayType, BinaryExpr, BinaryOp, Literal, Node, Type};
use sla_lowering::{abi_passes_as_pointer, abi_type_size, CheckInfo};

use crate::emit::ModuleBuilder;
use crate::error::{Error, Result};
use crate::ir::{BytesLiteral, ConstDecl, ConstValue, PrimType};

/// The scalar literal carried by a node, if it is one of the three
/// kinds the scalar-const machinery folds (Zig's repeated
/// `.int_val, .float_val, .bool_val` arms). String literals are
/// deliberately excluded, as in Zig.
fn scalar_literal(node: &Node) -> Option<Literal> {
    match node {
        Node::Literal(lit @ (Literal::IntVal(_) | Literal::FloatVal(_) | Literal::BoolVal(_))) => {
            Some(lit.clone())
        }
        _ => None,
    }
}

/// Zig `collectGlobalScalarConsts` (633–687), returning the
/// `global_scalar_consts` map (name -> the literal node it folds
/// to). Zig clears the map first; a fresh map is returned here.
pub fn collect_global_scalar_consts(decls: &[Node]) -> HashMap<String, Node> {
    let mut consts: HashMap<String, Node> = HashMap::new();
    // Phase 1: direct literals.
    for decl in decls {
        let Node::ConstStmt(c) = decl else { continue };
        if scalar_literal(&c.value).is_some() {
            consts.insert(c.name.clone(), (*c.value).clone());
        }
    }
    // Phase 2: alias fixpoint.
    loop {
        let mut changed = false;
        for decl in decls {
            let Node::ConstStmt(c) = decl else { continue };
            let Node::Identifier(target) = &*c.value else {
                continue;
            };
            if consts.contains_key(&c.name) {
                continue;
            }
            if let Some(target_literal) = consts.get(target).cloned() {
                consts.insert(c.name.clone(), target_literal);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Phase 3: binary-fold fixpoint, each round followed by the
    // alias fixpoint (Zig's nested loop structure, verbatim).
    loop {
        let mut folded_any = false;
        for decl in decls {
            let Node::ConstStmt(c) = decl else { continue };
            let Node::BinaryExpr(bin) = &*c.value else {
                continue;
            };
            if consts.contains_key(&c.name) {
                continue;
            }
            if let Some(folded) = fold_top_level_binary_const(bin, &consts) {
                consts.insert(c.name.clone(), folded);
                folded_any = true;
            }
        }
        if !folded_any {
            break;
        }
        loop {
            let mut alias_changed = false;
            for decl in decls {
                let Node::ConstStmt(c) = decl else { continue };
                let Node::Identifier(target) = &*c.value else {
                    continue;
                };
                if consts.contains_key(&c.name) {
                    continue;
                }
                if let Some(target_literal) = consts.get(target).cloned() {
                    consts.insert(c.name.clone(), target_literal);
                    alias_changed = true;
                }
            }
            if !alias_changed {
                break;
            }
        }
    }
    consts
}

/// Zig `scalarConstantNodeFor` (774–783): a scalar literal node
/// itself, or the folded literal an identifier const refers to.
pub fn scalar_constant_node_for<'a>(
    expr: &'a Node,
    consts: &'a HashMap<String, Node>,
) -> Option<&'a Node> {
    match expr {
        Node::Literal(Literal::StringVal(_)) => None,
        Node::Literal(_) => {
            if scalar_literal(expr).is_some() {
                Some(expr)
            } else {
                None
            }
        }
        Node::Identifier(name) => consts.get(name),
        _ => None,
    }
}

/// Zig `foldTopLevelBinaryConst` (785–835): fold a binary
/// expression whose operands both resolve to scalar literals of
/// the same kind. Returns the folded literal node, or `None` when
/// the fold does not apply (kind mismatch, unsupported op,
/// overflow, division by zero, out-of-range shift).
pub fn fold_top_level_binary_const(
    bin: &BinaryExpr,
    consts: &HashMap<String, Node>,
) -> Option<Node> {
    let left = scalar_literal(scalar_constant_node_for(&bin.left, consts)?)?;
    let right = scalar_literal(scalar_constant_node_for(&bin.right, consts)?)?;
    let folded = match (left, right) {
        (Literal::IntVal(li), Literal::IntVal(ri)) => {
            let value = match bin.op {
                BinaryOp::Add => li.checked_add(ri)?,
                BinaryOp::Sub => li.checked_sub(ri)?,
                BinaryOp::Mul => li.checked_mul(ri)?,
                BinaryOp::Div => {
                    if ri == 0 {
                        return None;
                    }
                    li.checked_div(ri)?
                }
                BinaryOp::Mod => {
                    if ri == 0 {
                        return None;
                    }
                    li.checked_rem(ri)?
                }
                BinaryOp::BitAnd => li & ri,
                BinaryOp::BitOr => li | ri,
                BinaryOp::BitXor => li ^ ri,
                BinaryOp::Shl => {
                    if (0..64).contains(&ri) {
                        li.wrapping_shl(ri as u32)
                    } else {
                        return None;
                    }
                }
                BinaryOp::Shr => {
                    if (0..64).contains(&ri) {
                        li.wrapping_shr(ri as u32)
                    } else {
                        return None;
                    }
                }
                _ => return None,
            };
            Literal::IntVal(value)
        }
        (Literal::FloatVal(lf), Literal::FloatVal(rf)) => {
            let value = match bin.op {
                BinaryOp::Add => lf + rf,
                BinaryOp::Sub => lf - rf,
                BinaryOp::Mul => lf * rf,
                BinaryOp::Div => {
                    if rf == 0.0 {
                        return None;
                    }
                    lf / rf
                }
                _ => return None,
            };
            Literal::FloatVal(value)
        }
        (Literal::BoolVal(lb), Literal::BoolVal(rb)) => {
            let value = match bin.op {
                BinaryOp::LogicalAnd => lb && rb,
                BinaryOp::LogicalOr => lb || rb,
                _ => return None,
            };
            Literal::BoolVal(value)
        }
        _ => return None,
    };
    Some(Node::Literal(folded))
}

/// Zig `storagePrimType` (1595–1601), the D2 helper C1 depends on
/// for array element encoding: bool stores as u8, raw pointers as
/// ptr, everything else as `primType` — whose non-primitive arm is
/// u64 for the File type and ptr for pointer-carried types
/// (lowering `abiPassesAsPointer`); anything else is the Zig
/// `Error.UnsupportedSabDirectFeature`.
pub fn storage_prim_type(ty: &Type) -> Result<PrimType> {
    match ty {
        Type::Primitive(p) => {
            use sla_ast::Primitive as P;
            Ok(match p {
                P::Boolean => PrimType::U8,
                P::RawPtr => PrimType::Ptr,
                P::I8 => PrimType::I8,
                P::I16 => PrimType::I16,
                P::I32 => PrimType::I32,
                P::I64 | P::Integer | P::Isize => PrimType::I64,
                P::U8 => PrimType::U8,
                P::U16 => PrimType::U16,
                P::U32 => PrimType::U32,
                P::U64 | P::Usize => PrimType::U64,
                P::F32 => PrimType::F32,
                P::F64 | P::Float => PrimType::F64,
                P::VoidType => PrimType::Void,
            })
        }
        _ if abi_passes_as_pointer(ty) => Ok(PrimType::Ptr),
        _ => Err(Error::UnsupportedSabDirectFeature(format!(
            "storagePrimType of non-storage type {ty:?} [group C1 prescan_consts; Zig sab_codegen.zig:1595-1601]"
        ))),
    }
}

/// sci `primTypeBits` for the storage types C1 can encode
/// (signature.zig; only the byte count is consumed here).
fn prim_type_bits(prim: PrimType) -> u32 {
    match prim {
        PrimType::Void => 0,
        PrimType::I1 => 1,
        PrimType::I8 | PrimType::U8 => 8,
        PrimType::I16 | PrimType::U16 => 16,
        PrimType::I32 | PrimType::U32 | PrimType::F32 => 32,
        PrimType::I64 | PrimType::U64 | PrimType::F64 | PrimType::Ptr | PrimType::BlobHandle => 64,
        PrimType::V128 => 128,
    }
}

/// Zig `arrayStride` (1624–1626) = lowering `inlineArrayStride`:
/// `max(abiTypeSize(elem), 1)`.
pub fn array_stride(elem: &Type) -> usize {
    abi_type_size(elem).max(1)
}

/// One element's bytes inside an array const (Zig's per-element
/// arm, duplicated for the literal and repeat forms in the
/// original): int literals write their bit pattern little-endian
/// over the storage type's byte width, bool writes one byte, and
/// every element is zero-padded up to `stride` bytes — including
/// non-literal elements, which write nothing at all.
fn append_array_element(
    buf: &mut Vec<u8>,
    elem: &Node,
    elem_ty: &Type,
    stride: usize,
) -> Result<()> {
    let start = buf.len();
    if let Node::Literal(lit) = elem {
        match lit {
            Literal::IntVal(v) => {
                let prim = storage_prim_type(elem_ty)?;
                let bits = *v as u64;
                let nbytes = (prim_type_bits(prim) / 8) as usize;
                for i in 0..nbytes {
                    buf.push(((bits >> (i * 8)) & 0xFF) as u8);
                }
            }
            Literal::BoolVal(v) => buf.push(if *v { 1 } else { 0 }),
            _ => {}
        }
    }
    while buf.len() < start + stride {
        buf.push(0);
    }
    Ok(())
}

/// Zig `emitGlobalArrayConst` (702–772): materialize one array
/// const as a `SLA_GCONST_<name>` hex const decl and register
/// `name -> label`. Returns the label, or `None` — having emitted
/// and registered nothing — when the value is neither an array
/// literal nor a repeat literal (the Zig silent return, mirrored).
pub fn emit_global_array_const(
    builder: &mut ModuleBuilder,
    name: &str,
    value: &Node,
    arr: &ArrayType,
) -> Result<Option<String>> {
    let label = format!("SLA_GCONST_{name}");
    let stride = array_stride(&arr.elem);
    let mut buf: Vec<u8> = Vec::new();
    match value {
        Node::ArrayLiteral(lit) => {
            for elem in &lit.elements {
                append_array_element(&mut buf, elem, &arr.elem, stride)?;
            }
        }
        Node::RepeatArrayLiteral(lit) => {
            for _ in 0..lit.len {
                append_array_element(&mut buf, &lit.value, &arr.elem, stride)?;
            }
        }
        _ => return Ok(None),
    }
    // Zig per-byte format is "\\x{x:0>2}" as written in the source
    // ("\\\\x..." in the file); sci's managedPrint collapses the
    // doubled backslash, so the product text has one backslash
    // per byte — byte-verified on the venue, see the module docs.
    let mut literal_text = String::from("hex:");
    for b in &buf {
        literal_text.push_str(&format!("\\x{b:02x}"));
    }
    builder.append_const_decl(ConstDecl {
        source_line: 0,
        expanded_line: 0,
        upstream_loc: None,
        name: label.clone(),
        literal_text,
        value: ConstValue::Hex(BytesLiteral {
            bytes: buf,
            repeat: None,
        }),
    });
    builder.intern(&label);
    Ok(Some(label))
}

/// Zig `collectGlobalArrayConsts` (689–700): every top-level const
/// whose *checker-recorded value type* is an array type is
/// materialized, in declaration order. Consts without a recorded
/// type, or with a non-array type, are skipped silently (Zig
/// `orelse continue` / `continue`), as is a const whose name is
/// already registered.
pub fn collect_global_array_consts(
    builder: &mut ModuleBuilder,
    decls: &[Node],
    check: &dyn CheckInfo,
    out: &mut HashMap<String, String>,
) -> Result<()> {
    for decl in decls {
        let Node::ConstStmt(c) = decl else { continue };
        let Some(ty) = check.expr_type(&c.value) else {
            continue;
        };
        let Type::Array(arr) = ty else { continue };
        if out.contains_key(&c.name) {
            continue;
        }
        if let Some(label) = emit_global_array_const(builder, &c.name, &c.value, arr)? {
            out.insert(c.name.clone(), label);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{BinaryExpr, ConstStmt, Literal, Primitive, RepeatArrayLiteral, ArrayLiteral};
    use sla_lowering::MapCheckInfo;

    fn const_stmt(name: &str, value: Node) -> Node {
        Node::ConstStmt(ConstStmt {
            name: name.to_string(),
            ty: None,
            value: Box::new(value),
        })
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

    fn folded(map: &HashMap<String, Node>, name: &str) -> Option<Literal> {
        match map.get(name) {
            Some(Node::Literal(l)) => Some(l.clone()),
            _ => None,
        }
    }

    #[test]
    fn scalar_direct_alias_and_binary_fixpoint() {
        // Mirrors the Zig corpus shapes
        // test_unit_toplevel_const_alias.sla and
        // test_unit_toplevel_const_binary_fold.sla:
        // COMBINED = (0 - 42) + (0 - 1) = -43, reached only via the
        // fold loop's inner alias propagation order.
        let decls = vec![
            const_stmt("BASE", int(42)),
            const_stmt("ALIAS", Node::Identifier("BASE".to_string())),
            const_stmt("NEG_BASE", bin(BinaryOp::Sub, int(0), Node::Identifier("BASE".to_string()))),
            const_stmt("NEG_ONE", bin(BinaryOp::Sub, int(0), int(1))),
            const_stmt(
                "COMBINED",
                bin(
                    BinaryOp::Add,
                    Node::Identifier("NEG_BASE".to_string()),
                    Node::Identifier("NEG_ONE".to_string()),
                ),
            ),
            const_stmt("FWD", Node::Identifier("LATER".to_string())),
            const_stmt("LATER", int(9)),
        ];
        let map = collect_global_scalar_consts(&decls);
        assert_eq!(folded(&map, "BASE"), Some(Literal::IntVal(42)));
        assert_eq!(folded(&map, "ALIAS"), Some(Literal::IntVal(42)));
        assert_eq!(folded(&map, "NEG_BASE"), Some(Literal::IntVal(-42)));
        assert_eq!(folded(&map, "NEG_ONE"), Some(Literal::IntVal(-1)));
        assert_eq!(folded(&map, "COMBINED"), Some(Literal::IntVal(-43)));
        assert_eq!(folded(&map, "FWD"), Some(Literal::IntVal(9)));
    }

    #[test]
    fn fold_declines_are_not_errors() {
        let empty = HashMap::new();
        let fold = |n: Node| match n {
            Node::BinaryExpr(b) => fold_top_level_binary_const(&b, &empty),
            _ => unreachable!(),
        };
        // Overflow declines (Zig catch return null).
        assert_eq!(fold(bin(BinaryOp::Add, int(i64::MAX), int(1))), None);
        assert_eq!(fold(bin(BinaryOp::Mul, int(i64::MAX), int(2))), None);
        // Division / remainder by zero decline.
        assert_eq!(fold(bin(BinaryOp::Div, int(7), int(0))), None);
        assert_eq!(fold(bin(BinaryOp::Mod, int(7), int(0))), None);
        // Truncating division and remainder (Zig @divTrunc/@rem).
        assert_eq!(
            fold(bin(BinaryOp::Div, int(-7), int(2))),
            Some(int(-3))
        );
        assert_eq!(
            fold(bin(BinaryOp::Mod, int(-7), int(2))),
            Some(int(-1))
        );
        // Shifts: in-range folds, out-of-range declines.
        assert_eq!(fold(bin(BinaryOp::Shl, int(1), int(63))), Some(int(i64::MIN)));
        assert_eq!(fold(bin(BinaryOp::Shl, int(1), int(64))), None);
        assert_eq!(fold(bin(BinaryOp::Shl, int(1), int(-1))), None);
        assert_eq!(fold(bin(BinaryOp::Shr, int(-8), int(1))), Some(int(-4)));
        // Kind mismatch and non-fold ops decline.
        assert_eq!(
            fold(bin(
                BinaryOp::Add,
                int(1),
                Node::Literal(Literal::FloatVal(1.0))
            )),
            None
        );
        assert_eq!(fold(bin(BinaryOp::Eq, int(1), int(1))), None);
        // String operands never fold.
        assert_eq!(
            fold(bin(
                BinaryOp::Add,
                Node::Literal(Literal::StringVal("a".to_string())),
                Node::Literal(Literal::StringVal("b".to_string()))
            )),
            None
        );
    }

    #[test]
    fn float_and_bool_folds() {
        let empty = HashMap::new();
        let fold = |n: Node| match n {
            Node::BinaryExpr(b) => fold_top_level_binary_const(&b, &empty),
            _ => unreachable!(),
        };
        let f = |v: f64| Node::Literal(Literal::FloatVal(v));
        let b = |v: bool| Node::Literal(Literal::BoolVal(v));
        assert_eq!(fold(bin(BinaryOp::Mul, f(1.5), f(4.0))), Some(f(6.0)));
        assert_eq!(fold(bin(BinaryOp::Div, f(1.0), f(0.0))), None);
        assert_eq!(
            fold(bin(BinaryOp::LogicalAnd, b(true), b(false))),
            Some(b(false))
        );
        assert_eq!(fold(bin(BinaryOp::Add, b(true), b(true))), None);
    }

    #[test]
    fn string_const_never_enters_scalar_map() {
        let decls = vec![
            const_stmt("S", Node::Literal(Literal::StringVal("hi".to_string()))),
            const_stmt("SALIAS", Node::Identifier("S".to_string())),
        ];
        let map = collect_global_scalar_consts(&decls);
        assert!(map.is_empty());
    }

    fn i32_array_ty(len: usize) -> Type {
        Type::Array(ArrayType {
            elem: Box::new(Type::Primitive(Primitive::I32)),
            len,
        })
    }

    #[test]
    fn array_const_bytes_and_literal_text() {
        let mut builder = ModuleBuilder::new();
        let arr = match i32_array_ty(3) {
            Type::Array(a) => a,
            _ => unreachable!(),
        };
        let value = Node::ArrayLiteral(ArrayLiteral {
            elements: vec![int(1), int(-2), int(0x0102_0304)],
        });
        let label = emit_global_array_const(&mut builder, "TABLE", &value, &arr)
            .unwrap()
            .expect("array literal must emit");
        assert_eq!(label, "SLA_GCONST_TABLE");
        let module = builder.finish();
        assert_eq!(module.const_decls.len(), 1);
        let decl = &module.const_decls[0];
        assert_eq!(decl.name, "SLA_GCONST_TABLE");
        match &decl.value {
            ConstValue::Hex(bytes) => assert_eq!(
                bytes.bytes,
                vec![1, 0, 0, 0, 0xFE, 0xFF, 0xFF, 0xFF, 0x04, 0x03, 0x02, 0x01]
            ),
            other => panic!("expected hex const, got {other:?}"),
        }
        assert_eq!(
            decl.literal_text,
            "hex:\\x01\\x00\\x00\\x00\\xfe\\xff\\xff\\xff\\x04\\x03\\x02\\x01"
        );
        // The label is interned (Zig: `_ = try self.intern(label)`).
        assert!(module.symbols.iter().any(|s| s == "SLA_GCONST_TABLE"));
    }

    #[test]
    fn array_const_repeat_and_silent_non_literal() {
        let mut builder = ModuleBuilder::new();
        let arr = match i32_array_ty(2) {
            Type::Array(a) => a,
            _ => unreachable!(),
        };
        let value = Node::RepeatArrayLiteral(RepeatArrayLiteral {
            value: Box::new(int(7)),
            len: 2,
        });
        let label = emit_global_array_const(&mut builder, "REP", &value, &arr)
            .unwrap()
            .expect("repeat literal must emit");
        assert_eq!(label, "SLA_GCONST_REP");
        // A non-literal value registers nothing and emits nothing
        // (Zig's silent return).
        let other = Node::Identifier("SOMEWHERE".to_string());
        assert_eq!(
            emit_global_array_const(&mut builder, "NOPE", &other, &arr).unwrap(),
            None
        );
        let module = builder.finish();
        assert_eq!(module.const_decls.len(), 1);
        match &module.const_decls[0].value {
            ConstValue::Hex(bytes) => {
                assert_eq!(bytes.bytes, vec![7, 0, 0, 0, 7, 0, 0, 0])
            }
            other => panic!("expected hex const, got {other:?}"),
        }
    }

    #[test]
    fn array_const_bool_stride_one() {
        let mut builder = ModuleBuilder::new();
        let arr = ArrayType {
            elem: Box::new(Type::Primitive(Primitive::Boolean)),
            len: 2,
        };
        let value = Node::ArrayLiteral(ArrayLiteral {
            elements: vec![
                Node::Literal(Literal::BoolVal(true)),
                Node::Literal(Literal::BoolVal(false)),
            ],
        });
        emit_global_array_const(&mut builder, "FLAGS", &value, &arr).unwrap();
        let module = builder.finish();
        match &module.const_decls[0].value {
            ConstValue::Hex(bytes) => assert_eq!(bytes.bytes, vec![1, 0]),
            other => panic!("expected hex const, got {other:?}"),
        }
    }

    #[test]
    fn collect_array_consts_needs_checker_array_type() {
        // Without a checker-recorded array type the const is
        // skipped (Zig `orelse continue`); with one it materializes.
        let value = Node::ArrayLiteral(ArrayLiteral {
            elements: vec![int(3), int(4)],
        });
        let decls = vec![const_stmt("ARR", value.clone())];
        let mut builder = ModuleBuilder::new();
        let mut out = HashMap::new();
        let check = MapCheckInfo::new();
        collect_global_array_consts(&mut builder, &decls, &check, &mut out).unwrap();
        assert!(out.is_empty());

        let mut check = MapCheckInfo::new();
        if let Node::ConstStmt(c) = &decls[0] {
            check.add_expr_type(&c.value, i32_array_ty(2));
        }
        let mut builder = ModuleBuilder::new();
        collect_global_array_consts(&mut builder, &decls, &check, &mut out).unwrap();
        assert_eq!(out.get("ARR").map(|s| s.as_str()), Some("SLA_GCONST_ARR"));
        assert_eq!(builder.finish().const_decls.len(), 1);
    }
}

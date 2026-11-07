//! E1 — operator-kind mapping, mirroring `sab_codegen.zig`
//! lines 2464–2508 in the reference tree (`/content/sa_all`,
//! HEAD 2cc12ea8): `scalarBinaryOpKind` and `opKindForBinary`.
//! (Decomposition doc lines 2318–2390 in the older 20,024-line
//! tree; the functions moved, their bodies did not change.)
//!
//! Zig's `opKindForBinary` reads `tc.expr_types` directly; here
//! the same facts arrive through [`CheckInfo::expr_type`], and a
//! missing operand type is the loud `MissingType` error, exactly
//! as in Zig (`orelse return Error.MissingType`). A `None` plan
//! from `planScalarBinaryOp` is `UnsupportedSabDirectFeature`,
//! again exactly as in Zig.

use sla_ast::BinaryExpr;
use sla_lowering::{plan_scalar_binary_op, CheckInfo, ScalarBinaryOp};

use crate::error::{Error, Result};
use crate::ir::OpKind;

/// Zig `scalarBinaryOpKind`: the planned scalar op -> the SAB
/// instruction op kind. Note the Zig merges: bit_and/logical_and
/// both land on `and`, bit_or/logical_or on `or`.
pub fn scalar_binary_op_kind(op: ScalarBinaryOp) -> OpKind {
    match op {
        ScalarBinaryOp::Add => OpKind::Add,
        ScalarBinaryOp::Sub => OpKind::Sub,
        ScalarBinaryOp::Mul => OpKind::Mul,
        ScalarBinaryOp::SDiv => OpKind::Sdiv,
        ScalarBinaryOp::UDiv => OpKind::Udiv,
        ScalarBinaryOp::SRem => OpKind::Srem,
        ScalarBinaryOp::URem => OpKind::Urem,
        ScalarBinaryOp::Eq => OpKind::Eq,
        ScalarBinaryOp::Ne => OpKind::Ne,
        ScalarBinaryOp::SLt => OpKind::Slt,
        ScalarBinaryOp::SLe => OpKind::Sle,
        ScalarBinaryOp::SGt => OpKind::Sgt,
        ScalarBinaryOp::SGe => OpKind::Sge,
        ScalarBinaryOp::ULt => OpKind::Ult,
        ScalarBinaryOp::ULe => OpKind::Ule,
        ScalarBinaryOp::UGt => OpKind::Ugt,
        ScalarBinaryOp::UGe => OpKind::Uge,
        ScalarBinaryOp::BitAnd | ScalarBinaryOp::LogicalAnd => OpKind::And,
        ScalarBinaryOp::BitOr | ScalarBinaryOp::LogicalOr => OpKind::Or,
        ScalarBinaryOp::BitXor => OpKind::Xor,
        ScalarBinaryOp::Shl => OpKind::Shl,
        ScalarBinaryOp::LShr => OpKind::Lshr,
        ScalarBinaryOp::AShr => OpKind::Ashr,
        ScalarBinaryOp::FAdd => OpKind::Fadd,
        ScalarBinaryOp::FSub => OpKind::Fsub,
        ScalarBinaryOp::FMul => OpKind::Fmul,
        ScalarBinaryOp::FDiv => OpKind::Fdiv,
        ScalarBinaryOp::FCmpEq => OpKind::FcmpEq,
        ScalarBinaryOp::FCmpNe => OpKind::FcmpNe,
        ScalarBinaryOp::FCmpLt => OpKind::FcmpLt,
        ScalarBinaryOp::FCmpLe => OpKind::FcmpLe,
        ScalarBinaryOp::FCmpGt => OpKind::FcmpGt,
        ScalarBinaryOp::FCmpGe => OpKind::FcmpGe,
    }
}

/// Zig `opKindForBinary`: plan the binary op from the checker's
/// operand types, then map the plan to an instruction op kind.
pub fn op_kind_for_binary(check: &dyn CheckInfo, bin: &BinaryExpr) -> Result<OpKind> {
    let lhs_ty = check.expr_type(&bin.left).ok_or_else(|| {
        Error::MissingType("opKindForBinary: no checker type for lhs".to_string())
    })?;
    let rhs_ty = check.expr_type(&bin.right).ok_or_else(|| {
        Error::MissingType("opKindForBinary: no checker type for rhs".to_string())
    })?;
    let plan = plan_scalar_binary_op(bin.op, lhs_ty, rhs_ty).ok_or_else(|| {
        Error::UnsupportedSabDirectFeature(
            "opKindForBinary: planScalarBinaryOp returned no plan".to_string(),
        )
    })?;
    Ok(scalar_binary_op_kind(plan))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{BinaryOp, Literal, Node, Primitive, Type};
    use sla_lowering::MapCheckInfo;

    fn i32_ty() -> Type {
        Type::Primitive(Primitive::I32)
    }

    fn bin_node(op: BinaryOp) -> BinaryExpr {
        BinaryExpr {
            op,
            left: Box::new(Node::Literal(Literal::IntVal(1))),
            right: Box::new(Node::Literal(Literal::IntVal(2))),
        }
    }

    #[test]
    fn merges_logical_and_bit_ops_like_zig() {
        assert_eq!(scalar_binary_op_kind(ScalarBinaryOp::BitAnd), OpKind::And);
        assert_eq!(scalar_binary_op_kind(ScalarBinaryOp::LogicalAnd), OpKind::And);
        assert_eq!(scalar_binary_op_kind(ScalarBinaryOp::BitOr), OpKind::Or);
        assert_eq!(scalar_binary_op_kind(ScalarBinaryOp::LogicalOr), OpKind::Or);
        assert_eq!(scalar_binary_op_kind(ScalarBinaryOp::SDiv), OpKind::Sdiv);
        assert_eq!(scalar_binary_op_kind(ScalarBinaryOp::FCmpLe), OpKind::FcmpLe);
    }

    #[test]
    fn plans_from_checker_types() {
        let bin = bin_node(BinaryOp::Div);
        let mut check = MapCheckInfo::new();
        check.add_expr_type(&bin.left, i32_ty());
        check.add_expr_type(&bin.right, i32_ty());
        assert_eq!(op_kind_for_binary(&check, &bin).unwrap(), OpKind::Sdiv);
    }

    #[test]
    fn missing_type_is_loud_missing_type() {
        let bin = bin_node(BinaryOp::Add);
        let check = MapCheckInfo::new();
        match op_kind_for_binary(&check, &bin) {
            Err(Error::MissingType(_)) => {}
            other => panic!("expected MissingType, got {other:?}"),
        }
    }
}

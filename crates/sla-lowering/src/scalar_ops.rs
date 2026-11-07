//! Scalar binary-operator planning, mirrored from lowering_rules.zig
//! `planScalarBinaryOp` / `scalarBinaryOpName` (lines 5562-5650). The
//! codegen-side `binaryOpName` (codegen.zig:3562) is exactly
//! `scalar_binary_op_name(plan_scalar_binary_op(...))`, returning no
//! name when the plan is `None` (float `%`/bit ops, `<=>` on scalars).

use sla_ast::{BinaryOp, Type};

use crate::abi::{is_float_type, is_unsigned_integer_type};

/// lowering_rules.zig `ScalarBinaryOp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarBinaryOp {
    Add,
    Sub,
    Mul,
    SDiv,
    UDiv,
    SRem,
    URem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    LShr,
    AShr,
    Eq,
    Ne,
    SLt,
    SLe,
    SGt,
    SGe,
    ULt,
    ULe,
    UGt,
    UGe,
    FAdd,
    FSub,
    FMul,
    FDiv,
    FCmpEq,
    FCmpNe,
    FCmpLt,
    FCmpLe,
    FCmpGt,
    FCmpGe,
    LogicalAnd,
    LogicalOr,
}

/// lowering_rules.zig:5562 `planScalarBinaryOp`. Signedness is decided
/// by the LEFT operand type only; float-ness by either side.
pub fn plan_scalar_binary_op(op: BinaryOp, left_ty: &Type, right_ty: &Type) -> Option<ScalarBinaryOp> {
    let use_float = is_float_type(left_ty) || is_float_type(right_ty);
    if use_float {
        return Some(match op {
            BinaryOp::Add => ScalarBinaryOp::FAdd,
            BinaryOp::Sub => ScalarBinaryOp::FSub,
            BinaryOp::Mul => ScalarBinaryOp::FMul,
            BinaryOp::Div => ScalarBinaryOp::FDiv,
            BinaryOp::Eq => ScalarBinaryOp::FCmpEq,
            BinaryOp::Ne => ScalarBinaryOp::FCmpNe,
            BinaryOp::Lt => ScalarBinaryOp::FCmpLt,
            BinaryOp::Le => ScalarBinaryOp::FCmpLe,
            BinaryOp::Gt => ScalarBinaryOp::FCmpGt,
            BinaryOp::Ge => ScalarBinaryOp::FCmpGe,
            _ => return None,
        });
    }

    let use_unsigned = is_unsigned_integer_type(left_ty);
    Some(match op {
        BinaryOp::Add => ScalarBinaryOp::Add,
        BinaryOp::Sub => ScalarBinaryOp::Sub,
        BinaryOp::Mul => ScalarBinaryOp::Mul,
        BinaryOp::Div => {
            if use_unsigned {
                ScalarBinaryOp::UDiv
            } else {
                ScalarBinaryOp::SDiv
            }
        }
        BinaryOp::Mod => {
            if use_unsigned {
                ScalarBinaryOp::URem
            } else {
                ScalarBinaryOp::SRem
            }
        }
        BinaryOp::BitAnd => ScalarBinaryOp::BitAnd,
        BinaryOp::BitOr => ScalarBinaryOp::BitOr,
        BinaryOp::BitXor => ScalarBinaryOp::BitXor,
        BinaryOp::Shl => ScalarBinaryOp::Shl,
        BinaryOp::Shr => {
            if use_unsigned {
                ScalarBinaryOp::LShr
            } else {
                ScalarBinaryOp::AShr
            }
        }
        BinaryOp::Eq => ScalarBinaryOp::Eq,
        BinaryOp::Ne => ScalarBinaryOp::Ne,
        BinaryOp::Lt => {
            if use_unsigned {
                ScalarBinaryOp::ULt
            } else {
                ScalarBinaryOp::SLt
            }
        }
        BinaryOp::Le => {
            if use_unsigned {
                ScalarBinaryOp::ULe
            } else {
                ScalarBinaryOp::SLe
            }
        }
        BinaryOp::Gt => {
            if use_unsigned {
                ScalarBinaryOp::UGt
            } else {
                ScalarBinaryOp::SGt
            }
        }
        BinaryOp::Ge => {
            if use_unsigned {
                ScalarBinaryOp::UGe
            } else {
                ScalarBinaryOp::SGe
            }
        }
        BinaryOp::LogicalAnd => ScalarBinaryOp::LogicalAnd,
        BinaryOp::LogicalOr => ScalarBinaryOp::LogicalOr,
        BinaryOp::Spaceship => return None,
    })
}

/// lowering_rules.zig:5604 `scalarBinaryOpName`.
pub fn scalar_binary_op_name(op: ScalarBinaryOp) -> &'static str {
    match op {
        ScalarBinaryOp::Add => "add",
        ScalarBinaryOp::Sub => "sub",
        ScalarBinaryOp::Mul => "mul",
        ScalarBinaryOp::SDiv => "sdiv",
        ScalarBinaryOp::UDiv => "udiv",
        ScalarBinaryOp::SRem => "srem",
        ScalarBinaryOp::URem => "urem",
        ScalarBinaryOp::BitAnd | ScalarBinaryOp::LogicalAnd => "and",
        ScalarBinaryOp::BitOr | ScalarBinaryOp::LogicalOr => "or",
        ScalarBinaryOp::BitXor => "xor",
        ScalarBinaryOp::Shl => "shl",
        ScalarBinaryOp::LShr => "lshr",
        ScalarBinaryOp::AShr => "ashr",
        ScalarBinaryOp::Eq => "eq",
        ScalarBinaryOp::Ne => "ne",
        ScalarBinaryOp::SLt => "slt",
        ScalarBinaryOp::SLe => "sle",
        ScalarBinaryOp::SGt => "sgt",
        ScalarBinaryOp::SGe => "sge",
        ScalarBinaryOp::ULt => "ult",
        ScalarBinaryOp::ULe => "ule",
        ScalarBinaryOp::UGt => "ugt",
        ScalarBinaryOp::UGe => "uge",
        ScalarBinaryOp::FAdd => "fadd",
        ScalarBinaryOp::FSub => "fsub",
        ScalarBinaryOp::FMul => "fmul",
        ScalarBinaryOp::FDiv => "fdiv",
        ScalarBinaryOp::FCmpEq => "fcmp_eq",
        ScalarBinaryOp::FCmpNe => "fcmp_ne",
        ScalarBinaryOp::FCmpLt => "fcmp_lt",
        ScalarBinaryOp::FCmpLe => "fcmp_le",
        ScalarBinaryOp::FCmpGt => "fcmp_gt",
        ScalarBinaryOp::FCmpGe => "fcmp_ge",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::Primitive;

    fn i32t() -> Type {
        Type::Primitive(Primitive::I32)
    }
    fn u32t() -> Type {
        Type::Primitive(Primitive::U32)
    }
    fn f64t() -> Type {
        Type::Primitive(Primitive::F64)
    }

    #[test]
    fn signed_and_unsigned_tables() {
        // Golden evidence: fixtures use sdiv/srem/sgt/sle/slt on i32.
        assert_eq!(plan_scalar_binary_op(BinaryOp::Div, &i32t(), &i32t()), Some(ScalarBinaryOp::SDiv));
        assert_eq!(plan_scalar_binary_op(BinaryOp::Mod, &i32t(), &i32t()), Some(ScalarBinaryOp::SRem));
        assert_eq!(plan_scalar_binary_op(BinaryOp::Div, &u32t(), &u32t()), Some(ScalarBinaryOp::UDiv));
        assert_eq!(plan_scalar_binary_op(BinaryOp::Shr, &i32t(), &i32t()), Some(ScalarBinaryOp::AShr));
        assert_eq!(plan_scalar_binary_op(BinaryOp::Shr, &u32t(), &u32t()), Some(ScalarBinaryOp::LShr));
        // Signedness comes from the LEFT type only (Zig reads left_ty).
        assert_eq!(plan_scalar_binary_op(BinaryOp::Lt, &u32t(), &i32t()), Some(ScalarBinaryOp::ULt));
        assert_eq!(scalar_binary_op_name(ScalarBinaryOp::SDiv), "sdiv");
        assert_eq!(scalar_binary_op_name(ScalarBinaryOp::LogicalAnd), "and");
    }

    #[test]
    fn float_and_spaceship_gaps() {
        assert_eq!(plan_scalar_binary_op(BinaryOp::Mod, &f64t(), &f64t()), None);
        assert_eq!(plan_scalar_binary_op(BinaryOp::Spaceship, &i32t(), &i32t()), None);
        assert_eq!(plan_scalar_binary_op(BinaryOp::Add, &f64t(), &i32t()), Some(ScalarBinaryOp::FAdd));
    }
}

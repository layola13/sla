//! Release-need and constant-value facts shared by the emitters,
//! mirrored from lowering_rules.zig (lines 5911-5990, 6131-6150).

use sla_ast::{BinaryOp, Literal, Node, Type};

use crate::abi::is_borrow_like_type;
use crate::names::root_identifier;

/// lowering_rules.zig:5911 `callArgNeedsRelease`.
pub fn call_arg_needs_release(arg: &Node) -> bool {
    match arg {
        Node::Literal(lit) => !matches!(lit, Literal::StringVal(_)),
        Node::Identifier(_) => false,
        Node::FieldExpr(_) | Node::IndexExpr(_) | Node::BorrowExpr(_) | Node::DerefExpr(_) => true,
        Node::MoveExpr(mv) => expr_result_needs_release(&mv.expr),
        Node::CastExpr(_) => false,
        _ => true,
    }
}

/// lowering_rules.zig:5925 `exprResultNeedsRelease`.
pub fn expr_result_needs_release(expr: &Node) -> bool {
    match expr {
        Node::Literal(lit) => !matches!(lit, Literal::StringVal(_)),
        Node::Identifier(_) => false,
        Node::FieldExpr(_) | Node::IndexExpr(_) | Node::BorrowExpr(_) | Node::DerefExpr(_) => true,
        Node::CastExpr(_) => false,
        _ => true,
    }
}

/// lowering_rules.zig:5991 `castResultMaterializesTemp`.
pub fn cast_result_materializes_temp(src_ty: &Type, dst_ty: &Type) -> bool {
    !(crate::abi::is_pointer_carrier_cast_type(src_ty)
        && crate::abi::is_pointer_carrier_cast_type(dst_ty))
}

/// lowering_rules.zig:5940 `intConstantExprValue`. The Zig version
/// takes the checker's scalar-const table (`name -> *Node`); here the
/// caller supplies the already-resolved integer value per name, which
/// is the only fact this function reads from it. Overflow folds to
/// `None` exactly like the Zig checked arithmetic.
pub fn int_constant_expr_value(
    expr: &Node,
    scalar_consts: &dyn Fn(&str) -> Option<i64>,
) -> Option<i64> {
    match expr {
        Node::Literal(Literal::IntVal(value)) => Some(*value),
        Node::Literal(_) => None,
        Node::Identifier(name) => scalar_consts(name),
        Node::CastExpr(cast) => int_constant_expr_value(&cast.expr, scalar_consts),
        Node::BinaryExpr(bin) => {
            let left = int_constant_expr_value(&bin.left, scalar_consts)?;
            let right = int_constant_expr_value(&bin.right, scalar_consts)?;
            match bin.op {
                BinaryOp::Add => left.checked_add(right),
                BinaryOp::Sub => left.checked_sub(right),
                BinaryOp::Mul => left.checked_mul(right),
                BinaryOp::Div => {
                    if right == 0 {
                        None
                    } else {
                        Some(left.wrapping_div(right))
                    }
                }
                BinaryOp::Mod => {
                    if right == 0 {
                        None
                    } else {
                        Some(left.wrapping_rem(right))
                    }
                }
                BinaryOp::BitAnd => Some(left & right),
                BinaryOp::BitOr => Some(left | right),
                BinaryOp::BitXor => Some(left ^ right),
                BinaryOp::Shl => {
                    if (0..64).contains(&right) {
                        Some(left << (right as u32))
                    } else {
                        None
                    }
                }
                BinaryOp::Shr => {
                    if (0..64).contains(&right) {
                        Some(left >> (right as u32))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// lowering_rules.zig:5973 `fieldBaseResultNeedsRelease`.
pub fn field_base_result_needs_release(
    expression_result_needs_release: bool,
    generated_register_is_temporary: bool,
    generated_register_is_resolved_binding: bool,
) -> bool {
    expression_result_needs_release
        || (generated_register_is_temporary && !generated_register_is_resolved_binding)
}

/// lowering_rules.zig:6131 `storedValueMovesIdentifier`.
pub fn stored_value_moves_identifier<'a>(
    value: &'a Node,
    value_ty: &Type,
    value_is_copy: bool,
) -> Option<&'a str> {
    if !matches!(value, Node::Identifier(_)) {
        return None;
    }
    if value_is_copy || is_borrow_like_type(value_ty) {
        return None;
    }
    if let Node::Identifier(name) = value {
        return Some(name);
    }
    None
}

/// lowering_rules.zig:6140 `assignmentMovesIdentifier`.
pub fn assignment_moves_identifier<'a>(
    target: &Node,
    value: &'a Node,
    value_ty: &Type,
    value_is_copy: bool,
) -> Option<&'a str> {
    let value_name = stored_value_moves_identifier(value, value_ty, value_is_copy)?;
    if let Some(target_name) = root_identifier(target) {
        if target_name == value_name {
            return None;
        }
    }
    Some(value_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{BinaryExpr, Literal, Primitive};

    #[test]
    fn release_need_table() {
        let lit = Node::Literal(Literal::IntVal(3));
        assert!(call_arg_needs_release(&lit));
        let s = Node::Literal(Literal::StringVal("x".to_string()));
        assert!(!call_arg_needs_release(&s));
        let id = Node::Identifier("a".to_string());
        assert!(!call_arg_needs_release(&id));
        assert!(!expr_result_needs_release(&id));
        let bin = Node::BinaryExpr(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(id.clone()),
            right: Box::new(lit.clone()),
        });
        assert!(expr_result_needs_release(&bin));
    }

    #[test]
    fn int_const_folding() {
        let no_consts = |_: &str| None;
        let expr = Node::BinaryExpr(BinaryExpr {
            op: BinaryOp::Sub,
            left: Box::new(Node::Literal(Literal::IntVal(0))),
            right: Box::new(Node::Literal(Literal::IntVal(1))),
        });
        assert_eq!(int_constant_expr_value(&expr, &no_consts), Some(-1));
        let div0 = Node::BinaryExpr(BinaryExpr {
            op: BinaryOp::Div,
            left: Box::new(Node::Literal(Literal::IntVal(1))),
            right: Box::new(Node::Literal(Literal::IntVal(0))),
        });
        assert_eq!(int_constant_expr_value(&div0, &no_consts), None);
        let i32ty = Type::Primitive(Primitive::I32);
        assert_eq!(stored_value_moves_identifier(&Node::Identifier("x".into()), &i32ty, true), None);
    }
}

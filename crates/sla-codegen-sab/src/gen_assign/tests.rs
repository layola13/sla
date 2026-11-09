use super::*;

use sla_ast::{IndexExpr, Primitive, TupleType, UserDefined};

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

/// The deref lifecycle plan, row for row with the Zig
/// lowering_rules tests (lowering_rules.zig:8613-8614 plus the
/// two combinations they leave implicit): a non-temporary
/// target is always kept; a temporary target releases its
/// borrow handle when it carries one, else the value itself.
#[test]
fn deref_assign_lifecycle_table() {
    assert_eq!(
        plan_deref_assignment_target_lifecycle(false, true),
        DerefAssignLifecycle::Keep
    );
    assert_eq!(
        plan_deref_assignment_target_lifecycle(false, false),
        DerefAssignLifecycle::Keep
    );
    assert_eq!(
        plan_deref_assignment_target_lifecycle(true, false),
        DerefAssignLifecycle::ReleaseValue
    );
    assert_eq!(
        plan_deref_assignment_target_lifecycle(true, true),
        DerefAssignLifecycle::ReleaseBorrowHandle
    );
    assert!(!DerefAssignLifecycle::Keep.should_release());
    assert!(DerefAssignLifecycle::ReleaseValue.should_release());
    assert!(DerefAssignLifecycle::ReleaseBorrowHandle.should_release());
}

/// The deref arm's guard probe (lowering_rules.zig:4781/4793):
/// only the two guard types yield an inner type; any other
/// user-defined type falls through to the borrow/pointer arm.
#[test]
fn guard_inner_type_probes() {
    let guard = ud("MutexGuard", vec![i32_ty()]);
    assert_eq!(
        user_defined_generic_inner(&guard, "MutexGuard"),
        Some(i32_ty())
    );
    let wguard = ud("RwLockWriteGuard", vec![i32_ty()]);
    assert_eq!(
        user_defined_generic_inner(&wguard, "RwLockWriteGuard"),
        Some(i32_ty())
    );
    let vec = ud("Vec", vec![i32_ty()]);
    assert_eq!(user_defined_generic_inner(&vec, "MutexGuard"), None);
    assert_eq!(user_defined_generic_inner(&vec, "RwLockWriteGuard"), None);
}

/// Array address facts the index arm relies on: byte stride
/// is the element's ABI size (min 1), and element offsets are
/// stride * index (lowering_rules `inlineArrayStride` /
/// `arrayElementLayout`).
#[test]
fn array_address_facts() {
    assert_eq!(array_stride(&Type::Primitive(Primitive::I8)), 1);
    assert_eq!(array_stride(&i32_ty()), 4);
    let arr = ArrayType {
        elem: Box::new(i32_ty()),
        len: 4,
    };
    let layout = array_element_layout(&arr, 2).expect("in-bounds element");
    assert_eq!(layout.offset, 8);
    assert!(array_element_layout(&arr, 4).is_none());
}

/// Tuple field offsets follow the aggregate layout walk
/// (lowering_rules.zig:3866): offsets accumulate packed,
/// except that an 8-byte element aligns the running offset
/// up to 8 (`alignAggregateOffset`, lowering_rules.zig:3851).
#[test]
fn tuple_field_offset_facts() {
    let tuple = TupleType {
        elems: vec![Type::Primitive(Primitive::I8), i32_ty()],
    };
    let layout = tuple_field_layout(&tuple, 1).expect("second element");
    assert_eq!(layout.offset, 1);
    assert!(tuple_field_layout(&tuple, 2).is_none());
    let wide = TupleType {
        elems: vec![
            Type::Primitive(Primitive::I8),
            Type::Primitive(Primitive::I64),
        ],
    };
    let layout = tuple_field_layout(&wide, 1).expect("second element");
    assert_eq!(layout.offset, 8);
}

/// The moved-source marking the index/field arms run after
/// the store (lowering_rules.zig:6140): storing a non-Copy
/// identifier into a different root moves it; a Copy value or
/// a self-rooted target does not.
#[test]
fn assignment_moved_source_facts() {
    let target = Node::IndexExpr(IndexExpr {
        target: Box::new(Node::Identifier("v".to_string())),
        index: Box::new(Node::Literal(Literal::IntVal(0))),
    });
    let value = Node::Identifier("x".to_string());
    let vec_ty = ud("Vec", vec![i32_ty()]);
    assert_eq!(
        sla_lowering::release_facts::assignment_moves_identifier(
            &target,
            &value,
            &vec_ty,
            false
        ),
        Some("x")
    );
    assert_eq!(
        sla_lowering::release_facts::assignment_moves_identifier(&target, &value, &vec_ty, true),
        None
    );
    let self_value = Node::Identifier("v".to_string());
    assert_eq!(
        sla_lowering::release_facts::assignment_moves_identifier(
            &target,
            &self_value,
            &vec_ty,
            false
        ),
        None
    );
}

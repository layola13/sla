//! Pure-fact tests for the S6 index helpers: the ordinary
//! address-target table, the fixed-array element table,
//! and the two-shape map index plan (Zig
//! lowering_rules.zig:153 / 169 / 4628).

use sla_ast::{ArrayType, Primitive, Type, UserDefined};

use super::{index_fixed_array_element_type, map_index_plan, ordinary_index_address_target_type, MapIndexShape};

fn prim(p: Primitive) -> Type {
    Type::Primitive(p)
}

fn array(elem: Type, len: usize) -> Type {
    Type::Array(ArrayType {
        elem: Box::new(elem),
        len,
    })
}

fn generic(name: &str, args: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics: args,
    })
}

#[test]
fn ordinary_index_address_target_table() {
    let arr = array(prim(Primitive::I32), 3);
    assert_eq!(
        ordinary_index_address_target_type(&arr),
        Some(arr.clone())
    );
    // Borrows peel to the array.
    assert_eq!(
        ordinary_index_address_target_type(&Type::Borrow(Box::new(arr.clone()))),
        Some(arr)
    );
    let slice = generic("Slice", vec![prim(Primitive::I32)]);
    assert_eq!(
        ordinary_index_address_target_type(&slice),
        Some(slice.clone())
    );
    // Vec / GrowVec / maps are not ordinary address targets.
    assert_eq!(
        ordinary_index_address_target_type(&generic("Vec", vec![prim(Primitive::I32)])),
        None
    );
    assert_eq!(
        ordinary_index_address_target_type(&generic(
            "HashMap",
            vec![prim(Primitive::I64), prim(Primitive::I64)]
        )),
        None
    );
    assert_eq!(
        ordinary_index_address_target_type(&prim(Primitive::I32)),
        None
    );
    // A Slice with the wrong generic count does not qualify.
    assert_eq!(
        ordinary_index_address_target_type(&generic("Slice", vec![])),
        None
    );
}

#[test]
fn index_fixed_array_element_table() {
    let arr = array(prim(Primitive::U8), 4);
    assert_eq!(
        index_fixed_array_element_type(&arr),
        Some(prim(Primitive::U8))
    );
    assert_eq!(
        index_fixed_array_element_type(&Type::Pointer(Box::new(arr))),
        Some(prim(Primitive::U8))
    );
    assert_eq!(
        index_fixed_array_element_type(&generic("Vec", vec![prim(Primitive::U8)])),
        None
    );
    assert_eq!(
        index_fixed_array_element_type(&generic("Slice", vec![prim(Primitive::U8)])),
        None
    );
}

#[test]
fn map_index_plan_table() {
    let hash = generic(
        "HashMap",
        vec![prim(Primitive::I64), prim(Primitive::U32)],
    );
    let plan = map_index_plan(&hash).expect("HashMap plan");
    assert_eq!(plan.macro_name, "MAP_TRY_GET");
    assert_eq!(plan.dep_symbol, "sa_map_try_get");
    assert_eq!(plan.import_path, "sa_std/hashmap.sa");
    assert_eq!(plan.shape, MapIndexShape::TryGetPtr);

    let btree = generic(
        "BTreeMap",
        vec![prim(Primitive::I64), prim(Primitive::U32)],
    );
    let plan = map_index_plan(&btree).expect("BTreeMap plan");
    assert_eq!(plan.macro_name, "BTREE_MAP_TRY_GET");
    assert_eq!(plan.dep_symbol, "sa_btree_map_try_get");
    assert_eq!(plan.import_path, "sa_std/btree_map.sa");
    assert_eq!(plan.shape, MapIndexShape::TryGetValue);

    assert!(map_index_plan(&generic("Vec", vec![prim(Primitive::I32)])).is_none());
    assert!(map_index_plan(&prim(Primitive::I32)).is_none());
}

use super::*;

fn call(target: Option<&str>, name: &str, argc: usize) -> CallExpr {
    CallExpr {
        func_name: name.to_string(),
        associated_target: target.map(|t| t.to_string()),
        generics: Vec::new(),
        args: (0..argc)
            .map(|_| Node::Literal(sla_ast::Literal::IntVal(0)))
            .collect(),
        callee: None,
    }
}

#[test]
fn atomic_new_plans_match_zig_table() {
    let plan = plan_atomic_new_call(&call(Some("AtomicI32"), "new", 1)).unwrap();
    assert_eq!(plan.size, 4);
    assert_eq!(plan.macro_name, "ATOMIC_I32_INIT");
    let plan = plan_atomic_new_call(&call(Some("AtomicUsize"), "new", 1)).unwrap();
    assert_eq!(plan.size, 8);
    assert_eq!(plan.macro_name, "ATOMIC_USIZE_INIT");
    let plan = plan_atomic_new_call(&call(Some("AtomicPtr"), "new", 1)).unwrap();
    assert_eq!(plan.size, 8);
    assert_eq!(plan.macro_name, "ATOMIC_PTR_INIT");
    assert!(plan_atomic_new_call(&call(Some("AtomicI32"), "new", 0)).is_none());
    assert!(plan_atomic_new_call(&call(Some("AtomicBool"), "new", 1)).is_none());
    assert!(plan_atomic_new_call(&call(None, "new", 1)).is_none());
}

#[test]
fn ctor_shorthand_recognition() {
    assert!(is_enum_ctor_shorthand("Option", "Some"));
    assert!(is_enum_ctor_shorthand("Option", "None"));
    assert!(is_enum_ctor_shorthand("Result", "Ok"));
    assert!(is_enum_ctor_shorthand("Result", "Err"));
    assert!(!is_enum_ctor_shorthand("Option", "unwrap"));
    assert!(!is_enum_ctor_shorthand("Vec", "Some"));
    assert_eq!(ctor_shorthand_arity("None"), 0);
    assert_eq!(ctor_shorthand_arity("Some"), 1);
}

#[test]
fn smart_pointer_unwrap_only_for_clone() {
    assert!(associated_rule_needs_underlying_smart_pointer("Rc", "clone"));
    assert!(associated_rule_needs_underlying_smart_pointer("Arc", "clone"));
    assert!(!associated_rule_needs_underlying_smart_pointer("Rc", "new"));
    assert!(!associated_rule_needs_underlying_smart_pointer("Box", "clone"));
}

#[test]
fn ptr_macro_recognition() {
    assert!(call_needs_ptr_macros_associated(&call(
        Some("ptr"),
        "read_volatile",
        1
    )));
    assert!(call_needs_ptr_macros_associated(&call(Some("ptr"), "null", 0)));
    assert!(!call_needs_ptr_macros_associated(&call(Some("ptr"), "drop", 1)));
    assert!(!call_needs_ptr_macros_associated(&call(
        None,
        "read_volatile",
        1
    )));
    assert_eq!(
        ptr_read_volatile_macro_name(&Type::Primitive(sla_ast::Primitive::I32)),
        Some("PTR_READ_VOLATILE_I32")
    );
    assert_eq!(
        ptr_read_volatile_macro_name(&Type::Primitive(sla_ast::Primitive::U64)),
        Some("PTR_READ_VOLATILE_U64")
    );
    assert_eq!(
        ptr_read_volatile_macro_name(&Type::Primitive(sla_ast::Primitive::I64)),
        None
    );
}

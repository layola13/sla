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

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(sla_ast::UserDefined {
        name: name.to_string(),
        generics,
    })
}

#[test]
fn bare_call_shape_recognition() {
    assert!(is_bare_push_binary_call(&call(None, "push", 2)));
    assert!(!is_bare_push_binary_call(&call(None, "push", 1)));
    assert!(!is_bare_push_binary_call(&call(Some("Vec"), "push", 2)));
    assert!(!is_bare_push_binary_call(&call(None, "push_back", 2)));
    assert!(is_bare_len_unary_call(&call(None, "len", 1)));
    assert!(!is_bare_len_unary_call(&call(None, "len", 2)));
    assert!(!is_bare_len_unary_call(&call(Some("Vec"), "len", 1)));
}

#[test]
fn atomic_prefix_and_ordering_tokens() {
    assert_eq!(
        atomic_int_macro_prefix(&ud("AtomicI32", vec![])),
        Some("ATOMIC_I32")
    );
    assert_eq!(
        atomic_int_macro_prefix(&ud("AtomicUsize", vec![])),
        Some("ATOMIC_USIZE")
    );
    assert_eq!(atomic_int_macro_prefix(&ud("AtomicPtr", vec![])), None);
    assert_eq!(
        atomic_int_macro_prefix(&ud("AtomicI32", vec![Type::Primitive(Primitive::I32)])),
        None
    );
    let seq_cst = Node::Identifier("Ordering::SeqCst".to_string());
    assert_eq!(atomic_ordering_arg_token(&seq_cst).unwrap(), "seq_cst");
    let acq_rel = Node::Identifier("Ordering::AcqRel".to_string());
    assert_eq!(atomic_ordering_arg_token(&acq_rel).unwrap(), "acq_rel");
    let bogus = Node::Identifier("Ordering::Bogus".to_string());
    assert!(atomic_ordering_arg_token(&bogus).is_err());
    let literal = Node::Literal(sla_ast::Literal::IntVal(0));
    assert!(atomic_ordering_arg_token(&literal).is_err());
}

#[test]
fn push_ownership_and_type_facts() {
    // A non-Copy, non-borrow element transfers ownership on
    // push; Copy elements and borrow-like elements do not.
    assert!(vec_element_push_transfers_ownership(
        &ud("String", vec![]),
        false
    ));
    assert!(!vec_element_push_transfers_ownership(
        &ud("String", vec![]),
        true
    ));
    assert!(!vec_element_push_transfers_ownership(
        &Type::Primitive(Primitive::RawPtr),
        false
    ));
    assert!(is_string_type(&ud("String", vec![])));
    assert!(!is_string_type(&ud("String", vec![Type::Primitive(Primitive::U8)])));
    let map = ud(
        "HashMap",
        vec![
            Type::Primitive(Primitive::U64),
            Type::Primitive(Primitive::I32),
        ],
    );
    let (key, value) = map_types(&map, "HashMap").unwrap();
    assert_eq!(key, Type::Primitive(Primitive::U64));
    assert_eq!(value, Type::Primitive(Primitive::I32));
    assert!(map_types(&map, "BTreeMap").is_none());
}

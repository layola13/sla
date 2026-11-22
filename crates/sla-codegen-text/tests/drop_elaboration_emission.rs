//! Regression (RUST STAGE 10, RUST-S9-RUNTIME-DOUBLE-FREE):
//! releasing an owned `Rc`/`Arc`/`Box`/`Vec<T>`/struct binding in
//! the TEXT emitter used to be a bare `!reg` for every type, which
//! the SA host treats as an unconditional block free — a by-value
//! parameter's exit release freed the shared box with no count
//! decrement (caller reads stale counts, or glibc reports a
//! double free), struct field drops never fired, and the clone
//! bump a by-value struct copy performs on `Rc`/`Arc` fields was
//! never compensated by the source binding's cleanup.
//!
//! The fix mirrors the Zig emitter's DEF-ARC-DROP (F1) drop
//! elaboration (codegen.zig `emitRelease` plan dispatch +
//! `emitSmartPointerDropText`): releases now dispatch on the
//! binding's drop plan. Each test below pins one emission shape;
//! all but the two controls fail pre-fix (the emitted text has
//! the bare `!reg` and none of the drop macros).

use sla_ast::{
    Field, FuncDecl, Literal, Node, Param, Primitive, Program, ReturnStmt, StructDecl, Type,
    UserDefined,
};
use sla_lowering::MapCheckInfo;

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
}

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

fn param(name: &str, ty: Type) -> Param {
    Param {
        name: name.to_string(),
        ty: Box::new(ty),
        is_borrow: false,
        is_move: false,
        fallible_sugar: false,
        is_var: false,
    }
}

/// A one-parameter function whose tail cleanup list releases the
/// parameter: `fn f(p: <ty>) -> i64 { return 0; }` with the
/// checker cleanup list `[p]` on the return statement. Check-info
/// keys are node-identity based, so the cleanups are attached
/// through the statement node inside the built program.
fn release_param_program(ty: Type, extra_decls: Vec<Node>) -> (Node, MapCheckInfo) {
    let ret = Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(Node::Literal(Literal::IntVal(0)))),
    });
    let mut decls = extra_decls;
    decls.push(Node::FuncDecl(FuncDecl {
        name: "f".to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params: vec![param("p", ty)],
        ret_ty: Box::new(i64_ty()),
        body: vec![ret],
        is_inline: false,
        is_async: false,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_variadic: false,
    }));
    let program = Node::Program(Program { decls });
    let Node::Program(prog) = &program else {
        unreachable!()
    };
    let Node::FuncDecl(f) = prog.decls.last().expect("func decl") else {
        unreachable!()
    };
    let ret_node = f.body.last().expect("return stmt");
    let mut check = MapCheckInfo::new();
    check.add_cleanups(ret_node, vec!["p".to_string()]);
    if let Node::ReturnStmt(r) = ret_node {
        check.add_expr_type(r.value.as_ref().expect("return value"), i64_ty());
    }
    (program, check)
}

fn emit(program: &Node, check: &MapCheckInfo) -> String {
    sla_codegen_text::generate(program, check).expect("emission must succeed")
}

#[test]
fn rc_param_release_emits_rc_drop_sequence() {
    let (program, check) = release_param_program(ud("Rc", vec![i64_ty()]), vec![]);
    let out = emit(&program, &check);
    assert!(
        out.contains("EXPAND RC_DROP "),
        "an owned Rc release must run the RC_DROP conditional macro, got:\n{out}"
    );
    assert!(
        out.contains("EXPAND WEAK_DROP "),
        "the last-strong path must run the weak drop, got:\n{out}"
    );
    assert!(
        out.contains("RcBox_data"),
        "the last-strong path must load the payload from RcBox_data, got:\n{out}"
    );
    assert!(
        out.contains("call @sa_mem_free(^"),
        "the last-weak path must free the box allocation, got:\n{out}"
    );
}

#[test]
fn arc_param_release_emits_arc_drop_sequence() {
    let (program, check) = release_param_program(ud("Arc", vec![i64_ty()]), vec![]);
    let out = emit(&program, &check);
    assert!(
        out.contains("EXPAND ARC_DROP "),
        "an owned Arc release must run the ARC_DROP conditional macro, got:\n{out}"
    );
    assert!(
        out.contains("EXPAND ARC_WEAK_DROP "),
        "the last-strong path must run the ARC weak drop, got:\n{out}"
    );
    assert!(
        out.contains("ArcBox_data"),
        "the last-strong path must load the payload from ArcBox_data, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND RC_DROP "),
        "an Arc release must not emit RC macros, got:\n{out}"
    );
}

#[test]
fn box_param_release_frees_box() {
    let (program, check) = release_param_program(ud("Box", vec![i64_ty()]), vec![]);
    let out = emit(&program, &check);
    assert!(
        out.contains("Box_value"),
        "a Box release must load the payload at Box_value, got:\n{out}"
    );
    assert!(
        out.contains("call @sa_mem_free(^"),
        "a Box release must free the box allocation, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND RC_DROP ") && !out.contains("EXPAND ARC_DROP "),
        "a Box release must not emit count macros, got:\n{out}"
    );
}

#[test]
fn struct_param_release_drops_arc_field() {
    let holder = StructDecl {
        name: "Holder".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![
            Field {
                name: "data".to_string(),
                ty: Box::new(ud("Arc", vec![i64_ty()])),
            },
            Field {
                name: "n".to_string(),
                ty: Box::new(i64_ty()),
            },
        ],
        is_union: false,
        is_opaque: false,
    };
    let (program, mut check) = release_param_program(
        ud("Holder", vec![]),
        vec![Node::StructDecl(holder.clone())],
    );
    check.add_struct(holder);
    let out = emit(&program, &check);
    assert!(
        out.contains("load p+"),
        "a struct release must load its droppable fields out of the record, got:\n{out}"
    );
    assert!(
        out.contains("EXPAND ARC_DROP "),
        "the Arc field must be dropped by the struct release, got:\n{out}"
    );
}

#[test]
fn vec_of_arc_release_drops_elements_and_frees() {
    let (program, check) =
        release_param_program(ud("Vec", vec![ud("Arc", vec![i64_ty()])]), vec![]);
    let out = emit(&program, &check);
    assert!(
        out.contains("Vec_len"),
        "a Vec<Arc> release must walk the elements by Vec_len, got:\n{out}"
    );
    assert!(
        out.contains("EXPAND ARC_DROP "),
        "each Arc element must be dropped, got:\n{out}"
    );
    assert!(
        out.contains("EXPAND VEC_FREE p"),
        "the container itself must be freed after the element drops, got:\n{out}"
    );
}

#[test]
fn scalar_param_release_stays_bare() {
    let (program, check) = release_param_program(i64_ty(), vec![]);
    let out = emit(&program, &check);
    assert!(out.contains("!p"), "a scalar release is the bare `!p`, got:\n{out}");
    assert!(
        !out.contains("EXPAND RC_DROP ") && !out.contains("EXPAND VEC_FREE"),
        "a scalar release must not emit drop machinery, got:\n{out}"
    );
}

#[test]
fn vec_of_scalar_release_frees_buffer() {
    // Task 72 (DEF-RUST-VEC-SCOPE-EXIT-NO-FREE): planValueDrop(Vec<i64>)
    // is Buffer, and the old emitRelease plan switch had no Buffer arm,
    // so a plain Vec binding release stayed the generic `!` tail and
    // leaked the backing store at scope exit (churnops: ~200 MB over
    // 200k rounds). The Zig side frees Buffer bindings at scope exit
    // through its vec_buf_bindings map; the Rust side now does the same
    // via the new Buffer arm (EXPAND VEC_FREE), with no element drops
    // (scalar elements).
    let (program, check) = release_param_program(ud("Vec", vec![i64_ty()]), vec![]);
    let out = emit(&program, &check);
    assert!(
        out.contains("EXPAND VEC_FREE p"),
        "a Vec<i64> binding release must free its buffer, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND ARC_DROP "),
        "a Vec<i64> release must not emit element drops, got:\n{out}"
    );
}

#[test]
fn growvec_of_scalar_release_frees_buffer() {
    // Task 72, same defect on the GrowVec surface: planValueDrop
    // (GrowVec<i64>) is BufferGrowVec, which likewise fell through to
    // the generic `!` tail. The new BufferGrowVec arm emits GROWVEC_FREE.
    let (program, check) = release_param_program(ud("GrowVec", vec![i64_ty()]), vec![]);
    let out = emit(&program, &check);
    assert!(
        out.contains("EXPAND GROWVEC_FREE p"),
        "a GrowVec<i64> binding release must free its buffer, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND ARC_DROP "),
        "a GrowVec<i64> release must not emit element drops, got:\n{out}"
    );
}

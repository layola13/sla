//! Regression (RUST-TEXT-EMITTER stage 5a resume, R1b): a string
//! literal passed to a String-typed parameter must materialize as
//! an owned String handle and be MOVED into the call (Zig BM-D2,
//! codegen.zig ~21843/21923/22009).
//!
//! The two wrong forms, both trapped by the host on
//! `test_unit_reg1_dns_client_a` (`reg1_veq(&bytes, "lit")`):
//! passing the stack Slice view plain traps CapabilityMismatch
//! (the `^s` contract is unmet); moving the slice traps
//! StackEscape ("stack allocation cannot be passed by move").
//! The owned handle moved in is the only accepted form.
//!
//! Pre-fix, `generate` emits the plain slice pass with no owned
//! materialization; post-fix it emits
//! `STRING_BUF_FROM_UTF8_UNCHECKED` and `call @sla__take(^<owned>)`.

use sla_ast::{
    CallExpr, FuncDecl, Literal, Node, Param, Primitive, Program, ReturnStmt, Type,
};
use sla_lowering::MapCheckInfo;

fn string_ty() -> Type {
    Type::UserDefined(sla_ast::UserDefined {
        name: "String".to_string(),
        generics: Vec::new(),
    })
}

fn raw_ptr_ty() -> Type {
    Type::Primitive(Primitive::RawPtr)
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

fn func_decl(name: &str, params: Vec<Param>, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params,
        ret_ty: Box::new(void_ty()),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_variadic: false,
    })
}

#[test]
fn string_literal_arg_to_string_param_moves_owned_handle() {
    // fn take(s: String) {}
    // fn f() { take("hi"); return; }
    let call = Node::CallExpr(CallExpr {
        func_name: "take".to_string(),
        associated_target: None,
        generics: Vec::new(),
        args: vec![Node::Literal(Literal::StringVal("hi".to_string()))],
        callee: None,
    });
    let program = Node::Program(Program {
        decls: vec![
            func_decl(
                "take",
                vec![Param {
                    name: "s".to_string(),
                    ty: Box::new(string_ty()),
                    is_borrow: false,
                    is_move: false,
                    fallible_sugar: false,
                    is_var: false,
                }],
                vec![],
            ),
            func_decl(
                "f",
                vec![],
                vec![
                    Node::ExprStmt(Box::new(call)),
                    Node::ReturnStmt(ReturnStmt { value: None }),
                ],
            ),
        ],
    });
    let mut check = MapCheckInfo::new();
    if let Node::Program(prog) = &program {
        if let Node::FuncDecl(f) = &prog.decls[1] {
            if let Node::ExprStmt(inner) = &f.body[0] {
                if let Node::CallExpr(c) = &**inner {
                    check.add_expr_type(&c.args[0], raw_ptr_ty());
                }
            }
        }
    }
    let out = sla_codegen_text::generate(&program, &check)
        .expect("literal-to-String-param call must emit on the text backend");
    assert!(
        out.contains("STRING_BUF_FROM_UTF8_UNCHECKED"),
        "expected owned-String materialization for the literal arg, got:\n{out}"
    );
    let owned = out
        .lines()
        .find_map(|l| {
            let t = l.trim();
            t.strip_prefix("EXPAND STRING_BUF_FROM_UTF8_UNCHECKED ")
                .and_then(|rest| rest.split([',', ' ']).next())
                .map(str::to_string)
        })
        .expect("owned-String tmp must be named in the emission");
    assert!(
        out.contains(&format!("call @sla__take(^{owned})")),
        "expected the owned handle moved into the call, got:\n{out}"
    );
}

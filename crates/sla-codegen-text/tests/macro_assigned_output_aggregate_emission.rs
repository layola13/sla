//! Regression (RUST-TEXT-EMITTER stage 5a resume, R2 / dns1006):
//! a statement-form imported-macro call whose leading output is
//! an aggregate (user-defined) local assigned by the macro body
//! must pass a fresh temp to the expansion and write the result
//! back into the binding afterwards. Passing the binding straight
//! through traps RegisterRedefinition (1006) on the host: the
//! expansion's assignment redefines the live register, e.g.
//! SLICE_GET(rec, sl, i) on a DnsRecord local in
//! dns_records_of_type (Zig codegen.zig:14084).
//!
//! Pre-fix, `generate` emits `EXPAND SLICE_GET rec, sl, i`;
//! post-fix it emits `EXPAND SLICE_GET <tmp>, sl, i` followed by
//! a writeback (`rec = <tmp>`, after releasing the old value).

use sla_ast::{
    CallExpr, FuncDecl, Node, Param, Primitive, Program, ReturnStmt, Type,
};
use sla_lowering::{ImportedMacroView, MapCheckInfo};

fn dns_record_ty() -> Type {
    Type::UserDefined(sla_ast::UserDefined {
        name: "DnsRecord".to_string(),
        generics: Vec::new(),
    })
}

fn slice_of_dns_record_ty() -> Type {
    Type::UserDefined(sla_ast::UserDefined {
        name: "Slice".to_string(),
        generics: vec![dns_record_ty()],
    })
}

fn u64_ty() -> Type {
    Type::Primitive(Primitive::U64)
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

#[test]
fn assigned_output_aggregate_uses_temp_and_writeback() {
    // fn f(rec: DnsRecord, sl: Slice<DnsRecord>, i: u64) {
    //     SLICE_GET(rec, sl, i);
    //     return;
    // }
    let call = Node::CallExpr(CallExpr {
        func_name: "SLICE_GET".to_string(),
        associated_target: None,
        generics: Vec::new(),
        args: vec![
            Node::Identifier("rec".to_string()),
            Node::Identifier("sl".to_string()),
            Node::Identifier("i".to_string()),
        ],
        callee: None,
    });
    let program = Node::Program(Program {
        decls: vec![Node::FuncDecl(FuncDecl {
            name: "f".to_string(),
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: Vec::new(),
            params: vec![
                Param {
                    name: "rec".to_string(),
                    ty: Box::new(dns_record_ty()),
                    is_borrow: false,
                    is_move: false,
                    fallible_sugar: false,
                    is_var: false,
                },
                Param {
                    name: "sl".to_string(),
                    ty: Box::new(slice_of_dns_record_ty()),
                    is_borrow: false,
                    is_move: false,
                    fallible_sugar: false,
                    is_var: false,
                },
                Param {
                    name: "i".to_string(),
                    ty: Box::new(u64_ty()),
                    is_borrow: false,
                    is_move: false,
                    fallible_sugar: false,
                    is_var: false,
                },
            ],
            ret_ty: Box::new(void_ty()),
            body: vec![
                Node::ExprStmt(Box::new(call)),
                Node::ReturnStmt(ReturnStmt { value: None }),
            ],
            is_inline: false,
            is_async: false,
            operator: None,
            is_arena: false,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
            is_variadic: false,
        })],
    });
    let mut check = MapCheckInfo::new();
    check.add_imported_macro(
        "SLICE_GET",
        ImportedMacroView {
            arity: 3,
            leading_outputs: 1,
            import_path: Some("sa_std/core/slice.sa".to_string()),
            borrowed_arg_mask: 0,
            address_slot_arg_mask: 0,
            assigned_output_mask: 0b001,
            direct_callees: Vec::new(),
        },
    );
    if let Node::Program(prog) = &program {
        if let Node::FuncDecl(f) = &prog.decls[0] {
            if let Node::ExprStmt(inner) = &f.body[0] {
                if let Node::CallExpr(c) = &**inner {
                    check.add_expr_type(&c.args[0], dns_record_ty());
                    // `is_void_call` routes a statement macro call
                    // to gen_imported_macro_call_stmt only when the
                    // checker records the call itself as Void, as
                    // the real checker does for statement calls.
                    check.add_expr_type(inner, void_ty());
                }
            }
        }
    }
    let out = sla_codegen_text::generate(&program, &check)
        .expect("SLICE_GET statement call must emit on the text backend");
    let expand = out
        .lines()
        .find(|l| l.trim_start().starts_with("EXPAND SLICE_GET "))
        .expect("expected an EXPAND SLICE_GET line");
    assert!(
        !expand.contains("EXPAND SLICE_GET rec,"),
        "the live aggregate binding must not be passed to the expansion, got:\n{out}"
    );
    let temp = expand
        .trim_start()
        .strip_prefix("EXPAND SLICE_GET ")
        .and_then(|rest| rest.split(',').next())
        .map(str::trim)
        .expect("expansion temp must be named in the emission");
    assert!(
        out.contains(&format!("rec = {temp}")),
        "expected the writeback rebind of rec to the expansion temp, got:\n{out}"
    );
}

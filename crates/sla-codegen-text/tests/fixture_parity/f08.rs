//! Fixture 08 (R-4 sec 6.1): a HashMap-typed binding forces the
//! hashmap macro family into the generated program. The assertion is
//! on the emitted `SLA_MAP_INSERT_OPTION_U64` macro body: its SOME arm
//! mirrors Zig's `emitHashMapMacros` (codegen.zig:4108-4125) exactly —
//! it must NOT carry the two stray lines that free the old entry cell
//! pointer (`__sla_map_insert_free_<out>` + `call @sa_mem_free(^..)`),
//! which the assembler rejects as UnknownRegister on the ^-consumed
//! temp when the macro is expanded for a real insert.

use sla_ast::{LetStmt, Node, Primitive, Type, UserDefined};
use sla_lowering::MapCheckInfo;

use super::{func, int, program, register_expr_types, ret, test_decl};

fn hashmap_i64_u64() -> Type {
    Type::UserDefined(UserDefined {
        name: "HashMap".to_string(),
        generics: vec![
            Type::Primitive(Primitive::I64),
            Type::Primitive(Primitive::U64),
        ],
    })
}

pub fn build() -> (Node, MapCheckInfo) {
    let prog = program(vec![
        func(
            "main",
            &[],
            vec![
                Node::LetStmt(LetStmt {
                    name: "m".to_string(),
                    ty: Some(Box::new(hashmap_i64_u64())),
                    value: Box::new(int(0)),
                }),
                ret(int(0)),
            ],
        ),
        test_decl("fixture 08 hashmap macro body"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    (prog, info)
}

pub fn assert_insert_macro_body() {
    let (prog, info) = build();
    let out = sla_codegen_text::generate(&prog, &info)
        .unwrap_or_else(|err| panic!("fixture 08: generate failed: {err}"));
    assert!(
        out.contains("[MACRO] SLA_MAP_INSERT_OPTION_U64"),
        "fixture 08: hashmap family macro must be emitted"
    );
    assert!(
        !out.contains("__sla_map_insert_free_"),
        "fixture 08: SOME arm must not define __sla_map_insert_free_<out> \
         (Zig emitHashMapMacros has no free of the old entry pointer)"
    );
    assert!(
        !out.contains("call @sa_mem_free(^__sla_map_insert_free_"),
        "fixture 08: SOME arm must not call @sa_mem_free on the free temp"
    );
}

//! R-3 impl/trait parser regression tests, split out of
//! decl.rs for the 700-line file cap (bodies unchanged).

#[cfg(test)]
mod tests_r3_impl {
    use crate::parser::{Parser, ParserOptions};
    use sla_ast::decls::ImplDecl;
    use sla_ast::node::Node;
    use sla_ast::types::Type;

    fn parse(src: &str) -> Result<Vec<Node>, crate::error::ParseError> {
        let mut p = Parser::with_options(src, ParserOptions::default());
        match p.parse_program()? {
            Node::Program(prog) => Ok(prog.decls),
            _ => panic!("root not program"),
        }
    }

    fn find_impl(decls: &[Node]) -> &ImplDecl {
        decls
            .iter()
            .find_map(|d| match d {
                Node::ImplDecl(i) => Some(i),
                _ => None,
            })
            .expect("impl decl present")
    }

    #[test]
    fn r3_impl_inherent_block() {
        // Corpus shape: demos/basic/12_impl_counter.
        let src = "struct Counter { count: i32 }\nimpl Counter {\n    fn new() -> Counter { return Counter { count: 0 }; }\n    fn inc(self) -> Counter { return Counter { count: self.count + 1 }; }\n}\n";
        let decls = parse(src).expect("impl parses");
        let imp = find_impl(&decls);
        assert_eq!(imp.trait_name, None);
        assert!(matches!(&*imp.target_ty, Type::UserDefined(u) if u.name == "Counter"));
        assert_eq!(imp.methods.len(), 2);
        let Node::FuncDecl(inc) = &imp.methods[1] else {
            panic!("method is a func decl")
        };
        assert_eq!(inc.name, "inc");
        assert_eq!(inc.params.len(), 1);
        assert_eq!(inc.params[0].name, "self");
        // self takes the impl target type.
        assert!(matches!(&*inc.params[0].ty, Type::UserDefined(u) if u.name == "Counter"));
    }

    #[test]
    fn r3_impl_trait_for_block() {
        let src = "struct P { x: i32 }\nimpl Draw for P {\n    fn draw(&self) -> i32 { return self.x; }\n}\n";
        let decls = parse(src).expect("trait impl parses");
        let imp = find_impl(&decls);
        assert_eq!(imp.trait_name.as_deref(), Some("Draw"));
        assert!(matches!(&*imp.target_ty, Type::UserDefined(u) if u.name == "P"));
        let Node::FuncDecl(m) = &imp.methods[0] else {
            panic!("method is a func decl")
        };
        assert!(m.params[0].is_borrow);
    }

    #[test]
    fn r3_impl_self_resolves_to_target() {
        // Corpus shape: tests/test_unit_impl_static_methods. `Self`
        // in a method signature resolves to the impl TARGET type at
        // parse time (finish_named_type + current_impl_target),
        // exactly as Zig's makeUserDefinedType("Self") indirection
        // does through the checker.
        let src = "struct S { a: i32 }\nimpl S {\n    fn make(a: i32) -> Self { return Self { a: a }; }\n}\n";
        let decls = parse(src).expect("parses");
        let imp = find_impl(&decls);
        let Node::FuncDecl(m) = &imp.methods[0] else {
            panic!("method is a func decl")
        };
        assert!(matches!(&*m.ret_ty, Type::UserDefined(u) if u.name == "S"));
    }

    #[test]
    fn r3_impl_self_literal_head_shape() {
        // `Self { ... }` as an expression parses as a struct literal
        // whose type is the user-defined name "Self" (Zig's AST
        // shape; the checker resolves it against the impl target).
        let src = "struct S { a: i32 }\nimpl S {\n    fn make(a: i32) -> Self { return Self { a: a }; }\n}\n";
        let decls = parse(src).expect("parses");
        let imp = find_impl(&decls);
        let Node::FuncDecl(m) = &imp.methods[0] else {
            panic!("method is a func decl")
        };
        let Node::ReturnStmt(r) = &m.body[0] else {
            panic!("body is a return")
        };
        let Node::StructLiteral(lit) = r.value.as_deref().expect("value") else {
            panic!("return value is a struct literal")
        };
        assert!(matches!(&*lit.ty, Type::UserDefined(u) if u.name == "Self"));
    }
}

#[cfg(test)]
mod tests_r3_trait {
    use crate::parser::{Parser, ParserOptions};
    use sla_ast::decls::TraitDecl;
    use sla_ast::node::Node;
    use sla_ast::types::Type;

    fn parse(src: &str) -> Result<Vec<Node>, crate::error::ParseError> {
        let mut p = Parser::with_options(src, ParserOptions::default());
        match p.parse_program()? {
            Node::Program(prog) => Ok(prog.decls),
            _ => panic!("root not program"),
        }
    }

    fn find_trait(decls: &[Node]) -> &TraitDecl {
        decls
            .iter()
            .find_map(|d| match d {
                Node::TraitDecl(t) => Some(t),
                _ => None,
            })
            .expect("trait decl present")
    }

    #[test]
    fn r3_trait_simple() {
        // Corpus shape: tests/test_unit_trait_static_dispatch.
        let src = "trait Score {\n    fn score(&self) -> i32;\n}\n";
        let decls = parse(src).expect("trait parses");
        let t = find_trait(&decls);
        assert_eq!(t.name, "Score");
        assert!(t.supertraits.is_empty());
        assert_eq!(t.methods.len(), 1);
        assert_eq!(t.methods[0].name, "score");
        assert!(t.methods[0].params[0].is_borrow);
        assert!(matches!(&*t.methods[0].params[0].ty, Type::UserDefined(u) if u.name == "Self"));
    }

    #[test]
    fn r3_trait_supertrait_chain() {
        let src = "trait Child: Parent {\n    fn child_value(&self) -> i32;\n}\n";
        let decls = parse(src).expect("parses");
        let t = find_trait(&decls);
        assert_eq!(t.supertraits, vec!["Parent".to_string()]);
    }

    #[test]
    fn r3_trait_multi_method_static() {
        let src = "trait T {\n    fn a(x: i32) -> i32;\n    fn b(&self, y: i32) -> i32;\n}\n";
        let decls = parse(src).expect("parses");
        let t = find_trait(&decls);
        assert_eq!(t.methods.len(), 2);
        assert_eq!(t.methods[1].params.len(), 2);
    }
}

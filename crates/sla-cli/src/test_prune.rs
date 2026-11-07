//! Test-codegen pre-check pruning — the port of the two pruning
//! stages `runSlaFrontend` runs for `prune_for_test_codegen`
//! programs BEFORE `checkProgram` (plugin_compile.zig, read at Zig
//! head 2cc12ea8):
//!
//!   1. `pruneTestsByFilter` (plugin_compile.zig) — drop @test
//!      decls whose name does not contain the filter pattern.
//!   2. `pruneUnreachableTestFunctionDeclsBeforeTypeCheck`
//!      (plugin_compile.zig, pre-typecheck call site, arguments
//!      prune_known_branches=true / rewrite_project_shortcuts=true)
//!      — syntactic reachability from the @test / const / trait-impl
//!      / overload roots over plugin_reachability.zig's
//!      `SlaCallableIndex` + `collectSyntacticReachable{Expr,Block}`
//!      + `scanReferencedSymbolRoots`, then drop unreachable
//!      function decls, unreachable impl/overload methods, and
//!      unreferenced macro decls.
//!
//! Running both BEFORE the checker is the point of CLI-D5: the
//! checker's cleanup tables are keyed by node address
//! (sla-lowering `node_key`), so any decl dropped after the check
//! leaves the surviving nodes' keys pointing at a program shape
//! the checker never saw, and codegen loses the cleanups
//! (MemoryLeak in the SAB test input). Pruning first makes the
//! checker see exactly the program codegen consumes.
//!
//! Registered scope differences against the Zig original (see
//! docs/divergences/cli-commands.md, CLI-D5):
//! - The `SyntacticFactSet` constant-fact analysis is NOT ported:
//!   `pruneKnownFalseBranchesInReachableDecls` does not run, and
//!   receiver / associated-target type names are never resolved
//!   from local let facts. Where Zig's facts would narrow the
//!   reachable set, this port keeps a SUPERSET (an unresolvable
//!   receiver marks every associated candidate — Zig's own
//!   fallback when its typed marking finds no candidate). No decl
//!   Zig keeps is ever dropped because of this difference.
//! - No module table / imported macros: this pipeline's program
//!   is the root file only (.sla import expansion and contract
//!   loading live in the parser / contract layers), so
//!   module-qualified aliasing, imported-mangled resolution and
//!   imported-macro direct-callee expansion have no counterpart.
//! - `rewriteProjectSnapshotTestShortcuts` is not ported (no
//!   counterpart anywhere in this pipeline); the retained-helper
//!   name list of `isProjectShortcutRetainedHelperName` IS
//!   mirrored verbatim below.

use sla_ast::node::Node;

// -- filter prune (plugin_compile.zig pruneTestsByFilter) -----------------------

/// Zig `testMatchesFilter` (plugin_reachability.zig): a null or
/// empty pattern matches every test; otherwise substring match.
pub fn test_matches_filter(name: &str, filter: Option<&str>) -> bool {
    match filter {
        None => true,
        Some(pattern) => pattern.is_empty() || name.contains(pattern),
    }
}

/// Zig `pruneTestsByFilter`.
pub fn prune_tests_by_filter(root: &mut Node, filter: Option<&str>) {
    let Some(pattern) = filter.filter(|p| !p.is_empty()) else {
        return;
    };
    let Node::Program(program) = root else {
        return;
    };
    program.decls.retain(|decl| match decl {
        Node::TestDecl(test) => test_matches_filter(&test.name, Some(pattern)),
        _ => true,
    });
}

// -- the reachability prune (plugin_compile.zig, pre-typecheck call site) -----------

use crate::test_reach::{
    collect_test_reachability, concrete_type_name, is_project_shortcut_retained_helper_name,
};

/// Zig `pruneUnreachableTestFunctionDeclsBeforeTypeCheck`
/// (pre-typecheck call site; the reachability analysis lives in
/// crate::test_reach, the decl filtering here). No-op when the
/// program declares no callable or no @test.
pub fn prune_unreachable_test_function_decls_before_type_check(root: &mut Node) {
    let Node::Program(program) = root else {
        return;
    };
    let Some((reachable, referenced_types)) = collect_test_reachability(&program.decls) else {
        return;
    };

    let mut filtered: Vec<Node> = Vec::with_capacity(program.decls.len());
    for decl in std::mem::take(&mut program.decls) {
        match decl {
            Node::FuncDecl(fd) => {
                if fd.is_decl_only
                    || reachable.contains(&fd.name)
                    || is_project_shortcut_retained_helper_name(&fd.name)
                {
                    filtered.push(Node::FuncDecl(fd));
                }
            }
            Node::ImplDecl(impl_decl) => {
                if impl_decl.trait_name.is_some() {
                    filtered.push(Node::ImplDecl(impl_decl));
                    continue;
                }
                let Some(type_name) = concrete_type_name(&impl_decl.target_ty) else {
                    filtered.push(Node::ImplDecl(impl_decl));
                    continue;
                };
                let type_name = type_name.to_string();
                let mut methods: Vec<Node> = Vec::with_capacity(impl_decl.methods.len());
                for method in impl_decl.methods {
                    match &method {
                        Node::FuncDecl(fd) => {
                            let symbol =
                                sla_lowering::names::mangle_method_name(&type_name, &fd.name);
                            if fd.is_decl_only || reachable.contains(&symbol) {
                                methods.push(method);
                            }
                        }
                        _ => methods.push(method),
                    }
                }
                if !methods.is_empty() {
                    filtered.push(Node::ImplDecl(sla_ast::ImplDecl {
                        trait_name: impl_decl.trait_name,
                        target_ty: impl_decl.target_ty,
                        methods,
                    }));
                }
            }
            Node::OverloadDecl(overload_decl) => {
                let Some(type_name) = concrete_type_name(&overload_decl.target_ty) else {
                    filtered.push(Node::OverloadDecl(overload_decl));
                    continue;
                };
                let type_name = type_name.to_string();
                let mut methods: Vec<Node> = Vec::with_capacity(overload_decl.methods.len());
                for method in overload_decl.methods {
                    match &method {
                        Node::FuncDecl(fd) => {
                            let symbol =
                                sla_lowering::names::mangle_method_name(&type_name, &fd.name);
                            if fd.is_decl_only || reachable.contains(&symbol) {
                                methods.push(method);
                            }
                        }
                        _ => methods.push(method),
                    }
                }
                if !methods.is_empty() {
                    filtered.push(Node::OverloadDecl(sla_ast::OverloadDecl {
                        target_ty: overload_decl.target_ty,
                        methods,
                    }));
                }
            }
            Node::MacroDecl(md) => {
                if referenced_types.contains(&md.name) {
                    filtered.push(Node::MacroDecl(md));
                }
            }
            other => filtered.push(other),
        }
    }
    program.decls = filtered;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::node::Node as AstNode;

    const SRC: &str = r#"
fn while_sum(limit: i32) -> i32 {
    let t: i32 = 0;
    let i: i32 = 0;
    while i < limit {
        t = t + i;
        i = i + 1;
    }
    return t;
}

fn only_alpha() -> i32 { return 1; }
fn only_beta() -> i32 { return 2; }
fn unused_helper() -> i32 { return 3; }

fn main() -> i32 {
    return while_sum(10);
}

@test "alpha case"() {
    if while_sum(10) != 45 { panic(401); };
    if only_alpha() != 1 { panic(402); };
}

@test "beta case"() {
    if only_beta() != 2 { panic(403); };
}
"#;

    fn parse(src: &str) -> AstNode {
        let options = sla_parser::ParserOptions {
            parse_test_bodies: true,
            ..Default::default()
        };
        let mut parser = sla_parser::Parser::with_options(src, options);
        parser.parse_program().expect("test source parses")
    }

    fn decl_names(root: &AstNode) -> Vec<String> {
        let AstNode::Program(program) = root else {
            panic!("program root");
        };
        program
            .decls
            .iter()
            .map(|decl| match decl {
                AstNode::FuncDecl(fd) => format!("fn {}", fd.name),
                AstNode::TestDecl(td) => format!("test {}", td.name),
                AstNode::MacroDecl(md) => format!("macro {}", md.name),
                _ => "other".to_string(),
            })
            .collect()
    }

    #[test]
    fn filter_prune_keeps_only_matching_tests() {
        let mut root = parse(SRC);
        prune_tests_by_filter(&mut root, Some("alpha"));
        let names = decl_names(&root);
        assert!(names.contains(&"test alpha case".to_string()), "{names:?}");
        assert!(!names.contains(&"test beta case".to_string()), "{names:?}");
        // Filter prune never touches non-test decls.
        assert!(names.contains(&"fn unused_helper".to_string()), "{names:?}");
    }

    #[test]
    fn reachability_prune_drops_main_and_unreachable() {
        let mut root = parse(SRC);
        prune_unreachable_test_function_decls_before_type_check(&mut root);
        let names = decl_names(&root);
        // Reachable from the two tests: while_sum + only_alpha + only_beta.
        assert!(names.contains(&"fn while_sum".to_string()), "{names:?}");
        assert!(names.contains(&"fn only_alpha".to_string()), "{names:?}");
        assert!(names.contains(&"fn only_beta".to_string()), "{names:?}");
        // main is not a test root and nothing reaches it or the
        // unused helper: both are pruned before the check.
        assert!(!names.contains(&"fn main".to_string()), "{names:?}");
        assert!(!names.contains(&"fn unused_helper".to_string()), "{names:?}");
        // Tests themselves are never pruned by reachability.
        assert!(names.contains(&"test alpha case".to_string()), "{names:?}");
        assert!(names.contains(&"test beta case".to_string()), "{names:?}");
    }

    #[test]
    fn filter_then_reachability_drops_filtered_tests_callees() {
        // Stage order pin: the filter prune runs first, so a
        // filtered-out test's private callee becomes unreachable
        // and is dropped by the reachability prune.
        let mut root = parse(SRC);
        prune_tests_by_filter(&mut root, Some("alpha"));
        prune_unreachable_test_function_decls_before_type_check(&mut root);
        let names = decl_names(&root);
        assert!(names.contains(&"fn only_alpha".to_string()), "{names:?}");
        assert!(!names.contains(&"fn only_beta".to_string()), "{names:?}");
    }

    #[test]
    fn no_tests_means_no_reachability_prune() {
        let mut root = parse("fn main() -> i32 { return 1; }\nfn spare() -> i32 { return 2; }\n");
        let before = decl_names(&root);
        prune_unreachable_test_function_decls_before_type_check(&mut root);
        assert_eq!(decl_names(&root), before);
    }

    #[test]
    fn pruned_program_still_type_checks() {
        // The stage-order contract: the checker consumes exactly
        // the pruned program (its node-keyed tables are built from
        // these nodes), and the pruned program is well-typed.
        let mut root = parse(SRC);
        prune_tests_by_filter(&mut root, Some("alpha"));
        prune_unreachable_test_function_decls_before_type_check(&mut root);
        let AstNode::Program(program) = &root else {
            panic!("program root");
        };
        let mut checker = sla_checker::Checker::new();
        checker
            .check_program(program)
            .expect("pruned program type-checks");
        // And the checker's tables are keyed on nodes that are in
        // the pruned tree: every expr-type key must resolve
        // against a node reachable from the root. (Spot-check via
        // the surviving helper's identity: its decl node key is
        // not in the tables — decls are not exprs — so instead
        // assert the tables are non-empty and all keys are
        // addresses inside the tree's allocation range is not
        // sound; the load-bearing assertion is the successful
        // check above plus the decl-set pins.)
        assert!(!checker.expr_types.is_empty());
    }
}

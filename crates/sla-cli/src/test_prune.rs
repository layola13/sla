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
//!
//! A1 line (2026-11-14): this module also hosts the Compile
//! kind's pre-check prune — same stage position (after .sla
//! import expansion, before contract load and checkProgram),
//! same analysis (crate::test_reach), different roots: the
//! primary (root-file) decls. See
//! `prune_unreachable_imported_function_decls_before_type_check`.

use sla_ast::node::Node;
use std::collections::BTreeSet;

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
    record_referenced_type, CallableIndex, Collector,
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

/// Which decls of the flattened (post-expansion) program
/// are the root file's own: sequential structural match of
/// the pre-expansion parse against the flattened program.
/// The include-all flatten emits the root's non-import
/// decls verbatim, in order, with each import's splice in
/// place of the import decl, so a greedy in-order scan
/// recovers exactly the root positions (a splice decl
/// structurally identical to the next root decl would take
/// its flag — the two are interchangeable for pruning).
/// sla_expand's `primary` cannot serve here: the include-all
/// flatten marks every materialized decl primary.
pub fn root_decl_flags(root_decls: &[Node], expanded_decls: &[Node]) -> Vec<bool> {
    let mut flags = vec![false; expanded_decls.len()];
    let mut j = 0usize;
    for decl in root_decls {
        if matches!(decl, Node::ImportDecl(_)) {
            continue;
        }
        while j < expanded_decls.len() {
            let hit = &expanded_decls[j] == decl;
            flags[j] = hit;
            j += 1;
            if hit {
                break;
            }
        }
    }
    flags
}

/// Compile-kind roots (A1 line, 2026-11-14): the same closure
/// as crate::test_reach's `collect_test_reachability`, but
/// rooted at the primary (root-file) decls — the root
/// discipline of Zig's reachability-selective expansion for
/// the compile front-end (IMP-1), in this pipeline's
/// single-module form. @test decls are NOT roots here: a
/// build does not build tests (the test kind owns them), so
/// bodies only a test calls are not part of the build
/// product. `root_flags` is parallel to `decls` and marks
/// the root file's own decls (see `root_decl_flags`):
/// every root callable is a root (marked reachable, its
/// body scanned by the closure), root consts are roots
/// exactly as in the test scan, and root trait-impl bodies
/// are collected as roots exactly as there. Imported decls
/// enter only through the closure. Returns None when the
/// prune must not run (flag/decl length mismatch, or no
/// callable indexed); a root set with no callable roots is
/// NOT a decline — the reachable set is empty and every
/// imported body is pruned (a tests-only root file builds
/// to its own decls alone, its baseline shape).
pub(crate) fn collect_compile_reachability(
    decls: &[Node],
    root_flags: &[bool],
) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    if root_flags.len() != decls.len() {
        return None;
    }
    let mut index = CallableIndex::new();
    index.add_decls(decls);
    if index.names.is_empty() {
        return None;
    }
    let mut collector = Collector::new(&index);

    for (decl, is_root) in decls.iter().zip(root_flags.iter()) {
        if !is_root {
            continue;
        }
        match decl {
            Node::FuncDecl(fd) => {
                collector.mark_reachable_func(&fd.name);
            }
            Node::ConstStmt(const_stmt) => {
                if let Some(ty) = &const_stmt.ty {
                    record_referenced_type(&mut collector.referenced_types, ty);
                }
                collector.collect_expr(&const_stmt.value, None);
            }
            Node::MacroDecl(md) => {
                collector.referenced_types.insert(md.name.clone());
            }
            Node::ImplDecl(impl_decl) => {
                record_referenced_type(&mut collector.referenced_types, &impl_decl.target_ty);
                if let Some(trait_name) = &impl_decl.trait_name {
                    collector.referenced_types.insert(trait_name.clone());
                    let Some(type_name) = concrete_type_name(&impl_decl.target_ty) else {
                        continue;
                    };
                    for method in &impl_decl.methods {
                        let Node::FuncDecl(fd) = method else { continue };
                        let symbol = sla_lowering::names::mangle_trait_method_name(
                            type_name,
                            trait_name,
                            &fd.name,
                        );
                        collector.collect_block(&fd.body, Some(&symbol));
                    }
                } else {
                    let Some(type_name) = concrete_type_name(&impl_decl.target_ty) else {
                        continue;
                    };
                    for method in &impl_decl.methods {
                        let Node::FuncDecl(fd) = method else { continue };
                        let symbol =
                            sla_lowering::names::mangle_method_name(type_name, &fd.name);
                        collector.mark_reachable_func(&symbol);
                    }
                }
            }
            Node::OverloadDecl(overload_decl) => {
                record_referenced_type(&mut collector.referenced_types, &overload_decl.target_ty);
                let Some(type_name) = concrete_type_name(&overload_decl.target_ty) else {
                    continue;
                };
                for method in &overload_decl.methods {
                    let Node::FuncDecl(fd) = method else { continue };
                    let symbol = sla_lowering::names::mangle_method_name(type_name, &fd.name);
                    collector.mark_reachable_func(&symbol);
                }
            }
            _ => {}
        }
    }

    let mut index_i = 0;
    loop {
        while index_i < collector.worklist.len() {
            let name = collector.worklist[index_i].clone();
            index_i += 1;
            let Some(fd) = index.decls.get(&name) else {
                continue;
            };
            let fd = *fd;
            for param in &fd.params {
                record_referenced_type(&mut collector.referenced_types, &param.ty);
            }
            record_referenced_type(&mut collector.referenced_types, &fd.ret_ty);
            collector.collect_block(&fd.body, Some(&name));
        }
        let expanded = collector.expand_reachable_overload_members(decls);
        let scanned = collector.scan_referenced_symbol_roots();
        if !expanded && !scanned {
            break;
        }
    }
    Some((collector.reachable, collector.referenced_types))
}

/// Compile-kind counterpart (A1 line, 2026-11-14): after the
/// bodies-kept include-all expansion, drop the imported
/// function / method bodies that are not reachable from the
/// root file's own decls, BEFORE contract load and
/// checkProgram — the root discipline of Zig's
/// reachability-selective compile expansion (IMP-1) in this
/// pipeline's single-module form. The SAB tail dispatches
/// per decl, so an unreached imported body left in the
/// program would be checked and emitted like a root body;
/// pruning first makes the checker and the tail see exactly
/// the program the selective expansion would have
/// materialized. Root decls are never dropped. No-op when
/// the reachability scan declines (mismatched flags, or no
/// callable indexed).
pub fn prune_unreachable_imported_function_decls_before_type_check(
    root: &mut Node,
    root_flags: &[bool],
) {
    let Node::Program(program) = root else {
        return;
    };
    let Some((reachable, referenced_types)) =
        collect_compile_reachability(&program.decls, root_flags)
    else {
        return;
    };

    let mut filtered: Vec<Node> = Vec::with_capacity(program.decls.len());
    for (decl, is_root) in std::mem::take(&mut program.decls)
        .into_iter()
        .zip(root_flags.iter())
    {
        if *is_root {
            filtered.push(decl);
            continue;
        }
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

    #[test]
    fn root_decl_flags_recover_root_positions_across_splices() {
        let root = parse(
            "@import \"x.sla\"\nfn main() -> i32 { return helper(); }\nfn helper() -> i32 { return 1; }",
        );
        let expanded = parse(
            "fn imported_a() -> i32 { return 9; }\nfn main() -> i32 { return helper(); }\nfn imported_b() -> i32 { return 8; }\nfn helper() -> i32 { return 1; }",
        );
        let AstNode::Program(root_program) = &root else {
            panic!("program root");
        };
        let AstNode::Program(expanded_program) = &expanded else {
            panic!("program root");
        };
        let flags = root_decl_flags(&root_program.decls, &expanded_program.decls);
        assert_eq!(flags, vec![false, true, false, true]);
    }

    #[test]
    fn compile_prune_drops_unreached_imported_bodies() {
        let root = parse(
            "@import \"x.sla\"\nfn main() -> i32 { return helper(); }\nfn helper() -> i32 { return 1; }",
        );
        let mut expanded = parse(
            "fn imported_a() -> i32 { return 9; }\nfn main() -> i32 { return helper(); }\nfn imported_b() -> i32 { return 8; }\nfn helper() -> i32 { return 1; }",
        );
        let AstNode::Program(root_program) = &root else {
            panic!("program root");
        };
        let AstNode::Program(expanded_program) = &expanded else {
            panic!("program root");
        };
        let flags = root_decl_flags(&root_program.decls, &expanded_program.decls);
        prune_unreachable_imported_function_decls_before_type_check(&mut expanded, &flags);
        let names = decl_names(&expanded);
        assert_eq!(
            names,
            vec!["fn main".to_string(), "fn helper".to_string()],
            "{names:?}"
        );
    }
}

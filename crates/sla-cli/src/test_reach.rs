//! Syntactic reachability for the test-codegen pre-check prune —
//! the plugin_reachability.zig machinery (SlaCallableIndex +
//! collectSyntacticReachable{Expr,Block} + scanReferencedSymbolRoots)
//! in the single-root-module form this pipeline needs. Consumed
//! only by crate::test_prune; see that module's doc for the stage
//! order and the registered scope differences (no SyntacticFactSet
//! analysis, no module table / imported macros).

use sla_ast::node::Node;
use sla_ast::Type;
use std::collections::{BTreeMap, BTreeSet};

// -- small lowering_rules mirrors -------------------------------------------------

/// lowering_rules `concreteTypeName`: peel borrow/pointer, then a
/// user-defined type's name.
pub(crate) fn concrete_type_name(ty: &Type) -> Option<&str> {
    match ty {
        Type::Pointer(inner) | Type::Borrow(inner) => concrete_type_name(inner),
        Type::UserDefined(ud) => Some(&ud.name),
        _ => None,
    }
}

/// plugin_reachability.zig `recordReferencedType`.
pub(crate) fn record_referenced_type(referenced: &mut BTreeSet<String>, ty: &Type) {
    match ty {
        Type::Array(arr) => record_referenced_type(referenced, &arr.elem),
        Type::Tuple(tup) => {
            for elem in &tup.elems {
                record_referenced_type(referenced, elem);
            }
        }
        Type::Future(inner) => record_referenced_type(referenced, inner),
        Type::Closure(cl) => {
            for param in &cl.params {
                record_referenced_type(referenced, param);
            }
            record_referenced_type(referenced, &cl.ret);
        }
        Type::FnPtr(fp) => {
            for param in &fp.params {
                record_referenced_type(referenced, param);
            }
            record_referenced_type(referenced, &fp.ret);
        }
        Type::UserDefined(ud) => {
            referenced.insert(ud.name.clone());
            for generic in &ud.generics {
                record_referenced_type(referenced, generic);
            }
        }
        _ => {}
    }
}

/// plugin_project_shortcuts.zig `isProjectShortcutRetainedHelperName`
/// (suffix list verbatim).
pub(crate) fn is_project_shortcut_retained_helper_name(name: &str) -> bool {
    const SUFFIXES: &[&str] = &[
        "project_empty_program",
        "project_empty_project",
        "project_config_file_registry_from_config",
        "project_file_change_summary_empty",
        "project_file_change_summary_change",
        "project_performance_telemetry_empty",
        "project_info_telemetry_empty",
        "program_options_default",
        "default_compiler_options",
        "program_state_from_counts",
        "program_new",
        "program_empty_source_file",
        "program_processed_files_empty",
        "program_checker_pool_new",
        "program_resolver_state_empty",
        "program_empty_resolved_module_entry",
        "program_empty_type_resolution_entry",
        "program_empty_package_json_cache_entry",
        "diagnostic_from_parse_error",
        "diagnostic_collection_empty",
        "diagnostic_collection_add_error",
        "diagnostic_collection_has_errors",
    ];
    SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

// -- callable index (plugin_reachability.zig SlaCallableIndex, one module) -------

pub(crate) struct CallableIndex<'a> {
    names: BTreeSet<String>,
    decls: BTreeMap<String, &'a sla_ast::FuncDecl>,
    const_decls: BTreeMap<String, &'a sla_ast::ConstStmt>,
    macro_decls: BTreeMap<String, &'a sla_ast::MacroDecl>,
    associated_candidates: BTreeMap<String, Vec<String>>,
}

impl<'a> CallableIndex<'a> {
    /// Zig `recordFunction` (first declaration wins), including
    /// the flattened-suffix candidates and the `__ov_` plain-name
    /// alias.
    fn record_function(&mut self, name: &str, fd: &'a sla_ast::FuncDecl) {
        self.names.insert(name.to_string());
        self.decls.entry(name.to_string()).or_insert(fd);
        self.add_flattened_suffix_candidates(name);
        if let Some(marker) = name.find("__ov_") {
            if marker > 0 {
                let plain = &name[..marker];
                self.names.insert(plain.to_string());
                self.decls.entry(plain.to_string()).or_insert(fd);
            }
        }
    }

    /// Zig `addFlattenedSuffixCandidates`: every `_`/` :` boundary
    /// not followed by the same class registers the suffix as an
    /// associated candidate of the whole symbol.
    fn add_flattened_suffix_candidates(&mut self, symbol: &str) {
        let bytes = symbol.as_bytes();
        for (index, &ch) in bytes.iter().enumerate() {
            if ch != b'_' && ch != b':' {
                continue;
            }
            if index + 1 >= bytes.len() {
                continue;
            }
            let next = bytes[index + 1];
            if next == b'_' || next == b':' {
                continue;
            }
            let suffix = &symbol[index + 1..];
            self.add_associated_candidate(suffix, symbol);
        }
    }

    fn add_associated_candidate(&mut self, method_name: &str, symbol: &str) {
        self.associated_candidates
            .entry(method_name.to_string())
            .or_default()
            .push(symbol.to_string());
    }

    /// Zig `addAssociatedFunction` / `addAssociatedFunctionWithModule`
    /// (module = null in this pipeline).
    fn add_associated_function(
        &mut self,
        method_name: &str,
        symbol: &str,
        fd: &'a sla_ast::FuncDecl,
    ) {
        self.record_function(symbol, fd);
        self.add_associated_candidate(method_name, symbol);
    }

    /// Zig `addDecls` (single root module).
    fn add_decls(&mut self, decls: &'a [Node]) {
        for decl in decls {
            match decl {
                Node::FuncDecl(fd) => self.record_function(&fd.name, fd),
                Node::ConstStmt(cs) => {
                    self.const_decls.entry(cs.name.clone()).or_insert(cs);
                }
                Node::MacroDecl(md) => {
                    self.macro_decls.entry(md.name.clone()).or_insert(md);
                }
                Node::ImplDecl(impl_decl) => {
                    let Some(type_name) = concrete_type_name(&impl_decl.target_ty) else {
                        continue;
                    };
                    for method in &impl_decl.methods {
                        let Node::FuncDecl(fd) = method else { continue };
                        let symbol = match &impl_decl.trait_name {
                            Some(trait_name) => sla_lowering::names::mangle_trait_method_name(
                                type_name, trait_name, &fd.name,
                            ),
                            None => {
                                sla_lowering::names::mangle_method_name(type_name, &fd.name)
                            }
                        };
                        self.add_associated_function(&fd.name, &symbol, fd);
                    }
                }
                Node::OverloadDecl(overload_decl) => {
                    let Some(type_name) = concrete_type_name(&overload_decl.target_ty) else {
                        continue;
                    };
                    for method in &overload_decl.methods {
                        let Node::FuncDecl(fd) = method else { continue };
                        let symbol =
                            sla_lowering::names::mangle_method_name(type_name, &fd.name);
                        self.add_associated_function(&fd.name, &symbol, fd);
                    }
                }
                _ => {}
            }
        }
    }

    /// Zig `syntacticFuncDeclForCall` minus the module-table arms:
    /// a direct decl, or the single associated candidate's decl.
    fn func_decl_for_call(&self, name: &str) -> Option<&'a sla_ast::FuncDecl> {
        if let Some(fd) = self.decls.get(name) {
            return Some(fd);
        }
        if let Some(candidates) = self.associated_candidates.get(name) {
            if candidates.len() == 1 {
                if let Some(fd) = self.decls.get(&candidates[0]) {
                    return Some(fd);
                }
            }
        }
        None
    }
}

// -- syntactic reachability collector ----------------------------------------------

pub(crate) struct Collector<'a> {
    index: &'a CallableIndex<'a>,
    reachable: BTreeSet<String>,
    referenced_types: BTreeSet<String>,
    worklist: Vec<String>,
    scanned_symbol_roots: BTreeSet<String>,
}

impl<'a> Collector<'a> {
    fn new(index: &'a CallableIndex<'a>) -> Self {
        Collector {
            index,
            reachable: BTreeSet::new(),
            referenced_types: BTreeSet::new(),
            worklist: Vec::new(),
            scanned_symbol_roots: BTreeSet::new(),
        }
    }

    /// Zig `markSyntacticReachableFunc` minus module qualification
    /// and fact merging.
    fn mark_reachable_func(&mut self, name: &str) {
        if !self.index.names.contains(name) {
            if let Some(marker) = name.find("__ov_") {
                if marker > 0 && self.index.names.contains(&name[..marker]) {
                    let plain = name[..marker].to_string();
                    self.mark_reachable_func(&plain);
                }
            }
            return;
        }
        if self.reachable.insert(name.to_string()) {
            self.worklist.push(name.to_string());
        }
    }

    /// Zig `markSyntacticAssociatedCallCandidates` minus facts:
    /// typed marking first; on no typed hit (or no type name) the
    /// plain name and every candidate are marked.
    fn mark_associated_candidates(&mut self, method_name: &str, target_type_name: Option<&str>) {
        let Some(candidates) = self.index.associated_candidates.get(method_name) else {
            self.mark_reachable_func(method_name);
            return;
        };
        let candidates = candidates.clone();
        if let Some(type_name) = target_type_name {
            let mut marked_typed = false;
            for candidate in &candidates {
                if associated_candidate_matches_receiver_type(candidate, type_name, method_name) {
                    marked_typed = true;
                    self.mark_reachable_func(candidate);
                }
            }
            if marked_typed {
                return;
            }
        }
        self.mark_reachable_func(method_name);
        for candidate in &candidates {
            self.mark_reachable_func(candidate);
        }
    }

    /// Zig `syntacticReceiverExprTypeName` with no facts: only a
    /// struct literal (through borrow/move) names its type.
    fn receiver_expr_type_name(&self, expr: &Node) -> Option<String> {
        match expr {
            Node::StructLiteral(lit) => concrete_type_name(&lit.ty).map(str::to_string),
            Node::BorrowExpr(borrow) => self.receiver_expr_type_name(&borrow.expr),
            Node::MoveExpr(mv) => self.receiver_expr_type_name(&mv.expr),
            _ => None,
        }
    }

    /// Zig `collectSyntacticReachableExpr` minus the fact engine:
    /// both if-branches are always walked (no known-branch eval).
    fn collect_expr(&mut self, expr: &Node, caller: Option<&str>) {
        match expr {
            Node::Identifier(name) => {
                self.mark_reachable_func(name);
                self.referenced_types.insert(name.clone());
            }
            Node::GenericFuncRef(gref) => {
                self.mark_reachable_func(&gref.func_name);
                self.referenced_types.insert(gref.func_name.clone());
                for ty in &gref.generics {
                    record_referenced_type(&mut self.referenced_types, ty);
                }
            }
            Node::CallExpr(call) => {
                if let Some(callee) = &call.callee {
                    self.collect_expr(callee, caller);
                }
                if let Some(target) = &call.associated_target {
                    // Zig resolves the target through local facts
                    // first; with no facts the target name itself is
                    // the type name.
                    let target = target.clone();
                    self.mark_associated_candidates(&call.func_name, Some(&target));
                } else {
                    self.mark_reachable_func(&call.func_name);
                    let receiver = if call.args.is_empty() {
                        None
                    } else {
                        self.receiver_expr_type_name(&call.args[0])
                    };
                    self.mark_associated_candidates(&call.func_name, receiver.as_deref());
                    if self.index.macro_decls.contains_key(&call.func_name)
                        || self.index.func_decl_for_call(&call.func_name).is_none()
                    {
                        self.referenced_types.insert(call.func_name.clone());
                    }
                }
                for ty in &call.generics {
                    record_referenced_type(&mut self.referenced_types, ty);
                }
                for arg in &call.args {
                    self.collect_expr(arg, caller);
                }
            }
            Node::IfExpr(ife) => {
                self.collect_expr(&ife.cond, caller);
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        self.collect_expr(&cond.value, caller);
                    }
                }
                self.collect_block(&ife.then_block, caller);
                if let Some(else_block) = &ife.else_block {
                    self.collect_block(else_block, caller);
                }
            }
            Node::SwitchExpr(swe) => {
                self.collect_expr(&swe.val, caller);
                for case in &swe.cases {
                    self.collect_expr(&case.pattern, caller);
                    self.collect_block(&case.body, caller);
                }
            }
            Node::MatchExpr(mat) => {
                self.collect_expr(&mat.val, caller);
                for case in &mat.cases {
                    if let Some(guard) = &case.guard {
                        self.collect_expr(guard, caller);
                    }
                    self.collect_block(&case.body, caller);
                }
            }
            Node::UnsafeExpr(unsafe_expr) => self.collect_block(&unsafe_expr.body, caller),
            Node::AwaitExpr(await_expr) => self.collect_expr(&await_expr.expr, caller),
            Node::TryExpr(try_expr) => self.collect_expr(&try_expr.expr, caller),
            Node::BinaryExpr(bin) => {
                self.collect_expr(&bin.left, caller);
                self.collect_expr(&bin.right, caller);
            }
            Node::ClosureLiteral(closure) => self.collect_expr(&closure.body, caller),
            Node::BorrowExpr(borrow) => self.collect_expr(&borrow.expr, caller),
            Node::MoveExpr(mv) => self.collect_expr(&mv.expr, caller),
            Node::DerefExpr(deref) => self.collect_expr(&deref.expr, caller),
            Node::CastExpr(cast) => {
                record_referenced_type(&mut self.referenced_types, &cast.ty);
                self.collect_expr(&cast.expr, caller);
            }
            Node::FieldExpr(field) => self.collect_expr(&field.expr, caller),
            Node::StructLiteral(lit) => {
                record_referenced_type(&mut self.referenced_types, &lit.ty);
                for field in &lit.fields {
                    self.collect_expr(&field.value, caller);
                }
                if let Some(update) = &lit.update_expr {
                    self.collect_expr(update, caller);
                }
            }
            Node::EnumLiteral(lit) => {
                self.referenced_types.insert(lit.enum_name.clone());
                for field in &lit.fields {
                    self.collect_expr(&field.value, caller);
                }
            }
            Node::TupleLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_expr(elem, caller);
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_expr(elem, caller);
                }
            }
            Node::RepeatArrayLiteral(lit) => self.collect_expr(&lit.value, caller),
            Node::IndexExpr(idx) => {
                self.collect_expr(&idx.target, caller);
                self.collect_expr(&idx.index, caller);
            }
            Node::SliceExpr(slice) => {
                self.collect_expr(&slice.target, caller);
                self.collect_expr(&slice.start, caller);
                self.collect_expr(&slice.end, caller);
            }
            _ => {}
        }
    }

    /// Zig `collectSyntacticReachableBlock` minus fact updates.
    fn collect_block(&mut self, block: &[Node], caller: Option<&str>) {
        for stmt in block {
            match stmt {
                Node::LetStmt(let_stmt) => {
                    if let Some(ty) = &let_stmt.ty {
                        record_referenced_type(&mut self.referenced_types, ty);
                    }
                    self.collect_expr(&let_stmt.value, caller);
                }
                Node::VarStmt(var_stmt) => {
                    record_referenced_type(&mut self.referenced_types, &var_stmt.ty);
                }
                Node::LetElseStmt(let_else) => {
                    self.collect_expr(&let_else.value, caller);
                    self.collect_block(&let_else.else_block, caller);
                }
                Node::LetDestructureStmt(let_dest) => {
                    self.collect_expr(&let_dest.value, caller);
                }
                Node::ConstStmt(const_stmt) => {
                    if let Some(ty) = &const_stmt.ty {
                        record_referenced_type(&mut self.referenced_types, ty);
                    }
                    self.collect_expr(&const_stmt.value, caller);
                }
                Node::AssignStmt(assign) => {
                    self.collect_expr(&assign.target, caller);
                    self.collect_expr(&assign.value, caller);
                }
                Node::BlockStmt(block_stmt) => self.collect_block(&block_stmt.body, caller),
                Node::ExprStmt(expr) => self.collect_expr(expr, caller),
                Node::ReturnStmt(ret) => {
                    if let Some(value) = &ret.value {
                        self.collect_expr(value, caller);
                    }
                }
                Node::ForStmt(for_stmt) => {
                    self.collect_expr(&for_stmt.start, caller);
                    if let Some(end) = &for_stmt.end {
                        self.collect_expr(end, caller);
                    } else {
                        let type_name = self.receiver_expr_type_name(&for_stmt.start);
                        self.mark_associated_candidates("iter_len", type_name.as_deref());
                        self.mark_associated_candidates("iter_at", type_name.as_deref());
                    }
                    self.collect_block(&for_stmt.body, caller);
                }
                Node::WhileStmt(while_stmt) => {
                    self.collect_expr(&while_stmt.cond, caller);
                    self.collect_block(&while_stmt.body, caller);
                }
                _ => self.collect_expr(stmt, caller),
            }
        }
    }

    /// Zig `scanReferencedSymbolRoots`: referenced const / macro
    /// names become roots. Returns true when new roots were scanned.
    fn scan_referenced_symbol_roots(&mut self) -> bool {
        if self.referenced_types.len() == self.scanned_symbol_roots.len() {
            return false;
        }
        let pending: Vec<String> = self
            .referenced_types
            .iter()
            .filter(|name| !self.scanned_symbol_roots.contains(*name))
            .cloned()
            .collect();
        for name in &pending {
            self.scanned_symbol_roots.insert(name.clone());
        }
        for name in &pending {
            if let Some(const_decl) = self.index.const_decls.get(name) {
                let const_decl = *const_decl;
                if let Some(ty) = &const_decl.ty {
                    record_referenced_type(&mut self.referenced_types, ty);
                }
                self.collect_expr(&const_decl.value, None);
            }
            if let Some(macro_decl) = self.index.macro_decls.get(name) {
                let macro_decl = *macro_decl;
                let caller = macro_decl.name.clone();
                self.collect_block(&macro_decl.body, Some(&caller));
            }
        }
        !pending.is_empty()
    }

    /// Zig `expandReachableOverloadMembers` minus facts: a plain
    /// name that is reachable expands to every `__ov_` member key,
    /// whose body is collected inline. Returns true on expansion.
    fn expand_reachable_overload_members(&mut self, decls: &[Node]) -> bool {
        let mut added = false;
        for decl in decls {
            let Node::FuncDecl(fd) = decl else { continue };
            let Some(marker) = fd.name.find("__ov_") else {
                continue;
            };
            if marker == 0 {
                continue;
            }
            if !self.reachable.contains(&fd.name[..marker]) {
                continue;
            }
            if !self.reachable.insert(fd.name.clone()) {
                continue;
            }
            let caller = fd.name.clone();
            self.collect_block(&fd.body, Some(&caller));
            added = true;
        }
        added
    }
}

/// Zig `associatedCandidateMatchesReceiverType`.
fn associated_candidate_matches_receiver_type(
    candidate: &str,
    type_name: &str,
    method_name: &str,
) -> bool {
    if !candidate.starts_with(type_name) {
        return false;
    }
    if candidate.len() == type_name.len() + 1 + method_name.len()
        && candidate.as_bytes()[type_name.len()] == b'_'
        && &candidate[type_name.len() + 1..] == method_name
    {
        return true;
    }
    if candidate.len() <= type_name.len() + 3 + method_name.len() {
        return false;
    }
    if candidate.as_bytes()[type_name.len()] != b'_'
        || candidate.as_bytes()[type_name.len() + 1] != b'_'
    {
        return false;
    }
    if !candidate.ends_with(method_name) {
        return false;
    }
    candidate.as_bytes()[candidate.len() - method_name.len() - 1] == b'_'
}

// -- roots + closure (the analysis half of the prune) --------------------------------

/// Run the root scan and the worklist closure of Zig's
/// `pruneUnreachableTestFunctionDeclsBeforeTypeCheck` over
/// `decls`. Returns (reachable, referenced_types), or None when
/// the prune must not run at all (no callable indexed, or no
/// @test present — the Zig early returns).
pub(crate) fn collect_test_reachability(
    decls: &[Node],
) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let mut index = CallableIndex {
        names: BTreeSet::new(),
        decls: BTreeMap::new(),
        const_decls: BTreeMap::new(),
        macro_decls: BTreeMap::new(),
        associated_candidates: BTreeMap::new(),
    };
    index.add_decls(decls);
    if index.names.is_empty() {
        return None;
    }
    let mut collector = Collector::new(&index);

    let mut saw_test = false;
    for decl in decls {
        match decl {
            Node::TestDecl(test_decl) => {
                saw_test = true;
                collector.collect_block(&test_decl.body, None);
            }
            Node::ConstStmt(const_stmt) => {
                if let Some(ty) = &const_stmt.ty {
                    record_referenced_type(&mut collector.referenced_types, ty);
                }
                collector.collect_expr(&const_stmt.value, None);
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
                    collector.collect_block(&fd.body, Some(&symbol));
                }
            }
            _ => {}
        }
    }
    if !saw_test {
        return None;
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

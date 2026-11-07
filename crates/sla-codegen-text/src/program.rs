//! Program-level emission: `generate()` (codegen.zig:7032-7278) and
//! the import/macro prelude it drives (7278-7302, 4080-4090).
//!
//! The conditional macro-family imports (cmp/option/result/mpsc/rc/
//! collections/atomics/cell/box/vec/iter/trait-object/async/ffi) are
//! driven in the Zig original by `programNeeds*Macros` scans over the
//! whole program. The slice supports none of those families, so the
//! scan here is a loud tripwire: a program that would need a family
//! import fails with `Unsupported` naming the family, instead of
//! silently emitting a header that's missing it.

use sla_ast::{Literal, Node};
use sla_lowering::int_constant_expr_value;

use crate::codegen::Codegen;
use crate::{unsupported, Result};

/// Macro families whose imports the Zig `generate` appends when the
/// program needs them, in Zig emission order.
const MACRO_FAMILIES: &[(&str, &str)] = &[
    ("cmp", "sa_std/cmp.sa"),
    ("option", "sa_std/core/option.sa"),
    ("result", "sa_std/core/result.sa"),
    ("mpsc", "sa_std/sync/mpsc.sa"),
    ("rc", "sa_std/core/rc.sa"),
    ("vec_deque", "sa_std/vec_deque.sa"),
    ("hashmap", "sa_std/hashmap.sa"),
    ("btreemap", "sa_std/btree_map.sa"),
    ("hashset", "sa_std/hashset.sa"),
    ("btreeset", "sa_std/btree_set.sa"),
    ("atomic", "sa_std/sync/atomic.sa"),
    ("cell", "sa_std/core/cell.sa"),
    ("refcell", "sa_std/core/refcell.sa"),
    ("box", "sa_std/core/box.sa"),
    ("vec", "sa_std/vec.sa"),
    ("iter", "sa_std/iter.sa"),
    ("trait_object", "sa_std/core/trait_object.sa"),
    ("async", "sa_std/core/future.sa"),
    ("ffi", "sa_std/ffi.sa"),
];

impl<'c> Codegen<'c> {
    /// codegen.zig:7032 `generate`.
    pub(crate) fn generate_program(&mut self, program: &Node) -> Result<String> {
        let decls = match program {
            Node::Program(prog) => &prog.decls,
            _ => return unsupported("generate on non-program node", "codegen.zig:7032"),
        };

        // Program-local function index for symbol/call lookups.
        for decl in decls {
            if let Node::FuncDecl(f) = decl {
                self.func_index.push((f.name.clone(), f.clone()));
            }
        }

        // 1. Imports in declaration order (`using` emits nothing).
        for decl in decls {
            match decl {
                Node::ImportDecl(import) => self.gen_import_decl(&import.path),
                Node::UsingDecl(_) => {}
                _ => {}
            }
        }

        // The five hard-coded imports, in this exact order, duplicates
        // kept (codegen.zig:7052-7056).
        self.wl("@import \"sa_std/string.sa\"");
        self.wl("@import \"sa_std/string_format.sa\"");
        self.wl("@import \"sa_std/io/print.sai\"");
        self.wl("@import \"sa_std/fmt.sai\"");
        self.wl("@import \"sa_std/core/panic.sa\"");

        self.emit_array_fill_macros();

        // Conditional family imports / macro bodies: loud tripwire.
        self.check_macro_families(program)?;

        // 3a-3c. Register top-level consts; fold scalar aliases and
        // scalar binary consts into the scalar-const table.
        for decl in decls {
            if let Node::ConstStmt(c) = decl {
                self.global_const_bindings.insert(c.name.clone());
            }
        }
        for decl in decls {
            if let Node::ConstStmt(c) = decl {
                if let Node::Literal(lit) = &*c.value {
                    match lit {
                        Literal::IntVal(_) | Literal::FloatVal(_) | Literal::BoolVal(_) => {
                            self.global_scalar_consts
                                .push((c.name.clone(), lit.clone()));
                        }
                        Literal::StringVal(_) => {}
                    }
                }
            }
        }
        self.fold_scalar_consts(decls);

        // 3d. Emit top-level const declarations. Scalar forms emit
        // nothing (they resolve at use sites); any other top-level
        // const shape is outside the slice.
        for decl in decls {
            if let Node::ConstStmt(c) = decl {
                if self.global_scalar_const(&c.name).is_none() {
                    return unsupported(
                        "top-level non-scalar const declaration",
                        "codegen.zig:emitTopLevelConstDecl",
                    );
                }
            }
        }

        // Trait impl vtables: outside the slice.
        for decl in decls {
            if let Node::ImplDecl(impl_decl) = decl {
                if impl_decl.trait_name.is_some() {
                    return unsupported("trait impl vtable declaration", "codegen.zig:7228");
                }
            }
        }

        // Function-pointer vtables for every non-decl-only function,
        // all before any function body (codegen.zig:7234-7245).
        for decl in decls {
            if let Node::FuncDecl(f) = decl {
                if f.is_decl_only {
                    return unsupported(
                        "extern (decl-only) function declaration",
                        "codegen.zig:emitExternDecl",
                    );
                }
                self.emit_function_pointer_vtable_decl(&f.name);
            }
        }

        // 4. Functions in declaration order (impl/overload methods
        // under their mangled names).
        for decl in decls {
            match decl {
                Node::FuncDecl(f) => {
                    if !f.is_decl_only {
                        self.gen_func_decl(f)?;
                    }
                }
                Node::ImplDecl(impl_decl) => {
                    let impl_name = match &*impl_decl.target_ty {
                        sla_ast::Type::UserDefined(ud) => ud.name.clone(),
                        _ => {
                            return unsupported(
                                "impl on non-user-defined type",
                                "codegen.zig:7255",
                            )
                        }
                    };
                    for method in &impl_decl.methods {
                        let method = match method {
                            Node::FuncDecl(m) => m,
                            _ => return unsupported("impl method is not a func decl", "codegen.zig:7260"),
                        };
                        if method.is_decl_only {
                            continue;
                        }
                        let mangled = match &impl_decl.trait_name {
                            Some(trait_name) => sla_lowering::mangle_trait_method_name(
                                &impl_name,
                                trait_name,
                                &method.name,
                            ),
                            None => sla_lowering::mangle_method_name(&impl_name, &method.name),
                        };
                        self.gen_func_decl_named(&mangled, method)?;
                    }
                }
                Node::OverloadDecl(overload) => {
                    let overload_name = match &*overload.target_ty {
                        sla_ast::Type::UserDefined(ud) => ud.name.clone(),
                        _ => {
                            return unsupported(
                                "overload on non-user-defined type",
                                "codegen.zig:7270",
                            )
                        }
                    };
                    for method in &overload.methods {
                        let method = match method {
                            Node::FuncDecl(m) => m,
                            _ => return unsupported("overload method is not a func decl", "codegen.zig:7272"),
                        };
                        if method.is_decl_only {
                            continue;
                        }
                        let mangled =
                            sla_lowering::mangle_method_name(&overload_name, &method.name);
                        self.gen_func_decl_named(&mangled, method)?;
                    }
                }
                _ => {}
            }
        }

        // 5. Test declarations in declaration order.
        for decl in decls {
            if let Node::TestDecl(t) = decl {
                self.gen_test_decl(t)?;
            }
        }

        Ok(std::mem::take(&mut self.out))
    }

    /// Alias + binary folding passes over the scalar-const table
    /// (codegen.zig:7183-7220): repeat until no pass changes anything.
    fn fold_scalar_consts(&mut self, decls: &[Node]) {
        loop {
            let mut changed = false;
            // Binary folds may unblock aliases and vice versa; run both
            // folds per pass like the original's interleaved loops.
            for decl in decls {
                if let Node::ConstStmt(c) = decl {
                    if self.global_scalar_const(&c.name).is_some() {
                        continue;
                    }
                    if let Node::BinaryExpr(_) = &*c.value {
                        let lookup = |name: &str| -> Option<i64> {
                            match self.global_scalar_const(name) {
                                Some(Literal::IntVal(v)) => Some(v),
                                _ => None,
                            }
                        };
                        if let Some(folded) = int_constant_expr_value(&c.value, &lookup) {
                            self.global_scalar_consts
                                .push((c.name.clone(), Literal::IntVal(folded)));
                            changed = true;
                        }
                    }
                }
            }
            for decl in decls {
                if let Node::ConstStmt(c) = decl {
                    if self.global_scalar_const(&c.name).is_some() {
                        continue;
                    }
                    if let Node::Identifier(target) = &*c.value {
                        if let Some(lit) = self.global_scalar_const(target) {
                            self.global_scalar_consts.push((c.name.clone(), lit));
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// codegen.zig:7278 `genImportDecl`: `.sla` paths are rewritten to
    /// `.sa`; absolute paths are normalized onto their `sa_std/` (or
    /// `.sa/std/`, `sci/sa_std/`) suffix; anything else prints as-is.
    fn gen_import_decl(&mut self, path: &str) {
        let mut effective = path.to_string();
        if let Some(base) = effective.strip_suffix(".sla") {
            effective = format!("{base}.sa");
        }
        if effective.starts_with('/') {
            if let Some(idx) = effective.find("sa_std/") {
                effective = effective[idx..].to_string();
            } else if let Some(idx) = effective.find(".sa/std/") {
                let suffix = &effective[idx + ".sa/std/".len()..];
                self.wl(&format!("@import \"sa_std/{suffix}\""));
                return;
            } else if let Some(idx) = effective.find("sci/sa_std/") {
                let suffix = &effective[idx + "sci/sa_std/".len()..];
                self.wl(&format!("@import \"sa_std/{suffix}\""));
                return;
            }
        }
        self.wl(&format!("@import \"{effective}\""));
    }

    /// codegen.zig:4080 `emitArrayFillMacros` (unconditional).
    fn emit_array_fill_macros(&mut self) {
        self.w("\n[MACRO] SLA_ARRAY_FILL_U8 %array_ptr, %value, %length\n    call @sa_mem_set(&%array_ptr, %value, %length)\n[END_MACRO]\n");
    }

    /// Tripwire replacing the `programNeeds*Macros` scans: walk every
    /// node once; a construct that would pull in a macro family fails
    /// loudly. Slice programs contain none of these shapes.
    fn check_macro_families(&self, program: &Node) -> Result<()> {
        fn walk(node: &Node, hit: &mut Option<&'static str>) {
            if hit.is_some() {
                return;
            }
            match node {
                Node::Program(p) => {
                    for d in &p.decls {
                        walk(d, hit);
                    }
                }
                Node::FuncDecl(f) => {
                    if f.is_async {
                        *hit = Some("async");
                    }
                    for s in &f.body {
                        walk(s, hit);
                    }
                }
                Node::TestDecl(t) => {
                    for s in &t.body {
                        walk(s, hit);
                    }
                }
                Node::CallExpr(call) => {
                    let name = call.func_name.as_str();
                    let family = if call.associated_target.is_some() {
                        match call.associated_target.as_deref() {
                            Some("Cell") => Some("cell"),
                            Some("AtomicI32") | Some("AtomicUsize") | Some("AtomicPtr") => {
                                Some("atomic")
                            }
                            Some("Box") => Some("box"),
                            Some("Rc") => Some("rc"),
                            Some("thread") => Some("async"),
                            _ => None,
                        }
                    } else {
                        match name {
                            "Some" | "None" => Some("option"),
                            "Ok" | "Err" => Some("result"),
                            "vec" => Some("vec"),
                            _ => None,
                        }
                    };
                    if let Some(fam) = family {
                        *hit = Some(fam);
                    }
                    for a in &call.args {
                        walk(a, hit);
                    }
                }
                Node::Identifier(name) if name == "None" => {
                    *hit = Some("option");
                }
                Node::LetStmt(l) => walk(&l.value, hit),
                Node::LetElseStmt(l) => {
                    walk(&l.value, hit);
                }
                Node::ConstStmt(c) => walk(&c.value, hit),
                Node::AssignStmt(a) => {
                    walk(&a.target, hit);
                    walk(&a.value, hit);
                }
                Node::ExprStmt(e) => walk(e, hit),
                Node::ReturnStmt(r) => {
                    if let Some(v) = &r.value {
                        walk(v, hit);
                    }
                }
                Node::BlockStmt(b) => {
                    for s in &b.body {
                        walk(s, hit);
                    }
                }
                Node::WhileStmt(w) => {
                    walk(&w.cond, hit);
                    for s in &w.body {
                        walk(s, hit);
                    }
                }
                Node::ForStmt(f) => {
                    walk(&f.start, hit);
                    if let Some(e) = &f.end {
                        walk(e, hit);
                    }
                    for s in &f.body {
                        walk(s, hit);
                    }
                }
                Node::BinaryExpr(b) => {
                    walk(&b.left, hit);
                    walk(&b.right, hit);
                }
                Node::IfExpr(i) => {
                    walk(&i.cond, hit);
                    for s in &i.then_block {
                        walk(s, hit);
                    }
                    if let Some(e) = &i.else_block {
                        for s in e {
                            walk(s, hit);
                        }
                    }
                }
                Node::BorrowExpr(b) => walk(&b.expr, hit),
                Node::MoveExpr(m) => walk(&m.expr, hit),
                Node::DerefExpr(d) => walk(&d.expr, hit),
                Node::CastExpr(c) => walk(&c.expr, hit),
                Node::FieldExpr(f) => walk(&f.expr, hit),
                Node::IndexExpr(i) => {
                    walk(&i.target, hit);
                    walk(&i.index, hit);
                }
                _ => {}
            }
        }
        let mut hit: Option<&'static str> = None;
        walk(program, &mut hit);
        if let Some(family) = hit {
            let import = MACRO_FAMILIES
                .iter()
                .find(|(name, _)| *name == family)
                .map(|(_, imp)| *imp)
                .unwrap_or("");
            return unsupported(
                match family {
                    "option" => "program needs option macros",
                    "result" => "program needs result macros",
                    "vec" => "program needs vec macros",
                    "cell" => "program needs cell macros",
                    "atomic" => "program needs atomic macros",
                    "box" => "program needs box macros",
                    "rc" => "program needs rc macros",
                    "async" => "program needs async/thread macros",
                    _ => "program needs macro family imports",
                },
                if import.is_empty() {
                    "codegen.zig:7060-7130 (programNeeds*Macros)"
                } else {
                    "codegen.zig:7060-7130 (programNeeds*Macros)"
                },
            );
        }
        Ok(())
    }
}

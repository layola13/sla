//! Program-level emission: `generate()` (codegen.zig:7032-7278) and
//! the import/macro prelude it drives (7278-7302, 4080-4090).
//!
//! The conditional macro-family imports (cmp/option/result/mpsc/rc/
//! collections/atomics/cell/box/vec/iter) are driven in the Zig
//! original by `programNeeds*Macros` scans over the whole program;
//! they are ported here as a collecting scan (checker-recorded
//! expression types + type annotations + call triggers) that emits
//! the family's `@import` in Zig order, plus the inline HashMap /
//! BTreeMap helper macros. The async / trait-object / ffi families
//! stay a loud tripwire: their emission involves helper-function
//! bodies outside this lane (registered in
//! docs/divergences/lowering-text.md).

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
    ("mem", "sa_std/mem.sa"),
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
    ("growvec", "sa_std/growvec.sa"),
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
        self.emit_macro_family_imports(program)?;

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

    /// Port of the `programNeeds*Macros` scans (codegen.zig:
    /// 6300-8200) + the conditional import block (8884-8970) and
    /// `emitHashMapMacros` / `emitBTreeMapMacros` (5633-5780).
    /// One collecting walk over the whole program: a family is
    /// needed when a checker-recorded expression type, a type
    /// annotation, or a call trigger names it (the Zig predicates
    /// key on the same three signals, per family). Imports are
    /// emitted in MACRO_FAMILIES order; duplicate lines are
    /// harmless (the flattener deduplicates), matching the Zig
    /// note on the thread import. The async family (async fns,
    /// `thread::spawn`, future/task types) stays loud: its
    /// emission also writes helper bodies this lane has not
    /// ported.
    fn emit_macro_family_imports(&mut self, program: &Node) -> Result<()> {
        fn type_families(ty: &sla_ast::Type, out: &mut std::collections::HashSet<&'static str>) {
            match ty {
                sla_ast::Type::UserDefined(u) => {
                    let fam = match u.name.as_str() {
                        "Vec" => Some("vec"),
                        "GrowVec" => Some("growvec"),
                        "VecDeque" => Some("vec_deque"),
                        "HashMap" => Some("hashmap"),
                        "BTreeMap" => Some("btreemap"),
                        "HashSet" => Some("hashset"),
                        "BTreeSet" => Some("btreeset"),
                        "Cell" => Some("cell"),
                        "RefCell" => Some("refcell"),
                        "Box" => Some("box"),
                        "Rc" | "Arc" => Some("rc"),
                        "AtomicI32" | "AtomicUsize" | "AtomicPtr" => Some("atomic"),
                        "Option" => Some("option"),
                        "Result" => Some("result"),
                        "Sender" | "Receiver" => Some("mpsc"),
                        "ManuallyDrop" => Some("mem"),
                        "Ordering" => Some("cmp"),
                        _ => None,
                    };
                    if let Some(f) = fam {
                        out.insert(f);
                    }
                    for g in &u.generics {
                        type_families(g, out);
                    }
                }
                sla_ast::Type::Borrow(inner) | sla_ast::Type::Pointer(inner) => {
                    type_families(inner, out)
                }
                sla_ast::Type::Array(arr) => type_families(&arr.elem, out),
                sla_ast::Type::Tuple(tup) => {
                    for e in &tup.elems {
                        type_families(e, out);
                    }
                }
                sla_ast::Type::Future(inner) => type_families(inner, out),
                sla_ast::Type::Closure(sig) => {
                    for p in &sig.params {
                        type_families(p, out);
                    }
                    type_families(&sig.ret, out);
                }
                sla_ast::Type::FnPtr(sig) => {
                    for p in &sig.params {
                        type_families(p, out);
                    }
                    type_families(&sig.ret, out);
                }
                _ => {}
            }
        }
        fn walk(
            check: &dyn sla_lowering::CheckInfo,
            node: &Node,
            out: &mut std::collections::HashSet<&'static str>,
        ) {
            if let Some(ty) = check.expr_type(node) {
                type_families(&ty, out);
            }
            match node {
                Node::Program(p) => {
                    for d in &p.decls {
                        walk(check, d, out);
                    }
                }
                Node::FuncDecl(f) => {
                    if f.is_async {
                        out.insert("async");
                    }
                    for param in &f.params {
                        type_families(&param.ty, out);
                    }
                    type_families(&f.ret_ty, out);
                    for s in &f.body {
                        walk(check, s, out);
                    }
                }
                Node::TestDecl(t) => {
                    for s in &t.body {
                        walk(check, s, out);
                    }
                }
                Node::StructDecl(sd) => {
                    for field in &sd.fields {
                        type_families(&field.ty, out);
                    }
                }
                Node::CallExpr(call) => {
                    if let Some(target) = &call.associated_target {
                        let leaf = target.rsplit("::").next().unwrap_or(target);
                        match leaf {
                            "Cell" => {
                                out.insert("cell");
                            }
                            "AtomicI32" | "AtomicUsize" | "AtomicPtr" => {
                                out.insert("atomic");
                            }
                            "Box" => {
                                out.insert("box");
                            }
                            "Rc" | "Arc" => {
                                out.insert("rc");
                            }
                            "RefCell" => {
                                out.insert("refcell");
                            }
                            "Vec" => {
                                out.insert("vec");
                            }
                            "GrowVec" => {
                                out.insert("growvec");
                            }
                            "VecDeque" => {
                                out.insert("vec_deque");
                            }
                            "HashMap" => {
                                out.insert("hashmap");
                            }
                            "BTreeMap" => {
                                out.insert("btreemap");
                            }
                            "HashSet" => {
                                out.insert("hashset");
                            }
                            "BTreeSet" => {
                                out.insert("btreeset");
                            }
                            "thread" => {
                                out.insert("async");
                            }
                            "mpsc" => {
                                out.insert("mpsc");
                            }
                            "cmp" => {
                                out.insert("cmp");
                            }
                            _ => {}
                        }
                    }
                    match call.func_name.as_str() {
                        "Some" | "None" => {
                            out.insert("option");
                        }
                        "Ok" | "Err" => {
                            out.insert("result");
                        }
                        "vec" => {
                            out.insert("vec");
                        }
                        "iter" | "into_iter" => {
                            out.insert("iter");
                        }
                        _ => {}
                    }
                    if let Some(callee) = &call.callee {
                        walk(check, callee, out);
                    }
                    for a in &call.args {
                        walk(check, a, out);
                    }
                }
                Node::Identifier(name) if name == "None" => {
                    out.insert("option");
                }
                Node::LetStmt(l) => {
                    if let Some(ty) = &l.ty {
                        type_families(ty, out);
                    }
                    walk(check, &l.value, out);
                }
                Node::LetElseStmt(l) => walk(check, &l.value, out),
                Node::VarStmt(v) => type_families(&v.ty, out),
                Node::ConstStmt(c) => walk(check, &c.value, out),
                Node::AssignStmt(a) => {
                    walk(check, &a.target, out);
                    walk(check, &a.value, out);
                }
                Node::ExprStmt(e) => walk(check, e, out),
                Node::ReturnStmt(r) => {
                    if let Some(v) = &r.value {
                        walk(check, v, out);
                    }
                }
                Node::BlockStmt(b) => {
                    for s in &b.body {
                        walk(check, s, out);
                    }
                }
                Node::ArenaStmt(b) => {
                    for s in &b.body {
                        walk(check, s, out);
                    }
                }
                Node::WhileStmt(w) => {
                    walk(check, &w.cond, out);
                    for s in &w.body {
                        walk(check, s, out);
                    }
                }
                Node::ForStmt(f) => {
                    walk(check, &f.start, out);
                    if let Some(e) = &f.end {
                        walk(check, e, out);
                    }
                    for s in &f.body {
                        walk(check, s, out);
                    }
                }
                Node::BinaryExpr(b) => {
                    walk(check, &b.left, out);
                    walk(check, &b.right, out);
                }
                Node::IfExpr(i) => {
                    walk(check, &i.cond, out);
                    for s in &i.then_block {
                        walk(check, s, out);
                    }
                    if let Some(e) = &i.else_block {
                        for s in e {
                            walk(check, s, out);
                        }
                    }
                }
                Node::BorrowExpr(b) => walk(check, &b.expr, out),
                Node::MoveExpr(m) => walk(check, &m.expr, out),
                Node::DerefExpr(d) => walk(check, &d.expr, out),
                Node::CastExpr(c) => {
                    type_families(&c.ty, out);
                    walk(check, &c.expr, out);
                }
                Node::FieldExpr(f) => walk(check, &f.expr, out),
                Node::IndexExpr(i) => {
                    walk(check, &i.target, out);
                    walk(check, &i.index, out);
                }
                Node::SliceExpr(sl) => {
                    walk(check, &sl.target, out);
                    walk(check, &sl.start, out);
                    walk(check, &sl.end, out);
                }
                Node::ClosureLiteral(c) => walk(check, &c.body, out),
                Node::AwaitExpr(a) => walk(check, &a.expr, out),
                Node::TryExpr(t) => walk(check, &t.expr, out),
                Node::StructLiteral(l) => {
                    for f in &l.fields {
                        walk(check, &f.value, out);
                    }
                }
                Node::EnumLiteral(l) => {
                    for f in &l.fields {
                        walk(check, &f.value, out);
                    }
                }
                Node::TupleLiteral(l) => {
                    for e in &l.elements {
                        walk(check, e, out);
                    }
                }
                Node::ArrayLiteral(l) => {
                    for e in &l.elements {
                        walk(check, e, out);
                    }
                }
                Node::SwitchExpr(sw) => {
                    walk(check, &sw.val, out);
                    for case in &sw.cases {
                        walk(check, &case.pattern, out);
                        for s in &case.body {
                            walk(check, s, out);
                        }
                    }
                }
                Node::MatchExpr(m) => {
                    walk(check, &m.val, out);
                    for case in &m.cases {
                        if let Some(g) = &case.guard {
                            walk(check, g, out);
                        }
                        for s in &case.body {
                            walk(check, s, out);
                        }
                    }
                }
                _ => {}
            }
        }
        let mut families: std::collections::HashSet<&'static str> = Default::default();
        walk(self.check, program, &mut families);
        if families.contains("async") {
            return unsupported(
                "program needs async/thread macros",
                "codegen.zig:7060-7130 (programNeeds*Macros)",
            );
        }
        for (name, import) in MACRO_FAMILIES {
            if families.contains(name) {
                self.wl(&format!("@import \"{import}\""));
            }
        }
        if families.contains("hashmap") {
            self.w(crate::family_macros::HASHMAP_HELPER_MACROS);
        }
        if families.contains("btreemap") {
            self.w(crate::family_macros::BTREEMAP_HELPER_MACROS);
        }
        Ok(())
    }
}

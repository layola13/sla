//! A1 minimal-import scan (LINE-SMALLTIER-FIX, R301-agreed form).
//!
//! `generate` used to emit five std imports unconditionally
//! (program.rs, codegen.zig:7052-7056); the SA host then paid the
//! flatten cost of expanding their shared closure for every
//! program — about 187 ms on the Colab 8-core profile, about
//! 1.5 s on this InstaBox venue — including programs that use
//! none of the five modules. This scan walks the (import-expanded)
//! program once and reports which of the five the emitted SA text
//! actually needs.
//!
//! The scan is deliberately site-specific, never type-presence:
//! a `String` parameter that is only moved or dropped emits no
//! `string.sa` reference at all (probe p02), while a `format()`
//! result is a `Vec`-backed handle, not a `StringBuf` (probe p04),
//! so "some type mentions String" would keep the import for
//! programs that do not need it. Every flag below is tied to an
//! emission site that was read out of the emitter sources and
//! confirmed by a probe build (`work/probe*.sh`, baseline `sla`):
//!
//! - `string` (`sa_std/string.sa`):
//!   `str_eq` (STR_EQ, call.rs), `format`/`str_eq` with a
//!   `String`-typed argument (STRING_BUF_AS_STR, format.rs /
//!   call.rs), `String`-typed `let`/`const` of a literal and
//!   assignment of a literal to a `String` binding
//!   (STRING_BUF_FROM_UTF8_UNCHECKED, stmt.rs / call.rs:166),
//!   `var x: String` fresh-empty (same macro, stmt.rs:676),
//!   a literal passed to a `String`-typed parameter (same macro,
//!   call.rs:166, resolved through the callee signature),
//!   `s.len()` / `len(s)` on a `String` value (STRING_BUF_LEN,
//!   surface.rs:367), `"lit".len` (STRING_LEN, into.rs:476),
//!   any other method call on a `String` receiver that resolves
//!   to no known callee (the `EXPAND <method>` catch-all,
//!   call.rs — the Zig surface lowering for such methods lives
//!   in string.sa),
//!   a field/index projection of a `String`-containing type
//!   passed by value or stored into a struct literal field
//!   (STRING_BUF_CLONE, call.rs shape C / into.rs shape D /
//!   extern_macro.rs writebacks), an imported-macro call whose
//!   macro lives in `string.sa` (the `EXPAND` itself names it),
//!   any `String::…` associated call (conservative).
//! - `string_format` (`sa_std/string_format.sa`):
//!   `format()` (FORMAT_BEGIN / FORMAT_PUSH_*, format.rs).
//!   Also the slice provider of last resort: every string
//!   literal evaluated as a value emits SLICE_NEW (codegen.rs
//!   `gen_literal_value`), `s.len()` / `len(s)` on a slice emits
//!   SLICE_GET_LEN (surface.rs:367), and `__slice_view` and slice
//!   destructure rest (`let [a, ..rest] = …`) emit SLICE_NEW;
//!   `SLICE_NEW` lives in `core/slice.sa`, which only
//!   `string.sa` and `string_format.sa` import among the five,
//!   so any such site keeps `string_format` (the cheaper of the
//!   two closures — the strip probe: dropping `string_format`
//!   alone saves ~25 ms, dropping `string` alone ~430 ms).
//! - `print` (`sa_std/io/print.sai`): any `println()` — every
//!   arm calls `@sa_print_bytes` (println.rs).
//! - `fmt` (`sa_std/fmt.sai`): a `println` `{}` hole filled with
//!   a primitive-typed (or unknown-typed) argument
//!   (`@sa_fmt_u64` / `@sa_fmt_f64` / `@sa_fmt_bool` +
//!   `@sa_fmt_buffer_*`, println.rs), or any `format()` use
//!   (`string_format.sa` itself imports `fmt.sai`; flagged
//!   explicitly so the need does not rest on transitivity).
//! - `panic` (`sa_std/core/panic.sa`): `panic` / `panic_msg`
//!   with a string-literal first argument (EXPAND PANIC_MSG,
//!   call.rs `gen_panic_call`). An int/bool/other first argument
//!   emits the bare `panic(…)` instruction and needs no import
//!   (probe p08). `todo!()` / `unreachable!()` are not builtins
//!   in this lane (no dispatch in checker or codegen).
//!
//! Anything the scan cannot prove unnecessary stays imported: a
//! missed need breaks the build at flatten time, while a kept
//! import only costs time. In particular the scan is conservative
//! wherever the checker facts are thin: an unknown-typed `println`
//! hole argument keeps `fmt`; an unresolvable callee with a
//! string-literal argument keeps `string`; an imported macro with
//! no recorded home file keeps all five.

use std::collections::{HashMap, HashSet};

use sla_ast::{
    CallExpr, FuncDecl, Literal, Node, Type, UserDefined,
};
use sla_lowering::{mangle_method_name, peel_borrow_pointer_type, CheckInfo};

/// The five conditional imports, in emission order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MinImports {
    pub string: bool,
    pub string_format: bool,
    pub print: bool,
    pub fmt: bool,
    pub panic: bool,
}

/// Scan the (import-expanded) program for the five modules the
/// emitted SA text needs. Pure: reads the tree and the checker
/// view, writes nothing.
pub fn scan_min_imports(program: &Node, check: &dyn CheckInfo) -> MinImports {
    let mut scanner = Scanner::new(program, check);
    scanner.walk(program);
    scanner.flags
}

/// A type mentions `String` by name anywhere in its tree
/// (borrow/pointer peeled, generics included). Syntactic only:
/// user structs are NOT resolved here — callers that need the
/// resolved closure use [`Scanner::ty_mentions_string`] instead.
fn ty_has_string(ty: &Type) -> bool {
    match ty {
        Type::UserDefined(u) => {
            u.name == "String" || u.generics.iter().any(ty_has_string)
        }
        Type::Borrow(inner) | Type::Pointer(inner) => ty_has_string(inner.as_ref()),
        Type::Array(arr) => ty_has_string(arr.elem.as_ref()),
        Type::Tuple(tup) => tup.elems.iter().any(ty_has_string),
        Type::Future(inner) => ty_has_string(inner.as_ref()),
        Type::Closure(sig) => {
            sig.params.iter().any(ty_has_string) || ty_has_string(sig.ret.as_ref())
        }
        Type::FnPtr(sig) => {
            sig.params.iter().any(ty_has_string) || ty_has_string(sig.ret.as_ref())
        }
        _ => false,
    }
}

/// Substitute a struct declaration's generic parameters with the
/// concrete arguments of a use site (`Pair<String>` => field `T`
/// reads as `String`). Mirrors `drop_plan_substitute` (drop.rs).
fn substitute_ty(ty: &Type, params: &[String], args: &[Type]) -> Type {
    match ty {
        Type::UserDefined(u) => {
            if u.generics.is_empty() {
                if let Some(idx) = params.iter().position(|p| p == &u.name) {
                    if let Some(arg) = args.get(idx) {
                        return arg.clone();
                    }
                }
                return ty.clone();
            }
            let generics = u
                .generics
                .iter()
                .map(|g| substitute_ty(g, params, args))
                .collect();
            Type::UserDefined(UserDefined {
                name: u.name.clone(),
                generics,
            })
        }
        Type::Borrow(inner) => {
            Type::Borrow(Box::new(substitute_ty(inner.as_ref(), params, args)))
        }
        Type::Pointer(inner) => {
            Type::Pointer(Box::new(substitute_ty(inner.as_ref(), params, args)))
        }
        Type::Array(arr) => Type::Array(sla_ast::ArrayType {
            elem: Box::new(substitute_ty(arr.elem.as_ref(), params, args)),
            len: arr.len,
        }),
        Type::Tuple(tup) => Type::Tuple(sla_ast::TupleType {
            elems: tup
                .elems
                .iter()
                .map(|e| substitute_ty(e, params, args))
                .collect(),
        }),
        Type::Future(inner) => {
            Type::Future(Box::new(substitute_ty(inner.as_ref(), params, args)))
        }
        Type::Closure(sig) => Type::Closure(sla_ast::ClosureType {
            params: sig
                .params
                .iter()
                .map(|p| substitute_ty(p, params, args))
                .collect(),
            ret: Box::new(substitute_ty(sig.ret.as_ref(), params, args)),
        }),
        Type::FnPtr(sig) => Type::FnPtr(sla_ast::FnPtrType {
            abi: sig.abi.clone(),
            params: sig
                .params
                .iter()
                .map(|p| substitute_ty(p, params, args))
                .collect(),
            ret: Box::new(substitute_ty(sig.ret.as_ref(), params, args)),
        }),
        _ => ty.clone(),
    }
}

struct Scanner<'a> {
    check: &'a dyn CheckInfo,
    flags: MinImports,
    /// Function declarations by name, collected from the program
    /// tree (plain names plus `Target_method` mangled keys for
    /// impl/overload methods): the fallback for the
    /// callee-signature literal rule when the checker table does
    /// not carry the callee (tail matches, `using` extensions).
    fn_index: HashMap<String, FuncDecl>,
}

impl<'a> Scanner<'a> {
    fn new(program: &Node, check: &'a dyn CheckInfo) -> Self {
        let mut scanner = Scanner {
            check,
            flags: MinImports::default(),
            fn_index: HashMap::new(),
        };
        scanner.collect_fn_index(program);
        scanner
    }

    fn collect_fn_index(&mut self, program: &Node) {
        let Node::Program(prog) = program else {
            return;
        };
        for decl in &prog.decls {
            if let Node::FuncDecl(f) = decl {
                self.fn_index.insert(f.name.clone(), f.clone());
            }
        }
        // Impl/overload methods need the target type name, handled
        // in a second pass to keep the match arms distinct.
        for decl in &prog.decls {
            let (target_ty, methods) = match decl {
                Node::ImplDecl(d) => (&d.target_ty, &d.methods),
                Node::OverloadDecl(d) => (&d.target_ty, &d.methods),
                _ => continue,
            };
            let target_name = match &**target_ty {
                Type::UserDefined(u) => u.name.clone(),
                _ => continue,
            };
            for method in methods {
                if let Node::FuncDecl(m) = method {
                    let key = mangle_method_name(&target_name, &m.name);
                    self.fn_index.insert(key, m.clone());
                }
            }
        }
    }

    /// Resolved `String` mention: the syntactic check, plus user
    /// structs resolved through the checker (a `Foo` whose field
    /// is `String` counts — the clone sites resolve fields the
    /// same way). Depth-capped; unknown structs do not count,
    /// matching `deep_clone_needed` (clone.rs), which returns
    /// false when the declaration is unavailable.
    fn ty_mentions_string(&self, ty: &Type, depth: usize) -> bool {
        if ty_has_string(ty) {
            return true;
        }
        if depth > 8 {
            return false;
        }
        match peel_borrow_pointer_type(ty) {
            Type::UserDefined(u) => {
                let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
                    return false;
                };
                decl.fields.iter().any(|f| {
                    let field_ty = substitute_ty(f.ty.as_ref(), &decl.generics, &u.generics);
                    self.ty_mentions_string(&field_ty, depth + 1)
                })
            }
            Type::Array(arr) => self.ty_mentions_string(&arr.elem, depth + 1),
            Type::Tuple(tup) => tup
                .elems
                .iter()
                .any(|e| self.ty_mentions_string(e, depth + 1)),
            _ => false,
        }
    }

    /// Mirror of `deep_clone_needed` (clone.rs) restricted to the
    /// question "can cloning a value of this type emit
    /// STRING_BUF_CLONE": `String` itself, `Vec`/`GrowVec`
    /// elements, `Box` payloads, array/tuple elements, and struct
    /// fields (resolved). `Rc`/`Arc` clone the handle only, maps
    /// and sets are uncloneable, `Option`/`Result` have no clone
    /// arm — all read false here, exactly as in the emitter.
    fn clone_mentions_string(&self, ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        let peeled = peel_borrow_pointer_type(ty);
        if let Type::UserDefined(u) = peeled {
            if u.name == "String" {
                return true;
            }
        }
        if let Some((_, elem)) = crate::place::vec_like_element_type(ty) {
            return self.clone_mentions_string(&elem, depth + 1);
        }
        if let Type::UserDefined(u) = peeled {
            if u.name == "Box" {
                if let Some(inner) = u.generics.first() {
                    return self.clone_mentions_string(inner, depth + 1);
                }
                return false;
            }
            if matches!(u.name.as_str(), "Rc" | "Arc") {
                return false;
            }
            if matches!(
                u.name.as_str(),
                "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet"
            ) {
                return false;
            }
        }
        match peeled {
            Type::Array(arr) => self.clone_mentions_string(&arr.elem, depth + 1),
            Type::Tuple(tup) => tup
                .elems
                .iter()
                .any(|e| self.clone_mentions_string(e, depth + 1)),
            Type::UserDefined(u) => {
                let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
                    return false;
                };
                decl.fields.iter().any(|f| {
                    let field_ty = substitute_ty(f.ty.as_ref(), &decl.generics, &u.generics);
                    self.clone_mentions_string(&field_ty, depth + 1)
                })
            }
            _ => false,
        }
    }

    /// The declared type of a struct field, with generics
    /// substituted from the use-site type (`Pair<String>.a`).
    fn struct_field_ty(&self, ty: &Type, field_name: &str) -> Option<Type> {
        let peeled = peel_borrow_pointer_type(ty);
        let Type::UserDefined(u) = peeled else {
            return None;
        };
        let decl = crate::place::struct_decl_lookup(self.check, &u.name)?;
        decl.fields.iter().find(|f| f.name == field_name).map(|f| {
            substitute_ty(f.ty.as_ref(), &decl.generics, &u.generics)
        })
    }

    /// First identifier at the root of a field/index projection
    /// (`a.b[0].c` -> `a`; anything else -> `None`). Mirrors
    /// `root_identifier` (sla-lowering): only projections with an
    /// identifier root clone on transfer (call.rs shape C,
    /// into.rs shape D).
    fn projection_root(node: &Node) -> Option<String> {
        match node {
            Node::Identifier(name) => Some(name.clone()),
            Node::FieldExpr(f) => Self::projection_root(&f.expr),
            Node::IndexExpr(i) => Self::projection_root(&i.target),
            _ => None,
        }
    }

    /// Callee parameter `String`-ness for the literal rule, in
    /// lookup order: checker table, program-tree index, extern
    /// signature (ABI spelling substring). `None` when the callee
    /// is not resolvable anywhere.
    fn callee_string_params(&self, call: &CallExpr) -> Option<Vec<bool>> {
        let mut keys: Vec<String> = vec![call.func_name.clone()];
        if let Some(target) = &call.associated_target {
            keys.push(mangle_method_name(target, &call.func_name));
            keys.push(format!("{target}__{}", call.func_name));
            keys.push(format!("{target}::{}", call.func_name));
        }
        for key in &keys {
            if let Some(decl) = self.check.func(key) {
                return Some(
                    decl.params
                        .iter()
                        .map(|p| self.ty_mentions_string(p.ty.as_ref(), 0))
                        .collect(),
                );
            }
            if let Some(decl) = self.fn_index.get(key) {
                return Some(
                    decl.params
                        .iter()
                        .map(|p| self.ty_mentions_string(p.ty.as_ref(), 0))
                        .collect(),
                );
            }
        }
        // Receiver method: resolve through the receiver's type
        // (`v.push(…)` -> `Vec_push`), if the checker recorded it.
        if call.associated_target.is_none() && !call.args.is_empty() {
            if let Some(recv_ty) = self.check.expr_type(&call.args[0]) {
                if let Type::UserDefined(u) = peel_borrow_pointer_type(recv_ty) {
                    let key = mangle_method_name(&u.name, &call.func_name);
                    if let Some(decl) = self.check.func(&key) {
                        return Some(
                            decl.params
                                .iter()
                                .map(|p| self.ty_mentions_string(p.ty.as_ref(), 0))
                                .collect(),
                        );
                    }
                    if let Some(decl) = self.fn_index.get(&key) {
                        return Some(
                            decl.params
                                .iter()
                                .map(|p| self.ty_mentions_string(p.ty.as_ref(), 0))
                                .collect(),
                        );
                    }
                }
            }
        }
        if let Some(sig) = self.check.extern_sig(&call.func_name) {
            return Some(
                sig.params
                    .iter()
                    .map(|p| p.abi_ty.contains("String"))
                    .collect(),
            );
        }
        None
    }

    /// A string literal passed to a `String`-typed parameter
    /// materializes as an owned handle (call.rs:166). Returns
    /// true when the rule fired (so the caller can skip the
    /// blunt fallback). Generic callees count as `String`
    /// (monomorphization may substitute `String` for `T`).
    fn literal_arg_rule(&mut self, call: &CallExpr) -> bool {
        let Some(string_params) = self.callee_string_params(call) else {
            return false;
        };
        let is_generic = self
            .check
            .func(&call.func_name)
            .map(|d| !d.generics.is_empty())
            .unwrap_or(false)
            || self
                .fn_index
                .get(&call.func_name)
                .map(|d| !d.generics.is_empty())
                .unwrap_or(false);
        for (i, arg) in call.args.iter().enumerate() {
            if !matches!(arg, Node::Literal(Literal::StringVal(_))) {
                continue;
            }
            if is_generic {
                self.flags.string = true;
                return true;
            }
            if string_params.get(i).copied().unwrap_or(false) {
                self.flags.string = true;
                return true;
            }
        }
        false
    }

    /// `println` `{}` hole argument indices (1-based into
    /// `call.args`), mirroring println.rs: `{{` is an escape,
    /// a lone `{}` consumes the next argument.
    fn println_hole_args(fmt: &str, arg_count: usize) -> Vec<usize> {
        let bytes = fmt.as_bytes();
        let mut holes = Vec::new();
        let mut arg_index = 1usize;
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'}' {
                if arg_index < arg_count {
                    holes.push(arg_index);
                }
                arg_index += 1;
                i += 2;
            } else if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                i += 2;
            } else {
                i += 1;
            }
        }
        holes
    }

    /// Whether a call resolves to a known callee (free function,
    /// receiver method, extern, or imported macro) — i.e. it does
    /// NOT fall into the `EXPAND <name>` catch-all (call.rs). Used
    /// by the String-receiver rule: only an unresolvable call can
    /// reach the catch-all.
    fn is_known_callee(&self, call: &CallExpr) -> bool {
        if self.check.func(&call.func_name).is_some()
            || self.fn_index.contains_key(&call.func_name)
            || self.check.extern_sig(&call.func_name).is_some()
            || self.check.imported_macro(&call.func_name).is_some()
        {
            return true;
        }
        if let Some(target) = &call.associated_target {
            for key in [
                mangle_method_name(target, &call.func_name),
                format!("{target}__{}", call.func_name),
                format!("{target}::{}", call.func_name),
            ] {
                if self.check.func(&key).is_some() || self.fn_index.contains_key(&key) {
                    return true;
                }
            }
            return false;
        }
        if !call.args.is_empty() {
            if let Some(recv_ty) = self.check.expr_type(&call.args[0]) {
                if let Type::UserDefined(u) = peel_borrow_pointer_type(recv_ty) {
                    let key = mangle_method_name(&u.name, &call.func_name);
                    if self.check.func(&key).is_some() || self.fn_index.contains_key(&key) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Call-expression arm: builtins, imported macros, the
    /// literal rule, the projection-clone rule, then the generic
    /// argument walk (literals reached here flag
    /// `string_format` for SLICE_NEW).
    fn walk_call(&mut self, call: &CallExpr) {
        // Imported SA-text macros: the EXPAND names the macro, so
        // its home file must be imported. (Explicit and automatch
        // imports cover the common cases; this is the backstop
        // for macros the checker registered without an import.)
        if let Some(view) = self.check.imported_macro(&call.func_name) {
            match view.import_path.as_deref() {
                Some(path) if path.ends_with("string_format.sa") => {
                    self.flags.string_format = true;
                    self.flags.fmt = true;
                }
                Some(path) if path.ends_with("string.sa") => {
                    self.flags.string = true;
                }
                Some(path) if path.ends_with("print.sai") => {
                    self.flags.print = true;
                }
                Some(path) if path.ends_with("fmt.sai") => {
                    self.flags.fmt = true;
                }
                Some(path) if path.ends_with("panic.sa") => {
                    self.flags.panic = true;
                }
                Some(_) => {}
                None => {
                    // Unattributed macro: keep everything rather
                    // than guess which file defines it.
                    self.flags.string = true;
                    self.flags.string_format = true;
                    self.flags.print = true;
                    self.flags.fmt = true;
                    self.flags.panic = true;
                }
            }
            // extern_macro.rs writebacks: an assigned-output
            // argument whose value needs a deep clone clones it
            // (STRING_BUF_CLONE when the closure holds String).
            for (i, arg) in call.args.iter().enumerate() {
                if view.call_arg_is_assigned_output(i) {
                    if let Some(ty) = self.check.expr_type(arg) {
                        if self.clone_mentions_string(ty, 0) {
                            self.flags.string = true;
                        }
                    }
                }
            }
        }

        match call.func_name.as_str() {
            "format" => {
                self.flags.string_format = true;
                self.flags.fmt = true;
                // arg 0 is the format literal (handled inline by
                // the emitter); a String-typed hole argument goes
                // through STRING_BUF_AS_STR.
                for arg in call.args.iter().skip(1) {
                    if matches!(arg, Node::Literal(Literal::StringVal(_))) {
                        continue;
                    }
                    if let Some(ty) = self.check.expr_type(arg) {
                        if self.ty_mentions_string(ty, 0) {
                            self.flags.string = true;
                        }
                    }
                    self.walk(arg);
                }
                self.projection_clone_args(call);
                return;
            }
            "println" => {
                self.flags.print = true;
                let Some(Node::Literal(Literal::StringVal(fmt_text))) = call.args.first()
                else {
                    // Non-literal first argument: only a newline
                    // is emitted; the arguments are not evaluated.
                    return;
                };
                let fmt_text = fmt_text.clone();
                let holes = Self::println_hole_args(&fmt_text, call.args.len());
                let hole_set: HashSet<usize> = holes.iter().copied().collect();
                for (i, arg) in call.args.iter().enumerate().skip(1) {
                    if !hole_set.contains(&i) {
                        // Past the holes: not emitted, not walked.
                        continue;
                    }
                    if matches!(arg, Node::Literal(Literal::StringVal(_))) {
                        continue;
                    }
                    match self.check.expr_type(arg) {
                        Some(Type::Primitive(_)) => {
                            self.flags.fmt = true;
                        }
                        None => {
                            // Unknown type: stay conservative, a
                            // primitive would need the fmt fns.
                            self.flags.fmt = true;
                        }
                        Some(_) => {}
                    }
                    self.walk(arg);
                }
                return;
            }
            "panic" | "panic_msg" => {
                if let Some(Node::Literal(Literal::StringVal(_))) = call.args.first() {
                    self.flags.panic = true;
                } else if let Some(first) = call.args.first() {
                    self.walk(first);
                }
                return;
            }
            "str_eq" => {
                self.flags.string = true;
                for arg in &call.args {
                    self.walk(arg);
                }
                return;
            }
            "__slice_view" => {
                self.flags.string_format = true;
                for arg in &call.args {
                    self.walk(arg);
                }
                return;
            }
            // surface.rs:367 free `len` / receiver `.len()`: the
            // macro follows args[0]'s type — String needs
            // string.sa (STRING_BUF_LEN), Slice needs
            // string_format.sa (SLICE_GET_LEN via core/slice.sa).
            // Vec/Map/Set/GrowVec len macros live in non-five
            // files (family imports cover them).
            "len" if call.args.len() == 1 && call.callee.is_none() => {
                if let Some(ty) = self.check.expr_type(&call.args[0]) {
                    match peel_borrow_pointer_type(ty) {
                        Type::UserDefined(u) if u.name == "String" => {
                            self.flags.string = true;
                        }
                        Type::UserDefined(u) if u.name == "Slice" => {
                            self.flags.string_format = true;
                        }
                        _ => {}
                    }
                }
                if let Some(arg) = call.args.first() {
                    self.walk(arg);
                }
                self.projection_clone_args(call);
                return;
            }
            _ => {}
        }

        // A method call on a String receiver that resolves to no
        // known callee reaches the `EXPAND <method>` catch-all
        // (call.rs). The method name itself is never a std macro
        // (lowercase names are absent from the automatch index),
        // but the String surface lowering the Zig backend uses for
        // these methods (STR_AS_BYTES / STRING_BUF_AS_BYTES, see
        // test_unit_def2_string_as_bytes) lives in string.sa — so
        // the import stays whenever such a call exists. A call
        // that fails at check time never emits; keeping the import
        // is only a (safe) over-approximation. Known callees are
        // excluded: their bodies are scanned separately and the
        // call itself emits `call @…`, never an EXPAND.
        if call.callee.is_none()
            && call.associated_target.is_none()
            && !call.args.is_empty()
            && !self.is_known_callee(call)
        {
            if let Some(ty) = self.check.expr_type(&call.args[0]) {
                if let Type::UserDefined(u) = peel_borrow_pointer_type(ty) {
                    if u.name == "String" {
                        self.flags.string = true;
                    }
                }
            }
        }

        // Associated `String::…` call: conservative — the method
        // may expand to a STRING_BUF_* macro (notably `.len()`
        // resolves through the receiver path, but a static call
        // keeps the import either way).
        if let Some(target) = &call.associated_target {
            let leaf = target.rsplit("::").next().unwrap_or(target);
            if leaf == "String" {
                self.flags.string = true;
            }
        }

        // String literal into a String parameter (owned handle);
        // when the callee cannot be resolved at all, a literal
        // argument to an associated call keeps the import rather
        // than risking a missing materialization.
        if !self.literal_arg_rule(call)
            && call.associated_target.is_some()
            && call
                .args
                .iter()
                .any(|a| matches!(a, Node::Literal(Literal::StringVal(_))))
        {
            self.flags.string = true;
        }

        self.projection_clone_args(call);
        if let Some(callee) = &call.callee {
            self.walk(callee);
        }
        for arg in &call.args {
            self.walk(arg);
        }
    }

    /// call.rs shape C: a field/index projection with an
    /// identifier root, passed by value, is deep-cloned on
    /// transfer — STRING_BUF_CLONE when the cloned closure holds
    /// a String. Applied to every call's arguments (a superset
    /// of the sites that can fire it; a false positive only
    /// keeps the import).
    fn projection_clone_args(&mut self, call: &CallExpr) {
        for arg in &call.args {
            if !matches!(arg, Node::FieldExpr(_) | Node::IndexExpr(_)) {
                continue;
            }
            if Self::projection_root(arg).is_none() {
                continue;
            }
            if let Some(ty) = self.check.expr_type(arg) {
                if self.clone_mentions_string(ty, 0) {
                    self.flags.string = true;
                }
            }
        }
    }

    fn walk(&mut self, node: &Node) {
        match node {
            Node::Program(p) => {
                for d in &p.decls {
                    self.walk(d);
                }
            }
            Node::FuncDecl(f) => {
                for s in &f.body {
                    self.walk(s);
                }
            }
            Node::TestDecl(t) => {
                for s in &t.body {
                    self.walk(s);
                }
            }
            Node::ImplDecl(d) => {
                for m in &d.methods {
                    self.walk(m);
                }
            }
            Node::OverloadDecl(d) => {
                for m in &d.methods {
                    self.walk(m);
                }
            }
            Node::MacroDecl(m) => {
                for s in &m.body {
                    self.walk(s);
                }
            }
            Node::LetStmt(l) => {
                if let Some(ty) = &l.ty {
                    if ty_has_string(ty.as_ref())
                        && matches!(&*l.value, Node::Literal(Literal::StringVal(_)))
                    {
                        self.flags.string = true;
                    }
                }
                self.walk(&l.value);
            }
            Node::ConstStmt(c) => {
                if let Some(ty) = &c.ty {
                    if ty_has_string(ty.as_ref())
                        && matches!(&*c.value, Node::Literal(Literal::StringVal(_)))
                    {
                        self.flags.string = true;
                    }
                }
                self.walk(&c.value);
            }
            Node::VarStmt(v) => {
                if ty_has_string(v.ty.as_ref()) {
                    self.flags.string = true;
                }
            }
            Node::LetElseStmt(l) => {
                self.walk(&l.value);
                for s in &l.else_block {
                    self.walk(s);
                }
            }
            Node::LetDestructureStmt(l) => {
                if let Some(rest) = &l.rest_name {
                    if rest != "_" {
                        // Slice rest binding: fresh Slice cell via
                        // SLICE_NEW (destructure.rs).
                        self.flags.string_format = true;
                    }
                }
                self.walk(&l.value);
            }
            Node::AssignStmt(a) => {
                if matches!(&*a.value, Node::Literal(Literal::StringVal(_))) {
                    // `s = "lit"` materializes an owned handle
                    // when the target holds a String (probe q06).
                    let mut needs_string = false;
                    if let Some(ty) = self.check.expr_type(&a.target) {
                        if self.ty_mentions_string(ty, 0) {
                            needs_string = true;
                        }
                    }
                    if !needs_string {
                        if let Node::FieldExpr(f) = &*a.target {
                            if let Some(base_ty) = self.check.expr_type(&f.expr) {
                                if let Some(field_ty) =
                                    self.struct_field_ty(base_ty, &f.field_name)
                                {
                                    if self.ty_mentions_string(&field_ty, 0) {
                                        needs_string = true;
                                    }
                                }
                            }
                        }
                    }
                    if needs_string {
                        self.flags.string = true;
                    }
                }
                self.walk(&a.target);
                self.walk(&a.value);
            }
            Node::ExprStmt(e) => self.walk(e),
            Node::ReturnStmt(r) => {
                if let Some(v) = &r.value {
                    self.walk(v);
                }
            }
            Node::BlockStmt(b) => {
                for s in &b.body {
                    self.walk(s);
                }
            }
            Node::ArenaStmt(b) => {
                for s in &b.body {
                    self.walk(s);
                }
            }
            Node::WhileStmt(w) => {
                self.walk(&w.cond);
                for s in &w.body {
                    self.walk(s);
                }
            }
            Node::ForStmt(f) => {
                self.walk(&f.start);
                if let Some(e) = &f.end {
                    self.walk(e);
                }
                for s in &f.body {
                    self.walk(s);
                }
            }
            Node::IfExpr(i) => {
                self.walk(&i.cond);
                if let Some(chain) = &i.let_chain {
                    for cond in chain {
                        self.walk(&cond.value);
                    }
                }
                for s in &i.then_block {
                    self.walk(s);
                }
                if let Some(e) = &i.else_block {
                    for s in e {
                        self.walk(s);
                    }
                }
            }
            Node::BinaryExpr(b) => {
                self.walk(&b.left);
                self.walk(&b.right);
            }
            Node::CallExpr(call) => self.walk_call(call),
            Node::GenericFuncRef(g) => {
                match g.func_name.as_str() {
                    "format" => {
                        self.flags.string_format = true;
                        self.flags.fmt = true;
                    }
                    "println" => self.flags.print = true,
                    "panic" | "panic_msg" => self.flags.panic = true,
                    "str_eq" => self.flags.string = true,
                    _ => {}
                }
            }
            Node::Identifier(name) => {
                // A bare builtin name used as a value (fn item);
                // defensive — a same-named local only over-flags.
                match name.as_str() {
                    "format" => {
                        self.flags.string_format = true;
                        self.flags.fmt = true;
                    }
                    "println" => self.flags.print = true,
                    "panic" | "panic_msg" => self.flags.panic = true,
                    "str_eq" => self.flags.string = true,
                    _ => {}
                }
            }
            Node::Literal(Literal::StringVal(_)) => {
                // Evaluated as a value: SLICE_NEW (codegen.rs
                // `gen_literal_value`). Literals in the builtin
                // argument positions are handled inside
                // `walk_call` and never reach this arm.
                self.flags.string_format = true;
            }
            Node::BorrowExpr(b) => self.walk(&b.expr),
            Node::MoveExpr(m) => self.walk(&m.expr),
            Node::DerefExpr(d) => self.walk(&d.expr),
            Node::CastExpr(c) => self.walk(&c.expr),
            Node::FieldExpr(f) => {
                if f.field_name == "len" {
                    if matches!(&*f.expr, Node::Literal(Literal::StringVal(_))) {
                        self.flags.string = true;
                    }
                }
                self.walk(&f.expr);
            }
            Node::IndexExpr(i) => {
                self.walk(&i.target);
                self.walk(&i.index);
            }
            Node::SliceExpr(sl) => {
                self.walk(&sl.target);
                self.walk(&sl.start);
                self.walk(&sl.end);
            }
            Node::ClosureLiteral(c) => {
                self.walk(&c.body);
            }
            Node::AwaitExpr(a) => self.walk(&a.expr),
            Node::TryExpr(t) => self.walk(&t.expr),
            Node::StructLiteral(l) => {
                // into.rs shape D: a projection value stored into
                // a struct literal field is deep-cloned when the
                // field type needs it.
                for field in &l.fields {
                    if matches!(&*field.value, Node::FieldExpr(_) | Node::IndexExpr(_))
                        && Self::projection_root(&field.value).is_some()
                    {
                        let mut needs_string = false;
                        if let Some(ty) = self.check.expr_type(&field.value) {
                            if self.clone_mentions_string(ty, 0) {
                                needs_string = true;
                            }
                        }
                        if !needs_string {
                            if let Some(field_ty) =
                                self.struct_field_ty(l.ty.as_ref(), &field.name)
                            {
                                if self.clone_mentions_string(&field_ty, 0) {
                                    needs_string = true;
                                }
                            }
                        }
                        if needs_string {
                            self.flags.string = true;
                        }
                    }
                    self.walk(&field.value);
                }
                if let Some(u) = &l.update_expr {
                    self.walk(u);
                }
            }
            Node::EnumLiteral(l) => {
                for f in &l.fields {
                    self.walk(&f.value);
                }
            }
            Node::TupleLiteral(l) => {
                for e in &l.elements {
                    self.walk_projection_clone_element(e);
                    self.walk(e);
                }
            }
            Node::ArrayLiteral(l) => {
                for e in &l.elements {
                    self.walk_projection_clone_element(e);
                    self.walk(e);
                }
            }
            Node::RepeatArrayLiteral(r) => self.walk(&r.value),
            Node::SwitchExpr(sw) => {
                self.walk(&sw.val);
                for case in &sw.cases {
                    self.walk(&case.pattern);
                    for s in &case.body {
                        self.walk(s);
                    }
                }
            }
            Node::MatchExpr(m) => {
                self.walk(&m.val);
                for case in &m.cases {
                    if let Some(g) = &case.guard {
                        self.walk(g);
                    }
                    for s in &case.body {
                        self.walk(s);
                    }
                }
            }
            Node::GoStmt(g) => self.walk(&g.call),
            Node::DeferStmt(d) => self.walk(&d.call),
            Node::SelectStmt(s) => {
                for arm in &s.arms {
                    if let Some(recv) = &arm.recv {
                        self.walk(recv);
                    }
                    for stmt in &arm.body {
                        self.walk(stmt);
                    }
                }
                if let Some(timeout) = &s.timeout {
                    self.walk(&timeout.expr);
                    for stmt in &timeout.body {
                        self.walk(stmt);
                    }
                }
            }
            Node::UnsafeExpr(u) => {
                for s in &u.body {
                    self.walk(s);
                }
            }
            Node::InlineAsmExpr(_) => {}
            // Declarations and other nodes with no emittable body:
            // StructDecl / EnumDecl / TraitDecl / TypeAliasDecl /
            // ImportDecl / UsingDecl / CheckHookDecl /
            // SublanguageDecl / ShaderDecl / StructDecl field types
            // emit nothing on their own.
            _ => {}
        }
    }

    /// Aggregate-literal elements (tuple / array): a projection
    /// element is cloned on transfer like a call argument when
    /// its closure holds a String (same shape-C discipline).
    fn walk_projection_clone_element(&mut self, elem: &Node) {
        if !matches!(elem, Node::FieldExpr(_) | Node::IndexExpr(_)) {
            return;
        }
        if Self::projection_root(elem).is_none() {
            return;
        }
        if let Some(ty) = self.check.expr_type(elem) {
            if self.clone_mentions_string(ty, 0) {
                self.flags.string = true;
            }
        }
    }
}

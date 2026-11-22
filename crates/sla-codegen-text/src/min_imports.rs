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
//!   Also the `@sa_mem_free` provider: deep drops of smart
//!   pointers (`Box` / `Rc` / `Arc`, drop.rs) and the HashMap
//!   slot free (map.rs) emit `call @sa_mem_free`, whose only
//!   declaration among the five is the `@extern` in
//!   `string.sa`; any node whose recorded type materializes
//!   such a value keeps `string` (walk.rs top-of-walk probe).
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
//!   The same provider shape covers the array fill: a repeat
//!   literal with u8 elements emits EXPAND SLA_ARRAY_FILL_U8
//!   (agg_literal.rs), whose unconditionally emitted macro body
//!   calls `@sa_mem_set`; that callee is defined in
//!   `core/mem.sa` and reaches the program through the same
//!   `core/slice.sa` importers, so a u8 repeat literal also
//!   keeps `string_format` (walk.rs
//!   `repeat_fill_needs_string_format`, plus the struct-field
//!   declared-type route for field position, where the
//!   literal's own recorded type may be absent).
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

/// The conditional imports, in emission order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MinImports {
    pub string: bool,
    pub string_format: bool,
    pub print: bool,
    pub fmt: bool,
    pub panic: bool,
    /// `sa_std/core/mem.sa`: a repeat array literal with a scalar
    /// non-u8 element type and length above the bulk threshold
    /// emits `call @sa_mem_copy_nonoverlapping` (agg_literal.rs's
    /// doubling bulk copy); that callee is defined in
    /// `core/mem.sa`. Without the import the host rejects the
    /// call as an undeclared callee (test_unit_def_r8pin_slot_transfer).
    /// Also a channel send of an aggregate (non-word) payload
    /// emits `call @sa_mem_copy` (chan.rs gen_chan_send, and the
    /// select.rs send staging); the same import declares it
    /// (task 89: Chan<(i64,i64)> trapped 1007 undeclared).
    pub mem: bool,
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

/// A type materializes a value whose deep drop (or map slot
/// free) emits `call @sa_mem_free`: a smart pointer or a
/// HashMap, at any nesting depth. Syntactic only, like
/// [`ty_has_string`].
fn ty_has_mem_free_value(ty: &Type) -> bool {
    match ty {
        Type::UserDefined(u) => {
            matches!(u.name.as_str(), "Box" | "Rc" | "Arc" | "HashMap")
                || u.generics.iter().any(ty_has_mem_free_value)
        }
        Type::Borrow(inner) | Type::Pointer(inner) => {
            ty_has_mem_free_value(inner.as_ref())
        }
        Type::Array(arr) => ty_has_mem_free_value(arr.elem.as_ref()),
        Type::Tuple(tup) => tup.elems.iter().any(ty_has_mem_free_value),
        Type::Future(inner) => ty_has_mem_free_value(inner.as_ref()),
        Type::Closure(sig) => {
            sig.params.iter().any(ty_has_mem_free_value)
                || ty_has_mem_free_value(sig.ret.as_ref())
        }
        Type::FnPtr(sig) => {
            sig.params.iter().any(ty_has_mem_free_value)
                || ty_has_mem_free_value(sig.ret.as_ref())
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
            is_variadic: sig.is_variadic,
        }),
        _ => ty.clone(),
    }
}

struct Scanner<'a> {
    check: &'a dyn CheckInfo,
    flags: MinImports,
    /// Top-level enum declaration names in the (import-expanded)
    /// program tree, collected like `Codegen::enum_index` in
    /// generate_program. Needed by the chan-send rule, whose
    /// word/aggregate predicate must know whether a payload type
    /// is an enum (CheckInfo carries no enum lookup).
    enum_names: HashSet<String>,
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
            enum_names: HashSet::new(),
        };
        scanner.collect_fn_index(program);
        scanner.collect_enum_names(program);
        scanner
    }

    fn collect_enum_names(&mut self, program: &Node) {
        let Node::Program(prog) = program else {
            return;
        };
        for decl in &prog.decls {
            if let Node::EnumDecl(e) = decl {
                self.enum_names.insert(e.name.clone());
            }
        }
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
}

mod call_rules;
mod type_rules;
mod walk;

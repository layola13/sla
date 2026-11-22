//! Call-site rules for the minimal-import scan (`Scanner` methods):
//! callee signature lookup, the literal-argument rule, and the
//! call-expression walk arm. Part of [`super`] — see the parent
//! module docs for the rule catalogue.

use super::*;

impl<'a> Scanner<'a> {
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
    pub(super) fn walk_call(&mut self, call: &CallExpr) {
        // Smart-pointer drop site: releasing a Box / Rc / Arc
        // binding emits `call @sa_mem_free` (drop.rs
        // emit_smart_pointer_drop_text), and @sa_mem_free is
        // declared in sa_std/fmt.sai (automatch index:
        // `sa_mem_free extern sa_std/fmt.sai`). Any smart-pointer
        // associated call puts such a value in play; the scan
        // cannot prove the drop unreachable, so fmt stays —
        // without it the host rejects the free call as an
        // undeclared callee (test_unit_ptr_int_casts).
        if let Some(target) = &call.associated_target {
            if matches!(target.as_str(), "Box" | "Rc" | "Arc") {
                self.flags.fmt = true;
            }
        }
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

        // FUSED pop().unwrap() (DEF-RUST-POP-OPTION-UNBOXING,
        // surface.rs try_gen_pop_unwrap_fused): the fused emission
        // expands PANIC_MSG in program text on the None arm, so a
        // direct `.unwrap()` on a `.pop()` call needs
        // sa_std/core/panic.sa even though the source has no panic
        // call. Conservative shape-only rule (a kept import only
        // costs time): any unwrap-on-pop call keeps panic.
        if call.func_name == "unwrap"
            && call.args.len() == 1
            && matches!(&call.args[0], Node::CallExpr(inner) if inner.func_name == "pop")
        {
            self.flags.panic = true;
        }

        // Channel send of an aggregate payload (task 89):
        // chan.rs gen_chan_send stages non-word payloads with
        // `call @sa_mem_copy(&slot, &val, size)`, and select.rs
        // stages send-case payloads the same way; @sa_mem_copy
        // is defined in sa_std/core/mem.sa. The receiver type
        // fact is the same one the emitter's `chan_method`
        // requires (a missing fact means no chan emission, so
        // no flag — emitter and scan agree by construction).
        // Select send cases reach here too: the walk visits a
        // select arm's call through arm.recv.
        if call.func_name == "send"
            && call.callee.is_none()
            && call.associated_target.is_none()
            && call.args.len() == 2
        {
            if let Some(Type::UserDefined(u)) = self.check.expr_type(&call.args[0]) {
                if u.name == "Chan" && u.generics.len() == 1 {
                    if self.chan_send_needs_mem_copy(&u.generics[0]) {
                        self.flags.mem = true;
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

        self.array_to_slice_call_rule(call);
        self.projection_clone_args(call);
        if let Some(callee) = &call.callee {
            self.walk(callee);
        }
        for arg in &call.args {
            self.walk(arg);
        }
    }

    /// Callee parameters as (is_borrow, type) pairs, in the same
    /// lookup order as `callee_string_params` (checker table,
    /// then the program-tree index, then the receiver's type).
    /// Extern signatures are skipped: their ABI spellings do not
    /// carry the slice shape, and checker-visited extern call
    /// sites are covered by the fact probe in walk().
    fn callee_param_shapes(&self, call: &CallExpr) -> Option<Vec<(bool, Type)>> {
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
                        .map(|p| (p.is_borrow, p.ty.as_ref().clone()))
                        .collect(),
                );
            }
            if let Some(decl) = self.fn_index.get(key) {
                return Some(
                    decl.params
                        .iter()
                        .map(|p| (p.is_borrow, p.ty.as_ref().clone()))
                        .collect(),
                );
            }
        }
        if call.associated_target.is_none() && !call.args.is_empty() {
            if let Some(recv_ty) = self.check.expr_type(&call.args[0]) {
                if let Type::UserDefined(u) = peel_borrow_pointer_type(recv_ty) {
                    let key = mangle_method_name(&u.name, &call.func_name);
                    if let Some(decl) = self.check.func(&key) {
                        return Some(
                            decl.params
                                .iter()
                                .map(|p| (p.is_borrow, p.ty.as_ref().clone()))
                                .collect(),
                        );
                    }
                    if let Some(decl) = self.fn_index.get(&key) {
                        return Some(
                            decl.params
                                .iter()
                                .map(|p| (p.is_borrow, p.ty.as_ref().clone()))
                                .collect(),
                        );
                    }
                }
            }
        }
        None
    }

    /// The macro-body twin of the walk() fact probe: a call
    /// inside a macro body is never checker-visited, so no
    /// `array_to_slice_borrow_args` fact exists for it, yet the
    /// inline expansion still materializes the slice view
    /// (SLICE_NEW). Flag `string_format` when a borrow of an
    /// array literal, or of an identifier with no recorded type
    /// (the macro-body signature), is passed to a borrow-of-slice
    /// parameter. A false positive only keeps the import.
    fn array_to_slice_call_rule(&mut self, call: &CallExpr) {
        let Some(params) = self.callee_param_shapes(call) else {
            return;
        };
        for (i, arg) in call.args.iter().enumerate() {
            let Some((is_borrow, ty)) = params.get(i) else {
                continue;
            };
            if !(*is_borrow || matches!(ty, Type::Borrow(_))) {
                continue;
            }
            if crate::place::slice_element_type(ty).is_none() {
                continue;
            }
            let Node::BorrowExpr(b) = arg else {
                continue;
            };
            match &*b.expr {
                Node::ArrayLiteral(_) | Node::RepeatArrayLiteral(_) => {
                    self.flags.string_format = true;
                }
                Node::Identifier(_) if self.check.expr_type(arg).is_none() => {
                    self.flags.string_format = true;
                }
                _ => {}
            }
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
}

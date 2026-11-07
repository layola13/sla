//! F group (import machine) — the cached std import modules
//! and macro templates of `sab_codegen.zig` (current-head lines
//! 6071–6330), the dep append/flush pair (`appendStdDepsNow`,
//! `flushPendingStdDeps`), and the wiring of D4's
//! [`SurfaceEmitter`] / [`StdDepEnsurer`] traits onto the real
//! F machinery.
//!
//! The one Zig dependency this crate cannot supply is
//! `flattenStdSnippet` — sci's SA-text flattener plus verifier,
//! host-side machinery that lives outside the SAB codegen in
//! Zig too (`sci_bridge`). It sits behind the
//! [`StdSnippetFlattener`] trait: the import machine builds the
//! exact snippet source Zig builds and asks the provider for
//! the flattened/decoded result. The default provider,
//! [`NoSnippetFlattener`], fails loudly with the F coordinate —
//! never a silent skip, never a text-backend fallback (the
//! SAB-D31 boundary registration).
//!
//! With a provider present, the whole chain runs: preload deps
//! append immediately outside function bodies (Zig's
//! `ensureStdDeps` branch — this resolves SAB-D26's queue-only
//! port), pending deps flush at function end, and surface rules
//! expand through cached templates or fresh fragments.

use crate::error::Result;
use crate::fragment::{unsupported_f, FragmentMachine};
use crate::ir::*;
use crate::surface_query::{StdDepEnsurer, SurfaceEmitter};
use crate::types::{PendingStdDep, StdImportModule, StdMacroTemplate};

/// The sci flattener boundary (Zig `flattenStdSnippet` +
/// verifier + `encodeSabFromFlatUnchecked` + `sab.decodeModule`,
/// reached through `sci_bridge`).
pub trait StdSnippetFlattener {
    /// Zig `cachedStdImportModule`'s flatten+verify+sig-clone
    /// chain for `@import "{import_path}"`: returns the module
    /// view the cache stores.
    fn flatten_import_module(&mut self, import_path: &str) -> Result<StdImportModule>;
    /// Zig `flattenStdSnippet` → `encodeSabFromFlatUnchecked` →
    /// `sab.decodeModule` for an explicit snippet source:
    /// returns the decoded module.
    fn flatten_and_decode(&mut self, source: &str) -> Result<Module>;
}

/// The un-wired flattener: every request is a loud F-group stop
/// at the sci boundary.
pub struct NoSnippetFlattener;

impl StdSnippetFlattener for NoSnippetFlattener {
    fn flatten_import_module(&mut self, import_path: &str) -> Result<StdImportModule> {
        Err(unsupported_f(
            &format!(
                "flattenStdSnippet for import `{import_path}` (sci flattener/verifier not ported)"
            ),
            "6071-6107",
        ))
    }

    fn flatten_and_decode(&mut self, _source: &str) -> Result<Module> {
        Err(unsupported_f(
            "flattenStdSnippet for macro fragment snippet (sci flattener/verifier not ported)",
            "6187-6230",
        ))
    }
}

impl<'a> FragmentMachine<'a> {
    /// Zig `cachedStdImportModule` (line 6071): cache by import
    /// path; on a miss, the provider flattens
    /// `@import "{import_path}"` and the result is stored.
    pub fn cached_std_import_module(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
        import_path: &str,
    ) -> Result<StdImportModule> {
        if let Some(&idx) = self.state.program.std_import_module_ids.get(import_path) {
            return Ok(self.state.program.std_import_modules[idx].clone());
        }
        let module = flattener.flatten_import_module(import_path)?;
        let idx = self.state.program.std_import_modules.len();
        self.state
            .program
            .std_import_module_ids
            .insert(import_path.to_string(), idx);
        self.state.program.std_import_modules.push(module);
        Ok(self.state.program.std_import_modules[idx].clone())
    }

    /// Zig `appendStdDepsNow` (line 6137): append the dep-
    /// filtered slice of the cached import module, then mark
    /// every requested dep included.
    pub fn append_std_deps_now(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
        import_path: &str,
        deps: &[String],
    ) -> Result<()> {
        if deps.is_empty() {
            return Ok(());
        }
        let cached = self.cached_std_import_module(flattener, import_path)?;
        let module = cached.module();
        self.append_decoded_module_filtered(&module, deps)?;
        for dep in deps {
            self.state.program.included_imports.insert(dep.clone());
        }
        Ok(())
    }

    /// Zig `pendingStdDepContains` (line 6150).
    pub fn pending_std_dep_contains(&self, dep: &str) -> bool {
        self.state
            .program
            .pending_std_deps
            .iter()
            .any(|pending| pending.dep == dep)
    }

    /// Zig `ensureStdDeps` (line 6108): deps missing from both
    /// `included_imports` and the pending queue either queue
    /// (inside a function body) or append immediately (outside
    /// one — the `generate`/preload case).
    pub fn ensure_std_deps(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
        import_path: &str,
        deps: &[String],
    ) -> Result<()> {
        if deps.is_empty() {
            return Ok(());
        }
        let missing: Vec<String> = deps
            .iter()
            .filter(|dep| {
                !self.state.program.included_imports.contains(*dep)
                    && !self.pending_std_dep_contains(dep)
            })
            .cloned()
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        if self.state.frame.in_function_body {
            for dep in missing {
                self.state.program.pending_std_deps.push(PendingStdDep {
                    import_path: import_path.to_string(),
                    dep,
                });
            }
            return Ok(());
        }
        self.append_std_deps_now(flattener, import_path, &missing)
    }

    /// Zig `flushPendingStdDeps` (line 6160): append every
    /// pending dep not yet included, in queue order, then clear
    /// the queue. Zig iterates the live list by index; appends
    /// do not grow the queue, so an index walk over a snapshot
    /// of the entries is the same order.
    pub fn flush_pending_std_deps(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
    ) -> Result<()> {
        if self.state.program.pending_std_deps.is_empty() {
            return Ok(());
        }
        let mut i = 0;
        while i < self.state.program.pending_std_deps.len() {
            let pending = self.state.program.pending_std_deps[i].clone();
            if !self.state.program.included_imports.contains(&pending.dep) {
                self.append_std_deps_now(
                    flattener,
                    &pending.import_path,
                    std::slice::from_ref(&pending.dep),
                )?;
            }
            i += 1;
        }
        self.state.program.pending_std_deps.clear();
        Ok(())
    }

    /// Zig `stdMacroTemplateKey` (line 6183).
    pub fn std_macro_template_key(
        import_path: &str,
        macro_name: &str,
        arg_count: usize,
    ) -> String {
        format!("{import_path}\x1f{macro_name}\x1f{arg_count}")
    }

    /// Zig `cachedStdMacroTemplate` (line 6187): build the
    /// placeholder snippet, flatten + decode it, and cache the
    /// resulting template module under
    /// `(import_path, macro_name, arg_count)`.
    pub fn cached_std_macro_template(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
        import_path: &str,
        macro_name: &str,
        arg_count: usize,
    ) -> Result<StdMacroTemplate> {
        let lookup_key = Self::std_macro_template_key(import_path, macro_name, arg_count);
        if let Some(&idx) = self.state.program.std_macro_template_ids.get(&lookup_key) {
            return Ok(self.state.program.std_macro_templates[idx].clone());
        }
        let func_name = format!(
            "__sla_macro_template_{}",
            self.state.program.std_macro_templates.len()
        );
        let mut source = format!(
            "@import \"{import_path}\"\n@{func_name}() -> void:\nL_ENTRY:\n    EXPAND {macro_name}"
        );
        for idx in 0..arg_count {
            let placeholder = crate::fragment_template::std_macro_placeholder(idx);
            if idx == 0 {
                source.push_str(&format!(" {placeholder}"));
            } else {
                source.push_str(&format!(", {placeholder}"));
            }
        }
        source.push_str("\n    return\n");
        let module = flattener.flatten_and_decode(&source)?;
        // Zig's `ownDecodedModuleSymbols` is ownership
        // bookkeeping for borrowed pool slices; this port's
        // decoder already owns its strings (SAB-D29 family).
        let idx = self.state.program.std_macro_templates.len();
        self.state
            .program
            .std_macro_template_ids
            .insert(lookup_key.clone(), idx);
        self.state.program.std_macro_templates.push(StdMacroTemplate {
            key: lookup_key,
            func_name,
            arg_count,
            module,
        });
        Ok(self.state.program.std_macro_templates[idx].clone())
    }

    /// Zig `emitCachedStdMacroFragment` (line 6278): the
    /// template-cache path. Returns `false` when the cache
    /// cannot serve this call (unsafe args, the
    /// string_format hygiene bypass, or a template that does
    /// not support the args) and the caller must go fresh.
    pub fn emit_cached_std_macro_fragment(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
        import_path: &str,
        macro_name: &str,
        args: &[String],
    ) -> Result<bool> {
        if !crate::fragment_template::std_macro_template_args_safe(args) {
            return Ok(false);
        }
        // FORMAT_* / STRFMT_* hygiene locals desync under the
        // cache (Zig comment, lines 6280–6290): bypass it for
        // the whole string_format module.
        if import_path == "sa_std/string_format.sa" {
            return Ok(false);
        }
        let template =
            self.cached_std_macro_template(flattener, import_path, macro_name, args.len())?;
        if template.arg_count != args.len() {
            return Err(unsupported_f(
                "std macro template arg count mismatch",
                "6291-6292",
            ));
        }
        if !crate::fragment_template::std_macro_template_supports_args(&template, args)? {
            return Ok(false);
        }
        let module = template.module.clone();
        self.append_decoded_module_const_decls(&module)?;
        self.append_renamed_template_fragment_body(&module, &template.func_name.clone(), args)?;
        Ok(true)
    }

    /// Zig `emitStdMacroFragment` (line 6299).
    pub fn emit_std_macro_fragment(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
        import_path: &str,
        macro_name: &str,
        args: &[String],
    ) -> Result<()> {
        self.emit_std_macro_fragment_with_literal_args(flattener, import_path, macro_name, args, &[])
    }

    /// Zig `emitStdMacroFragmentWithLiteralArgs` (line 6303):
    /// any literal arg forces the fresh path; otherwise the
    /// cache is tried first. (Zig falls back to fresh only on
    /// `error.UnsupportedType` from the cached attempt and
    /// propagates everything else; this port propagates all
    /// cached-path errors — SAB-D32.)
    pub fn emit_std_macro_fragment_with_literal_args(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
        import_path: &str,
        macro_name: &str,
        args: &[String],
        literal_args: &[bool],
    ) -> Result<()> {
        if !literal_args.is_empty() && literal_args.len() != args.len() {
            return Err(unsupported_f(
                "literal_args length does not match args",
                "6303-6305",
            ));
        }
        let has_literal_args = literal_args.iter().any(|is_literal| *is_literal);
        if has_literal_args {
            return self.emit_std_macro_fragment_fresh(
                flattener,
                import_path,
                macro_name,
                args,
                literal_args,
            );
        }
        let used_cached =
            self.emit_cached_std_macro_fragment(flattener, import_path, macro_name, args)?;
        if used_cached {
            return Ok(());
        }
        self.emit_std_macro_fragment_fresh(flattener, import_path, macro_name, args, literal_args)
    }

    /// Zig `emitStdMacroFragmentFresh` (line 6323): flatten a
    /// one-off fragment function (`__sla_macro_fragment_{n}`)
    /// whose EXPAND line carries placeholders — or the literal
    /// arg text for literal args — decode it, append its consts,
    /// and splice its body with placeholder substitution and
    /// hygiene renaming.
    pub fn emit_std_macro_fragment_fresh(
        &mut self,
        flattener: &mut dyn StdSnippetFlattener,
        import_path: &str,
        macro_name: &str,
        args: &[String],
        literal_args: &[bool],
    ) -> Result<()> {
        let func_name = format!("__sla_macro_fragment_{}", self.state.macro_fragment_idx);
        self.state.macro_fragment_idx += 1;
        let mut source = format!(
            "@import \"{import_path}\"\n@{func_name}() -> void:\nL_ENTRY:\n    EXPAND {macro_name}"
        );
        for (i, arg) in args.iter().enumerate() {
            let use_literal = !literal_args.is_empty() && literal_args[i];
            let rendered = if use_literal {
                arg.clone()
            } else {
                crate::fragment_template::std_macro_placeholder(i)
            };
            if i == 0 {
                source.push_str(&format!(" {rendered}"));
            } else {
                source.push_str(&format!(", {rendered}"));
            }
        }
        source.push_str("\n    return\n");
        let module = flattener.flatten_and_decode(&source)?;
        self.append_decoded_module_const_decls(&module)?;
        self.append_renamed_template_fragment_body(&module, &func_name, args)?;
        Ok(())
    }

    /// Zig `emitCallBody`: a call instruction in text-body form,
    /// with the body's registers recorded (the BTreeSet surface
    /// special case's tail, via D4's `SurfaceEmitter`).
    pub fn emit_call_body(&mut self, out_reg: Option<u32>, body: String) -> Result<()> {
        let mut inst = Instruction::new(
            InstKind::Call,
            0,
            self.b().instructions_len() as u32,
        );
        if let Some(reg) = out_reg {
            self.b().record_reg(reg);
            inst.operands[0] = Operand::Reg(reg);
            inst.operands[1] = Operand::Text(body.clone());
        } else {
            inst.operands[0] = Operand::Text(body.clone());
        }
        self.record_instruction_regs(&inst);
        self.b().append_instruction(inst);
        Ok(())
    }
}

/// D4's dep ensurer over the real F machine: Zig's
/// `ensureStdDeps` with both branches (queue inside a function
/// body, append-now outside one). This replaces the queue-only
/// `PendingStdDeps` at the driver's step-2 call site (SAB-D26
/// resolution); `PendingStdDeps` itself remains as the
/// queue-branch unit under test.
pub struct MachineStdDeps<'a> {
    pub machine: FragmentMachine<'a>,
    pub flattener: &'a mut dyn StdSnippetFlattener,
}

impl StdDepEnsurer for MachineStdDeps<'_> {
    fn ensure_std_deps(&mut self, import_path: &str, deps: &[String]) -> Result<()> {
        self.machine
            .ensure_std_deps(self.flattener, import_path, deps)
    }
}

/// D4's surface emitter over the real F machine: the
/// `SurfaceEmitter` trait's production implementation (the
/// loud default remains for call sites that have no machine).
pub struct MachineSurfaceEmitter<'a> {
    pub machine: FragmentMachine<'a>,
    pub flattener: &'a mut dyn StdSnippetFlattener,
}

impl SurfaceEmitter for MachineSurfaceEmitter<'_> {
    fn emit_call_body(&mut self, out_reg: Option<u32>, body: String) -> Result<()> {
        self.machine.emit_call_body(out_reg, body)
    }

    fn emit_std_macro_fragment_with_literal_args(
        &mut self,
        import_path: &str,
        macro_name: &str,
        args: &[String],
        literal_args: &[bool],
    ) -> Result<()> {
        self.machine.emit_std_macro_fragment_with_literal_args(
            self.flattener,
            import_path,
            macro_name,
            args,
            literal_args,
        )
    }
}

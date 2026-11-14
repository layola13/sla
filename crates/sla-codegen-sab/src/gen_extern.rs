//! B1 — contract extern emission for the direct SAB driver.
//!
//! Zig anchors: `makeContractExternSig` / `emitContractExternDecl`
//! / `emitReferencedContractExternDecls` (sab_codegen.zig
//! 7573–7640, 15442–15480) and the fallible-extern result body
//! `emitFallibleExternResultBody` (20580–20640). Split from
//! `driver.rs` / `driver_gen.rs` for the 700-line file discipline.
//!
//! The checker lane registers `.sai` contracts into
//! `tc.extern_funcs`; the SAB driver consumes them through the
//! `CheckInfo::extern_sig` projection (the declaration side of the
//! SAB surface registry: the signature is built from the contract
//! declaration, never patched at a call site). Calls render with
//! the same planned-operand discipline as user-function static
//! calls (shared helpers below); the driver records every
//! referenced extern during body emission and declares them all in
//! step 7 of `driver.rs`'s orchestration.

use sla_ast::{CallExpr, Node};
use sla_lowering::ExternSigView;

use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::fragment::FragmentMachine;
use crate::ir::{CapPrefix, FunctionKind, FunctionSig, OpKind, Operand, ParamSpec, PrimType};

/// Zig `abiPrimType` (sab_codegen.zig:7542): an ABI type spelling
/// to its SAB primitive. Unknown spellings map to void, exactly as
/// in Zig (composite handle types ride as pointers at the sites
/// that need them; the contracts in the corpus use scalar
/// spellings). The `i1` arm is SA's own spelling of bool in `.sai`
/// contracts (DEF-SAB-FMT-BOOL-I1 on the Zig side).
pub(crate) fn abi_prim_type(raw: &str) -> PrimType {
    let mut name = raw.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
    if let Some(rest) = name.strip_prefix(['&', '^', '*']) {
        name = rest.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
    }
    if let Some(rest) = name.strip_suffix('!') {
        name = rest.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
    }
    match name {
        "ptr" => PrimType::Ptr,
        "bool" | "i1" => PrimType::I1,
        "i8" => PrimType::I8,
        "i16" => PrimType::I16,
        "i32" => PrimType::I32,
        "i64" | "int" | "isize" => PrimType::I64,
        "u8" => PrimType::U8,
        "u16" => PrimType::U16,
        "u32" => PrimType::U32,
        "u64" | "usize" => PrimType::U64,
        "f32" => PrimType::F32,
        "f64" | "float" => PrimType::F64,
        _ => PrimType::Void,
    }
}

/// Zig `abiReturnCap` over `abiCapKindFromRawTypeString`: the
/// return spelling's capability prefix, if any.
fn abi_return_cap(raw: &str) -> Option<CapPrefix> {
    let name = raw.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
    match name.chars().next() {
        Some('&') => Some(CapPrefix::Borrow),
        Some('^') => Some(CapPrefix::Move),
        Some('*') => Some(CapPrefix::Raw),
        _ => None,
    }
}

/// Zig `abiFalliblePayloadOffset` (lowering_rules.zig:3985): the
/// payload of a fallible extern cell sits after the i32 status
/// word, aligned to the payload's own alignment (8 for
/// pointer-width / f64 spellings, 4 otherwise).
fn abi_fallible_payload_offset(raw: &str) -> usize {
    let mut name = raw.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
    if let Some(rest) = name.strip_prefix(['&', '^', '*']) {
        name = rest.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
    }
    if let Some(rest) = name.strip_suffix('!') {
        name = rest.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
    }
    let payload_align: usize = match name {
        "ptr" | "i64" | "u64" | "isize" | "usize" | "int" | "f64" | "float" => 8,
        _ => 4,
    };
    (4 + payload_align - 1) & !(payload_align - 1)
}

/// A planned call's rendered body plus its post-call bookkeeping
/// (Zig `genStaticCall`'s planned materializations): registers the
/// call consumed (cap Move) and registers to release after the
/// call (borrow arguments that were materialized temporaries).
pub(crate) struct RenderedCall {
    pub body: String,
    pub post_call_releases: Vec<u32>,
    pub post_call_consumed: Vec<u32>,
}

impl<'c> Driver<'c> {
    /// Render a planned call's operand list under the given
    /// per-parameter caps (shared by the user-function static-call
    /// tail in `driver_gen.rs` and the contract-extern arm below):
    /// the host verifier reads the per-arg cap glyphs off the call
    /// body text, so a borrow arg must render `&place` (address-of,
    /// never a materialized borrow register) and a move-cap arg
    /// must render `^value`; bare names are cap ByValue and trap
    /// 1013 against borrow/move contracts.
    pub(crate) fn render_planned_call_operands(
        &mut self,
        call: &CallExpr,
        lowered: &str,
        param_caps: &[CapPrefix],
    ) -> Result<RenderedCall> {
        let mut rendered: Vec<String> = Vec::with_capacity(call.args.len());
        let mut post_call_releases: Vec<u32> = Vec::new();
        let mut post_call_consumed: Vec<u32> = Vec::new();
        for (i, arg) in call.args.iter().enumerate() {
            let cap = param_caps.get(i).copied();
            let operand = match cap {
                Some(CapPrefix::Borrow) => match arg {
                    Node::BorrowExpr(borrow) => {
                        self.gen_prefixed_borrow_call_arg(borrow, &mut post_call_releases)?
                    }
                    _ => {
                        let reg = self.gen_expr(arg)?;
                        if !self.is_local_reg(reg) {
                            post_call_releases.push(reg);
                        }
                        format!("&{}", self.b().symbol_name(reg))
                    }
                },
                Some(CapPrefix::Move) => {
                    // Zig's shallow-copy call-arg branch
                    // (genStaticCall): an identifier arg of a
                    // shallow-copyable struct type is copied
                    // and the copy is moved into the call, so
                    // the source binding stays live afterwards.
                    let shallow_ty = match arg {
                        Node::Identifier(_) => self.check.expr_type(arg).cloned().filter(|ty| {
                            self.check.type_is_shallow_copy_call_arg_value(ty)
                        }),
                        _ => None,
                    };
                    if let Some(ty) = shallow_ty {
                        let reg = self.gen_expr(arg)?;
                        let copied = self.gen_shallow_copy_call_arg_value(reg, &ty)?;
                        post_call_consumed.push(copied);
                        format!("^{}", self.b().symbol_name(copied))
                    } else {
                        let reg = self.gen_expr(arg)?;
                        post_call_consumed.push(reg);
                        format!("^{}", self.b().symbol_name(reg))
                    }
                }
                _ => {
                    let reg = self.gen_expr(arg)?;
                    self.b().symbol_name(reg).to_string()
                }
            };
            rendered.push(operand);
        }
        Ok(RenderedCall {
            body: format!("@{}({})", lowered, rendered.join(", ")),
            post_call_releases,
            post_call_consumed,
        })
    }

    /// Emit a rendered planned call and hand back its result
    /// register (Zig `emitPlannedCallBody`: the void arm's result
    /// is a sentinel tmp assigned 0).
    pub(crate) fn emit_planned_call_result(&mut self, body: &str, is_void: bool) -> u32 {
        if is_void {
            self.b().emit_call(None, body);
            let sentinel = self.b().new_tmp();
            self.b().emit_assign_imm(sentinel, 0);
            sentinel
        } else {
            let dst = self.b().new_tmp();
            self.b().emit_call(Some(dst), body);
            dst
        }
    }

    /// Post-call bookkeeping for a rendered planned call: cap-Move
    /// registers join the released set so later cleanups stay
    /// silent (release_emit::mark_consumed — the RegState::Released
    /// bookkeeping release_locals consults, not the Consumed
    /// classification); borrow temporaries are released.
    pub(crate) fn apply_planned_post_call(&mut self, rendered: RenderedCall) -> Result<()> {
        for reg in rendered.post_call_consumed {
            crate::release_emit::mark_consumed(&mut self.state, reg);
        }
        for reg in &rendered.post_call_releases {
            self.emit_release(*reg)?;
        }
        Ok(())
    }

    /// A call to a contract extern (Zig genCall's extern target
    /// resolution + planned-call emission): caps and arity come
    /// from the contract signature the checker registered, the
    /// target symbol stays as-is (`loweredFuncSymbol`), and the
    /// referenced name is recorded for driver step 7's declaration
    /// emission. A fallible extern's raw cell is wrapped into a
    /// `Result<T, i32>` value, mirroring the SA-text backend.
    /// Probe for a concrete `.sa` definition of a
    /// contract-extern name: already-flattened import modules
    /// first, then the root program's direct `.sa` imports
    /// (flattened on demand; the flattener caches). Returns the
    /// import path whose module defines the name.
    fn sa_defined_extern_path(&mut self, name: &str) -> Option<String> {
        if let Some(m) = self
            .state
            .program
            .std_import_modules
            .iter()
            .find(|m| module_defines(m, name))
        {
            return Some(m.import_path.clone());
        }
        let roots = self.state.program.root_sa_imports.clone();
        for path in roots {
            if let Ok(m) = self.flattener.flatten_import_module(&path) {
                if module_defines(&m, name) {
                    return Some(path);
                }
            }
        }
        None
    }

    pub(crate) fn gen_extern_call(&mut self, call: &CallExpr) -> Result<u32> {
        let view = self.check.extern_sig(&call.func_name).ok_or_else(|| {
            unsupported(
                &format!(
                    "extern call `{}` (contract signature unavailable to the SAB driver)",
                    call.func_name
                ),
                "B1 contract extern emission",
                "15460-15480",
            )
        })?;
        if view.is_variadic {
            if call.args.len() < view.params.len() {
                return Err(Error::UnsupportedSabDirectFeature(format!(
                    "call to variadic extern `{}` with {} args, declared at least {} [group B1 contract extern emission; Zig sab_codegen.zig:15451]",
                    call.func_name,
                    call.args.len(),
                    view.params.len()
                )));
            }
        } else if call.args.len() != view.params.len() {
            return Err(Error::UnsupportedSabDirectFeature(format!(
                "call to extern `{}` with {} args, declared arity {} [group B1 contract extern emission; Zig sab_codegen.zig:15451]",
                call.func_name,
                call.args.len(),
                view.params.len()
            )));
        }
        // A contract name that an imported `.sa` module
        // concretely defines is a program function, not an
        // archive extern: ensure its body and skip the extern
        // declaration (registering one would pull the archive
        // object that also defines its inlined siblings —
        // duplicate symbol at link; growvec_addr_stable).
        let sa_defined_path = self.sa_defined_extern_path(&call.func_name);
        match &sa_defined_path {
            Some(path) => {
                let name = call.func_name.clone();
                self.ensure_method_std_deps(path, &[&name])?;
            }
            None => {
                self.referenced_externs.insert(call.func_name.clone());
            }
        }
        let lowered = crate::symbols::lowered_func_symbol(&call.func_name, self.check);
        let mut param_caps: Vec<CapPrefix> = view
            .params
            .iter()
            .map(|p| {
                if p.is_borrow {
                    CapPrefix::Borrow
                } else if p.is_move {
                    CapPrefix::Move
                } else {
                    CapPrefix::ByValue
                }
            })
            .collect();
        // Variadic tail arguments ride by value.
        while param_caps.len() < call.args.len() {
            param_caps.push(CapPrefix::ByValue);
        }
        let rendered = self.render_planned_call_operands(call, &lowered, &param_caps)?;
        if view.fallible {
            let result = self.gen_fallible_extern_result(&view, &rendered.body)?;
            self.apply_planned_post_call(rendered)?;
            return Ok(result);
        }
        let is_void = abi_prim_type(&view.ret_abi) == PrimType::Void;
        let result = self.emit_planned_call_result(&rendered.body, is_void);
        self.apply_planned_post_call(rendered)?;
        Ok(result)
    }

    /// Zig `emitFallibleExternResultBody` (20580–20640): call the
    /// extern into its fallible cell, load the i32 status at
    /// offset 0, and materialize a `Result<T, i32>` — payload
    /// loaded only on the Ok arm, the status itself becomes the
    /// Err payload. Branch/temp discipline mirrors the port's
    /// other branch emitters (branch_state snapshot/merge).
    fn gen_fallible_extern_result(&mut self, view: &ExternSigView, body: &str) -> Result<u32> {
        let fallible_reg = self.b().new_tmp();
        self.b().emit_call(Some(fallible_reg), body);
        let status_reg = self.b().new_tmp();
        self.emit_load(status_reg, fallible_reg, 0, PrimType::I32);
        let result_reg = self.b().new_tmp();
        let ok_reg = self.b().new_tmp();
        self.b().emit_op(
            ok_reg,
            OpKind::Eq,
            Operand::Reg(status_reg),
            Operand::ImmI64(0),
        );
        let ok_label = self.b().new_label("L_FALLIBLE_OK");
        let err_label = self.b().new_label("L_FALLIBLE_ERR");
        let end_label = self.b().new_label("L_FALLIBLE_END");
        self.b().emit_branch(ok_reg, &ok_label, &err_label);
        let branch_locals_len = self.state.frame.locals.len();
        let pre_released = crate::branch_state::clone_released(&self.state);

        self.b().emit_label(&ok_label);
        crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
        crate::release_emit::mark_consumed(&mut self.state, ok_reg);
        let payload_reg = self.b().new_tmp();
        self.emit_load(
            payload_reg,
            fallible_reg,
            abi_fallible_payload_offset(&view.ret_abi),
            abi_prim_type(&view.ret_abi),
        );
        let result_name = self.b().symbol_name(result_reg).to_string();
        let payload_name = self.b().symbol_name(payload_reg).to_string();
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
            &mut *self.flattener,
            "sa_std/core/result.sa",
            "RESULT_NEW_OK",
            &[result_name.clone(), payload_name],
        )?;
        // The store inside RESULT_NEW_OK moves the payload value
        // into the Result allocation; retire the source register.
        crate::release_emit::mark_consumed(&mut self.state, payload_reg);
        self.b().emit_jmp(&end_label);
        let then_released = crate::branch_state::clone_released(&self.state);
        self.state.frame.pop_locals_to(branch_locals_len);
        crate::branch_state::restore_released(&mut self.state, &pre_released);

        self.b().emit_label(&err_label);
        crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
        let status_name = self.b().symbol_name(status_reg).to_string();
        FragmentMachine::new(&mut self.state).emit_std_macro_fragment(
            &mut *self.flattener,
            "sa_std/core/result.sa",
            "RESULT_NEW_ERR",
            &[result_name, status_name],
        )?;
        self.b().emit_jmp(&end_label);
        let else_released = crate::branch_state::clone_released(&self.state);
        self.state.frame.pop_locals_to(branch_locals_len);
        crate::branch_state::set_merge_released(
            &mut self.state,
            false,
            &then_released,
            false,
            &else_released,
            &pre_released,
        );
        self.b().emit_label(&end_label);
        if !self.is_local_reg(status_reg) {
            self.emit_release(status_reg)?;
        }
        self.emit_release(fallible_reg)?;
        Ok(result_reg)
    }

    /// Whether the builder already holds an external signature
    /// under this (lowered) name — Zig `hasFunctionSig(name,
    /// .external)` for the emitContractExternDecl dedupe.
    pub(crate) fn has_external_sig(&mut self, lowered: &str) -> bool {
        let len = self.b().function_sigs_len();
        (0..len).any(|i| {
            let sig = self.b().sig_clone(i);
            sig.kind == FunctionKind::External && sig.name == lowered
        })
    }

    /// Zig `makeContractExternSig` + `emitContractExternDecl`
    /// (7573–7615): build the external signature from the contract
    /// declaration (reg ids are the parameter ids) and append its
    /// decl instruction. The SAB-D17 dual track is mirrored as in
    /// `begin_sig`: the sig's own id switches to the emission
    /// counter only after the decl operands are captured.
    pub(crate) fn gen_contract_extern_decl(
        &mut self,
        name: &str,
        view: &ExternSigView,
    ) -> Result<()> {
        let lowered = crate::symbols::lowered_func_symbol(name, self.check);
        let test_count = self.state.program.test_sigs.len();
        let mut sig = FunctionSig::new(0, &lowered, abi_prim_type(&view.ret_abi), 0);
        sig.kind = FunctionKind::External;
        sig.return_cap = abi_return_cap(&view.ret_abi);
        sig.return_fallible = view.fallible;
        sig.is_variadic = view.is_variadic;
        self.state.frame.begin_function();
        let sig_index = self.b().add_sig(sig);
        self.b().begin_function(sig_index);
        let mut specs = Vec::with_capacity(view.params.len());
        let mut param_ids = Vec::with_capacity(view.params.len());
        for p in &view.params {
            let param_id = self.b().intern(&p.name);
            specs.push(ParamSpec {
                name: p.name.clone(),
                ty: abi_prim_type(&p.abi_ty),
                cap: if p.is_borrow {
                    CapPrefix::Borrow
                } else if p.is_move {
                    CapPrefix::Move
                } else {
                    CapPrefix::ByValue
                },
            });
            param_ids.push(param_id);
            self.b().record_reg(param_id);
        }
        self.b().set_sig_params(sig_index, specs, param_ids);
        self.b().set_sig_return_cap(sig_index, abi_return_cap(&view.ret_abi));
        self.b().append_decl_inst(sig_index);
        let counter_id = (sig_index + test_count) as u32;
        self.b().set_sig_id(sig_index, counter_id);
        self.b().finish_function();
        self.state.frame.in_function_body = false;
        Ok(())
    }
}

/// Whether a flattened import module CONCRETELY defines `name`:
/// its function sigs carry the name with a non-External kind.
/// (The module's plain symbol list is not evidence — it also
/// names functions the module merely references or declares
/// extern, e.g. io.sa's macros calling @sa_io_stdin.)
fn module_defines(module: &crate::types::StdImportModule, name: &str) -> bool {
    module
        .function_sigs
        .iter()
        .any(|s| s.name == name && s.kind != crate::ir::FunctionKind::External)
}


#[cfg(test)]
mod module_defines_tests {
    use super::module_defines;
    use crate::ir::{FunctionKind, FunctionSig, PrimType};
    use crate::types::StdImportModule;

    fn module(symbols: Vec<&str>, sigs: Vec<FunctionSig>) -> StdImportModule {
        StdImportModule {
            import_path: "sa_std/test.sa".to_string(),
            symbols: symbols.into_iter().map(str::to_string).collect(),
            function_sigs: sigs,
            const_decls: Vec::new(),
            instructions: Vec::new(),
        }
    }

    fn sig(name: &str, kind: FunctionKind) -> FunctionSig {
        let mut sig = FunctionSig::new(0, name, PrimType::U64, 0);
        sig.kind = kind;
        sig
    }

    #[test]
    fn referenced_extern_is_not_a_definition() {
        // io.sa shape: the name is only in the symbol list, as a
        // referenced runtime extern declared by io.sai.
        let m = module(vec!["sa_io_stdin"], vec![sig("sa_io_stdin", FunctionKind::External)]);
        assert!(!module_defines(&m, "sa_io_stdin"));
    }

    #[test]
    fn symbol_only_mention_is_not_a_definition() {
        let m = module(vec!["sa_io_stdin"], vec![]);
        assert!(!module_defines(&m, "sa_io_stdin"));
    }

    #[test]
    fn concrete_export_is_a_definition() {
        // growvec.sa shape: sa_growvec_data_ptr is @export-defined
        // in the flattened module while its .sai declares it
        // @extern — the probe must claim it so the body is
        // ensured and the archive object is never pulled.
        let m = module(
            vec!["sa_growvec_data_ptr"],
            vec![sig("sa_growvec_data_ptr", FunctionKind::Exported)],
        );
        assert!(module_defines(&m, "sa_growvec_data_ptr"));
    }
}

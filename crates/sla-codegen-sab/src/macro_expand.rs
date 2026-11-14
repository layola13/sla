//! G group (expandMacroOps): the imported-macro expansion
//! orchestration.
//!
//! Faithful port of `genImportedMacroCall` (sab_codegen.zig:15072,
//! reference tree @3de91932): destination allocation for
//! expression-output macros, per-argument lowering via the G
//! ArgRole dispatch (`callsite_args::gen_imported_macro_arg`),
//! collection of release / restore / output-rebinding /
//! output-store obligations, the direct-macro fast path, the
//! F-group fragment splice, and the ordered post-expansion
//! fixups. The statement-level sentinel (`assign 0`) for
//! non-expression macros is reproduced exactly.
//!
//! Seams: the direct-macro emitter family
//! (`emitDirectImportedMacroCall`, sab_codegen.zig:14709–15070 —
//! the JSON/ONCE/BTREE fast paths) and the fragment machine
//! (F group) are consumed through [`MacroExpandHost`]; the
//! release-lifecycle planning (`plannedCallArgReleaseReg`,
//! sab_codegen.zig:15640) belongs to the lowering/E seam and is
//! likewise a host service here. See the G-group report.

use sla_ast::{Node, Type};

use crate::callsite_args::{
    emit_load, emit_stack_alloc, emit_store, gen_imported_macro_arg, MacroCallHost,
    SabLoweredCallArg,
};
use crate::error::{Error, Result};
use crate::ir::PrimType;
use crate::macro_plan::ImportedMacroCallPlan;
use crate::types::MacroExpansionContext;

/// Zig `DirectImportedMacroResult` (sab_codegen.zig:14707).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectImportedMacroResult {
    pub emitted: bool,
    pub consumed_dst: Option<u32>,
}

/// Host services for the expansion orchestration, on top of the
/// call-site argument host.
pub trait MacroExpandHost: MacroCallHost {
    /// Zig `plannedCallArgReleaseReg` (sab_codegen.zig:15640):
    /// RefCell-borrow-handle-aware release planning for a lowered
    /// argument's release candidate.
    fn planned_call_arg_release_reg(&self, candidate: Option<u32>) -> Option<u32>;

    /// Zig `emitDirectImportedMacroCall` (sab_codegen.zig:14709).
    fn emit_direct_imported_macro_call(
        &mut self,
        macro_name: &str,
        arg_names: &[String],
        known_init_func: Option<&str>,
    ) -> Result<DirectImportedMacroResult>;

    /// Zig `emitStdMacroFragment` — the F-group fragment machine's
    /// splice of a baked std macro body with these argument names.
    fn emit_std_macro_fragment(
        &mut self,
        import_path: &str,
        macro_name: &str,
        arg_names: &[String],
    ) -> Result<()>;

    /// Zig `pushTypedLocal(name, reg, false, ty)` for a leading
    /// output rebinding.
    fn push_typed_local(&mut self, name: &str, reg: u32, ty: &Type) -> Result<()>;

    /// Zig `releaseNonLocalTemps`.
    fn release_non_local_temps(&mut self, regs: &[u32]) -> Result<()>;
}

struct Restore {
    slot: u32,
    value: u32,
}

struct OutputRebinding {
    name: String,
    reg: u32,
    ty: Type,
}

struct OutputStore {
    slot: u32,
    value: u32,
    ty: PrimType,
}

/// Zig `genImportedMacroCall` (sab_codegen.zig:15072 in the port's
/// reference tree; the current Zig tree carries the
/// W1-DEF-SAB-PTRBYTEADD-MOVE re-materialisation near its tail,
/// flowed back here — see the `result_alias_risk` block below).
pub fn expand_imported_macro_call<H: MacroExpandHost>(
    host: &mut H,
    plan: &ImportedMacroCallPlan,
    call_args: &[Node],
    ctx: Option<&MacroExpansionContext>,
    result_ty: Option<&Type>,
) -> Result<u32> {
    let import_path = plan.import_path.clone().ok_or_else(|| {
        Error::UnsupportedSabDirectFeature(format!(
            "imported macro {} has no import path",
            plan.macro_name
        ))
    })?;
    let dst = if plan.expression_output {
        let reg = host.builder().new_tmp();
        host.builder().record_reg(reg);
        Some(reg)
    } else {
        None
    };

    let mut arg_names: Vec<String> = Vec::new();
    let mut release_regs: Vec<u32> = Vec::new();
    let mut restores: Vec<Restore> = Vec::new();
    let mut output_rebindings: Vec<OutputRebinding> = Vec::new();
    let mut output_stores: Vec<OutputStore> = Vec::new();
    // W1-DEF-SAB-PTRBYTEADD-MOVE: set when a borrow-expression
    // argument contributed a temp that this call releases at its
    // end (the explicit borrow the SAB lowering forms for
    // `&place`, or an address temp). A pointer result derived
    // from that borrow is an interior child of it, and the host
    // verifier consumes interior children when the parent borrow
    // is released — retiring the result before its first real
    // use. (Zig comment, genImportedMacroCall.)
    let mut result_alias_risk = false;

    if let Some(reg) = dst {
        arg_names.push(host.builder().symbol_name(reg).to_string());
    }
    for (i, arg) in call_args.iter().enumerate() {
        // Direct ONCE_GET_OR_INIT calls named initializers by their
        // lowered function symbol, so materializing an
        // otherwise-unused vtable value would leave an invalid
        // indirect-call register in the SAB module.
        // (Zig comment, sab_codegen.zig:15102–15104.)
        if plan.macro_name == "ONCE_GET_OR_INIT" && i == 1 {
            if let Node::Identifier(name) = arg {
                if host.funcs_contains(name) {
                    arg_names.push(name.clone());
                    continue;
                }
            }
        }
        let lowered_arg: SabLoweredCallArg = {
            let host_dyn: &mut dyn MacroCallHost = host;
            gen_imported_macro_arg(host_dyn, plan, i, arg, ctx)?
        };
        arg_names.push(lowered_arg.operand.clone());
        if matches!(arg, Node::BorrowExpr(_))
            && (lowered_arg.release_reg.is_some() || !lowered_arg.release_regs.is_empty())
        {
            result_alias_risk = true;
        }
        if let Some(reg) = host.planned_call_arg_release_reg(lowered_arg.release_reg) {
            release_regs.push(reg);
        }
        release_regs.extend_from_slice(&lowered_arg.release_regs);
        if let Some(slot) = lowered_arg.restore_slot {
            let value = lowered_arg.restore_value.ok_or_else(|| {
                Error::UnsupportedSabDirectFeature(
                    "imported macro arg restore without value".to_string(),
                )
            })?;
            restores.push(Restore { slot, value });
        }
        if let Some(name) = &lowered_arg.output_bind_name {
            let reg = host.builder().intern(&lowered_arg.operand);
            let ty = lowered_arg.output_bind_ty.clone().ok_or_else(|| {
                Error::MissingType("imported macro output binding type".to_string())
            })?;
            output_rebindings.push(OutputRebinding {
                name: name.clone(),
                reg,
                ty,
            });
        }
        if let Some(slot) = lowered_arg.output_store_slot {
            let value = host.builder().intern(&lowered_arg.operand);
            let ty = lowered_arg.output_store_ty.ok_or_else(|| {
                Error::MissingType("imported macro output store type".to_string())
            })?;
            output_stores.push(OutputStore { slot, value, ty });
        }
    }

    let known_init_func: Option<String> =
        if plan.macro_name == "ONCE_GET_OR_INIT" && call_args.len() == 2 {
            match &call_args[1] {
                Node::Identifier(name) if host.funcs_contains(name) => Some(name.clone()),
                _ => None,
            }
        } else {
            None
        };
    let direct_result = host.emit_direct_imported_macro_call(
        &plan.macro_name,
        &arg_names,
        known_init_func.as_deref(),
    )?;
    if !direct_result.emitted {
        host.emit_std_macro_fragment(&import_path, &plan.macro_name, &arg_names)?;
    }
    {
        let host_dyn: &mut dyn MacroCallHost = host;
        for restore in &restores {
            emit_store(host_dyn, restore.slot, 0, restore.value, PrimType::Ptr);
            host_dyn.mark_consumed(restore.value)?;
        }
    }
    for binding in &output_rebindings {
        host.push_typed_local(&binding.name, binding.reg, &binding.ty)?;
    }
    if let Some(consumed) = direct_result.consumed_dst {
        host.mark_consumed(consumed)?;
    }
    {
        let host_dyn: &mut dyn MacroCallHost = host;
        for store in &output_stores {
            emit_store(host_dyn, store.slot, 0, store.value, store.ty);
            host_dyn.emit_release(store.value)?;
        }
    }
    // W1-DEF-SAB-PTRBYTEADD-MOVE: with a borrow-derived pointer
    // result at risk (see result_alias_risk above), re-materialize
    // the result through a stack slot BEFORE the argument borrows
    // are released — the same store/load shape the legal
    // bound-variable form takes. The loaded register is parented
    // to the slot, which outlives every later use of the result;
    // the store itself consumes the interior child while its
    // parent borrow is still live, which the verifier accepts.
    let mut effective_dst = dst;
    if let Some(reg) = dst {
        if result_alias_risk {
            if let Some(ty) = result_ty {
                if host.type_is_pointer_scalar_value(ty) {
                    let slot = host.builder().new_tmp();
                    {
                        let host_dyn: &mut dyn MacroCallHost = host;
                        emit_stack_alloc(host_dyn, slot, 8);
                        emit_store(host_dyn, slot, 0, reg, PrimType::Ptr);
                    }
                    let fresh = host.builder().new_tmp();
                    {
                        let host_dyn: &mut dyn MacroCallHost = host;
                        emit_load(host_dyn, fresh, slot, 0, PrimType::Ptr);
                    }
                    effective_dst = Some(fresh);
                }
            }
        }
    }
    host.release_non_local_temps(&release_regs)?;
    if let Some(reg) = effective_dst {
        return Ok(reg);
    }
    let sentinel = host.builder().new_tmp();
    host.builder().emit_assign_imm(sentinel, 0);
    Ok(sentinel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::ModuleBuilder;
    use crate::types::AddressSource;
    use sla_ast::Primitive;
    use std::collections::{HashMap, HashSet};

    struct MockHost {
        builder: ModuleBuilder,
        next_reg: u32,
        types: HashMap<String, Type>,
        fragments: Vec<(String, String, Vec<String>)>,
        direct: DirectImportedMacroResult,
        pushed: Vec<String>,
        released_temps: Vec<u32>,
        funcs: HashSet<String>,
        all_regs_non_local: bool,
    }

    impl MockHost {
        fn new() -> Self {
            MockHost {
                builder: ModuleBuilder::new(),
                next_reg: 100,
                types: HashMap::new(),
                fragments: vec![],
                direct: DirectImportedMacroResult {
                    emitted: false,
                    consumed_dst: None,
                },
                pushed: vec![],
                released_temps: vec![],
                funcs: HashSet::new(),
                all_regs_non_local: false,
            }
        }
    }

    impl MacroCallHost for MockHost {
        fn builder(&mut self) -> &mut ModuleBuilder {
            &mut self.builder
        }
        fn gen_expr(&mut self, _arg: &Node, _ctx: Option<&MacroExpansionContext>) -> Result<u32> {
            let name = format!("val{}", self.next_reg);
            self.next_reg += 1;
            Ok(self.builder.intern(&name))
        }
        fn gen_address_of(
            &mut self,
            _arg: &Node,
            _ctx: Option<&MacroExpansionContext>,
        ) -> Result<AddressSource> {
            Ok(AddressSource {
                reg: 42,
                release_regs: vec![],
                restore_slot: None,
            })
        }
        fn arg_type(
            &mut self,
            arg: &Node,
            _ctx: Option<&MacroExpansionContext>,
        ) -> Result<Option<Type>> {
            Ok(self.types.get(&format!("{arg:?}")).cloned())
        }
        fn gen_string_literal(&mut self, _value: &str) -> Result<u32> {
            Ok(self.builder.intern("slice_lit"))
        }
        fn binding_reg(&mut self, name: &str) -> Result<u32> {
            Ok(self.builder.intern(&format!("bind_{name}")))
        }
        fn macro_assign_target_name(
            &mut self,
            _arg: &Node,
            _ctx: &MacroExpansionContext,
        ) -> Option<String> {
            None
        }
        fn stack_local_reg(&self, _name: &str) -> Option<u32> {
            None
        }
        fn stack_alloc_emitted(&self, _reg: u32) -> bool {
            false
        }
        fn note_stack_alloc(&mut self, _reg: u32) {}
        fn is_local_reg(&self, _reg: u32) -> bool {
            !self.all_regs_non_local
        }
        fn has_global_scalar_const(&self, _name: &str) -> bool {
            false
        }
        fn expr_has_fnptr_type(&self, _arg: &Node) -> bool {
            false
        }
        fn funcs_contains(&self, name: &str) -> bool {
            self.funcs.contains(name)
        }
        fn expr_type(&self, _arg: &Node) -> Option<Type> {
            None
        }
        fn has_constructor_surface_rule(&self, _t: &str, _n: &str) -> bool {
            false
        }
        fn type_is_pointer_scalar_value(&self, ty: &Type) -> bool {
            crate::release_emit::type_is_pointer_scalar_value(ty)
        }
        fn emit_release(&mut self, _reg: u32) -> Result<()> {
            Ok(())
        }
        fn emit_move(&mut self, _reg: u32) -> Result<()> {
            Ok(())
        }
        fn mark_consumed(&mut self, _reg: u32) -> Result<()> {
            Ok(())
        }
    }

    impl MacroExpandHost for MockHost {
        fn planned_call_arg_release_reg(&self, candidate: Option<u32>) -> Option<u32> {
            candidate
        }
        fn emit_direct_imported_macro_call(
            &mut self,
            _macro_name: &str,
            _arg_names: &[String],
            _known_init_func: Option<&str>,
        ) -> Result<DirectImportedMacroResult> {
            Ok(self.direct)
        }
        fn emit_std_macro_fragment(
            &mut self,
            import_path: &str,
            macro_name: &str,
            arg_names: &[String],
        ) -> Result<()> {
            self.fragments.push((
                import_path.to_string(),
                macro_name.to_string(),
                arg_names.to_vec(),
            ));
            Ok(())
        }
        fn push_typed_local(&mut self, name: &str, _reg: u32, _ty: &Type) -> Result<()> {
            self.pushed.push(name.to_string());
            Ok(())
        }
        fn release_non_local_temps(&mut self, regs: &[u32]) -> Result<()> {
            self.released_temps.extend_from_slice(regs);
            Ok(())
        }
    }

    fn plan(expr_out: bool) -> ImportedMacroCallPlan {
        ImportedMacroCallPlan {
            macro_name: "VEC_PUSH".to_string(),
            import_path: Some("sa_std/vec.sa".to_string()),
            arity: 2,
            leading_outputs: 0,
            borrowed_arg_mask: 0,
            address_slot_arg_mask: 0,
            expression_output: expr_out,
        }
    }

    fn typed(host: &mut MockHost, arg: &Node) {
        host.types
            .insert(format!("{arg:?}"), Type::Primitive(Primitive::I32));
    }

    #[test]
    fn statement_macro_splices_fragment_and_returns_sentinel() {
        let mut host = MockHost::new();
        let args = vec![
            Node::Identifier("v".to_string()),
            Node::Identifier("x".to_string()),
        ];
        for a in &args {
            typed(&mut host, a);
        }
        let reg = expand_imported_macro_call(&mut host, &plan(false), &args, None, None).unwrap();
        assert_eq!(host.fragments.len(), 1);
        assert_eq!(host.fragments[0].0, "sa_std/vec.sa");
        assert_eq!(host.fragments[0].2, vec!["val100", "val101"]);
        // Sentinel: a fresh tmp with an assign-imm 0 in the module.
        let name = host.builder.symbol_name(reg).to_string();
        assert!(name.starts_with("tmp_"));
        let module = host.builder.finish();
        assert_eq!(
            module.instructions.last().map(|i| i.kind),
            Some(crate::ir::InstKind::Assign)
        );
    }

    #[test]
    fn expression_macro_returns_dst_and_skips_sentinel() {
        let mut host = MockHost::new();
        let args = vec![Node::Identifier("x".to_string())];
        typed(&mut host, &args[0]);
        let reg = expand_imported_macro_call(&mut host, &plan(true), &args, None, None).unwrap();
        // dst is the first tmp; arg names lead with it.
        assert_eq!(host.fragments[0].2[0], "tmp_0");
        assert_eq!(host.builder.symbol_name(reg), "tmp_0");
    }

    #[test]
    fn direct_emitted_skips_fragment() {
        let mut host = MockHost::new();
        host.direct = DirectImportedMacroResult {
            emitted: true,
            consumed_dst: None,
        };
        let args = vec![Node::Identifier("x".to_string())];
        typed(&mut host, &args[0]);
        expand_imported_macro_call(&mut host, &plan(false), &args, None, None).unwrap();
        assert!(host.fragments.is_empty());
    }

    #[test]
    fn missing_import_path_is_loud() {
        let mut host = MockHost::new();
        let mut p = plan(false);
        p.import_path = None;
        let err = expand_imported_macro_call(&mut host, &p, &[], None, None).unwrap_err();
        assert!(matches!(err, Error::UnsupportedSabDirectFeature(_)));
    }

    /// DEF-RUST-PTRADD-BORROW-CONSUMED (W1-DEF-SAB-PTRBYTEADD-MOVE):
    /// a borrow-expression argument whose temp this call releases
    /// at its end leaves a borrow-derived pointer result an
    /// interior child of the borrow; the host verifier consumes
    /// interior children when the parent borrow is released, so
    /// the first later use traps UseAfterMove 1009. The expansion
    /// must re-materialise a pointer-scalar result through a stack
    /// slot before the releases and return the loaded register.
    /// Source-level reproducers (trap pre-fix / pass post-fix
    /// under the CLI): tests/fixtures/def_rust_ptradd_borrow_minims.sla;
    /// corpus carriers test_smoke1_fs and the sla2b copy of
    /// test_unit_wrap_net_smoke.
    #[test]
    fn borrow_arg_pointer_result_is_rematerialized_through_slot() {
        let mut host = MockHost::new();
        host.all_regs_non_local = true;
        host.direct = DirectImportedMacroResult {
            emitted: true,
            consumed_dst: None,
        };
        let args = vec![Node::BorrowExpr(sla_ast::BorrowExpr {
            expr: Box::new(Node::Identifier("buf".to_string())),
        })];
        typed(&mut host, &args[0]);
        let result_ty = Type::Primitive(Primitive::RawPtr);
        let reg =
            expand_imported_macro_call(&mut host, &plan(true), &args, None, Some(&result_ty))
                .unwrap();
        let dst_id = host.builder.intern("tmp_0");
        let borrow_id = host.builder.intern("val100");
        // The returned register is the loaded fresh tmp, not dst,
        // and the borrow temp was collected for release.
        assert_ne!(reg, dst_id);
        assert_eq!(host.released_temps, vec![borrow_id]);
        // The module carries stack_alloc -> store(dst) -> load(reg).
        let module = host.builder.finish();
        let kinds: Vec<crate::ir::InstKind> =
            module.instructions.iter().map(|i| i.kind).collect();
        let pos = |k: crate::ir::InstKind| kinds.iter().position(|x| *x == k).unwrap();
        assert!(pos(crate::ir::InstKind::StackAlloc) < pos(crate::ir::InstKind::Store));
        assert!(pos(crate::ir::InstKind::Store) < pos(crate::ir::InstKind::Load));
        let store = &module.instructions[pos(crate::ir::InstKind::Store)];
        assert!(matches!(store.operands[2], crate::ir::Operand::Reg(r) if r == dst_id));
        let load = &module.instructions[pos(crate::ir::InstKind::Load)];
        assert!(matches!(load.operands[0], crate::ir::Operand::Reg(r) if r == reg));
    }

    /// W1 gate control: without a pointer-scalar result type the
    /// same borrow argument leaves dst untouched.
    #[test]
    fn borrow_arg_without_result_type_is_not_rematerialized() {
        let mut host = MockHost::new();
        host.all_regs_non_local = true;
        host.direct = DirectImportedMacroResult {
            emitted: true,
            consumed_dst: None,
        };
        let args = vec![Node::BorrowExpr(sla_ast::BorrowExpr {
            expr: Box::new(Node::Identifier("buf".to_string())),
        })];
        typed(&mut host, &args[0]);
        let reg = expand_imported_macro_call(&mut host, &plan(true), &args, None, None).unwrap();
        assert_eq!(host.builder.symbol_name(reg), "tmp_0");
        let module = host.builder.finish();
        assert!(module
            .instructions
            .iter()
            .all(|i| i.kind != crate::ir::InstKind::StackAlloc));
    }

    /// W1 gate control: a non-borrow argument never puts the
    /// result at risk, even with a pointer-scalar result type.
    #[test]
    fn non_borrow_arg_pointer_result_is_not_rematerialized() {
        let mut host = MockHost::new();
        host.all_regs_non_local = true;
        host.direct = DirectImportedMacroResult {
            emitted: true,
            consumed_dst: None,
        };
        let args = vec![Node::Identifier("x".to_string())];
        typed(&mut host, &args[0]);
        let result_ty = Type::Primitive(Primitive::RawPtr);
        let reg =
            expand_imported_macro_call(&mut host, &plan(true), &args, None, Some(&result_ty))
                .unwrap();
        assert_eq!(host.builder.symbol_name(reg), "tmp_0");
        let module = host.builder.finish();
        assert!(module
            .instructions
            .iter()
            .all(|i| i.kind != crate::ir::InstKind::StackAlloc));
    }
}

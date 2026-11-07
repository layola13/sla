//! B4 — the `generate` driver, mirroring `Codegen.generate`
//! (sab_codegen.zig lines 535–632) plus the per-decl dispatch it
//! calls for the shapes this phase covers.
//!
//! Orchestration order, exactly as in Zig:
//! 1. pre-scan (C group call points): global scalar consts, global
//!    array consts, assigned bindings;
//! 2. std surface load + preload (D group call points);
//! 3. future-task helpers, if the program uses them (H group);
//! 4. trait vtable decls for impl blocks (I group);
//! 5. per-decl dispatch in source order: func / test / impl /
//!    overload; struct/enum/trait/alias/macro/import/using/const
//!    decls emit nothing, as in Zig;
//! 6. escaped closure entries (I group);
//! 7. referenced contract extern decls;
//! 8. encode.
//!
//! Coverage boundary for this phase: the fixture01-level subset —
//! user function declarations, inter-function calls, literals,
//! scalar binary operations, `if` / `while` forms, `let` / assign /
//! return / break / continue, and the `panic(code)` builtin. The
//! C group (pre-scan and symbols) is wired as of phase 3: global
//! scalar consts fold (aliases and binary expressions included),
//! checker-typed global array consts materialize as hex consts,
//! assigned bindings are collected by the full C2 walker, trait
//! vtable decls are emitted for program-local traits, and
//! decl-only extern prototypes with a concrete twin are
//! suppressed. The D group (std surface infrastructure) is wired
//! as of the SAB-D slice: step 2 loads the surface rules (D1)
//! and preloads surface deps into the pending queue (D5/D4,
//! SAB-D26); surface *expansion* at call sites still belongs to
//! the F-group fragment machinery. Every call point into a group
//! this phase does not implement (F fragments, G macros,
//! H async, I closures/impls) fails loudly with
//! `UnsupportedSabDirectFeature` naming the group and the Zig line
//! range — never a silent skip, never a text-backend fallback.
//!
//! Emission conventions inside the subset follow the phase-1 hand
//! model (`fixture01.rs`): the `ModuleBuilder` trunk interns in
//! emission order, no `L_ENTRY` label is emitted, and scalar Copy
//! temporaries get no release instructions (the E-group release
//! engine is not wired yet; registered as SAB-D19). Two Zig
//! conventions the model did not exercise are ported faithfully
//! here: function symbols are lowered (`main` stays `main`, other
//! user functions become `sla__<name>`, Zig `loweredFuncSymbol`
//! lines 953–961) and parameter registers are the scoped names
//! `<lowered>__param_<i>_<name>` (Zig `genFuncSig`). A third is
//! the SAB-D17 sig.id dual track: sig ids follow the Zig emission
//! counter, decl operands keep intern ids (see `begin_sig`).

use std::collections::HashMap;

use sla_ast::{FuncDecl, Node, Primitive, TestDecl, Type};
use sla_lowering::{
    abi_cap_kind_from_type, by_value_raw_pointer_param, is_borrow_like_type,
    is_void_type, shared_abi_param_cap, AbiCapKind,
    CheckInfo, ScalarBinaryOp,
};

use crate::encode::encode_module;
use crate::error::{Error, Result};
use crate::ir::{CapPrefix, FunctionKind, FunctionSig, OpKind, ParamSpec, PrimType};
use crate::prescan_bindings;
use crate::prescan_consts;
use crate::state::CodegenState;
use crate::surface_load::{self, SurfaceRoots};
use crate::surface_preload::SurfacePreloader;
use crate::surface_query::PendingStdDeps;
use crate::surface_scan;
use crate::symbols;
use crate::types::Local;

/// A loud, group-attributed unsupported error (SAB-D15: the reason
/// string always travels with the error).
pub(crate) fn unsupported(what: &str, group: &str, zig_ref: &str) -> Error {
    Error::UnsupportedSabDirectFeature(format!(
        "{what} [group {group}; Zig sab_codegen.zig:{zig_ref}]"
    ))
}

/// Zig `primType` (lines 1569–1591), primitive arm. Non-primitive
/// types route through the D2 layout adapter in Zig; only the
/// pointer-shaped cases are resolved here, the rest is loud.
fn prim_type(ty: &Type) -> Result<PrimType> {
    match ty {
        Type::Primitive(p) => Ok(match p {
            Primitive::I8 => PrimType::I8,
            Primitive::I16 => PrimType::I16,
            Primitive::I32 => PrimType::I32,
            Primitive::I64 | Primitive::Integer | Primitive::Isize => PrimType::I64,
            Primitive::U8 => PrimType::U8,
            Primitive::U16 => PrimType::U16,
            Primitive::U32 => PrimType::U32,
            Primitive::U64 | Primitive::Usize => PrimType::U64,
            Primitive::F32 => PrimType::F32,
            Primitive::F64 | Primitive::Float => PrimType::F64,
            Primitive::Boolean => PrimType::I1,
            Primitive::RawPtr => PrimType::Ptr,
            Primitive::VoidType => PrimType::Void,
        }),
        Type::Pointer(_) | Type::Borrow(_) | Type::FnPtr(_) | Type::Closure(_) => {
            Ok(PrimType::Ptr)
        }
        _ => Err(unsupported(
            "non-primitive type has no primType in the driver subset",
            "D2 layout",
            "1590-1706",
        )),
    }
}

/// Zig `abiReturnPrimType`: bool returns widen to i32.
fn abi_return_prim_type(ty: &Type) -> Result<PrimType> {
    if matches!(ty, Type::Primitive(Primitive::Boolean)) {
        return Ok(PrimType::I32);
    }
    prim_type(ty)
}

/// Zig `returnCapForType` (lines 5628–5635) over sla-lowering's
/// `abiCapKindFromType`.
fn return_cap_for_type(ty: &Type) -> Option<CapPrefix> {
    match abi_cap_kind_from_type(ty) {
        AbiCapKind::Borrow => Some(CapPrefix::Borrow),
        AbiCapKind::Move => Some(CapPrefix::Move),
        AbiCapKind::Raw => Some(CapPrefix::Raw),
        AbiCapKind::None => None,
    }
}

/// Zig `scalarBinaryOpKind` (lines 2349–2380): lowering plan ->
/// SAB op kind.
pub(crate) fn scalar_binary_op_kind(plan: ScalarBinaryOp) -> OpKind {
    match plan {
        ScalarBinaryOp::Add => OpKind::Add,
        ScalarBinaryOp::Sub => OpKind::Sub,
        ScalarBinaryOp::Mul => OpKind::Mul,
        ScalarBinaryOp::SDiv => OpKind::Sdiv,
        ScalarBinaryOp::UDiv => OpKind::Udiv,
        ScalarBinaryOp::SRem => OpKind::Srem,
        ScalarBinaryOp::URem => OpKind::Urem,
        ScalarBinaryOp::BitAnd | ScalarBinaryOp::LogicalAnd => OpKind::And,
        ScalarBinaryOp::BitOr | ScalarBinaryOp::LogicalOr => OpKind::Or,
        ScalarBinaryOp::BitXor => OpKind::Xor,
        ScalarBinaryOp::Shl => OpKind::Shl,
        ScalarBinaryOp::LShr => OpKind::Lshr,
        ScalarBinaryOp::AShr => OpKind::Ashr,
        ScalarBinaryOp::Eq => OpKind::Eq,
        ScalarBinaryOp::Ne => OpKind::Ne,
        ScalarBinaryOp::SLt => OpKind::Slt,
        ScalarBinaryOp::SLe => OpKind::Sle,
        ScalarBinaryOp::SGt => OpKind::Sgt,
        ScalarBinaryOp::SGe => OpKind::Sge,
        ScalarBinaryOp::ULt => OpKind::Ult,
        ScalarBinaryOp::ULe => OpKind::Ule,
        ScalarBinaryOp::UGt => OpKind::Ugt,
        ScalarBinaryOp::UGe => OpKind::Uge,
        ScalarBinaryOp::FAdd => OpKind::Fadd,
        ScalarBinaryOp::FSub => OpKind::Fsub,
        ScalarBinaryOp::FMul => OpKind::Fmul,
        ScalarBinaryOp::FDiv => OpKind::Fdiv,
        ScalarBinaryOp::FCmpEq => OpKind::FcmpEq,
        ScalarBinaryOp::FCmpNe => OpKind::FcmpNe,
        ScalarBinaryOp::FCmpLt => OpKind::FcmpLt,
        ScalarBinaryOp::FCmpLe => OpKind::FcmpLe,
        ScalarBinaryOp::FCmpGt => OpKind::FcmpGt,
        ScalarBinaryOp::FCmpGe => OpKind::FcmpGe,
    }
}

// The step-3 future scan is D5's full walker,
// `surface_scan::program_uses_future_task_runtime` — Zig's
// `programUsesFutureTaskRuntime`, lines 2247–2253.

/// One user function's callable shape, collected before emission
/// (calls may reference functions declared later in the program).
pub(crate) struct FuncShape {
    pub(crate) ret_prim: PrimType,
    pub(crate) is_void: bool,
    pub(crate) arity: usize,
}

pub struct Driver<'c> {
    pub(crate) state: CodegenState,
    pub(crate) check: &'c dyn CheckInfo,
    /// D1 surface-location roots for step 2 (defaults to the
    /// process environment; harnesses may substitute fixtures).
    pub surface_roots: SurfaceRoots,
    pub(crate) funcs: HashMap<String, FuncShape>,
    /// C2 per-function repeated-let names for the function currently
    /// being emitted (Zig `multi_let_bindings` is program-global
    /// there; the subset scopes it per function, as the Zig field
    /// comment describes the intent — see SAB-D08).
    pub(crate) multi_let: std::collections::HashSet<String>,
}

impl<'c> Driver<'c> {
    pub fn new(check: &'c dyn CheckInfo) -> Self {
        Driver {
            state: CodegenState::new(),
            check,
            surface_roots: SurfaceRoots::from_env(),
            funcs: HashMap::new(),
            multi_let: std::collections::HashSet::new(),
        }
    }

    pub(crate) fn b(&mut self) -> &mut crate::emit::ModuleBuilder {
        &mut self.state.builder
    }

    /// Zig `loweredFuncSymbol` (lines 953–963), via C3
    /// (`symbols::lowered_func_symbol_with`): `main`, extern and
    /// no-mangle functions keep their name; checker-known extern
    /// functions and `sa_`-prefixed names keep theirs; other user
    /// functions are prefixed `sla__`. The AST declaration supplies
    /// the extern/no-mangle fact (in the full pipeline the same
    /// declaration is what `tc.funcs` holds).
    fn lowered_symbol(&self, f: &FuncDecl) -> String {
        symbols::lowered_func_symbol_with(
            &f.name,
            Some(f.is_extern || f.no_mangle),
            self.check.is_extern_func(&f.name),
        )
    }

    /// Zig `bindingReg` (lines 948–951), via C3
    /// (`symbols::binding_reg`): repeated-let names get a fresh
    /// tmp register per binding site.
    pub(crate) fn binding_reg(&mut self, name: &str) -> u32 {
        let is_multi = self.multi_let.contains(name);
        symbols::binding_reg(self.b(), name, is_multi)
    }

    // -- B4 orchestration (Zig 535-632) ---------------------------

    pub fn generate(mut self, program: &Node) -> Result<Vec<u8>> {
        let prog = match program {
            Node::Program(p) => p,
            _ => {
                return Err(unsupported(
                    "top-level node is not a program",
                    "B4 driver",
                    "536",
                ))
            }
        };

        // Step 1: pre-scan (C1/C2 call points).
        self.prescan(prog)?;

        // Step 2: std surface load + preload (Zig 606–613), via
        // D1 (`surface_load::load_std_surface_rules`) and D5
        // (`SurfacePreloader`). The load's fallback walk is
        // recorded in the program state (SAB-D02's loud log);
        // a missing surface file leaves zero rules, as in Zig.
        // Preload discovers every surface dep and queues it in
        // `pending_std_deps` through D4's ensurer (SAB-D26: the
        // F-group append that Zig performs here is not ported
        // yet). Zig wraps preload errors with a trace and
        // re-raises; the port's errors carry their reasons
        // (SAB-D15), so propagation is direct. Surface *calls*
        // still stop loudly at their gen_call sites (group D).
        {
            let roots = self.surface_roots.clone();
            let log = surface_load::load_std_surface_rules(
                &mut self.state.program.std_surface_rules,
                &roots,
            )?;
            self.state.program.surface_load_log = Some(log);
            let rules = self.state.program.std_surface_rules.clone();
            let check = self.check;
            let mut ensurer = PendingStdDeps::new(
                &mut self.state.program.included_imports,
                &mut self.state.program.pending_std_deps,
            );
            let preloader = SurfacePreloader::new(check, &rules);
            preloader.preload_std_surface_deps(&mut ensurer, program)?;
        }

        // Step 3: future helpers (H call point). The gate is
        // D5's full scan (`programUsesFutureTaskRuntime`, Zig
        // 614–617); helper *emission* is H-group and not ported,
        // so a positive scan is a loud H stop, as before.
        if surface_scan::program_uses_future_task_runtime(program) {
            return Err(unsupported(
                "program uses future/task runtime; helper emission not ported",
                "H async",
                "614-617",
            ));
        }

        // Step 4: trait vtable decls (Zig 550–556), via C3
        // (`symbols::emit_trait_vtable_decl`): one vtable const
        // per trait impl, emitted before any function. Trait and
        // decl-only facts come from the program's own declarations
        // (plus the checker's function table under mangled names);
        // an impl of a trait the program does not declare fails
        // loudly inside C3, as Zig's `tc.traits` miss does. The
        // impl *methods* themselves remain I-group (step 5).
        {
            let mut traits: std::collections::HashMap<String, sla_ast::TraitDecl> =
                std::collections::HashMap::new();
            let mut decl_only: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for decl in &prog.decls {
                match decl {
                    Node::TraitDecl(t) => {
                        traits.insert(t.name.clone(), t.clone());
                    }
                    Node::ImplDecl(i) => {
                        if let Some(type_name) = symbols::concrete_type_name(&i.target_ty) {
                            for method in &i.methods {
                                if let Node::FuncDecl(f) = method {
                                    if f.is_decl_only {
                                        if let Some(trait_name) = &i.trait_name {
                                            decl_only.insert(
                                                symbols::mangle_trait_method_name(
                                                    type_name, trait_name, &f.name,
                                                ),
                                            );
                                        }
                                        decl_only.insert(symbols::mangle_method_name(
                                            type_name, &f.name,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            let facts = symbols::TraitFacts {
                traits: &traits,
                decl_only_funcs: &decl_only,
            };
            let check = self.check;
            for decl in &prog.decls {
                if let Node::ImplDecl(i) = decl {
                    if i.trait_name.is_some() {
                        symbols::emit_trait_vtable_decl(self.b(), &facts, check, i)?;
                    }
                }
            }
        }

        // Callable shapes for every concrete user function first:
        // Zig resolves callees through the checker's `tc.funcs`,
        // independent of declaration order.
        for decl in &prog.decls {
            if let Node::FuncDecl(f) = decl {
                if !f.is_decl_only {
                    self.funcs.insert(
                        f.name.clone(),
                        FuncShape {
                            ret_prim: abi_return_prim_type(&f.ret_ty)?,
                            is_void: is_void_type(&f.ret_ty),
                            arity: f.params.len(),
                        },
                    );
                }
            }
        }

        // Step 5: per-decl dispatch, source order (Zig 558-619).
        for decl in &prog.decls {
            match decl {
                Node::FuncDecl(f) => {
                    if f.is_decl_only {
                        // Zig generate: an extern prototype with a
                        // concrete definition in the same program
                        // emits nothing (C3
                        // `hasConcreteFunctionSymbol`, 965–973).
                        if f.is_extern
                            && !symbols::has_concrete_function_symbol(&prog.decls, &f.name)
                        {
                            self.gen_extern_decl(f)?;
                        }
                        // A decl-only non-extern emits nothing.
                    } else {
                        self.gen_func_decl(f)?;
                    }
                }
                Node::TestDecl(t) => self.gen_test_decl(t)?,
                Node::ImplDecl(_) => {
                    return Err(unsupported(
                        "impl decl emission",
                        "I closures/impls",
                        "574-582",
                    ))
                }
                Node::OverloadDecl(_) => {
                    return Err(unsupported(
                        "overload decl emission",
                        "I closures/impls",
                        "583-591",
                    ))
                }
                Node::StructDecl(_)
                | Node::EnumDecl(_)
                | Node::TraitDecl(_)
                | Node::TypeAliasDecl(_)
                | Node::MacroDecl(_)
                | Node::ImportDecl(_)
                | Node::UsingDecl(_)
                | Node::ConstStmt(_) => {}
                _ => {
                    return Err(unsupported(
                        "top-level declaration kind",
                        "B4 driver",
                        "593-598",
                    ))
                }
            }
        }

        // Step 6: escaped closures (I call point) — the subset
        // collects none; a closure literal anywhere was already
        // refused at its expression site.
        // Step 7: contract externs (call point) — none are
        // referenced by the subset's call path; extern *calls* stop
        // loudly in gen_call.

        // Step 8: encode (Zig 622-629).
        let module = self.state.builder.finish();
        encode_module(&module)
    }

    /// C1/C2 pre-scan (Zig generate lines 538–541, calling
    /// 633–934): scalar consts first, then array consts (which
    /// *emit* their hex const decls at this point, ahead of every
    /// function), then assigned bindings via the full C2 walker.
    /// Borrowed bindings are not part of this pre-scan in Zig
    /// either: they are collected per function at its entry
    /// (see `gen_func_decl`), as in the original (SAB-D23).
    fn prescan(&mut self, prog: &sla_ast::Program) -> Result<()> {
        self.state.program.global_scalar_consts =
            prescan_consts::collect_global_scalar_consts(&prog.decls);
        let check = self.check;
        let mut array_consts = std::collections::HashMap::new();
        prescan_consts::collect_global_array_consts(
            self.b(),
            &prog.decls,
            check,
            &mut array_consts,
        )?;
        self.state.program.global_array_consts = array_consts;
        self.state.program.assigned_bindings =
            prescan_bindings::collect_assigned_bindings(&prog.decls);
        Ok(())
    }

    // -- declarations ---------------------------------------------

    /// Zig `genFuncSig` + `appendDeclInst` for the subset: builds
    /// the sig (interning the lowered name first, then the scoped
    /// param names in order), appends the decl instruction, and
    /// binds the params as locals.
    ///
    /// SAB-D17 dual track, mirrored faithfully: the sig's `id` is
    /// the Zig emission counter `function_sigs.len + test_sigs.len`
    /// sampled at `genFuncSig` entry (sab_codegen.zig:5964) — NOT
    /// the interned name id the trunk's `add_sig` defaults to —
    /// while the decl instruction's operands keep the intern id
    /// (`declInstForSig`, sab_codegen.zig:6008). A test sig counts
    /// twice for later sigs: `genTestDecl` appends it to both
    /// lists (sab_codegen.zig:6480, 6493), so the counter after a
    /// test jumps by 2. The override is applied after
    /// `append_decl_inst` so the decl operands are captured with
    /// the intern id still in place.
    fn begin_sig(
        &mut self,
        lowered: &str,
        kind: FunctionKind,
        f_params: &[sla_ast::Param],
        ret_ty: &Type,
        ignored: bool,
        should_panic: bool,
    ) -> Result<usize> {
        // Zig counter at genFuncSig entry: test_sigs.len is sampled
        // before this sig exists; function_sigs.len is the index
        // `add_sig` is about to return.
        let test_count = self.state.program.test_sigs.len();
        let mut sig = FunctionSig::new(0, lowered, abi_return_prim_type(ret_ty)?, 0);
        sig.kind = kind;
        let sig_index = self.b().add_sig(sig);
        self.b().begin_function(sig_index);
        // Scoped param registers, Zig genFuncSig lines 6055-6068.
        let mut specs = Vec::with_capacity(f_params.len());
        let mut param_ids = Vec::with_capacity(f_params.len());
        for (i, p) in f_params.iter().enumerate() {
            let scoped = format!("{lowered}__param_{i}_{}", p.name);
            let param_id = self.b().intern(&scoped);
            let cap = match shared_abi_param_cap(
                p.is_borrow,
                p.is_move,
                matches!(&*p.ty, Type::Borrow(_)),
                by_value_raw_pointer_param(p),
                self.check.type_is_copy_value(&p.ty),
                is_borrow_like_type(&p.ty),
            ) {
                AbiCapKind::Borrow => CapPrefix::Borrow,
                AbiCapKind::Move => CapPrefix::Move,
                AbiCapKind::Raw => CapPrefix::Raw,
                AbiCapKind::None => CapPrefix::ByValue,
            };
            specs.push(ParamSpec {
                name: p.name.clone(),
                ty: prim_type(&p.ty)?,
                cap,
            });
            param_ids.push(param_id);
            self.b().record_reg(param_id);
            self.state.frame.push_local(Local {
                name: p.name.clone(),
                reg: param_id,
                is_param: true,
                stack_ty: None,
                ty: Some((*p.ty).clone()),
                param_cap: Some(cap),
                is_stack_alloc: false,
            });
        }
        let ret_cap = return_cap_for_type(ret_ty);
        self.b().set_sig_params(sig_index, specs, param_ids);
        self.b().set_sig_return_cap(sig_index, ret_cap);
        if kind == FunctionKind::TestFunc {
            self.b().set_sig_test_meta(sig_index, ignored, should_panic);
        }
        self.b().append_decl_inst(sig_index);
        // SAB-D17: only now, with the decl instruction's intern-id
        // operands already captured, does the sig's own id switch
        // to the Zig emission counter (function index + test count
        // sampled at entry).
        let counter_id = (sig_index + test_count) as u32;
        self.b().set_sig_id(sig_index, counter_id);
        Ok(sig_index)
    }

    /// Zig `genFuncDecl` / `genFuncDeclNamed` (lines 6183–6265),
    /// non-async path.
    fn gen_func_decl(&mut self, f: &FuncDecl) -> Result<()> {
        if f.is_async {
            return Err(unsupported(
                &format!("async function `{}` (continuation plans)", f.name),
                "H async",
                "6194-6212",
            ));
        }
        if !f.generics.is_empty() || f.operator.is_some() {
            return Err(unsupported(
                &format!("generic/operator function `{}`", f.name),
                "G generics",
                "6183",
            ));
        }
        self.state.frame.begin_function();
        self.multi_let = prescan_bindings::prepare_multi_let_bindings(&f.body);
        // C2 per-function borrowed scan (Zig genFuncDeclNamed
        // 6211–6212, immediately after prepareMultiLetBindings).
        self.state.program.borrowed_bindings = {
            let mut set = std::collections::HashSet::new();
            prescan_bindings::collect_borrowed_bindings_in_block(
                &f.body,
                &prescan_bindings::NoImportedMacroPlans,
                &mut set,
            );
            set
        };
        let lowered = self.lowered_symbol(f);
        let _sig_index = self.begin_sig(&lowered, FunctionKind::Normal, &f.params, &f.ret_ty, false, false)?;

        // Tail-expression return path (Zig 6231-6250): a non-void
        // function whose last statement is an expression statement
        // and whose block does not terminate returns that value.
        let tail_return = !is_void_type(&f.ret_ty)
            && matches!(f.body.last(), Some(Node::ExprStmt(_)))
            && !sla_lowering::block_terminates(&f.body);
        if tail_return {
            let split = f.body.len() - 1;
            for stmt in &f.body[..split] {
                self.gen_stmt(stmt)?;
                if self.b().last_is_terminator() {
                    break;
                }
            }
            if !self.b().last_is_terminator() {
                let tail = match &f.body[split] {
                    Node::ExprStmt(e) => e,
                    _ => unreachable!(),
                };
                let value = self.gen_expr(tail)?;
                self.b().emit_return(Some(value));
            }
        } else {
            self.gen_block(&f.body)?;
            if is_void_type(&f.ret_ty) && !self.b().last_is_terminator() {
                self.b().emit_return(None);
            }
        }
        self.b().finish_function();
        self.state.frame.in_function_body = false;
        Ok(())
    }

    /// Zig `genTestDecl` (lines 6472–6500): test sig (quoted name,
    /// test kind), `L_TEST_ENTRY` label, body, void return; the sig
    /// is also registered in `test_sigs`.
    fn gen_test_decl(&mut self, t: &TestDecl) -> Result<()> {
        self.state.frame.begin_function();
        self.multi_let = prescan_bindings::prepare_multi_let_bindings(&t.body);
        // C2 per-function borrowed scan (Zig genTestDecl 6476–6477).
        self.state.program.borrowed_bindings = {
            let mut set = std::collections::HashSet::new();
            prescan_bindings::collect_borrowed_bindings_in_block(
                &t.body,
                &prescan_bindings::NoImportedMacroPlans,
                &mut set,
            );
            set
        };
        let quoted = format!("\"{}\"", t.name);
        let void_ty = Type::Primitive(Primitive::VoidType);
        let sig_index =
            self.begin_sig(&quoted, FunctionKind::TestFunc, &[], &void_ty, t.is_ignored, t.should_panic)?;
        let entry = self.b().new_label("L_TEST_ENTRY");
        self.b().emit_label(&entry);
        self.gen_block(&t.body)?;
        if !self.b().last_is_terminator() {
            self.b().emit_return(None);
        }
        self.b().finish_function();
        // Zig genTestDecl also registers the sig in `test_sigs`.
        let sig = self.b().sig_clone(sig_index);
        self.state.program.test_sigs.push(sig);
        self.state.frame.in_function_body = false;
        Ok(())
    }

    /// Zig `genExternDecl` (lines 6267–6272): sig + decl only, no
    /// body; reg ids are the param ids.
    fn gen_extern_decl(&mut self, f: &FuncDecl) -> Result<()> {
        let lowered = self.lowered_symbol(f);
        let _sig_index = {
            self.state.frame.begin_function();
            let idx = self.begin_sig(&lowered, FunctionKind::External, &f.params, &f.ret_ty, false, false)?;
            idx
        };
        // Zig sets fsig.reg_ids = fsig.param_ids for externs.
        let param_ids = {
            // Recompute: params were interned by begin_sig in order.
            let mut ids = Vec::with_capacity(f.params.len());
            for (i, p) in f.params.iter().enumerate() {
                let scoped = format!("{lowered}__param_{i}_{}", p.name);
                ids.push(self.b().intern(&scoped));
            }
            ids
        };
        let b = self.b();
        for id in param_ids {
            b.record_reg(id);
        }
        b.finish_function();
        self.state.frame.in_function_body = false;
        Ok(())
    }

}

/// B4 entry point: drive `program` through the orchestration above
/// and encode the resulting module.
pub fn generate_from_program(program: &Node, check: &dyn CheckInfo) -> Result<Vec<u8>> {
    Driver::new(check).generate(program)
}

/// [`generate_from_program`] with explicit D1 surface roots.
pub fn generate_from_program_with_roots(
    program: &Node,
    check: &dyn CheckInfo,
    roots: SurfaceRoots,
) -> Result<Vec<u8>> {
    let mut driver = Driver::new(check);
    driver.surface_roots = roots;
    driver.generate(program)
}

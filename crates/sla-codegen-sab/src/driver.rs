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
//! return / break / continue, and the `panic(code)` builtin. Every
//! call point into a group this phase does not implement (C prescan
//! shapes beyond scalar consts, D std surface, F fragments, G
//! macros, H async, I closures/impls) fails loudly with
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
use crate::state::CodegenState;
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

/// Whether any node in the tree calls into the future/task runtime
/// (Zig `programUsesFutureTaskRuntime` gate for step 3). The subset
/// recognizes the call names the H group keys on; finding one is a
/// loud H-group stop, not a silent skip.
fn scan_future_use(node: &Node) -> Option<String> {
    match node {
        Node::CallExpr(call) => {
            if matches!(
                call.func_name.as_str(),
                "spawn" | "join" | "select" | "future_ready" | "future_pending"
            ) || call.associated_target.as_deref() == Some("Task")
            {
                return Some(call.func_name.clone());
            }
            call.args.iter().find_map(scan_future_use)
        }
        Node::FuncDecl(f) => f.body.iter().find_map(scan_future_use),
        Node::TestDecl(t) => t.body.iter().find_map(scan_future_use),
        Node::BinaryExpr(b) => scan_future_use(&b.left).or_else(|| scan_future_use(&b.right)),
        Node::LetStmt(l) => scan_future_use(&l.value),
        Node::ConstStmt(c) => scan_future_use(&c.value),
        Node::AssignStmt(a) => scan_future_use(&a.target).or_else(|| scan_future_use(&a.value)),
        Node::ExprStmt(e) => scan_future_use(e),
        Node::ReturnStmt(r) => r.value.as_ref().and_then(|v| scan_future_use(v)),
        Node::BlockStmt(b) => b.body.iter().find_map(scan_future_use),
        Node::WhileStmt(w) => {
            scan_future_use(&w.cond).or_else(|| w.body.iter().find_map(scan_future_use))
        }
        Node::IfExpr(i) => scan_future_use(&i.cond)
            .or_else(|| i.then_block.iter().find_map(scan_future_use))
            .or_else(|| {
                i.else_block
                    .as_ref()
                    .and_then(|b| b.iter().find_map(scan_future_use))
            }),
        Node::Program(p) => p.decls.iter().find_map(scan_future_use),
        _ => None,
    }
}

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
            funcs: HashMap::new(),
            multi_let: std::collections::HashSet::new(),
        }
    }

    pub(crate) fn b(&mut self) -> &mut crate::emit::ModuleBuilder {
        &mut self.state.builder
    }

    /// Zig `loweredFuncSymbol` (lines 953–961) for the subset:
    /// `main`, extern and no-mangle functions keep their name; other
    /// user functions are prefixed `sla__`.
    fn lowered_symbol(&self, f: &FuncDecl) -> String {
        if sla_lowering::is_main_name(&f.name) || f.is_extern || f.no_mangle {
            f.name.clone()
        } else {
            format!("sla__{}", f.name)
        }
    }

    /// Zig `bindingReg` (lines 948–951): repeated-let names get a
    /// fresh tmp register per binding site.
    pub(crate) fn binding_reg(&mut self, name: &str) -> u32 {
        if self.multi_let.contains(name) {
            self.b().new_tmp()
        } else {
            self.b().intern(name)
        }
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

        // Step 2: std surface load + preload (D1/D5 call points).
        // The subset carries no surface rules; a program that needs
        // them stops loudly at the call site that needs a rule
        // (gen_call / gen_expr), attributed to group D there.
        if prog.decls.iter().any(|d| matches!(d, Node::ImportDecl(_))) {
            // Imports name std surface files; with no D-group loader
            // wired, any *use* of an imported surface fails at its
            // call site. The decl itself emits nothing, as in Zig.
        }

        // Step 3: future helpers (H call point).
        if let Some(name) = scan_future_use(program) {
            return Err(unsupported(
                &format!("program uses future/task runtime via `{name}`; helper emission not ported"),
                "H async",
                "545-548",
            ));
        }

        // Step 4: trait vtables (I call point).
        for decl in &prog.decls {
            if let Node::ImplDecl(i) = decl {
                if i.trait_name.is_some() {
                    return Err(unsupported(
                        "trait vtable decl for impl block",
                        "I closures/impls",
                        "550-556",
                    ));
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
                        if f.is_extern {
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

    /// C1/C2 pre-scan call points (Zig 538–541 + 633–934).
    fn prescan(&mut self, prog: &sla_ast::Program) -> Result<()> {
        for decl in &prog.decls {
            match decl {
                Node::ConstStmt(c) => match &*c.value {
                    Node::Literal(_) => {
                        self.state
                            .program
                            .global_scalar_consts
                            .insert(c.name.clone(), (*c.value).clone());
                    }
                    Node::Identifier(_) => {
                        return Err(unsupported(
                            "global const alias folding (collectGlobalScalarConsts fixpoint)",
                            "C1 prescan_consts",
                            "633-720",
                        ))
                    }
                    _ => {
                        return Err(unsupported(
                            "global array/non-scalar const collection",
                            "C1 prescan_consts",
                            "721-854",
                        ))
                    }
                },
                Node::FuncDecl(f) => self.collect_assigned(&f.body),
                Node::TestDecl(t) => self.collect_assigned(&t.body),
                _ => {}
            }
        }
        Ok(())
    }

    /// C2 `collectAssignedBindings`: identifier assignment targets,
    /// recursively. Membership only — never iterated for emission.
    fn collect_assigned(&mut self, body: &[Node]) {
        for node in body {
            self.collect_assigned_node(node);
        }
    }

    fn collect_assigned_node(&mut self, node: &Node) {
        match node {
            Node::AssignStmt(a) => {
                if let Node::Identifier(name) = &*a.target {
                    self.state.program.assigned_bindings.insert(name.clone());
                }
                self.collect_assigned_node(&a.value);
            }
            Node::BlockStmt(b) => self.collect_assigned(&b.body),
            Node::WhileStmt(w) => self.collect_assigned(&w.body),
            Node::IfExpr(i) => {
                self.collect_assigned(&i.then_block);
                if let Some(e) = &i.else_block {
                    self.collect_assigned(e);
                }
            }
            Node::LetStmt(l) => self.collect_assigned_node(&l.value),
            Node::ExprStmt(e) => self.collect_assigned_node(e),
            Node::ReturnStmt(r) => {
                if let Some(v) = &r.value {
                    self.collect_assigned_node(v);
                }
            }
            _ => {}
        }
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
        self.multi_let = sla_lowering::collect_repeated_let_bindings(&f.body);
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
        self.multi_let = sla_lowering::collect_repeated_let_bindings(&t.body);
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

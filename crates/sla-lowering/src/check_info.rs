//! The checked-program boundary between the checker lane and the
//! backends. In the Zig original, codegen reads the type checker's
//! tables directly (`tc.expr_types`, `tc.cleanups`, `tc.phi_cleanups`,
//! `tc.funcs`, `tc.extern_funcs`, `tc.macros`, alias resolution and the
//! Copy-ness oracles). The workspace verdict says backends consume
//! upstream data only through sla-lowering, so that surface is defined
//! here as the [`CheckInfo`] trait; the checker lane (or a test
//! harness) provides an implementation.
//!
//! All node-keyed tables use [`crate::node_key`] (node address), the
//! same key the checker lane computes. Cleanup lists are consumed in
//! the exact order the checker produced them — codegen never reorders.

use std::collections::{HashMap, HashSet};

use sla_ast::{FuncDecl, MacroDecl, Node, StructDecl, Type};

use crate::abi::type_is_copy_value_leaf;
use crate::node_key;

/// One contract-extern parameter as a backend needs it (Zig
/// `tc.extern_funcs` param entries): the ABI type spelling plus the
/// capability the contract declared for the argument (`&name` =
/// borrow, `^name` = move, `*name` = raw — the SA-text call site
/// carries the `*` capability prefix, Zig `abiCallArgPrefix`
/// `.raw`; anything else = by value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternParamView {
    pub name: String,
    pub abi_ty: String,
    pub is_borrow: bool,
    pub is_move: bool,
    pub is_raw: bool,
}

/// A contract extern's full signature as a backend needs it (Zig
/// `tc.extern_funcs` values): parameter views, the ABI return
/// spelling (payload spelling for a fallible extern — the contract
/// parser strips the trailing `!`), the fallible flag and the
/// C-variadic flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternSigView {
    pub params: Vec<ExternParamView>,
    pub ret_abi: String,
    pub fallible: bool,
    pub is_variadic: bool,
}

/// One imported SA-text macro as a backend needs it (Zig
/// `tc.imported_macros` values): the macro's arity and leading
/// output count from its `[MACRO]` declaration in the imported
/// `.sa` file, the file it came from, and the per-parameter
/// borrow / address-slot masks the declaration analysis computed.
/// The registry is generated from the imported files' own
/// declarations and merged per package — the SAB registry
/// federation shape — so a backend that consumes this view never
/// hardcodes a macro's home file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedMacroView {
    pub arity: usize,
    pub leading_outputs: usize,
    pub import_path: Option<String>,
    pub borrowed_arg_mask: u64,
    pub address_slot_arg_mask: u64,
    /// Zig `ImportedMacroCallPlan.assigned_output_mask` (see
    /// `ImportedMacroInfo`): bit `i` set when the macro body's
    /// first reference to parameter `i` is its assignment.
    pub assigned_output_mask: u64,
    pub direct_callees: Vec<String>,
}

impl ImportedMacroView {
    /// Zig `ImportedMacroCallPlan.callArgIsAssignedOutput`
    /// (`lowering_rules.zig:612`) for statement position, where
    /// `macroParamIndexForCallArg` is the identity (no
    /// expression-output shift). The expression-position caller
    /// never asks: its leading output is a fresh result register.
    pub fn call_arg_is_assigned_output(&self, call_arg_index: usize) -> bool {
        if call_arg_index >= 64 {
            return false;
        }
        (self.assigned_output_mask & (1u64 << call_arg_index)) != 0
    }
}

/// Zig `importedMacroDirectCallConsumesValueArg`
/// (lowering_rules.zig:734): imported macros whose body moves
/// (`^`) a by-value argument — the caller's binding is consumed
/// by the expansion (the verifier sees the `^` inside the macro
/// body, e.g. `FS_READ_BUFFER_FREE` expands to
/// `call @sa_fs_read_buffer_free(^%buffer)`), so no scope-end
/// release may be emitted for it afterwards. Shared by the
/// SA-text emitter (compiler-side consume marking) and the SAB
/// direct emitter (release suppression, sla-codegen-sab
/// macro_plan.rs) so both backends agree on these macro
/// surfaces.
pub fn imported_macro_direct_call_consumes_value_arg(
    macro_name: &str,
    call_arg_index: usize,
) -> bool {
    (macro_name == "FS_READ_BUFFER_FREE" || macro_name == "SLA_FS_BUFFER_FREE")
        && call_arg_index == 0
}

/// Read-only view of everything the text emitter needs from checking.
pub trait CheckInfo {
    /// `tc.expr_types` (and codegen's `resolvedTypeForExpr`, which
    /// thins to it for every shape the slice produces).
    fn expr_type(&self, node: &Node) -> Option<&Type>;

    /// `tc.cleanups`: names to release at a statement's exit point, in
    /// checker-computed order.
    fn cleanups(&self, stmt: &Node) -> Option<&[String]>;

    /// `tc.phi_cleanups`: raw `!name` releases printed after a branch
    /// block whose last statement is the key.
    fn phi_cleanups(&self, stmt: &Node) -> Option<&[String]>;

    /// `tc.await_cleanups` (stage 6 async cluster): bindings to
    /// release before an async function returns its pending future
    /// at an await point, keyed by the await expression node. The
    /// default keeps lanes without the recording compiling.
    fn await_cleanups(&self, _expr: &Node) -> Option<&[String]> {
        None
    }

    /// `tc.match_arm_exit_cleanups` (stage 5b checker-plans):
    /// the checker's per-arm pattern-binding exit cleanups for
    /// `expr` (a `MatchExpr` node), arm `arm` in arm order. `None`
    /// when the checker recorded nothing for this match (a lane
    /// without the recording, or a harness `MapCheckInfo` with no
    /// arm data added); `Some(&[])` when the arm's bindings all
    /// left consumed. The emitter prints the list like phi
    /// cleanups and drops its arm-exit approximation.
    fn match_arm_exit_cleanups(&self, _expr: &Node, _arm: usize) -> Option<&[String]> {
        None
    }

    /// `tc.select_arm_exit_cleanups` (goroutine batch): the
    /// checker's per-arm pattern-binding exit cleanups for `stmt`
    /// (a `SelectStmt` node), indexed by arm position in the
    /// statement's arm list, with the timeout arm at index
    /// `arms.len()`. Same contract as `match_arm_exit_cleanups`:
    /// `None` when the checker recorded nothing for this select;
    /// the emitter prints the list at the arm's fallthrough exit.
    fn select_arm_exit_cleanups(&self, _stmt: &Node, _arm: usize) -> Option<&[String]> {
        None
    }

    /// `tc.funcs`: function declarations by (unmangled) name.
    fn func(&self, _name: &str) -> Option<&FuncDecl> {
        None
    }

    /// `tc.extern_funcs` membership.
    fn is_extern_func(&self, _name: &str) -> bool {
        false
    }

    /// `tc.imported_macros` entry for an imported SA-text macro.
    fn imported_macro(&self, _name: &str) -> Option<ImportedMacroView> {
        None
    }

    /// `tc.extern_funcs` full signature for a contract extern, when
    /// the lane registered exactly one. Backends that emit extern
    /// declarations / calls consume this; the default keeps lanes
    /// that only track membership compiling.
    fn extern_sig(&self, _name: &str) -> Option<ExternSigView> {
        None
    }

    /// `tc.macros` / imported-macro presence by name.
    fn is_macro(&self, _name: &str) -> bool {
        false
    }

    /// `tc.macros`: the user-macro declaration (params, declared
    /// parameter types, body) the text backend's inline expansion
    /// consumes (Zig codegen reads `tc.macros` directly).
    fn macro_decl(&self, _name: &str) -> Option<&MacroDecl> {
        None
    }

    /// `tc.macro_call_try_cleanups`: caller-side bindings to
    /// release when a `?` inside the macro body propagates out of
    /// this call, keyed by the call node.
    fn macro_call_try_cleanups(&self, _call: &Node) -> Option<&[String]> {
        None
    }

    /// `tc.array_to_slice_borrow_args`: the checker accepted this
    /// call argument by the borrow-array -> slice coercion, so the
    /// emitter must materialize a slice view of the array
    /// (stack_alloc Slice_SIZE + SLICE_NEW) instead of passing the
    /// raw array borrow.
    fn call_arg_array_to_slice_borrow(&self, _arg: &Node) -> bool {
        false
    }

    /// `tc.dyn_rc_coercions` / `tc.dyn_box_coercions`: the checker
    /// accepted this value node through a smart-pointer dyn
    /// coercion at a let (declared `Rc<__dyn_T>` / `Box<__dyn_T>`
    /// against an `Rc<X>` / `Box<X>` value with X: T), yielding
    /// the trait name. The text emitter must materialise the
    /// trait-object fat cell the wrapper holds instead of passing
    /// the raw payload to the `*_NEW` macro (F-DYN L3).
    fn dyn_smart_coercion(&self, _expr: &Node) -> Option<&str> {
        None
    }

    /// `tc.expr_types` by precomputed node key (the macro-arg
    /// table stores arg nodes by key, mirroring Zig's
    /// pointer-keyed `macro_arg_exprs`).
    fn expr_type_by_key(&self, _key: usize) -> Option<&Type> {
        None
    }

    /// Zig `lowering_rules.macroBodyConsumesParam` over the named
    /// macro's body: true when the body consumes the parameter's
    /// value (the temp-arg retirement in the inline expansion
    /// skips parameters the body already consumed). On checked
    /// programs this equals the checker's sequence consumption
    /// being Always (conditional consumption is a checker error).
    fn macro_body_consumes_param(&self, _macro_name: &str, _param: &str) -> bool {
        false
    }

    /// `tc.resolveFunctionAlias`.
    fn resolve_alias(&self, name: &str) -> String {
        name.to_string()
    }

    /// `tc.resolved_call_symbols`: monomorphized / operator-resolved
    /// call target for an expression node, when the checker recorded
    /// one (drives the resolved-static-call lowering).
    fn resolved_call_symbol(&self, _expr: &Node) -> Option<&str> {
        None
    }

    /// The mangled SA symbol of the overload-set member the
    /// checker resolved this call to, when the call targets a
    /// plain-function overload set (SPRINT-100 capability). The
    /// text emitter emits every member under
    /// `sla_ast::overload_member_symbol` and binds the call here.
    fn overload_call_symbol(&self, _expr: &Node) -> Option<&str> {
        None
    }

    /// The checker's Copy oracle (`typeIsCopyValue`): primitives and
    /// fn-pointers are Copy; structs are Copy when every field is.
    fn type_is_copy_value(&self, ty: &Type) -> bool;

    /// `typeIsCopyStruct`: a user-defined type that is Copy.
    fn type_is_copy_struct(&self, ty: &Type) -> bool {
        matches!(ty, Type::UserDefined(_)) && self.type_is_copy_value(ty)
    }

    /// `typeIsShallowCopyCallArgValue` (drives assigned-value slots).
    fn type_is_shallow_copy_call_arg_value(&self, _ty: &Type) -> bool {
        false
    }

    /// `tc.structs`: struct declarations by name.
    fn struct_decl(&self, _name: &str) -> Option<&StructDecl> {
        None
    }

    /// `tc.traits`: a trait's declaration shape as the backends
    /// need it — supertrait names and declared method signatures
    /// in declaration order (drives trait vtable emission and
    /// dyn-method slot resolution).
    fn trait_decl(&self, _name: &str) -> Option<TraitDeclView> {
        None
    }

    /// The checker's trait-implementation oracle
    /// (`typeImplementsTrait`): whether the concrete type was
    /// registered as implementing the trait (directly or through
    /// a supertrait chain). Drives dyn-borrow coercion decisions
    /// in the text emitter, which recompute the checker's own
    /// acceptance predicate (the Rust checker keeps no
    /// `dyn_borrow_args` side table, D-C21).
    fn type_implements_trait(&self, _ty: &Type, _trait_name: &str) -> bool {
        false
    }

    /// The checker's supertrait oracle (`traitExtendsTrait`):
    /// whether the first trait extends the second, directly or
    /// transitively (a trait extends itself).
    fn trait_extends_trait(&self, _sub: &str, _super_trait: &str) -> bool {
        false
    }
}

/// One trait's declaration as a backend needs it (Zig `tc.traits`
/// values): supertrait names in declaration order and the trait's
/// own method signatures in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitDeclView {
    pub supertraits: Vec<String>,
    pub methods: Vec<TraitMethodView>,
}

/// One trait method signature (Zig `ast.TraitMethod` projected to
/// what emission needs): the method name, its declared parameters
/// (the receiver parameter first) and the return type.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitMethodView {
    pub name: String,
    pub params: Vec<sla_ast::Param>,
    pub ret_ty: Type,
}

/// Map-backed [`CheckInfo`] for harnesses and tests. The production
/// wiring (checker lane's TypeChecker) can implement the trait over
/// its own tables instead; this struct exists so the slice can be
/// exercised end-to-end before that wiring lands.
#[derive(Debug, Default)]
pub struct MapCheckInfo {
    pub expr_types: HashMap<usize, Type>,
    pub cleanups: HashMap<usize, Vec<String>>,
    pub phi_cleanups: HashMap<usize, Vec<String>>,
    /// Stage 6 async cluster: await node key -> pending-return
    /// cleanup names (`tc.await_cleanups`).
    pub await_cleanups: HashMap<usize, Vec<String>>,
    /// Stage 5b checker-plans: match node key -> one exit-cleanup
    /// list per arm, in arm order.
    pub match_arm_exit_cleanups: HashMap<usize, Vec<Vec<String>>>,
    /// Goroutine batch: select node key -> one exit-cleanup list
    /// per arm (arm order; timeout arm at index `arms.len()`).
    pub select_arm_exit_cleanups: HashMap<usize, Vec<Vec<String>>>,
    pub funcs: HashMap<String, FuncDecl>,
    pub extern_funcs: HashSet<String>,
    pub extern_sigs: HashMap<String, ExternSigView>,
    pub imported_macros: HashMap<String, ImportedMacroView>,
    pub macros: HashSet<String>,
    pub macro_decls: HashMap<String, MacroDecl>,
    pub macro_call_try_cleanups: HashMap<usize, Vec<String>>,
    pub array_to_slice_borrow_args: std::collections::HashSet<usize>,
    pub aliases: HashMap<String, String>,
    pub resolved_symbols: HashMap<usize, String>,
    /// Struct names whose values are Copy even though the field walk
    /// is unavailable (e.g. imported Copy structs).
    pub copy_structs: HashSet<String>,
    pub structs: HashMap<String, StructDecl>,
}

impl MapCheckInfo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_expr_type(&mut self, node: &Node, ty: Type) -> &mut Self {
        self.expr_types.insert(node_key(node), ty);
        self
    }

    pub fn add_cleanups(&mut self, stmt: &Node, names: Vec<String>) -> &mut Self {
        self.cleanups.insert(node_key(stmt), names);
        self
    }

    pub fn add_phi_cleanups(&mut self, stmt: &Node, names: Vec<String>) -> &mut Self {
        self.phi_cleanups.insert(node_key(stmt), names);
        self
    }

    pub fn add_await_cleanups(&mut self, expr: &Node, names: Vec<String>) -> &mut Self {
        self.await_cleanups.insert(node_key(expr), names);
        self
    }

    pub fn add_match_arm_exit_cleanups(
        &mut self,
        expr: &Node,
        names: Vec<Vec<String>>,
    ) -> &mut Self {
        self.match_arm_exit_cleanups.insert(node_key(expr), names);
        self
    }

    pub fn add_select_arm_exit_cleanups(
        &mut self,
        stmt: &Node,
        names: Vec<Vec<String>>,
    ) -> &mut Self {
        self.select_arm_exit_cleanups.insert(node_key(stmt), names);
        self
    }

    pub fn add_func(&mut self, func: FuncDecl) -> &mut Self {
        self.funcs.insert(func.name.clone(), func);
        self
    }

    pub fn add_extern_func(&mut self, name: &str) -> &mut Self {
        self.extern_funcs.insert(name.to_string());
        self
    }

    pub fn add_imported_macro(&mut self, name: &str, view: ImportedMacroView) -> &mut Self {
        self.imported_macros.insert(name.to_string(), view);
        self
    }

    pub fn add_extern_sig(&mut self, name: &str, sig: ExternSigView) -> &mut Self {
        self.extern_funcs.insert(name.to_string());
        self.extern_sigs.insert(name.to_string(), sig);
        self
    }

    pub fn add_macro(&mut self, name: &str) -> &mut Self {
        self.macros.insert(name.to_string());
        self
    }

    pub fn add_macro_decl(&mut self, decl: MacroDecl) -> &mut Self {
        self.macros.insert(decl.name.clone());
        self.macro_decls.insert(decl.name.clone(), decl);
        self
    }

    pub fn add_macro_call_try_cleanups(
        &mut self,
        call: &Node,
        names: Vec<String>,
    ) -> &mut Self {
        self.macro_call_try_cleanups.insert(node_key(call), names);
        self
    }

    pub fn add_array_to_slice_borrow_arg(&mut self, arg: &Node) -> &mut Self {
        self.array_to_slice_borrow_args.insert(node_key(arg));
        self
    }

    pub fn add_copy_struct(&mut self, name: &str) -> &mut Self {
        self.copy_structs.insert(name.to_string());
        self
    }

    pub fn add_struct(&mut self, decl: StructDecl) -> &mut Self {
        self.structs.insert(decl.name.clone(), decl);
        self
    }

    fn copy_value_walk(&self, ty: &Type, depth: usize) -> bool {
        if let Some(leaf) = type_is_copy_value_leaf(ty) {
            return leaf;
        }
        if depth > 16 {
            return false;
        }
        match ty {
            Type::Tuple(tuple) => tuple
                .elems
                .iter()
                .all(|elem| self.copy_value_walk(elem, depth + 1)),
            Type::Array(array) => self.copy_value_walk(&array.elem, depth + 1),
            Type::UserDefined(ud) => {
                if self.copy_structs.contains(&ud.name) {
                    return true;
                }
                match self.structs.get(&ud.name) {
                    Some(decl) => decl
                        .fields
                        .iter()
                        .all(|field| self.copy_value_walk(&field.ty, depth + 1)),
                    None => false,
                }
            }
            _ => false,
        }
    }
}

impl CheckInfo for MapCheckInfo {
    fn expr_type(&self, node: &Node) -> Option<&Type> {
        self.expr_types.get(&node_key(node))
    }

    fn cleanups(&self, stmt: &Node) -> Option<&[String]> {
        self.cleanups.get(&node_key(stmt)).map(|v| v.as_slice())
    }

    fn phi_cleanups(&self, stmt: &Node) -> Option<&[String]> {
        self.phi_cleanups.get(&node_key(stmt)).map(|v| v.as_slice())
    }

    fn await_cleanups(&self, expr: &Node) -> Option<&[String]> {
        self.await_cleanups.get(&node_key(expr)).map(|v| v.as_slice())
    }

    fn match_arm_exit_cleanups(&self, expr: &Node, arm: usize) -> Option<&[String]> {
        self.match_arm_exit_cleanups
            .get(&node_key(expr))?
            .get(arm)
            .map(|v| v.as_slice())
    }

    fn select_arm_exit_cleanups(&self, stmt: &Node, arm: usize) -> Option<&[String]> {
        self.select_arm_exit_cleanups
            .get(&node_key(stmt))?
            .get(arm)
            .map(|v| v.as_slice())
    }

    fn func(&self, name: &str) -> Option<&FuncDecl> {
        self.funcs.get(name)
    }

    fn is_extern_func(&self, name: &str) -> bool {
        self.extern_funcs.contains(name) || self.extern_sigs.contains_key(name)
    }

    fn extern_sig(&self, name: &str) -> Option<ExternSigView> {
        self.extern_sigs.get(name).cloned()
    }

    fn imported_macro(&self, name: &str) -> Option<ImportedMacroView> {
        self.imported_macros.get(name).cloned()
    }

    fn is_macro(&self, name: &str) -> bool {
        self.macros.contains(name)
    }

    fn macro_decl(&self, name: &str) -> Option<&MacroDecl> {
        self.macro_decls.get(name)
    }

    fn macro_call_try_cleanups(&self, call: &Node) -> Option<&[String]> {
        self.macro_call_try_cleanups
            .get(&node_key(call))
            .map(Vec::as_slice)
    }

    fn call_arg_array_to_slice_borrow(&self, arg: &Node) -> bool {
        self.array_to_slice_borrow_args.contains(&node_key(arg))
    }

    fn expr_type_by_key(&self, key: usize) -> Option<&Type> {
        self.expr_types.get(&key)
    }

    fn resolve_alias(&self, name: &str) -> String {
        self.aliases
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    fn resolved_call_symbol(&self, expr: &Node) -> Option<&str> {
        self.resolved_symbols.get(&node_key(expr)).map(|s| s.as_str())
    }

    fn type_is_copy_value(&self, ty: &Type) -> bool {
        self.copy_value_walk(ty, 0)
    }

    fn struct_decl(&self, name: &str) -> Option<&StructDecl> {
        self.structs.get(name)
    }
}

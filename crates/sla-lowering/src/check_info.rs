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

use sla_ast::{FuncDecl, Node, StructDecl, Type};

use crate::abi::type_is_copy_value_leaf;
use crate::node_key;

/// One contract-extern parameter as a backend needs it (Zig
/// `tc.extern_funcs` param entries): the ABI type spelling plus the
/// capability the contract declared for the argument (`&name` =
/// borrow, `^name` = move, anything else = by value; a raw `*name`
/// pointer parameter is by-value at the call site).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternParamView {
    pub name: String,
    pub abi_ty: String,
    pub is_borrow: bool,
    pub is_move: bool,
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
    pub direct_callees: Vec<String>,
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
    pub funcs: HashMap<String, FuncDecl>,
    pub extern_funcs: HashSet<String>,
    pub extern_sigs: HashMap<String, ExternSigView>,
    pub imported_macros: HashMap<String, ImportedMacroView>,
    pub macros: HashSet<String>,
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

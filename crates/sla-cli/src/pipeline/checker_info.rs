//! CheckInfo adapter (same wiring shape as sla-oracle's): the
//! checked-program boundary the two codegen tails consume.

use sla_ast::node::Node;
use sla_checker::Checker;

pub(crate) struct CheckerInfo<'c> {
    checker: &'c Checker,
    /// `tc.structs` projected back to AST decls. The checker's
    /// `StructInfo` is a lossless projection of the AST
    /// `StructDecl` (fields / derives / generics / is_union /
    /// is_opaque), so this conversion restores exactly the table
    /// Zig's codegen reads from `tc.structs` directly. While the
    /// adapter left `struct_decl` at the trait default (`None`),
    /// the SAB driver's `structDeclForType` saw an empty table
    /// and every struct literal reached in a directly generated
    /// position stopped with "struct literal type has no struct
    /// decl" (the structlit-39 wall).
    decls: std::collections::HashMap<String, sla_ast::StructDecl>,
}

impl<'c> CheckerInfo<'c> {
    pub(crate) fn new(checker: &'c Checker) -> Self {
        let decls = checker
            .structs
            .iter()
            .map(|(name, info)| {
                (
                    name.clone(),
                    sla_ast::StructDecl {
                        name: name.clone(),
                        derives: info.derives.clone(),
                        generics: info.generics.clone(),
                        fields: info
                            .fields
                            .iter()
                            .map(|(field_name, field_ty)| sla_ast::Field {
                                name: field_name.clone(),
                                ty: Box::new(field_ty.clone()),
                            })
                            .collect(),
                        is_union: info.is_union,
                        is_opaque: info.is_opaque,
                    },
                )
            })
            .collect();
        CheckerInfo { checker, decls }
    }
}

impl sla_lowering::CheckInfo for CheckerInfo<'_> {
    fn expr_type(&self, node: &Node) -> Option<&sla_ast::Type> {
        self.checker.expr_types.get(&sla_lowering::node_key(node))
    }
    fn overload_call_symbol(&self, node: &Node) -> Option<&str> {
        self.checker
            .overload_call_symbols
            .get(&sla_lowering::node_key(node))
            .map(|s| s.as_str())
    }
    fn cleanups(&self, stmt: &Node) -> Option<&[String]> {
        self.checker
            .cleanups
            .get(&sla_lowering::node_key(stmt))
            .map(Vec::as_slice)
    }
    fn phi_cleanups(&self, stmt: &Node) -> Option<&[String]> {
        self.checker
            .phi_cleanups
            .get(&sla_lowering::node_key(stmt))
            .map(Vec::as_slice)
    }
    fn await_cleanups(&self, expr: &Node) -> Option<&[String]> {
        // Stage 6 async cluster: `tc.await_cleanups`, keyed by the
        // await node's address like every other node-keyed table.
        self.checker
            .await_cleanups
            .get(&sla_lowering::node_key(expr))
            .map(Vec::as_slice)
    }
    fn match_arm_exit_cleanups(&self, expr: &Node, arm: usize) -> Option<&[String]> {
        // Stage 5b checker-plans: the checker keys the table by the
        // match node's own address, which is the same tree the
        // emitter reads, so the key lines up exactly.
        self.checker
            .match_arm_exit_cleanups
            .get(&sla_lowering::node_key(expr))?
            .get(arm)
            .map(Vec::as_slice)
    }
    fn select_arm_exit_cleanups(&self, stmt: &Node, arm: usize) -> Option<&[String]> {
        // Goroutine batch: same keying as the match-arm table --
        // the checker keys by the select node's own address.
        self.checker
            .select_arm_exit_cleanups
            .get(&sla_lowering::node_key(stmt))?
            .get(arm)
            .map(Vec::as_slice)
    }
    fn type_is_copy_value(&self, ty: &sla_ast::Type) -> bool {
        self.checker.type_is_copy(ty)
    }
    fn is_extern_func(&self, name: &str) -> bool {
        self.checker.extern_funcs.contains_key(name)
    }
    fn extern_sig(&self, name: &str) -> Option<sla_lowering::ExternSigView> {
        // `tc.extern_funcs` projection: the checker registers one
        // signature per contract name (R-6 dedupes identical
        // duplicates; overload promotion keeps the first-registered
        // candidate in this map). The parameter register names drop
        // a raw-pointer `*` prefix, matching the SAB signature's
        // reg-id convention (Zig `makeContractExternSig` reg_ids).
        let sig = self.checker.extern_funcs.get(name)?;
        Some(sla_lowering::ExternSigView {
            params: sig
                .params
                .iter()
                .map(|p| sla_lowering::ExternParamView {
                    name: p.name.trim_start_matches('*').to_string(),
                    abi_ty: p.ty.clone(),
                    is_borrow: p.is_borrow,
                    is_move: p.is_move,
                    is_raw: p.is_raw,
                })
                .collect(),
            ret_abi: sig.ret_abi.clone(),
            fallible: sig.fallible,
            is_variadic: sig.is_variadic,
        })
    }
    fn is_macro(&self, name: &str) -> bool {
        self.checker.macros.contains_key(name)
    }
    fn macro_decl(&self, name: &str) -> Option<&sla_ast::MacroDecl> {
        self.checker.macros.get(name)
    }
    fn macro_call_try_cleanups(&self, call: &Node) -> Option<&[String]> {
        self.checker
            .macro_call_try_cleanups
            .get(&sla_lowering::node_key(call))
            .map(Vec::as_slice)
    }
    fn call_arg_array_to_slice_borrow(&self, arg: &Node) -> bool {
        self.checker.is_array_to_slice_borrow_arg(arg)
    }
    fn dyn_smart_coercion(&self, expr: &Node) -> Option<&str> {
        // F-DYN L3: the checker keys both coercion tables by the
        // let value node's address, the same keying as expr_types.
        let key = sla_lowering::node_key(expr);
        self.checker
            .dyn_rc_coercions
            .get(&key)
            .or_else(|| self.checker.dyn_box_coercions.get(&key))
            .map(|s| s.as_str())
    }
    fn expr_type_by_key(&self, key: usize) -> Option<&sla_ast::Type> {
        self.checker.expr_types.get(&key)
    }
    fn macro_body_consumes_param(&self, macro_name: &str, param: &str) -> bool {
        let Some(mac) = self.checker.macros.get(macro_name) else {
            return false;
        };
        sla_checker::rules::macro_param_consumption(&mac.body, param)
            == sla_checker::rules::MacroParamConsumption::Always
    }
    fn imported_macro(&self, name: &str) -> Option<sla_lowering::ImportedMacroView> {
        // `tc.imported_macros` projection (the registry the
        // contract loader generated from each imported `.sa`
        // file's own `[MACRO]` declarations).
        let info = self.checker.imported_macros.get(name)?;
        Some(sla_lowering::ImportedMacroView {
            arity: info.arity,
            leading_outputs: info.leading_outputs,
            import_path: info.import_path.clone(),
            borrowed_arg_mask: info.borrowed_arg_mask,
            address_slot_arg_mask: info.address_slot_arg_mask,
            assigned_output_mask: info.assigned_output_mask,
            direct_callees: info.direct_callees.clone(),
        })
    }
    fn struct_decl(&self, name: &str) -> Option<&sla_ast::StructDecl> {
        self.decls.get(name)
    }
    fn trait_decl(&self, name: &str) -> Option<sla_lowering::TraitDeclView> {
        // `tc.traits` projected to the backend view: supertrait
        // names and method signatures in declaration order.
        let info = self.checker.traits.get(name)?;
        Some(sla_lowering::TraitDeclView {
            supertraits: info.supertraits.clone(),
            methods: info
                .methods
                .iter()
                .map(|m| sla_lowering::TraitMethodView {
                    name: m.name.clone(),
                    params: m
                        .params
                        .iter()
                        .map(|p| sla_ast::Param {
                            name: p.name.clone(),
                            ty: Box::new(p.ty.clone()),
                            is_borrow: p.is_borrow,
                            is_move: p.is_move,
                            fallible_sugar: false,
                            is_var: p.is_var,
                        })
                        .collect(),
                    ret_ty: m.ret_ty.clone(),
                })
                .collect(),
        })
    }
    fn type_implements_trait(&self, ty: &sla_ast::Type, trait_name: &str) -> bool {
        self.checker.type_implements_trait(ty, trait_name)
    }
    fn trait_extends_trait(&self, sub: &str, super_trait: &str) -> bool {
        self.checker.trait_extends_trait(sub, super_trait)
    }
}

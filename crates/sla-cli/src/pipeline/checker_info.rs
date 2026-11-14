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
            direct_callees: info.direct_callees.clone(),
        })
    }
    fn struct_decl(&self, name: &str) -> Option<&sla_ast::StructDecl> {
        self.decls.get(name)
    }
}

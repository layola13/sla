//! Decl-only (extern) function declaration emission, mirrored
//! from codegen.zig `emitExternDecl` (3062) and its call site in
//! the vtable pass (10023): a decl-only function emits
//! `@extern <lowered>(<prefix><name>: <abi type>, ...) -> <ret>`
//! unless a concrete function with the same lowered symbol
//! exists in the program. Before this port, any program carrying
//! an `@extern` declaration (the FFI staple shape, e.g.
//! test_unit_ptr_out_param) failed the whole emission with
//! "unsupported emission `extern (decl-only) function
//! declaration`".

use sla_ast::{FuncDecl, Node, Type};
use sla_lowering::{
    abi_param_type_string, abi_return_type_string, by_value_raw_pointer_param,
    is_borrow_like_type, is_void_type, shared_abi_param_prefix,
};

use crate::codegen::Codegen;

impl<'c> Codegen<'c> {
    /// codegen.zig:3082 `hasConcreteFunctionSymbol`.
    pub(crate) fn has_concrete_function_symbol(
        &self,
        decls: &[Node],
        decl_only: &FuncDecl,
    ) -> bool {
        let want = self.lowered_func_symbol(&decl_only.name);
        decls.iter().any(|d| {
            matches!(d, Node::FuncDecl(f)
                if !f.is_decl_only && self.lowered_func_symbol(&f.name) == want)
        })
    }

    /// codegen.zig:3062 `emitExternDecl`. The parameter prefix
    /// follows the same shared-ABI computation as the function
    /// signature emission in func.rs.
    pub(crate) fn emit_extern_decl(&mut self, f: &FuncDecl) {
        let lowered = self.lowered_func_symbol(&f.name);
        let mut line = format!("@extern {lowered}(");
        for (i, p) in f.params.iter().enumerate() {
            if i > 0 {
                line.push_str(", ");
            }
            let prefix = shared_abi_param_prefix(
                p.is_borrow,
                p.is_move,
                matches!(&*p.ty, Type::Borrow(_)),
                by_value_raw_pointer_param(p),
                self.type_is_copy_value(&p.ty),
                is_borrow_like_type(&p.ty),
            );
            line.push_str(&format!(
                "{prefix}{}: {}",
                p.name,
                abi_param_type_string(p.is_borrow, p.is_move, &p.ty)
            ));
        }
        if f.is_variadic {
            line.push_str(", ...");
        }
        if is_void_type(&f.ret_ty) {
            line.push_str(") -> void");
        } else {
            line.push_str(&format!(
                ") -> {}",
                abi_return_type_string(&f.ret_ty)
            ));
        }
        self.wl(&line);
    }
}

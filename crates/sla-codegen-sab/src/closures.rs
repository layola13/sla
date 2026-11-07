//! I group — closures and impl/overload declaration dispatch.
//!
//! Faithful port of the current-head Zig (reference tree
//! /content/sa_all/sa_plugin_sla @ 2cc12ea8, sab_codegen.zig):
//! - `closureLiteralFromExpr` (4009): a closure literal, possibly
//!   under a `^` move wrapper;
//! - the inline-closure family (19351–19412):
//!   `genInlineClosureUnary`, `genInlineClosureNullary`,
//!   `genInlineClosureBinary`, `genClosureCall`, with
//!   `restoreClosureParams` (19340) — closure parameters are
//!   register substitutions in `closure_param_regs`, saved and
//!   restored around the inlined body (restored in reverse order,
//!   as in Zig; the restore's `catch {}` is SAB-D05's family and
//!   has no Rust counterpart because the map cannot fail);
//! - the `genLet` closure-binding arms (8156–8180): a let-bound
//!   closure literal registers in `closure_bindings` and
//!   materializes as `dst = 0`; a plain alias of a closure binding
//!   registers under the new name (FS-9);
//! - `genMove` (14283) with lowering_rules'
//!   `moveExprConsumesSource` (1309);
//! - `genImplDecl` / `genOverloadDecl` (7314/7330): methods are
//!   emitted through the named function path under their mangled
//!   names.
//!
//! Owned-tree discipline (D-A1): closure values are never
//! materialized as shared handles — a binding is a name →
//! `ClosureLiteral` AST registration plus a zero-valued register,
//! exactly the Zig shape; no `Rc` is introduced anywhere in this
//! group. Escaped (thread-spawn) closures are *not* ported: their
//! entry creation (Zig 14523), capture collection (E6, Zig
//! 4159–4370) and wrapper/worker emission (Zig 6700–6800) belong
//! to the thread surface and stay loudly unported — see the
//! I-group REPORT for the death-point coordinates.

use sla_ast::{CallExpr, ClosureLiteral, ImplDecl, MoveExpr, Node, OverloadDecl, Type};
use sla_lowering::is_borrow_like_type;

use crate::driver::Driver;
use crate::error::{Error, Result};
use crate::symbols;
use crate::types::Local;

/// Zig `closureLiteralFromExpr` (4009).
pub(crate) fn closure_literal_from_expr(expr: &Node) -> Option<&ClosureLiteral> {
    match expr {
        Node::ClosureLiteral(lit) => Some(lit),
        Node::MoveExpr(mv) => closure_literal_from_expr(&mv.expr),
        _ => None,
    }
}

/// Zig `lowering_rules.moveExprConsumesSource` (1309).
fn move_expr_consumes_source(inner_ty: Option<&Type>, value_is_copy: bool) -> bool {
    let Some(ty) = inner_ty else {
        return true;
    };
    if value_is_copy {
        return false;
    }
    if is_borrow_like_type(ty) {
        return false;
    }
    true
}

/// One saved closure-param mapping (Zig `SavedClosureParam`):
/// the register a param name resolved to before inlining, if any.
struct SavedClosureParam {
    name: String,
    old: Option<u32>,
}

impl<'c> Driver<'c> {
    /// Zig `restoreClosureParams` (19340): restore in reverse
    /// order; a param with no previous mapping is removed.
    fn restore_closure_params(&mut self, saved: &[SavedClosureParam]) {
        for item in saved.iter().rev() {
            match item.old {
                Some(old) => {
                    self.state
                        .program
                        .closure_param_regs
                        .insert(item.name.clone(), old);
                }
                None => {
                    self.state.program.closure_param_regs.remove(&item.name);
                }
            }
        }
    }

    fn bind_closure_params(
        &mut self,
        closure: &ClosureLiteral,
        arg_regs: &[u32],
    ) -> Vec<SavedClosureParam> {
        let mut saved = Vec::with_capacity(closure.params.len());
        for (param, &arg_reg) in closure.params.iter().zip(arg_regs.iter()) {
            saved.push(SavedClosureParam {
                name: param.name.clone(),
                old: self.state.program.closure_param_regs.get(&param.name).copied(),
            });
            self.state
                .program
                .closure_param_regs
                .insert(param.name.clone(), arg_reg);
        }
        saved
    }

    /// Zig `genInlineClosureUnary` (19351).
    pub(crate) fn gen_inline_closure_unary(
        &mut self,
        closure: &ClosureLiteral,
        arg_reg: u32,
    ) -> Result<u32> {
        if closure.params.len() != 1 {
            return Err(Error::UnsupportedSabDirectFeature(
                "unary inline closure with a non-1 parameter count".to_string(),
            ));
        }
        let saved = self.bind_closure_params(closure, &[arg_reg]);
        let result = self.gen_expr(&closure.body);
        self.restore_closure_params(&saved);
        result
    }

    /// Zig `genInlineClosureNullary` (19361).
    pub(crate) fn gen_inline_closure_nullary(
        &mut self,
        closure: &ClosureLiteral,
    ) -> Result<u32> {
        if !closure.params.is_empty() {
            return Err(Error::UnsupportedSabDirectFeature(
                "nullary inline closure with parameters".to_string(),
            ));
        }
        self.gen_expr(&closure.body)
    }

    /// Zig `genInlineClosureBinary` (19366).
    pub(crate) fn gen_inline_closure_binary(
        &mut self,
        closure: &ClosureLiteral,
        left_reg: u32,
        right_reg: u32,
    ) -> Result<u32> {
        if closure.params.len() != 2 {
            return Err(Error::UnsupportedSabDirectFeature(
                "binary inline closure with a non-2 parameter count".to_string(),
            ));
        }
        let saved = self.bind_closure_params(closure, &[left_reg, right_reg]);
        let result = self.gen_expr(&closure.body);
        self.restore_closure_params(&saved);
        result
    }

    /// Zig `genClosureCall` (19378): evaluate the arguments, bind
    /// them as the closure's parameters, inline the body, then
    /// release every argument register that is neither the result
    /// nor a local.
    pub(crate) fn gen_closure_call(
        &mut self,
        closure: &ClosureLiteral,
        call: &CallExpr,
    ) -> Result<u32> {
        if closure.params.len() != call.args.len() {
            return Err(Error::UnsupportedSabDirectFeature(
                "closure call arity does not match the closure's parameters".to_string(),
            ));
        }
        let mut arg_regs = Vec::with_capacity(call.args.len());
        for arg in &call.args {
            arg_regs.push(self.gen_expr(arg)?);
        }
        let saved = self.bind_closure_params(closure, &arg_regs);
        let result = self.gen_expr(&closure.body);
        self.restore_closure_params(&saved);
        let result = result?;
        for &arg_reg in &arg_regs {
            if arg_reg == result || self.is_local_reg(arg_reg) {
                continue;
            }
            self.emit_release(arg_reg)?;
        }
        Ok(result)
    }

    /// The `genLet` closure arms (8156–8180). Returns `true` when
    /// the binding was a closure registration (literal or alias)
    /// and the let is fully handled.
    pub(crate) fn try_gen_closure_let(&mut self, name: &str, value: &Node) -> Result<bool> {
        if let Some(closure) = closure_literal_from_expr(value) {
            let closure = closure.clone();
            let dst = self.binding_reg(name);
            self.state
                .program
                .closure_bindings
                .insert(name.to_string(), closure);
            self.b().emit_assign_imm(dst, 0);
            self.state.frame.push_local(Local::plain(name, dst, false));
            return Ok(true);
        }
        // FS-9 closure-alias arm: `let g = f;` where `f` names a
        // closure binding registers `g` for the same closure.
        if let Node::Identifier(src_name) = value {
            if let Some(closure) = self
                .state
                .program
                .closure_bindings
                .get(src_name)
                .cloned()
            {
                let dst = self.binding_reg(name);
                self.state
                    .program
                    .closure_bindings
                    .insert(name.to_string(), closure);
                if let Some(src) = self.state.frame.local_reg(src_name) {
                    self.b().emit_assign_reg(dst, src);
                } else {
                    self.b().emit_assign_imm(dst, 0);
                }
                self.state.frame.push_local(Local::plain(name, dst, false));
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Zig `genMove` (14283).
    pub(crate) fn gen_move_expr(&mut self, mv: &MoveExpr) -> Result<u32> {
        let source = self.gen_expr(&mv.expr)?;
        let inner_ty = self.check.expr_type(&mv.expr).cloned();
        let value_is_copy = inner_ty
            .as_ref()
            .map(|ty| self.check.type_is_copy_value(ty))
            .unwrap_or(false);
        if move_expr_consumes_source(inner_ty.as_ref(), value_is_copy) {
            self.mark_consumed(source);
        }
        Ok(source)
    }

    /// Zig `genImplDecl` (7314).
    pub(crate) fn gen_impl_decl(&mut self, impl_decl: &ImplDecl) -> Result<()> {
        let impl_name = symbols::concrete_type_name(&impl_decl.target_ty)
            .ok_or(Error::UnsupportedSabDirectFeature(
                "impl decl target has no concrete type name".to_string(),
            ))?;
        for method in &impl_decl.methods {
            let Node::FuncDecl(f) = method else {
                return Err(Error::UnsupportedSabDirectFeature(
                    "impl decl member is not a function declaration".to_string(),
                ));
            };
            if f.is_decl_only {
                continue;
            }
            let mangled = match &impl_decl.trait_name {
                Some(trait_name) => {
                    symbols::mangle_trait_method_name(&impl_name, trait_name, &f.name)
                }
                None => symbols::mangle_method_name(&impl_name, &f.name),
            };
            let lowered = symbols::lowered_func_symbol(&mangled, self.check);
            self.gen_func_decl_named(&lowered, f)?;
        }
        Ok(())
    }

    /// Zig `genOverloadDecl` (7330).
    pub(crate) fn gen_overload_decl(&mut self, overload_decl: &OverloadDecl) -> Result<()> {
        let overload_name = symbols::concrete_type_name(&overload_decl.target_ty)
            .ok_or(Error::UnsupportedSabDirectFeature(
                "overload decl target has no concrete type name".to_string(),
            ))?;
        for method in &overload_decl.methods {
            let Node::FuncDecl(f) = method else {
                return Err(Error::UnsupportedSabDirectFeature(
                    "overload decl member is not a function declaration".to_string(),
                ));
            };
            if f.is_decl_only {
                continue;
            }
            let mangled = symbols::mangle_method_name(&overload_name, &f.name);
            let lowered = symbols::lowered_func_symbol(&mangled, self.check);
            self.gen_func_decl_named(&lowered, f)?;
        }
        Ok(())
    }

    /// Call-site half of the closure family for `gen_call`'s
    /// dispatch: a call through a let-bound closure name inlines
    /// the closure (Zig 17812). Returns `None` when the name is
    /// not a closure binding.
    pub(crate) fn try_gen_bound_closure_call(
        &mut self,
        call: &CallExpr,
    ) -> Result<Option<u32>> {
        let Some(closure) = self
            .state
            .program
            .closure_bindings
            .get(&call.func_name)
            .cloned()
        else {
            return Ok(None);
        };
        Ok(Some(self.gen_closure_call(&closure, call)?))
    }

    /// Call-site half for an immediately-invoked closure callee
    /// (Zig 17770–17774): the callee node must be a closure
    /// literal itself — anything else is the plain
    /// `UnsupportedSabDirectFeature` Zig returns.
    pub(crate) fn gen_invoked_closure_call(
        &mut self,
        callee: &Node,
        call: &CallExpr,
    ) -> Result<u32> {
        let Node::ClosureLiteral(closure) = callee else {
            return Err(Error::UnsupportedSabDirectFeature(
                "immediately-invoked callee is not a closure literal".to_string(),
            ));
        };
        let closure = closure.clone();
        self.gen_closure_call(&closure, call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{BinaryExpr, BinaryOp, Literal, Param, Primitive};
    use sla_lowering::MapCheckInfo;

    fn i32_ty() -> Type {
        Type::Primitive(Primitive::I32)
    }

    fn param(name: &str) -> Param {
        Param {
            name: name.to_string(),
            ty: Box::new(i32_ty()),
            is_borrow: false,
            is_move: false,
        }
    }

    #[test]
    fn inline_unary_identity_returns_the_argument_register() {
        let closure = ClosureLiteral {
            params: vec![param("x")],
            body: Box::new(Node::Identifier("x".to_string())),
        };
        let check = MapCheckInfo::new();
        let mut driver = Driver::new(&check);
        let result = driver.gen_inline_closure_unary(&closure, 42).unwrap();
        assert_eq!(result, 42, "the param name substitutes the argument register");
        assert!(
            driver.state.program.closure_param_regs.is_empty(),
            "the substitution is restored after inlining"
        );
    }

    #[test]
    fn inline_binary_substitutes_both_params() {
        let closure = ClosureLiteral {
            params: vec![param("a"), param("b")],
            body: Box::new(Node::BinaryExpr(BinaryExpr {
                op: BinaryOp::Add,
                left: Box::new(Node::Identifier("a".to_string())),
                right: Box::new(Node::Identifier("b".to_string())),
            })),
        };
        let mut check = MapCheckInfo::new();
        if let Node::BinaryExpr(b) = &*closure.body {
            check.add_expr_type(&b.left, i32_ty());
            check.add_expr_type(&b.right, i32_ty());
        }
        let mut driver = Driver::new(&check);
        let result = driver.gen_inline_closure_binary(&closure, 10, 20).unwrap();
        assert!(result != 10 && result != 20, "the body result is a fresh tmp");
        assert!(driver.state.program.closure_param_regs.is_empty());
    }

    #[test]
    fn inline_unary_arity_mismatch_is_loud() {
        let closure = ClosureLiteral {
            params: vec![param("a"), param("b")],
            body: Box::new(Node::Literal(Literal::IntVal(0))),
        };
        let check = MapCheckInfo::new();
        let mut driver = Driver::new(&check);
        assert!(driver.gen_inline_closure_unary(&closure, 1).is_err());
    }

    #[test]
    fn closure_literal_from_expr_unwraps_move() {
        let node = Node::MoveExpr(MoveExpr {
            expr: Box::new(Node::ClosureLiteral(ClosureLiteral {
                params: Vec::new(),
                body: Box::new(Node::Literal(Literal::IntVal(0))),
            })),
        });
        assert!(closure_literal_from_expr(&node).is_some());
        assert!(closure_literal_from_expr(&Node::Identifier("x".to_string())).is_none());
    }
}

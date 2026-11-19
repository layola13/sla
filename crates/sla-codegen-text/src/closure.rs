//! Inline closure expansion, mirrored from codegen.zig's
//! closure family: a closure literal never materializes as a
//! function. A let/const binding records the literal in
//! `closure_bindings` (the binding register is the literal
//! `0` placeholder), and a call through the binding — or an
//! immediately-invoked literal — evaluates the arguments,
//! binds the parameter names to the argument registers in
//! `closure_param_regs`, and generates the body inline
//! (codegen.zig:12799 `genClosureCall`, 12590
//! `genInlineClosureUnary`, 12610 `genInlineClosureNullary`).

use sla_ast::{CallExpr, ClosureLiteral, Node};

use crate::codegen::Codegen;
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// Push one inline-expansion parameter binding, returning
    /// the saved previous binding for restore.
    fn push_closure_param(&mut self, name: &str, reg: &str) -> Option<String> {
        let old = self
            .closure_param_regs
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r.clone());
        self.closure_param_regs
            .push((name.to_string(), reg.to_string()));
        old
    }

    fn restore_closure_params(&mut self, saved: Vec<(String, Option<String>)>) {
        for (name, old) in saved.into_iter().rev() {
            // Remove the innermost binding for this name.
            if let Some(pos) = self
                .closure_param_regs
                .iter()
                .rposition(|(n, _)| n == &name)
            {
                self.closure_param_regs.remove(pos);
            }
            if let Some(old_reg) = old {
                self.closure_param_regs.push((name, old_reg));
            }
        }
    }

    /// codegen.zig:12799 `genClosureCall`: evaluate the args,
    /// bind params to the arg registers, expand the body.
    pub(crate) fn gen_closure_call(
        &mut self,
        lit: &ClosureLiteral,
        call: &CallExpr,
    ) -> Result<String> {
        if lit.params.len() != call.args.len() {
            return unsupported(
                "closure call arity mismatch",
                "codegen.zig:12799 (genClosureCall)",
            );
        }
        let mut arg_regs: Vec<String> = Vec::new();
        for arg in &call.args {
            arg_regs.push(self.gen_expr(arg)?);
        }
        let mut saved: Vec<(String, Option<String>)> = Vec::new();
        for (param, reg) in lit.params.iter().zip(arg_regs.iter()) {
            let old = self.push_closure_param(&param.name, reg);
            saved.push((param.name.clone(), old));
        }
        let flow = self.gen_expr(&lit.body);
        self.restore_closure_params(saved);
        flow
    }

    /// codegen.zig:12590 `genInlineClosureUnary` (Option map /
    /// and_then payload expansion).
    pub(crate) fn gen_inline_closure_unary(
        &mut self,
        lit: &ClosureLiteral,
        arg_reg: &str,
    ) -> Result<String> {
        if lit.params.len() != 1 {
            return unsupported(
                "inline closure unary arity",
                "codegen.zig:12590 (genInlineClosureUnary)",
            );
        }
        let name = lit.params[0].name.clone();
        let old = self.push_closure_param(&name, arg_reg);
        let flow = self.gen_expr(&lit.body);
        self.restore_closure_params(vec![(name, old)]);
        flow
    }

    /// codegen.zig:12610 `genInlineClosureNullary`
    /// (unwrap_or_else default expansion).
    pub(crate) fn gen_inline_closure_nullary(
        &mut self,
        lit: &ClosureLiteral,
    ) -> Result<String> {
        if !lit.params.is_empty() {
            return unsupported(
                "inline closure nullary arity",
                "codegen.zig:12610 (genInlineClosureNullary)",
            );
        }
        self.gen_expr(&lit.body)
    }

    /// The closure literal bound to a name by an earlier
    /// let/const, if any (innermost binding wins).
    pub(crate) fn closure_binding_for(&self, name: &str) -> Option<ClosureLiteral> {
        self.closure_bindings
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, lit)| lit.clone())
    }

    /// Extract the closure literal from a call argument that
    /// may sit under a move wrapper (planOptionClosureCall's
    /// closureLiteralArity does the same).
    pub(crate) fn closure_literal_of(node: &Node) -> Option<&ClosureLiteral> {
        match node {
            Node::ClosureLiteral(lit) => Some(lit),
            Node::MoveExpr(m) => Self::closure_literal_of(&m.expr),
            _ => None,
        }
    }
}

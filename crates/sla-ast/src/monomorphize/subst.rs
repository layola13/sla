//! The substituting node walk — mirrors Zig `substituteNode` /
//! `substituteBlock` in `monomorphizer.zig`. Used to clone a
//! template body with the generic parameters replaced by the
//! concrete type arguments, BEFORE the specializing walk runs
//! over the clone (so a `GenericFuncRef` inside a template body
//! keeps its node shape here, with substituted type arguments,
//! and is only resolved by the subsequent specialize pass).
//!
//! Infallible: the Zig original only fails on allocation.
//! Declaration nodes other than `FuncDecl` are unreachable here
//! (bodies contain statements; impl methods are `FuncDecl`s) and
//! pass through unchanged.

use super::Monomorphizer;
use crate::decls::{FuncDecl, Param};
use crate::exprs::{
    AwaitExpr, BinaryExpr, BorrowExpr, CallExpr, Case, CastExpr, ClosureLiteral, DerefExpr,
    EnumLiteral, EnumLiteralField, FieldExpr, GenericFuncRef, IfExpr, IfLetCond, IndexExpr,
    MatchCase, MatchExpr, MoveExpr, SliceExpr, StructLiteral, StructLiteralField, SwitchExpr,
    TryExpr, TupleLiteral, ArrayLiteral, RepeatArrayLiteral, UnsafeExpr,
};
use crate::node::Node;
use crate::stmts::{
    AssignStmt, BlockStmt, ConstStmt, DeferStmt, ForStmt, GoStmt, LetDestructureStmt, LetElseStmt,
    LetStmt, ReleaseStmt, ReturnStmt, SelectStmt, VarStmt, WhileStmt,
};

impl Monomorphizer {
    pub(crate) fn substitute_block(
        &self,
        block: Vec<Node>,
        params: &[String],
        args: &[crate::types::Type],
    ) -> Vec<Node> {
        block
            .into_iter()
            .map(|n| self.substitute_node(n, params, args))
            .collect()
    }

    fn substitute_param(&self, param: Param, params: &[String], args: &[crate::types::Type]) -> Param {
        Param {
            name: param.name,
            ty: Box::new(self.substitute_type(&param.ty, params, args)),
            is_borrow: param.is_borrow,
            is_move: param.is_move,
            fallible_sugar: param.fallible_sugar,
            is_var: param.is_var,
        }
    }

    fn substitute_opt_node(
        &self,
        node: Option<Box<Node>>,
        params: &[String],
        args: &[crate::types::Type],
    ) -> Option<Box<Node>> {
        node.map(|inner| Box::new(self.substitute_node(*inner, params, args)))
    }

    pub(crate) fn substitute_node(
        &self,
        node: Node,
        params: &[String],
        args: &[crate::types::Type],
    ) -> Node {
        match node {
            Node::FuncDecl(f) => {
                let params_sub = f
                    .params
                    .into_iter()
                    .map(|p| self.substitute_param(p, params, args))
                    .collect();
                Node::FuncDecl(FuncDecl {
                    name: f.name,
                    is_pub: f.is_pub,
                    is_extern: f.is_extern,
                    abi: f.abi,
                    no_mangle: f.no_mangle,
                    is_decl_only: f.is_decl_only,
                    generics: Vec::new(),
                    params: params_sub,
                    ret_ty: Box::new(self.substitute_type(&f.ret_ty, params, args)),
                    body: self.substitute_block(f.body, params, args),
                    is_inline: f.is_inline,
                    is_async: f.is_async,
                    operator: f.operator,
                    is_arena: f.is_arena,
                    ret_fallible_sugar: f.ret_fallible_sugar,
                    ret_borrow_origin: f.ret_borrow_origin,
                    is_variadic: f.is_variadic,
                })
            }
            Node::Program(_)
            | Node::StructDecl(_)
            | Node::EnumDecl(_)
            | Node::TraitDecl(_)
            | Node::ImplDecl(_)
            | Node::TypeAliasDecl(_)
            | Node::OverloadDecl(_)
            | Node::MacroDecl(_)
            | Node::CheckHookDecl(_)
            | Node::SublanguageDecl(_)
            | Node::ShaderDecl(_)
            | Node::ImportDecl(_)
            | Node::UsingDecl(_)
            | Node::TestDecl(_) => node,

            // -- statements --
            Node::LetStmt(l) => Node::LetStmt(LetStmt {
                name: l.name,
                ty: l
                    .ty
                    .map(|ty| Box::new(self.substitute_type(&ty, params, args))),
                value: Box::new(self.substitute_node(*l.value, params, args)),
            }),
            Node::LetElseStmt(l) => Node::LetElseStmt(LetElseStmt {
                pattern: l.pattern,
                value: Box::new(self.substitute_node(*l.value, params, args)),
                else_block: self.substitute_block(l.else_block, params, args),
            }),
            Node::LetDestructureStmt(l) => Node::LetDestructureStmt(LetDestructureStmt {
                names: l.names,
                value: Box::new(self.substitute_node(*l.value, params, args)),
                rest_name: l.rest_name,
                rest_alias: l.rest_alias,
                is_slice: l.is_slice,
            }),
            Node::ConstStmt(c) => Node::ConstStmt(ConstStmt {
                name: c.name,
                ty: c
                    .ty
                    .map(|ty| Box::new(self.substitute_type(&ty, params, args))),
                value: Box::new(self.substitute_node(*c.value, params, args)),
            }),
            Node::VarStmt(v) => Node::VarStmt(VarStmt {
                name: v.name,
                ty: Box::new(self.substitute_type(&v.ty, params, args)),
            }),
            Node::AssignStmt(a) => Node::AssignStmt(AssignStmt {
                target: Box::new(self.substitute_node(*a.target, params, args)),
                value: Box::new(self.substitute_node(*a.value, params, args)),
            }),
            Node::BlockStmt(b) => Node::BlockStmt(BlockStmt {
                body: self.substitute_block(b.body, params, args),
            }),
            Node::ArenaStmt(b) => Node::ArenaStmt(BlockStmt {
                body: self.substitute_block(b.body, params, args),
            }),
            Node::ExprStmt(expr) => {
                Node::ExprStmt(Box::new(self.substitute_node(*expr, params, args)))
            }
            Node::ReturnStmt(r) => Node::ReturnStmt(ReturnStmt {
                value: self.substitute_opt_node(r.value, params, args),
            }),
            Node::ForStmt(f) => Node::ForStmt(ForStmt {
                var_name: f.var_name,
                start: Box::new(self.substitute_node(*f.start, params, args)),
                end: self.substitute_opt_node(f.end, params, args),
                body: self.substitute_block(f.body, params, args),
            }),
            Node::WhileStmt(w) => Node::WhileStmt(WhileStmt {
                cond: Box::new(self.substitute_node(*w.cond, params, args)),
                let_pattern: w.let_pattern,
                body: self.substitute_block(w.body, params, args),
            }),
            Node::BreakStmt(_) | Node::ContinueStmt(_) => node,
            Node::ReleaseStmt(r) => Node::ReleaseStmt(ReleaseStmt { var_name: r.var_name }),
            Node::GoStmt(g) => Node::GoStmt(GoStmt {
                supervised: g.supervised,
                call: Box::new(self.substitute_node(*g.call, params, args)),
            }),
            Node::DeferStmt(d) => Node::DeferStmt(DeferStmt {
                call: Box::new(self.substitute_node(*d.call, params, args)),
            }),
            Node::SelectStmt(s) => {
                let arms = s
                    .arms
                    .into_iter()
                    .map(|arm| crate::stmts::SelectArm {
                        pattern: arm.pattern,
                        recv: self.substitute_opt_node(arm.recv, params, args),
                        body: self.substitute_block(arm.body, params, args),
                        is_default: arm.is_default,
                    })
                    .collect();
                let timeout = s.timeout.map(|t| crate::stmts::SelectTimeout {
                    expr: Box::new(self.substitute_node(*t.expr, params, args)),
                    body: self.substitute_block(t.body, params, args),
                });
                Node::SelectStmt(SelectStmt { arms, timeout })
            }

            // -- expressions --
            Node::Literal(_) | Node::Identifier(_) => node,
            Node::GenericFuncRef(gref) => Node::GenericFuncRef(GenericFuncRef {
                func_name: gref.func_name,
                generics: gref
                    .generics
                    .iter()
                    .map(|g| self.substitute_type(g, params, args))
                    .collect(),
            }),
            Node::IfExpr(ife) => {
                let let_chain = ife.let_chain.map(|chain| {
                    chain
                        .into_iter()
                        .map(|cond| IfLetCond {
                            pattern: cond.pattern,
                            value: Box::new(self.substitute_node(*cond.value, params, args)),
                        })
                        .collect()
                });
                Node::IfExpr(IfExpr {
                    cond: Box::new(self.substitute_node(*ife.cond, params, args)),
                    let_chain,
                    then_block: self.substitute_block(ife.then_block, params, args),
                    else_block: ife
                        .else_block
                        .map(|b| self.substitute_block(b, params, args)),
                })
            }
            Node::SwitchExpr(swe) => Node::SwitchExpr(SwitchExpr {
                val: Box::new(self.substitute_node(*swe.val, params, args)),
                cases: swe
                    .cases
                    .into_iter()
                    .map(|case| Case {
                        pattern: Box::new(self.substitute_node(*case.pattern, params, args)),
                        body: self.substitute_block(case.body, params, args),
                    })
                    .collect(),
            }),
            Node::MatchExpr(m) => Node::MatchExpr(MatchExpr {
                val: Box::new(self.substitute_node(*m.val, params, args)),
                cases: m
                    .cases
                    .into_iter()
                    .map(|case| MatchCase {
                        pattern: case.pattern,
                        guard: self.substitute_opt_node(case.guard, params, args),
                        body: self.substitute_block(case.body, params, args),
                    })
                    .collect(),
            }),
            Node::UnsafeExpr(u) => Node::UnsafeExpr(UnsafeExpr {
                body: self.substitute_block(u.body, params, args),
            }),
            Node::AwaitExpr(a) => Node::AwaitExpr(AwaitExpr {
                expr: Box::new(self.substitute_node(*a.expr, params, args)),
            }),
            Node::InlineAsmExpr(_) => node,
            Node::BinaryExpr(b) => Node::BinaryExpr(BinaryExpr {
                op: b.op,
                left: Box::new(self.substitute_node(*b.left, params, args)),
                right: Box::new(self.substitute_node(*b.right, params, args)),
            }),
            Node::CallExpr(call) => Node::CallExpr(CallExpr {
                func_name: call.func_name,
                associated_target: call.associated_target,
                generics: call
                    .generics
                    .iter()
                    .map(|g| self.substitute_type(g, params, args))
                    .collect(),
                args: self.substitute_block(call.args, params, args),
                callee: self.substitute_opt_node(call.callee, params, args),
            }),
            Node::ClosureLiteral(lit) => Node::ClosureLiteral(ClosureLiteral {
                params: lit
                    .params
                    .into_iter()
                    .map(|p| self.substitute_param(p, params, args))
                    .collect(),
                body: Box::new(self.substitute_node(*lit.body, params, args)),
            }),
            Node::BorrowExpr(b) => Node::BorrowExpr(BorrowExpr {
                expr: Box::new(self.substitute_node(*b.expr, params, args)),
            }),
            Node::MoveExpr(m) => Node::MoveExpr(MoveExpr {
                expr: Box::new(self.substitute_node(*m.expr, params, args)),
            }),
            Node::DerefExpr(d) => Node::DerefExpr(DerefExpr {
                expr: Box::new(self.substitute_node(*d.expr, params, args)),
            }),
            Node::CastExpr(c) => Node::CastExpr(CastExpr {
                expr: Box::new(self.substitute_node(*c.expr, params, args)),
                ty: Box::new(self.substitute_type(&c.ty, params, args)),
            }),
            Node::FieldExpr(f) => Node::FieldExpr(FieldExpr {
                expr: Box::new(self.substitute_node(*f.expr, params, args)),
                field_name: f.field_name,
            }),
            Node::StructLiteral(lit) => Node::StructLiteral(StructLiteral {
                ty: Box::new(self.substitute_type(&lit.ty, params, args)),
                fields: lit
                    .fields
                    .into_iter()
                    .map(|f| StructLiteralField {
                        name: f.name,
                        value: Box::new(self.substitute_node(*f.value, params, args)),
                    })
                    .collect(),
                update_expr: self.substitute_opt_node(lit.update_expr, params, args),
            }),
            Node::EnumLiteral(lit) => Node::EnumLiteral(EnumLiteral {
                enum_name: lit.enum_name,
                variant_name: lit.variant_name,
                fields: lit
                    .fields
                    .into_iter()
                    .map(|f| EnumLiteralField {
                        name: f.name,
                        value: Box::new(self.substitute_node(*f.value, params, args)),
                    })
                    .collect(),
            }),
            Node::TupleLiteral(lit) => Node::TupleLiteral(TupleLiteral {
                elements: self.substitute_block(lit.elements, params, args),
            }),
            Node::ArrayLiteral(lit) => Node::ArrayLiteral(ArrayLiteral {
                elements: self.substitute_block(lit.elements, params, args),
            }),
            Node::RepeatArrayLiteral(lit) => Node::RepeatArrayLiteral(RepeatArrayLiteral {
                value: Box::new(self.substitute_node(*lit.value, params, args)),
                len: lit.len,
            }),
            Node::IndexExpr(idx) => Node::IndexExpr(IndexExpr {
                target: Box::new(self.substitute_node(*idx.target, params, args)),
                index: Box::new(self.substitute_node(*idx.index, params, args)),
            }),
            Node::SliceExpr(s) => Node::SliceExpr(SliceExpr {
                target: Box::new(self.substitute_node(*s.target, params, args)),
                start: Box::new(self.substitute_node(*s.start, params, args)),
                end: Box::new(self.substitute_node(*s.end, params, args)),
            }),
            Node::TryExpr(t) => Node::TryExpr(TryExpr {
                expr: Box::new(self.substitute_node(*t.expr, params, args)),
            }),
        }
    }
}

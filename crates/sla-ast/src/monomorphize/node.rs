//! The specializing node walk — mirrors Zig `specializeNode` /
//! `specializeBlock` / `specializeEnumLiteralForType` in
//! `monomorphizer.zig`. Every node is rebuilt with specialized
//! types and rewritten generic use sites; leaf nodes pass
//! through unchanged. Rust-only payload fields the Zig AST does
//! not carry (`Param::fallible_sugar`, `Param::is_var`,
//! `FuncDecl::is_arena`, `EnumVariant::discriminant`,
//! `MacroDecl::param_tys`, `LetDestructureStmt::rest_alias`)
//! are preserved verbatim.

use super::types::mangled_name;
use super::{MonomorphizeError, Monomorphizer};
use crate::decls::{
    EnumDecl, FuncDecl, ImplDecl, MacroDecl, OverloadDecl, Param, StructDecl, TestDecl,
    TraitDecl, TraitMethod, TypeAliasComponent, TypeAliasDecl,
};
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
use crate::types::Type;

impl Monomorphizer {
    pub(crate) fn specialize_block(
        &mut self,
        block: Vec<Node>,
    ) -> Result<Vec<Node>, MonomorphizeError> {
        block.into_iter().map(|n| self.specialize_node(n)).collect()
    }

    fn specialize_param(&mut self, param: Param) -> Result<Param, MonomorphizeError> {
        Ok(Param {
            name: param.name,
            ty: Box::new(self.specialize_type(*param.ty)?),
            is_borrow: param.is_borrow,
            is_move: param.is_move,
            fallible_sugar: param.fallible_sugar,
            is_var: param.is_var,
        })
    }

    fn specialize_params(&mut self, params: Vec<Param>) -> Result<Vec<Param>, MonomorphizeError> {
        params.into_iter().map(|p| self.specialize_param(p)).collect()
    }

    fn specialize_opt_node(
        &mut self,
        node: Option<Box<Node>>,
    ) -> Result<Option<Box<Node>>, MonomorphizeError> {
        match node {
            Some(inner) => Ok(Some(Box::new(self.specialize_node(*inner)?))),
            None => Ok(None),
        }
    }

    /// Mirrors Zig `specializeEnumLiteralForType`: an enum
    /// literal whose declared type was specialized to a mangled
    /// instance is renamed to that instance; anything else goes
    /// through the plain walk.
    fn specialize_enum_literal_for_type(
        &mut self,
        value: Node,
        declared_ty: &Type,
        specialized_ty: &Type,
    ) -> Result<Node, MonomorphizeError> {
        if let Node::EnumLiteral(lit) = &value {
            if let (Type::UserDefined(declared_ud), Type::UserDefined(specialized_ud)) =
                (declared_ty, specialized_ty)
            {
                if lit.enum_name == declared_ud.name {
                    let specialized_name = specialized_ud.name.clone();
                    let Node::EnumLiteral(lit) = value else { unreachable!() };
                    let mut fields = Vec::with_capacity(lit.fields.len());
                    for f in lit.fields {
                        fields.push(EnumLiteralField {
                            name: f.name,
                            value: Box::new(self.specialize_node(*f.value)?),
                        });
                    }
                    return Ok(Node::EnumLiteral(EnumLiteral {
                        enum_name: specialized_name,
                        variant_name: lit.variant_name,
                        fields,
                    }));
                }
            }
        }
        self.specialize_node(value)
    }

    pub(crate) fn specialize_node(&mut self, node: Node) -> Result<Node, MonomorphizeError> {
        Ok(match node {
            Node::Program(_) => node,
            Node::StructDecl(s) => {
                let mut fields = Vec::with_capacity(s.fields.len());
                for f in s.fields {
                    fields.push(crate::decls::Field {
                        name: f.name,
                        ty: Box::new(self.specialize_type(*f.ty)?),
                    });
                }
                Node::StructDecl(StructDecl {
                    name: s.name,
                    derives: s.derives,
                    generics: Vec::new(),
                    fields,
                    is_union: s.is_union,
                    is_opaque: s.is_opaque,
                })
            }
            Node::EnumDecl(e) => {
                let mut variants = Vec::with_capacity(e.variants.len());
                for v in e.variants {
                    let mut fields = Vec::with_capacity(v.fields.len());
                    for f in v.fields {
                        fields.push(crate::decls::Field {
                            name: f.name,
                            ty: Box::new(self.specialize_type(*f.ty)?),
                        });
                    }
                    variants.push(crate::decls::EnumVariant {
                        name: v.name,
                        fields,
                        discriminant: v.discriminant,
                    });
                }
                Node::EnumDecl(EnumDecl {
                    name: e.name,
                    generics: Vec::new(),
                    variants,
                })
            }
            Node::TraitDecl(t) => {
                let mut methods = Vec::with_capacity(t.methods.len());
                for m in t.methods {
                    methods.push(TraitMethod {
                        name: m.name,
                        params: self.specialize_params(m.params)?,
                        ret_ty: Box::new(self.specialize_type(*m.ret_ty)?),
                    });
                }
                Node::TraitDecl(TraitDecl {
                    name: t.name,
                    supertraits: t.supertraits,
                    methods,
                })
            }
            Node::ImplDecl(i) => Node::ImplDecl(ImplDecl {
                trait_name: i.trait_name,
                target_ty: Box::new(self.specialize_type(*i.target_ty)?),
                methods: self.specialize_block(i.methods)?,
            }),
            Node::TypeAliasDecl(a) => {
                let mut components = Vec::with_capacity(a.components.len());
                for component in a.components {
                    components.push(match component {
                        TypeAliasComponent::Ty(ty) => {
                            TypeAliasComponent::Ty(Box::new(self.specialize_type(*ty)?))
                        }
                        TypeAliasComponent::InlineStruct(fields) => {
                            let mut new_fields = Vec::with_capacity(fields.len());
                            for f in fields {
                                new_fields.push(crate::decls::Field {
                                    name: f.name,
                                    ty: Box::new(self.specialize_type(*f.ty)?),
                                });
                            }
                            TypeAliasComponent::InlineStruct(new_fields)
                        }
                    });
                }
                Node::TypeAliasDecl(TypeAliasDecl {
                    name: a.name,
                    components,
                })
            }
            Node::OverloadDecl(o) => Node::OverloadDecl(OverloadDecl {
                target_ty: Box::new(self.specialize_type(*o.target_ty)?),
                methods: self.specialize_block(o.methods)?,
            }),
            Node::FuncDecl(f) => {
                let params = self.specialize_params(f.params)?;
                let ret_ty = Box::new(self.specialize_type(*f.ret_ty)?);
                let body = self.specialize_block(f.body)?;
                Node::FuncDecl(FuncDecl {
                    name: f.name,
                    is_pub: f.is_pub,
                    is_extern: f.is_extern,
                    abi: f.abi,
                    no_mangle: f.no_mangle,
                    is_decl_only: f.is_decl_only,
                    generics: Vec::new(),
                    params,
                    ret_ty,
                    body,
                    is_inline: f.is_inline,
                    is_async: f.is_async,
                    operator: f.operator,
                    is_arena: f.is_arena,
                    ret_fallible_sugar: f.ret_fallible_sugar,
                    ret_borrow_origin: f.ret_borrow_origin,
                    is_variadic: f.is_variadic,
                })
            }
            Node::MacroDecl(m) => Node::MacroDecl(MacroDecl {
                name: m.name,
                params: m.params,
                param_tys: m.param_tys,
                body: self.specialize_block(m.body)?,
            }),
            Node::CheckHookDecl(_)
            | Node::SublanguageDecl(_)
            | Node::ShaderDecl(_)
            | Node::ImportDecl(_)
            | Node::UsingDecl(_) => node,
            Node::TestDecl(t) => Node::TestDecl(TestDecl {
                name: t.name,
                is_ignored: t.is_ignored,
                should_panic: t.should_panic,
                body: self.specialize_block(t.body)?,
            }),

            // -- statements --
            Node::LetStmt(l) => {
                let new_ty = match &l.ty {
                    Some(ty) => Some(Box::new(self.specialize_type((**ty).clone())?)),
                    None => None,
                };
                let new_val = match (&l.ty, &new_ty) {
                    (Some(declared), Some(new_ty)) => {
                        self.specialize_enum_literal_for_type(*l.value, declared, new_ty)?
                    }
                    _ => self.specialize_node(*l.value)?,
                };
                Node::LetStmt(LetStmt {
                    name: l.name,
                    ty: new_ty,
                    value: Box::new(new_val),
                })
            }
            Node::LetElseStmt(l) => Node::LetElseStmt(LetElseStmt {
                pattern: l.pattern,
                value: Box::new(self.specialize_node(*l.value)?),
                else_block: self.specialize_block(l.else_block)?,
            }),
            Node::LetDestructureStmt(l) => Node::LetDestructureStmt(LetDestructureStmt {
                names: l.names,
                value: Box::new(self.specialize_node(*l.value)?),
                rest_name: l.rest_name,
                rest_alias: l.rest_alias,
                is_slice: l.is_slice,
            }),
            Node::ConstStmt(c) => {
                let new_ty = match &c.ty {
                    Some(ty) => Some(Box::new(self.specialize_type((**ty).clone())?)),
                    None => None,
                };
                let new_val = match (&c.ty, &new_ty) {
                    (Some(declared), Some(new_ty)) => {
                        self.specialize_enum_literal_for_type(*c.value, declared, new_ty)?
                    }
                    _ => self.specialize_node(*c.value)?,
                };
                Node::ConstStmt(ConstStmt {
                    name: c.name,
                    ty: new_ty,
                    value: Box::new(new_val),
                })
            }
            Node::VarStmt(v) => Node::VarStmt(VarStmt {
                name: v.name,
                ty: Box::new(self.specialize_type(*v.ty)?),
            }),
            Node::AssignStmt(a) => Node::AssignStmt(AssignStmt {
                target: Box::new(self.specialize_node(*a.target)?),
                value: Box::new(self.specialize_node(*a.value)?),
            }),
            Node::BlockStmt(b) => Node::BlockStmt(BlockStmt {
                body: self.specialize_block(b.body)?,
            }),
            Node::ArenaStmt(b) => Node::ArenaStmt(BlockStmt {
                body: self.specialize_block(b.body)?,
            }),
            Node::ExprStmt(expr) => Node::ExprStmt(Box::new(self.specialize_node(*expr)?)),
            Node::ReturnStmt(r) => Node::ReturnStmt(ReturnStmt {
                value: self.specialize_opt_node(r.value)?,
            }),
            Node::ForStmt(f) => Node::ForStmt(ForStmt {
                var_name: f.var_name,
                start: Box::new(self.specialize_node(*f.start)?),
                end: self.specialize_opt_node(f.end)?,
                body: self.specialize_block(f.body)?,
            }),
            Node::WhileStmt(w) => Node::WhileStmt(WhileStmt {
                cond: Box::new(self.specialize_node(*w.cond)?),
                let_pattern: w.let_pattern,
                body: self.specialize_block(w.body)?,
            }),
            Node::BreakStmt(_) | Node::ContinueStmt(_) => node,
            Node::ReleaseStmt(r) => Node::ReleaseStmt(ReleaseStmt { var_name: r.var_name }),
            Node::GoStmt(g) => Node::GoStmt(GoStmt {
                supervised: g.supervised,
                call: Box::new(self.specialize_node(*g.call)?),
            }),
            Node::DeferStmt(d) => Node::DeferStmt(DeferStmt {
                call: Box::new(self.specialize_node(*d.call)?),
            }),
            Node::SelectStmt(s) => {
                let mut arms = Vec::with_capacity(s.arms.len());
                for arm in s.arms {
                    arms.push(crate::stmts::SelectArm {
                        pattern: arm.pattern,
                        recv: self.specialize_opt_node(arm.recv)?,
                        body: self.specialize_block(arm.body)?,
                        is_default: arm.is_default,
                    });
                }
                let timeout = match s.timeout {
                    Some(t) => Some(crate::stmts::SelectTimeout {
                        expr: Box::new(self.specialize_node(*t.expr)?),
                        body: self.specialize_block(t.body)?,
                    }),
                    None => None,
                };
                Node::SelectStmt(SelectStmt { arms, timeout })
            }

            // -- expressions --
            Node::Literal(_) | Node::Identifier(_) => node,
            Node::GenericFuncRef(gref) => {
                // Zig: the reference's type arguments are
                // specialized first, then mangled and
                // instantiated; the node becomes a plain
                // identifier naming the instance.
                let mut spec_generics = Vec::with_capacity(gref.generics.len());
                for g in gref.generics {
                    spec_generics.push(self.specialize_type(g)?);
                }
                let mangled = mangled_name(&gref.func_name, &spec_generics);
                if !self.specialized_funcs.contains(&mangled) {
                    let template = self
                        .func_templates
                        .get(&gref.func_name)
                        .cloned()
                        .ok_or_else(|| self.template_not_found(&gref.func_name))?;
                    self.instantiate_function(&mangled, &template, &spec_generics)?;
                }
                Node::Identifier(mangled)
            }
            Node::IfExpr(ife) => {
                let let_chain = match ife.let_chain {
                    Some(chain) => {
                        let mut items = Vec::with_capacity(chain.len());
                        for cond in chain {
                            items.push(IfLetCond {
                                pattern: cond.pattern,
                                value: Box::new(self.specialize_node(*cond.value)?),
                            });
                        }
                        Some(items)
                    }
                    None => None,
                };
                Node::IfExpr(IfExpr {
                    cond: Box::new(self.specialize_node(*ife.cond)?),
                    let_chain,
                    then_block: self.specialize_block(ife.then_block)?,
                    else_block: match ife.else_block {
                        Some(b) => Some(self.specialize_block(b)?),
                        None => None,
                    },
                })
            }
            Node::SwitchExpr(swe) => {
                let mut cases = Vec::with_capacity(swe.cases.len());
                for case in swe.cases {
                    cases.push(Case {
                        pattern: Box::new(self.specialize_node(*case.pattern)?),
                        body: self.specialize_block(case.body)?,
                    });
                }
                Node::SwitchExpr(SwitchExpr {
                    val: Box::new(self.specialize_node(*swe.val)?),
                    cases,
                })
            }
            Node::MatchExpr(m) => {
                let mut cases = Vec::with_capacity(m.cases.len());
                for case in m.cases {
                    cases.push(MatchCase {
                        pattern: case.pattern,
                        guard: self.specialize_opt_node(case.guard)?,
                        body: self.specialize_block(case.body)?,
                    });
                }
                Node::MatchExpr(MatchExpr {
                    val: Box::new(self.specialize_node(*m.val)?),
                    cases,
                })
            }
            Node::UnsafeExpr(u) => Node::UnsafeExpr(UnsafeExpr {
                body: self.specialize_block(u.body)?,
            }),
            Node::AwaitExpr(a) => Node::AwaitExpr(AwaitExpr {
                expr: Box::new(self.specialize_node(*a.expr)?),
            }),
            Node::InlineAsmExpr(_) => node,
            Node::BinaryExpr(b) => Node::BinaryExpr(BinaryExpr {
                op: b.op,
                left: Box::new(self.specialize_node(*b.left)?),
                right: Box::new(self.specialize_node(*b.right)?),
            }),
            Node::CallExpr(call) => self.specialize_call(call)?,
            Node::ClosureLiteral(lit) => Node::ClosureLiteral(ClosureLiteral {
                params: self.specialize_params(lit.params)?,
                body: Box::new(self.specialize_node(*lit.body)?),
            }),
            Node::BorrowExpr(b) => Node::BorrowExpr(BorrowExpr {
                expr: Box::new(self.specialize_node(*b.expr)?),
            }),
            Node::MoveExpr(m) => Node::MoveExpr(MoveExpr {
                expr: Box::new(self.specialize_node(*m.expr)?),
            }),
            Node::DerefExpr(d) => Node::DerefExpr(DerefExpr {
                expr: Box::new(self.specialize_node(*d.expr)?),
            }),
            Node::CastExpr(c) => Node::CastExpr(CastExpr {
                expr: Box::new(self.specialize_node(*c.expr)?),
                ty: Box::new(self.specialize_type(*c.ty)?),
            }),
            Node::FieldExpr(f) => Node::FieldExpr(FieldExpr {
                expr: Box::new(self.specialize_node(*f.expr)?),
                field_name: f.field_name,
            }),
            Node::StructLiteral(lit) => {
                let mut fields = Vec::with_capacity(lit.fields.len());
                for f in lit.fields {
                    fields.push(StructLiteralField {
                        name: f.name,
                        value: Box::new(self.specialize_node(*f.value)?),
                    });
                }
                Node::StructLiteral(StructLiteral {
                    ty: Box::new(self.specialize_type(*lit.ty)?),
                    fields,
                    update_expr: self.specialize_opt_node(lit.update_expr)?,
                })
            }
            Node::EnumLiteral(lit) => {
                let mut fields = Vec::with_capacity(lit.fields.len());
                for f in lit.fields {
                    fields.push(EnumLiteralField {
                        name: f.name,
                        value: Box::new(self.specialize_node(*f.value)?),
                    });
                }
                Node::EnumLiteral(EnumLiteral {
                    enum_name: lit.enum_name,
                    variant_name: lit.variant_name,
                    fields,
                })
            }
            Node::TupleLiteral(lit) => Node::TupleLiteral(TupleLiteral {
                elements: self.specialize_block(lit.elements)?,
            }),
            Node::ArrayLiteral(lit) => Node::ArrayLiteral(ArrayLiteral {
                elements: self.specialize_block(lit.elements)?,
            }),
            Node::RepeatArrayLiteral(lit) => Node::RepeatArrayLiteral(RepeatArrayLiteral {
                value: Box::new(self.specialize_node(*lit.value)?),
                len: lit.len,
            }),
            Node::IndexExpr(idx) => Node::IndexExpr(IndexExpr {
                target: Box::new(self.specialize_node(*idx.target)?),
                index: Box::new(self.specialize_node(*idx.index)?),
            }),
            Node::SliceExpr(s) => Node::SliceExpr(SliceExpr {
                target: Box::new(self.specialize_node(*s.target)?),
                start: Box::new(self.specialize_node(*s.start)?),
                end: Box::new(self.specialize_node(*s.end)?),
            }),
            Node::TryExpr(t) => Node::TryExpr(TryExpr {
                expr: Box::new(self.specialize_node(*t.expr)?),
            }),
        })
    }

    /// The `call_expr` arm of Zig `specializeNode`, with its
    /// four-way split:
    ///   * explicit generics naming a non-template: pass through
    ///     with generics kept (specialized);
    ///   * explicit generics naming a template: instantiate and
    ///     rewrite to the mangled name;
    ///   * no explicit generics naming a template: rewrite only
    ///     when the shallow inference binds every parameter;
    ///   * anything else: an ordinary call.
    fn specialize_call(&mut self, call: CallExpr) -> Result<Node, MonomorphizeError> {
        let CallExpr {
            func_name,
            associated_target,
            generics,
            args,
            callee,
        } = call;

        if !generics.is_empty() && !self.func_templates.contains_key(&func_name) {
            let mut spec_generics = Vec::with_capacity(generics.len());
            for g in generics {
                spec_generics.push(self.specialize_type(g)?);
            }
            return Ok(Node::CallExpr(CallExpr {
                func_name,
                associated_target,
                generics: spec_generics,
                args: self.specialize_block(args)?,
                callee: self.specialize_opt_node(callee)?,
            }));
        }

        if !generics.is_empty() {
            // Zig mangles and instantiates from the call's raw
            // type arguments here (unlike the func-ref arm);
            // instantiation substitutes, then specializes.
            let mangled = mangled_name(&func_name, &generics);
            if !self.specialized_funcs.contains(&mangled) {
                let template = self
                    .func_templates
                    .get(&func_name)
                    .cloned()
                    .ok_or_else(|| self.template_not_found(&func_name))?;
                self.instantiate_function(&mangled, &template, &generics)?;
            }
            return Ok(Node::CallExpr(CallExpr {
                func_name: mangled,
                associated_target,
                generics: Vec::new(),
                args: self.specialize_block(args)?,
                callee: self.specialize_opt_node(callee)?,
            }));
        }

        if let Some(template) = self.func_templates.get(&func_name).cloned() {
            if let Some(inferred) = self.infer_generic_args_for_call(&template, &args) {
                let mangled = mangled_name(&func_name, &inferred);
                if !self.specialized_funcs.contains(&mangled) {
                    self.instantiate_function(&mangled, &template, &inferred)?;
                }
                return Ok(Node::CallExpr(CallExpr {
                    func_name: mangled,
                    associated_target,
                    generics: Vec::new(),
                    args: self.specialize_block(args)?,
                    callee: self.specialize_opt_node(callee)?,
                }));
            }
        }

        Ok(Node::CallExpr(CallExpr {
            func_name,
            associated_target,
            generics: Vec::new(),
            args: self.specialize_block(args)?,
            callee: self.specialize_opt_node(callee)?,
        }))
    }
}

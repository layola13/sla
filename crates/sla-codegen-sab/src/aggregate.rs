//! I group — aggregate literal materialization: struct and union
//! literals, field plans, and the value-copy machinery they share.
//!
//! Faithful port of the current-head Zig (reference tree
//! /content/sa_all/sa_plugin_sla @ 2cc12ea8, sab_codegen.zig):
//! - `genStructLiteral` (21212–21471) with
//!   `structLiteralFieldPlans` (7913) and lowering_rules'
//!   `planStructLiteralFieldTransfer` (5928), `StructLiteralFieldPlan`
//!   (3340), `structFieldIsPointerBacked` (5920),
//!   `structLiteralDirectFieldUsesConstStringSymbol` (5946);
//! - `genUnionLiteral` (21172);
//! - `genCopyValue` (22032), `genShallowCopyCallArgValue` (22058),
//!   `copyArrayValueInto` (21986), and the `genDeepCloneValue`
//!   (7663) arms that stay inside this group (array/tuple/struct
//!   clones); the fragment-backed clone arms (String/Vec/GrowVec/
//!   Box/Rc/Arc) stop loudly with their coordinates.
//!
//! Ownership bookkeeping with no byte effect in Zig
//! (`markObjectPtrReg`, `markConsumed` beyond the register set,
//! the `current_expr_later_nodes` stack) is mirrored through the
//! port's `RegState` set where a set exists; the later-nodes stack
//! has no baseline counterpart, so the copy-elision liveness test
//! is re-derived locally from the literal's own later field
//! values (registered in the I-group REPORT as SAB-D164).
//! `pending_moved_fields` is an `AutoHashMap` in Zig and is
//! iterated in hash order there; the port keeps insertion order
//! (house hash-order discipline, SAB-D18 family).

use sla_ast::{Node, StructDecl, StructLiteral, Type};
use sla_lowering::{expr_result_needs_release, stored_value_moves_identifier};

use crate::copy_facts::CopyFacts;
use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::layout::{self, LayoutAdapter};
use crate::types::FieldLayout;

/// Zig `lowering_rules.StructLiteralFieldSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldSource {
    Explicit,
    Update,
}

/// Zig `lowering_rules.StructLiteralFieldPlan` (3340), with the
/// layout in the port's D2 form (offset + storage prim).
struct FieldPlan<'n> {
    source: FieldSource,
    /// Zig plans carry the field name (used by its trace output;
    /// the port's errors name fields at plan-construction time).
    #[allow(dead_code)]
    name: String,
    value: Option<&'n Node>,
    layout: FieldLayout,
    field_ty: Type,
    release_loaded: bool,
}

/// Zig `lowering_rules.StructLiteralFieldTransfer` (3324).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldTransfer {
    Direct,
    DeepCopy,
    Move,
}

/// Zig `lowering_rules.structFieldIsPointerBacked` (5920).
fn struct_field_is_pointer_backed(field_ty: &Type) -> bool {
    match field_ty {
        Type::Infer | Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_) | Type::FnPtr(_) => {
            false
        }
        Type::UserDefined(_) | Type::Tuple(_) | Type::Array(_) => true,
        _ => true,
    }
}

/// Zig `lowering_rules.planStructLiteralFieldTransfer` (5928).
fn plan_field_transfer(plan: &FieldPlan, field_is_copy_struct: bool) -> FieldTransfer {
    match plan.source {
        FieldSource::Explicit => {
            let Some(value) = plan.value else {
                return FieldTransfer::Direct;
            };
            if matches!(value, Node::MoveExpr(_)) {
                return FieldTransfer::Move;
            }
            if field_is_copy_struct {
                return FieldTransfer::DeepCopy;
            }
            if struct_field_is_pointer_backed(&plan.field_ty) {
                return FieldTransfer::Move;
            }
            FieldTransfer::Direct
        }
        FieldSource::Update => {
            if !struct_field_is_pointer_backed(&plan.field_ty) {
                return FieldTransfer::Direct;
            }
            if field_is_copy_struct {
                return FieldTransfer::DeepCopy;
            }
            FieldTransfer::Move
        }
    }
}

/// Zig `lowering_rules.structLiteralDirectFieldUsesConstStringSymbol`.
fn direct_field_uses_const_string_symbol(value: &Node, field_ty: &Type) -> bool {
    if !matches!(value, Node::Literal(sla_ast::Literal::StringVal(_))) {
        return false;
    }
    crate::agg_emit::type_is_pointer_scalar_value(field_ty)
}

/// Does `name` occur as an identifier use inside any of `nodes`,
/// respecting closure-parameter shadowing (the Zig Y-SHARE scan's
/// `sharedClosureShadowsIdentifier` rule)? Local re-derivation of
/// `identifierMustStayLiveForLaterUse`'s current-expr arm over the
/// literal's later field values (SAB-D164).
fn identifier_used_in_nodes(name: &str, nodes: &[&Node]) -> bool {
    nodes.iter().any(|node| node_uses_identifier(node, name))
}

fn node_uses_identifier(node: &Node, name: &str) -> bool {
    match node {
        Node::Identifier(n) => n == name,
        Node::ClosureLiteral(closure) => {
            if closure.params.iter().any(|p| p.name == name) {
                false
            } else {
                node_uses_identifier(&closure.body, name)
            }
        }
        Node::BinaryExpr(b) => {
            node_uses_identifier(&b.left, name) || node_uses_identifier(&b.right, name)
        }
        Node::CallExpr(c) => {
            c.args.iter().any(|a| node_uses_identifier(a, name))
                || c.callee
                    .as_deref()
                    .map(|callee| node_uses_identifier(callee, name))
                    .unwrap_or(false)
        }
        Node::FieldExpr(f) => node_uses_identifier(&f.expr, name),
        Node::BorrowExpr(b) => node_uses_identifier(&b.expr, name),
        Node::MoveExpr(m) => node_uses_identifier(&m.expr, name),
        Node::DerefExpr(d) => node_uses_identifier(&d.expr, name),
        Node::CastExpr(c) => node_uses_identifier(&c.expr, name),
        Node::IndexExpr(i) => {
            node_uses_identifier(&i.target, name) || node_uses_identifier(&i.index, name)
        }
        Node::StructLiteral(lit) => {
            lit.fields.iter().any(|f| node_uses_identifier(&f.value, name))
                || lit
                    .update_expr
                    .as_deref()
                    .map(|u| node_uses_identifier(u, name))
                    .unwrap_or(false)
        }
        Node::TupleLiteral(lit) => lit.elements.iter().any(|e| node_uses_identifier(e, name)),
        Node::ArrayLiteral(lit) => lit.elements.iter().any(|e| node_uses_identifier(e, name)),
        Node::RepeatArrayLiteral(lit) => node_uses_identifier(&lit.value, name),
        Node::EnumLiteral(lit) => lit.fields.iter().any(|f| node_uses_identifier(&f.value, name)),
        Node::IfExpr(ife) => {
            node_uses_identifier(&ife.cond, name)
                || ife.then_block.iter().any(|s| node_uses_identifier(s, name))
                || ife
                    .else_block
                    .as_ref()
                    .map(|b| b.iter().any(|s| node_uses_identifier(s, name)))
                    .unwrap_or(false)
        }
        Node::BlockStmt(b) => b.body.iter().any(|s| node_uses_identifier(s, name)),
        Node::ArenaStmt(b) => b.body.iter().any(|s| node_uses_identifier(s, name)),
        Node::ExprStmt(e) => node_uses_identifier(e, name),
        Node::LetStmt(l) => node_uses_identifier(&l.value, name),
        Node::ReturnStmt(r) => r
            .value
            .as_ref()
            .map(|v| node_uses_identifier(v, name))
            .unwrap_or(false),
        _ => false,
    }
}

impl<'c> Driver<'c> {
    /// Zig `structLiteralFieldPlans` (7913): one plan per declared
    /// field, in declaration order; a literal field the decl does
    /// not declare, or a declared field with neither an explicit
    /// value nor an update source, is `UnsupportedSabDirectFeature`.
    fn struct_literal_field_plans<'n>(
        &mut self,
        decl: &StructDecl,
        lit: &'n StructLiteral,
    ) -> Result<Vec<FieldPlan<'n>>> {
        if decl.is_union {
            return Err(Error::UnsupportedSabDirectFeature(
                "struct literal field plans for a union decl".to_string(),
            ));
        }
        for literal_field in &lit.fields {
            if !decl.fields.iter().any(|f| f.name == literal_field.name) {
                return Err(Error::UnsupportedSabDirectFeature(format!(
                    "struct literal field `{}` not in decl `{}`",
                    literal_field.name, decl.name
                )));
            }
        }
        let mut plans = Vec::with_capacity(decl.fields.len());
        for field in &decl.fields {
            let layout = layout::struct_field_layout_detailed(decl, &field.name)
                .collapsed()
                .ok_or_else(|| {
                    Error::UnsupportedSabDirectFeature(format!(
                        "struct literal field `{}` has no layout",
                        field.name
                    ))
                })?;
            let explicit = lit
                .fields
                .iter()
                .find(|f| f.name == field.name)
                .map(|f| &*f.value);
            if let Some(value) = explicit {
                plans.push(FieldPlan {
                    source: FieldSource::Explicit,
                    name: field.name.clone(),
                    value: Some(value),
                    layout,
                    field_ty: (*field.ty).clone(),
                    release_loaded: false,
                });
            } else if let Some(update_expr) = &lit.update_expr {
                plans.push(FieldPlan {
                    source: FieldSource::Update,
                    name: field.name.clone(),
                    value: None,
                    layout,
                    field_ty: (*field.ty).clone(),
                    release_loaded: Self::update_source_needs_release(update_expr),
                });
            } else {
                return Err(Error::UnsupportedSabDirectFeature(format!(
                    "struct literal for `{}` is missing field `{}` and has no update source",
                    decl.name, field.name
                )));
            }
        }
        Ok(plans)
    }

    /// Zig `genStructLiteral` (21212).
    pub(crate) fn gen_struct_literal(&mut self, lit: &StructLiteral) -> Result<u32> {
        let mut layouts = LayoutAdapter::new(self.check);
        let decl = layouts
            .struct_decl_for_type(&lit.ty)
            .ok_or(Error::UnsupportedSabDirectFeature(
                "struct literal type has no struct decl".to_string(),
            ))?;
        if decl.is_opaque {
            return Err(Error::UnsupportedSabDirectFeature(
                "struct literal of an opaque decl".to_string(),
            ));
        }
        if decl.is_union {
            return self.gen_union_literal(lit, &decl);
        }

        let dst = self.b().new_tmp();
        self.emit_alloc(dst, layout::struct_size(&decl));
        // Zig: markObjectPtrReg(dst) — provenance bookkeeping with
        // no baseline RegState variant; see REPORT SAB-D165.

        let update_reg = match &lit.update_expr {
            Some(expr) => Some(self.gen_expr(expr)?),
            None => None,
        };
        // Zig defers releaseExprResultIfNeeded(update) past the
        // whole body, including the pending-move loop below; the
        // port runs it at the same relative point.
        let plans = self.struct_literal_field_plans(&decl, lit)?;
        let mut pending_moved_fields: Vec<u32> = Vec::new();

        for (field_index, plan) in plans.iter().enumerate() {
            // Fixed-array fields: element-wise copy into the inline
            // field region (Zig 21247–21265).
            if let Type::Array(arr) = &plan.field_ty {
                if plan.source != FieldSource::Explicit {
                    return Err(Error::UnsupportedSabDirectFeature(
                        "update-sourced fixed-array struct field".to_string(),
                    ));
                }
                let value = plan.value.ok_or(Error::UnsupportedSabDirectFeature(
                    "explicit array field without a value".to_string(),
                ))?;
                let value_reg = self.gen_expr(value)?;
                self.store_array_value_into_struct_field(
                    dst,
                    plan.layout.offset,
                    arr,
                    value_reg,
                )?;
                self.release_copied_array_field_source(value, value_reg, &plan.field_ty)?;
                continue;
            }

            let prim = layout::storage_prim_type(&plan.field_ty)?;
            let field_is_copy_struct = {
                let mut facts = CopyFacts::new(self.check);
                facts.type_is_copy_struct(&plan.field_ty)
            };
            let mut transfer = plan_field_transfer(plan, field_is_copy_struct);
            let mut copy_elided_move = false;
            if transfer == FieldTransfer::DeepCopy && plan.source == FieldSource::Explicit {
                if let Some(Node::Identifier(name)) = plan.value {
                    let later: Vec<&Node> = plans[field_index + 1..]
                        .iter()
                        .filter(|p| p.source == FieldSource::Explicit)
                        .filter_map(|p| p.value)
                        .collect();
                    if !identifier_used_in_nodes(name, &later) {
                        transfer = FieldTransfer::Move;
                        copy_elided_move = true;
                    }
                }
            }

            match plan.source {
                FieldSource::Explicit => {
                    let value = plan.value.ok_or(Error::UnsupportedSabDirectFeature(
                        "explicit struct field without a value".to_string(),
                    ))?;
                    if transfer == FieldTransfer::DeepCopy {
                        let source_reg = self.gen_expr(value)?;
                        let copied = self.gen_copy_value(source_reg, &plan.field_ty)?;
                        self.emit_store(dst, plan.layout.offset, copied, prim);
                        self.release_copied_field_value(copied, &plan.field_ty)?;
                    } else {
                        let direct_const_string =
                            direct_field_uses_const_string_symbol(value, &plan.field_ty);
                        let value_reg = if direct_const_string {
                            let label = match value {
                                Node::Literal(sla_ast::Literal::StringVal(raw)) => {
                                    self.b().string_literal_const_label(raw)?
                                }
                                _ => unreachable!(),
                            };
                            self.b().intern(&label)
                        } else if transfer == FieldTransfer::Move
                            && matches!(value, Node::MoveExpr(_))
                        {
                            match value {
                                Node::MoveExpr(mv) => self.gen_expr(&mv.expr)?,
                                _ => unreachable!(),
                            }
                        } else {
                            self.gen_expr(value)?
                        };
                        let explicit_move = matches!(value, Node::MoveExpr(_));
                        let moved_value: &Node = match value {
                            Node::MoveExpr(mv) => &mv.expr,
                            other => other,
                        };
                        let moves_identifier = match moved_value {
                            Node::Identifier(_) => {
                                let copy = self.check.type_is_copy_value(&plan.field_ty);
                                stored_value_moves_identifier(moved_value, &plan.field_ty, copy)
                                    .is_some()
                                    || explicit_move
                                    || copy_elided_move
                            }
                            _ => false,
                        };
                        let shallow_candidate = transfer == FieldTransfer::Move
                            && !explicit_move
                            && !copy_elided_move
                            && !moves_identifier
                            && layouts.struct_decl_for_type(&plan.field_ty).is_some()
                            && {
                                let mut facts = CopyFacts::new(self.check);
                                facts.type_is_shallow_copy_call_arg_value(&plan.field_ty, 0)
                            };
                        if shallow_candidate {
                            let copied =
                                self.gen_shallow_copy_call_arg_value(value_reg, &plan.field_ty)?;
                            self.emit_store(dst, plan.layout.offset, copied, prim);
                            self.emit_consumed_marker(copied);
                            self.release_moved_shallow_copy_source(
                                value,
                                value_reg,
                                &plan.field_ty,
                            )?;
                        } else if direct_const_string {
                            let symbol = self.b().symbol_name(value_reg).to_string();
                            self.emit_store_text(
                                dst,
                                plan.layout.offset,
                                &format!("&{symbol}"),
                                prim,
                            );
                        } else {
                            self.emit_store(dst, plan.layout.offset, value_reg, prim);
                        }
                        if transfer == FieldTransfer::Move {
                            match moved_value {
                                Node::Identifier(name) => {
                                    if moves_identifier {
                                        if let Some(reg) = self.state.frame.local_reg(name) {
                                            if !pending_moved_fields.contains(&reg) {
                                                pending_moved_fields.push(reg);
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    if expr_result_needs_release(moved_value) {
                                        self.emit_consumed_marker(value_reg);
                                        self.mark_consumed(value_reg);
                                    }
                                }
                            }
                        } else if !direct_const_string {
                            self.release_stored_expr_result_if_needed(
                                value,
                                value_reg,
                                &plan.field_ty,
                            )?;
                        }
                    }
                }
                FieldSource::Update => {
                    let src = update_reg.ok_or(Error::UnsupportedSabDirectFeature(
                        "update-sourced field without an update register".to_string(),
                    ))?;
                    let loaded = self.b().new_tmp();
                    self.emit_load(loaded, src, plan.layout.offset, prim);
                    match transfer {
                        FieldTransfer::Direct => {
                            self.emit_store(dst, plan.layout.offset, loaded, prim);
                            if plan.release_loaded && !self.is_local_reg(loaded) {
                                self.emit_release(loaded)?;
                            }
                        }
                        FieldTransfer::DeepCopy => {
                            let copied = self.gen_copy_value(loaded, &plan.field_ty)?;
                            self.emit_store(dst, plan.layout.offset, copied, prim);
                            self.release_copied_field_value(copied, &plan.field_ty)?;
                            if plan.release_loaded && !self.is_local_reg(loaded) {
                                self.emit_release(loaded)?;
                            }
                        }
                        FieldTransfer::Move => {
                            self.emit_store(dst, plan.layout.offset, loaded, prim);
                        }
                    }
                }
            }
        }

        // Zig emits the SAB-visible `move_` for every local whose
        // ownership transferred into the struct (pending set above).
        for reg in pending_moved_fields {
            self.emit_move(reg);
        }
        if let (Some(expr), Some(reg)) = (&lit.update_expr, update_reg) {
            self.release_expr_result_if_needed(expr, reg)?;
        }
        Ok(dst)
    }

    /// Zig `genUnionLiteral` (21172). The ManuallyDrop-new field
    /// arm emits a std macro fragment (F-group machine); it stops
    /// loudly with that attribution.
    pub(crate) fn gen_union_literal(
        &mut self,
        lit: &StructLiteral,
        decl: &StructDecl,
    ) -> Result<u32> {
        let dst = self.b().new_tmp();
        self.emit_alloc(dst, layout::struct_size(decl));
        for literal_field in &lit.fields {
            let layout = layout::struct_field_layout_detailed(decl, &literal_field.name)
                .collapsed()
                .ok_or(Error::UnsupportedSabDirectFeature(
                    "union literal field has no layout".to_string(),
                ))?;
            let field_ty = decl
                .fields
                .iter()
                .find(|f| f.name == literal_field.name)
                .map(|f| (*f.ty).clone())
                .ok_or(Error::UnsupportedSabDirectFeature(
                    "union literal field not in decl".to_string(),
                ))?;
            if layout::user_defined_generic_inner(&field_ty, "ManuallyDrop").is_some() {
                if let Node::CallExpr(call) = &*literal_field.value {
                    let _ = call;
                    return Err(unsupported(
                        "ManuallyDrop::new field in a union literal (MANUALLY_DROP_U64_NEW fragment)",
                        "F fragments",
                        "21185-21205",
                    ));
                }
            }
            let val_reg = self.gen_expr(&literal_field.value)?;
            let prim = layout::storage_prim_type(&field_ty)?;
            let _ = layout.ty;
            self.emit_store(dst, layout.offset, val_reg, prim);
            if !self.is_local_reg(val_reg) {
                self.emit_release(val_reg)?;
            }
        }
        Ok(dst)
    }

    /// Zig `releaseMovedShallowCopySource` (21473).
    pub(crate) fn release_moved_shallow_copy_source(
        &mut self,
        value: &Node,
        value_reg: u32,
        value_ty: &Type,
    ) -> Result<()> {
        if let Node::Identifier(name) = value {
            let copy = self.check.type_is_copy_value(value_ty);
            if stored_value_moves_identifier(value, value_ty, copy).is_none() {
                return Ok(());
            }
            if let Some(local_reg) = self.state.frame.local_reg(name) {
                self.emit_release(local_reg)?;
            }
            return Ok(());
        }
        if expr_result_needs_release(value) {
            self.emit_release(value_reg)?;
        }
        Ok(())
    }
}

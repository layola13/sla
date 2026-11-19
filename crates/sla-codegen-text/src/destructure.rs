//! `let (a, b) = value;` statement emission, mirrored from
//! codegen.zig's `genStmt` `let_destructure_stmt` arm (the arm the
//! F-1 registration cites as codegen.zig:11000; in the current Zig
//! tree it sits in the genStmt switch). Two shapes:
//!
//! - tuple: evaluate the value once, load each component out of
//!   the tuple storage at its `tupleFieldLayout` offset into the
//!   pattern binding (through a stack cell when the storage plan
//!   marks the binding addressable), then retire the value
//!   register — except a Copy-tuple identifier root, which the
//!   checker does not consume (DEF-TUPLE-COPY-EMIT).
//! - slice/array: load the base pointer + length, load each named
//!   element at its running byte offset, and materialise the rest
//!   binding as a fresh `Slice` cell over the remaining elements.
//!
//! The `ch.recv()` statement shape never reaches this arm in the
//! Zig pipeline (syntax_lowering `emitRecvBinding` binds it
//! directly); the text lane has no lowering stage, so a recv value
//! flows through `gen_expr` like any other call and surfaces the
//! Chan method-surface layer underneath — that layer is a separate
//! follow-up, not this arm's jurisdiction.
//!
//! The mpsc channel-constructor special form at the top of the
//! Zig arm has no counterpart here: the text lane has no
//! `isMpscChannelCall` lowering helper, and no in-scope corpus
//! name takes that shape.

use sla_ast::{Node, Type};
use sla_lowering::{call_arg_needs_release, is_discard_name};

use crate::codegen::{type_size, type_string, Codegen};
use crate::layout;
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// codegen.zig `genStmt` `let_destructure_stmt` arm.
    pub(crate) fn gen_let_destructure_stmt(&mut self, stmt: &Node) -> Result<()> {
        let let_stmt = match stmt {
            Node::LetDestructureStmt(l) => l,
            _ => unreachable!(),
        };
        let value_reg = self.gen_expr(&let_stmt.value)?;
        let value_ty = self
            .expr_type(&let_stmt.value)
            .ok_or(crate::TextError::MissingType {
                context: "let-destructure value type".to_string(),
            })?;
        if let_stmt.is_slice {
            self.gen_destructure_slice(let_stmt, &value_reg, &value_ty)?;
            if call_arg_needs_release(&let_stmt.value) {
                self.emit_release(&value_reg)?;
            } else if matches!(
                &*let_stmt.value,
                Node::Identifier(_) | Node::MoveExpr(_)
            ) && (matches!(&value_ty, Type::Array(_))
                || !self.type_is_copy_value(&value_ty))
                && !sla_lowering::is_borrow_like_type(&value_ty)
            {
                // LINE-CENSUS-MEMLEAK: mirror the tuple path's
                // !copy_root release with the checker's caliber
                // (check_let_destructure, FB-010): a non-Copy,
                // non-borrow-like identifier root is CONSUMED
                // by the destructure in the checker, so it
                // leaves the exit cleanup list — but this arm
                // never retired its register, stranding it at
                // function exit (MemoryLeak 1012, `a` in
                // test_unit_let_destructure; arrays are not
                // Copy in the checker, so an array root always
                // lands here).
                self.emit_release(&value_reg)?;
            }
            return Ok(());
        }
        let elems = match &value_ty {
            Type::Tuple(t) => t.elems.clone(),
            _ => {
                return unsupported(
                    "let-destructure of non-tuple value",
                    "codegen.zig:11000 (let_destructure_stmt emission)",
                )
            }
        };
        for (i, name) in let_stmt.names.iter().enumerate() {
            let (offset, elem_ty) =
                layout::tuple_field_layout(&elems, i).ok_or_else(|| {
                    crate::TextError::Unsupported {
                        feature: format!("tuple field layout {i}"),
                        zig_ref: "lowering_rules.zig (tupleFieldLayout)",
                    }
                })?;
            if !is_discard_name(name) {
                self.remember_local_binding_type(name, &elem_ty);
            }
            if !is_discard_name(name)
                && self.binding_needs_addressable_storage(name, &elem_ty)
            {
                // DEF-TEXT-VIEW-STORE (Zig arm comment): a component
                // binding the storage plan marks addressable must
                // live in a stack cell exactly like the plain-let
                // arm materialises it; binding the loaded component
                // into a plain register made every later read
                // dereference the component value itself.
                let comp_reg = self.new_tmp();
                self.ins(&format!(
                    "{comp_reg} = load {value_reg}+{offset} as {}",
                    type_string(&elem_ty)
                ));
                self.stack_alloc_bindings.insert(name.clone());
                self.ins(&format!(
                    "{name} = stack_alloc {}",
                    type_size(&elem_ty)
                ));
                self.ins(&format!(
                    "store {name}+0, {comp_reg} as {}",
                    type_string(&elem_ty)
                ));
                self.emit_slot_transferred_retire(&elem_ty, &comp_reg)?;
            } else {
                self.ins(&format!(
                    "{name} = load {value_reg}+{offset} as {}",
                    type_string(&elem_ty)
                ));
            }
        }
        // DEF-TUPLE-COPY-EMIT: when the destructured root is an
        // identifier of a Copy tuple (checker caliber
        // `typeIsCopyBinding`), the checker does not consume the
        // root, so the emitter must not retire its register either.
        // Fresh temps and moved (non-Copy) roots keep the release.
        let copy_root = matches!(&*let_stmt.value, Node::Identifier(_))
            && self.type_is_copy_value(&value_ty);
        if !copy_root {
            self.emit_release(&value_reg)?;
        }
        Ok(())
    }

    /// The `is_slice` half of the Zig arm: elements load out of the
    /// array/slice base at a running byte offset; the rest binding
    /// becomes a fresh `Slice` cell over the tail.
    fn gen_destructure_slice(
        &mut self,
        let_stmt: &sla_ast::LetDestructureStmt,
        value_reg: &str,
        value_ty: &Type,
    ) -> Result<()> {
        let (elem_ty, array_len) = match value_ty {
            Type::Array(arr) => ((*arr.elem).clone(), Some(arr.len)),
            Type::UserDefined(u) if u.name == "Slice" && u.generics.len() == 1 => {
                (u.generics[0].clone(), None)
            }
            _ => {
                return unsupported(
                    "slice destructure of non-array/slice value",
                    "codegen.zig:11000 (let_destructure_stmt emission)",
                )
            }
        };
        let base_ptr = self.new_tmp();
        let len_reg = self.new_tmp();
        if let Some(len) = array_len {
            self.ins(&format!("{base_ptr} = &{value_reg}"));
            self.emit_int_const(&len_reg, len as i64);
        } else {
            self.ins(&format!("{base_ptr} = load {value_reg}+Slice_ptr as ptr"));
            self.ins(&format!("{len_reg} = load {value_reg}+Slice_len as u64"));
        }
        let elem_size = type_size(&elem_ty);
        let mut offset = 0usize;
        for name in &let_stmt.names {
            let reg = self.new_tmp();
            self.ins(&format!(
                "{reg} = load {base_ptr}+{offset} as {}",
                type_string(&elem_ty)
            ));
            if !is_discard_name(name) {
                self.remember_local_binding_type(name, &elem_ty);
                self.ins(&format!("{name} = {reg}"));
            }
            offset += elem_size;
        }
        if let Some(rest_name) = &let_stmt.rest_name {
            if is_discard_name(rest_name) {
                self.emit_release(&base_ptr)?;
                self.emit_release(&len_reg)?;
                if let Some(rest_alias) = &let_stmt.rest_alias {
                    if !is_discard_name(rest_alias) {
                        self.ins(&format!("{rest_alias} = {base_ptr}"));
                    }
                }
                return Ok(());
            }
            let rest_ptr = self.new_tmp();
            self.ins(&format!("{rest_ptr} = ptr_add {base_ptr}, {offset}"));
            let rest_len = self.new_tmp();
            self.ins(&format!(
                "{rest_len} = sub {len_reg}, {}",
                let_stmt.names.len()
            ));
            self.stack_alloc_bindings.insert(rest_name.clone());
            self.ins(&format!("{rest_name} = stack_alloc Slice_SIZE"));
            self.ins(&format!(
                "EXPAND SLICE_NEW {rest_name}, {rest_ptr}, {rest_len}"
            ));
            let slice_ty = Type::UserDefined(sla_ast::UserDefined {
                name: "Slice".to_string(),
                generics: vec![elem_ty.clone()],
            });
            self.remember_local_binding_type(rest_name, &slice_ty);
            if let Some(rest_alias) = &let_stmt.rest_alias {
                if !is_discard_name(rest_alias) {
                    self.ins(&format!("{rest_alias} = {rest_name}"));
                }
            }
            self.emit_release(&rest_ptr)?;
            self.emit_release(&rest_len)?;
        }
        self.emit_release(&base_ptr)?;
        self.emit_release(&len_reg)?;
        Ok(())
    }

    /// codegen.zig `emitSlotTransferredRetire`: a component loaded
    /// out of the destructured root into an addressable cell — the
    /// loaded register is released when it is a borrow (or a borrow
    /// source temp), otherwise forgotten as a moved value. (The Zig
    /// condition's `refcell_borrow_handles` disjunct has no
    /// counterpart table in the text lane.)
    fn emit_slot_transferred_retire(&mut self, ty: &Type, reg: &str) -> Result<()> {
        if matches!(ty, Type::Borrow(_)) || self.borrow_source_temps.contains_key(reg) {
            self.emit_release(reg)
        } else {
            self.emit_forget_moved_value(reg)
        }
    }
}

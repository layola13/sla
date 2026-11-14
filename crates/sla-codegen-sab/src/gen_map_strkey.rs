//! String-key routing and materialization for the map
//! probes (split from gen_map_call.rs for the 700-line cap):
//! the content-keyed (`_STR`) routing decision, the owned
//! `String` handle materialization for literal keys, and the
//! probe-key retirement. See INTERIM2 stage 2B for the
//! design record (routing landed; the materialization's
//! host-verification signature is at the morphology
//! stop-loss, ruling with the main session).

use sla_ast::{Literal, Node, Type};

use crate::driver::Driver;
use crate::error::Result;
use crate::gen_method_call::{is_string_type, map_types};
use crate::ir::PrimType;

impl<'c> Driver<'c> {
    /// `gen_map_key_reg` with the content-keyed (`_STR`) routing
    /// flag: a literal key then materializes as an owned `String`
    /// handle instead of a borrow-only slice cell, because the
    /// `_STR` std functions read the key through the Vec-shaped
    /// handle layout and store the handle pointer itself.
    pub(crate) fn gen_map_key_reg_mode(
        &mut self,
        arg: &Node,
        string_content: bool,
    ) -> Result<u32> {
        // Zig genMapKeyReg's string arm (21785-21795): a literal
        // key materializes through genStringLiteral (its const
        // label is cached by the builder, so repeated keys share
        // the bytes).
        if let Node::Literal(Literal::StringVal(text)) = arg {
            let text = text.clone();
            if string_content {
                return self.gen_owned_string_key(&text);
            }
            return self.gen_string_literal(&text);
        }
        self.gen_expr(arg)
    }

    /// Whether this map key must route through the content-keyed
    /// `_STR` std variants: either the map's declared key type is
    /// `String`, or the key argument is a string literal. The
    /// literal arm matters because the checker types a bare
    /// string literal as a raw pointer, so an unannotated
    /// `HashMap::new()` never shows `String` as its key type even
    /// when every key is a string; routing by the declared type
    /// alone sends those keys to the pointer-keyed generic map,
    /// whose per-materialization handle addresses can never match
    /// (overwrite duplicates, lookup always misses).
    pub(crate) fn map_key_string_content(&self, recv_ty: &Type, key_arg: &Node) -> bool {
        if let Some((key_ty, _)) = map_types(recv_ty, "HashMap") {
            if is_string_type(&key_ty) {
                return true;
            }
        }
        matches!(key_arg, Node::Literal(Literal::StringVal(_)))
    }

    /// Materialize a string literal as an owned `String` handle
    /// (the Vec-shaped {ptr, cap, len} cell the `_STR` map
    /// functions read): slice cell first (genStringLiteral form),
    /// then direct calls — with_capacity, reserve (the standard
    /// `^handle` rebirth), mem_copy of the literal bytes, and the
    /// length store at +16. Direct calls rather than the
    /// STRING_BUF_FROM_STR fragment: that macro assigns its out
    /// param from a call result, and the fragment machine does
    /// not thread a call-result out param back into the named
    /// register (the named key register stayed unassigned and
    /// its later probe tripped the host verifier).
    pub(crate) fn gen_owned_string_key(&mut self, text: &str) -> Result<u32> {
        let slice_reg = self.gen_string_literal(text)?;
        self.ensure_method_std_deps(
            "sa_std/alloc/vec.sa",
            &["sa_vec_with_capacity", "sa_vec_reserve", "sa_vec_free"],
        )?;
        self.ensure_method_std_deps("sa_std/core/mem.sa", &["sa_mem_copy"])?;
        let len_reg = self.b().new_tmp();
        self.b().record_reg(len_reg);
        self.emit_load(len_reg, slice_reg, 8, PrimType::U64);
        let src_reg = self.b().new_tmp();
        self.b().record_reg(src_reg);
        self.emit_load(src_reg, slice_reg, 0, PrimType::Ptr);
        let handle = self.b().new_tmp();
        self.b().record_reg(handle);
        // Define the handle register with a structured
        // instruction before any text call body names it: a
        // register whose first definition is a text-body call
        // result is not reliably bound by name for the body
        // operands when the host flattens text bodies at load
        // (the vec_push precedent only names registers that
        // structured code defined first).
        self.b().emit_assign_imm(handle, 0);
        let len_name = self.b().symbol_name(len_reg).to_string();
        let handle_name = self.b().symbol_name(handle).to_string();
        self.emit_method_call_body(
            Some(handle),
            format!("@sa_vec_with_capacity({len_name}, 1)"),
        )?;
        self.emit_method_call_body(
            Some(handle),
            format!("@sa_vec_reserve(^{handle_name}, {len_name}, 1)"),
        )?;
        let borrow_reg = self.b().new_tmp();
        self.emit_borrow_reg(borrow_reg, handle, "read");
        let buf_reg = self.b().new_tmp();
        self.b().record_reg(buf_reg);
        self.emit_load(buf_reg, borrow_reg, 0, PrimType::Ptr);
        let src_name = self.b().symbol_name(src_reg).to_string();
        let buf_name = self.b().symbol_name(buf_reg).to_string();
        self.emit_method_call_body(
            None,
            format!("@sa_mem_copy(&{buf_name}, &{src_name}, {len_name})"),
        )?;
        self.emit_store(borrow_reg, 16, len_reg, PrimType::U64);
        self.emit_release(borrow_reg)?;
        self.emit_release(buf_reg)?;
        self.emit_release(src_reg)?;
        self.emit_release(len_reg)?;
        Ok(handle)
    }

    /// Retire a probe key this call materialized as an owned
    /// String: the `_STR` probe does not adopt the key, so free
    /// the buffer with `@sa_vec_free(^key)`. That call consumes
    /// the register and is itself the retire (the canonical
    /// `emit_owned_buffer_free` shape): only mark the register
    /// released in the builder bookkeeping - an emitted
    /// `release` after the consuming free traps UseAfterMove
    /// in the host verifier.
    pub(crate) fn retire_owned_probe_key(&mut self, key_reg: u32) -> Result<()> {
        self.ensure_method_std_deps("sa_std/alloc/vec.sa", &["sa_vec_free"])?;
        let key_name = self.b().symbol_name(key_reg).to_string();
        self.emit_method_call_body(None, format!("@sa_vec_free(^{key_name})"))?;
        crate::release_prim::mark_released(&mut self.state, key_reg);
        Ok(())
    }
}

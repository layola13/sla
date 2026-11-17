//! `Chan<T>` method surface for the text lane (goroutine syntax
//! batch): `send` / `recv` / `destroy` / `close` / `len` / `cap`.
//!
//! In the Zig pipeline these calls never reach codegen as method
//! calls — syntax_lowering.zig `emitChanCall` rewrites them into
//! `sa_chan_*` extern calls over a fixed 16-byte staging slot
//! (send stages the payload into the slot, recv reads it back
//! out, and every call's status word is gated by an rc check
//! with a per-op panic code). The text lane has no
//! syntax-lowering stage, so the same instruction sequences are
//! emitted here directly, in the lowering's order and with its
//! panic codes (60912 send, 60913 close, 60914 destroy, 60915
//! recv, 60917 len, 60918 cap; `Chan::new`'s 60911 lives in
//! call.rs).
//!
//! Payload classes mirror the lowering's `payloadIsWord` /
//! `payloadSendViaView` and the checker's `payloadStorageSize`
//! (syntax_lowering.zig / type_checker.zig):
//!
//! * word payloads (primitives, raw pointers, and decl-less
//!   handle types — String, Vec, Chan, Box, ...): the register
//!   holds the head word itself. Non-pointer word sends MOVE
//!   the word into the slot (the lowering's A-form view store);
//!   raw-pointer sends instead byte-copy the word out of the
//!   value's home, so the sender keeps its binding (DEF-ACL
//!   borrow-send). Receives load the word back out of the slot.
//! * aggregate payloads (structs, tuples, enums, arrays) are
//!   address-resident: sends byte-copy the storage into the
//!   slot; receives bind the payload to the slot address
//!   itself (the lowering's address cast), which the tuple
//!   materialisation stores as the element word.
//!
//! `recv` materialises the `(payload, closed)` pair as a real
//! tuple cell (the tuple-literal form: `alloc` + element
//! stores), so destructuring lets, plain lets and projections
//! all read it through the ordinary tuple layout. The slot is
//! zeroed before the call so a closed-channel receive reads a
//! zero payload, matching the lowering's zero-filled slot
//! array.
//!
//! Register discipline (proven by the CHANNEW / F-1 lines):
//! `stack_alloc` cells are never explicitly released (the host
//! frees them at scope exit; an explicit `!` is a StackEscape),
//! and a value whose ownership moved into the slot is settled
//! with the forget form, never a drop.

use sla_ast::{CallExpr, Primitive, TupleType, Type};

use crate::codegen::{type_string, Codegen};
use crate::into::is_heap_handle_type;
use crate::layout;
use crate::Result;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChanMethod {
    Send,
    Recv,
    Destroy,
    Close,
    Len,
    Cap,
}

impl<'c> Codegen<'c> {
    /// Detect a `Chan<T>` method call (the receiver rides
    /// args[0], the checker-leaf convention) and return its
    /// kind plus the payload type. Anything else returns None
    /// and the ordinary dispatch proceeds.
    pub(crate) fn chan_method(&self, call: &CallExpr) -> Option<(ChanMethod, Type)> {
        let kind = match call.func_name.as_str() {
            "send" => ChanMethod::Send,
            "recv" => ChanMethod::Recv,
            "destroy" => ChanMethod::Destroy,
            "close" => ChanMethod::Close,
            "len" => ChanMethod::Len,
            "cap" => ChanMethod::Cap,
            _ => return None,
        };
        if call.callee.is_some() || call.associated_target.is_some() {
            return None;
        }
        let want_args = match kind {
            ChanMethod::Send => 2,
            _ => 1,
        };
        if call.args.len() != want_args {
            return None;
        }
        match self.expr_type(&call.args[0])? {
            Type::UserDefined(u) if u.name == "Chan" && u.generics.len() == 1 => {
                Some((kind, u.generics[0].clone()))
            }
            _ => None,
        }
    }

    /// Emit a Chan method call in expression position; returns
    /// the register holding the call's value (the status word
    /// for send, the materialised (payload, closed) tuple cell
    /// for recv, the count for len/cap, an unused temp for the
    /// void destroy/close — the same dummy the surface Void arm
    /// returns).
    pub(crate) fn gen_chan_method_call(
        &mut self,
        call: &CallExpr,
        kind: ChanMethod,
        payload_ty: &Type,
    ) -> Result<String> {
        match kind {
            ChanMethod::Send => self.gen_chan_send(call, payload_ty),
            ChanMethod::Recv => self.gen_chan_recv(call, payload_ty),
            ChanMethod::Destroy | ChanMethod::Close => {
                let (extern_name, code) = if kind == ChanMethod::Destroy {
                    ("sa_chan_destroy", 60914)
                } else {
                    ("sa_chan_close", 60913)
                };
                let handle = self.gen_expr(&call.args[0])?;
                let rc = self.new_tmp();
                self.ins(&format!("{rc} = call @{extern_name}({handle})"));
                self.emit_chan_rc_check(&rc, code)?;
                self.emit_release(&rc)?;
                Ok(self.new_tmp())
            }
            ChanMethod::Len | ChanMethod::Cap => {
                let (extern_name, code) = if kind == ChanMethod::Len {
                    ("sa_chan_len", 60917)
                } else {
                    ("sa_chan_cap", 60918)
                };
                let handle = self.gen_expr(&call.args[0])?;
                let cell = self.new_tmp();
                self.ins(&format!("{cell} = stack_alloc 8"));
                self.ins(&format!("store {cell}+0, 0 as u64"));
                let rc = self.new_tmp();
                self.ins(&format!("{rc} = call @{extern_name}({handle}, &{cell})"));
                self.emit_chan_rc_check(&rc, code)?;
                self.emit_release(&rc)?;
                let val = self.new_tmp();
                self.ins(&format!("{val} = load {cell}+0 as u64"));
                Ok(val)
            }
        }
    }

    /// syntax_lowering.zig `emitChanCall` `.send` arm.
    fn gen_chan_send(&mut self, call: &CallExpr, payload_ty: &Type) -> Result<String> {
        let size = self.chan_payload_storage_size(payload_ty).ok_or_else(|| {
            crate::TextError::Unsupported {
                feature: format!("chan send payload size for {payload_ty:?}"),
                zig_ref: "type_checker.zig payloadStorageSize",
            }
        })?;
        let handle = self.gen_expr(&call.args[0])?;
        let slot = self.new_tmp();
        self.ins(&format!("{slot} = stack_alloc 16"));
        self.ins(&format!("store {slot}+0, 0 as u64"));
        self.ins(&format!("store {slot}+8, 0 as u64"));
        let value_node = &call.args[1];
        if matches!(payload_ty, Type::Pointer(_) | Type::FnPtr(_)) {
            // Pointer word: store the pointer value itself
            // into the slot — a borrow-copy, the sender keeps
            // its binding (DEF-ACL). The Zig form mem-copies
            // out of the value's home address, but in this
            // lane a pointer value is a register-resident
            // word: copying from `&val` would read the
            // pointee, not the pointer.
            let val = self.gen_expr(value_node)?;
            self.ins(&format!(
                "store {slot}+0, {val} as {}",
                type_string(payload_ty)
            ));
            if sla_lowering::call_arg_needs_release(value_node) {
                self.emit_release(&val)?;
            }
        } else if self.chan_payload_is_word(payload_ty) {
            // A form (payloadSendViaView): the payload word
            // moves into the slot.
            let val = self.gen_expr(value_node)?;
            self.ins(&format!(
                "store {slot}+0, {val} as {}",
                type_string(payload_ty)
            ));
            if self.type_is_copy_value(payload_ty) {
                // Copy payload: the source binding stays live;
                // only a fresh temp needs retiring.
                if sla_lowering::call_arg_needs_release(value_node) {
                    self.emit_release(&val)?;
                }
            } else {
                self.emit_forget_moved_value(&val)?;
            }
        } else {
            // Borrow-copy staging (emitMemCopy): pointer words
            // copy out of the value's home so the sender keeps
            // its binding; aggregate storage copies out of the
            // value's own cell, and the husk is forgotten (its
            // contents now belong to the channel).
            let val = self.gen_expr(value_node)?;
            self.ins(&format!("call @sa_mem_copy(&{slot}, &{val}, {size})"));
            self.emit_forget_moved_value(&val)?;
        }
        let rc = self.new_tmp();
        // The runtime contract spells the slot parameter
        // `*value: ptr`: a `*`-spelled parameter is passed in
        // deref form at the call site (the go_spawn.rs
        // precedent for `*entry`), unlike the `&out` borrows
        // of make/recv/len/cap.
        self.ins(&format!("{rc} = call @sa_chan_send({handle}, *{slot})"));
        self.emit_chan_rc_check(&rc, 60912)?;
        Ok(rc)
    }

    /// syntax_lowering.zig `emitChanCall` `.recv` arm, with the
    /// (payload, closed) pair materialised as a tuple cell (see
    /// the module doc for why the text lane materialises where
    /// the lowering binds directly).
    fn gen_chan_recv(&mut self, call: &CallExpr, payload_ty: &Type) -> Result<String> {
        let _ = self.chan_payload_storage_size(payload_ty).ok_or_else(|| {
            crate::TextError::Unsupported {
                feature: format!("chan recv payload size for {payload_ty:?}"),
                zig_ref: "type_checker.zig payloadStorageSize",
            }
        })?;
        let handle = self.gen_expr(&call.args[0])?;
        let slot = self.new_tmp();
        self.ins(&format!("{slot} = stack_alloc 16"));
        self.ins(&format!("store {slot}+0, 0 as u64"));
        self.ins(&format!("store {slot}+8, 0 as u64"));
        let closed_cell = self.new_tmp();
        self.ins(&format!("{closed_cell} = stack_alloc 8"));
        self.ins(&format!("store {closed_cell}+0, 0 as u64"));
        let rc = self.new_tmp();
        self.ins(&format!(
            "{rc} = call @sa_chan_recv({handle}, &{slot}, &{closed_cell})"
        ));
        self.emit_chan_rc_check(&rc, 60915)?;
        self.emit_release(&rc)?;
        let elems = vec![
            payload_ty.clone(),
            Type::Primitive(Primitive::Boolean),
        ];
        let tuple = TupleType {
            elems: elems.clone(),
        };
        let cell = self.new_tmp();
        self.ins(&format!(
            "{cell} = alloc {}",
            sla_lowering::tuple_abi_size(&tuple)
        ));
        if self.chan_payload_is_word(payload_ty) {
            let w = self.new_tmp();
            self.ins(&format!(
                "{w} = load {slot}+0 as {}",
                type_string(payload_ty)
            ));
            self.ins(&format!(
                "store {cell}+0, {w} as {}",
                type_string(payload_ty)
            ));
            // The word now belongs to the tuple cell; settle
            // the load temp the way a stored tuple element is
            // settled (forget for owning handles, plain retire
            // for copy words).
            if is_heap_handle_type(payload_ty) {
                self.emit_forget_moved_value(&w)?;
            } else {
                self.emit_release(&w)?;
            }
        } else {
            // Address-resident payload: the element word is
            // the slot address itself (the lowering's cast
            // form); the payload's bytes stay in the slot,
            // which outlives the receive in this frame.
            self.ins(&format!("store {cell}+0, {slot} as ptr"));
        }
        let (bool_off, _) =
            layout::tuple_field_layout(&elems, 1).ok_or_else(|| {
                crate::TextError::Unsupported {
                    feature: "chan recv tuple bool layout".to_string(),
                    zig_ref: "lowering_rules.zig (tupleFieldLayout)",
                }
            })?;
        let cw = self.new_tmp();
        self.ins(&format!("{cw} = load {closed_cell}+0 as u32"));
        let cb = self.new_tmp();
        self.ins(&format!("{cb} = ne {cw}, 0"));
        self.emit_release(&cw)?;
        self.ins(&format!("store {cell}+{bool_off}, {cb} as u8"));
        Ok(cell)
    }

    /// syntax_lowering.zig `emitRcCheck`: panic when the
    /// runtime status word is nonzero. The rc register itself
    /// is the caller's to settle (send returns it as its
    /// value).
    pub(crate) fn emit_chan_rc_check(&mut self, rc: &str, code: i64) -> Result<()> {
        let cond = self.new_tmp();
        self.ins(&format!("{cond} = ne {rc}, 0"));
        let fail_label = self.new_label("L_CHAN_RC_FAIL");
        let ok_label = self.new_label("L_CHAN_RC_OK");
        self.ins(&format!("br {cond} -> {fail_label}, {ok_label}"));
        self.wl(&format!("{fail_label}:"));
        self.ins(&format!("panic({code})"));
        self.wl(&format!("{ok_label}:"));
        self.emit_release(&cond)?;
        Ok(())
    }

    /// type_checker.zig `payloadStorageSize`: the byte count
    /// the runtime's fixed slot carries for this payload.
    pub(crate) fn chan_payload_storage_size(&self, ty: &Type) -> Option<usize> {
        match ty {
            Type::Primitive(p) => match p {
                Primitive::Boolean | Primitive::U8 | Primitive::I8 => Some(1),
                Primitive::U16 | Primitive::I16 => Some(2),
                Primitive::U32 | Primitive::I32 | Primitive::F32 => Some(4),
                Primitive::VoidType => None,
                _ => Some(8),
            },
            Type::Pointer(_) | Type::FnPtr(_) => Some(8),
            Type::Tuple(t) => Some(sla_lowering::tuple_abi_size(t)),
            Type::Array(a) => Some(layout::array_size(a)),
            Type::UserDefined(u) => {
                if let Some(decl) = self.check.struct_decl(&u.name) {
                    return Some(layout::struct_size(decl));
                }
                if let Some(decl) = self.enum_decl(&u.name) {
                    let mut max_span = 0usize;
                    for variant in &decl.variants {
                        let elems: Vec<Type> =
                            variant.fields.iter().map(|f| (*f.ty).clone()).collect();
                        let span = sla_lowering::tuple_abi_size(&TupleType { elems });
                        if span > max_span {
                            max_span = span;
                        }
                    }
                    return Some(8 + max_span);
                }
                // Decl-less handle type: the transferred value
                // is the head word itself.
                Some(8)
            }
            _ => None,
        }
    }

    /// syntax_lowering.zig `payloadIsWord`: primitives, raw
    /// pointers, and user-defined types with no struct/enum
    /// declaration (decl-less handles).
    pub(crate) fn chan_payload_is_word(&self, ty: &Type) -> bool {
        match ty {
            Type::Primitive(_) | Type::Pointer(_) | Type::FnPtr(_) => true,
            Type::UserDefined(u) => {
                self.check.struct_decl(&u.name).is_none()
                    && self.enum_decl(&u.name).is_none()
            }
            _ => false,
        }
    }
}

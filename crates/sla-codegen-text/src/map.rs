//! HashMap / HashSet bridge materialisation — the
//! DEF-HASHMAP-XTALK A-form, ported from codegen.zig's
//! `genHashMapKeyReg` (:14592), `genHashMapValueSlot` (:14625),
//! `genOwnedStringLiteral` (:14295) and the method-surface
//! insert/get arms (:21362-21414) that `mapMacroBridge`
//! (:13621) mirrors for imported-macro calls.
//!
//! The std table is a pointer-identity table: it stores the
//! value word verbatim, so the value a map entry owns must be
//! a per-entry HEAP cell (alloc 8) holding the value word —
//! never the raw value and never a per-call-site stack cell
//! (a shared stack cell made every loop-inserted entry alias
//! one address). Keys are materialised against the RECEIVER's
//! key type: a string literal for a String-keyed map becomes
//! an owned String handle (the Slice form misreads as a String
//! handle and the _STR macros segfault on it); a string
//! literal for any other key type reuses one cached slice slot
//! per distinct literal in the function, so identity probes
//! for the same literal hit the same slot.

use sla_ast::{Literal, Node, Type};
use sla_lowering::peel_borrow_pointer_type;

use crate::codegen::{escaped_string_byte_len, Codegen};
use crate::Result;

/// The materialised key register plus whether it is an owned
/// String handle built from a literal (Zig
/// `keyRegIsOwnedLiteralString`).
pub(crate) struct KeyReg {
    pub reg: String,
    pub owned_literal_string: bool,
}

pub(crate) fn is_string_key_type(ty: &Type) -> bool {
    matches!(
        peel_borrow_pointer_type(ty),
        Type::UserDefined(u) if u.name == "String"
    )
}

impl<'c> Codegen<'c> {
    /// Zig `genOwnedStringLiteral` (:14295): a fresh owned
    /// String handle {ptr, len, cap} from a string literal.
    pub(crate) fn gen_owned_string_literal(&mut self, value: &str) -> Result<String> {
        let label = self.new_string_const();
        self.ins(&format!("@const {label} = utf8:\"{value}\""));
        let len_reg = self.new_tmp();
        self.emit_int_const(&len_reg, escaped_string_byte_len(value) as i64);
        let bytes_reg = self.new_tmp();
        self.stack_alloc_bindings.insert(bytes_reg.clone());
        self.ins(&format!("{bytes_reg} = stack_alloc Slice_SIZE"));
        self.ins(&format!(
            "EXPAND SLICE_NEW {bytes_reg}, &{label}, {len_reg}"
        ));
        self.emit_release(&len_reg)?;
        let string_reg = self.new_tmp();
        self.ins(&format!(
            "EXPAND STRING_BUF_FROM_UTF8_UNCHECKED {string_reg}, {bytes_reg}"
        ));
        Ok(string_reg)
    }

    /// Zig `genHashMapKeyReg` (:14592) for the literal shapes;
    /// returns None for a non-literal argument (the caller
    /// generates it through its usual planned path).
    pub(crate) fn gen_hashmap_literal_key_reg(
        &mut self,
        arg: &Node,
        key_ty: Option<&Type>,
    ) -> Result<Option<KeyReg>> {
        let Node::Literal(Literal::StringVal(value)) = arg else {
            return Ok(None);
        };
        let value = value.clone();
        // DEF-IDX-B: a string literal used as a key in a
        // String-keyed map must be materialised as an owned
        // String handle. Never cached: each call site gets a
        // fresh owned String (an insert adopts it, so a cached
        // register would be adopted twice). BTree keys stay on
        // the Slice form: their key model is raw_ptr/slice by
        // design (isStringType is false for them at the Zig
        // call sites, which pass the map's key type — for a
        // BTreeMap the caller passes None here to force the
        // slice form).
        if let Some(kt) = key_ty {
            if is_string_key_type(kt) {
                let reg = self.gen_owned_string_literal(&value)?;
                return Ok(Some(KeyReg {
                    reg,
                    owned_literal_string: true,
                }));
            }
        }
        if let Some(slot) = self.hashmap_key_slots.get(&value) {
            return Ok(Some(KeyReg {
                reg: slot.clone(),
                owned_literal_string: false,
            }));
        }
        let slot = self.new_tmp();
        self.hashmap_key_slots.insert(value.clone(), slot.clone());
        let label = self.new_string_const();
        self.ins(&format!("@const {label} = utf8:\"{value}\""));
        let len_reg = self.new_tmp();
        self.emit_int_const(&len_reg, escaped_string_byte_len(&value) as i64);
        self.stack_alloc_bindings.insert(slot.clone());
        self.ins(&format!("{slot} = stack_alloc Slice_SIZE"));
        self.ins(&format!("EXPAND SLICE_NEW {slot}, &{label}, {len_reg}"));
        self.emit_release(&len_reg)?;
        Ok(Some(KeyReg {
            reg: slot,
            owned_literal_string: false,
        }))
    }

    /// Zig `emitHashMapValueCellsFreeText` (:14652): walk the
    /// slot array and return every FILLED entry's heap value
    /// cell to the allocator. Runs before MAP_CLEAR (and is
    /// what a map drop owes its entries): the std clear zeroes
    /// the slots, so a later walk finds nothing FILLED and no
    /// cell is freed twice.
    pub(crate) fn emit_hashmap_value_cells_free(&mut self, map_reg: &str) -> Result<()> {
        let slots_reg = self.new_tmp();
        self.stack_alloc_bindings.insert(slots_reg.clone());
        self.ins(&format!("{slots_reg} = stack_alloc 8"));
        let slots_val = self.new_tmp();
        self.ins(&format!(
            "{slots_val} = load {map_reg}+HashMap_slots as u64"
        ));
        self.ins(&format!("store {slots_reg}+0, {slots_val} as u64"));
        self.emit_release(&slots_val)?;
        let cap_reg = self.new_tmp();
        self.stack_alloc_bindings.insert(cap_reg.clone());
        self.ins(&format!("{cap_reg} = stack_alloc 8"));
        let cap_val0 = self.new_tmp();
        self.ins(&format!("{cap_val0} = load {map_reg}+HashMap_cap as u64"));
        self.ins(&format!("store {cap_reg}+0, {cap_val0} as u64"));
        self.emit_release(&cap_val0)?;
        let idx_reg = self.new_tmp();
        self.stack_alloc_bindings.insert(idx_reg.clone());
        self.ins(&format!("{idx_reg} = stack_alloc 8"));
        self.ins(&format!("store {idx_reg}+0, 0 as u64"));
        let head_label = self.new_label("L_MAP_CELLS_FREE_HEAD");
        let body_label = self.new_label("L_MAP_CELLS_FREE_BODY");
        let free_label = self.new_label("L_MAP_CELLS_FREE_ONE");
        let next_label = self.new_label("L_MAP_CELLS_FREE_NEXT");
        let end_label = self.new_label("L_MAP_CELLS_FREE_END");
        self.ins(&format!("{head_label}:"));
        let idx_val = self.new_tmp();
        self.ins(&format!("{idx_val} = load {idx_reg}+0 as u64"));
        let cap_val = self.new_tmp();
        self.ins(&format!("{cap_val} = load {cap_reg}+0 as u64"));
        let done = self.new_tmp();
        self.ins(&format!("{done} = eq {idx_val}, {cap_val}"));
        self.ins(&format!("br {done} -> {end_label}, {body_label}"));
        self.ins(&format!("{body_label}:"));
        self.ins(&format!("!{done}"));
        let slot_off = self.new_tmp();
        self.ins(&format!(
            "{slot_off} = mul {idx_val}, HashMapSlot_SIZE"
        ));
        let slots_base = self.new_tmp();
        self.ins(&format!("{slots_base} = load {slots_reg}+0 as u64"));
        let base = self.new_tmp();
        self.ins(&format!("{base} = add {slots_base}, {slot_off}"));
        self.emit_release(&slots_base)?;
        let state = self.new_tmp();
        self.ins(&format!(
            "{state} = load {base}+HashMapSlot_state as u64"
        ));
        let filled = self.new_tmp();
        self.ins(&format!("{filled} = eq {state}, HashMap_STATE_FILLED"));
        self.ins(&format!("br {filled} -> {free_label}, {next_label}"));
        self.ins(&format!("{free_label}:"));
        let cell_reg = self.new_tmp();
        self.ins(&format!(
            "{cell_reg} = load {base}+HashMapSlot_value as ptr"
        ));
        let free_reg = self.new_tmp();
        self.ins(&format!("{free_reg} = {cell_reg}"));
        self.ins(&format!("call @sa_mem_free(^{free_reg})"));
        self.emit_release(&cell_reg)?;
        self.ins(&format!("jmp {next_label}"));
        self.ins(&format!("{next_label}:"));
        self.ins(&format!("!{filled}"));
        self.ins(&format!("!{state}"));
        self.emit_release(&slot_off)?;
        self.emit_release(&base)?;
        let idx_next = self.new_tmp();
        self.ins(&format!("{idx_next} = load {idx_reg}+0 as u64"));
        let inc = self.new_tmp();
        self.ins(&format!("{inc} = add {idx_next}, 1"));
        self.ins(&format!("store {idx_reg}+0, {inc} as u64"));
        self.emit_release(&idx_next)?;
        self.emit_release(&inc)?;
        self.ins(&format!("jmp {head_label}"));
        self.ins(&format!("{end_label}:"));
        self.emit_release(&idx_val)?;
        self.emit_release(&cap_val)?;
        self.emit_release(&done)?;
        Ok(())
    }

    /// Zig `genHashMapValueSlot` (:14625): stage a value into
    /// its per-entry heap cell. The caller forgets the cell
    /// register into the map after the insert expansion (the
    /// cell's ownership moves into the map; a release would
    /// free the live cell the table now points at) and settles
    /// the value register itself (forget when the checker
    /// consumed it, release when it is a plain temp).
    pub(crate) fn gen_hashmap_value_slot(&mut self, value_reg: &str) -> String {
        let slot = self.new_tmp();
        self.ins(&format!("{slot} = alloc 8"));
        self.ins(&format!("store {slot}+0, {value_reg} as u64"));
        slot
    }
}

//! Deep clone dispatcher: Zig `genDeepCloneValue`
//! (codegen.zig:12421) and its family — `genVecCloneValue`
//! (:12149), `genVecCloneValueDeep` (:12544),
//! `genGrowVecCloneValueDeep` (:12557),
//! `genReplaceBufferElementsWithClones` (:12570),
//! `genBoxCloneValue` (:12465), `genArrayCloneValue` (:12484),
//! `genTupleCloneValue` (:12528), `deepCloneNeeded` (:12170),
//! `typeHasRecursiveClone` / `cloneTypeKey` / `cloneGraphReaches`
//! (:12211-12353), plus the record arm's target
//! `genShallowCopyCallArgValue` (:5705). A deep clone returns a
//! fresh register holding an independently-owned copy of the
//! source value, or None when the type is plain-copyable and the
//! caller copies the source word as-is. Map/set types have no
//! clone routine: the copy is refused with an UncloneableCopy
//! diagnostic, as in the Zig original. Recursive struct types
//! (whose clone graph reaches itself, e.g. RedisValue) route
//! through per-type `@sla_clone_<key>` helpers in Zig; that
//! helper-emission port is NOT in this module — the dispatcher
//! stays loud for them (see copy.rs's record-copyable gate).

use std::collections::HashSet;

use sla_ast::Type;
use sla_lowering::peel_borrow_pointer_type;

use crate::codegen::{type_size, type_string, Codegen};
use crate::layout;
use crate::{Result, TextError};

fn peeled_name(ty: &Type) -> Option<&str> {
    match peel_borrow_pointer_type(ty) {
        Type::UserDefined(u) => Some(u.name.as_str()),
        _ => None,
    }
}

fn generic_inner(ty: &Type, name: &str) -> Option<Type> {
    match peel_borrow_pointer_type(ty) {
        Type::UserDefined(u) if u.name == name => u.generics.first().cloned(),
        _ => None,
    }
}

fn is_string_ty(ty: &Type) -> bool {
    peeled_name(ty) == Some("String")
}

impl<'c> Codegen<'c> {
    /// Zig `deepCloneNeeded` (:12170): does cloning a value of
    /// this type require more than copying its word(s)?
    pub(crate) fn deep_clone_needed(&self, ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        if is_string_ty(ty) {
            return true;
        }
        if crate::place::vec_like_element_type(ty).is_some() {
            return true;
        }
        if generic_inner(ty, "Box").is_some()
            || generic_inner(ty, "Rc").is_some()
            || generic_inner(ty, "Arc").is_some()
        {
            return true;
        }
        match peel_borrow_pointer_type(ty) {
            Type::UserDefined(u) => {
                let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
                    return false;
                };
                decl.fields
                    .iter()
                    .any(|f| self.deep_clone_needed(&f.ty, depth + 1))
            }
            Type::Array(a) => self.deep_clone_needed(&a.elem, depth + 1),
            Type::Tuple(t) => t
                .elems
                .iter()
                .any(|e| self.deep_clone_needed(e, depth + 1)),
            _ => false,
        }
    }

    /// Zig `uncloneableTypeName` (type_checker.zig:4944): the
    /// first map/set type in the clone closure, if any.
    pub(crate) fn uncloneable_type_name(&self, ty: &Type, depth: usize) -> Option<&'static str> {
        if depth > 8 {
            return None;
        }
        match peeled_name(ty) {
            Some("HashMap") => return Some("HashMap"),
            Some("HashSet") => return Some("HashSet"),
            Some("BTreeMap") => return Some("BTreeMap"),
            Some("BTreeSet") => return Some("BTreeSet"),
            _ => {}
        }
        if let Some((_, elem)) = crate::place::vec_like_element_type(ty) {
            return self.uncloneable_type_name(&elem, depth + 1);
        }
        if let Some(inner) = generic_inner(ty, "Box") {
            return self.uncloneable_type_name(&inner, depth + 1);
        }
        match peel_borrow_pointer_type(ty) {
            Type::UserDefined(u) => {
                if let Some(decl) = self.enum_decl(&u.name).cloned() {
                    for variant in &decl.variants {
                        for field in &variant.fields {
                            if let Some(bad) = self.uncloneable_type_name(&field.ty, depth + 1) {
                                return Some(bad);
                            }
                        }
                    }
                    return None;
                }
                let decl = crate::place::struct_decl_lookup(self.check, &u.name)?;
                for field in &decl.fields {
                    if let Some(bad) = self.uncloneable_type_name(&field.ty, depth + 1) {
                        return Some(bad);
                    }
                }
                None
            }
            Type::Array(a) => self.uncloneable_type_name(&a.elem, depth + 1),
            Type::Tuple(t) => {
                for elem in &t.elems {
                    if let Some(bad) = self.uncloneable_type_name(elem, depth + 1) {
                        return Some(bad);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Zig `cloneTypeKey` (:12211): symbol-safe key for a type,
    /// built from its name plus generic arguments recursively.
    pub(crate) fn clone_type_key(&self, ty: &Type) -> String {
        let mut buf = String::new();
        self.append_clone_type_key(&mut buf, ty);
        buf.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    fn append_clone_type_key(&self, buf: &mut String, ty: &Type) {
        let peeled = peel_borrow_pointer_type(ty);
        if let Some((is_grow, elem)) = crate::place::vec_like_element_type(peeled) {
            buf.push_str(if is_grow { "GrowVec_" } else { "Vec_" });
            return self.append_clone_type_key(buf, &elem);
        }
        for (name, prefix) in [("Box", "Box_"), ("Rc", "Rc_"), ("Arc", "Arc_")] {
            if let Some(inner) = generic_inner(peeled, name) {
                buf.push_str(prefix);
                return self.append_clone_type_key(buf, &inner);
            }
        }
        match peeled {
            Type::UserDefined(u) => {
                buf.push_str(&u.name);
                for g in &u.generics {
                    buf.push_str("__");
                    self.append_clone_type_key(buf, g);
                }
            }
            Type::Array(a) => {
                buf.push_str("Arr_");
                self.append_clone_type_key(buf, &a.elem);
            }
            Type::Tuple(t) => {
                buf.push_str("Tup");
                for elem in &t.elems {
                    buf.push('_');
                    self.append_clone_type_key(buf, elem);
                }
            }
            other => buf.push_str(&type_string(other)),
        }
    }

    /// Zig `cloneGraphReaches` (:12284): does the type graph
    /// reachable from `ty` contain the struct keyed `root_key`?
    fn clone_graph_reaches(
        &self,
        root_key: &str,
        ty: &Type,
        visited: &mut HashSet<String>,
    ) -> bool {
        if let Some((_, elem)) = crate::place::vec_like_element_type(ty) {
            return self.clone_graph_reaches(root_key, &elem, visited);
        }
        for name in ["Box", "Rc", "Arc"] {
            if let Some(inner) = generic_inner(ty, name) {
                return self.clone_graph_reaches(root_key, &inner, visited);
            }
        }
        match peel_borrow_pointer_type(ty) {
            Type::UserDefined(u) => {
                let key = self.clone_type_key(ty);
                if key == root_key {
                    return true;
                }
                if !visited.insert(key) {
                    return false;
                }
                let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
                    return false;
                };
                decl.fields
                    .iter()
                    .any(|f| self.clone_graph_reaches(root_key, &f.ty, visited))
            }
            Type::Array(a) => self.clone_graph_reaches(root_key, &a.elem, visited),
            Type::Tuple(t) => t
                .elems
                .iter()
                .any(|e| self.clone_graph_reaches(root_key, e, visited)),
            _ => false,
        }
    }

    /// Zig `typeHasRecursiveClone` (:12311): is `ty` a user
    /// struct whose deep clone transitively reaches itself?
    pub(crate) fn type_has_recursive_clone(&self, ty: &Type) -> bool {
        let peeled = peel_borrow_pointer_type(ty);
        let Type::UserDefined(u) = peeled else {
            return false;
        };
        if is_string_ty(peeled) {
            return false;
        }
        let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
            return false;
        };
        let key = self.clone_type_key(peeled);
        let mut visited = HashSet::new();
        decl.fields
            .iter()
            .any(|f| self.clone_graph_reaches(&key, &f.ty, &mut visited))
    }

    /// Does the type's clone closure contain an owned String?
    /// The checker's copy plan consumes bindings of such types
    /// in call-arg / push positions (it spares Vec-field
    /// structs but not String-field ones — the p5/p8 probe
    /// evidence), so an emitter-side copy there would leave
    /// the original live with no cleanup owner: the copy
    /// materialisation gates must decline these types and let
    /// the move happen.
    pub(crate) fn type_contains_string(&self, ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        if is_string_ty(ty) {
            return true;
        }
        if let Some((_, elem)) = crate::place::vec_like_element_type(ty) {
            return self.type_contains_string(&elem, depth + 1);
        }
        for name in ["Box", "Rc", "Arc"] {
            if let Some(inner) = generic_inner(ty, name) {
                return self.type_contains_string(&inner, depth + 1);
            }
        }
        match peel_borrow_pointer_type(ty) {
            Type::Array(a) => self.type_contains_string(&a.elem, depth + 1),
            Type::Tuple(t) => t
                .elems
                .iter()
                .any(|e| self.type_contains_string(e, depth + 1)),
            Type::UserDefined(u) => {
                if let Some(decl) = self.enum_decl(&u.name).cloned() {
                    return decl.variants.iter().any(|v| {
                        v.fields
                            .iter()
                            .any(|f| self.type_contains_string(&f.ty, depth + 1))
                    });
                }
                match crate::place::struct_decl_lookup(self.check, &u.name) {
                    Some(decl) => decl
                        .fields
                        .iter()
                        .any(|f| self.type_contains_string(&f.ty, depth + 1)),
                    None => false,
                }
            }
            _ => false,
        }
    }

    /// Zig `genVecCloneValue` (:12149).
    pub(crate) fn gen_vec_clone_value(&mut self, source_reg: &str, elem: &Type) -> String {
        let out = self.new_tmp();
        self.ins(&format!(
            "EXPAND VEC_CLONE {out}, {source_reg}, {}",
            type_size(elem).max(8)
        ));
        out
    }

    /// Zig `genGrowVecCloneValue` (:12164).
    pub(crate) fn gen_growvec_clone_value(&mut self, source_reg: &str, elem: &Type) -> String {
        let out = self.new_tmp();
        self.ins(&format!(
            "EXPAND GROWVEC_CLONE {out}, {source_reg}, {}",
            type_size(elem).max(8)
        ));
        out
    }

    /// Zig `genDeepCloneValue` (:12421). Returns the clone
    /// register, or None when the type is plain-copyable.
    pub(crate) fn gen_deep_clone_value(
        &mut self,
        source_reg: &str,
        ty: &Type,
    ) -> Result<Option<String>> {
        if is_string_ty(ty) {
            let cloned = self.new_tmp();
            self.ins(&format!(
                "EXPAND STRING_BUF_CLONE {cloned}, {source_reg}"
            ));
            return Ok(Some(cloned));
        }
        if let Some((is_grow, elem)) = crate::place::vec_like_element_type(ty) {
            return Ok(Some(self.gen_vec_clone_value_deep(source_reg, &elem, is_grow)?));
        }
        if let Some(inner) = generic_inner(ty, "Box") {
            return Ok(Some(self.gen_box_clone_value(source_reg, &inner)?));
        }
        for (name, macro_name) in [("Rc", "RC_CLONE"), ("Arc", "ARC_CLONE")] {
            if generic_inner(ty, name).is_some() {
                let reg = self.new_tmp();
                self.ins(&format!("{reg} = add {source_reg}, 0"));
                self.ins(&format!("EXPAND {macro_name} {reg}"));
                return Ok(Some(reg));
            }
        }
        if let Some(bad) = self.uncloneable_type_name(ty, 0) {
            return Err(TextError::Unsupported {
                feature: format!(
                    "UncloneableCopy: a value of type `{}` cannot be copied: \
                     its closure contains {bad}, which has no clone",
                    type_string(ty)
                ),
                zig_ref: "codegen.zig:12421 (genDeepCloneValue)",
            });
        }
        match peel_borrow_pointer_type(ty) {
            Type::Array(arr) => {
                let arr = arr.clone();
                Ok(Some(self.gen_array_clone_value(source_reg, &arr.elem, arr.len)?))
            }
            Type::Tuple(t) => {
                let elems = t.elems.clone();
                Ok(Some(self.gen_tuple_clone_value(source_reg, &elems)?))
            }
            Type::UserDefined(u) => {
                let has_decl = crate::place::struct_decl_lookup(self.check, &u.name).is_some();
                if has_decl
                    && self.shallow_copy_call_arg_value(ty, 0)
                    && !matches!(peeled_name(ty), Some("Vec" | "GrowVec" | "String"))
                    && generic_inner(ty, "Box").is_none()
                {
                    // REG3-DEF-2: a recursive struct cannot be
                    // clone-generated inline; Zig routes it
                    // through a per-type helper (@sla_clone_<key>,
                    // genRecursiveCloneCall :12354). That helper
                    // port is a named fork: DECLINE here (Ok(None))
                    // so copy call sites fall back to the route
                    // they used before the dispatcher existed —
                    // erroring would regress files that passed
                    // through that route (reg3_redis_*).
                    if self.type_has_recursive_clone(ty) {
                        return Ok(None);
                    }
                    return self.gen_shallow_record_clone(source_reg, ty);
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Zig `genVecCloneValueDeep` (:12544) / `genGrowVecCloneValueDeep`
    /// (:12557): buffer-clone, then replace each element with a
    /// deep clone when the element type needs one.
    fn gen_vec_clone_value_deep(
        &mut self,
        source_reg: &str,
        elem: &Type,
        is_grow: bool,
    ) -> Result<String> {
        let out = if is_grow {
            self.gen_growvec_clone_value(source_reg, elem)
        } else {
            self.gen_vec_clone_value(source_reg, elem)
        };
        if !self.deep_clone_needed(elem, 0) {
            return Ok(out);
        }
        let len_reg = self.new_tmp();
        if is_grow {
            self.ins(&format!("{len_reg} = load {out}+GrowVec_len as u64"));
        } else {
            self.ins(&format!("{len_reg} = call @sa_vec_len(&{out})"));
        }
        let data_reg = self.new_tmp();
        let ptr_field = if is_grow { "GrowVec_ptr" } else { "Vec_ptr" };
        self.ins(&format!("{data_reg} = load {out}+{ptr_field} as ptr"));
        self.gen_replace_buffer_elements_with_clones(&data_reg, &len_reg, elem)?;
        self.emit_release(&len_reg)?;
        self.emit_release(&data_reg)?;
        Ok(out)
    }

    /// Zig `genReplaceBufferElementsWithClones` (:12570): loop
    /// over a cloned buffer and replace every element slot with
    /// a deep clone of the element it currently (aliasingly)
    /// holds.
    fn gen_replace_buffer_elements_with_clones(
        &mut self,
        data_reg: &str,
        len_reg: &str,
        elem: &Type,
    ) -> Result<()> {
        let elem_ty_str = type_string(elem);
        let idx_slot = self.new_tmp();
        self.ins(&format!("{idx_slot} = stack_alloc 8"));
        self.ins(&format!("store {idx_slot}+0, 0 as u64"));
        let head_label = self.new_label("L_CLONE_ELEM_HEAD");
        let body_label = self.new_label("L_CLONE_ELEM_BODY");
        let end_label = self.new_label("L_CLONE_ELEM_END");
        self.ins(&format!("jmp {head_label}"));
        self.wl("");
        self.wl(&format!("{head_label}:"));
        let idx_reg = self.new_tmp();
        self.ins(&format!("{idx_reg} = load {idx_slot}+0 as u64"));
        let more_reg = self.new_tmp();
        self.ins(&format!("{more_reg} = ult {idx_reg}, {len_reg}"));
        self.ins(&format!("br {more_reg} -> {body_label}, {end_label}"));
        self.wl("");
        self.wl(&format!("{body_label}:"));
        self.ins(&format!("!{more_reg}"));
        let off_reg = self.new_tmp();
        self.ins(&format!(
            "{off_reg} = mul {idx_reg}, {}",
            type_size(elem).max(8)
        ));
        let slot_reg = self.new_tmp();
        self.ins(&format!("{slot_reg} = ptr_add {data_reg}, {off_reg}"));
        let item_reg = self.new_tmp();
        self.ins(&format!("{item_reg} = load {slot_reg}+0 as {elem_ty_str}"));
        let cloned = self
            .gen_deep_clone_value(&item_reg, elem)?
            .unwrap_or_else(|| item_reg.clone());
        self.ins(&format!(
            "store {slot_reg}+0, ^{cloned} as {elem_ty_str}"
        ));
        if cloned != item_reg {
            self.emit_release(&item_reg)?;
        }
        self.emit_release(&off_reg)?;
        self.emit_release(&slot_reg)?;
        let next_idx = self.new_tmp();
        self.ins(&format!("{next_idx} = add {idx_reg}, 1"));
        self.ins(&format!("store {idx_slot}+0, {next_idx} as u64"));
        self.emit_release(&next_idx)?;
        self.emit_release(&idx_reg)?;
        self.ins(&format!("jmp {head_label}"));
        self.wl("");
        self.wl(&format!("{end_label}:"));
        self.ins(&format!("!{more_reg}"));
        Ok(())
    }

    /// Zig `genBoxCloneValue` (:12465): clone the pointee into a
    /// fresh cell.
    fn gen_box_clone_value(&mut self, source_reg: &str, inner: &Type) -> Result<String> {
        let word_reg = self.new_tmp();
        self.ins(&format!("{word_reg} = load {source_reg}+0 as u64"));
        let stored = self
            .gen_deep_clone_value(&word_reg, inner)?
            .unwrap_or_else(|| word_reg.clone());
        let out = self.new_tmp();
        self.ins(&format!("EXPAND BOX_NEW {out}, {stored}"));
        // The stored word's ownership moved into the fresh box
        // cell (the BOX_NEW discipline); a cloned pointee retires
        // its source word, a plain word was itself transferred.
        self.emit_forget_moved_value(&stored)?;
        if stored != word_reg {
            self.emit_release(&word_reg)?;
        }
        Ok(out)
    }

    /// Zig `genArrayCloneValue` (:12484): clone a fixed array
    /// into a fresh block, element by element (the count is a
    /// compile-time constant, so the copy is unrolled).
    fn gen_array_clone_value(
        &mut self,
        source_reg: &str,
        elem: &Type,
        len: usize,
    ) -> Result<String> {
        let elem_size = type_size(elem);
        let elem_ty_str = type_string(elem);
        let out = self.new_tmp();
        self.ins(&format!("{out} = alloc {}", (elem_size * len).max(1)));
        if crate::place::inline_pod_element(self.check, elem) {
            // Inline-POD element slots hold the packed record
            // image: copying the word is the deep copy.
            for i in 0..len {
                let word = self.new_tmp();
                self.ins(&format!(
                    "{word} = load {source_reg}+{} as u64",
                    elem_size * i
                ));
                self.ins(&format!(
                    "store {out}+{}, {word} as u64",
                    elem_size * i
                ));
                self.emit_forget_moved_value(&word)?;
            }
            return Ok(out);
        }
        for i in 0..len {
            let elem_reg = self.new_tmp();
            self.ins(&format!(
                "{elem_reg} = load {source_reg}+{} as {elem_ty_str}",
                elem_size * i
            ));
            if let Some(cloned) = self.gen_deep_clone_value(&elem_reg, elem)? {
                self.ins(&format!(
                    "store {out}+{}, ^{cloned} as {elem_ty_str}",
                    elem_size * i
                ));
            } else {
                self.ins(&format!(
                    "store {out}+{}, {elem_reg} as {elem_ty_str}",
                    elem_size * i
                ));
            }
            self.emit_release(&elem_reg)?;
        }
        Ok(out)
    }

    /// Zig `genTupleCloneValue` (:12528): clone a tuple
    /// element-wise into a fresh tuple block.
    fn gen_tuple_clone_value(
        &mut self,
        source_reg: &str,
        elems: &[Type],
    ) -> Result<String> {
        let tuple = sla_ast::TupleType {
            elems: elems.to_vec(),
        };
        let out = self.new_tmp();
        self.ins(&format!(
            "{out} = alloc {}",
            sla_lowering::tuple_abi_size(&tuple)
        ));
        for (i, elem_ty) in elems.iter().enumerate() {
            let (offset, _) = layout::tuple_field_layout(elems, i).ok_or_else(|| {
                TextError::Unsupported {
                    feature: format!("tuple clone field layout {i}"),
                    zig_ref: "lowering_rules.zig (tupleFieldLayout)",
                }
            })?;
            let elem_ty_str = type_string(elem_ty);
            let elem_reg = self.new_tmp();
            self.ins(&format!(
                "{elem_reg} = load {source_reg}+{offset} as {elem_ty_str}"
            ));
            if let Some(cloned) = self.gen_deep_clone_value(&elem_reg, elem_ty)? {
                self.ins(&format!(
                    "store {out}+{offset}, ^{cloned} as {elem_ty_str}"
                ));
            } else {
                self.ins(&format!(
                    "store {out}+{offset}, {elem_reg} as {elem_ty_str}"
                ));
            }
            self.emit_release(&elem_reg)?;
        }
        Ok(out)
    }

    /// Zig `genShallowCopyCallArgValue` (:5705): a record copy
    /// whose fields dispatch through the deep clone dispatcher —
    /// handle fields clone, plain fields copy their word, nested
    /// structs recurse through the dispatcher's struct arm.
    /// Returns None without emitting when the type is outside
    /// the shallow-copyable subset.
    pub(crate) fn gen_shallow_record_clone(
        &mut self,
        source_reg: &str,
        ty: &Type,
    ) -> Result<Option<String>> {
        let Type::UserDefined(u) = peel_borrow_pointer_type(ty) else {
            return Ok(None);
        };
        let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
            return Ok(None);
        };
        if decl.is_opaque || decl.is_union || !self.shallow_copy_call_arg_value(ty, 0) {
            return Ok(None);
        }
        if self.type_has_recursive_clone(ty) {
            // Decline, as in gen_deep_clone_value's struct arm:
            // the @sla_clone_<key> helper port is a named fork,
            // and declining keeps the pre-dispatcher route.
            return Ok(None);
        }
        let decl = decl.clone();
        let target = self.new_tmp();
        self.ins(&format!("{target} = alloc {}", layout::struct_size(&decl)));
        for field in &decl.fields {
            let Some((offset, field_ty)) = layout::struct_field_layout(&decl, &field.name)
            else {
                return Ok(None);
            };
            let field_reg = self.new_tmp();
            self.ins(&format!(
                "{field_reg} = load {source_reg}+{offset} as {}",
                type_string(&field_ty)
            ));
            if let Some(cloned) = self.gen_deep_clone_value(&field_reg, &field_ty)? {
                self.ins(&format!(
                    "store {target}+{offset}, ^{cloned} as {}",
                    type_string(&field_ty)
                ));
            } else {
                self.ins(&format!(
                    "store {target}+{offset}, {field_reg} as {}",
                    type_string(&field_ty)
                ));
            }
            self.emit_release(&field_reg)?;
        }
        Ok(Some(target))
    }
}

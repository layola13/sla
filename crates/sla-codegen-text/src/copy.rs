//! Record-copy materialisation: Zig `genCopyValueInto`
//! (codegen.zig:3913), `genShallowCopyCallArgValue` (:5705) and
//! the CLASSB-DEF1 push copy (`pushArgCopyMaterialization`,
//! :5043). Plain-field subset first: a copy whose fields are all
//! plain words (primitive / pointer / borrow / fn-ptr / enum)
//! or nested plain structs copies field by field. A field
//! owning a heap handle (Vec / String / Box / array / ...) is
//! copied by the deep clone dispatcher (clone.rs,
//! `genDeepCloneValue`); a map/set field stays uncloneable and
//! a recursive clone graph stays loud (per-type helper fork).

use sla_ast::{CallExpr, Node, StructDecl, Type};
use sla_lowering::is_borrow_like_type;

use crate::codegen::{type_string, Codegen};
use crate::{layout, Result};

/// Zig `isStdCollectionType` approximation over the names this
/// emitter's surface layer special-cases.
fn is_std_collection_type(ty: &Type) -> bool {
    match ty {
        Type::UserDefined(u) => matches!(
            u.name.as_str(),
            "Vec"
                | "GrowVec"
                | "String"
                | "Slice"
                | "HashMap"
                | "HashSet"
                | "BTreeMap"
                | "BTreeSet"
                | "VecDeque"
        ),
        _ => false,
    }
}

fn is_smart_pointer_type(ty: &Type) -> bool {
    match ty {
        Type::UserDefined(u) => matches!(u.name.as_str(), "Box" | "Rc" | "Arc"),
        _ => false,
    }
}

impl<'c> Codegen<'c> {
    /// Zig `typeIsShallowCopyCallArgValue` (:4998) over
    /// lowering_rules' base facts: collections and std owners
    /// are shallow-copyable only when nested (depth > 0); plain
    /// words always; tuples / arrays recurse; pure enums are
    /// POD-like; structs need the non-opaque / non-union gate
    /// plus all-shallow fields.
    pub(crate) fn shallow_copy_call_arg_value(&self, ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        if is_std_collection_type(ty) {
            return depth > 0;
        }
        match ty {
            Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_) | Type::FnPtr(_) => true,
            Type::Tuple(t) => t
                .elems
                .iter()
                .all(|e| self.shallow_copy_call_arg_value(e, depth + 1)),
            Type::Array(a) => self.shallow_copy_call_arg_value(&a.elem, depth + 1),
            Type::UserDefined(u) => {
                if is_smart_pointer_type(ty) {
                    return depth > 0;
                }
                if self.enum_decl(&u.name).is_some()
                    || self.enum_decl(crate::place::local_name(&u.name)).is_some()
                {
                    return true;
                }
                let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
                    return false;
                };
                if decl.is_opaque || decl.is_union {
                    return false;
                }
                decl.fields
                    .iter()
                    .all(|f| self.shallow_copy_call_arg_value(&f.ty, depth + 1))
            }
            _ => false,
        }
    }

    /// Whether a record copy of `ty` stays inside the plain-word
    /// subset (no heap-handle field anywhere, recursively).
    pub(crate) fn record_copy_plain(&self, ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        match ty {
            Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_) | Type::FnPtr(_) => true,
            Type::UserDefined(u) => {
                if self.enum_decl(&u.name).is_some()
                    || self.enum_decl(crate::place::local_name(&u.name)).is_some()
                {
                    return true;
                }
                let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
                    return false;
                };
                if decl.is_opaque || decl.is_union {
                    return false;
                }
                decl.fields
                    .iter()
                    .all(|f| self.record_copy_plain(&f.ty, depth + 1))
            }
            _ => false,
        }
    }

    /// Emit the field-by-field record copy into `target` (which
    /// the caller allocated-named; this emits the `alloc`).
    /// Returns false without emitting when the type is outside
    /// the plain subset.
    pub(crate) fn gen_copy_value_into_inner(
        &mut self,
        target: &str,
        source_reg: &str,
        ty: &Type,
    ) -> Result<bool> {
        let Type::UserDefined(u) = ty else {
            return Ok(false);
        };
        let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
            return Ok(false);
        };
        if decl.is_opaque || decl.is_union || !self.record_copy_plain(ty, 0) {
            return Ok(false);
        }
        let decl: StructDecl = decl.clone();
        self.ins(&format!("{target} = alloc {}", layout::struct_size(&decl)));
        for field in &decl.fields {
            let Some((offset, field_ty)) = layout::struct_field_layout(&decl, &field.name)
            else {
                return Ok(false);
            };
            let field_reg = self.new_tmp();
            self.ins(&format!(
                "{field_reg} = load {source_reg}+{offset} as {}",
                type_string(&field_ty)
            ));
            if matches!(field_ty, Type::UserDefined(_))
                && self.enum_decl(field_name_of(&field_ty)).is_none()
                && crate::place::struct_decl_lookup(self.check, field_name_of(&field_ty))
                    .is_some()
            {
                let cloned = self.new_tmp();
                let ok = self.gen_copy_value_into_inner(&cloned, &field_reg, &field_ty)?;
                if !ok {
                    return Ok(false);
                }
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
        Ok(true)
    }

    /// Whether a materializing copy of a tuple with these
    /// element types stays inside the plain subset: every
    /// element plain (`record_copy_plain`), nested tuples
    /// recursively.
    fn tuple_copy_plain(&self, elems: &[Type], depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        elems.iter().all(|e| match e {
            Type::Tuple(t) => self.tuple_copy_plain(&t.elems, depth + 1),
            _ => self.record_copy_plain(e, depth),
        })
    }

    /// DEF-TUPLE-COPY-EMIT (Zig `genTupleCopyValueInto`):
    /// element-wise materializing copy of a tuple value into a
    /// fresh allocation, the tuple analog of the record copy
    /// above. Nested tuples recurse; nested struct elements go
    /// through the record copy, exactly like struct fields of
    /// struct copies. A nested copy settles with the `^` consume
    /// marker in the store itself and the loaded element
    /// register with a plain release (the REG2-D3 pairing);
    /// plain elements store directly. Returns false without
    /// emitting when the type is outside the plain subset.
    pub(crate) fn gen_tuple_copy_value_into_inner(
        &mut self,
        target: &str,
        source_reg: &str,
        ty: &Type,
    ) -> Result<bool> {
        let Type::Tuple(tuple) = ty else {
            return Ok(false);
        };
        if !self.tuple_copy_plain(&tuple.elems, 0) {
            return Ok(false);
        }
        let elems = tuple.elems.clone();
        self.ins(&format!(
            "{target} = alloc {}",
            sla_lowering::tuple_abi_size(&sla_ast::TupleType {
                elems: elems.clone()
            })
        ));
        for (i, elem_ty) in elems.iter().enumerate() {
            let Some((offset, elem_ty)) = layout::tuple_field_layout(&elems, i) else {
                return Ok(false);
            };
            let elem_reg = self.new_tmp();
            self.ins(&format!(
                "{elem_reg} = load {source_reg}+{offset} as {}",
                type_string(&elem_ty)
            ));
            let nested_struct = matches!(elem_ty, Type::UserDefined(_))
                && self.enum_decl(field_name_of(&elem_ty)).is_none()
                && crate::place::struct_decl_lookup(self.check, field_name_of(&elem_ty))
                    .is_some();
            if matches!(elem_ty, Type::Tuple(_)) || nested_struct {
                let copied = self.new_tmp();
                let ok = if matches!(elem_ty, Type::Tuple(_)) {
                    self.gen_tuple_copy_value_into_inner(&copied, &elem_reg, &elem_ty)?
                } else {
                    self.gen_copy_value_into_inner(&copied, &elem_reg, &elem_ty)?
                };
                if !ok {
                    return Ok(false);
                }
                self.ins(&format!(
                    "store {target}+{offset}, ^{copied} as {}",
                    type_string(&elem_ty)
                ));
                self.emit_release(&elem_reg)?;
            } else {
                self.ins(&format!(
                    "store {target}+{offset}, {elem_reg} as {}",
                    type_string(&elem_ty)
                ));
            }
        }
        Ok(true)
    }

    /// Whether a record copy of `ty` is emittable at all: the
    /// plain-field subset, or the deep clone dispatcher
    /// (clone.rs) for a shallow-copyable record with no
    /// uncloneable (map/set) member and no recursive clone
    /// graph.
    pub(crate) fn record_copyable(&self, ty: &Type) -> bool {
        self.record_copy_plain(ty, 0)
            || (self.shallow_copy_call_arg_value(ty, 0)
                && self.uncloneable_type_name(ty, 0).is_none()
                && !self.type_has_recursive_clone(ty))
    }

    /// A fresh record copy of the value in `source_reg`, or None
    /// when the type is outside both the plain subset and the
    /// deep clone dispatcher's record arm.
    pub(crate) fn gen_record_copy(
        &mut self,
        source_reg: &str,
        ty: &Type,
    ) -> Result<Option<String>> {
        let target = self.new_tmp();
        if self.gen_copy_value_into_inner(&target, source_reg, ty)? {
            Ok(Some(target))
        } else {
            self.gen_shallow_record_clone(source_reg, ty)
        }
    }

    /// CLASSB-DEF1 (Zig `pushArgCopyMaterialization`): a push of
    /// a live identifier binding whose element type is copy-like
    /// (Copy value or shallow-copyable) must store an independent
    /// copy and leave the source binding live. Returns the copy
    /// register to push, or None when no copy is owed / the copy
    /// is outside the plain subset (the caller keeps its
    /// ownership-transfer treatment).
    pub(crate) fn push_arg_copy_materialization(
        &mut self,
        call: &CallExpr,
        elem: &Type,
    ) -> Result<Option<String>> {
        self.push_arg_copy_materialization_node(&call.args[1], elem)
    }

    /// Node form of `push_arg_copy_materialization` (the bare
    /// `vec(...)` constructor's elements sit at varying call
    /// indices, not at args[1]).
    pub(crate) fn push_arg_copy_materialization_node(
        &mut self,
        arg: &Node,
        elem: &Type,
    ) -> Result<Option<String>> {
        if !matches!(arg, Node::Identifier(_)) {
            return Ok(None);
        }
        let copy_like =
            self.check.type_is_copy_value(elem) || self.shallow_copy_call_arg_value(elem, 0);
        let consumes = !copy_like && !is_borrow_like_type(elem);
        if consumes {
            return Ok(None);
        }
        let Type::UserDefined(u) = elem else {
            return Ok(None);
        };
        if crate::place::struct_decl_lookup(self.check, &u.name).is_none() {
            return Ok(None);
        }
        if !self.check.type_is_copy_struct(elem) && !self.shallow_copy_call_arg_value(elem, 0) {
            return Ok(None);
        }
        if !self.record_copyable(elem) {
            return Ok(None);
        }
        // A String in the element closure means the checker
        // consumes the binding at this push (its copy plan
        // spares Vec-field structs, not String-field ones):
        // cloning would orphan the original (arena Cmd case).
        if self.type_contains_string(elem, 0) {
            return Ok(None);
        }
        let raw = self.gen_expr(arg)?;
        self.gen_record_copy(&raw, elem)
    }
}

fn field_name_of(ty: &Type) -> &str {
    match ty {
        Type::UserDefined(u) => &u.name,
        _ => "",
    }
}

//! Drop elaboration for the TEXT backend (RUST-S9-RUNTIME-DOUBLE-FREE).
//!
//! Mirror of the Zig emitter's DEF-ARC-DROP (F1) machinery:
//! `planValueDrop`/`computeDropPlan` (type_checker.zig:1493+) for the
//! drop-plan classification and the `emitRelease` plan dispatch +
//! `emitDeepValueDropText` / `emitSmartPointerDropText` /
//! `emitStructFieldDropsText` / `emitContainerElementDropsText`
//! family (codegen.zig) for the emission. Before this port, every
//! release in the TEXT emitter was a bare `!reg`, which the SA host
//! treats as an unconditional block free: a by-value `Rc`/`Arc`
//! parameter's exit release freed the shared box with no count
//! decrement (the caller's subsequent reads hit stale counts or a
//! glibc double free), struct field drops never fired, and the
//! clone bump a by-value struct copy performs on `Rc`/`Arc` fields
//! was never compensated by the source binding's cleanup.
//!
//! The Rust checker exposes no drop-taint surface, so the taint
//! checks in the Zig originals are treated as never-covering. The
//! one shape taint decides in the failing corpus — struct-update
//! fields raw-moved out of the source place — is handled at the
//! update site in into.rs, which settles the source register with
//! the plan dispatch suppressed and compensates only the fields
//! its copy arm actually bumped.

use sla_ast::{Type, UserDefined};

use crate::codegen::{type_size, type_string, Codegen};
use crate::layout;
use crate::place;
use crate::Result;

/// type_checker.zig `DropPlan` (the TEXT-side classification).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DropPlan {
    None,
    Buffer,
    BufferGrowVec,
    DeepVec(Box<DropPlan>),
    DeepGrowVec(Box<DropPlan>),
    DeepStruct(Vec<FieldDropPlan>),
    ArcPtr(Box<DropPlan>),
    RcPtr(Box<DropPlan>),
    BoxPtr(Box<DropPlan>),
}

/// type_checker.zig `FieldDropPlan`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldDropPlan {
    pub name: String,
    pub plan: DropPlan,
}

/// type_checker.zig `drop_excluded_type_names` (the names whose
/// `drop_excluded` flag the checker sets; the checker's own
/// `drop_excluded` query is consulted first, this list is the
/// belt-and-braces mirror for names the Rust checker does not flag).
const DROP_EXCLUDED_TYPE_NAMES: [&str; 15] = [
    "HashMap",
    "BTreeMap",
    "HashSet",
    "BTreeSet",
    "VecDeque",
    "StringBuf",
    "Mutex",
    "RwLock",
    "OnceLock",
    "Chan",
    "Rc",
    "Arc",
    "Weak",
    "RefCell",
    "Box",
];

/// type_checker.zig `dropPlanSubstitute`: substitute a struct
/// decl's generic parameters with an instantiation's arguments.
fn drop_plan_substitute(ty: &Type, params: &[String], args: &[Type]) -> Type {
    match ty {
        Type::UserDefined(u) => {
            if u.generics.is_empty() {
                if let Some(i) = params.iter().position(|p| p == &u.name) {
                    if let Some(arg) = args.get(i) {
                        return arg.clone();
                    }
                }
                return ty.clone();
            }
            Type::UserDefined(UserDefined {
                name: u.name.clone(),
                generics: u
                    .generics
                    .iter()
                    .map(|g| drop_plan_substitute(g, params, args))
                    .collect(),
            })
        }
        Type::Pointer(inner) => {
            Type::Pointer(Box::new(drop_plan_substitute(inner, params, args)))
        }
        Type::Borrow(inner) => {
            Type::Borrow(Box::new(drop_plan_substitute(inner, params, args)))
        }
        _ => ty.clone(),
    }
}

/// codegen.zig `vecElementSlotSize` as the TEXT crate's clone/copy
/// paths spell it (`slot_size` in surface.rs).
fn vec_element_slot_size(elem: &Type) -> usize {
    type_size(elem).max(8)
}

impl<'c> Codegen<'c> {
    /// type_checker.zig `planValueDrop` (with the smart-pointer
    /// interception inlined before the excluded check, exactly as
    /// `planValueDrop` wraps `computeDropPlan`).
    pub(crate) fn plan_value_drop(&self, ty: &Type) -> DropPlan {
        let mut active: Vec<String> = Vec::new();
        self.compute_drop_plan(ty, &mut active)
    }

    fn compute_drop_plan(&self, ty: &Type, active: &mut Vec<String>) -> DropPlan {
        if let Type::UserDefined(u) = ty {
            match u.name.as_str() {
                "String" => return DropPlan::Buffer,
                "Vec" => {
                    let Some(elem) = u.generics.first() else {
                        return DropPlan::None;
                    };
                    return match self.compute_drop_plan(elem, active) {
                        DropPlan::None => DropPlan::Buffer,
                        plan => DropPlan::DeepVec(Box::new(plan)),
                    };
                }
                "GrowVec" => {
                    let Some(elem) = u.generics.first() else {
                        return DropPlan::None;
                    };
                    return match self.compute_drop_plan(elem, active) {
                        DropPlan::None => DropPlan::BufferGrowVec,
                        plan => DropPlan::DeepGrowVec(Box::new(plan)),
                    };
                }
                // Smart pointers are intercepted BEFORE the
                // excluded check (planValueDrop): Rc/Arc/Box are
                // drop-excluded names whose payloads still drop.
                "Arc" | "Rc" | "Box" => {
                    if let Some(inner) = u.generics.first() {
                        let inner_plan = self.compute_drop_plan(inner, active);
                        return match u.name.as_str() {
                            "Arc" => DropPlan::ArcPtr(Box::new(inner_plan)),
                            "Rc" => DropPlan::RcPtr(Box::new(inner_plan)),
                            _ => DropPlan::BoxPtr(Box::new(inner_plan)),
                        };
                    }
                }
                _ => {}
            }
        }
        if self.drop_plan_excluded_type(ty) {
            return DropPlan::None;
        }
        if let Type::UserDefined(u) = ty {
            // Recursive re-entry through a type already being
            // planned is not deep-dropped (computeDropPlan's
            // drop_plan_active cut).
            let key = type_string(ty).to_string();
            if active.contains(&key) {
                return DropPlan::None;
            }
            if let Some(decl) = place::struct_decl_lookup(self.check, &u.name) {
                if !decl.is_opaque && !decl.is_union {
                    active.push(key);
                    let mut fields: Vec<FieldDropPlan> = Vec::new();
                    for field in &decl.fields {
                        let field_ty =
                            drop_plan_substitute(&field.ty, &decl.generics, &u.generics);
                        let field_plan = self.compute_drop_plan(&field_ty, active);
                        if field_plan != DropPlan::None {
                            fields.push(FieldDropPlan {
                                name: field.name.clone(),
                                plan: field_plan,
                            });
                        }
                    }
                    active.pop();
                    if !fields.is_empty() {
                        return DropPlan::DeepStruct(fields);
                    }
                }
            }
        }
        DropPlan::None
    }

    /// type_checker.zig `dropPlanExcludedType`.
    fn drop_plan_excluded_type(&self, ty: &Type) -> bool {
        if self.check.drop_excluded(ty) {
            return true;
        }
        match ty {
            Type::UserDefined(u) => DROP_EXCLUDED_TYPE_NAMES.contains(&u.name.as_str()),
            _ => false,
        }
    }

    /// codegen.zig `emitDeepValueDropText` (taint arms omitted:
    /// taint never covers without a checker surface).
    pub(crate) fn emit_deep_value_drop(
        &mut self,
        value_reg: &str,
        ty: &Type,
        plan: &DropPlan,
        free_container: bool,
    ) -> Result<()> {
        match plan {
            DropPlan::None => self.emit_release(value_reg),
            DropPlan::Buffer => {
                if free_container {
                    self.ins(&format!("EXPAND VEC_FREE {value_reg}"));
                    self.consumed_bindings.insert(value_reg.to_string());
                    Ok(())
                } else {
                    self.emit_release(value_reg)
                }
            }
            DropPlan::BufferGrowVec => {
                if free_container {
                    self.ins(&format!("EXPAND GROWVEC_FREE {value_reg}"));
                    self.consumed_bindings.insert(value_reg.to_string());
                    Ok(())
                } else {
                    self.emit_release(value_reg)
                }
            }
            DropPlan::DeepVec(elem_plan) => {
                self.emit_container_element_drops(value_reg, ty, elem_plan, "Vec_ptr", "Vec_len")?;
                if free_container {
                    self.ins(&format!("EXPAND VEC_FREE {value_reg}"));
                    self.consumed_bindings.insert(value_reg.to_string());
                    Ok(())
                } else {
                    self.emit_release(value_reg)
                }
            }
            DropPlan::DeepGrowVec(elem_plan) => {
                self.emit_container_element_drops(
                    value_reg,
                    ty,
                    elem_plan,
                    "GrowVec_ptr",
                    "GrowVec_len",
                )?;
                if free_container {
                    self.ins(&format!("EXPAND GROWVEC_FREE {value_reg}"));
                    self.consumed_bindings.insert(value_reg.to_string());
                    Ok(())
                } else {
                    self.emit_release(value_reg)
                }
            }
            DropPlan::DeepStruct(fields) => {
                self.emit_struct_field_drops(value_reg, ty, fields)?;
                self.emit_release(value_reg)
            }
            DropPlan::ArcPtr(_) | DropPlan::RcPtr(_) | DropPlan::BoxPtr(_) => {
                self.emit_smart_pointer_drop_text(value_reg, ty, plan, true)
            }
        }
    }

    /// codegen.zig `emitStructFieldDropsText`.
    pub(crate) fn emit_struct_field_drops(
        &mut self,
        struct_reg: &str,
        struct_ty: &Type,
        fields: &[FieldDropPlan],
    ) -> Result<()> {
        let Type::UserDefined(u) = struct_ty else {
            return Ok(());
        };
        let Some(decl) = place::struct_decl_lookup(self.check, &u.name) else {
            return Ok(());
        };
        for field in fields {
            let Some((offset, _)) = layout::struct_field_layout(&decl, &field.name) else {
                continue;
            };
            let Some(field_decl) = decl.fields.iter().find(|f| f.name == field.name) else {
                continue;
            };
            let field_ty = drop_plan_substitute(&field_decl.ty, &decl.generics, &u.generics);
            let val_reg = self.new_tmp();
            self.ins(&format!("{val_reg} = load {struct_reg}+{offset} as ptr"));
            self.emit_deep_value_drop(&val_reg, &field_ty, &field.plan, true)?;
        }
        Ok(())
    }

    /// codegen.zig `emitVecElementDropsText` /
    /// `emitGrowVecElementDropsText` (shared body:
    /// `emitContainerElementDropsText`).
    pub(crate) fn emit_container_element_drops(
        &mut self,
        vec_reg: &str,
        vec_ty: &Type,
        elem_plan: &DropPlan,
        ptr_def: &str,
        len_def: &str,
    ) -> Result<()> {
        let Some((_, elem_ty)) = place::vec_like_element_type(vec_ty) else {
            return Ok(());
        };
        let buf_reg = self.new_tmp();
        let len_reg = self.new_tmp();
        let i_reg = self.new_tmp();
        let cond_reg = self.new_tmp();
        let head_label = self.new_label("L_DD_HEAD");
        let body_label = self.new_label("L_DD_BODY");
        let done_label = self.new_label("L_DD_DONE");
        self.ins(&format!("{buf_reg} = load {vec_reg}+{ptr_def} as ptr"));
        self.ins(&format!("{len_reg} = load {vec_reg}+{len_def} as u64"));
        self.ins(&format!("{i_reg} = 0"));
        self.ins(&format!("jmp {head_label}"));
        self.wl("");
        self.wl(&format!("{head_label}:"));
        self.ins(&format!("{cond_reg} = ult {i_reg}, {len_reg}"));
        self.ins(&format!("br {cond_reg} -> {body_label}, {done_label}"));
        self.wl("");
        self.wl(&format!("{body_label}:"));
        self.ins(&format!("!{cond_reg}"));
        let off_reg = self.new_tmp();
        let addr_reg = self.new_tmp();
        self.ins(&format!(
            "{off_reg} = mul {i_reg}, {}",
            vec_element_slot_size(&elem_ty)
        ));
        self.ins(&format!("{addr_reg} = ptr_add {buf_reg}, {off_reg}"));
        self.ins(&format!("!{off_reg}"));
        // Vec element slots hold the element VALUE; for aggregate
        // elements that value is the pointer to the boxed struct.
        // Load the element out of the slot, then drop it through
        // the deep-value path (codegen.zig's DEF deep-drop note).
        let elem_reg = self.new_tmp();
        self.ins(&format!("{elem_reg} = load {addr_reg}+0 as ptr"));
        self.emit_deep_value_drop(&elem_reg, &elem_ty, elem_plan, true)?;
        self.ins(&format!("!{addr_reg}"));
        let one_reg = self.new_tmp();
        let next_reg = self.new_tmp();
        self.ins(&format!("{one_reg} = 1"));
        self.ins(&format!("{next_reg} = add {i_reg}, {one_reg}"));
        self.ins(&format!("!{one_reg}"));
        self.ins(&format!("!{i_reg}"));
        self.ins(&format!("{i_reg} = {next_reg}"));
        self.ins(&format!("jmp {head_label}"));
        self.wl("");
        self.wl(&format!("{done_label}:"));
        self.ins(&format!("!{cond_reg}"));
        self.ins(&format!("!{i_reg}"));
        self.emit_release(&buf_reg)?;
        self.emit_release(&len_reg)?;
        Ok(())
    }

    /// codegen.zig `emitSmartPointerDropText` (TEXT form: the
    /// `EXPAND ARC_DROP`/`RC_DROP` conditional macro performs the
    /// decrement and branches to the clean label when this release
    /// took the strong count to zero; the weak drop then decides
    /// whether the box allocation itself is freed).
    pub(crate) fn emit_smart_pointer_drop_text(
        &mut self,
        value_reg: &str,
        ty: &Type,
        plan: &DropPlan,
        release_value: bool,
    ) -> Result<()> {
        enum SmartKind {
            Arc,
            Rc,
            Box,
        }
        let (kind, payload_plan) = match (ty, plan) {
            (Type::UserDefined(u), DropPlan::ArcPtr(p)) if u.name == "Arc" => {
                (SmartKind::Arc, (**p).clone())
            }
            (Type::UserDefined(u), DropPlan::RcPtr(p)) if u.name == "Rc" => {
                (SmartKind::Rc, (**p).clone())
            }
            (Type::UserDefined(u), DropPlan::BoxPtr(p)) if u.name == "Box" => {
                (SmartKind::Box, (**p).clone())
            }
            _ => {
                if release_value {
                    self.emit_release(value_reg)?;
                }
                return Ok(());
            }
        };
        let Type::UserDefined(u) = ty else {
            unreachable!("smart kind matched a user-defined type")
        };
        let Some(payload_ty) = u.generics.first().cloned() else {
            if release_value {
                self.emit_release(value_reg)?;
            }
            return Ok(());
        };
        // Work on a private copy: the ARC_DROP/RC_DROP macro
        // consumes its operand register.
        let h = self.new_tmp();
        self.ins(&format!("{h} = add {value_reg}, 0"));
        if matches!(kind, SmartKind::Box) {
            let data_reg = self.new_tmp();
            self.ins(&format!("{data_reg} = load {h}+Box_value as ptr"));
            self.emit_deep_value_drop(&data_reg, &payload_ty, &payload_plan, true)?;
            self.ins(&format!("call @sa_mem_free(^{h})"));
            if release_value {
                self.emit_release(value_reg)?;
            }
            return Ok(());
        }
        let is_arc = matches!(kind, SmartKind::Arc);
        let clean_label = self.new_label("L_SMART_CLEAN");
        let free_label = self.new_label("L_SMART_FREE");
        let after_label = self.new_label("L_SMART_AFTER");
        let end_label = self.new_label("L_SMART_END");
        self.ins(&format!(
            "EXPAND {} {h}, {clean_label}",
            if is_arc { "ARC_DROP" } else { "RC_DROP" }
        ));
        self.ins(&format!("jmp {after_label}"));
        self.wl("");
        self.wl(&format!("{clean_label}:"));
        if is_arc {
            self.ins(&format!("!__arc_is_last_{h}"));
            self.ins(&format!("!__arc_strong_{h}"));
        } else {
            self.ins(&format!("!__rc_is_last_{h}"));
            self.ins(&format!("!__rc_next_strong_{h}"));
            self.ins(&format!("!__rc_strong_{h}"));
        }
        let data_reg = self.new_tmp();
        self.ins(&format!(
            "{data_reg} = load {h}+{} as ptr",
            if is_arc { "ArcBox_data" } else { "RcBox_data" }
        ));
        self.emit_deep_value_drop(&data_reg, &payload_ty, &payload_plan, true)?;
        self.ins(&format!(
            "EXPAND {} {h}, {free_label}",
            if is_arc { "ARC_WEAK_DROP" } else { "WEAK_DROP" }
        ));
        self.ins(&format!("jmp {after_label}"));
        self.wl("");
        self.wl(&format!("{free_label}:"));
        self.ins(&format!("!__weak_is_last_{h}"));
        if !is_arc {
            self.ins(&format!("!__weak_count_{h}"));
        }
        self.ins(&format!("!__weak_next_count_{h}"));
        let free_reg = self.new_tmp();
        self.ins(&format!("{free_reg} = add {h}, 0"));
        self.ins(&format!("call @sa_mem_free(^{free_reg})"));
        self.ins(&format!("jmp {end_label}"));
        self.wl("");
        self.wl(&format!("{after_label}:"));
        self.ins(&format!("jmp {end_label}"));
        self.wl("");
        self.wl(&format!("{end_label}:"));
        self.ins(&format!("!{h}"));
        if release_value {
            self.emit_release(value_reg)?;
        }
        Ok(())
    }
}

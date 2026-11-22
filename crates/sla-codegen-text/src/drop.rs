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
//! the plan dispatch suppressed (its copy arm is a word-wise
//! record copy that creates no new ownership, so no field
//! compensation is owed there; the clone bumps of the call-argument
//! deep-clone copy are compensated by the source binding's own
//! plan-dispatch cleanup, which is this dispatch).

use sla_ast::{CallExpr, Node, Type, UserDefined};
use sla_lowering::{is_borrow_like_type, peel_borrow_pointer_type, root_identifier};

use crate::codegen::{type_string, vec_element_slot_size, Codegen};
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

/// type_checker.zig `drop_excluded_type_names`: the names the
/// checker flags drop-excluded (their values are settled by
/// family-specific machinery, never by the plan dispatch). The
/// Rust CheckInfo trait has no drop_excluded query, so this list
/// is the classification's only source for the excluded set.
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

    /// type_checker.zig `dropPlanExcludedType`: the Rust
    /// CheckInfo trait carries no drop_excluded query, so the
    /// classification rests on the name list alone (in Zig the
    /// list is likewise the checker's data source here).
    fn drop_plan_excluded_type(&self, ty: &Type) -> bool {
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
                self.emit_struct_field_drops(value_reg, ty, fields, None)?;
                self.emit_release(value_reg)
            }
            DropPlan::ArcPtr(_) | DropPlan::RcPtr(_) | DropPlan::BoxPtr(_) => {
                self.emit_smart_pointer_drop_text(value_reg, ty, plan, true)
            }
        }
    }

    /// codegen.zig `emitStructFieldDropsText`. `taint_root` is
    /// the source binding name when the dropped value is a named
    /// binding's register: fields a struct update moved out of
    /// that binding (recorded in `update_moved_fields`) are
    /// skipped — their ownership already transferred.
    pub(crate) fn emit_struct_field_drops(
        &mut self,
        struct_reg: &str,
        struct_ty: &Type,
        fields: &[FieldDropPlan],
        taint_root: Option<&str>,
    ) -> Result<()> {
        let Type::UserDefined(u) = struct_ty else {
            return Ok(());
        };
        let Some(decl) = place::struct_decl_lookup(self.check, &u.name) else {
            return Ok(());
        };
        for field in fields {
            if let Some(root) = taint_root {
                if self
                    .update_moved_fields
                    .contains(&(root.to_string(), field.name.clone()))
                {
                    continue;
                }
            }
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

/// RUST-STAGE10-DROP-TAINT (B1): the form of a tainted binding.
///
/// A binding is tainted when its value aliases storage owned by
/// someone else. The Rust emitter has no clone-on-extract, so a
/// shallow copy of a borrowed/moved-out deep value aliases the
/// source's buffers, and the stage-10 plan dispatch would free
/// them (UAF / double free). The form records whether the
/// binding's own block (husk) is fresh or aliased, which decides
/// its release: a FreshHusk DeepStruct frees its husk with `!`
/// after skipping field drops; an AliasedHusk value is forgotten
/// with `^`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaintForm {
    /// Husk is a fresh owned block; only its fields alias (a call
    /// result the callee constructed).
    FreshHusk,
    /// Husk itself aliases the source's storage (a place read,
    /// whose register holds a pointer into the source).
    AliasedHusk,
}

/// The outcome of classifying one let/assign value for B1 taint,
/// decided purely (no mutation) so a self-referential assignment
/// (`x = x.field`) still judges against the OLD taint state.
enum TaintDecision {
    /// No taint for the binding itself. (An M1 move-out is
    /// reported separately.)
    None,
    /// The binding itself is tainted (M2 / M2-call /
    /// M2-propagation).
    Tainted(TaintForm),
    /// M1 move-out: the field moved out of the (owned) root;
    /// record (root, field) in `update_moved_fields`.
    MovedOut(String, String),
}

/// `clone` (and qualified `__clone` / `::clone`) returns a fresh
/// deep copy; its result never aliases the borrow arg.
fn is_clone_callee(call: &CallExpr) -> bool {
    call.func_name == "clone"
        || call.func_name.ends_with("__clone")
        || call.func_name.ends_with("::clone")
}

impl<'c> Codegen<'c> {
    /// RUST-STAGE10-CLONE-EXTRACT (M2 refinement): a callee recorded
    /// in `clone_extract_funcs` (its return value is produced by a
    /// cloning extraction, extern_macro.rs) returns a fresh-owned
    /// value; its result never aliases the borrow arg, so M2-call
    /// must not taint it — tainting would suppress the fresh clone's
    /// field drops and leak. Name match mirrors `is_clone_callee`
    /// (exact, `__`-qualified, `::`-qualified).
    fn is_clone_extract_callee(&self, call: &CallExpr) -> bool {
        self.clone_extract_funcs.iter().any(|name| {
            call.func_name == *name
                || call.func_name.ends_with(&format!("__{name}"))
                || call.func_name.ends_with(&format!("::{name}"))
        })
    }
}

/// Root identifier of a place expression, peeling deref as
/// well as field/index/borrow/move (`*rc` -> `rc`; `(*rc).f` -> `rc`).
/// The shared `root_identifier` (sla-lowering, mirroring Zig 6031)
/// does NOT peel DerefExpr, so `decide_value_taint`'s place arm
/// bailed with None for every deref value -- the arm was dead code
/// for `*rc` / `*arc` / `*box` (task 72: the def_r8 double-free).
/// Local to this file: the shared helper's contract is Zig's.
fn place_root_identifier(expr: &Node) -> Option<&str> {
    match expr {
        Node::Identifier(name) => Some(name),
        Node::FieldExpr(f) => place_root_identifier(&f.expr),
        Node::IndexExpr(idx) => place_root_identifier(&idx.target),
        Node::DerefExpr(d) => place_root_identifier(&d.expr),
        Node::BorrowExpr(b) => place_root_identifier(&b.expr),
        Node::MoveExpr(m) => place_root_identifier(&m.expr),
        _ => None,
    }
}

/// The field of the root place in a place expression
/// (`p.a.b` -> `a`; `p.items[0]` -> `items`).
fn place_first_field(mut expr: &Node) -> Option<String> {
    let mut first: Option<String> = None;
    loop {
        match expr {
            Node::FieldExpr(f) => {
                first = Some(f.field_name.clone());
                expr = &f.expr;
            }
            Node::IndexExpr(idx) => expr = &idx.target,
            Node::DerefExpr(d) => expr = &d.expr,
            Node::BorrowExpr(b) => expr = &b.expr,
            Node::MoveExpr(m) => expr = &m.expr,
            _ => break,
        }
    }
    first
}

impl<'c> Codegen<'c> {
    /// Clear every B1 taint record for a binding (rebind hygiene).
    pub(crate) fn clear_binding_taint(&mut self, name: &str) {
        self.tainted_bindings.remove(name);
        self.update_moved_fields.retain(|(root, _)| root != name);
    }

    /// Apply a B1 taint decision for a binding (after
    /// `clear_binding_taint`): record the taint form, and for a
    /// tainted DeepStruct suppress every deep field drop through
    /// the existing `update_moved_fields` machinery
    /// (`emit_struct_field_drops` consults it with the binding as
    /// taint root), so the release frees only the fresh husk.
    fn apply_taint_decision(&mut self, name: &str, decision: TaintDecision, plan: &DropPlan) {
        match decision {
            TaintDecision::None => {}
            TaintDecision::Tainted(form) => {
                if let DropPlan::DeepStruct(fields) = plan {
                    for field in fields {
                        self.update_moved_fields
                            .insert((name.to_string(), field.name.clone()));
                    }
                }
                self.tainted_bindings.insert(name.to_string(), form);
            }
            TaintDecision::MovedOut(root, field) => {
                self.update_moved_fields.insert((root, field));
            }
        }
    }

    /// Classify one let/assign value for B1 taint and record it
    /// (decide-then-apply: the decision reads the pre-rebind
    /// taint state, so `x = x.field` judges the OLD x).
    /// No-op for plan-None values. Arc/Rc/Box RESULTS are never
    /// tainted (bump-compensated; tainting them would leak
    /// counts) — an M1 move-out of an Arc/Rc field is still
    /// recorded, with the PARAM-source exception below.
    pub(crate) fn record_value_taint(&mut self, name: &str, value: &Node, value_ty: &Type) {
        let plan = self.plan_value_drop(value_ty);
        if plan == DropPlan::None {
            self.clear_binding_taint(name);
            return;
        }
        let decision = self.decide_value_taint(value, value_ty, &plan);
        self.clear_binding_taint(name);
        self.apply_taint_decision(name, decision, &plan);
    }

    /// Pure classification half of `record_value_taint`.
    fn decide_value_taint(&self, value: &Node, value_ty: &Type, plan: &DropPlan) -> TaintDecision {
        let is_smart = matches!(
            plan,
            DropPlan::ArcPtr(_) | DropPlan::RcPtr(_) | DropPlan::BoxPtr(_)
        );
        match value {
            Node::MoveExpr(m) => self.decide_value_taint(&m.expr, value_ty, plan),
            Node::Identifier(id) => {
                if is_smart {
                    return TaintDecision::None;
                }
                let resolved = self.resolve_binding_name(id);
                if let Some(form) = self
                    .tainted_bindings
                    .get(&resolved)
                    .copied()
                    .or_else(|| self.tainted_bindings.get(id).copied())
                {
                    return TaintDecision::Tainted(form);
                }
                TaintDecision::None
            }
            Node::CallExpr(call) => {
                // RUST-STAGE10-CLONE-EXTRACT (M2 refinement): a
                // clone-extract callee returns a fresh-owned value,
                // so its result never aliases the borrow arg — the
                // clone replaces the taint for this shape.
                if is_smart || is_clone_callee(call) || self.is_clone_extract_callee(call) {
                    return TaintDecision::None;
                }
                if self.call_result_aliases_borrow(call, value_ty) {
                    return TaintDecision::Tainted(TaintForm::FreshHusk);
                }
                TaintDecision::None
            }
            Node::FieldExpr(_) | Node::IndexExpr(_) | Node::DerefExpr(_) => {
                let Some(root) = place_root_identifier(value) else {
                    return TaintDecision::None;
                };
                let resolved_root = self.resolve_binding_name(root);
                let root_ty = self
                    .local_binding_type_for_name(&resolved_root)
                    .or_else(|| self.local_binding_type_for_name(root));
                let root_tainted = self.tainted_bindings.contains_key(&resolved_root)
                    || self.tainted_bindings.contains_key(root);
                let root_borrow = root_ty
                    .as_ref()
                    .map(is_borrow_like_type)
                    .unwrap_or(false)
                    || self.borrow_param_bindings.contains(&resolved_root)
                    || self.borrow_param_bindings.contains(root);
                if root_tainted || root_borrow {
                    if is_smart {
                        return TaintDecision::None;
                    }
                    return TaintDecision::Tainted(TaintForm::AliasedHusk);
                }
                // Owned root.
                if let Node::FieldExpr(_) = value {
                    if let Some(first_field) = place_first_field(value) {
                        // Arc/Rc field of a PARAM root: the
                        // call-argument copy clone-bumped it, and
                        // the source's plan drop is the bump's
                        // compensation (into.rs struct-update
                        // rule) — never taint it. (Box is NOT
                        // excepted: a moved-out Box field still
                        // needs the source drop suppressed.)
                        let arc_rc = matches!(
                            plan,
                            DropPlan::ArcPtr(_) | DropPlan::RcPtr(_)
                        );
                        let is_param = self.param_bindings.contains(&resolved_root)
                            || self.param_bindings.contains(root);
                        if !(arc_rc && is_param) {
                            return TaintDecision::MovedOut(resolved_root, first_field);
                        }
                    }
                    return TaintDecision::None;
                }
                if matches!(value, Node::IndexExpr(_)) {
                    // No field to record; the element still lives
                    // in the parent's buffer.
                    if is_smart {
                        return TaintDecision::None;
                    }
                    return TaintDecision::Tainted(TaintForm::AliasedHusk);
                }
                // DerefExpr with an owned root: the result aliases
                // the cell's payload (task 72 census: `let s = *rc` /
                // `*arc` / `*box` is a VIEW into the smart-pointer
                // cell, whose drop frees the payload -- freeing the
                // view as well double-frees it: def_r8 SIGSEGV x3).
                // Mirror IndexExpr: taint AliasedHusk. Smart-pointer
                // results stay untainted (bump-compensated).
                if is_smart {
                    return TaintDecision::None;
                }
                TaintDecision::Tainted(TaintForm::AliasedHusk)
            }
            _ => TaintDecision::None,
        }
    }

    /// Whether an expression's value derives from a borrow: a
    /// `&e` expression, a borrow-typed or tainted binding, or a
    /// place read / call whose root / argument is borrow-derived.
    fn expr_is_borrow_derived(&self, expr: &Node) -> bool {
        match expr {
            Node::BorrowExpr(_) => true,
            Node::Identifier(name) => {
                let resolved = self.resolve_binding_name(name);
                let ty = self
                    .local_binding_type_for_name(&resolved)
                    .or_else(|| self.local_binding_type_for_name(name));
                ty.as_ref().map(is_borrow_like_type).unwrap_or(false)
                    || self.borrow_param_bindings.contains(&resolved)
                    || self.borrow_param_bindings.contains(name)
                    || self.tainted_bindings.contains_key(&resolved)
                    || self.tainted_bindings.contains_key(name)
            }
            Node::FieldExpr(f) => self.expr_is_borrow_derived(&f.expr),
            Node::IndexExpr(idx) => self.expr_is_borrow_derived(&idx.target),
            Node::DerefExpr(d) => self.expr_is_borrow_derived(&d.expr),
            Node::MoveExpr(m) => self.expr_is_borrow_derived(&m.expr),
            Node::CallExpr(call) => call.args.iter().any(|a| self.expr_is_borrow_derived(a)),
            _ => false,
        }
    }

    /// M2-call: the call takes a borrow-derived argument whose
    /// inner type is the result type, or a struct holding a field
    /// of the result type — so the result may alias the borrowed
    /// storage. (Exact-type rule per the B1 design; widen only if
    /// the gate shows a miss.)
    fn call_result_aliases_borrow(&self, call: &CallExpr, result_ty: &Type) -> bool {
        for arg in &call.args {
            if !self.expr_is_borrow_derived(arg) {
                continue;
            }
            let Some(arg_ty) = self.expr_type(arg) else {
                continue;
            };
            let inner = peel_borrow_pointer_type(&arg_ty);
            if inner == result_ty {
                return true;
            }
            if self.struct_has_field_of_type(inner, result_ty) {
                return true;
            }
        }
        false
    }

    /// Whether a struct type has a field of exactly the target
    /// type (generic parameters substituted).
    fn struct_has_field_of_type(&self, ty: &Type, target: &Type) -> bool {
        let Type::UserDefined(u) = ty else {
            return false;
        };
        let Some(decl) = place::struct_decl_lookup(self.check, &u.name) else {
            return false;
        };
        decl.fields.iter().any(|f| {
            let field_ty = drop_plan_substitute(&f.ty, &decl.generics, &u.generics);
            field_ty == *target
        })
    }
}

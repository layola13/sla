//! Iterator-window reduction machinery, ported from codegen.zig
//! P1 (LINE-FITER-TEXT-PORT, DEF-RUST-FITER-WINDOW-MISSING): an
//! iterator chain reduced to a window over its source collection —
//! logical position p reads source index (rev ? start - p :
//! start + p) for p in [0, count), where start/count are computed
//! from the source length and the skip/take stages at emission
//! time (codegen.zig:143 `IterChainPlan`, :19417 `iterChainPlan`,
//! :19465 `genIterWindowTerminal`, dispatch :21773-21826).
//!
//! Before this port the lane had no window arm at all: a call like
//! `a.iter().sum()` fell through the whole gen_expr_call cascade
//! into the EXPAND catch-all and emitted `EXPAND iter` /
//! `EXPAND sum` for macros that exist in no std tree, so the host
//! rejected the program with UndefinedMacro 1060
//! (test_unit_bend2_parity_array). The Zig lane compiles the same
//! shapes through this machinery; the checker side lives in
//! sla-checker (check_call.rs `count`/`max` arms next to `sum`,
//! check_receiver_leaves2.rs `rev`/`skip`/`take` leaf arms).
//!
//! Node-identity discipline: this lane's checker facts are keyed
//! by node ADDRESS (sla-lowering `node_key`), so a cloned node
//! loses every fact (expr_type, call symbols, cleanups). The Zig
//! original stores raw node pointers in the plan; this port
//! therefore walks and emits DIRECT chains over the original
//! nodes in a single pass at the dispatch site, and only HELD
//! chain bindings (recorded at a let, consumed at a later
//! statement) travel as owned clones — with the source type
//! recorded at the binding site, where the original node was
//! still at hand. Held sources in practice are identifiers or
//! literals, whose emission is name/value based and exact; a
//! composite held source is regenerated structurally.
//!
//! Coverage: terminals sum / count / max over Vec, Array and Slice
//! sources; stages rev / skip / take / copied over iter() /
//! into_iter() chains, direct or held in a let binding. Deviations
//! from the Zig shape, both deliberate: (1) `copied()` is a
//! transparent stage in the plan walk — Zig's walk refuses it and
//! its historical fused sum arms handle it instead (:21842); this
//! lane has no historical arms, so transparency is the
//! behaviour-preserving equivalent. (2) Slice sources are claimed
//! by the window emitter — Zig routes plain slice sums through
//! its historical genSliceIterSum, which this lane never had; the
//! window loop over Slice_ptr/Slice_len reaches the same program
//! behaviour. NOT covered: find (inline-closure terminal — a
//! separate form), collect over a window, map/filter stages,
//! GrowVec sources (Zig's emitter errors on them too: its source
//! dispatch knows only vecElementType / arrayType).

use sla_ast::{BinaryExpr, BinaryOp, Literal, Node, Type};
use sla_lowering::{call_arg_needs_release, is_unsigned_integer_type};

use crate::codegen::{type_size, type_string, vec_element_slot_size, Codegen};
use crate::{unsupported, Result, TextError};

/// codegen.zig:143 `IterChainPlan`, as stored for a HELD chain
/// binding (`let c = v.iter().rev();`): owned node clones plus
/// the source type recorded at the binding site (see the module
/// head for why the type travels with the plan).
#[derive(Debug, Clone)]
pub(crate) struct IterChainPlan {
    pub(crate) source: Node,
    pub(crate) rev: bool,
    pub(crate) skip: Option<Node>,
    pub(crate) take: Option<Node>,
    pub(crate) source_ty: Option<Type>,
}

/// A window plan over borrowed nodes, ready for emission: the
/// direct path borrows the receiver tree's original nodes; the
/// held path borrows the dispatch site's local clone of the
/// stored plan, with `source_ty` pre-recorded.
struct WindowPlan<'n> {
    source: &'n Node,
    source_ty: Option<Type>,
    rev: bool,
    skip: Option<&'n Node>,
    take: Option<&'n Node>,
}

/// codegen.zig `IterWindowKind` (the kinds this port emits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IterWindowKind {
    Sum,
    Count,
    Max,
}

/// lowering_rules.zig:4660 `isIterCall` / :4665 `isIntoIterCall`:
/// pure call-name facts.
fn is_iter_or_into_iter_call(call: &sla_ast::CallExpr) -> bool {
    call.func_name == "iter" || call.func_name == "into_iter"
}

impl<'c> Codegen<'c> {
    /// codegen.zig:19417 `iterChainPlan`, reference form: reduce
    /// a direct iterator chain (no held-binding composition) to
    /// its window plan over the ORIGINAL nodes. Returns None for
    /// any other shape so callers fall through to their
    /// historical paths.
    fn iter_chain_plan_ref<'n>(&self, recv: &'n Node) -> Option<WindowPlan<'n>> {
        let mut rev = false;
        let mut skip: Option<&Node> = None;
        let mut take: Option<&Node> = None;
        let mut cur: &Node = recv;
        loop {
            match cur {
                Node::CallExpr(call) => {
                    if is_iter_or_into_iter_call(call) && call.args.len() == 1 {
                        return Some(WindowPlan {
                            source: &call.args[0],
                            source_ty: None,
                            rev,
                            skip,
                            take,
                        });
                    }
                    if call.func_name == "rev" && call.args.len() == 1 {
                        rev = !rev;
                        cur = &call.args[0];
                        continue;
                    }
                    if call.func_name == "skip" && call.args.len() == 2 {
                        if skip.is_some() {
                            return None;
                        }
                        skip = Some(&call.args[1]);
                        cur = &call.args[0];
                        continue;
                    }
                    if call.func_name == "take" && call.args.len() == 2 {
                        if take.is_some() {
                            return None;
                        }
                        take = Some(&call.args[1]);
                        cur = &call.args[0];
                        continue;
                    }
                    // Deliberate deviation (see the module head):
                    // `copied()` is transparent in this walk.
                    if call.func_name == "copied" && call.args.len() == 1 {
                        cur = &call.args[0];
                        continue;
                    }
                    return None;
                }
                _ => return None,
            }
        }
    }

/// Compose two skip amounts additively: skip(m) then skip(n) is
/// skip(m+n). Folds to a single integer literal when both amounts
/// are literals (the common case, incl. H5's 1 + 1 = 2); otherwise
/// synthesizes a `BinaryExpr(Add)` node, which the window loop
/// generates as an ordinary expression (its skip slot is filled
/// via `gen_expr`, `gen_iter_window_loop`).
fn compose_skip_add(outer: Node, inner: Node) -> Node {
    if let (Node::Literal(Literal::IntVal(a)), Node::Literal(Literal::IntVal(b))) =
        (&outer, &inner)
    {
        if let Some(sum) = a.checked_add(*b) {
            return Node::Literal(Literal::IntVal(sum));
        }
    }
    Node::BinaryExpr(BinaryExpr {
        op: BinaryOp::Add,
        left: Box::new(outer),
        right: Box::new(inner),
    })
}

/// Compose two take amounts by min: take(m) then take(n) is
/// take(min(m, n)). Only folds when both amounts are integer
/// literals; otherwise returns None (a runtime min has no
/// single-node form, so the caller refuses as before).
fn compose_take_min(outer: Node, inner: Node) -> Option<Node> {
    if let (Node::Literal(Literal::IntVal(a)), Node::Literal(Literal::IntVal(b))) =
        (&outer, &inner)
    {
        return Some(Node::Literal(Literal::IntVal((*a).min(*b))));
    }
    None
}

    /// The held-binding composition of `iterChainPlan`: walk the
    /// receiver's stage calls (references) down to an identifier
    /// with a recorded plan and compose. On a hit, returns the
    /// composed plan as owned clones (plus the held plan's
    /// recorded source type). Held stages are the inner stages;
    /// an outer skip over a held skip COMPOSES additively
    /// (skip(m) then skip(n) is skip(m+n) — H5 emission
    /// completion, DEF-RUST-CODEGEN-HELDCHAIN-STAGE-EMISSION);
    /// an outer take over a held take composes by min when both
    /// amounts are integer literals, and still refuses (None)
    /// otherwise — a runtime min has no single-node form.
    /// `rev` composes by XOR, as before.
    fn iter_chain_plan_held(&self, recv: &Node) -> Option<IterChainPlan> {
        let mut rev = false;
        let mut skip: Option<Node> = None;
        let mut take: Option<Node> = None;
        let mut cur: &Node = recv;
        loop {
            match cur {
                Node::CallExpr(call) => {
                    if call.func_name == "rev" && call.args.len() == 1 {
                        rev = !rev;
                        cur = &call.args[0];
                        continue;
                    }
                    if call.func_name == "skip" && call.args.len() == 2 {
                        if skip.is_some() {
                            return None;
                        }
                        skip = Some(call.args[1].clone());
                        cur = &call.args[0];
                        continue;
                    }
                    if call.func_name == "take" && call.args.len() == 2 {
                        if take.is_some() {
                            return None;
                        }
                        take = Some(call.args[1].clone());
                        cur = &call.args[0];
                        continue;
                    }
                    if call.func_name == "copied" && call.args.len() == 1 {
                        cur = &call.args[0];
                        continue;
                    }
                    return None;
                }
                Node::Identifier(name) => {
                    let resolved = self.resolve_binding_name(name);
                    let held = self.iter_chain_bindings.get(&resolved)?;
                    // Compose outer stages over the held plan
                    // (H5 emission completion): skip adds, take
                    // takes the min. A non-literal take-over-take
                    // still refuses (None), as before.
                    let skip = match (skip, held.skip.clone()) {
                        (Some(outer), Some(inner)) => {
                            Some(Self::compose_skip_add(outer, inner))
                        }
                        (Some(outer), None) => Some(outer),
                        (None, Some(inner)) => Some(inner),
                        (None, None) => None,
                    };
                    let take = match (take, held.take.clone()) {
                        (Some(outer), Some(inner)) => {
                            match Self::compose_take_min(outer, inner) {
                                Some(min) => Some(min),
                                None => return None,
                            }
                        }
                        (Some(outer), None) => Some(outer),
                        (None, Some(inner)) => Some(inner),
                        (None, None) => None,
                    };
                    return Some(IterChainPlan {
                        source: held.source.clone(),
                        rev: rev != held.rev,
                        skip,
                        take,
                        source_ty: held.source_ty.clone(),
                    });
                }
                _ => return None,
            }
        }
    }

    /// Record a held chain binding at a let site (called from
    /// gen_let_stmt, mirroring codegen.zig:15961): if the let's
    /// value is an iterator chain, reduce it and return the plan
    /// to store. The source type is read from the ORIGINAL
    /// source node here, at the one site that still holds it.
    pub(crate) fn iter_chain_plan_for_let(&self, value: &Node) -> Option<IterChainPlan> {
        if let Some(plan) = self.iter_chain_plan_ref(value) {
            return Some(IterChainPlan {
                source: plan.source.clone(),
                rev: plan.rev,
                skip: plan.skip.cloned(),
                take: plan.take.cloned(),
                source_ty: self.expr_type(plan.source),
            });
        }
        // A let that re-binds or extends an existing held chain
        // (`let d = c.skip(1);`): compose through the held map.
        self.iter_chain_plan_held(value)
    }

    /// The dispatch entry of gen_expr_call (codegen.zig:21773):
    /// a sum / count / max call whose single argument reduces to
    /// a window plan. Returns None when the call is not a window
    /// terminal (the cascade then continues to its historical
    /// paths); Some(result) once the shape is claimed.
    pub(crate) fn try_gen_iter_window_terminal(
        &mut self,
        call: &sla_ast::CallExpr,
    ) -> Option<Result<String>> {
        if call.associated_target.is_some() || call.callee.is_some() {
            return None;
        }
        let kind = match call.func_name.as_str() {
            "sum" => IterWindowKind::Sum,
            "count" => IterWindowKind::Count,
            "max" => IterWindowKind::Max,
            _ => return None,
        };
        if call.args.len() != 1 {
            return None;
        }
        // Direct chain: walk and emit over the original nodes.
        if let Some(plan) = self.iter_chain_plan_ref(&call.args[0]) {
            return Some(self.gen_iter_window_terminal(&plan, kind));
        }
        // Held chain binding (possibly under outer stages):
        // compose into a local clone and emit from it.
        if let Some(held) = self.iter_chain_plan_held(&call.args[0]) {
            let plan = WindowPlan {
                source: &held.source,
                source_ty: held.source_ty.clone(),
                rev: held.rev,
                skip: held.skip.as_ref(),
                take: held.take.as_ref(),
            };
            return Some(self.gen_iter_window_terminal(&plan, kind));
        }
        None
    }

    /// codegen.zig:19465 `genIterWindowTerminal`: emit one
    /// iterator-window terminal (sum / count / max) over a Vec,
    /// Array or Slice source. The window's (start, count) pair is
    /// computed branch-free into stack slots (saturating skip,
    /// clamped take), then a single indexed loop walks logical
    /// positions; rev reads the window backwards. Mirrors the
    /// slot discipline of this lane's gen_string_collect.
    fn gen_iter_window_terminal(
        &mut self,
        plan: &WindowPlan,
        kind: IterWindowKind,
    ) -> Result<String> {
        let source = plan.source;
        let source_ty = match &plan.source_ty {
            Some(ty) => ty.clone(),
            None => self.expr_type(source).ok_or_else(|| TextError::MissingType {
                context: "iter window source type".to_string(),
            })?,
        };
        let peeled = sla_lowering::peel_borrow_pointer_type(&source_ty).clone();

        // Source acquisition: (elem, stride, data_reg, len_reg)
        // plus the release bookkeeping of the Zig arms.
        let elem_ty: Type;
        let stride: usize;
        let mut data_reg: String;
        let len_reg: String;
        // Vec/Slice arms: the container register, released at the
        // tail iff the source expression needs it. Array arm: the
        // data register IS the source register.
        let mut container_reg: Option<String> = None;
        let mut base_extra_reg: Option<String> = None;
        let mut vec_source = false;
        if let Some((is_growvec, elem)) =
            crate::place::vec_like_element_type(&source_ty)
        {
            if is_growvec {
                return unsupported(
                    "iter window over GrowVec source",
                    "codegen.zig:19465 (genIterWindowTerminal source dispatch)",
                );
            }
            elem_ty = elem;
            stride = vec_element_slot_size(&elem_ty);
            let vreg = self.gen_expr(source)?;
            len_reg = self.new_tmp();
            self.ins(&format!("{len_reg} = call @sa_vec_len(&{vreg})"));
            data_reg = self.new_tmp();
            self.ins(&format!("{data_reg} = load {vreg}+Vec_ptr as ptr"));
            container_reg = Some(vreg);
            vec_source = true;
        } else if let Type::Array(arr) = &peeled {
            elem_ty = (*arr.elem).clone();
            stride = type_size(&elem_ty);
            let arr_len = arr.len;
            let base_source_reg = self.gen_expr(source)?;
            data_reg = base_source_reg.clone();
            if let Node::Identifier(name) = source {
                if self.global_const_bindings.contains(name) {
                    let addr_reg = self.new_tmp();
                    self.ins(&format!("{addr_reg} = &{base_source_reg}"));
                    base_extra_reg = Some(addr_reg.clone());
                    data_reg = addr_reg;
                }
            }
            len_reg = self.new_tmp();
            self.emit_int_const(&len_reg, arr_len as i64);
        } else if let Some(elem) = crate::place::slice_element_type(&source_ty) {
            elem_ty = elem;
            stride = type_size(&elem_ty);
            let sreg = self.gen_expr(source)?;
            data_reg = self.new_tmp();
            self.ins(&format!("{data_reg} = load {sreg}+Slice_ptr as ptr"));
            len_reg = self.new_tmp();
            self.ins(&format!("{len_reg} = load {sreg}+Slice_len as u64"));
            container_reg = Some(sreg);
        } else {
            return unsupported(
                "iter window source shape",
                "codegen.zig:19465 (genIterWindowTerminal source dispatch)",
            );
        }
        self.gen_iter_window_loop(
            plan,
            kind,
            source,
            &elem_ty,
            stride,
            data_reg,
            len_reg,
            container_reg,
            base_extra_reg,
            vec_source,
        )
    }

    /// The window body of gen_iter_window_terminal: skip/count
    /// slots, the reduction loop and the terminal tail. Split out
    /// only to keep the source dispatch readable; the emission
    /// order is exactly the Zig function's.
    #[allow(clippy::too_many_arguments)]
    fn gen_iter_window_loop(
        &mut self,
        plan: &WindowPlan,
        kind: IterWindowKind,
        source: &Node,
        elem_ty: &Type,
        stride: usize,
        data_reg: String,
        len_reg: String,
        container_reg: Option<String>,
        base_extra_reg: Option<String>,
        vec_source: bool,
    ) -> Result<String> {
        let elem_str = type_string(elem_ty);
        // Item load type: Vec slots hold the element word (this
        // lane's index-read convention — scalars as their type,
        // anything else as the stored pointer word); inline
        // sources (Array / Slice) load as the element type.
        let item_load_str = if vec_source {
            match elem_ty {
                Type::Primitive(_) => elem_str,
                _ => "ptr",
            }
        } else {
            elem_str
        };

        // skip value into a slot (0 when no skip stage).
        let skip_slot = self.new_tmp();
        self.ins(&format!("{skip_slot} = stack_alloc 8"));
        if let Some(skip_expr) = plan.skip {
            let skip_reg = self.gen_expr(skip_expr)?;
            self.ins(&format!("store {skip_slot}+0, {skip_reg} as u64"));
            if call_arg_needs_release(skip_expr) {
                self.emit_release(&skip_reg)?;
            }
        } else {
            self.ins(&format!("store {skip_slot}+0, 0 as u64"));
        }

        // count = min(len - skip saturating, take?) into a slot.
        let count_slot = self.new_tmp();
        self.ins(&format!("{count_slot} = stack_alloc 8"));
        {
            let skip_val = self.new_tmp();
            self.ins(&format!("{skip_val} = load {skip_slot}+0 as u64"));
            let avail_raw = self.new_tmp();
            self.ins(&format!("{avail_raw} = sub {len_reg}, {skip_val}"));
            let enough = self.new_tmp();
            self.ins(&format!("{enough} = uge {len_reg}, {skip_val}"));
            let mask = self.new_tmp();
            self.ins(&format!("{mask} = sub 0, {enough}"));
            let mut count_reg = self.new_tmp();
            self.ins(&format!("{count_reg} = and {avail_raw}, {mask}"));
            self.emit_release(&avail_raw)?;
            self.emit_release(&enough)?;
            self.emit_release(&mask)?;
            self.emit_release(&skip_val)?;
            if let Some(take_expr) = plan.take {
                let take_reg = self.gen_expr(take_expr)?;
                let less = self.new_tmp();
                self.ins(&format!("{less} = ult {count_reg}, {take_reg}"));
                let mask2 = self.new_tmp();
                self.ins(&format!("{mask2} = sub 0, {less}"));
                let diff = self.new_tmp();
                self.ins(&format!("{diff} = xor {count_reg}, {take_reg}"));
                let masked = self.new_tmp();
                self.ins(&format!("{masked} = and {diff}, {mask2}"));
                let new_count = self.new_tmp();
                self.ins(&format!("{new_count} = xor {take_reg}, {masked}"));
                self.emit_release(&less)?;
                self.emit_release(&mask2)?;
                self.emit_release(&diff)?;
                self.emit_release(&masked)?;
                self.emit_release(&count_reg)?;
                if call_arg_needs_release(take_expr) {
                    self.emit_release(&take_reg)?;
                }
                count_reg = new_count;
            }
            self.ins(&format!("store {count_slot}+0, {count_reg} as u64"));
            self.emit_release(&count_reg)?;
        }

        if kind == IterWindowKind::Count {
            let result_reg = self.new_tmp();
            self.ins(&format!("{result_reg} = load {count_slot}+0 as u64"));
            self.emit_release(&len_reg)?;
            self.release_iter_window_source(
                source,
                &data_reg,
                &container_reg,
                &base_extra_reg,
            )?;
            return Ok(result_reg);
        }

        // start = rev ? (len - 1 - skip) : skip, into a slot. When
        // the window is empty the rev start wraps; it is never read.
        let start_slot = self.new_tmp();
        self.ins(&format!("{start_slot} = stack_alloc 8"));
        {
            let skip_val = self.new_tmp();
            self.ins(&format!("{skip_val} = load {skip_slot}+0 as u64"));
            if plan.rev {
                let lenm1 = self.new_tmp();
                self.ins(&format!("{lenm1} = sub {len_reg}, 1"));
                let start_reg = self.new_tmp();
                self.ins(&format!("{start_reg} = sub {lenm1}, {skip_val}"));
                self.ins(&format!("store {start_slot}+0, {start_reg} as u64"));
                self.emit_release(&lenm1)?;
                self.emit_release(&start_reg)?;
            } else {
                self.ins(&format!("store {start_slot}+0, {skip_val} as u64"));
            }
            self.emit_release(&skip_val)?;
        }

        // Terminal state slots.
        let acc_slot = self.new_tmp();
        self.ins(&format!("{acc_slot} = stack_alloc {}", stride.max(8)));
        self.ins(&format!("store {acc_slot}+0, 0 as {elem_str}"));
        let has_slot = self.new_tmp();
        self.ins(&format!("{has_slot} = stack_alloc 8"));
        self.ins(&format!("store {has_slot}+0, 0 as u64"));

        let idx_slot = self.new_tmp();
        self.ins(&format!("{idx_slot} = stack_alloc 8"));
        self.ins(&format!("store {idx_slot}+0, 0 as u64"));
        let head_label = self.new_label("L_ITERWIN_HEAD");
        let body_label = self.new_label("L_ITERWIN_BODY");
        let end_label = self.new_label("L_ITERWIN_END");
        self.ins(&format!("jmp {head_label}"));
        self.wl("");
        self.wl(&format!("{head_label}:"));
        let idx_reg = self.new_tmp();
        self.ins(&format!("{idx_reg} = load {idx_slot}+0 as u64"));
        let count_val = self.new_tmp();
        self.ins(&format!("{count_val} = load {count_slot}+0 as u64"));
        let more_reg = self.new_tmp();
        self.ins(&format!("{more_reg} = ult {idx_reg}, {count_val}"));
        self.ins(&format!("br {more_reg} -> {body_label}, {end_label}"));
        self.wl("");
        self.wl(&format!("{body_label}:"));
        self.ins(&format!("!{more_reg}"));
        self.emit_release(&count_val)?;
        let start_val = self.new_tmp();
        self.ins(&format!("{start_val} = load {start_slot}+0 as u64"));
        let src_idx = self.new_tmp();
        if plan.rev {
            self.ins(&format!("{src_idx} = sub {start_val}, {idx_reg}"));
        } else {
            self.ins(&format!("{src_idx} = add {start_val}, {idx_reg}"));
        }
        self.emit_release(&start_val)?;
        let off_reg = self.new_tmp();
        self.ins(&format!("{off_reg} = mul {src_idx}, {stride}"));
        self.emit_release(&src_idx)?;
        let slot_reg = self.new_tmp();
        self.ins(&format!("{slot_reg} = ptr_add {data_reg}, {off_reg}"));
        self.emit_release(&off_reg)?;
        let item_reg = self.new_tmp();
        self.ins(&format!("{item_reg} = load {slot_reg}+0 as {item_load_str}"));
        self.emit_release(&slot_reg)?;
        match kind {
            IterWindowKind::Sum => {
                let acc_reg = self.new_tmp();
                self.ins(&format!("{acc_reg} = load {acc_slot}+0 as {elem_str}"));
                let next_acc = self.new_tmp();
                self.ins(&format!("{next_acc} = add {acc_reg}, {item_reg}"));
                self.ins(&format!("store {acc_slot}+0, {next_acc} as {elem_str}"));
                self.emit_release(&acc_reg)?;
                self.emit_release(&next_acc)?;
                self.emit_release(&item_reg)?;
            }
            IterWindowKind::Max => {
                let has_reg = self.new_tmp();
                self.ins(&format!("{has_reg} = load {has_slot}+0 as u64"));
                let best_reg = self.new_tmp();
                self.ins(&format!("{best_reg} = load {acc_slot}+0 as {elem_str}"));
                let cmp_op = if is_unsigned_integer_type(elem_ty) {
                    "ult"
                } else {
                    "slt"
                };
                let better_reg = self.new_tmp();
                self.ins(&format!(
                    "{better_reg} = {cmp_op} {best_reg}, {item_reg}"
                ));
                let first_reg = self.new_tmp();
                self.ins(&format!("{first_reg} = eq {has_reg}, 0"));
                let keep_reg = self.new_tmp();
                self.ins(&format!("{keep_reg} = or {first_reg}, {better_reg}"));
                self.emit_release(&best_reg)?;
                self.emit_release(&better_reg)?;
                self.emit_release(&first_reg)?;
                let keep_label = self.new_label("L_ITERWIN_KEEP");
                let skip_label = self.new_label("L_ITERWIN_SKIP");
                let merge_label = self.new_label("L_ITERWIN_MERGE");
                self.ins(&format!("br {keep_reg} -> {keep_label}, {skip_label}"));
                self.wl("");
                self.wl(&format!("{keep_label}:"));
                self.ins(&format!("!{keep_reg}"));
                self.ins(&format!("store {acc_slot}+0, {item_reg} as {elem_str}"));
                self.ins(&format!("store {has_slot}+0, 1 as u64"));
                self.ins(&format!("jmp {merge_label}"));
                self.wl("");
                self.wl(&format!("{skip_label}:"));
                self.ins(&format!("!{keep_reg}"));
                self.ins(&format!("jmp {merge_label}"));
                self.wl("");
                self.wl(&format!("{merge_label}:"));
                self.emit_release(&has_reg)?;
                self.emit_release(&item_reg)?;
            }
            IterWindowKind::Count => unreachable!(),
        }
        let next_idx = self.new_tmp();
        self.ins(&format!("{next_idx} = add {idx_reg}, 1"));
        self.ins(&format!("store {idx_slot}+0, {next_idx} as u64"));
        self.emit_release(&next_idx)?;
        self.emit_release(&idx_reg)?;
        self.ins(&format!("jmp {head_label}"));
        self.wl("");
        self.wl(&format!("{end_label}:"));
        self.ins(&format!("!{more_reg}"));
        self.emit_release(&idx_reg)?;
        self.emit_release(&len_reg)?;
        self.release_iter_window_source(
            source,
            &data_reg,
            &container_reg,
            &base_extra_reg,
        )?;
        match kind {
            IterWindowKind::Sum => {
                let result_reg = self.new_tmp();
                self.ins(&format!(
                    "{result_reg} = load {acc_slot}+0 as {elem_str}"
                ));
                Ok(result_reg)
            }
            IterWindowKind::Max => {
                let has_reg = self.new_tmp();
                self.ins(&format!("{has_reg} = load {has_slot}+0 as u64"));
                let result_reg = self.new_tmp();
                let some_label = self.new_label("L_ITERWIN_SOME");
                let none_label = self.new_label("L_ITERWIN_NONE");
                let done_label = self.new_label("L_ITERWIN_DONE");
                self.ins(&format!(
                    "br {has_reg} -> {some_label}, {none_label}"
                ));
                self.wl("");
                self.wl(&format!("{some_label}:"));
                self.ins(&format!("!{has_reg}"));
                let best_reg = self.new_tmp();
                self.ins(&format!(
                    "{best_reg} = load {acc_slot}+0 as {elem_str}"
                ));
                self.ins(&format!(
                    "EXPAND OPTION_NEW_SOME {result_reg}, {best_reg}"
                ));
                self.emit_release(&best_reg)?;
                self.ins(&format!("jmp {done_label}"));
                self.wl("");
                self.wl(&format!("{none_label}:"));
                self.ins(&format!("!{has_reg}"));
                self.ins(&format!("EXPAND OPTION_NEW_NONE {result_reg}"));
                self.ins(&format!("jmp {done_label}"));
                self.wl("");
                self.wl(&format!("{done_label}:"));
                Ok(result_reg)
            }
            IterWindowKind::Count => unreachable!(),
        }
    }

    /// The source-release tail shared by the count early return
    /// and the loop tail (codegen.zig genIterWindowTerminal
    /// epilogues): the Vec/Slice data register is a fresh load
    /// temporary and is always released; the container register
    /// is released iff the source expression needs it; in the
    /// Array arm the data register IS the source register.
    fn release_iter_window_source(
        &mut self,
        source: &Node,
        data_reg: &str,
        container_reg: &Option<String>,
        base_extra_reg: &Option<String>,
    ) -> Result<()> {
        if let Some(creg) = container_reg {
            self.emit_release(data_reg)?;
            if call_arg_needs_release(source) {
                self.emit_release(creg)?;
            }
        } else if call_arg_needs_release(source) {
            // Array arm: the data register IS the source register.
            self.emit_release(data_reg)?;
        }
        if let Some(extra) = base_extra_reg {
            if extra != data_reg {
                self.emit_release(extra)?;
            }
        }
        Ok(())
    }
}

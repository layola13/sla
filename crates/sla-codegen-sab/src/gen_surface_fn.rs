//! D surface (surface_call) — the bare-call branches of Zig
//! `genStdSurfaceCall` (sab_codegen.zig, current head), ported
//! for the S5-helperfn slice: the function/method rules keyed
//! on a receiver argument (`len(v)`, `push(v, x)`,
//! `unwrap(o)`, ...) and the constructor rules keyed on the
//! call's result type (bare `Some(x)` / `Ok(x)` / `Err(x)` /
//! `None`), over the D1/D4 std-surface machinery (rules loaded
//! at driver step 2; emission through the F fragment machine).
//!
//! Probe contract mirrors the preload classification in
//! `surface_preload.rs` (Function, then Method, keyed on the
//! first argument's type; Constructor keyed on the result
//! type). A call no rule claims returns `Ok(None)` and keeps
//! falling through `gen_call`'s ordinary chain; a name the
//! program itself defines is a user call and is never claimed.
//! Fallible-method rules (dual ok/out results, e.g. Vec
//! `remove`) are NOT ported here; those calls keep the
//! existing outcome. Nothing here reroutes to the text
//! backend: an unclaimed call stays exactly as loud as before.

use sla_ast::{CallExpr, Node, Type};

use crate::driver::{unsupported, Driver};
use crate::error::Result;
use crate::fragment::FragmentMachine;
use crate::fragment_imports::MachineSurfaceEmitter;
use crate::layout::{element_slot_size, type_base_name};
use crate::surface_query::{
    emit_std_surface_rule, find_std_surface_rule, PendingStdDeps, StdSurfaceValues,
};
use crate::ir::PrimType;
use crate::types::{StdSurfaceArgKind, StdSurfaceRule, StdSurfaceRuleKind};

impl<'c> Driver<'c> {
    /// The bare-call probe chain of `genStdSurfaceCall`,
    /// wired just before `gen_call`'s plain user-function
    /// tail. `Ok(None)` = no rule claimed the call.
    pub(crate) fn gen_bare_surface_call(
        &mut self,
        expr: &Node,
        call: &CallExpr,
    ) -> Result<Option<u32>> {
        if call.associated_target.is_some() || call.callee.is_some() {
            return Ok(None);
        }
        // A name the program itself defines resolves to the
        // user function in the plain tail, never to a rule
        // (same lookup the tail performs, alias included).
        let resolved = self.check.resolve_alias(&call.func_name);
        if self.funcs.contains_key(&resolved) {
            return Ok(None);
        }
        // Zig `genVecLenCall`'s non-rule receivers: the
        // surface table carries `len` rules only for the
        // named containers (Vec/String/GrowVec/HashSet/
        // BTreeSet); a fixed array's length is a compile-time
        // immediate and a slice's length is the second word
        // of its {ptr, len} value.
        if let Some(reg) = self.gen_bare_len_call(call)? {
            return Ok(Some(reg));
        }
        // Receiver-keyed rules: Function, then Method (the
        // preload classification order).
        if !call.args.is_empty() {
            if let Some(receiver_ty) = self.check.expr_type(&call.args[0]).cloned() {
                if let Some(type_name) = type_base_name(&receiver_ty) {
                    for kind in [StdSurfaceRuleKind::Function, StdSurfaceRuleKind::Method] {
                        let rule = find_std_surface_rule(
                            &self.state.program.std_surface_rules,
                            kind,
                            &type_name,
                            Some(&call.func_name),
                        )
                        .cloned();
                        if let Some(rule) = rule {
                            return self.emit_receiver_surface_rule(call, &rule, &receiver_ty);
                        }
                    }
                }
            }
        }
        // Constructor rules keyed on the result type.
        if let Some(result_ty) = self.check.expr_type(expr).cloned() {
            if let Some(type_name) = type_base_name(&result_ty) {
                let rule = find_std_surface_rule(
                    &self.state.program.std_surface_rules,
                    StdSurfaceRuleKind::Constructor,
                    &type_name,
                    Some(&call.func_name),
                )
                .cloned();
                if let Some(rule) = rule {
                    return self.emit_ctor_surface_rule(call, &rule, &result_ty);
                }
            }
        }
        Ok(None)
    }

    /// The `len(x)` shapes with no surface rule, mirroring
    /// Zig `genVecLenCall` (array / slice branches).
    fn gen_bare_len_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.func_name != "len" || call.args.len() != 1 {
            return Ok(None);
        }
        let Some(receiver_ty) = self.check.expr_type(&call.args[0]).cloned() else {
            return Ok(None);
        };
        match bare_len_shape(&receiver_ty) {
            Some(BareLenShape::ArrayImm(len)) => {
                let dst = self.b().new_tmp();
                self.b().emit_assign_imm(dst, len as i64);
                Ok(Some(dst))
            }
            Some(BareLenShape::SliceLenField) => {
                let base_reg = self.gen_expr(&call.args[0])?;
                let dst = self.b().new_tmp();
                // SliceAbi: {ptr +0, len +8}; the len word
                // is u64.
                self.emit_load(dst, base_reg, 8, PrimType::U64);
                if !self.is_local_reg(base_reg) {
                    self.emit_release(base_reg)?;
                }
                Ok(Some(dst))
            }
            None => Ok(None),
        }
    }

    /// Zig `emitStdSurfaceRule` (D4) with the production
    /// emitter (bare-call copy of the helper in
    /// gen_surface_call.rs, renamed to avoid a duplicate
    /// inherent-impl definition). The dep ensurer takes
    /// D4's documented
    /// in-function-body branch (queue into `pending_std_deps`,
    /// flushed at function-body end); the queue fields are
    /// taken out of the program state for the call so the
    /// emitter can hold the whole state mutably, then put
    /// back. The symbol table is cloned for the same reason:
    /// D4 renders every argument text from it before the
    /// emitter appends anything, and every register rendered
    /// here was interned before this call, so the clone is
    /// exact for the values read.
    fn emit_bare_surface_rule(
        &mut self,
        rule: &StdSurfaceRule,
        values: &StdSurfaceValues,
    ) -> Result<()> {
        let symbols: Vec<String> = self.b().symbols().to_vec();
        let mut included = std::mem::take(&mut self.state.program.included_imports);
        let mut pending = std::mem::take(&mut self.state.program.pending_std_deps);
        let result = {
            let mut emitter = MachineSurfaceEmitter {
                machine: FragmentMachine::new(&mut self.state),
                flattener: &mut *self.flattener,
            };
            let mut ensurer = PendingStdDeps::new(&mut included, &mut pending);
            emit_std_surface_rule(&mut emitter, &mut ensurer, rule, values, &symbols)
        };
        self.state.program.included_imports = included;
        self.state.program.pending_std_deps = pending;
        result
    }

    /// Emission for a receiver-keyed function/method rule.
    /// Call arguments map onto rule arguments positionally:
    /// the receiver is the first call argument; `value` /
    /// `index` rule arguments consume the remaining call
    /// arguments in rule order (`out` / `ok` / `elem_size` /
    /// `elem_ty` are not call arguments).
    fn emit_receiver_surface_rule(
        &mut self,
        call: &CallExpr,
        rule: &StdSurfaceRule,
        receiver_ty: &Type,
    ) -> Result<Option<u32>> {
        if call.args.len() != surface_call_arg_count(rule) {
            return Err(unsupported(
                &format!(
                    "std surface call `{}` with an argument count its rule does not take",
                    call.func_name
                ),
                "D surface (surface_call)",
                "21345-21410",
            ));
        }
        let mut values = StdSurfaceValues {
            elem_size: element_slot_size(receiver_ty),
            ..Default::default()
        };
        let dst = if rule.args.contains(&StdSurfaceArgKind::Out) {
            let dst = self.b().new_tmp();
            self.b().record_reg(dst);
            values.out = Some(dst);
            Some(dst)
        } else {
            None
        };
        let mut next = 0usize;
        let mut value_reg = None;
        let mut index_reg = None;
        for kind in &rule.args {
            match kind {
                StdSurfaceArgKind::Receiver => {
                    values.receiver = Some(self.gen_expr(&call.args[next])?);
                    next += 1;
                }
                StdSurfaceArgKind::Value => {
                    let reg = self.gen_expr(&call.args[next])?;
                    values.value = Some(reg);
                    value_reg = Some(reg);
                    next += 1;
                }
                StdSurfaceArgKind::Index => {
                    let reg = self.gen_expr(&call.args[next])?;
                    values.index = Some(reg);
                    index_reg = Some(reg);
                    next += 1;
                }
                _ => {}
            }
        }
        self.emit_bare_surface_rule(rule, &values)?;
        if let Some(reg) = value_reg {
            if rule.consume_value {
                // The rule moves ownership out of the source;
                // a real `move_` retires it in the emitted
                // stream (markConsumed alone would leave it
                // live at function exit).
                self.emit_move(reg);
            } else if !self.is_local_reg(reg) {
                self.emit_release(reg)?;
            }
        }
        if let Some(reg) = index_reg {
            if !self.is_local_reg(reg) {
                self.emit_release(reg)?;
            }
        }
        match dst {
            Some(dst) => Ok(Some(dst)),
            None => {
                // Void rule (e.g. Vec push): the plain-call
                // void arm's sentinel.
                let sentinel = self.b().new_tmp();
                self.b().emit_assign_imm(sentinel, 0);
                Ok(Some(sentinel))
            }
        }
    }

    /// Emission for a constructor rule keyed on the result
    /// type: `(out)` for `None`, `(out, value)` for
    /// `Some` / `Ok` / `Err`. The payload obligation mirrors
    /// the associated shorthand: a non-Copy payload is
    /// consumed by the macro store, never released.
    fn emit_ctor_surface_rule(
        &mut self,
        call: &CallExpr,
        rule: &StdSurfaceRule,
        result_ty: &Type,
    ) -> Result<Option<u32>> {
        let has_value = rule.args.contains(&StdSurfaceArgKind::Value);
        if call.args.len() != surface_call_arg_count(rule) {
            return Err(unsupported(
                &format!(
                    "std surface constructor `{}` with an argument count its rule does not take",
                    call.func_name
                ),
                "D surface (surface_call)",
                "21345-21410",
            ));
        }
        let ctor_value = if has_value {
            Some(self.gen_expr(&call.args[0])?)
        } else {
            None
        };
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let values = StdSurfaceValues {
            out: Some(dst),
            value: ctor_value,
            elem_size: element_slot_size(result_ty),
            ..Default::default()
        };
        self.emit_bare_surface_rule(rule, &values)?;
        if let Some(reg) = ctor_value {
            let value_ty = self.check.expr_type(&call.args[0]).cloned();
            match value_ty {
                Some(ty) if !self.check.type_is_copy_value(&ty) => {
                    self.mark_consumed(reg);
                }
                _ => {
                    if !self.is_local_reg(reg) {
                        self.emit_release(reg)?;
                    }
                }
            }
        }
        Ok(Some(dst))
    }
}

/// The `len` receiver shapes claimed by `gen_bare_len_call`
/// (everything else is rule territory or not a len shape).
#[derive(Debug, PartialEq)]
enum BareLenShape {
    /// `[T; N]` receiver: the length is the immediate `N`.
    ArrayImm(usize),
    /// `Slice<T>` receiver: load the len word at +8.
    SliceLenField,
}

fn bare_len_shape(ty: &Type) -> Option<BareLenShape> {
    match ty {
        Type::Array(arr) => Some(BareLenShape::ArrayImm(arr.len)),
        Type::UserDefined(ud) if ud.name == "Slice" && ud.generics.len() == 1 => {
            Some(BareLenShape::SliceLenField)
        }
        _ => None,
    }
}

/// How many call arguments a rule consumes: one per
/// `receiver` / `value` / `index` rule argument.
fn surface_call_arg_count(rule: &StdSurfaceRule) -> usize {
    rule.args
        .iter()
        .filter(|kind| {
            matches!(
                kind,
                StdSurfaceArgKind::Receiver
                    | StdSurfaceArgKind::Value
                    | StdSurfaceArgKind::Index
            )
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface_load::parse_std_surface_rules;

    // Rule lines copied verbatim from
    // `sla_std/std_surface.sla_meta` (the parser's grammar:
    // member precedes type for function/constructor lines).
    const TABLE: &str = "\
function len Vec sa_std/vec.sa VEC_LEN out,receiver deps=sa_vec_len
method Vec push sa_std/vec.sa VEC_PUSH receiver,value,elem_size deps=sa_vec_push,sa_mem_copy
method Option unwrap sa_std/core/option.sa OPTION_UNWRAP out,receiver
constructor Some Option sa_std/core/option.sa OPTION_NEW_SOME out,value
constructor None Option sa_std/core/option.sa OPTION_NEW_NONE out
fallible_method Vec remove sa_std/vec.sa VEC_REMOVE ok,out,receiver,index,elem_size deps=sa_vec_try_remove panic=86
";

    #[test]
    fn bare_surface_arg_counts_follow_rule_shapes() {
        let rules = parse_std_surface_rules(TABLE).expect("table parses");
        let count = |kind, ty, member| {
            let rule = find_std_surface_rule(&rules, kind, ty, Some(member)).expect("rule found");
            surface_call_arg_count(rule)
        };
        assert_eq!(count(StdSurfaceRuleKind::Function, "Vec", "len"), 1);
        assert_eq!(count(StdSurfaceRuleKind::Method, "Vec", "push"), 2);
        assert_eq!(count(StdSurfaceRuleKind::Method, "Option", "unwrap"), 1);
        assert_eq!(count(StdSurfaceRuleKind::Constructor, "Option", "Some"), 1);
        assert_eq!(count(StdSurfaceRuleKind::Constructor, "Option", "None"), 0);
        // The fallible shape counts both payloads; the probe
        // chain never looks this kind up (documented above).
        assert_eq!(
            count(StdSurfaceRuleKind::FallibleMethod, "Vec", "remove"),
            2
        );
    }

    #[test]
    fn bare_len_shapes_claim_only_array_and_slice() {
        use sla_ast::{ArrayType, Primitive, UserDefined};
        let arr = Type::Array(ArrayType {
            elem: Box::new(Type::Primitive(Primitive::I32)),
            len: 4,
        });
        assert_eq!(bare_len_shape(&arr), Some(BareLenShape::ArrayImm(4)));
        let slice = Type::UserDefined(UserDefined {
            name: "Slice".to_string(),
            generics: vec![Type::Primitive(Primitive::U8)],
        });
        assert_eq!(bare_len_shape(&slice), Some(BareLenShape::SliceLenField));
        // Rule territory (Vec) and non-len shapes stay
        // unclaimed by the bare-len branch.
        let vec = Type::UserDefined(UserDefined {
            name: "Vec".to_string(),
            generics: vec![Type::Primitive(Primitive::I32)],
        });
        assert_eq!(bare_len_shape(&vec), None);
        assert_eq!(bare_len_shape(&Type::Primitive(Primitive::I32)), None);
    }
}

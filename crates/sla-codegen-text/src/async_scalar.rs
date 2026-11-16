//! Async continuation scalar-value emission, mirrored from
//! codegen.zig: emitAsyncContinuationScalarValue (8364),
//! emitAsyncLinearAwaitScalarReady (8786) and
//! emitAsyncGeneralAwaitScalarReady (9301 — byte-identical shape
//! to the linear one over the general plan's offsets), plus the
//! async vtable/poll name helpers (8316-8363). Stage 6.

use sla_lowering::{
    AsyncContinuationScalarPlan, AsyncGeneralAwaitContinuationPlan,
    AsyncLinearAwaitContinuationPlan,
};

use crate::codegen::Codegen;
use crate::Result;

pub(crate) fn async_single_vt(name: &str) -> String {
    format!("SLA_ASYNC_{name}_VT")
}
pub(crate) fn async_single_poll(name: &str) -> String {
    format!("sla_async_{name}_poll")
}
pub(crate) fn async_two_vt(name: &str) -> String {
    format!("SLA_ASYNC_{name}_TWO_AWAIT_VT")
}
pub(crate) fn async_two_poll(name: &str) -> String {
    format!("sla_async_{name}_two_await_poll")
}
pub(crate) fn async_linear_vt(name: &str) -> String {
    format!("SLA_ASYNC_{name}_LINEAR_AWAIT_VT")
}
pub(crate) fn async_linear_poll(name: &str) -> String {
    format!("sla_async_{name}_linear_await_poll")
}
pub(crate) fn async_general_vt(name: &str) -> String {
    format!("SLA_ASYNC_{name}_GENERAL_AWAIT_VT")
}
pub(crate) fn async_general_poll(name: &str) -> String {
    format!("sla_async_{name}_general_await_poll")
}
pub(crate) fn async_loop_vt(name: &str) -> String {
    format!("SLA_ASYNC_{name}_LOOP_AWAIT_VT")
}
pub(crate) fn async_loop_poll(name: &str) -> String {
    format!("sla_async_{name}_loop_await_poll")
}
pub(crate) fn async_join2_vt(name: &str) -> String {
    format!("SLA_ASYNC_{name}_JOIN2_AWAIT_VT")
}
pub(crate) fn async_join2_poll(name: &str) -> String {
    format!("sla_async_{name}_join2_await_poll")
}

impl<'c> Codegen<'c> {
    /// codegen.zig:8364 `emitAsyncContinuationScalarValue`.
    pub(crate) fn emit_async_continuation_scalar_value(
        &mut self,
        scalar: &AsyncContinuationScalarPlan,
        result_reg: &str,
        awaited_reg: &str,
        captured_regs: [Option<&str>; 2],
        prefix: &str,
    ) -> Result<()> {
        let awaited_abs = format!("{prefix}_awaited_abs");
        let awaited_scaled = format!("{prefix}_awaited_scaled");
        let captured_abs = format!("{prefix}_captured_abs");
        let captured_scaled = format!("{prefix}_captured_scaled");
        let expr_sum = format!("{prefix}_expr_sum");
        let captured2_abs = format!("{prefix}_captured2_abs");
        let captured2_scaled = format!("{prefix}_captured2_scaled");
        let expr_sum2 = format!("{prefix}_expr_sum2");

        let mut current_reg: Option<String> = None;
        let mut release_awaited_abs = false;
        let mut release_awaited_scaled = false;
        let mut release_captured_abs = false;
        let mut release_captured_scaled = false;
        let mut release_expr_sum = false;
        let mut release_captured2_abs = false;
        let mut release_captured2_scaled = false;
        let mut release_expr_sum2 = false;

        if scalar.awaited_coeff != 0 {
            if scalar.awaited_coeff == 1 {
                current_reg = Some(awaited_reg.to_string());
            } else if scalar.awaited_coeff == -1 {
                self.ins(&format!("{awaited_scaled} = sub 0, {awaited_reg}"));
                current_reg = Some(awaited_scaled.clone());
                release_awaited_scaled = true;
            } else {
                let abs_coeff = scalar.awaited_coeff.abs();
                self.ins(&format!(
                    "{awaited_abs} = mul {awaited_reg}, {abs_coeff}"
                ));
                release_awaited_abs = true;
                if scalar.awaited_coeff < 0 {
                    self.ins(&format!("{awaited_scaled} = sub 0, {awaited_abs}"));
                    current_reg = Some(awaited_scaled.clone());
                    release_awaited_scaled = true;
                } else {
                    current_reg = Some(awaited_abs.clone());
                }
            }
        }

        if scalar.captured_coeff != 0 {
            let addend_reg = match captured_regs[0] {
                Some(r) => r,
                None => {
                    return crate::unsupported(
                        "async scalar captured reg missing",
                        "codegen.zig:8364",
                    )
                }
            };
            let mut captured_term = addend_reg.to_string();
            let abs_coeff = scalar.captured_coeff.abs();
            if abs_coeff != 1 {
                self.ins(&format!("{captured_abs} = mul {addend_reg}, {abs_coeff}"));
                captured_term = captured_abs.clone();
                release_captured_abs = true;
            }
            if let Some(current) = &current_reg {
                let sum_dest = if scalar.captured2_coeff == 0 && scalar.immediate == 0 {
                    result_reg.to_string()
                } else {
                    expr_sum.clone()
                };
                let op = if scalar.captured_coeff < 0 { "sub" } else { "add" };
                self.ins(&format!("{sum_dest} = {op} {current}, {captured_term}"));
                current_reg = Some(sum_dest);
                release_expr_sum = scalar.captured2_coeff != 0 || scalar.immediate != 0;
            } else if scalar.captured_coeff < 0 {
                self.ins(&format!("{captured_scaled} = sub 0, {captured_term}"));
                current_reg = Some(captured_scaled.clone());
                release_captured_scaled = true;
            } else {
                current_reg = Some(captured_term);
            }
        }

        if scalar.captured2_coeff != 0 {
            let addend_reg = match captured_regs[1] {
                Some(r) => r,
                None => {
                    return crate::unsupported(
                        "async scalar captured2 reg missing",
                        "codegen.zig:8364",
                    )
                }
            };
            let mut captured_term = addend_reg.to_string();
            let abs_coeff = scalar.captured2_coeff.abs();
            if abs_coeff != 1 {
                self.ins(&format!(
                    "{captured2_abs} = mul {addend_reg}, {abs_coeff}"
                ));
                captured_term = captured2_abs.clone();
                release_captured2_abs = true;
            }
            if let Some(current) = &current_reg {
                let sum_dest = if scalar.immediate == 0 {
                    result_reg.to_string()
                } else {
                    expr_sum2.clone()
                };
                let op = if scalar.captured2_coeff < 0 { "sub" } else { "add" };
                self.ins(&format!("{sum_dest} = {op} {current}, {captured_term}"));
                current_reg = Some(sum_dest);
                release_expr_sum2 = scalar.immediate != 0;
            } else if scalar.captured2_coeff < 0 {
                self.ins(&format!("{captured2_scaled} = sub 0, {captured_term}"));
                current_reg = Some(captured2_scaled.clone());
                release_captured2_scaled = true;
            } else {
                current_reg = Some(captured_term);
            }
        }

        if scalar.immediate != 0 {
            let current = current_reg.clone().unwrap_or_else(|| "0".to_string());
            let imm_abs = scalar.immediate.abs();
            let op = if scalar.immediate < 0 { "sub" } else { "add" };
            self.ins(&format!("{result_reg} = {op} {current}, {imm_abs}"));
            current_reg = Some(result_reg.to_string());
        }

        let current = current_reg.unwrap_or_else(|| "0".to_string());
        if current != result_reg {
            self.ins(&format!("{result_reg} = add {current}, 0"));
        }
        if release_expr_sum {
            self.ins(&format!("!{expr_sum}"));
        }
        if release_expr_sum2 {
            self.ins(&format!("!{expr_sum2}"));
        }
        if release_captured2_scaled {
            self.ins(&format!("!{captured2_scaled}"));
        }
        if release_captured2_abs {
            self.ins(&format!("!{captured2_abs}"));
        }
        if release_captured_scaled {
            self.ins(&format!("!{captured_scaled}"));
        }
        if release_captured_abs {
            self.ins(&format!("!{captured_abs}"));
        }
        if release_awaited_scaled {
            self.ins(&format!("!{awaited_scaled}"));
        }
        if release_awaited_abs {
            self.ins(&format!("!{awaited_abs}"));
        }
        Ok(())
    }

    /// Shared body of codegen.zig:8786/9301 (linear/general scalar
    /// ready): accumulate `immediate + sum(coeff_i * value_i)` over
    /// the plan's value slots and complete the poll as ready.
    fn emit_async_scalar_ready(
        &mut self,
        count: usize,
        coeffs: &[i64],
        immediate: i64,
        value_offset: &dyn Fn(usize) -> usize,
    ) -> Result<()> {
        let acc0 = self.new_tmp();
        self.ins(&format!("{acc0} = {immediate}"));
        let mut acc = acc0;
        for i in 0..count {
            let coeff = coeffs[i];
            if coeff == 0 {
                continue;
            }
            let val_name = self.new_tmp();
            self.ins(&format!(
                "{val_name} = load data_slot+{} as u64",
                value_offset(i)
            ));
            let next_acc = self.new_tmp();
            if coeff == 1 {
                self.ins(&format!("{next_acc} = add {acc}, {val_name}"));
            } else if coeff == -1 {
                self.ins(&format!("{next_acc} = sub {acc}, {val_name}"));
            } else {
                let abs_coeff = coeff.abs();
                let scaled = self.new_tmp();
                self.ins(&format!("{scaled} = mul {val_name}, {abs_coeff}"));
                if coeff < 0 {
                    self.ins(&format!("{next_acc} = sub {acc}, {scaled}"));
                } else {
                    self.ins(&format!("{next_acc} = add {acc}, {scaled}"));
                }
                self.ins(&format!("!{scaled}"));
            }
            self.ins(&format!("!{val_name}"));
            self.ins(&format!("!{acc}"));
            acc = next_acc;
        }
        self.ins(&format!("EXPAND POLL_SET_READY out_poll_slot, {acc}"));
        self.ins(&format!("!{acc}"));
        Ok(())
    }

    /// codegen.zig:8786 `emitAsyncLinearAwaitScalarReady`.
    pub(crate) fn emit_async_linear_await_scalar_ready(
        &mut self,
        plan: &AsyncLinearAwaitContinuationPlan,
    ) -> Result<()> {
        let coeffs = plan.scalar.coeffs;
        let count = plan.count;
        self.emit_async_scalar_ready(count, &coeffs, plan.scalar.immediate, &|i| {
            plan.value_offset(i)
        })
    }

    /// codegen.zig:9301 `emitAsyncGeneralAwaitScalarReady`.
    pub(crate) fn emit_async_general_await_scalar_ready(
        &mut self,
        plan: &AsyncGeneralAwaitContinuationPlan,
    ) -> Result<()> {
        let coeffs = plan.scalar.coeffs;
        let count = plan.count;
        self.emit_async_scalar_ready(count, &coeffs, plan.scalar.immediate, &|i| {
            plan.value_offset(i)
        })
    }
}

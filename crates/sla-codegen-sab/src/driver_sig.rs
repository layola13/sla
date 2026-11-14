//! Signature emission for function declarations (split
//! from driver.rs for the 700-line cap): the shared body
//! behind begin_sig, with the variadic form of VaList
//! handling (the VaList parameter is not a signature
//! parameter; va_start materializes it in the body).
//! See driver.rs for the orchestration record.

use sla_ast::{Primitive, Type};
use sla_lowering::{
    by_value_raw_pointer_param, is_borrow_like_type, shared_abi_param_cap, AbiCapKind,
};

use crate::driver::Driver;
use crate::driver_types::{abi_return_prim_type, return_cap_for_type};
use crate::layout::prim_type;
use crate::error::Result;
use crate::ir::{CapPrefix, FunctionKind, FunctionSig, ParamSpec};
use crate::types::Local;

impl<'c> Driver<'c> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin_sig_variadic(
        &mut self,
        lowered: &str,
        kind: FunctionKind,
        f_params: &[sla_ast::Param],
        ret_ty: &Type,
        ignored: bool,
        should_panic: bool,
        variadic: bool,
    ) -> Result<usize> {
        // Zig counter at genFuncSig entry: test_sigs.len is sampled
        // before this sig exists; function_sigs.len is the index
        // `add_sig` is about to return.
        let test_count = self.state.program.test_sigs.len();
        let mut sig = FunctionSig::new(0, lowered, abi_return_prim_type(ret_ty)?, 0);
        sig.kind = kind;
        let sig_index = self.b().add_sig(sig);
        self.b().begin_function(sig_index);
        // Scoped param registers, Zig genFuncSig lines 6055-6068.
        let mut specs = Vec::with_capacity(f_params.len());
        let mut param_ids = Vec::with_capacity(f_params.len());
        for (i, p) in f_params.iter().enumerate() {
            if variadic
                && matches!(*p.ty, sla_ast::Type::Primitive(sla_ast::Primitive::VaList))
            {
                // C-variadic: the VaList parameter is not a
                // signature parameter (rendered as `...`) and
                // gets no local binding — `va_start` defines the
                // list state in a fresh register.
                continue;
            }
            let scoped = format!("{lowered}__param_{i}_{}", p.name);
            let param_id = self.b().intern(&scoped);
            let cap = match shared_abi_param_cap(
                p.is_borrow,
                p.is_move,
                matches!(&*p.ty, Type::Borrow(_)),
                by_value_raw_pointer_param(p),
                self.check.type_is_copy_value(&p.ty),
                is_borrow_like_type(&p.ty),
            ) {
                AbiCapKind::Borrow => CapPrefix::Borrow,
                AbiCapKind::Move => CapPrefix::Move,
                AbiCapKind::Raw => CapPrefix::Raw,
                AbiCapKind::None => CapPrefix::ByValue,
            };
            specs.push(ParamSpec {
                name: p.name.clone(),
                ty: prim_type(&p.ty)?,
                cap,
            });
            param_ids.push(param_id);
            self.b().record_reg(param_id);
            self.state.frame.push_local(Local {
                name: p.name.clone(),
                reg: param_id,
                is_param: true,
                stack_ty: None,
                ty: Some((*p.ty).clone()),
                param_cap: Some(cap),
                is_stack_alloc: false,
            });
            // Zig `pushParamLocal` (locals.rs keeps the same
            // rule in its tested helper): a borrow-typed
            // parameter register IS an object pointer — the
            // genField through-load guards consult this mark.
            if matches!(&*p.ty, Type::Borrow(_)) {
                crate::locals::mark_object_ptr_reg(&mut self.state, param_id);
            }
        }
        let ret_cap = return_cap_for_type(ret_ty);
        self.b().set_sig_params(sig_index, specs, param_ids);
        self.b().set_sig_return_cap(sig_index, ret_cap);
        if kind == FunctionKind::TestFunc {
            self.b().set_sig_test_meta(sig_index, ignored, should_panic);
        }
        self.b().append_decl_inst(sig_index);
        // SAB-D17: only now, with the decl instruction's intern-id
        // operands already captured, does the sig's own id switch
        // to the Zig emission counter (function index + test count
        // sampled at entry).
        let counter_id = (sig_index + test_count) as u32;
        self.b().set_sig_id(sig_index, counter_id);
        Ok(sig_index)
    }
}

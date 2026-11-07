//! Associated-call helpers split out of check_call.rs by the
//! R-2 replay line (700-line discipline: the R-1/R-2 wiring
//! pushed check_call.rs to 713 lines; this arm moves verbatim,
//! behavior unchanged).

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::type_desc;
use sla_ast::types::Type;

/// The `sum` special arm of the call dispatch (Zig sum arm):
/// array element, else sliceElementType, else vecElementType,
/// else growVecElementType — NOT "any generic user type's
/// first parameter" (the phase-1 slice accepted e.g.
/// Option<i32>, which Zig rejects). (GrowVec added with the
/// R-1 port.)
pub(crate) fn check_sum_call(
    _checker: &mut Checker,
    target_ty: &Type,
) -> CheckResult<Type> {
    match target_ty {
        Type::Array(a) => Ok((*a.elem).clone()),
        ref other => {
            match crate::types_util::slice_element_type(other)
                .or_else(|| crate::types_util::vec_element_type(other))
                .or_else(|| crate::check_leaves::ud_inner(other, "GrowVec"))
            {
                Some(elem) => Ok(elem),
                None => Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!("sum over {}", type_desc(other)),
                )),
            }
        }
    }
}

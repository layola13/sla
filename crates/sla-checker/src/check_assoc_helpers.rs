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

/// The shared element extraction of the iterator-window
/// terminals (Zig `iterableElementType`, type_checker.zig
/// :3310): array element, else slice / Vec / GrowVec element.
/// `terminal` names the caller for the mismatch message, in
/// the `check_sum_call` style.
fn window_element_type(terminal: &str, target_ty: &Type) -> CheckResult<Type> {
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
                    format!("{terminal} over {}", type_desc(other)),
                )),
            }
        }
    }
}

/// The `count` special arm (Zig type_checker.zig :10824):
/// the window length as u64.
pub(crate) fn check_count_call(
    _checker: &mut Checker,
    target_ty: &Type,
) -> CheckResult<Type> {
    window_element_type("count", target_ty)?;
    Ok(crate::types_util::u64_ty())
}

/// The `max` special arm (Zig type_checker.zig :10848):
/// Option<elem>, with the numeric element gate (Zig
/// `isNumericType` admits integer and float primitives; the
/// error text is the Zig arm's own).
pub(crate) fn check_max_call(
    _checker: &mut Checker,
    target_ty: &Type,
) -> CheckResult<Type> {
    let elem = window_element_type("max", target_ty)?;
    if !crate::types_util::is_numeric_type(&elem) {
        return Err(Diagnostic::new(
            ErrorKind::TypeMismatch,
            format!(
                "iter max: element type must be numeric (float max is loudly unsupported), got {}",
                type_desc(&elem)
            ),
        ));
    }
    Ok(crate::check_leaves::user_ty("Option", vec![elem]))
}

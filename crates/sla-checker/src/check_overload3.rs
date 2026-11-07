//! Extern contract helpers (R-6 follow-up, split from
//! check_overload2 to respect the 700-line file cap).

use crate::checker::{ExternParam, ExternSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::Checker;

/// Build the checker registration of one `.sai` contract
/// declaration from its parameter parts: each part is
/// (name, spelling, is_borrow, is_move). Front ends convert their
/// parser-level contract records into these parts because
/// sla-checker does not depend on sla-parser.
pub fn extern_sig_from_parts(
    params: &[(String, String, bool, bool)],
    ret_abi: &str,
    fallible: bool,
) -> ExternSig {
    ExternSig {
        move_params: params.iter().map(|p| p.3).collect(),
        params: params
            .iter()
            .map(|p| ExternParam {
                name: p.0.clone(),
                ty: p.1.clone(),
                is_borrow: p.2,
                is_move: p.3,
                is_raw: false,
            })
            .collect(),
        is_variadic: false,
        ret_abi: ret_abi.to_string(),
        fallible,
    }
}

/// Zig `externParamsEqual` (staging :5777): two contract
/// signatures have the same parameter identity when the variadic
/// flag and every parameter's borrow/move/raw flags and trimmed
/// spelling agree.
pub fn extern_params_equal(a: &ExternSig, b: &ExternSig) -> bool {
    if a.is_variadic != b.is_variadic {
        return false;
    }
    // Signatures built without spellings (hand-built unit-test
    // fixtures) fall back to the move-contract identity.
    if a.params.is_empty() && b.params.is_empty() {
        return a.move_params == b.move_params;
    }
    a.params.len() == b.params.len()
        && a.params.iter().zip(b.params.iter()).all(|(p, q)| {
            p.is_borrow == q.is_borrow
                && p.is_move == q.is_move
                && p.is_raw == q.is_raw
                && p.ty.trim() == q.ty.trim()
        })
}

impl Checker {
    /// Zig `registerExternContract` (staging :5819), the .sai
    /// contract registration discipline:
    ///   * an identical redeclaration (same parameters, same
    ///     return, per `externParamsEqual` on spellings) dedupes
    ///     silently — sa_std itself declares some contracts in two
    ///     files, and source expansion may inline several files
    ///     into one load;
    ///   * the same parameters with a DIFFERENT return type are a
    ///     loud Redeclaration;
    ///   * a second DISTINCT declaration promotes the name into
    ///     `extern_overload_sets`: both members move out of
    ///     `extern_funcs` (member keys are a codegen concern; the
    ///     checker resolves against the set itself).
    pub fn register_extern_contract(
        &mut self,
        name: &str,
        sig: ExternSig,
    ) -> CheckResult<()> {
        if let Some(set) = self.extern_overload_sets.get(name) {
            for existing in set {
                if extern_params_equal(existing, &sig) {
                    if existing.ret_abi != sig.ret_abi {
                        return Err(Diagnostic::new(
                            ErrorKind::Redeclaration,
                            format!(
                                "Redeclaration: extern contract `{name}` is declared with the same parameters but a different return type"
                            ),
                        ));
                    }
                    return Ok(());
                }
            }
            let mut set = set.clone();
            set.push(sig);
            self.extern_overload_sets.insert(name.to_string(), set);
            return Ok(());
        }
        if let Some(existing) = self.extern_funcs.get(name).cloned() {
            if extern_params_equal(&existing, &sig) {
                if existing.ret_abi != sig.ret_abi {
                    return Err(Diagnostic::new(
                        ErrorKind::Redeclaration,
                        format!(
                            "Redeclaration: extern contract `{name}` is declared with the same parameters but a different return type"
                        ),
                    ));
                }
                return Ok(());
            }
            self.extern_funcs.remove(name);
            self.extern_overload_sets
                .insert(name.to_string(), vec![existing, sig]);
            return Ok(());
        }
        self.extern_funcs.insert(name.to_string(), sig);
        Ok(())
    }
}

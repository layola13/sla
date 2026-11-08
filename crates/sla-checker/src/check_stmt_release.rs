//! Statement-check release handling split out of check_stmt.rs
//! for the 700-line file cap (body unchanged).

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;

impl Checker {
    /// `release x` (the `!x` statement): explicit consumption with
    /// state pre-checks, mirroring the release arm.
    pub fn check_release(&mut self, name: &str) -> CheckResult<()> {
        match self.lookup_state(name) {
            None => Err(Diagnostic::new(
                ErrorKind::UndefinedVariable,
                format!("UndefinedVariable: `{name}`"),
            )),
            Some(ValueState::Consumed) => Err(Diagnostic::new(
                ErrorKind::UseAfterMove,
                format!("UseAfterMove: `{name}` was already consumed"),
            )),
            Some(ValueState::Uninitialized) => Err(Diagnostic::new(
                ErrorKind::UseBeforeInit,
                format!("UseBeforeInit: `{name}` is not initialized"),
            )),
            Some(ValueState::Active) => self.consume_binding(name, "explicit release"),
        }
    }
}

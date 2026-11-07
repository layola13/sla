//! Diagnostics, mirroring `TypeError` + the `last_error` channel of
//! `type_checker.zig` (lines 7-21, 234-252).
//!
//! Zig model: a check either returns `error.<Kind>` (an error-union tag)
//! or succeeds; the human-readable text travels out-of-band in ONE fixed
//! 1024-byte buffer (`last_error_buf`), formatted by `setError`. Several
//! named sub-diagnostics have NO kind of their own and exist only as
//! message text with a `Name:` prefix (e.g. `LoopConditionalConsume`,
//! named in the 2026-11-06 census as one of the 57 checker diagnostics).
//!
//! Rust model (divergence D-C2 in docs/divergences/checker.md): the
//! diagnostic is a value. `kind` keeps the Zig `TypeError` name so kinds
//! align one-to-one; `marker` carries the named sub-diagnostics Zig
//! smuggles through message text, so they stay machine-distinguishable.

/// Mirrors Zig `TypeError`. Variant order matches type_checker.zig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    UndefinedVariable,
    Redeclaration,
    TypeMismatch,
    UseAfterMove,
    UseBeforeInit,
    InvalidBorrow,
    DereferenceNonPointer,
    FieldNotFound,
    NotAStruct,
    InvalidArgsCount,
    AmbiguousCall,
    CompileError,
    OutOfMemory,
    // R-6 additions: variants the Zig checker gained after the original
    // port slice — `GenericOverloadConflict` (monomorphizer/checker
    // overload discipline) and the lang-gap A_min pair
    // `FallibleResultDiscarded` / `FalliblePayloadUnsupported`
    // (type_checker.zig TypeError, impl-lang-gap phase 1).
    GenericOverloadConflict,
    FallibleResultDiscarded,
    FalliblePayloadUnsupported,
}

impl ErrorKind {
    /// Zig error name, e.g. `error.UseAfterMove` -> `"UseAfterMove"`.
    pub fn name(self) -> &'static str {
        match self {
            ErrorKind::UndefinedVariable => "UndefinedVariable",
            ErrorKind::Redeclaration => "Redeclaration",
            ErrorKind::TypeMismatch => "TypeMismatch",
            ErrorKind::UseAfterMove => "UseAfterMove",
            ErrorKind::UseBeforeInit => "UseBeforeInit",
            ErrorKind::InvalidBorrow => "InvalidBorrow",
            ErrorKind::DereferenceNonPointer => "DereferenceNonPointer",
            ErrorKind::FieldNotFound => "FieldNotFound",
            ErrorKind::NotAStruct => "NotAStruct",
            ErrorKind::InvalidArgsCount => "InvalidArgsCount",
            ErrorKind::AmbiguousCall => "AmbiguousCall",
            ErrorKind::CompileError => "CompileError",
            ErrorKind::OutOfMemory => "OutOfMemory",
            ErrorKind::GenericOverloadConflict => "GenericOverloadConflict",
            ErrorKind::FallibleResultDiscarded => "FallibleResultDiscarded",
            ErrorKind::FalliblePayloadUnsupported => "FalliblePayloadUnsupported",
        }
    }
}

/// One checker diagnostic. Zig keeps exactly one of these at a time
/// (first error aborts the whole check); this port keeps that contract:
/// checking stops at the first `Err`.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub kind: ErrorKind,
    /// Human-readable text, mirroring what Zig's `setError` formatted
    /// into `last_error` (wording is best-effort; kinds align first).
    pub message: String,
    /// Named sub-diagnostic that Zig encodes only as a message prefix
    /// (e.g. `"LoopConditionalConsume"`, `"MacroConditionalConsume"`),
    /// or `"OutOfSlice"` for node forms this slice does not implement.
    pub marker: Option<&'static str>,
}

impl Diagnostic {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            kind,
            message: message.into(),
            marker: None,
        }
    }

    pub fn marked(
        kind: ErrorKind,
        marker: &'static str,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            kind,
            message: message.into(),
            marker: Some(marker),
        }
    }

    /// Node form outside this checker's slice (see the crate's
    /// not-covered list). Zig checks these forms; the slice refuses
    /// loudly instead of approximating them.
    pub fn out_of_slice(what: &str) -> Diagnostic {
        Diagnostic::marked(
            ErrorKind::CompileError,
            "OutOfSlice",
            format!("sla-checker slice: unsupported node form: {what}"),
        )
    }
}

pub type CheckResult<T> = Result<T, Diagnostic>;

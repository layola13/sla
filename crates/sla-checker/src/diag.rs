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
    // R-3 additions: the arena domain diagnostics (F2-A C1 design;
    // the feature postdates the mirrored Zig tree, so these names
    // come from the design + corpus `// expect:` lines).
    ArenaUnsafe,
    ArenaEscape,
    ArenaDomain,
    ArenaGoroutine,
    // R-3 (J-4): expression-position use of a macro whose leading
    // output parameter declares no type.
    MacroExpressionOutputUntyped,
    // R-3 (A_min): the `T!` sugar in parameter position.
    FallibleSugarParam,
    // R-3 (borrow origins, E_res corpus): the five origin gates.
    BorrowAnnotationInvalid,
    BorrowEscape,
    BorrowOriginNotABinding,
    BorrowOriginMerge,
    BorrowUseAfterInvalidation,
    // R-3 (goroutine batch, go_diag_* corpus): the spawn/select
    // shape gates. Judged by standalone `sla check` rejection
    // (sala doc 13); kind names are form-named in this port.
    GoAsyncCallee,
    GoNonVoidCallee,
    GoArity,
    GoClosureCallee,
    GoMethodCallee,
    GoBorrowPayload,
    GoAsyncSelect,
    DeferNonCall,
    // R-3 closing: a syntax sugar whose callee shape the
    // desugarer cannot clone (corpus neg_desugar_failed).
    DesugarFailed,
    // R-3 (J-5 check hooks): unknown hook name, and the
    // macro_param_audit finding for an unreferenced parameter.
    UnknownCheckHook,
    MacroParamUnused,
    // R-3 (J-1 shader sublanguage): the sublanguage gates.
    SublanguageNotBound,
    ShaderDuplicateName,
    ShaderDimensionMismatch,
    ShaderStageViolation,
    Overflow,
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
            ErrorKind::ArenaUnsafe => "ArenaUnsafe",
            ErrorKind::ArenaEscape => "ArenaEscape",
            ErrorKind::ArenaDomain => "ArenaDomain",
            ErrorKind::ArenaGoroutine => "ArenaGoroutine",
            ErrorKind::MacroExpressionOutputUntyped => {
                "MacroExpressionOutputUntyped"
            }
            ErrorKind::FallibleSugarParam => "FallibleSugarParam",
            ErrorKind::BorrowAnnotationInvalid => "BorrowAnnotationInvalid",
            ErrorKind::BorrowEscape => "BorrowEscape",
            ErrorKind::BorrowOriginNotABinding => "BorrowOriginNotABinding",
            ErrorKind::BorrowOriginMerge => "BorrowOriginMerge",
            ErrorKind::BorrowUseAfterInvalidation => {
                "BorrowUseAfterInvalidation"
            }
            ErrorKind::GoAsyncCallee => "GoAsyncCallee",
            ErrorKind::GoNonVoidCallee => "GoNonVoidCallee",
            ErrorKind::GoArity => "GoArity",
            ErrorKind::GoClosureCallee => "GoClosureCallee",
            ErrorKind::GoMethodCallee => "GoMethodCallee",
            ErrorKind::GoBorrowPayload => "GoBorrowPayload",
            ErrorKind::GoAsyncSelect => "GoAsyncSelect",
            ErrorKind::DeferNonCall => "DeferNonCall",
            ErrorKind::DesugarFailed => "DesugarFailed",
            ErrorKind::UnknownCheckHook => "UnknownCheckHook",
            ErrorKind::MacroParamUnused => "MacroParamUnused",
            ErrorKind::SublanguageNotBound => "SublanguageNotBound",
            ErrorKind::ShaderDuplicateName => "ShaderDuplicateName",
            ErrorKind::ShaderDimensionMismatch => "ShaderDimensionMismatch",
            ErrorKind::ShaderStageViolation => "ShaderStageViolation",
            ErrorKind::Overflow => "Overflow",
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

//! Error type for the SAB codegen crate.
//!
//! Mirrors the Zig error set of `sab_codegen.zig` (lines 58-65):
//! `UnsupportedSabDirectFeature`, `ConflictingConstDecl`, `MissingType`,
//! `OutOfMemory`, `InvalidStringLiteral` — plus the encoder errors of
//! sci's `sab.zig` that can surface while writing a module.
//!
//! Divergence note (see docs/divergences/sab.md, entry SAB-D11): in the Zig
//! original the `UnsupportedSabDirectFeature` error carries no payload; the
//! human-readable reason is only printed to stderr when the environment
//! variable `SLA_SAB_TRACE_UNSUPPORTED` is set. This Rust port keeps the
//! error *loud* (never a silent fallback) but attaches the reason string
//! unconditionally, which is a deliberate, registered difference in
//! diagnostics, not in emitted bytes.

use core::fmt;

/// Errors that abort SAB generation. Generation never degrades to the text
/// backend: an unsupported construct is a hard error here, exactly as the
/// Zig `Codegen` raises `error.UnsupportedSabDirectFeature`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The input uses a feature the SAB direct path does not cover yet.
    /// The string names the construct (the Zig code passes the same string
    /// to its stderr-only trace helper).
    UnsupportedSabDirectFeature(String),
    /// A const declaration name was re-declared with an incompatible value
    /// (Zig: `ensureConstDeclCompatible` -> `error.ConflictingConstDecl`).
    ConflictingConstDecl(String),
    /// A type fact required for emission was absent
    /// (Zig: `error.MissingType`).
    MissingType(String),
    /// A string literal in the source could not be decoded
    /// (Zig: `error.InvalidStringLiteral`).
    InvalidStringLiteral(String),
    /// The encoder was handed a structurally invalid module (e.g. a pool
    /// index or enum ordinal out of range). The Zig encoder would fail
    /// with `error.InvalidTag` / an assertion; this port reports it.
    InvalidModule(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnsupportedSabDirectFeature(what) => {
                write!(f, "UnsupportedSabDirectFeature: {what}")
            }
            Error::ConflictingConstDecl(name) => {
                write!(f, "ConflictingConstDecl: {name}")
            }
            Error::MissingType(what) => write!(f, "MissingType: {what}"),
            Error::InvalidStringLiteral(what) => {
                write!(f, "InvalidStringLiteral: {what}")
            }
            Error::InvalidModule(what) => write!(f, "InvalidModule: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// Convenience alias used across the crate.
pub type Result<T> = core::result::Result<T, Error>;

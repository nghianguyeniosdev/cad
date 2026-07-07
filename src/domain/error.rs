//! Domain error types.
//!
//! `FailureKind` is the classification that drives control flow. Richer,
//! layer-specific error enums (and `thiserror`) are introduced by the slices
//! that first need to propagate errors; `anyhow` is used only at the binary
//! edge.

/// Classification of a failure encountered while working an Asset.
///
/// The distinction drives control flow: an `AuthExpired` failure triggers a
/// Session Re-login (and does not consume an Asset's retry budget), a
/// `Transient` failure is eligible for retry, and a `Fatal` failure ends work
/// on that Asset. Adapters map their raw errors into this kind so that `app`
/// components branch on the variant, never on error strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// The SSO session is missing or expired.
    AuthExpired,
    /// A recoverable error (network blip, throttling, corrupt transfer).
    Transient,
    /// A non-recoverable error (not found, bad request, exhausted retries).
    Fatal,
}

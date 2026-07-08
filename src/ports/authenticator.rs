use async_trait::async_trait;

use crate::domain::Failure;

/// Whether the AWS SSO session is usable, or needs a fresh login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// Credentials resolve and are valid.
    Valid,
    /// The session is missing or expired and requires `login`.
    NeedsLogin,
}

/// The SSO seam: check whether the session is usable, and (re)establish it via
/// the browser login flow. Backed by the `aws` CLI (ADR 0001).
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Report whether the session for `profile` is currently valid.
    async fn session_status(&self, profile: Option<&str>) -> SessionStatus;

    /// Establish a session for `profile` (interactive browser login), blocking
    /// until it completes. A non-zero login is a `Failure`.
    async fn login(&self, profile: Option<&str>) -> Result<(), Failure>;
}

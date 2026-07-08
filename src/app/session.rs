use crate::domain::Failure;
use crate::ports::{Authenticator, SessionStatus};

/// Preflight auto-login: ensure the SSO session is usable before downloading.
/// If the session needs login, trigger it and block until it completes; a login
/// failure aborts (propagates the `Failure`).
pub async fn ensure_session(
    authenticator: &dyn Authenticator,
    profile: Option<&str>,
) -> Result<(), Failure> {
    match authenticator.session_status(profile).await {
        SessionStatus::Valid => Ok(()),
        SessionStatus::NeedsLogin => authenticator.login(profile).await,
    }
}

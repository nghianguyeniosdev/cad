use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::domain::Failure;
use crate::ports::{Authenticator, SessionStatus};

/// Number of consecutive re-logins (with no download progress between them)
/// allowed before the run aborts.
pub const MAX_CONSECUTIVE_REAUTH: usize = 2;

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

/// The single home for mid-run credential recovery: when a worker hits an
/// `AuthExpired` failure it calls `reauth`, which performs a **single-flight,
/// pause-the-world** `aws sso login` (one login even when many workers expire
/// at once). Guards against a re-login loop that never makes progress. See
/// ADR 0003.
pub struct SessionCoordinator {
    auth: Arc<dyn Authenticator>,
    profile: Option<String>,
    /// Advances by one on each successful re-login; lets late workers detect
    /// that someone already re-logged in and just retry.
    generation: AtomicU64,
    /// Serializes re-login so only one browser login happens at a time.
    relogin_lock: Mutex<()>,
    /// Consecutive re-logins since the last successful download.
    consecutive_reauth: AtomicUsize,
}

impl SessionCoordinator {
    pub fn new(auth: Arc<dyn Authenticator>, profile: Option<String>) -> Self {
        Self {
            auth,
            profile,
            generation: AtomicU64::new(0),
            relogin_lock: Mutex::new(()),
            consecutive_reauth: AtomicUsize::new(0),
        }
    }

    /// The current re-login generation. A worker records this before an attempt
    /// so `reauth` can tell whether a peer already recovered the session.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Reset the no-progress guard after a successful download.
    pub fn note_progress(&self) {
        self.consecutive_reauth.store(0, Ordering::SeqCst);
    }

    /// Recover from an `AuthExpired` failure. `seen` is the generation the
    /// caller observed before its failed attempt. Returns the current
    /// generation (so the caller retries), or a `Failure` to abort the Asset.
    pub async fn reauth(&self, seen: u64) -> Result<u64, Failure> {
        let _guard = self.relogin_lock.lock().await;

        let current = self.generation.load(Ordering::Acquire);
        if current != seen {
            // A peer already re-logged in for this generation; just retry.
            return Ok(current);
        }

        if self.consecutive_reauth.load(Ordering::SeqCst) >= MAX_CONSECUTIVE_REAUTH {
            return Err(Failure::auth_expired(
                "SSO re-login is not yielding a working session; aborting",
            ));
        }

        self.auth.login(self.profile.as_deref()).await?;
        self.consecutive_reauth.fetch_add(1, Ordering::SeqCst);
        let next = current + 1;
        self.generation.store(next, Ordering::Release);
        Ok(next)
    }
}

/// Default authenticator when none is configured: reports a valid session and
/// refuses to log in (a stray mid-run expiry then fails cleanly rather than
/// looping).
pub(crate) struct NoLoginAuthenticator;

#[async_trait]
impl Authenticator for NoLoginAuthenticator {
    async fn session_status(&self, _profile: Option<&str>) -> SessionStatus {
        SessionStatus::Valid
    }

    async fn login(&self, _profile: Option<&str>) -> Result<(), Failure> {
        Err(Failure::fatal("no authenticator configured for re-login"))
    }
}

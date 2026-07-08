use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

use crate::domain::Failure;
use crate::ports::{Authenticator, SessionStatus};

/// An `Authenticator` backed by the `aws` CLI (ADR 0001): session status via
/// `aws sts get-caller-identity`, login via `aws sso login` (interactive).
pub struct SsoAuthenticator;

fn with_profile(cmd: &mut Command, profile: Option<&str>) {
    if let Some(profile) = profile {
        cmd.args(["--profile", profile]);
    }
}

#[async_trait]
impl Authenticator for SsoAuthenticator {
    async fn session_status(&self, profile: Option<&str>) -> SessionStatus {
        let mut cmd = Command::new("aws");
        cmd.args(["sts", "get-caller-identity"]);
        with_profile(&mut cmd, profile);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        match cmd.status().await {
            Ok(status) if status.success() => SessionStatus::Valid,
            _ => SessionStatus::NeedsLogin,
        }
    }

    async fn login(&self, profile: Option<&str>) -> Result<(), Failure> {
        let mut cmd = Command::new("aws");
        cmd.args(["sso", "login"]);
        with_profile(&mut cmd, profile);
        // Inherit stdio so the login URL / prompts reach the user.

        let status = cmd
            .status()
            .await
            .map_err(|e| Failure::fatal(format!("failed to run `aws sso login`: {e}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(Failure::fatal(format!(
                "`aws sso login` exited with {status}"
            )))
        }
    }
}

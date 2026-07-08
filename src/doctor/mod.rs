//! Cohesive Doctor module: a `Check` trait, concrete checks, and a Composite
//! runner. Invoked standalone (`acd doctor`) and as a `download` preflight.
//! See ADR 0004.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

/// The result of a single environment check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    Pass,
    /// Failed, with an actionable hint for the user.
    Fail {
        hint: String,
    },
}

impl CheckOutcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, CheckOutcome::Pass)
    }

    pub fn fail(hint: impl Into<String>) -> Self {
        CheckOutcome::Fail { hint: hint.into() }
    }
}

/// One environment check (aws CLI present, profile exists, dest writable, ...).
#[async_trait]
pub trait Check: Send + Sync {
    /// Short human-readable name (e.g. "aws CLI installed").
    fn name(&self) -> &str;
    /// Run the check.
    async fn run(&self) -> CheckOutcome;
}

/// One check's name paired with its outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub name: String,
    pub outcome: CheckOutcome,
}

/// The aggregate result of running all checks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DoctorReport {
    pub results: Vec<CheckReport>,
}

impl DoctorReport {
    /// True only if every check passed.
    pub fn ok(&self) -> bool {
        self.results.iter().all(|r| r.outcome.is_pass())
    }
}

/// Decide whether an `aws --version` output reports a v2 CLI (SSO-capable).
/// Pure so it can be unit-tested without invoking the CLI.
pub fn check_cli_version(version_output: &str) -> CheckOutcome {
    if version_output.trim_start().starts_with("aws-cli/2") {
        CheckOutcome::Pass
    } else {
        CheckOutcome::fail(format!(
            "acd needs aws CLI v2 (for SSO); found: {}. Install/upgrade: https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html",
            version_output.trim()
        ))
    }
}

/// Checks that a destination directory can be created and written to.
pub struct DestWritable {
    path: PathBuf,
}

impl DestWritable {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl Check for DestWritable {
    fn name(&self) -> &str {
        "destination writable"
    }

    async fn run(&self) -> CheckOutcome {
        if let Err(e) = tokio::fs::create_dir_all(&self.path).await {
            return CheckOutcome::fail(format!(
                "cannot create destination {}: {e}",
                self.path.display()
            ));
        }
        let probe = self.path.join(".acd-write-probe");
        match tokio::fs::write(&probe, b"").await {
            Ok(()) => {
                let _ = tokio::fs::remove_file(&probe).await;
                CheckOutcome::Pass
            }
            Err(e) => CheckOutcome::fail(format!(
                "destination {} is not writable: {e}",
                self.path.display()
            )),
        }
    }
}

/// Checks that the `aws` CLI is on the PATH.
pub struct CliInstalled;

#[async_trait]
impl Check for CliInstalled {
    fn name(&self) -> &str {
        "aws CLI installed"
    }

    async fn run(&self) -> CheckOutcome {
        match Command::new("aws")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
        {
            Ok(status) if status.success() => CheckOutcome::Pass,
            _ => CheckOutcome::fail(
                "aws CLI not found on PATH; install AWS CLI v2: \
                 https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html",
            ),
        }
    }
}

/// Checks that the installed `aws` CLI is v2 (SSO-capable).
pub struct CliVersion;

#[async_trait]
impl Check for CliVersion {
    fn name(&self) -> &str {
        "aws CLI is v2"
    }

    async fn run(&self) -> CheckOutcome {
        match Command::new("aws").arg("--version").output().await {
            Ok(output) => {
                // `aws --version` prints to stdout (v2) or stderr (older).
                let text = if output.stdout.is_empty() {
                    String::from_utf8_lossy(&output.stderr)
                } else {
                    String::from_utf8_lossy(&output.stdout)
                };
                check_cli_version(text.trim())
            }
            Err(_) => CheckOutcome::fail("aws CLI not found on PATH; install AWS CLI v2"),
        }
    }
}

/// Checks that the given profile exists (skipped when no profile is set).
pub struct ProfileExists {
    profile: Option<String>,
}

impl ProfileExists {
    pub fn new(profile: Option<String>) -> Self {
        Self { profile }
    }
}

#[async_trait]
impl Check for ProfileExists {
    fn name(&self) -> &str {
        "aws profile exists"
    }

    async fn run(&self) -> CheckOutcome {
        let Some(profile) = &self.profile else {
            return CheckOutcome::Pass;
        };
        match Command::new("aws")
            .args(["configure", "list-profiles"])
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                let profiles = String::from_utf8_lossy(&output.stdout);
                if profiles.lines().any(|line| line.trim() == profile) {
                    CheckOutcome::Pass
                } else {
                    CheckOutcome::fail(format!(
                        "profile '{profile}' not found in ~/.aws/config; \
                         configure it with `aws configure sso`"
                    ))
                }
            }
            _ => CheckOutcome::fail("could not list aws profiles (is the aws CLI installed?)"),
        }
    }
}

/// Assemble the standard environment checks: aws CLI present + v2, profile
/// exists, and each destination writable.
pub fn environment_checks(profile: Option<String>, dests: Vec<PathBuf>) -> Vec<Box<dyn Check>> {
    let mut checks: Vec<Box<dyn Check>> = vec![
        Box::new(CliInstalled),
        Box::new(CliVersion),
        Box::new(ProfileExists::new(profile)),
    ];
    for dest in dests {
        checks.push(Box::new(DestWritable::new(dest)));
    }
    checks
}

/// Run every check (Composite) and collect the results, in order.
pub async fn run_all(checks: &[Box<dyn Check>]) -> DoctorReport {
    let mut results = Vec::with_capacity(checks.len());
    for check in checks {
        results.push(CheckReport {
            name: check.name().to_string(),
            outcome: check.run().await,
        });
    }
    DoctorReport { results }
}

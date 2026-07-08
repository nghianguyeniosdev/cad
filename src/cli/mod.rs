//! CLI surface: command parsing (clap) and dispatch to `app` via the
//! composition root. See ADR 0004.

use std::io::Write;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::domain::RawManifest;

/// Exit code for a usage error (unrecognized command / bad invocation).
const EXIT_USAGE: i32 = 2;
/// Exit code for an environment/precondition failure (Doctor check failed,
/// SSO login failed, ...).
const EXIT_ENV: i32 = 2;

#[derive(Parser)]
#[command(
    name = "acd",
    about = "Download and verify AWS CodeArtifact generic-package assets"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the version.
    Version,
    /// Download the Assets described by a Manifest.
    Download {
        /// Path to the Manifest.
        #[arg(long, default_value = "codeartifact.yaml")]
        manifest: PathBuf,
        /// AWS profile to use (falls back to AWS_PROFILE / default).
        #[arg(long)]
        profile: Option<String>,
        /// Maximum number of Assets to download concurrently.
        #[arg(long, default_value_t = 10)]
        concurrency: usize,
        /// Ignore any cached asset lists and re-query CodeArtifact.
        #[arg(long)]
        refresh_cache: bool,
        /// Cache Root for versioned layout (overrides ~/.acd/config.yml).
        #[arg(long)]
        cache_root: Option<String>,
    },
    /// Check the local environment.
    Doctor {
        /// AWS profile to check for.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Scaffold a starter Manifest (`codeartifact.yaml`).
    Init {
        /// Overwrite an existing `codeartifact.yaml`.
        #[arg(long)]
        force: bool,
    },
}

/// Library entry point: parse `args` (including argv[0]), write user-facing
/// output to `out`, and return the process exit code. The primary test seam.
pub fn run(args: impl IntoIterator<Item = String>, out: &mut dyn Write) -> i32 {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            let _ = write!(out, "{err}");
            // clap uses stderr for real errors and stdout for --help/--version.
            return if err.use_stderr() { EXIT_USAGE } else { 0 };
        }
    };

    match cli.command {
        Command::Version => {
            let _ = writeln!(out, "acd {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Command::Download {
            manifest,
            profile,
            concurrency,
            refresh_cache,
            cache_root,
        } => run_download(
            manifest,
            profile,
            concurrency,
            refresh_cache,
            cache_root,
            out,
        ),
        Command::Doctor { profile } => run_doctor(profile, out),
        Command::Init { force } => run_init(force, out),
    }
}

fn run_doctor(profile: Option<String>, out: &mut dyn Write) -> i32 {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            let _ = writeln!(out, "error: failed to start async runtime: {err}");
            return 1;
        }
    };
    runtime.block_on(async {
        // Standalone doctor checks the current directory for writability.
        let checks = crate::doctor::environment_checks(profile, vec![PathBuf::from(".")]);
        let report = crate::doctor::run_all(&checks).await;
        print_report(&report, out);
        if report.ok() {
            0
        } else {
            EXIT_ENV
        }
    })
}

/// Print a Doctor report: `✓`/`✗` per check, with the hint on failures.
fn print_report(report: &crate::doctor::DoctorReport, out: &mut dyn Write) {
    for result in &report.results {
        match &result.outcome {
            crate::doctor::CheckOutcome::Pass => {
                let _ = writeln!(out, "  ✓ {}", result.name);
            }
            crate::doctor::CheckOutcome::Fail { hint } => {
                let _ = writeln!(out, "  ✗ {} — {hint}", result.name);
            }
        }
    }
}

fn run_init(force: bool, out: &mut dyn Write) -> i32 {
    let path = PathBuf::from("codeartifact.yaml");
    match crate::app::init::init_manifest(&path, force) {
        Ok(()) => {
            let _ = writeln!(out, "Created {}", path.display());
            0
        }
        Err(failure) => {
            let _ = writeln!(out, "error: {}", failure.message);
            EXIT_USAGE
        }
    }
}

fn run_download(
    manifest_path: PathBuf,
    profile: Option<String>,
    concurrency: usize,
    refresh_cache: bool,
    cache_root: Option<String>,
    out: &mut dyn Write,
) -> i32 {
    let yaml = match std::fs::read_to_string(&manifest_path) {
        Ok(yaml) => yaml,
        Err(err) => {
            let _ = writeln!(
                out,
                "error: cannot read manifest {}: {err}",
                manifest_path.display()
            );
            return EXIT_USAGE;
        }
    };

    // Resolve the Cache Root (flag > ~/.acd/config.yml > default) for versioned
    // layout, then parse + resolve the Manifest's dests.
    let config_path = crate::config::default_config_path();
    let cache_root = match crate::config::resolve_cache_root(cache_root, &config_path) {
        Ok(root) => root,
        Err(err) => {
            let _ = writeln!(out, "error: {err}");
            return EXIT_ENV;
        }
    };
    let manifest = match RawManifest::from_yaml(&yaml).and_then(|raw| raw.resolve(&cache_root)) {
        Ok(manifest) => manifest,
        Err(err) => {
            let _ = writeln!(out, "error: {err}");
            return EXIT_USAGE;
        }
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            let _ = writeln!(out, "error: failed to start async runtime: {err}");
            return 1;
        }
    };

    runtime.block_on(async {
        // Preflight 1: Doctor — aws CLI present + v2, profile exists, dests writable.
        let dests: Vec<PathBuf> = manifest.packages.iter().map(|e| e.dest.clone()).collect();
        let checks = crate::doctor::environment_checks(profile.clone(), dests);
        let report = crate::doctor::run_all(&checks).await;
        if !report.ok() {
            print_report(&report, out);
            return EXIT_ENV;
        }

        // Preflight 2: ensure a usable SSO session, auto-logging in if needed.
        // The same authenticator powers mid-run re-login recovery.
        let authenticator: std::sync::Arc<dyn crate::ports::Authenticator> =
            std::sync::Arc::new(crate::adapters::sso::SsoAuthenticator);
        if let Err(failure) =
            crate::app::ensure_session(authenticator.as_ref(), profile.as_deref()).await
        {
            let _ = writeln!(out, "error: {}", failure.message);
            return EXIT_ENV;
        }

        let service = match crate::wiring::build_download_service(
            manifest.connection.clone(),
            profile,
            concurrency,
            authenticator,
            refresh_cache,
        )
        .await
        {
            Ok(service) => service,
            Err(msg) => {
                let _ = writeln!(out, "error: {msg}");
                return 1;
            }
        };

        // Enumerate Phase: print an apt-style summary before downloading.
        let plan = match service.enumerate(&manifest).await {
            Ok(plan) => plan,
            Err(failure) => {
                let _ = writeln!(
                    out,
                    "error: failed to enumerate assets: {}",
                    failure.message
                );
                return 1;
            }
        };
        let _ = writeln!(
            out,
            "Need to get {} in {} files across {} packages.",
            human_bytes(plan.total_bytes()),
            plan.total_files(),
            plan.package_count()
        );

        // Download Phase.
        let summary = service.download(&plan).await;
        let _ = writeln!(
            out,
            "Fetched {} in {} files ({} cached); {} failed.",
            human_bytes(summary.bytes),
            summary.downloaded,
            summary.cached,
            summary.failed
        );
        for failed in &summary.failed_assets {
            let _ = writeln!(out, "  failed: {}: {}", failed.name, failed.reason);
        }
        summary.exit_code()
    })
}

/// Render a byte count in human-readable units (apt-style).
fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

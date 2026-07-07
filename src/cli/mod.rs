//! CLI surface: command parsing (clap) and dispatch to `app` via the
//! composition root. See ADR 0004.

use std::io::Write;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::domain::Manifest;

/// Exit code for a usage error (unrecognized command / bad invocation).
const EXIT_USAGE: i32 = 2;
/// Exit code for a recognized command that has no implementation yet.
const EXIT_NOT_IMPLEMENTED: i32 = 3;

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
    },
    /// Check the local environment.
    Doctor,
    /// Scaffold a starter Manifest.
    Init,
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
        } => run_download(manifest, profile, concurrency, out),
        Command::Doctor => not_implemented("doctor", out),
        Command::Init => not_implemented("init", out),
    }
}

fn not_implemented(command: &str, out: &mut dyn Write) -> i32 {
    let _ = writeln!(out, "acd {command}: not implemented yet");
    EXIT_NOT_IMPLEMENTED
}

fn run_download(
    manifest_path: PathBuf,
    profile: Option<String>,
    concurrency: usize,
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
    let manifest = match Manifest::from_yaml(&yaml) {
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
        let service = match crate::wiring::build_download_service(
            manifest.connection.clone(),
            profile,
            concurrency,
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
            Err(_) => {
                let _ = writeln!(out, "error: failed to enumerate assets");
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
            "Fetched {} in {} files; {} failed.",
            human_bytes(summary.bytes),
            summary.downloaded,
            summary.failed
        );
        for name in &summary.failed_assets {
            let _ = writeln!(out, "  failed: {name}");
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

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
        Command::Download { manifest, profile } => run_download(manifest, profile, out),
        Command::Doctor => not_implemented("doctor", out),
        Command::Init => not_implemented("init", out),
    }
}

fn not_implemented(command: &str, out: &mut dyn Write) -> i32 {
    let _ = writeln!(out, "acd {command}: not implemented yet");
    EXIT_NOT_IMPLEMENTED
}

fn run_download(manifest_path: PathBuf, profile: Option<String>, out: &mut dyn Write) -> i32 {
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
        let summary = match crate::wiring::run_download(&manifest, profile).await {
            Ok(summary) => summary,
            Err(msg) => {
                let _ = writeln!(out, "error: {msg}");
                return 1;
            }
        };

        let _ = writeln!(
            out,
            "Fetched {} files ({} bytes); {} failed.",
            summary.downloaded, summary.bytes, summary.failed
        );
        for name in &summary.failed_assets {
            let _ = writeln!(out, "  failed: {name}");
        }
        summary.exit_code()
    })
}

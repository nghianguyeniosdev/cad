use std::io::Write;

pub mod adapters;
pub mod app;
pub mod cli;
pub mod doctor;
pub mod domain;
pub mod ports;
pub mod wiring;

/// Exit code for a usage error (unrecognized command / bad invocation).
const EXIT_USAGE: i32 = 2;
/// Exit code for a recognized command that has no implementation yet.
const EXIT_NOT_IMPLEMENTED: i32 = 3;

/// Library entry point: parse `args` (including argv[0]), write user-facing
/// output to `out`, and return the process exit code.
///
/// This is the primary test seam — `main.rs` is a thin shim over it.
pub fn run(args: impl IntoIterator<Item = String>, out: &mut dyn Write) -> i32 {
    let args: Vec<String> = args.into_iter().collect();
    match args.get(1).map(String::as_str) {
        Some("version") => {
            let _ = writeln!(out, "acd {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some(cmd @ ("download" | "doctor" | "init")) => {
            let _ = writeln!(out, "acd {cmd}: not implemented yet");
            EXIT_NOT_IMPLEMENTED
        }
        other => {
            let cmd = other.unwrap_or("<none>");
            let _ = writeln!(out, "error: unrecognized command `{cmd}`");
            EXIT_USAGE
        }
    }
}

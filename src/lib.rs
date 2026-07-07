use std::io::Write;

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
        _ => 0,
    }
}

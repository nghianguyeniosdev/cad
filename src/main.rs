use std::io::Write;

fn main() {
    let mut stdout = std::io::stdout().lock();
    let code = acd::run(std::env::args(), &mut stdout);
    let _ = stdout.flush();
    std::process::exit(code);
}

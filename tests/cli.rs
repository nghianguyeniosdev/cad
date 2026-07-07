use acd::run;

/// Drive the library seam with argv-style args and capture (exit code, stdout).
fn run_capture(args: &[&str]) -> (i32, String) {
    let mut out: Vec<u8> = Vec::new();
    let code = run(args.iter().map(|s| s.to_string()), &mut out);
    (code, String::from_utf8(out).expect("output is valid UTF-8"))
}

#[test]
fn version_command_prints_version_and_exits_zero() {
    let (code, output) = run_capture(&["acd", "version"]);

    assert_eq!(code, 0, "`acd version` should exit 0");
    // Independent literal from the spec (Cargo.toml package version),
    // not recomputed from the code under test.
    assert!(
        output.contains("0.1.0"),
        "`acd version` output should contain the crate version, got: {output:?}"
    );
}

#[test]
fn unknown_command_exits_nonzero() {
    let (code, _output) = run_capture(&["acd", "bogus"]);

    assert_ne!(code, 0, "an unrecognized command should exit non-zero");
}

#[test]
fn stub_subcommands_are_recognized_but_not_yet_implemented() {
    for cmd in ["download", "doctor", "init"] {
        let (code, output) = run_capture(&["acd", cmd]);

        assert_eq!(
            code, 3,
            "`acd {cmd}` should be a recognized command reporting not-implemented (exit 3)"
        );
        assert!(
            output.to_lowercase().contains("not implemented"),
            "`acd {cmd}` should say it is not implemented, got: {output:?}"
        );
    }
}

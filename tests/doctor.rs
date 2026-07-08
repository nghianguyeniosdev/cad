use async_trait::async_trait;

use acd::doctor::{run_all, Check, CheckOutcome, DestWritable, DoctorReport};

struct FakeCheck {
    name: String,
    outcome: CheckOutcome,
}

#[async_trait]
impl Check for FakeCheck {
    fn name(&self) -> &str {
        &self.name
    }
    async fn run(&self) -> CheckOutcome {
        self.outcome.clone()
    }
}

fn fake(name: &str, outcome: CheckOutcome) -> Box<dyn Check> {
    Box::new(FakeCheck {
        name: name.into(),
        outcome,
    })
}

#[tokio::test]
async fn report_fails_and_names_the_failing_check() {
    let checks = vec![
        fake("cli installed", CheckOutcome::Pass),
        fake(
            "profile exists",
            CheckOutcome::fail("run `aws configure sso`"),
        ),
    ];

    let report: DoctorReport = run_all(&checks).await;

    assert!(
        !report.ok(),
        "a failing check should make the report not-ok"
    );
    let failing = report
        .results
        .iter()
        .find(|r| r.name == "profile exists")
        .expect("the failing check should appear in the report");
    assert_eq!(
        failing.outcome,
        CheckOutcome::fail("run `aws configure sso`"),
        "the report should carry the failing check's hint"
    );
}

#[test]
fn cli_version_accepts_v2_and_rejects_v1_or_garbage() {
    use acd::doctor::check_cli_version;

    // Real `aws --version` output shapes (independent examples).
    assert!(
        check_cli_version("aws-cli/2.15.0 Python/3.11.6 Darwin/23.5.0").is_pass(),
        "v2 should pass"
    );

    match check_cli_version("aws-cli/1.27.100 Python/3.9.6") {
        CheckOutcome::Fail { hint } => {
            assert!(hint.contains("v2"), "hint should mention v2: {hint}")
        }
        CheckOutcome::Pass => panic!("v1 must not pass"),
    }

    assert!(
        matches!(
            check_cli_version("not aws at all"),
            CheckOutcome::Fail { .. }
        ),
        "unrecognized output must fail"
    );
}

#[tokio::test]
async fn dest_writable_passes_for_a_writable_dir_and_fails_under_a_file() {
    let dir = tempfile::tempdir().unwrap();

    // A fresh path under a writable temp dir can be created + written.
    let good = dir.path().join("artifacts/out");
    assert!(
        DestWritable::new(&good).run().await.is_pass(),
        "a writable destination should pass"
    );

    // A path whose parent is a regular file cannot be created.
    let file = dir.path().join("not-a-dir");
    std::fs::write(&file, b"x").unwrap();
    let bad = file.join("sub");
    assert!(
        !DestWritable::new(&bad).run().await.is_pass(),
        "an uncreatable destination should fail"
    );
}

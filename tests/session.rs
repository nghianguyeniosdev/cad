mod fakes;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acd::app::{ensure_session, DownloadService, RetryPolicy};
use acd::domain::{Asset, ConnectionSettings, Entry, Manifest};
use acd::ports::SessionStatus;

use fakes::FakeAuthenticator;

const HELLO_MD5: &str = "5d41402abc4b2a76b9719d911017c592";

fn one_entry_manifest() -> Manifest {
    Manifest {
        connection: ConnectionSettings {
            domain: "d".into(),
            domain_owner: "111122223333".into(),
            repository: "r".into(),
            region: None,
        },
        packages: vec![Entry {
            namespace: None,
            package: "pkg".into(),
            version: "1.0.0".into(),
            dest: PathBuf::from("out"),
        }],
    }
}

fn assets(count: usize) -> Vec<Asset> {
    (0..count)
        .map(|i| Asset {
            name: format!("f{i}.bin"),
            size: 5,
            expected_md5: HELLO_MD5.into(),
        })
        .collect()
}

fn zero_backoff() -> RetryPolicy {
    RetryPolicy {
        max_retries: 3,
        backoff: Duration::ZERO,
    }
}

#[tokio::test]
async fn valid_session_does_not_trigger_login() {
    let auth = FakeAuthenticator::new(SessionStatus::Valid, false);

    let result = ensure_session(&auth, Some("prof")).await;

    assert!(result.is_ok());
    assert_eq!(
        auth.login_calls.load(Ordering::SeqCst),
        0,
        "a valid session must not trigger a login"
    );
}

#[tokio::test]
async fn needs_login_triggers_login_then_proceeds() {
    let auth = FakeAuthenticator::new(SessionStatus::NeedsLogin, false);

    let result = ensure_session(&auth, Some("prof")).await;

    assert!(
        result.is_ok(),
        "after a successful login the preflight succeeds"
    );
    assert_eq!(
        auth.login_calls.load(Ordering::SeqCst),
        1,
        "an expired/missing session must trigger exactly one login"
    );
}

#[tokio::test]
async fn login_failure_aborts() {
    let auth = FakeAuthenticator::new(SessionStatus::NeedsLogin, true);

    let result = ensure_session(&auth, Some("prof")).await;

    let failure = result.expect_err("a failed login must abort the preflight");
    assert!(
        failure.message.contains("aws sso login failed"),
        "the abort should carry the login failure reason, got: {:?}",
        failure.message
    );
    assert_eq!(auth.login_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn concurrent_auth_expiry_triggers_a_single_relogin() {
    // 8 Assets all fail with AuthExpired until one login happens; then succeed.
    let logins = Arc::new(AtomicUsize::new(0));
    let source = Arc::new(fakes::AuthExpiringSource {
        assets: assets(8),
        bytes: b"hello".to_vec(),
        logins: logins.clone(),
        succeed_after_logins: 1,
        fetch_calls: Arc::new(AtomicUsize::new(0)),
    });
    let auth = Arc::new(fakes::SharedLoginAuthenticator {
        logins: logins.clone(),
        login_fails: false,
    });
    let files = Arc::new(fakes::FakeFileStore::default());
    let service = DownloadService::new(source, files)
        .with_authenticator(auth, Some("prof".into()))
        .with_retry_policy(zero_backoff());

    let summary = service.run(&one_entry_manifest()).await;

    assert_eq!(summary.downloaded, 8, "all Assets download after re-login");
    assert_eq!(summary.failed, 0);
    assert_eq!(
        logins.load(Ordering::SeqCst),
        1,
        "single-flight: exactly one login for many concurrent expiries"
    );
}

fn shared_auth() -> Arc<fakes::SharedLoginAuthenticator> {
    Arc::new(fakes::SharedLoginAuthenticator {
        logins: Arc::new(AtomicUsize::new(0)),
        login_fails: false,
    })
}

#[tokio::test]
async fn auth_expiry_does_not_consume_the_transient_retry_budget() {
    // One AuthExpired, then a FULL budget of 3 transient failures, then success.
    // With max_retries=3 this only passes if the AuthExpired did NOT eat into
    // the transient budget.
    use fakes::FetchStep::{AuthExpired, Ok as OkStep, Transient};
    let steps = VecDeque::from(vec![
        AuthExpired,
        Transient,
        Transient,
        Transient,
        OkStep(b"hello".to_vec()),
    ]);
    let source = Arc::new(fakes::ScriptedSource {
        assets: assets(1),
        steps: Mutex::new(steps),
    });
    let files = Arc::new(fakes::FakeFileStore::default());
    let service = DownloadService::new(source, files)
        .with_authenticator(shared_auth(), Some("prof".into()))
        .with_retry_policy(zero_backoff());

    let summary = service.run(&one_entry_manifest()).await;

    assert_eq!(summary.downloaded, 1, "budget survived the auth expiry");
    assert_eq!(summary.failed, 0);
}

#[tokio::test]
async fn relogin_that_never_helps_aborts_after_two_attempts() {
    // Session never becomes usable -> two re-logins, then abort on the 3rd.
    let logins = Arc::new(AtomicUsize::new(0));
    let source = Arc::new(fakes::AuthExpiringSource {
        assets: assets(1),
        bytes: b"hello".to_vec(),
        logins: logins.clone(),
        succeed_after_logins: 999,
        fetch_calls: Arc::new(AtomicUsize::new(0)),
    });
    let auth = Arc::new(fakes::SharedLoginAuthenticator {
        logins: logins.clone(),
        login_fails: false,
    });
    let files = Arc::new(fakes::FakeFileStore::default());
    let service = DownloadService::new(source, files)
        .with_authenticator(auth, None)
        .with_retry_policy(zero_backoff());

    let summary = service.run(&one_entry_manifest()).await;

    assert_eq!(summary.failed, 1);
    assert_eq!(summary.downloaded, 0);
    assert_eq!(summary.exit_code(), 1);
    assert_eq!(
        logins.load(Ordering::SeqCst),
        2,
        "two re-logins are performed, then the run aborts on the third request"
    );
    assert!(
        summary.failed_assets[0].reason.contains("not yielding"),
        "got: {:?}",
        summary.failed_assets[0].reason
    );
}

#[tokio::test]
async fn mid_run_login_failure_aborts_the_asset() {
    let logins = Arc::new(AtomicUsize::new(0));
    let source = Arc::new(fakes::AuthExpiringSource {
        assets: assets(1),
        bytes: b"hello".to_vec(),
        logins: logins.clone(),
        succeed_after_logins: 999,
        fetch_calls: Arc::new(AtomicUsize::new(0)),
    });
    let auth = Arc::new(fakes::SharedLoginAuthenticator {
        logins: logins.clone(),
        login_fails: true,
    });
    let files = Arc::new(fakes::FakeFileStore::default());
    let service = DownloadService::new(source, files)
        .with_authenticator(auth, None)
        .with_retry_policy(zero_backoff());

    let summary = service.run(&one_entry_manifest()).await;

    assert_eq!(summary.failed, 1);
    assert_eq!(summary.downloaded, 0);
    assert!(
        summary.failed_assets[0]
            .reason
            .contains("aws sso login failed"),
        "the abort should carry the login failure reason, got: {:?}",
        summary.failed_assets[0].reason
    );
}

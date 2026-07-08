mod fakes;

use std::sync::atomic::Ordering;

use acd::app::ensure_session;
use acd::ports::SessionStatus;

use fakes::FakeAuthenticator;

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

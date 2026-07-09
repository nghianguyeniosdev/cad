//! Manual demo of the mid-run Session Re-login (ADR 0003) — NOT part of the
//! shipped binary.
//!
//! It wires the REAL `SsoAuthenticator` (so a genuine `aws sso login` runs) to
//! an in-memory source whose Assets all fail with `AuthExpired` until a login
//! succeeds. With several Assets expiring at once, you should see the
//! SessionCoordinator perform exactly ONE login (single-flight, pause-the-world)
//! and then all Assets complete.
//!
//! Run:
//!   cargo run --example relogin_demo -- <your-aws-profile>
//!
//! (You'll be prompted to complete a real SSO login in the browser.)

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use acd::adapters::fs::LocalFileStore;
use acd::adapters::sso::SsoAuthenticator;
use acd::app::DownloadService;
use acd::domain::{Asset, ConnectionSettings, Entry, Failure, Layout, Manifest};
use acd::ports::{AssetStream, Authenticator, PackageSource, SessionStatus};

/// Wraps the real `SsoAuthenticator`: flips a shared flag once login succeeds,
/// and counts login calls so we can prove single-flight.
struct DemoAuthenticator {
    inner: SsoAuthenticator,
    logged_in: Arc<AtomicBool>,
    login_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Authenticator for DemoAuthenticator {
    async fn session_status(&self, profile: Option<&str>) -> SessionStatus {
        self.inner.session_status(profile).await
    }

    async fn login(&self, profile: Option<&str>) -> Result<(), Failure> {
        let n = self.login_calls.fetch_add(1, Ordering::SeqCst) + 1;
        eprintln!("[demo] >>> SessionCoordinator triggering real `aws sso login` (call #{n})");
        let result = self.inner.login(profile).await;
        if result.is_ok() {
            self.logged_in.store(true, Ordering::SeqCst);
        }
        result
    }
}

/// Every fetch fails with `AuthExpired` until `logged_in` becomes true.
struct ExpireUntilLoggedInSource {
    logged_in: Arc<AtomicBool>,
    assets: Vec<Asset>,
}

#[async_trait]
impl PackageSource for ExpireUntilLoggedInSource {
    async fn list_assets(&self, _entry: &Entry) -> Result<Vec<Asset>, Failure> {
        Ok(self.assets.clone())
    }

    async fn fetch_asset(&self, _entry: &Entry, asset: &Asset) -> Result<AssetStream, Failure> {
        if self.logged_in.load(Ordering::SeqCst) {
            Ok(Box::pin(futures::stream::once(async {
                Ok(b"hello".to_vec())
            })))
        } else {
            eprintln!("[demo] fetch of {} hit an expired session", asset.name);
            Err(Failure::auth_expired("simulated expired SSO token"))
        }
    }
}

#[tokio::main]
async fn main() {
    let profile = std::env::args().nth(1);

    // 5 Assets, all expiring at once -> should trigger a single re-login.
    let assets: Vec<Asset> = (0..5)
        .map(|i| Asset {
            name: format!("demo{i}.bin"),
            size: 5,
            expected_md5: "5d41402abc4b2a76b9719d911017c592".into(), // md5("hello")
        })
        .collect();

    let logged_in = Arc::new(AtomicBool::new(false));
    let login_calls = Arc::new(AtomicUsize::new(0));

    let source = Arc::new(ExpireUntilLoggedInSource {
        logged_in: logged_in.clone(),
        assets,
    });
    let files = Arc::new(LocalFileStore);
    let authenticator: Arc<dyn Authenticator> = Arc::new(DemoAuthenticator {
        inner: SsoAuthenticator,
        logged_in,
        login_calls: login_calls.clone(),
    });

    let manifest = Manifest {
        layout: Layout::Flat,
        connection: ConnectionSettings {
            domain: "demo".into(),
            domain_owner: "000000000000".into(),
            repository: "demo".into(),
            region: None,
        },
        packages: vec![Entry {
            namespace: None,
            package: "demo".into(),
            version: "0.0.0".into(),
            dest: std::env::temp_dir().join("acd-relogin-demo"),
        }],
    };

    let service = DownloadService::new(source, files).with_authenticator(authenticator, profile);
    let summary = service.run(&manifest).await;

    println!(
        "\n[demo] result: downloaded={}, failed={}, login_calls={}",
        summary.downloaded,
        summary.failed,
        login_calls.load(Ordering::SeqCst)
    );
    println!("[demo] expected: downloaded=5, failed=0, login_calls=1 (single-flight)");
}

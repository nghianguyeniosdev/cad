//! Orchestration over the ports: Planner, Downloader, AssetTask, RetryPolicy,
//! and SessionCoordinator. Depends on `domain` + `ports` only. See ADR 0004.

pub mod download;
pub mod init;
pub mod planner;
pub mod retry;
pub mod session;

pub use download::DownloadService;
pub use planner::Planner;
pub use retry::RetryPolicy;
pub use session::{ensure_session, SessionCoordinator};

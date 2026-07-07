//! Orchestration over the ports: Planner, Downloader, AssetTask, RetryPolicy,
//! and SessionCoordinator. Depends on `domain` + `ports` only. See ADR 0004.

pub mod download;
pub mod planner;

pub use download::DownloadService;
pub use planner::Planner;

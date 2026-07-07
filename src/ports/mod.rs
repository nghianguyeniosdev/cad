//! Port traits — the abstract seams (`Arc<dyn Trait>` + `async-trait`) that
//! `app` depends on and `adapters` implement. See ADR 0004.
//!
//! Remaining seams (`Authenticator`, `ProgressReporter`) are introduced by the
//! slices that first consume them, so their shapes are driven by real tests.

pub mod file_store;
pub mod package_source;
pub mod progress;

pub use file_store::FileStore;
pub use package_source::{AssetStream, PackageSource};
pub use progress::{NoopReporter, ProgressReporter};

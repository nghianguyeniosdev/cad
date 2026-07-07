//! In-memory fakes for the ports, used to drive the library seam end-to-end
//! without touching AWS. Concrete fakes are added alongside the port traits
//! they implement (starting with the download path in #2):
//!
//! - fake `PackageSource` returning scripted Assets and scriptable failures
//! - fake `FileStore` backed by an in-memory map
//! - fake `Authenticator` that can emit `AuthExpired` on a schedule
//! - recording `ProgressReporter`
//!
//! This module is included by integration tests via `mod fakes;` once they
//! exercise the pipeline.

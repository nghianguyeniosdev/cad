//! Concrete implementations of the ports: AWS SDK (`PackageSource`),
//! `aws sso login` subprocess (`Authenticator`), local filesystem
//! (`FileStore`), and `indicatif`/plain progress (`ProgressReporter`).
//! See ADR 0004.

pub mod aws;
pub mod fs;

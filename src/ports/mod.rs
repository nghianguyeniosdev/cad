//! Port traits — the abstract seams (`Arc<dyn Trait>` + `async-trait`) that
//! `app` depends on and `adapters` implement. See ADR 0004.
//!
//! The trait signatures are introduced by the slices that first consume them
//! (`PackageSource`/`FileStore` with the download path, `Authenticator` with
//! auto-login), so their shapes are driven by real tests rather than guessed
//! up front.

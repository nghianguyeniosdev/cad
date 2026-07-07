//! Composition root: constructs concrete adapters and injects them into `app`
//! services. The only place (besides `cli`/`main`) allowed to depend on every
//! layer. Populated as adapters and services come online. See ADR 0004.

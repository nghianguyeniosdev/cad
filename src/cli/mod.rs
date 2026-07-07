//! CLI surface: command parsing and dispatch to `app`. Currently the dispatch
//! lives in `lib::run`; richer parsing (flags like `--manifest`, `--profile`,
//! `--concurrency`) moves here with the slices that introduce those options.

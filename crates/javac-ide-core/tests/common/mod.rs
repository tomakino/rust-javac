//! Shared test setup: one process-wide platform directory.
//!
//! `RAVAC_PLATFORM_DIR` is process-global. All `#[test]` functions in an
//! integration-test binary may run concurrently, and `std::env::set_var` is
//! unsafe precisely because concurrent environment mutation is undefined.
//! The old per-file copies guarded only with an `if` check, so two tests could
//! both pass the check and call `set_var` concurrently. `Once` serializes the
//! initialization: it runs exactly once per test binary, while every caller
//! afterwards only reads the environment.

use std::path::PathBuf;
use std::sync::Once;

static PLATFORM_INIT: Once = Once::new();

/// Ensure `RAVAC_PLATFORM_DIR` is initialized and return its value.
pub fn platform_path() -> PathBuf {
    PLATFORM_INIT.call_once(|| {
        if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("debug");
            // SAFETY: `Once` guarantees that no two threads execute this
            // environment write concurrently.
            unsafe {
                std::env::set_var("RAVAC_PLATFORM_DIR", &root);
            }
        }
    });
    PathBuf::from(
        std::env::var_os("RAVAC_PLATFORM_DIR").expect("RAVAC_PLATFORM_DIR is initialized"),
    )
}

/// Ensure `RAVAC_PLATFORM_DIR` is initialized (files that only need the side
/// effect keep their existing `platform_dir()` calls).
pub fn platform_dir() {
    let _ = platform_path();
}

//! Shared test setup: one process-wide platform directory.
//!
//! `RAVAC_PLATFORM_DIR` is process-global. All `#[test]` functions in an
//! integration-test binary may run concurrently, and `std::env::set_var` is
//! unsafe precisely because concurrent environment mutation is undefined.
//! The old per-file copies guarded only with an `if` check, so two tests could
//! both pass the check and call `set_var` concurrently. `Once` serializes the
//! initialization: it runs exactly once per test binary, while every caller
//! afterwards only reads the environment.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::sync::{Mutex, Once};

static PLATFORM_INIT: Once = Once::new();
/// Serializes every test-side environment mutation in this test binary.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Ensure `RAVAC_PLATFORM_DIR` is initialized and return its value.
pub fn platform_path() -> PathBuf {
    // Fast path: once the platform is initialized there is no env write and
    // callers inside `with_env_vars` must not try to take ENV_MUTEX again.
    if !PLATFORM_INIT.is_completed() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        PLATFORM_INIT.call_once(|| {
            if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
                let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("target")
                    .join("debug");
                // SAFETY: ENV_MUTEX serializes this write with `with_env_vars`,
                // and Once guarantees it happens at most once.
                unsafe {
                    std::env::set_var("RAVAC_PLATFORM_DIR", &root);
                }
            }
        });
    }
    PathBuf::from(
        std::env::var_os("RAVAC_PLATFORM_DIR").expect("RAVAC_PLATFORM_DIR is initialized"),
    )
}

/// Ensure `RAVAC_PLATFORM_DIR` is initialized (files that only need the side
/// effect keep their existing `platform_dir()` calls).
pub fn platform_dir() {
    let _ = platform_path();
}

/// Run `f` while temporary environment variables are set.
///
/// `std::env::set_var`/`remove_var` are process-global and unsafe when other
/// threads can read or mutate the environment. ENV_MUTEX serializes all
/// environment changes made by tests in this binary; `platform_dir()` is
/// initialized before taking the lock so its one-time setup cannot deadlock.
/// Original values (including absence) are restored even if `f` panics.
pub fn with_env_vars<T>(vars: &[(&str, &OsStr)], f: impl FnOnce() -> T) -> T {
    // Initialize the platform first: this may write RAVAC_PLATFORM_DIR once,
    // and must happen outside the env guard to keep the lock non-reentrant.
    platform_dir();
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut saved: Vec<(String, Option<OsString>)> = Vec::new();
    for (name, value) in vars {
        saved.push(((*name).to_string(), std::env::var_os(name)));
        // SAFETY: ENV_MUTEX is held for the complete mutation interval.
        unsafe {
            std::env::set_var(name, value);
        }
    }

    struct EnvRestore {
        saved: Vec<(String, Option<OsString>)>,
    }
    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                // SAFETY: with_env_vars still holds ENV_MUTEX during drop.
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }
    let _restore = EnvRestore { saved };

    f()
}

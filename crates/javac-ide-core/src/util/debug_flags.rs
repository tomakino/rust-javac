//! Debug switches read from the environment.
//!
//! The compiler checks these switches from hot paths (overload resolution,
//! type substitution, lowering). Querying the operating system on every check
//! showed up as `RtlQueryEnvironmentVariable` in profiles, so the environment
//! is scanned once per process and the switches are cached.
//!
//! `DSH_DEBUG` is the process-wide master switch: without it (the normal
//! compiler run) every check answers false after one cached boolean load, and
//! the per-switch set is never built.

use std::collections::HashSet;
use std::sync::OnceLock;

/// Whether the master `DSH_DEBUG` switch is set for this process.
static DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();
static DEBUG_FLAGS: OnceLock<HashSet<String>> = OnceLock::new();

/// Return whether the named `DSH_DEBUG_*` environment switch is present.
///
/// Requires the `DSH_DEBUG` master switch; when it is absent every check is a
/// cached boolean load and a branch.
pub fn debug_enabled(name: &str) -> bool {
    if !*DEBUG_ENABLED.get_or_init(|| std::env::var_os("DSH_DEBUG").is_some()) {
        return false;
    }
    DEBUG_FLAGS
        .get_or_init(|| {
            std::env::vars_os()
                .filter_map(|(key, _)| key.into_string().ok())
                .filter(|key| key.starts_with("DSH_DEBUG_"))
                .collect()
        })
        .contains(name)
}

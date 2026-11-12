//! Locate the platform data that is compiled into the binary.
//!
//! The repository bundles one platform image in
//! `internal/platform/<version>/{modules, ct.sym}`, generated from the
//! reference JDK (see `internal/platform/README.md`).  `main.rs` embeds both
//! files with `include_bytes!`; this script only reports them as build
//! dependencies so a regenerated image triggers a rebuild.

use std::env;
use std::path::PathBuf;

/// Version of the bundled platform data (the reference compiler version).
const PLATFORM_VERSION: &str = "17.0.19+10";

fn main() {
    println!("cargo:rerun-if-env-changed=BUILD_DATE");
    // -version provenance: honour an explicit BUILD_DATE (reproducible builds),
    // otherwise use the build day in UTC.
    let build_date = env::var("BUILD_DATE").unwrap_or_else(|_| today_utc());
    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    // The platform data is compiled into the binary (`include_bytes!` in
    // main.rs), so the build script only has to watch it for changes.
    if let Some(dir) = bundled_platform() {
        println!("cargo:rerun-if-changed={}", dir.join("modules").display());
        println!("cargo:rerun-if-changed={}", dir.join("ct.sym").display());
    }
}

/// The bundled platform directory, when the checkout contains it.
fn bundled_platform() -> Option<PathBuf> {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    let dir = manifest
        .join("..")
        .join("..")
        .join("internal")
        .join("platform")
        .join(PLATFORM_VERSION);
    if dir.join("modules").is_file() {
        Some(dir)
    } else {
        None
    }
}

/// The current UTC date as YYYY-MM-DD (converted from the Unix day count).
fn today_utc() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

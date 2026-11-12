//! Prepare the platform data next to the development binary.
//!
//! The repository bundles one platform image in
//! `internal/platform/<version>/{modules, ct.sym}`, generated from the
//! reference JDK (see `internal/platform/README.md`).  A development build
//! lives in `target/<profile>/ravac.exe` and reads the data next to itself,
//! so this script links the bundled files into `target/<profile>/` (flat
//! layout).  When the bundle is absent it falls back to `$JAVA_HOME/lib`, so
//! a checkout without the data still builds on a machine with a JDK 17.
//!
//! The resulting binary behaves exactly like the packaged distribution and
//! never consults JAVA_HOME or PATH at run time.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Version of the bundled platform data (the reference compiler version).
const PLATFORM_VERSION: &str = "17.0.19+10";

fn main() {
    println!("cargo:rerun-if-env-changed=JAVA_HOME");
    println!("cargo:rerun-if-env-changed=BUILD_DATE");
    // -version provenance: honour an explicit BUILD_DATE (reproducible builds),
    // otherwise use the build day in UTC.
    let build_date = env::var("BUILD_DATE").unwrap_or_else(|_| today_utc());
    println!("cargo:rustc-env=BUILD_DATE={build_date}");

    let Ok(out_dir) = env::var("OUT_DIR") else {
        return;
    };
    let out = PathBuf::from(out_dir);
    // OUT_DIR = target/<profile>/build/<crate>/<hash>/out
    let Some(profile_dir) = out.ancestors().nth(4) else {
        return;
    };

    let bundled = bundled_platform();
    if let Some(dir) = &bundled {
        println!("cargo:rerun-if-changed={}", dir.join("modules").display());
        println!("cargo:rerun-if-changed={}", dir.join("ct.sym").display());
    }
    let source_dir = bundled.or_else(|| {
        env::var_os("JAVA_HOME")
            .map(PathBuf::from)
            .map(|home| home.join("lib"))
    });
    let Some(source_dir) = source_dir else {
        println!(
            "cargo:warning=no bundled platform data and JAVA_HOME is not set; \
             target/<profile>/ is not prepared, so ravac.exe will not find modules"
        );
        return;
    };

    for name in ["modules", "ct.sym"] {
        link_platform_file(&source_dir, profile_dir, name);
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

/// Hard-links (or copies) one platform file into the profile directory.
fn link_platform_file(source_dir: &Path, profile_dir: &Path, name: &str) {
    let source = source_dir.join(name);
    if !source.is_file() {
        return;
    }
    let target = profile_dir.join(name);
    let same_size = match (fs::metadata(&target), fs::metadata(&source)) {
        (Ok(target_meta), Ok(source_meta)) => target_meta.len() == source_meta.len(),
        _ => false,
    };
    if same_size {
        return;
    }
    let _ = fs::remove_file(&target);
    if fs::hard_link(&source, &target).is_err() {
        let _ = fs::copy(&source, &target);
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

//! Prepare the platform data next to the development binary.
//!
//! The runtime only reads modules (and ct.sym) next to ravac.exe (or one
//! level above).  A development build lives in target/<profile>/ravac.exe,
//! so this script hard-links the baseline JDK data from JAVA_HOME into
//! target/<profile>/ (flat layout).
//! The resulting binary behaves exactly like the packaged distribution and
//! never consults JAVA_HOME or PATH at run time.

use std::env;
use std::fs;
use std::path::PathBuf;

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
    let Some(java_home) = env::var_os("JAVA_HOME").map(PathBuf::from) else {
        println!(
            "cargo:warning=JAVA_HOME is not set; target/<profile>/lib is not prepared, \
             so ravac.exe will not find modules (set JAVA_HOME and rebuild)"
        );
        return;
    };

    for name in ["modules", "ct.sym"] {
        let source = java_home.join("lib").join(name);
        if !source.is_file() {
            continue;
        }
        let target = profile_dir.join(name);
        let same_size = match (fs::metadata(&target), fs::metadata(&source)) {
            (Ok(target_meta), Ok(source_meta)) => target_meta.len() == source_meta.len(),
            _ => false,
        };
        if same_size {
            continue;
        }
        let _ = fs::remove_file(&target);
        if fs::hard_link(&source, &target).is_err() {
            let _ = fs::copy(&source, &target);
        }
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

//! gen-ct-sym: convert the JDK's `lib/ct.sym` zip into the compact RCS1 container.
//!
//! Entry order, names and payloads are preserved; identical payloads are stored
//! once and the unique payloads are block-compressed (see javac-ctsym::rcs).

use std::path::{Path, PathBuf};

use javac_ctsym::RcsBuilder;
use javac_zip::ZipArchive;

/// Version of the bundled platform data (the reference compiler version).
const PLATFORM_VERSION: &str = "17.0.19+10";

pub fn run(args: &[String]) -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut check = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--in" => {
                input = Some(PathBuf::from(
                    args.get(index + 1).ok_or("--in needs a path")?,
                ));
                index += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(
                    args.get(index + 1).ok_or("--out needs a path")?,
                ));
                index += 2;
            }
            "--check" => {
                check = true;
                index += 1;
            }
            other => return Err(format!("unknown gen-ct-sym option: {other}")),
        }
    }
    let input = input
        .or_else(|| {
            std::env::var_os("JAVA_HOME").map(|home| PathBuf::from(home).join("lib").join("ct.sym"))
        })
        .ok_or("no --in given and JAVA_HOME is not set")?;
    if check {
        if out.is_some() {
            return Err("--check cannot be combined with --out".to_string());
        }
        let bundled = PathBuf::from("internal/platform")
            .join(PLATFORM_VERSION)
            .join("ct.sym");
        if !bundled.is_file() {
            return Err(format!("no bundled ct.sym at {}", bundled.display()));
        }
        let temp = PathBuf::from("target/platform/check-ct.sym");
        if let Some(parent) = temp.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        convert(&input, &temp)?;
        let expected = std::fs::read(&bundled)
            .map_err(|error| format!("read {}: {error}", bundled.display()))?;
        let actual =
            std::fs::read(&temp).map_err(|error| format!("read {}: {error}", temp.display()))?;
        let _ = std::fs::remove_file(&temp);
        if expected != actual {
            return Err(format!(
                "bundled {} does not match a fresh conversion ({} vs {} bytes)",
                bundled.display(),
                expected.len(),
                actual.len()
            ));
        }
        println!("gen-ct-sym: bundled ct.sym matches a fresh conversion");
        return Ok(());
    }
    let out = out.ok_or("--out is required")?;
    convert(&input, &out)
}

fn convert(input: &Path, out: &Path) -> Result<(), String> {
    let started = std::time::Instant::now();
    let archive =
        ZipArchive::open(input).map_err(|error| format!("open {}: {error}", input.display()))?;
    let mut builder = RcsBuilder::new();
    let mut payload_bytes = 0usize;
    for (index, entry) in archive.entries().iter().enumerate() {
        let data = if entry.is_directory() {
            Vec::new()
        } else {
            archive
                .get_entry_of(index)
                .ok_or_else(|| format!("cannot read entry {}", entry.name()))?
        };
        payload_bytes += data.len();
        builder.add_entry(entry.name(), entry.is_directory(), data);
    }
    builder
        .write(out)
        .map_err(|error| format!("write {}: {error}", out.display()))?;
    let size = std::fs::metadata(out).map(|meta| meta.len()).unwrap_or(0);
    println!(
        "gen-ct-sym: {} entries, {} payload bytes, {} bytes, {:.1}s",
        archive.entries().len(),
        payload_bytes,
        size,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

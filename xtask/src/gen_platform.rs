//! gen-platform: convert a JDK `lib/modules` jimage into the stripped RPI1 image.
//!
//! The output keeps every module, package and class the compiler can see, but each
//! class file loses its method bodies (`Code` attribute). Nothing else changes: the
//! constant pool and the other attributes are copied byte for byte.

use std::path::{Path, PathBuf};
use std::time::Instant;

use javac_jimage::JImage;
use javac_platform::ImageBuilder;

pub fn run(args: &[String]) -> Result<(), String> {
    let mut modules: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--modules" => {
                let value = args
                    .get(index + 1)
                    .ok_or("--modules needs a path")?;
                modules = Some(PathBuf::from(value));
                index += 2;
            }
            "--out" => {
                let value = args.get(index + 1).ok_or("--out needs a path")?;
                out = Some(PathBuf::from(value));
                index += 2;
            }
            other => return Err(format!("unknown gen-platform option: {other}")),
        }
    }
    let modules = modules
        .or_else(|| {
            std::env::var_os("JAVA_HOME")
                .map(|home| PathBuf::from(home).join("lib").join("modules"))
        })
        .ok_or("no --modules given and JAVA_HOME is not set")?;
    let out = out.ok_or("--out is required")?;
    convert(&modules, &out)
}

fn convert(modules: &Path, out: &Path) -> Result<(), String> {
    let started = Instant::now();
    let image = JImage::open(modules).map_err(|error| format!("open {}: {error}", modules.display()))?;
    let mut builder = ImageBuilder::new();
    for module in image.modules() {
        builder.add_module(module);
    }

    let mut resource_count = 0usize;
    let mut class_count = 0usize;
    let mut removed_bytes = 0usize;
    for name in image.resources() {
        let (module, path) = name
            .split_once('/')
            .ok_or_else(|| format!("malformed resource name: {name}"))?;
        let bytes = image
            .find_resource(name)
            .ok_or_else(|| format!("missing resource: {name}"))?;
        let bytes = if path.ends_with(".class") {
            class_count += 1;
            match javac_platform::compact_class(&bytes) {
                Some(stripped) => {
                    removed_bytes += bytes.len() - stripped.len();
                    stripped
                }
                None => return Err(format!("cannot parse class file: {name}")),
            }
        } else {
            bytes
        };
        resource_count += 1;
        builder.add_resource(module, path, bytes);
    }

    builder
        .write(out)
        .map_err(|error| format!("write {}: {error}", out.display()))?;
    let image_bytes = std::fs::metadata(out).map(|meta| meta.len()).unwrap_or(0);
    println!(
        "gen-platform: {resource_count} resources ({class_count} classes), \
         {removed_bytes} bytes removed (Code + dead pool entries), image {image_bytes} bytes, {:.1}s",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

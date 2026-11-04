//! Compile-time version and provenance information.

/// The `-version` output: the upstream javac prefix followed by the port
/// provenance (implementation, Cargo version, build date, author), so
/// version-sniffing scripts still see `javac 17.0.19`.
pub const VERSION_LINE: &str = concat!(
    "javac 17.0.19 (ravac ",
    env!("CARGO_PKG_VERSION"),
    ", built ",
    env!("BUILD_DATE"),
    ", Tomakino)"
);

/// The `-fullversion` output.
pub const FULL_VERSION_LINE: &str = concat!(
    "javac full version \"17.0.19\" (ravac ",
    env!("CARGO_PKG_VERSION"),
    ", built ",
    env!("BUILD_DATE"),
    ", Tomakino)"
);

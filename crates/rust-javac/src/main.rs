//! Command line entry point.
//!
//! The driver itself lives in the library (`ravac::run`), so the same code
//! also backs the `ravac_compile` C ABI exported by the shared library.

fn main() {
    std::process::exit(ravac::run());
}

//! com.sun.tools.javac.platform
//!
//! An internal API for plugging in --release implementations.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES (Porting rules): one Rust file per Java file of the package; the
//! package-info.java documentation is kept here.  PlatformDescription and
//! PlatformProvider are traits, PlatformUtils keeps the provider selection (the
//! ServiceLoader dependency is P6, see platform_utils.rs), and JDKPlatformProvider
//! is ported without its JVM/filesystem-dependent parts (dependency report in
//! jdk_platform_provider.rs).

pub mod jdk_platform_provider;
pub mod platform_description;
pub mod platform_provider;
pub mod platform_utils;

// Origin: jdk/internal/math/FloatingDecimal.java + FDBigInteger.java @ 982d53c (jdk-17.0.19-ga)

//! Ports of the JDK floating-point conversion classes used by javac:
//! `jdk.internal.math.FloatingDecimal` (JLS-compatible `Float.valueOf`/`Double.valueOf`
//! parsing and `Float.toString`/`Double.toString` formatting) and its helper
//! `jdk.internal.math.FDBigInteger`.
//!
//! PORTING_RULES §9.17 (standard-library adapter):
//! * These classes are JDK standard library sources, not javac sources; the port
//!   keeps the upstream algorithms, control flow, comments and constants 1:1 and
//!   only adapts Java language constructs to Rust (see docs/DEPENDENCIES.md).
//! * Java `int[]` big-integer words become `Vec<i32>`, with Java's two's
//!   complement/wrapping semantics preserved explicitly (`wrapping_*`, `as u32`
//!   for logical shifts, `i64` for the upstream `long` arithmetic).
//! * Java `String`/`char[]` inputs become `&str`/`&[u16]`; `StringBuilder` becomes
//!   `String`.
//! * `java.math.BigInteger` is not ported (debug-only upstream conversions): the
//!   corresponding methods are explicit `unimplemented!(TODO)`.
//!
//! The crate is dependency-free so that the compiler front end can use it without
//! pulling in the JDK class library.
//!
//! PORTING_RULES §9.17/§17: the upstream control flow/structure is kept 1:1, so
//! clippy style lints that would require restructuring the ported code are
//! allowed crate-wide (they are not behavioral changes):
#![allow(
    clippy::needless_return,
    clippy::unnecessary_cast,
    clippy::manual_div_ceil,
    clippy::manual_range_contains,
    clippy::collapsible_if,
    clippy::needless_late_init,
    clippy::needless_range_loop,
    clippy::assign_op_pattern,
    clippy::approx_constant,
    clippy::manual_clamp,
    clippy::float_cmp
)]

pub mod fdbig_integer;
pub mod floating_decimal;

pub use floating_decimal::{
    parse_double, parse_float, to_java_string_double, to_java_string_float, try_parse_double,
    try_parse_float, NumberFormatException,
};

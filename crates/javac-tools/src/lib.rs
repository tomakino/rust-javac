//! Rust representation of the `java.compiler` module API (JDK 17) that javac
//! implements and uses: `javax.tools`, plus the minimal `javax.lang.model`
//! types required by its signatures.
//!
//! Ported from: java.compiler/share/classes/javax/tools/*.java @ 982d53c
//!               (jdk-17.0.19-ga); `javax.lang.model` files are referenced by
//!               individual source anchors.
//!
//! Source layout mirrors the Java package tree (the crate root stands for
//! `javax`, exactly as `javac-core` stands for `com/sun/tools/javac`):
//!
//!   javax/tools/X.java               -> src/tools/x.rs
//!   javax/tools/package-info.java    -> src/tools/mod.rs (module docs)
//!   javax/lang/model/X.java          -> src/lang/model/x.rs
//!   javax/lang/model/element/X.java  -> src/lang/model/element/x.rs
//!
//! — language-shape mapping used by this crate
//! ---------------------------------------------------------
//!
//! * Java interface -> Rust trait; Java default method -> trait default method.
//!   Default methods whose upstream body throws `UnsupportedOperationException`
//!   return `Err(Error::UnsupportedOperationException)`.
//! * Java concrete class -> Rust struct; Java class inheritance -> an embedded
//!   base-class field (`super_`), because Rust has no inheritance. See
//!   `ForwardingJavaFileObject`.
//! * Java object reference -> `Rc<dyn Trait>` (shared immutable identity, as in
//!   Java).  This crate never builds a mutable object graph with
//!   `Rc<RefCell<..>>`; the only `RefCell` is the shared mutable I/O channel
//!   `Rc<RefCell<dyn Writer>>` handed to `JavaCompiler::get_task`, which is
//!   Java's mutable `Writer` reference.
//! * Java `String`/`CharSequence` -> `javac_strings::JavaString` (UTF-16 code
//!   units, docs/porting adapter.md §7).  Java string semantics are never
//!   expressed with Rust `String` in this crate; literal constants are built
//!   with `JavaString::from("...")`.  `javac-strings` is the bottom-most layer
//!   shared with `javac-core`.
//! * Java `Iterable<T>`/`Collection<T>`/`List<T>` argument -> `&[T]`;
//!   return -> `Vec<T>`.  `Iterator<T>` argument ->
//!   `&mut dyn Iterator<Item = T>`.  `Set<T>` argument -> `&[T]`.
//! * Java checked `IOException` -> `Result<T, Error>` (`crate::error`, a
//!   placeholder replaced by the final error type later).
//!   `UnsupportedOperationException` results -> `Err(Error::UnsupportedOperationException)`.
//!   Other runtime exceptions (`IllegalArgumentException`,
//!   `IllegalStateException`, `NullPointerException`) do not appear in
//!   signatures; they are API misuse and map to `panic!` (documented per
//!   method), caught only at entry points (docs/porting adapter.md §6).
//! * JDK types without a Rust equivalent are mapped minimally and registered in
//!   the adapter files: `java.net.URI` -> `crate::uri::URI`;
//!   `java.io.Reader/Writer` -> `crate::java_io::{Reader, Writer}`;
//!   `java.io.InputStream/OutputStream` -> `std::io::Read/Write`;
//!   `java.lang.Class`, `java.lang.ClassLoader`, `java.util.ServiceLoader`,
//!   `javax.annotation.processing.Processor` -> placeholders in `crate::jdk`;
//!   `java.nio.charset.Charset` and `java.util.Locale` -> their names
//!   (`JavaString`); `java.io.File` and `java.nio.file.Path` ->
//!   `std::path::PathBuf`.
//!
//! REGISTERED P0: register the adapter files `error.rs`, `uri.rs`, `java_io.rs`,
//! `jdk.rs` in docs/porting adapter.md §9 and docs/STATUS.md (Lead).
//! P2: `java.net.URI` implements create/path/normalize/resolve/relativize/to_url;
//! REGISTERED: component-level validation and the remaining
//! `java.net.URI` semantics are added with the `file` package, which is the
//! first consumer.
//! REGISTERED P6: annotation processing is out of scope (docs/PLAN.md §3.1);
//! `Class`, `ClassLoader`, `ServiceLoader`, `Processor` placeholders and
//! `ToolProvider::get_system_java_compiler` are finalized by the API phase.

pub mod error;
pub mod java_io;
pub mod jdk;
pub mod lang;
pub mod tools;
pub mod uri;

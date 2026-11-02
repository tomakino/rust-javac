//! Interfaces for the compiler tree API of the Java programming language
//! (`com.sun.source`, JDK 17).
//!
//! Ported from: jdk.compiler/share/classes/com/sun/source/**.java @ 982d53c
//! (jdk-17.0.19-ga).  Packages: tree, doctree, util.
//!
//! This crate holds the interface layer only: the tree implementations
//! (`com.sun.tools.javac.tree.JCTree`) live in javac-core (P1) and the API
//! implementation (`JavacTask`, `Trees`, ...) in the P6 API phase.
//!
//! PORTING_RULES — language-shape mapping (docs/PORTING_RULES.md §12/§13/§14):
//! * Java interface -> Rust trait; Java abstract class -> Rust trait with the
//!   upstream default behavior (core version) and subtraits for the versioned
//!   class hierarchy.
//! * visitor interface methods are abstract in Java and carry a panicking
//!   default in Rust; the util layers supply the real defaults.
//! * generic double dispatch (`Tree.accept(TreeVisitor, D)`) cannot be called on
//!   `&dyn Tree` and keeps `where Self: Sized`; the util layer reproduces the
//!   dispatch from `getKind()` plus the object-conversion accessors
//!   (`Tree::as_xxx`, the Rust form of `instanceof`/class cast; same contract as
//!   javax.lang.model in javac-tools, §13).
//! * Java `String`/`CharSequence` -> `javac_strings::JavaString`.
//! * `List<? extends X>` -> `Vec<Rc<dyn X>>`; `Set<Modifier>` -> `Vec<Modifier>`;
//!   nullable returns -> `Option<...>`.
//! * Java object references -> `Rc<dyn Trait>`.
//! * the single-argument convenience overloads that only pass `null` are not
//!   ported.
//! * Java `assert` -> `debug_assert!`; unconditional `AssertionError` ->
//!   `panic!` with the upstream message.

pub mod doctree;
pub mod tree;
pub mod util;

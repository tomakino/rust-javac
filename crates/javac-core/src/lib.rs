#![allow(special_module_name)]

//! Core of rust-javac: a Rust port of OpenJDK javac 17.
//!
//! Ported from: jdk17u @ 982d53c (jdk-17.0.19-ga).
//! Source layout mirrors `com/sun/tools/javac` package by package.

pub mod api;
pub mod code;
pub mod comp;
pub mod compiler_context;
pub mod file;
pub mod ids;
pub mod jvm;
pub mod launcher;
pub mod main;
pub mod model;
pub mod parser;
pub mod platform;
pub mod processing;
pub mod resources;
pub mod tree;
pub mod util;

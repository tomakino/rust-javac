//! Completion: spec-driven receiver resolution and member collection.
//!
//! - `entry`:    request entry point
//! - `receiver`: receiver/selector completion
//! - `expected`: expected-type inference for arguments, assignments, returns
//! - `new`:      `new` class/constructor completion
//! - `members`:  member collection and visibility
//! - `common`:   shared items, keywords, and small helpers
//! - `context`, `imports`, `insert`, `lexer`, `matcher`, `rank`, `search`:
//!   focused support modules

mod common;
mod context;
mod entry;
mod expected;
mod imports;
mod insert;
mod lexer;
mod matcher;
mod members;
mod new;
mod rank;
mod receiver;
mod search;

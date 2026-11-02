//! `javax.lang` package tree.
//!
//! Only the members required by the `javax.tools` signatures are ported so
//! far: `javax.lang.model.SourceVersion`, `javax.lang.model.element.Modifier`
//! and `javax.lang.model.element.NestingKind`.  The remaining language-model
//! interfaces are ported with the phases that use them.

pub mod model;

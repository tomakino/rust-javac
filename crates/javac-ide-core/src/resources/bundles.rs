//! Embedded official message bundles: raw .properties text under
//! `crates/javac-core/resources/l10n/` (kept outside `src/`).

use crate::resources::message_table::MessageTable;

pub static COMPILER_EN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/compiler.properties"));
pub static JAVAC_EN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/javac.properties"));
pub static LAUNCHER_EN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/launcher.properties"));
/// CLDR likely-subtags subset: region/script defaults for picking a bundle.
pub static LIKELY_SUBTAGS: MessageTable = MessageTable::new(include_str!(
    "../../resources/l10n/likely-subtags.properties"
));
/// Locale variants discovered from the resources/l10n file names by build.rs
/// (one static table plus a _VARIANTS list per bundle).
pub use crate::resources::locales::{
    COMPILER_JA, COMPILER_VARIANTS, COMPILER_ZH_CN, COMPILER_ZH_HANT, JAVAC_JA, JAVAC_VARIANTS,
    JAVAC_ZH_CN, JAVAC_ZH_HANT, LAUNCHER_JA, LAUNCHER_VARIANTS, LAUNCHER_ZH_CN,
    LAUNCHER_ZH_HANT,
};

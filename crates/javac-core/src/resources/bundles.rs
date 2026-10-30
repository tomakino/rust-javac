//! Embedded official message bundles: raw .properties text under
//! `crates/javac-core/resources/l10n/` (kept outside `src/`).

use crate::resources::message_table::MessageTable;

pub static COMPILER_EN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/compiler.properties"));
pub static JAVAC_EN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/javac.properties"));
pub static LAUNCHER_EN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/launcher.properties"));
pub static COMPILER_ZH_CN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/compiler_zh_CN.properties"));
pub static JAVAC_ZH_CN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/javac_zh_CN.properties"));
pub static LAUNCHER_ZH_CN: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/launcher_zh_CN.properties"));
pub static COMPILER_JA: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/compiler_ja.properties"));
pub static JAVAC_JA: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/javac_ja.properties"));
pub static LAUNCHER_JA: MessageTable =
    MessageTable::new(include_str!("../../resources/l10n/launcher_ja.properties"));

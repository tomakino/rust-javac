//! Generated resource tables and the P4 diagnostic contract (upstream javac message resources).

#[path = "compiler_zh_CN.rs"]
pub mod compiler_zh_cn;
#[path = "javac_zh_CN.rs"]
pub mod javac_zh_cn;
#[path = "launcher_zh_CN.rs"]
pub mod launcher_zh_cn;

pub mod compiler_ja;
pub mod compiler_properties;
pub mod diagnostic_contract;
pub mod javac_ja;
pub mod javac_properties;
pub mod launcher_ja;
pub mod launcher_properties;

#[cfg(test)]
mod tests {
    use super::compiler_ja::COMPILER_JA;
    use super::compiler_properties::{errors, COMPILER_KEYS, COMPILER_MESSAGES};
    use super::compiler_zh_cn::COMPILER_ZH_CN;
    use super::diagnostic_contract::{DiagnosticType, SymbolArg};
    use super::javac_ja::JAVAC_JA;
    use super::javac_properties::JAVAC_MESSAGES;
    use super::javac_zh_cn::JAVAC_ZH_CN;
    use super::launcher_ja::LAUNCHER_JA;
    use super::launcher_properties::LAUNCHER_MESSAGES;
    use super::launcher_zh_cn::LAUNCHER_ZH_CN;

    #[test]
    fn generated_metadata_matches_upstream_snapshot() {
        assert_eq!(COMPILER_KEYS.len(), 936);
        assert_eq!(COMPILER_MESSAGES.len(), 936);
        let entry = COMPILER_KEYS
            .iter()
            .find(|(key, ..)| *key == "compiler.err.plugin.not.found")
            .expect("plugin.not.found key");
        assert_eq!(entry.1, "Error");
        assert_eq!(entry.2, "plugin_not_found");
        assert_eq!(entry.3, 1);
        let kinds = ["Error", "Warning", "Note", "Fragment"];
        for kind in kinds {
            let count = COMPILER_KEYS
                .iter()
                .filter(|(_, entry_kind, ..)| *entry_kind == kind)
                .count();
            assert!(count > 0, "missing kind {kind}");
        }
    }

    #[test]
    fn generated_accessors_return_typed_diagnostic_info() {
        let info = errors::abstract_meth_cant_have_body();
        assert_eq!(info.info().kind, DiagnosticType::Error);
        assert_eq!(info.info().code, "abstract.meth.cant.have.body");
        assert_eq!(
            info.info().key(),
            "compiler.err.abstract.meth.cant.have.body"
        );
        assert!(info.info().args().is_empty());

        let info = errors::abstract_cant_be_instantiated(SymbolArg::from("X"));
        assert_eq!(info.info().code, "abstract.cant.be.instantiated");
        assert_eq!(info.info().args().len(), 1);
        assert_eq!(info.info().args()[0].0, "X");
    }

    #[test]
    fn message_tables_match_recorded_snapshot() {
        assert_eq!(COMPILER_MESSAGES.len(), 936);
        assert_eq!(COMPILER_ZH_CN.len(), 913);
        assert_eq!(COMPILER_JA.len(), 913);
        assert_eq!(LAUNCHER_MESSAGES.len(), 15);
        assert_eq!(LAUNCHER_ZH_CN.len(), 15);
        assert_eq!(LAUNCHER_JA.len(), 15);
        assert_eq!(JAVAC_MESSAGES.len(), 145);
        assert_eq!(JAVAC_ZH_CN.len(), 143);
        assert_eq!(JAVAC_JA.len(), 143);
    }

    #[test]
    fn translation_keys_are_base_keys_except_recorded_stale_entries() {
        fn extras<'a>(base: &[(&str, &str)], translation: &'a [(&str, &str)]) -> Vec<&'a str> {
            let base_keys: std::collections::BTreeSet<&str> =
                base.iter().map(|(key, _)| *key).collect();
            let mut extra: Vec<&str> = translation
                .iter()
                .map(|(key, _)| *key)
                .filter(|key| !base_keys.contains(key))
                .collect();
            extra.sort_unstable();
            extra
        }
        assert_eq!(
            extras(COMPILER_MESSAGES, COMPILER_ZH_CN),
            vec!["compiler.err.switch.null.not.allowed"]
        );
        assert_eq!(
            extras(COMPILER_MESSAGES, COMPILER_JA),
            vec!["compiler.err.switch.null.not.allowed"]
        );
        assert!(extras(LAUNCHER_MESSAGES, LAUNCHER_ZH_CN).is_empty());
        assert!(extras(LAUNCHER_MESSAGES, LAUNCHER_JA).is_empty());
        assert_eq!(
            extras(JAVAC_MESSAGES, JAVAC_ZH_CN),
            vec!["javac.opt.doclint.format"]
        );
        assert_eq!(
            extras(JAVAC_MESSAGES, JAVAC_JA),
            vec!["javac.opt.doclint.format"]
        );
    }

    #[test]
    fn translation_samples_match_expected_text() {
        fn value<'a>(table: &'a [(&'static str, &'static str)], key: &str) -> &'a str {
            table
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| *value)
                .unwrap_or_else(|| panic!("missing {key}"))
        }
        assert_eq!(
            value(COMPILER_ZH_CN, "compiler.err.abstract.meth.cant.have.body"),
            "抽象方法不能有主体"
        );
        assert_eq!(
            value(COMPILER_JA, "compiler.err.abstract.meth.cant.have.body"),
            "abstractメソッドが本体を持つことはできません"
        );
        assert_eq!(
            value(LAUNCHER_ZH_CN, "launcher.err.cant.find.class"),
            "找不到类：{0}"
        );
        assert_eq!(
            value(LAUNCHER_JA, "launcher.err.cant.find.class"),
            "クラスが見つかりません: {0}"
        );
        assert!(value(JAVAC_ZH_CN, "javac.msg.usage").contains("{0}"));
        assert!(value(JAVAC_JA, "javac.msg.usage").contains("{0}"));
    }
}

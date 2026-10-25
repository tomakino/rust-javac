//! 数据完整性测试:tree/token oracle 对照数据(运行期零 JVM 依赖)。
//!
//! 数据由开发期工具生成(见 tools/gen-tree-oracle、tools/gen-token-oracle),
//! 入库到 crates/javac-tests/data/,生成命令/JDK 版本/基线 commit 记录在数据头。
//! 本模块只校验数据文件的结构与完整性,不启动任何 JVM。

const TREE_ORACLE: &str = include_str!("../data/tree-oracle.txt");
const TOKEN_ORACLE: &str = include_str!("../data/token-oracle.txt");

fn header_value<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.lines().find_map(|line| line.strip_prefix(prefix))
}

/// 解析 tree oracle:返回 (记录名, Pretty 输出) 列表。
fn tree_records(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut records = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if let Some(rest) = lines[index].strip_prefix("=== FILE: ") {
            let name = rest
                .strip_suffix(" ===")
                .expect("tree oracle file marker must end with ===")
                .to_string();
            let body_start = index + 1;
            let mut end = body_start;
            while end < lines.len() && lines[end] != "=== END ===" {
                end += 1;
            }
            assert!(end < lines.len(), "unterminated tree oracle record: {name}");
            records.push((name, lines[body_start..end].join("\n")));
            index = end + 1;
        } else {
            index += 1;
        }
    }
    records
}

/// 解析 token oracle 的 TokenKind 清单:返回 (枚举名, name 字段, tag 字段)。
fn token_kinds(text: &str) -> Vec<(String, String, String)> {
    text.lines()
        .filter_map(|line| line.strip_prefix("KIND "))
        .map(|rest| {
            let parts: Vec<&str> = rest.split(' ').collect();
            assert!(
                parts.len() == 3,
                "malformed KIND line (expected 3 fields): {rest}"
            );
            assert!(parts[1].starts_with("name="), "missing name= in: {rest}");
            assert!(parts[2].starts_with("tag="), "missing tag= in: {rest}");
            (
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            )
        })
        .collect()
}

/// 解析 token oracle 的样本:返回 (样本名, token 行, 是否被截断)。
fn token_samples(text: &str) -> Vec<(String, Vec<String>, bool)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut samples = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if let Some(rest) = lines[index].strip_prefix("=== SAMPLE: ") {
            let name = rest
                .strip_suffix(" ===")
                .expect("token oracle sample marker must end with ===")
                .to_string();
            let body_start = index + 1;
            let mut end = body_start;
            while end < lines.len() && lines[end] != "=== END ===" {
                end += 1;
            }
            assert!(
                end < lines.len(),
                "unterminated token oracle sample: {name}"
            );
            let body = &lines[body_start..end];
            let tokens: Vec<String> = body
                .iter()
                .filter(|line| line.starts_with("TOKEN "))
                .map(|line| (*line).to_string())
                .collect();
            let truncated = body.iter().any(|line| line.starts_with("# truncated at "));
            samples.push((name, tokens, truncated));
            index = end + 1;
        } else {
            index += 1;
        }
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn tree_oracle_metadata_is_recorded() {
        assert!(
            TREE_ORACLE.contains("# generated-by: tools/gen-tree-oracle/GenTreeOracle.java"),
            "tree oracle must record the generator"
        );
        assert_eq!(
            header_value(TREE_ORACLE, "# jdk-version: "),
            Some("17.0.19")
        );
        assert_eq!(
            header_value(TREE_ORACLE, "# baseline: "),
            Some("982d53c (jdk-17.0.19-ga)")
        );
        assert!(
            header_value(TREE_ORACLE, "# print-mode: ").is_some(),
            "tree oracle must record the Pretty print mode"
        );
        assert!(
            header_value(TREE_ORACLE, "# parse-mode: ").is_some(),
            "tree oracle must record the parse mode"
        );
        assert!(
            header_value(TREE_ORACLE, "# command: ").is_some(),
            "tree oracle must record the generation command"
        );
        assert!(
            header_value(TREE_ORACLE, "# roots: ").is_some(),
            "tree oracle must record the corpus roots"
        );
        assert!(
            header_value(TREE_ORACLE, "# doc-comments: ").is_some(),
            "tree oracle must record the doc comment handling of the parse mode"
        );
    }

    #[test]
    fn tree_oracle_records_are_well_formed() {
        let records = tree_records(TREE_ORACLE);
        let expected = header_value(TREE_ORACLE, "# records: ")
            .expect("tree oracle records header")
            .parse::<usize>()
            .expect("records count");
        assert_eq!(
            records.len(),
            expected,
            "record count must match the header"
        );
        assert!(
            records.len() >= 30,
            "expected at least 30 tree oracle records, found {}",
            records.len()
        );
        let mut names = BTreeSet::new();
        for (name, pretty) in &records {
            assert!(
                !pretty.trim().is_empty(),
                "empty tree oracle record: {name}"
            );
            assert!(
                names.insert(name.clone()),
                "duplicate tree oracle record: {name}"
            );
        }
    }

    #[test]
    fn tree_oracle_covers_syntax_samples() {
        let records = tree_records(TREE_ORACLE);
        let lookup = |name: &str| {
            records
                .iter()
                .find(|(record, _)| record == name)
                .map(|(_, pretty)| pretty.as_str())
                .unwrap_or_else(|| panic!("missing tree oracle sample: {name}"))
        };
        let sample1 = lookup("samples/Sample1.java");
        assert!(
            sample1.contains("sealed interface Shape permits Circle, Square"),
            "Sample1 must cover sealed interfaces"
        );
        assert!(
            sample1.contains("text block with"),
            "Sample1 must cover text blocks"
        );
        assert!(
            sample1.contains("->"),
            "Sample1 must cover lambda/switch arrows"
        );
        let sample2 = lookup("samples/Sample2.java");
        assert!(sample2.contains("enum Color"), "Sample2 must cover enums");
        assert!(
            sample2.contains("try ("),
            "Sample2 must cover try-with-resources"
        );
        assert!(
            sample2.contains("Sample2::new"),
            "Sample2 must cover method references"
        );
        let sample3 = lookup("samples/Sample3.java");
        assert!(
            sample3.contains("<error>") || sample3.contains("(ERROR)"),
            "Sample3 must cover the erroneous tree printing"
        );
        let sample4 = lookup("samples/Sample4.java");
        assert!(sample4.contains("class Sample4"));
        // JavacTask.parse 会设置 keepComments=true,因此 doc comment 会打印出来。
        assert!(
            sample4.contains("/**") && sample4.contains("A documented class"),
            "Sample4 must cover doc comment printing"
        );
        let sample5 = lookup("samples/Sample5.java");
        assert!(
            sample5.contains("3735928559L"),
            "Sample5 must cover hexadecimal literal normalization"
        );
        assert!(
            sample5.contains("'A'"),
            "Sample5 must cover unicode escape processing"
        );
    }

    #[test]
    fn token_oracle_metadata_is_recorded() {
        assert!(
            TOKEN_ORACLE.contains("# generated-by: tools/gen-token-oracle/GenTokenOracle.java"),
            "token oracle must record the generator"
        );
        assert_eq!(
            header_value(TOKEN_ORACLE, "# jdk-version: "),
            Some("17.0.19")
        );
        assert_eq!(
            header_value(TOKEN_ORACLE, "# baseline: "),
            Some("982d53c (jdk-17.0.19-ga)")
        );
        assert!(
            header_value(TOKEN_ORACLE, "# scanner: ").is_some(),
            "token oracle must record the scanner mode"
        );
        assert!(
            header_value(TOKEN_ORACLE, "# command: ").is_some(),
            "token oracle must record the generation command"
        );
    }

    #[test]
    fn token_oracle_token_kinds_are_complete() {
        let kinds = token_kinds(TOKEN_ORACLE);
        let expected = header_value(TOKEN_ORACLE, "# count: ")
            .expect("TokenKind count header")
            .parse::<usize>()
            .expect("TokenKind count");
        assert_eq!(
            kinds.len(),
            expected,
            "KIND line count must match the header"
        );
        // 实测基线 JDK 17.0.19 的 Tokens.TokenKind 有 114 个值。
        assert_eq!(kinds.len(), 114, "TokenKind value count");
        let mut names = BTreeSet::new();
        for (name, text, tag) in &kinds {
            assert!(names.insert(name.clone()), "duplicate TokenKind: {name}");
            assert!(
                ["name=-", "name=true", "name=false", "name=null"]
                    .iter()
                    .any(|pattern| text == pattern)
                    || text.starts_with("name="),
                "unexpected TokenKind text field: {text}"
            );
            assert!(
                tag.starts_with("tag="),
                "unexpected TokenKind tag field: {tag}"
            );
        }
        for required in [
            "EOF",
            "ERROR",
            "CUSTOM",
            "IDENTIFIER",
            "ARROW",
            "MONKEYS_AT",
        ] {
            assert!(names.contains(required), "missing TokenKind: {required}");
        }
    }

    #[test]
    fn token_oracle_samples_are_well_formed() {
        let kinds = token_kinds(TOKEN_ORACLE);
        let known: BTreeSet<&str> = kinds.iter().map(|(name, _, _)| name.as_str()).collect();
        let samples = token_samples(TOKEN_ORACLE);
        assert!(
            samples.len() >= 10,
            "expected at least 10 token samples, found {}",
            samples.len()
        );
        let mut observed = BTreeSet::new();
        for (sample, tokens, truncated) in &samples {
            assert!(!tokens.is_empty(), "empty token sample: {sample}");
            assert!(
                tokens.len() <= 600,
                "sample exceeds the recorded token cap: {sample}"
            );
            if !truncated {
                assert!(tokens.len() <= 600);
            }
            for token in tokens {
                let parts: Vec<&str> = token.split(' ').collect();
                assert!(parts.len() >= 4, "malformed TOKEN line: {token}");
                assert_eq!(parts[0], "TOKEN");
                let kind = parts[1];
                assert!(
                    known.contains(kind),
                    "unknown TokenKind {kind} in sample {sample}"
                );
                observed.insert(kind);
                let pos = parts[2].parse::<i64>().expect("token pos");
                let end_pos = parts[3].parse::<i64>().expect("token endPos");
                assert!(pos <= end_pos, "inverted token range in: {token}");
            }
        }
        // 112/114:EOF 是流终止符不出现在流中,CUSTOM 不会被正常扫描产生。
        assert_eq!(observed.len(), 112, "observed TokenKind coverage");
        let missing: Vec<&str> = known
            .iter()
            .copied()
            .filter(|kind| !observed.contains(kind))
            .collect();
        assert_eq!(missing, vec!["CUSTOM", "EOF"]);
    }

    #[test]
    fn token_oracle_covers_error_tokens_and_keywords() {
        let samples = token_samples(TOKEN_ORACLE);
        let error_sample = samples
            .iter()
            .find(|(name, _, _)| name == "error-tokens")
            .map(|(_, tokens, _)| tokens)
            .expect("error-tokens sample");
        assert!(
            error_sample
                .iter()
                .any(|token| token.starts_with("TOKEN ERROR ")),
            "error-tokens sample must contain ERROR tokens"
        );
        let all_tokens = samples
            .iter()
            .find(|(name, _, _)| name == "all-tokens")
            .map(|(_, tokens, _)| tokens)
            .expect("all-tokens sample");
        let kinds: BTreeSet<&str> = all_tokens
            .iter()
            .map(|token| token.split(' ').nth(1).expect("kind field"))
            .collect();
        for keyword in [
            "ABSTRACT",
            "ASSERT",
            "BOOLEAN",
            "BREAK",
            "BYTE",
            "CASE",
            "CATCH",
            "CHAR",
            "CLASS",
            "CONST",
            "CONTINUE",
            "DEFAULT",
            "DO",
            "DOUBLE",
            "ELSE",
            "ENUM",
            "EXTENDS",
            "FINAL",
            "FINALLY",
            "FLOAT",
            "FOR",
            "GOTO",
            "IF",
            "IMPLEMENTS",
            "IMPORT",
            "INSTANCEOF",
            "INT",
            "INTERFACE",
            "LONG",
            "NATIVE",
            "NEW",
            "PACKAGE",
            "PRIVATE",
            "PROTECTED",
            "PUBLIC",
            "RETURN",
            "SHORT",
            "STATIC",
            "STRICTFP",
            "SUPER",
            "SWITCH",
            "SYNCHRONIZED",
            "THIS",
            "THROW",
            "THROWS",
            "TRANSIENT",
            "TRY",
            "VOID",
            "VOLATILE",
            "WHILE",
            "TRUE",
            "FALSE",
            "NULL",
            "UNDERSCORE",
        ] {
            assert!(
                kinds.contains(keyword),
                "all-tokens sample is missing keyword token: {keyword}"
            );
        }
        for operator in [
            "ARROW",
            "COLCOL",
            "LPAREN",
            "RPAREN",
            "LBRACE",
            "RBRACE",
            "LBRACKET",
            "RBRACKET",
            "SEMI",
            "COMMA",
            "DOT",
            "ELLIPSIS",
            "EQ",
            "GT",
            "LT",
            "BANG",
            "TILDE",
            "QUES",
            "COLON",
            "EQEQ",
            "LTEQ",
            "GTEQ",
            "BANGEQ",
            "AMPAMP",
            "BARBAR",
            "PLUSPLUS",
            "SUBSUB",
            "PLUS",
            "SUB",
            "STAR",
            "SLASH",
            "AMP",
            "BAR",
            "CARET",
            "PERCENT",
            "LTLT",
            "GTGT",
            "GTGTGT",
            "MONKEYS_AT",
        ] {
            assert!(
                kinds.contains(operator),
                "all-tokens sample is missing operator token: {operator}"
            );
        }
    }

    #[test]
    fn token_oracle_headers_match_sample_contents() {
        let samples = token_samples(TOKEN_ORACLE);
        for (name, tokens, _) in &samples {
            // 每个样本末尾的 "# tokens: N" 与该样本的 TOKEN 行数一致。
            let mut token_count = 0usize;
            let lines: Vec<&str> = TOKEN_ORACLE.lines().collect();
            let marker = format!("=== SAMPLE: {name} ===");
            let start = lines
                .iter()
                .position(|line| *line == marker)
                .expect("sample marker");
            let mut end = start + 1;
            while end < lines.len() && lines[end] != "=== END ===" {
                end += 1;
            }
            for line in &lines[start + 1..end] {
                if let Some(rest) = line.strip_prefix("# tokens: ") {
                    token_count = rest.parse::<usize>().expect("token count");
                }
            }
            assert_eq!(
                token_count,
                tokens.len(),
                "token count header mismatch in sample {name}"
            );
        }
        let observed =
            header_value(TOKEN_ORACLE, "# observed-kinds: ").expect("observed-kinds header");
        assert_eq!(observed, "112/114");
        let missing = header_value(TOKEN_ORACLE, "# missing-kinds:")
            .expect("missing-kinds header")
            .trim();
        assert_eq!(missing, "EOF CUSTOM");
    }
}

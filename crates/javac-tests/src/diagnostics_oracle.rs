//! 诊断 oracle 数据完整性测试（运行期零 JVM 依赖）。
//!
//! 用途锚点：docs/PLAN.md §7（差分测试体系）。本文件是测试基础设施，不是 javac 源码的
//! 移植文件，因此不写移植来源锚点、不写版权头。
//!
//! 数据由开发期工具 crates/javac-tests/src/bin/gen_diagnostics_oracle.rs 生成（见
//! crates/javac-tests/README.md 与数据头），入库到 data/diagnostics-oracle.txt。
//! 数据解析与还原在 [crate::diagnostics_data]；本模块只校验数据文件的结构与完整性，
//! 不启动任何 JVM。

use std::collections::BTreeSet;

use crate::diagnostics_data::{BuiltinCases, Record};

const DIAGNOSTICS_ORACLE: &str = include_str!("../data/diagnostics-oracle.txt");

fn find<'a>(records: &'a [Record], name: &str) -> &'a Record {
    records
        .iter()
        .find(|record| record.name == name)
        .unwrap_or_else(|| panic!("missing diagnostics oracle case {name:?}"))
}

fn contains_line(record: &Record, section: &str, expected: &str) {
    let text = match section {
        "stderr-default" => &record.stderr_default,
        "stderr-draw" => &record.stderr_draw,
        other => panic!("unknown section {other}"),
    };
    assert!(
        text.lines().any(|line| line == expected),
        "case {} section {} must contain a line exactly equal to {:?}, got:\n{}",
        record.name,
        section,
        expected,
        text
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::diagnostics_data::{count_draw_diagnostics, extract_draw_keys, header_value, parse};

    fn records() -> Vec<Record> {
        parse(DIAGNOSTICS_ORACLE)
    }

    #[test]
    fn metadata_is_recorded() {
        assert!(
            DIAGNOSTICS_ORACLE
                .contains("# generated-by: crates/javac-tests/src/bin/gen_diagnostics_oracle.rs")
        );
        assert_eq!(
            header_value(DIAGNOSTICS_ORACLE, "# jdk-version: "),
            Some("17.0.19")
        );
        assert_eq!(
            header_value(DIAGNOSTICS_ORACLE, "# baseline: "),
            Some("982d53c (jdk-17.0.19-ga)")
        );
        let oracle_env = header_value(DIAGNOSTICS_ORACLE, "# oracle-env: ").expect("oracle-env");
        assert!(oracle_env.contains("-encoding UTF-8"), "{oracle_env}");
        assert!(
            oracle_env.contains("-J-Dfile.encoding=UTF-8"),
            "{oracle_env}"
        );
        assert!(oracle_env.contains("-J-Duser.language=en"), "{oracle_env}");
        assert!(oracle_env.contains("-J-Duser.country=US"), "{oracle_env}");
        let command = header_value(DIAGNOSTICS_ORACLE, "# command: ").expect("command");
        assert!(command.contains("gen_diagnostics_oracle"), "{command}");
        assert!(
            header_value(DIAGNOSTICS_ORACLE, "# corpus-root: ")
                .expect("corpus-root")
                .contains("diags/examples")
        );
        assert!(header_value(DIAGNOSTICS_ORACLE, "# draw-source-names: ").is_some());

        let records = records();
        let total: usize = header_value(DIAGNOSTICS_ORACLE, "# total-cases: ")
            .expect("total-cases")
            .parse()
            .expect("total-cases number");
        assert_eq!(total, records.len());
        let total_diagnostics: usize =
            header_value(DIAGNOSTICS_ORACLE, "# total-draw-diagnostics: ")
                .expect("total-draw-diagnostics")
                .parse()
                .expect("total-draw-diagnostics number");
        assert_eq!(
            total_diagnostics,
            records
                .iter()
                .map(|record| record.draw_diagnostics)
                .sum::<usize>()
        );
        let unique: usize = header_value(DIAGNOSTICS_ORACLE, "# unique-draw-keys: ")
            .expect("unique-draw-keys")
            .parse()
            .expect("unique-draw-keys number");
        let actual_keys: BTreeSet<&str> = records
            .iter()
            .flat_map(|record| record.draw_keys.iter().map(String::as_str))
            .collect();
        assert_eq!(unique, actual_keys.len());
    }

    #[test]
    fn builtin_case_sources_are_recorded() {
        let cases = BuiltinCases::parse(include_str!("../data/diagnostics-builtin-cases.txt"));
        assert_eq!(cases.cases.len(), 35, "builtin case count");
        let mut names = std::collections::BTreeSet::new();
        for case in &cases.cases {
            assert!(
                names.insert(case.name.as_str()),
                "duplicate builtin case {}",
                case.name
            );
            assert!(
                !case.sources.is_empty(),
                "builtin case {} has no source",
                case.name
            );
            for source in &case.sources {
                assert!(
                    source.name.ends_with(".java"),
                    "{}: source name {} must end with .java",
                    case.name,
                    source.name
                );
                assert!(
                    !source.contents.is_empty(),
                    "{}: empty source {}",
                    case.name,
                    source.name
                );
                assert!(
                    source.contents.ends_with('\n'),
                    "{}: source {} must end with an LF",
                    case.name,
                    source.name
                );
            }
        }
        // 每条 builtin 数据记录都必须能找到入库源码,且 args 一致。
        let records = records();
        let mut builtin_records = 0usize;
        for record in &records {
            let Some(name) = record.source.strip_prefix("builtin:") else {
                continue;
            };
            builtin_records += 1;
            let case = cases
                .find(name)
                .unwrap_or_else(|| panic!("no builtin case source for {name}"));
            assert_eq!(
                case.args, record.args,
                "args mismatch for builtin case {name}"
            );
        }
        assert_eq!(builtin_records, cases.cases.len());
        assert_eq!(records.len(), 80, "80 = 35 builtin + 45 corpus");
        assert_eq!(records.len() - builtin_records, 45);
    }

    #[test]
    fn records_are_well_formed() {
        let records = records();
        assert!(
            records.len() >= 70,
            "expected the full builtin + corpus case set, got {}",
            records.len()
        );
        let mut names = BTreeSet::new();
        for record in &records {
            assert!(
                names.insert(record.name.as_str()),
                "duplicate case {}",
                record.name
            );
            assert!(
                record.exit_code == record.draw_exit_code,
                "case {}: default exit {} differs from draw exit {}",
                record.name,
                record.exit_code,
                record.draw_exit_code
            );
            let expected = extract_draw_keys(&record.stderr_draw);
            let actual: BTreeSet<String> = record.draw_keys.iter().cloned().collect();
            assert_eq!(
                expected, actual,
                "case {}: draw key list mismatch",
                record.name
            );
            assert_eq!(
                count_draw_diagnostics(&record.stderr_draw),
                record.draw_diagnostics,
                "case {}: draw diagnostic count mismatch",
                record.name
            );
            if record.exit_code == 0 {
                assert!(
                    !record
                        .draw_keys
                        .iter()
                        .any(|key| key.starts_with("compiler.err.")),
                    "case {} succeeded but recorded error keys {:?}",
                    record.name,
                    record.draw_keys
                );
            }
            for section in [
                &record.stdout_default,
                &record.stderr_default,
                &record.stdout_draw,
                &record.stderr_draw,
            ] {
                assert!(
                    !section.contains("C:\\") && !section.contains("C:/"),
                    "case {}: absolute Windows path was not normalized:\n{}",
                    record.name,
                    section
                );
                assert!(
                    !section.contains("\\Users\\") && !section.contains("/Users/"),
                    "case {}: absolute user path was not normalized:\n{}",
                    record.name,
                    section
                );
            }
        }
        // The success cases are still diagnostics data: they may carry warnings and notes but
        // never errors.
        let clean = find(&records, "clean/HelloWorld");
        assert_eq!(clean.exit_code, 0);
        assert!(clean.stderr_default.is_empty());
        assert!(clean.stderr_draw.is_empty());
        assert_eq!(clean.classes, vec!["HelloWorld.class".to_string()]);
    }

    #[test]
    fn sample_syntax_error_structure() {
        let records = records();
        let record = find(&records, "syntax/IllegalStartOfExpression");
        assert_eq!(record.exit_code, 1);
        assert_eq!(record.draw_keys, vec!["compiler.err.illegal.start.of.expr"]);
        assert_eq!(record.draw_diagnostics, 1);
        // -XDrawDiagnostics lines use the simple source name; the default format keeps the
        // normalized source path.
        contains_line(
            record,
            "stderr-draw",
            "IllegalStartOfExpression.java:3:17: compiler.err.illegal.start.of.expr",
        );
        contains_line(record, "stderr-draw", "1 error");
        assert!(
            record.stderr_default.contains(
                "<src>/IllegalStartOfExpression.java:3: error: illegal start of expression"
            ),
            "{}",
            record.stderr_default
        );
        assert!(
            record.stderr_default.contains("        int x = ;"),
            "{}",
            record.stderr_default
        );
        assert!(
            record
                .stderr_default
                .lines()
                .any(|line| line.trim_start().starts_with('^')),
            "default diagnostics must keep the caret line:\n{}",
            record.stderr_default
        );
        assert!(record.stderr_default.trim_end().ends_with("1 error"));

        let multiple = find(&records, "syntax/MultipleErrors");
        assert_eq!(multiple.draw_diagnostics, 2);
        assert_eq!(
            multiple.draw_keys,
            vec![
                "compiler.err.illegal.start.of.expr".to_string(),
                "compiler.err.unclosed.str.lit".to_string()
            ]
        );
        assert!(multiple.stderr_default.trim_end().ends_with("2 errors"));
    }

    #[test]
    fn sample_type_error_structure() {
        let records = records();
        let record = find(&records, "type/CannotFindSymbol");
        assert_eq!(record.exit_code, 1);
        assert_eq!(
            record.draw_keys,
            vec!["compiler.err.cant.resolve.location.args"]
        );
        assert_eq!(record.draw_diagnostics, 1);
        assert!(
            record.stderr_draw.lines().any(|line| line.starts_with(
                "CannotFindSymbol.java:3:9: compiler.err.cant.resolve.location.args: "
            )),
            "{}",
            record.stderr_draw
        );
        assert!(
            record
                .stderr_default
                .contains("<src>/CannotFindSymbol.java:3: error: cannot find symbol"),
            "{}",
            record.stderr_default
        );
        assert!(
            record
                .stderr_default
                .contains("symbol:   method unknownSymbol()"),
            "{}",
            record.stderr_default
        );
        assert!(
            record
                .stderr_default
                .contains("location: class CannotFindSymbol"),
            "{}",
            record.stderr_default
        );

        let incompatible = find(&records, "type/IncompatibleTypes");
        assert_eq!(incompatible.draw_keys, vec!["compiler.err.prob.found.req"]);
        assert!(
            incompatible
                .stderr_default
                .contains("error: incompatible types: String cannot be converted to int")
        );
    }

    #[test]
    fn lint_werror_note_and_preview_are_covered() {
        let records = records();
        let lint = find(&records, "lint/AllWarnings");
        assert_eq!(lint.exit_code, 0);
        assert_eq!(lint.draw_diagnostics, 8);
        for key in [
            "compiler.warn.has.been.deprecated",
            "compiler.warn.raw.class.use",
            "compiler.warn.unchecked.call.mbr.of.raw.type",
            "compiler.warn.empty.if",
            "compiler.warn.div.zero",
            "compiler.warn.missing.SVUID",
            "compiler.warn.possible.fall-through.into.case",
        ] {
            assert!(
                lint.draw_keys.iter().any(|actual| actual == key),
                "lint/AllWarnings must record {key}, got {:?}",
                lint.draw_keys
            );
        }

        let werror = find(&records, "werror/DeprecationAsError");
        assert_eq!(werror.exit_code, 1);
        assert_eq!(
            werror.draw_keys,
            vec![
                "compiler.err.warnings.and.werror".to_string(),
                "compiler.warn.has.been.deprecated".to_string()
            ]
        );
        assert!(
            werror
                .stderr_default
                .contains("error: warnings found and -Werror specified")
        );

        let deprecated_note = find(&records, "note/DeprecationNote");
        assert_eq!(deprecated_note.exit_code, 0);
        assert_eq!(
            deprecated_note.draw_keys,
            vec![
                "compiler.note.deprecated.filename".to_string(),
                "compiler.note.deprecated.recompile".to_string()
            ]
        );
        contains_line(
            deprecated_note,
            "stderr-draw",
            "- compiler.note.deprecated.filename: DeprecationNote.java",
        );
        assert!(
            deprecated_note
                .stderr_default
                .contains("Note: <src>/DeprecationNote.java uses or overrides a deprecated API.")
        );
        assert!(
            deprecated_note
                .stderr_default
                .contains("Note: Recompile with -Xlint:deprecation for details.")
        );

        let unchecked_note = find(&records, "note/UncheckedNote");
        assert_eq!(unchecked_note.exit_code, 0);
        assert!(
            unchecked_note
                .stderr_default
                .contains("Note: <src>/UncheckedNote.java uses unchecked or unsafe operations.")
        );
        assert!(
            unchecked_note
                .stderr_default
                .contains("Note: Recompile with -Xlint:unchecked for details.")
        );

        let preview = find(&records, "preview/RecordApiInRelease14");
        assert_eq!(preview.exit_code, 1);
        assert_eq!(preview.draw_keys, vec!["compiler.err.is.preview"]);
        assert!(
            preview
                .stderr_default
                .contains("Record is a preview API and is disabled by default.")
        );
        assert!(
            preview
                .stderr_default
                .contains("(use --enable-preview to enable preview APIs)")
        );

        let maxerrs = find(&records, "maxerrs/TooManyErrors");
        assert_eq!(maxerrs.draw_diagnostics, 1);
        assert!(maxerrs.stderr_draw.contains(
            "only showing the first 1 errors, of 2 total; use -Xmaxerrs if you would like to see more"
        ));
        assert!(maxerrs.stderr_default.contains("first type:  int"));
        assert!(maxerrs.stderr_default.contains("second type: boolean"));
    }

    #[test]
    fn stop_policy_and_release_cases_are_recorded() {
        let records = records();
        let parse_only = find(&records, "stop/ShouldStopAtParse");
        assert_eq!(parse_only.args, vec!["-XDshould-stop.at=PARSE".to_string()]);
        assert_eq!(
            parse_only.draw_keys,
            vec!["compiler.err.illegal.start.of.expr"]
        );
        let default_policy = find(&records, "stop/DefaultPolicy");
        assert!(default_policy.args.is_empty());
        // The policy is the error stop stage; the parser already failed, so both runs emit the
        // same diagnostics for this input.
        assert_eq!(default_policy.stderr_draw, parse_only.stderr_draw);
        assert_eq!(default_policy.stderr_default, parse_only.stderr_default);

        let release = find(&records, "release/RecordNotInRelease13");
        assert_eq!(
            release.args,
            vec!["--release".to_string(), "13".to_string()]
        );
        assert_eq!(
            release.draw_keys,
            vec!["compiler.err.cant.resolve.location"]
        );
        assert!(
            release
                .stderr_default
                .contains("<src>/RecordNotInRelease13.java:2: error: cannot find symbol")
        );
        assert!(release.stderr_default.contains("symbol:   class Record"));
        assert!(
            release
                .stderr_default
                .contains("location: package java.lang")
        );
    }

    #[test]
    fn draw_key_distribution_is_representative() {
        let records = records();
        let keys: BTreeSet<&str> = records
            .iter()
            .flat_map(|record| record.draw_keys.iter().map(String::as_str))
            .collect();
        assert!(keys.len() >= 30, "only {} distinct draw keys", keys.len());
        for required in [
            "compiler.err.illegal.start.of.expr",
            "compiler.err.expected",
            "compiler.err.illegal.char",
            "compiler.err.premature.eof",
            "compiler.err.cant.resolve.location",
            "compiler.err.prob.found.req",
            "compiler.err.operator.cant.be.applied.1",
            "compiler.err.unreachable.stmt",
            "compiler.err.unreported.exception.need.to.catch.or.throw",
            "compiler.err.duplicate.class",
            "compiler.err.warnings.and.werror",
            "compiler.err.is.preview",
        ] {
            assert!(
                keys.contains(required),
                "missing required diagnostic key {required}"
            );
        }
        assert!(keys.iter().any(|key| key.starts_with("compiler.warn.")));
        assert!(keys.iter().any(|key| key.starts_with("compiler.note.")));
        let failures = records
            .iter()
            .filter(|record| record.exit_code != 0)
            .count();
        assert!(failures >= 60, "only {failures} failing cases");
        let successes = records.len() - failures;
        assert!(successes >= 10, "only {successes} success cases");
    }

    #[test]
    fn corpus_cases_are_recorded() {
        let records = records();
        let corpus: Vec<&Record> = records
            .iter()
            .filter(|record| record.source.starts_with("corpus:"))
            .collect();
        assert!(corpus.len() >= 40, "only {} corpus cases", corpus.len());
        let sources: BTreeSet<&str> = corpus.iter().map(|record| record.source.as_str()).collect();
        assert!(sources.contains("corpus:diags/examples/IllegalStartOfExpr.java"));
        assert!(sources.contains("corpus:diags/examples/CantResolveLocation.java"));
        assert!(sources.contains("corpus:diags/examples/WarningAndWerror.java"));
        let mut without_overlap = Vec::new();
        for record in corpus {
            assert!(
                !record.corpus_keys.is_empty(),
                "case {} should record the example's declared // key: lines",
                record.name
            );
            let declared: BTreeSet<&str> = record.corpus_keys.iter().map(String::as_str).collect();
            let produced: BTreeSet<&str> = record.draw_keys.iter().map(String::as_str).collect();
            if declared.intersection(&produced).next().is_none() {
                without_overlap.push(record.name.clone());
            }
        }
        // corpus/MissingSemicolon declares annotation-processor diagnostics (dc.missing.semicolon,
        // note.proc.messager) and needs the example framework's processor setup; a plain javac
        // invocation reports the missing processor instead. It is kept on purpose as a
        // documented example of the framework requirement.
        assert_eq!(without_overlap, vec!["corpus/MissingSemicolon".to_string()]);
    }
}

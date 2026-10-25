//! Token-stream oracle tests (development-time JDK oracle, no JVM at test time).
//!
//! The expectations come from crates/javac-tests/data/token-oracle.txt, which
//! was produced by tools/gen-token-oracle/GenTokenOracle.java on the baseline
//! JDK 17.0.19: ScannerFactory.newScanner(text, keepDocComments=true), then
//! nextToken() until EOF, serialized as
//!   TOKEN <kind> <pos> <endPos> [name=...] [string=...] [radix=...]
//! (strings escaped by GenTokenOracle.escape).  This test rebuilds the
//! all-tokens/error-tokens sources exactly like GenTokenOracle and reads the
//! file samples from the reference tree (skipped with a printed note when the
//! reference tree is not present).

use std::rc::Rc;

use javac_strings::JavaString;

use crate::code::source::Source;
use crate::parser::lexer::Lexer;
use crate::parser::scanner_factory::ScannerFactory;
use crate::parser::tokens::{Token, TokenKind, TokenTag};
use crate::util::abstract_log::AbstractLog;
use crate::util::context::Context;
use crate::util::names::Names;

const TOKEN_ORACLE: &str = include_str!("../../../javac-tests/data/token-oracle.txt");

/// The source of the all-tokens sample (the same String.join(" ", ...) input as
/// GenTokenOracle).
const ALL_TOKENS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    "_",
    "identifier",
    "123",
    "0x1F",
    "0b101",
    "017",
    "1.5",
    "1.5e3",
    "1L",
    "1.5f",
    "1.5d",
    "0x1.8p1",
    "'c'",
    "\"str\"",
    "->",
    "::",
    "(",
    ")",
    "{",
    "}",
    "[",
    "]",
    ";",
    ",",
    ".",
    "...",
    "=",
    ">",
    "<",
    "!",
    "~",
    "?",
    ":",
    "==",
    "<=",
    ">=",
    "!=",
    "&&",
    "||",
    "++",
    "--",
    "+",
    "-",
    "*",
    "/",
    "&",
    "|",
    "^",
    "%",
    "<<",
    ">>",
    ">>>",
    "+=",
    "-=",
    "*=",
    "/=",
    "&=",
    "|=",
    "^=",
    "%=",
    "<<=",
    ">>=",
    ">>>=",
    "@",
    "// line comment\n",
    "/* block comment */",
    "/** javadoc comment */",
    "\"\"\"\n  text block\n  \"\"\"",
];

/// The source of the error-tokens sample (GenTokenOracle: "int # = ;\n" +
/// "\"unterminated\n" + "'x\n" + "\\u" + "ZZZZ\n").
const ERROR_TOKENS: &str = "int # = ;\n\"unterminated\n'x\n\\uZZZZ\n";

fn factory() -> ScannerFactory {
    let mut context = Context::new();
    // The lexer reports errors through Log; a source object is needed for the
    // diagnostics (GenTokenOracle uses the same DummySource setup).
    let log = crate::util::log::Log::instance(&mut context);
    let dummy = Rc::new(
        javac_tools::tools::simple_java_file_object::SimpleJavaFileObject::new(
            javac_tools::uri::URI::create(&JavaString::from("file:///Dummy.java")),
            javac_tools::tools::java_file_object::Kind::SOURCE,
        ),
    );
    log.borrow_mut().use_source(Some(dummy));
    let names = Rc::new(Names::new(false));
    ScannerFactory::new(&mut context, names, Source::DEFAULT)
}

fn escape_units(units: &[u16]) -> String {
    let mut out = String::new();
    for unit in units {
        match *unit {
            0x5C => out.push_str("\\\\"),
            0x0A => out.push_str("\\n"),
            0x0D => out.push_str("\\r"),
            0x09 => out.push_str("\\t"),
            _ => out.push(char::from_u32(u32::from(*unit)).unwrap_or('\u{FFFD}')),
        }
    }
    out
}

fn serialize_token(token: &Token) -> String {
    let mut line = format!(
        "TOKEN {:?} {} {}",
        token.kind, token.pos, token.end_pos
    );
    match token.kind.tag() {
        TokenTag::DEFAULT => {}
        TokenTag::NAMED => line.push_str(&format!(" name={}", token.name())),
        TokenTag::STRING | TokenTag::NUMERIC => {
            line.push_str(" string=");
            line.push_str(&escape_units(token.string_val().as_units()));
            if token.kind.tag() == TokenTag::NUMERIC {
                let radix = token.radix();
                if radix != 0 {
                    line.push_str(&format!(" radix={}", radix));
                }
            }
        }
    }
    line
}

const MAX_TOKENS: usize = 600;

fn scan_token_lines(text: &str, keep_doc_comments: bool) -> Vec<String> {
    let factory = factory();
    let mut scanner = factory.new_scanner(&JavaString::from(text), keep_doc_comments);
    let mut lines = Vec::new();
    scanner.next_token();
    while scanner.token().kind != TokenKind::EOF {
        if lines.len() >= MAX_TOKENS {
            break;
        }
        lines.push(serialize_token(scanner.token()));
        scanner.next_token();
    }
    lines
}

struct OracleSample {
    name: String,
    lines: Vec<String>,
    truncated: bool,
}

fn parse_samples(text: &str) -> Vec<OracleSample> {
    let mut samples: Vec<OracleSample> = Vec::new();
    let mut current: Option<OracleSample> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("=== SAMPLE: ") {
            let name = rest.strip_suffix(" ===").expect("sample marker").to_string();
            current = Some(OracleSample { name, lines: Vec::new(), truncated: false });
        } else if line == "=== END ===" {
            if let Some(sample) = current.take() {
                samples.push(sample);
            }
        } else if line.starts_with("# truncated at ") {
            if let Some(sample) = current.as_mut() {
                sample.truncated = true;
            }
        } else if line.starts_with("TOKEN ")
            && let Some(sample) = current.as_mut()
        {
            sample.lines.push(line.to_string());
        }
    }
    samples
}

fn sample_source_path(name: &str) -> Option<String> {
    if name.starts_with("samples/") {
        Some(format!("tools/gen-tree-oracle/{}", name))
    } else {
        name.strip_prefix("javac/")
            .map(|rest| format!("reference/jdk17u/test/langtools/tools/javac/{}", rest))
    }
}

#[test]
fn token_kinds_match_the_oracle_enumeration() {
    let mut expected = Vec::new();
    for line in TOKEN_ORACLE.lines() {
        let Some(rest) = line.strip_prefix("KIND ") else {
            continue;
        };
        let parts: Vec<&str> = rest.split(' ').collect();
        expected.push((parts[0].to_string(), parts[1], parts[2]));
    }
    assert_eq!(expected.len(), TokenKind::values().len(), "TokenKind count");
    for (kind, (variant, name, tag)) in TokenKind::values().iter().zip(expected.iter()) {
        assert_eq!(format!("{:?}", kind), *variant, "TokenKind order/name {}", variant);
        // GenTokenOracle writes "name=-" both for a null name and for the SUB
        // token text "-": the recorded data cannot distinguish them, so SUB is
        // restored from Tokens.java (reported as an oracle data-format
        // limitation).
        let expected_name = if *name == "name=-" {
            if *variant == "SUB" {
                Some("-")
            } else {
                None
            }
        } else {
            Some(&name["name=".len()..])
        };
        assert_eq!(kind.name(), expected_name, "name of {}", variant);
        let expected_tag = &tag["tag=".len()..];
        assert_eq!(format!("{:?}", kind.tag()), expected_tag, "tag of {}", variant);
    }
    // The oracle records 112/114 kinds observed; EOF is the loop terminator and
    // CUSTOM is never produced by the scanner.
    assert!(TOKEN_ORACLE.contains("# observed-kinds: 112/114"));
    assert!(TOKEN_ORACLE.contains("# missing-kinds: EOF CUSTOM"));
}

#[test]
fn token_streams_match_the_oracle() {
    let samples = parse_samples(TOKEN_ORACLE);
    assert!(!samples.is_empty(), "token oracle samples");
    let mut compared = 0;
    let mut skipped = Vec::new();
    for sample in &samples {
        let text: String = if sample.name == "all-tokens" {
            ALL_TOKENS.join(" ")
        } else if sample.name == "error-tokens" {
            ERROR_TOKENS.to_string()
        } else {
            let Some(path) = sample_source_path(&sample.name) else {
                skipped.push(sample.name.clone());
                continue;
            };
            match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => {
                    skipped.push(sample.name.clone());
                    continue;
                }
            }
        };
        let actual = scan_token_lines(&text, true);
        if actual.len() != sample.lines.len() {
            eprintln!(
                "sample {}: actual {} tokens, expected {}",
                sample.name,
                actual.len(),
                sample.lines.len()
            );
            let first_diff = actual
                .iter()
                .zip(sample.lines.iter())
                .position(|(left, right)| left != right)
                .unwrap_or(actual.len().min(sample.lines.len()));
            let start = first_diff.saturating_sub(5);
            let end = (first_diff + 8).min(actual.len()).min(sample.lines.len());
            for (index, (actual_line, expected_line)) in actual
                .iter()
                .zip(sample.lines.iter())
                .enumerate()
                .skip(start)
                .take(end - start)
            {
                eprintln!("A{index} {actual_line}\nE{index} {expected_line}");
            }
        }
        assert_eq!(
            actual.len(),
            sample.lines.len(),
            "token count for sample {} (truncated: {})",
            sample.name,
            sample.truncated
        );
        for (index, (actual_line, expected_line)) in
            actual.iter().zip(sample.lines.iter()).enumerate()
        {
            if actual_line != expected_line {
                let start = index.saturating_sub(5);
                let end = (index + 5).min(actual.len()).min(sample.lines.len());
                for (position, (actual_line, expected_line)) in actual
                    .iter()
                    .zip(sample.lines.iter())
                    .enumerate()
                    .skip(start)
                    .take(end - start)
                {
                    eprintln!("A{position} {actual_line}\nE{position} {expected_line}");
                }
                assert_eq!(
                    actual_line, expected_line,
                    "sample {} token {}",
                    sample.name, index
                );
            }
        }
        compared += 1;
    }
    assert!(compared >= 2, "at least the built-in samples must be compared");
    if !skipped.is_empty() {
        eprintln!(
            "skipped file samples (reference tree not present): {}",
            skipped.join(", ")
        );
    }
}

#[test]
fn doc_comment_extraction_does_not_change_the_token_stream() {
    // The oracle scans with keepDocComments=true (JavadocTokenizer); the plain
    // JavaTokenizer must produce the same token stream.
    let all_tokens_text = ALL_TOKENS.join(" ");
    let with_docs = scan_token_lines(&all_tokens_text, true);
    let without_docs = scan_token_lines(&all_tokens_text, false);
    assert_eq!(with_docs, without_docs);
    let error_text = ERROR_TOKENS.to_string();
    assert_eq!(
        scan_token_lines(&error_text, true),
        scan_token_lines(&error_text, false)
    );
}

#[test]
fn tokens_lookup_and_scanner_helpers() {
    let mut context = Context::new();
    let names = Rc::new(Names::new(false));
    let factory = ScannerFactory::new(&mut context, names, Source::DEFAULT);
    assert_eq!(factory.tokens.lookup_kind_str("assert"), TokenKind::ASSERT);
    assert_eq!(factory.tokens.lookup_kind_str("not_a_keyword"), TokenKind::IDENTIFIER);
    assert_eq!(factory.tokens.lookup_kind_str("_"), TokenKind::UNDERSCORE);

    let mut scanner = factory.new_scanner(&JavaString::from("class A {}"), true);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::CLASS);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::IDENTIFIER);
    assert_eq!(scanner.token().name().to_string(), "A");
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::LBRACE);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::RBRACE);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::EOF);
    assert_eq!(scanner.prev_token().kind, TokenKind::RBRACE);

    // Lookahead beyond the current token.
    let mut scanner = factory.new_scanner(&JavaString::from("int x;"), true);
    scanner.next_token();
    assert_eq!(scanner.lookahead_token(1).kind, TokenKind::IDENTIFIER);
    assert_eq!(scanner.lookahead_token(2).kind, TokenKind::SEMI);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::IDENTIFIER);

    // split() of '>>>'/'>>='.
    let mut scanner = factory.new_scanner(&JavaString::from("a >>> b"), true);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::IDENTIFIER);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::GTGTGT);
    let split = scanner.split();
    assert_eq!(split.kind, TokenKind::GT);
    assert_eq!(scanner.token().kind, TokenKind::GTGT);
}

#[test]
fn lexical_error_recovery_matches_the_oracle() {
    // The error-tokens sample of the oracle exercises the recovery paths: an
    // invalid character, an unterminated string literal, an unterminated char
    // literal and an invalid unicode escape.  Reference lines (recorded by
    // GenTokenOracle):
    let expected = [
        "TOKEN INT 0 3 name=int",
        "TOKEN ERROR 4 5",
        "TOKEN EQ 6 7",
        "TOKEN SEMI 8 9",
        "TOKEN ERROR 10 23",
        "TOKEN ERROR 24 26",
        "TOKEN IDENTIFIER 29 33 name=ZZZZ",
    ];
    let actual = scan_token_lines(ERROR_TOKENS, true);
    assert_eq!(actual, expected);
    // The invalid character is reported at pos 4, the unterminated string at
    // pos 10, the unterminated char literal at pos 24, and the invalid unicode
    // escape is consumed by the reader (no extra token).
    let factory = factory();
    let mut scanner = factory.new_scanner(&JavaString::from(ERROR_TOKENS), true);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::INT);
    scanner.next_token();
    assert_eq!(scanner.token().kind, TokenKind::ERROR);
    assert_eq!(scanner.token().pos, 4);
}

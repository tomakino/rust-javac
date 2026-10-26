// Full-table oracle test for the javac-unicode data tables (runtime: no JVM).
//
// tools/gen-unicode/GenUnicode.java (run once against the baseline JDK 17 at
// development time) writes tests/data/char-oracle.txt: for each ported
// java.lang.Character predicate, the FNV-1a 64 hash of the predicate over every
// code point 0x000000..=0x10FFFF (one hash bit per code point). This test
// re-computes the hashes from the ported tables, so a single changed code point
// anywhere in the range fails the test.

const ORACLE: &str = include_str!("data/char-oracle.txt");

/// FNV-1a 64 over one bit per code point (false -> 0, true -> 1).
fn full_table_hash(mut predicate: impl FnMut(i32) -> bool) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for code_point in 0..=0x10FFFF {
        hash ^= if predicate(code_point) { 1 } else { 0 };
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn expected(name: &str) -> u64 {
    for line in ORACLE.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&format!("{name} ")) {
            let hex = rest.trim_start_matches("0x");
            return u64::from_str_radix(hex, 16).expect("oracle hash");
        }
    }
    panic!("oracle entry missing for {name}");
}

#[test]
fn full_table_matches_the_development_time_oracle() {
    assert_eq!(
        full_table_hash(javac_unicode::is_java_identifier_start),
        expected("JAVA_ID_START"),
        "isJavaIdentifierStart over 0x000000..=0x10FFFF"
    );
    assert_eq!(
        full_table_hash(javac_unicode::is_java_identifier_part),
        expected("JAVA_ID_PART"),
        "isJavaIdentifierPart over 0x000000..=0x10FFFF"
    );
    assert_eq!(
        full_table_hash(javac_unicode::is_unicode_identifier_start),
        expected("UNICODE_ID_START"),
        "isUnicodeIdentifierStart over 0x000000..=0x10FFFF"
    );
    assert_eq!(
        full_table_hash(javac_unicode::is_unicode_identifier_part),
        expected("UNICODE_ID_PART"),
        "isUnicodeIdentifierPart over 0x000000..=0x10FFFF"
    );
    assert_eq!(
        full_table_hash(javac_unicode::is_whitespace),
        expected("WHITESPACE"),
        "isWhitespace over 0x000000..=0x10FFFF"
    );
    assert_eq!(
        full_table_hash(javac_unicode::is_identifier_ignorable),
        expected("IDENTIFIER_IGNORABLE"),
        "isIdentifierIgnorable over 0x000000..=0x10FFFF"
    );
}

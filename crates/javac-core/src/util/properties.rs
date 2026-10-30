//! java.util.Properties.load semantics for embedded .properties resources.
//!
//! Adapted from xtask/src/properties.rs (generator-side parser); the runtime
//! copies it here so localized bundles can be embedded as raw text.

use std::collections::BTreeMap;

/// Parses Java .properties text: line continuations, '=' / ':' / whitespace
/// separators, backslash escapes and \uXXXX escapes (including surrogate
/// pairs).
pub fn parse_properties(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        // Logical line: a natural line ending with an odd number of
        // backslashes continues on the next line; the next line's leading
        // whitespace is skipped and the trailing backslash is dropped.
        let mut logical = String::from(trimmed);
        loop {
            let mut backslashes = 0usize;
            for c in logical.chars().rev() {
                if c == '\\' {
                    backslashes += 1;
                } else {
                    break;
                }
            }
            if backslashes % 2 == 0 {
                break;
            }
            logical.pop();
            match lines.next() {
                Some(next) => logical.push_str(next.trim_start()),
                None => break,
            }
        }

        let mut chars = logical.chars().peekable();
        let mut key = String::new();
        loop {
            match chars.peek().copied() {
                None | Some('=') | Some(':') | Some(' ') | Some('\t') | Some('\r') | Some('\n') => {
                    break;
                }
                Some('\\') => {
                    chars.next();
                    key.push_str(&unescape(&mut chars));
                }
                Some(c) => {
                    chars.next();
                    key.push(c);
                }
            }
        }
        // Skip whitespace before the separator and the separator itself.
        while matches!(
            chars.peek().copied(),
            Some(' ') | Some('\t') | Some('\r') | Some('\n')
        ) {
            chars.next();
        }
        if matches!(chars.peek().copied(), Some('=') | Some(':')) {
            chars.next();
        }
        // Skip leading whitespace of the value; trailing whitespace is kept.
        while matches!(
            chars.peek().copied(),
            Some(' ') | Some('\t') | Some('\r') | Some('\n')
        ) {
            chars.next();
        }
        let mut value = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => value.push_str(&unescape(&mut chars)),
                _ => value.push(c),
            }
        }
        map.insert(key, value);
    }
    map
}

fn unescape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    match chars.next() {
        Some('t') => "\t".to_string(),
        Some('n') => "\n".to_string(),
        Some('r') => "\r".to_string(),
        Some('f') => "\u{c}".to_string(),
        Some('u') => {
            let Some(hi) = read_hex4(chars) else {
                return "\u{fffd}".to_string();
            };
            if (0xD800..=0xDBFF).contains(&hi) {
                // Try to combine a UTF-16 surrogate pair: \uD83D\uDE00.
                let mut probe = chars.clone();
                if probe.next() == Some('\\')
                    && probe.next() == Some('u')
                    && let Some(lo) = read_hex4(&mut probe)
                    && (0xDC00..=0xDFFF).contains(&lo)
                {
                    *chars = probe;
                    let code = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                    return char::from_u32(code)
                        .map(String::from)
                        .unwrap_or_else(|| "\u{fffd}".to_string());
                }
                return "\u{fffd}".to_string();
            }
            char::from_u32(hi)
                .map(String::from)
                .unwrap_or_else(|| "\u{fffd}".to_string())
        }
        Some(c) => c.to_string(),
        None => " ".to_string(),
    }
}

fn read_hex4(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<u32> {
    let mut hex = String::new();
    for _ in 0..4 {
        hex.push(chars.next()?);
    }
    u32::from_str_radix(&hex, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> BTreeMap<String, String> {
        parse_properties(text)
    }

    fn get<'a>(map: &'a BTreeMap<String, String>, key: &str) -> &'a str {
        map.get(key).map(String::as_str).unwrap_or_else(|| panic!("missing {key}"))
    }

    #[test]
    fn continuation_joins_and_strips_indent() {
        let map = parse("compiler.err.x=\\\n    unreachable statement\n");
        assert_eq!(get(&map, "compiler.err.x"), "unreachable statement");
    }

    #[test]
    fn multi_line_continuation() {
        let map = parse("k=\\\n    one\\\n    two\\\n    three\n");
        assert_eq!(get(&map, "k"), "onetwothree");
    }

    #[test]
    fn even_backslashes_do_not_continue() {
        let map = parse("k=x\\\\\nnext=y\n");
        assert_eq!(get(&map, "k"), "x\\");
        assert_eq!(get(&map, "next"), "y");
    }

    #[test]
    fn trailing_backslash_at_eof_is_dropped() {
        let map = parse("k=x\\");
        assert_eq!(get(&map, "k"), "x");
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let map = parse("# comment\n! bang\n   # indented\n\n   \n k=v\n");
        assert_eq!(map.len(), 1);
        assert_eq!(get(&map, "k"), "v");
    }

    #[test]
    fn separators_equals_colon_and_whitespace() {
        let map = parse("a=1\nb:2\nc 3\nd = 4\ne\t: 5\n");
        assert_eq!(get(&map, "a"), "1");
        assert_eq!(get(&map, "b"), "2");
        assert_eq!(get(&map, "c"), "3");
        assert_eq!(get(&map, "d"), "4");
        assert_eq!(get(&map, "e"), "5");
    }

    #[test]
    fn escaped_separators_stay_in_key() {
        let map = parse("a\\=b=c\n");
        assert_eq!(get(&map, "a=b"), "c");
    }

    #[test]
    fn escapes_are_decoded() {
        let map = parse("k=\\t\\n\\r\\f\\\\\\'\\\"\n");
        assert_eq!(get(&map, "k"), "\t\n\r\u{c}\\'\"");
    }

    #[test]
    fn unicode_escapes_are_decoded() {
        let map = parse("k=\\u0041\\u4E2D\n");
        assert_eq!(get(&map, "k"), "A中");
    }

    #[test]
    fn surrogate_pairs_combine() {
        let map = parse("k=\\uD83D\\uDE00\n");
        assert_eq!(get(&map, "k"), "😀");
    }

    #[test]
    fn lone_surrogate_becomes_replacement_char() {
        let map = parse("k=\\uD83D\n");
        assert_eq!(get(&map, "k"), "\u{fffd}");
    }

    #[test]
    fn leading_whitespace_trimmed_trailing_kept() {
        let map = parse("k=  x  \n");
        assert_eq!(get(&map, "k"), "x  ");
    }

    #[test]
    fn duplicate_keys_last_wins() {
        let map = parse("k=1\nk=2\n");
        assert_eq!(get(&map, "k"), "2");
    }

    #[test]
    fn empty_values() {
        let map = parse("a=\nb\nc=\n");
        assert_eq!(get(&map, "a"), "");
        assert_eq!(get(&map, "b"), "");
        assert_eq!(get(&map, "c"), "");
    }

    #[test]
    fn crlf_lines_are_supported() {
        let map = parse("a=1\r\nb=2\r\n");
        assert_eq!(get(&map, "a"), "1");
        assert_eq!(get(&map, "b"), "2");
    }

    #[test]
    fn escaped_backslash_before_newline_keeps_value() {
        let map = parse("k=a\\\\\n");
        assert_eq!(get(&map, "k"), "a\\");
    }
}

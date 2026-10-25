//! Java `.properties` 解析(java.util.Properties.load 语义)。
//!
//! 来源锚点:make/langtools/tools/propertiesparser/parser/MessageFile.java @ 982d53c
//! (jdk-17.0.19-ga)使用 java.util.Properties 读取资源文件;本文件是生成器的适配实现
//! (不是移植文件),支持行续接、'=' / ':' 分隔、反斜杠转义与 \uXXXX 转义。

use std::collections::BTreeMap;

/// 按 java.util.Properties.load 语义解析文本,返回按 key 排序的键值表。
pub(crate) fn parse_properties(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
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
            if backslashes.is_multiple_of(2) {
                break;
            }
            logical.pop();
            match lines.next() {
                Some(next) => logical.push_str(next.trim_start()),
                None => break,
            }
        }
        let mut key = String::new();
        let mut value = String::new();
        let mut chars = logical.chars().peekable();
        let mut in_value = false;
        while let Some(c) = chars.next() {
            if !in_value {
                match c {
                    '\\' => key.push(unescape(&mut chars)),
                    '=' | ':' => in_value = true,
                    ' ' | '\t' | '\r' | '\n' => in_value = true,
                    _ => key.push(c),
                }
            } else {
                match c {
                    '\\' => value.push(unescape(&mut chars)),
                    _ => value.push(c),
                }
            }
        }
        map.insert(key, value.trim_start().to_string());
    }
    map
}

fn unescape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> char {
    match chars.next() {
        Some('t') => '\t',
        Some('n') => '\n',
        Some('r') => '\r',
        Some('f') => '\u{c}',
        Some('u') => {
            let mut hex = String::new();
            for _ in 0..4 {
                if let Some(c) = chars.next() {
                    hex.push(c);
                }
            }
            u32::from_str_radix(&hex, 16)
                .ok()
                .and_then(char::from_u32)
                .unwrap_or('\u{fffd}')
        }
        Some(c) => c,
        None => ' ',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_continuations_escapes_and_separators() {
        let text = "# comment\nkey.one=value one\nkey.two=\\\n    continued value\nkey.three:colon\n";
        let map = parse_properties(text);
        assert_eq!(map.get("key.one").map(String::as_str), Some("value one"));
        assert_eq!(map.get("key.two").map(String::as_str), Some("continued value"));
        assert_eq!(map.get("key.three").map(String::as_str), Some("colon"));
    }

    #[test]
    fn parses_unicode_and_escaped_quotes() {
        let text = "k=v\\u0041\\t\\'quoted\\'\n";
        let map = parse_properties(text);
        assert_eq!(map.get("k").map(String::as_str), Some("vA\t'quoted'"));
    }
}

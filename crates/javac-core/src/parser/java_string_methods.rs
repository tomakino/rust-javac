// PORTING ADAPTER (not a 1:1 port of a javac file)
//
// Adapter for: the java.lang.String methods that JavaTokenizer depends on
// (String.stripIndent / String.translateEscapes / String.lines /
// String.isBlank / indexOfNonWhitespace / lastIndexOfNonWhitespace) because the
// port has no java.lang.String class. The implementations are 1:1 with the
// JDK 17.0.19 java.base/java/lang/String.java algorithms (stripIndent at
// String.java:3832, outdent at :3852, indent at :3718, translateEscapes at
// :3955, lines/indexOfNonWhitespace helpers).
//
// Java: Character.isWhitespace is the ported javac_unicode::is_whitespace
// (JDK 17 java.lang.Character data; excludes U+00A0/U+2007/U+202F like the JDK).

//! java.lang.String helpers used by the tokenizer.

use javac_strings::JavaString;

/// Java: Character.isWhitespace(char) — javac-unicode ported JDK 17 table.
pub(crate) fn is_java_whitespace(unit: u16) -> bool {
    javac_unicode::is_whitespace(i32::from(unit))
}

/// Java: String.lines() — split at \n, \r, \r\n; no trailing empty line.
pub(crate) fn lines(text: &JavaString) -> Vec<JavaString> {
    let units = text.as_units();
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    while index < units.len() {
        let unit = units[index];
        if unit == '\r' as u16 {
            result.push(JavaString::from_units(units[start..index].to_vec()));
            index += 1;
            if index < units.len() && units[index] == '\n' as u16 {
                index += 1;
            }
            start = index;
        } else if unit == '\n' as u16 {
            result.push(JavaString::from_units(units[start..index].to_vec()));
            index += 1;
            start = index;
        } else {
            index += 1;
        }
    }
    if start < units.len() {
        result.push(JavaString::from_units(units[start..].to_vec()));
    }
    result
}

/// Java: String.indexOfNonWhitespace() — the length when all whitespace.
pub(crate) fn index_of_non_whitespace(text: &JavaString) -> i32 {
    let units = text.as_units();
    for (index, unit) in units.iter().enumerate() {
        if !is_java_whitespace(*unit) {
            return index as i32;
        }
    }
    units.len() as i32
}

/// Java: String.lastIndexOfNonWhitespace() — one past the last non-whitespace
/// character, zero when the line is blank.
pub(crate) fn last_index_of_non_whitespace(text: &JavaString) -> i32 {
    let units = text.as_units();
    for index in (0..units.len()).rev() {
        if !is_java_whitespace(units[index]) {
            return (index + 1) as i32;
        }
    }
    0
}

/// Java: String.isBlank().
pub(crate) fn is_blank(text: &JavaString) -> bool {
    text.as_units().iter().all(|unit| is_java_whitespace(*unit))
}

/// Java: String.stripIndent() (JDK 17.0.19 String.java:3832).
pub(crate) fn strip_indent(text: &JavaString) -> JavaString {
    let units = text.as_units();
    let length = units.len();
    if length == 0 {
        return JavaString::new();
    }
    let last_char = units[length - 1];
    let opt_out = last_char == '\n' as u16 || last_char == '\r' as u16;
    let split = lines(text);
    let outdent = if opt_out { 0 } else { outdent(&split) };
    let mut result: Vec<u16> = Vec::new();
    for (index, line) in split.iter().enumerate() {
        if index > 0 {
            result.push('\n' as u16);
        }
        let first_non_whitespace = index_of_non_whitespace(line);
        let last_non_whitespace = last_index_of_non_whitespace(line);
        let incidental_whitespace = outdent.min(first_non_whitespace);
        if first_non_whitespace > last_non_whitespace {
            continue;
        }
        result.extend_from_slice(
            &line.as_units()[incidental_whitespace as usize..last_non_whitespace as usize],
        );
    }
    if opt_out {
        result.push('\n' as u16);
    }
    JavaString::from_units(result)
}

/// Java: String.outdent(List<String>) (JDK 17.0.19 String.java:3852).
fn outdent(lines: &[JavaString]) -> i32 {
    let mut outdent = i32::MAX;
    for line in lines {
        let leading_whitespace = index_of_non_whitespace(line);
        if leading_whitespace != line.len() as i32 {
            outdent = outdent.min(leading_whitespace);
        }
    }
    let last_line = &lines[lines.len() - 1];
    if is_blank(last_line) {
        outdent = outdent.min(last_line.len() as i32);
    }
    outdent
}

/// Java: String.translateEscapes() (JDK 17.0.19 String.java:3955).
///
/// Java throws IllegalArgumentException for a malformed escape sequence and
/// JavaTokenizer catches/ignores it; the port reports Err(message) for the
/// callers that ignore the result.
pub(crate) fn translate_escapes(text: &JavaString) -> Result<JavaString, String> {
    let chars = text.as_units();
    let length = chars.len();
    if length == 0 {
        return Ok(JavaString::new());
    }
    let mut from = 0usize;
    let mut result: Vec<u16> = Vec::with_capacity(length);
    while from < length {
        let mut ch = chars[from];
        from += 1;
        if ch == '\\' as u16 {
            ch = if from < length {
                let next = chars[from];
                from += 1;
                next
            } else {
                0
            };
            match char::from_u32(u32::from(ch)) {
                Some('b') => ch = 0x08,
                Some('f') => ch = 0x0C,
                Some('n') => ch = 0x0A,
                Some('r') => ch = 0x0D,
                Some('s') => ch = 0x20,
                Some('t') => ch = 0x09,
                Some('\'') | Some('"') | Some('\\') => {}
                Some(digit @ '0'..='7') => {
                    let digit_value = digit as u32 - '0' as u32;
                    let limit = (from + if digit_value <= 3 { 2 } else { 1 }).min(length);
                    let mut code = digit_value as i32;
                    while from < limit {
                        let next = chars[from];
                        if !(b'0' as u16..=b'7' as u16).contains(&next) {
                            break;
                        }
                        from += 1;
                        code = (code << 3) | i32::from(next - '0' as u16);
                    }
                    ch = code as u16;
                }
                Some('\n') => continue,
                Some('\r') => {
                    if from < length && chars[from] == '\n' as u16 {
                        from += 1;
                    }
                    continue;
                }
                _ => {
                    return Err(format!(
                        "Invalid escape sequence: \\u{:04X}",
                        ch
                    ));
                }
            }
        }
        result.push(ch);
    }
    Ok(JavaString::from_units(result))
}

#[cfg(test)]
mod unicode_classification_tests {
    /// Character.isWhitespace over the BMP must match the JDK 17 table (the
    /// string helpers consume UTF-16 units).
    #[test]
    fn whitespace_uses_the_jdk_table() {
        for unit in 0u16..=0xFFFF {
            assert_eq!(
                super::is_java_whitespace(unit),
                javac_unicode::is_whitespace(i32::from(unit)),
                "isWhitespace {unit:#x}"
            );
        }
    }
}

// Adapter for: java.text.MessageFormat (util/message_format.rs)
//
// Why not 1:1: java.text.MessageFormat is a JDK API (java.text) with pattern
// parsing, ChoiceFormat, DecimalFormat and locale-aware formats. javac only
// uses a small, fixed subset of it for diagnostic message templates. This
// adapter implements exactly the subset that the upstream resources actually
// use, verified by scanning all message templates (compiler.properties,
// launcher.properties, javac.properties, ct.properties, legacy.properties and
// their zh_CN/ja variants):
// - argument placeholders {n} (ArgumentIndex with optional leading zeros)
// - single-quote escaping / quoting: '' -> ', a pair of single quotes starts
// and ends a quoted string where { and } are literal; an unterminated quote
// runs to the end of the pattern
// - out-of-range argument indexes are left as-is in the output (Java
// MessageFormat behaviour, e.g. {5} with two arguments stays "{5}")
// - IllegalArgument-equivalent patterns (unclosed brace, non-numeric index,
// negative index, index overflow, incomplete sub-format) panic, matching
// Java's IllegalArgumentException (PORTING_RULES section 6).
//
// REGISTERED: number/date/time/choice sub-formats are not implemented
// ({n,number}, {n,choice,...}) - a scan of all javac message resources found
// zero uses of any sub-format; a sub-format in a pattern panics with a clear
// message instead of silently formatting.
//
// Oracle: the test module embeds data produced by a development-time Java
// program running the JDK 17 MessageFormat (recorded from the baseline JDK
// 17.0.19); the tests themselves never start a JVM.

/// Formats a message pattern with already-rendered argument strings.
///
/// Java: java.text.MessageFormat.format(String pattern, Object... arguments)
/// (javac usage subset; see the adapter note above).
pub fn format(pattern: &str, arguments: &[String]) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut index = 0usize;
    while index < chars.len() {
        let c = chars[index];
        if c == '\'' {
            // Quote handling (Java MessageFormat): '' is one literal quote;
            // a single quote starts a quoted string where braces are literal.
            if index + 1 < chars.len() && chars[index + 1] == '\'' {
                out.push('\'');
                index += 2;
                continue;
            }
            index += 1;
            while index < chars.len() {
                if chars[index] == '\'' {
                    if index + 1 < chars.len() && chars[index + 1] == '\'' {
                        out.push('\'');
                        index += 2;
                        continue;
                    }
                    index += 1;
                    break;
                }
                out.push(chars[index]);
                index += 1;
            }
        } else if c == '{' {
            let (rendered, next) = render_placeholder(&chars, index, arguments);
            out.push_str(&rendered);
            index = next;
        } else if c == '}' {
            // Java MessageFormat: a lone '}' is literal.
            out.push('}');
            index += 1;
        } else {
            out.push(c);
            index += 1;
        }
    }
    out
}

/// Renders {n} at chars[open]; returns the rendered text and the next index.
fn render_placeholder(chars: &[char], open: usize, arguments: &[String]) -> (String, usize) {
    let mut index = open + 1;
    let mut digits = String::new();
    while index < chars.len() && chars[index].is_ascii_digit() {
        digits.push(chars[index]);
        index += 1;
    }
    if digits.is_empty() {
        panic!("MessageFormat: invalid argument placeholder at index {open}");
    }
    if index >= chars.len() {
        panic!("MessageFormat: unterminated argument placeholder");
    }
    match chars[index] {
        '}' => {
            // Java MessageFormat parses the index as a Java int.
            let parsed = digits
                .parse::<i32>()
                .unwrap_or_else(|_| panic!("MessageFormat: argument index out of range: {digits}"))
                as usize;
            let text: String = chars[open..=index].iter().collect();
            index += 1;
            if parsed < arguments.len() {
                (arguments[parsed].clone(), index)
            } else {
                // Java MessageFormat leaves out-of-range indexes as written.
                (text, index)
            }
        }
        ',' => {
            panic!(
                "MessageFormat: sub-formats are not used by javac message resources and are not supported"
            );
        }
        _ => panic!("MessageFormat: malformed argument placeholder"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Development-time oracle data: JDK 17.0.19 java.text.MessageFormat.
    /// Format: (pattern, argument set, expected output).
    const ORACLE: &[(&str, &[&str], &str)] = &[
        ("", &[], ""),
        ("no args", &[], "no args"),
        ("{0}", &["A", "B", "C"], "A"),
        ("{0} {1}", &["A", "B", "C"], "A B"),
        ("x{1}y{0}z", &["A", "B", "C"], "xByAz"),
        ("''", &["A", "B", "C"], "'"),
        ("''''", &["A", "B", "C"], "''"),
        ("'''", &["A", "B", "C"], "'"),
        ("'{0}'", &["A", "B", "C"], "{0}"),
        ("''{0}''", &["A", "B", "C"], "'A'"),
        ("'a''b'", &["A", "B", "C"], "a'b"),
        ("it''s {0}", &["A", "B", "C"], "it's A"),
        ("x'{'y", &["A", "B", "C"], "x{y"),
        ("'abc", &["A", "B", "C"], "abc"),
        ("abc'd", &["A", "B", "C"], "abcd"),
        ("{0}{0}", &["A", "B", "C"], "AA"),
        ("{00}", &["A", "B", "C"], "A"),
        ("{0}x", &["first", "second"], "firstx"),
        ("A{0}B{1}C", &["first", "second"], "AfirstBsecondC"),
        ("}{0}", &["A", "B", "C"], "}A"),
        ("}", &["A", "B", "C"], "}"),
        ("{5}", &["first", "second"], "{5}"),
        (
            "annotation @{0} is missing a default value for the element ''{1}''",
            &["@Ann", "value"],
            "annotation @@Ann is missing a default value for the element 'value'",
        ),
        ("cannot assign to ''this''", &["A", "B", "C"], "cannot assign to 'this'"),
        ("x{0}y''z", &["A", "B", "C"], "xAy'z"),
        ("'{0}'{1}", &["A", "B", "C"], "{0}B"),
        ("a''b", &["A", "B", "C"], "a'b"),
        ("''''{0}", &["A", "B", "C"], "''A"),
        ("{0}''", &["A", "B", "C"], "A'"),
        ("no {0} args {1} end", &["X"], "no X args {1} end"),
        ("'{''}'", &["A", "B", "C"], "{'}"),
    ];

    fn owned(arguments: &[&str]) -> Vec<String> {
        arguments.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn matches_jdk_messageformat_oracle() {
        for (pattern, arguments, expected) in ORACLE {
            let actual = format(pattern, &owned(arguments));
            assert_eq!(&actual, expected, "pattern: {pattern:?}");
        }
    }

    #[test]
    fn leaves_out_of_range_indexes_as_written() {
        let arguments = owned(&["only"]);
        assert_eq!(format("{0} {3}", &arguments), "only {3}");
        // Java keeps the written form (leading zeros included).
        assert_eq!(format("{007}", &arguments), "{007}");
    }

    #[test]
    fn argument_text_is_not_reparsed() {
        let arguments = owned(&["a'b", "x{0}", "c'd"]);
        assert_eq!(format("{0}/{1}/{2}", &arguments), "a'b/x{0}/c'd");
        assert_eq!(format("''{0}''", &arguments), "'a'b'");
    }

    #[test]
    fn invalid_patterns_raise_illegal_argument_equivalent_panics() {
        let arguments = owned(&["A", "B", "C"]);
        for pattern in ["{", "{a}", "{0,", "{ 0 }", "{-1}", "{999999999999}"] {
            let result = std::panic::catch_unwind(|| format(pattern, &arguments));
            assert!(result.is_err(), "pattern should panic: {pattern:?}");
        }
    }

    #[test]
    fn subformats_are_not_used_by_javac_resources() {
        // Registered limitation: javac resource scan found zero sub-format uses.
        let arguments = owned(&["5"]);
        let result = std::panic::catch_unwind(|| format("{0,number}", &arguments));
        assert!(result.is_err());
        let result = std::panic::catch_unwind(|| format("{0,choice,0#zero|1#one}", &arguments));
        assert!(result.is_err());
    }

    #[test]
    fn handles_utf16_text_and_escapes() {
        assert_eq!(format("中文{0}尾", &owned(&["值"])), "中文值尾");
        assert_eq!(format("a\tb{0}", &owned(&["c"])), "a\tbc");
        assert_eq!(format("line\n{0}", &owned(&["x"])), "line\nx");
    }
}

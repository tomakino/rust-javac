//! Completion search: five-level prefix matching (IDEA CamelHump aligned).
//!
//! 1. ExactStart  - case-exact prefix ("File" -> FileInputStream)
//! 2. Start       - case-insensitive prefix ("file" -> FileInputStream)
//! 3. Camel       - camel-hump subsequence; a character is accepted on a word
//!                  boundary or when it extends the previous contiguous run
//!                  ("zie" -> ZipEntry, "gSM" -> getSecurityManager)
//! 4. Contiguous  - substring anywhere ("co" -> arraycopy)
//! 5. Fuzzy       - loose subsequence; only used as a fallback by the caller

/// How well a label matched the prefix, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MatchKind {
    ExactStart,
    Start,
    Camel,
    Contiguous,
    Fuzzy,
}

/// A successful match plus the matched character positions in the label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Match {
    pub kind: MatchKind,
    pub positions: Vec<usize>,
}

impl Match {
    /// Numeric rank used by the sorter (lower is better).
    pub fn rank(self: &Match) -> u8 {
        match self.kind {
            MatchKind::ExactStart => 0,
            MatchKind::Start => 1,
            MatchKind::Camel => 2,
            MatchKind::Contiguous => 3,
            MatchKind::Fuzzy => 4,
        }
    }
}

/// Match without the fuzzy fallback (the common case).
pub(crate) fn match_label(prefix: &str, label: &str) -> Option<Match> {
    match_with_fuzzy(prefix, label, false)
}

/// Match with the fuzzy fallback enabled (used when nothing else matched).
pub(crate) fn match_label_fuzzy(prefix: &str, label: &str) -> Option<Match> {
    match_with_fuzzy(prefix, label, true)
}

fn match_with_fuzzy(prefix: &str, label: &str, fuzzy: bool) -> Option<Match> {
    if prefix.is_empty() {
        return Some(Match {
            kind: MatchKind::ExactStart,
            positions: Vec::new(),
        });
    }
    let prefix_chars: Vec<char> = prefix.chars().collect();
    let label_chars: Vec<char> = label.chars().collect();
    if prefix_chars.len() > label_chars.len() {
        return None;
    }
    let same = |left: char, right: char| left == right || left.eq_ignore_ascii_case(&right);
    if prefix_chars
        .iter()
        .enumerate()
        .all(|(index, expected)| *expected == label_chars[index])
    {
        return Some(Match {
            kind: MatchKind::ExactStart,
            positions: (0..prefix_chars.len()).collect(),
        });
    }
    if prefix_chars
        .iter()
        .enumerate()
        .all(|(index, expected)| same(*expected, label_chars[index]))
    {
        return Some(Match {
            kind: MatchKind::Start,
            positions: (0..prefix_chars.len()).collect(),
        });
    }
    if let Some(positions) = camel_positions(&prefix_chars, &label_chars, &same) {
        return Some(Match {
            kind: MatchKind::Camel,
            positions,
        });
    }
    if let Some(positions) = contiguous_positions(&prefix_chars, &label_chars, &same) {
        return Some(Match {
            kind: MatchKind::Contiguous,
            positions,
        });
    }
    if fuzzy {
        if let Some(positions) = fuzzy_positions(&prefix_chars, &label_chars, &same) {
            return Some(Match {
                kind: MatchKind::Fuzzy,
                positions,
            });
        }
    }
    None
}

fn camel_positions(
    prefix: &[char],
    label: &[char],
    same: &impl Fn(char, char) -> bool,
) -> Option<Vec<usize>> {
    let mut positions = Vec::with_capacity(prefix.len());
    let mut index = 0usize;
    let mut last: Option<usize> = None;
    for expected in prefix {
        while index < label.len() {
            let boundary = index == 0
                || label[index - 1] == '_'
                || label[index - 1] == '$'
                || label[index - 1] == '.'
                || label[index].is_uppercase();
            let contiguous = last.map(|last| index == last + 1).unwrap_or(false);
            if (boundary || contiguous) && same(*expected, label[index]) {
                break;
            }
            index += 1;
        }
        if index >= label.len() {
            return None;
        }
        positions.push(index);
        last = Some(index);
        index += 1;
    }
    Some(positions)
}

fn contiguous_positions(
    prefix: &[char],
    label: &[char],
    same: &impl Fn(char, char) -> bool,
) -> Option<Vec<usize>> {
    (0..=label.len() - prefix.len()).find_map(|start| {
        if prefix
            .iter()
            .enumerate()
            .all(|(offset, expected)| same(*expected, label[start + offset]))
        {
            Some((start..start + prefix.len()).collect())
        } else {
            None
        }
    })
}

fn fuzzy_positions(
    prefix: &[char],
    label: &[char],
    same: &impl Fn(char, char) -> bool,
) -> Option<Vec<usize>> {
    let mut positions = Vec::with_capacity(prefix.len());
    let mut index = 0usize;
    for expected in prefix {
        while index < label.len() && !same(*expected, label[index]) {
            index += 1;
        }
        if index >= label.len() {
            return None;
        }
        positions.push(index);
        index += 1;
    }
    Some(positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_start_beats_case_insensitive_start() {
        assert_eq!(match_label("File", "FileInputStream").unwrap().kind, MatchKind::ExactStart);
        assert_eq!(match_label("file", "FileInputStream").unwrap().kind, MatchKind::Start);
        assert_eq!(match_label("Zip", "ZipFile").unwrap().kind, MatchKind::ExactStart);
        assert_eq!(match_label("zip", "ZipFile").unwrap().kind, MatchKind::Start);
    }

    #[test]
    fn camel_hump() {
        assert_eq!(match_label("zE", "ZipEntry").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("gSM", "getSecurityManager").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("cTM", "currentTimeMillis").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("oP", "out_Print").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("oP", "out$Print").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("oP", "out.Print").unwrap().kind, MatchKind::Camel);
    }

    #[test]
    fn camel_accepts_contiguous_run() {
        assert_eq!(match_label("zie", "ZipEntry").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("zie", "ZipException").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("zie", "ZipError").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("ziE", "ZipEntry").unwrap().kind, MatchKind::Camel);
        assert_eq!(match_label("zit", "ZipEntry"), None);
    }

    #[test]
    fn contiguous_substring() {
        assert_eq!(match_label("co", "arraycopy").unwrap().kind, MatchKind::Contiguous);
        assert_eq!(match_label("ile", "File").unwrap().kind, MatchKind::Contiguous);
        assert_eq!(match_label("ort", "import").unwrap().kind, MatchKind::Contiguous);
    }

    #[test]
    fn rejects_unrelated_labels() {
        assert_eq!(match_label("File", "FilterOutputStream"), None);
        assert_eq!(match_label("Fileinp", "File"), None);
        assert_eq!(match_label("lo", "lineSeparator"), None);
        assert_eq!(match_label("lo", "clearProperty"), None);
        assert_eq!(match_label("xyz", "System"), None);
    }

    #[test]
    fn fuzzy_is_opt_in() {
        assert_eq!(match_label("abc", "a1b2c3"), None);
        assert_eq!(match_label_fuzzy("abc", "a1b2c3").unwrap().kind, MatchKind::Fuzzy);
        assert_eq!(match_label_fuzzy("abc", "cba"), None);
    }

    #[test]
    fn empty_prefix_matches_everything() {
        assert_eq!(match_label("", "Anything").unwrap().kind, MatchKind::ExactStart);
    }

    #[test]
    fn positions_are_recorded() {
        let matched = match_label("zie", "ZipEntry").unwrap();
        assert_eq!(matched.positions, vec![0, 1, 3]);
        let matched = match_label("co", "arraycopy").unwrap();
        assert_eq!(matched.positions, vec![5, 6]);
    }
}
/// Split a query at dots: "java.util.Ba" -> ["java", "util", "Ba"].
/// A trailing dot produces a trailing empty segment ("java." ->
/// ["java", ""]), which callers treat as an empty prefix.
pub(crate) fn query_segments(query: &str) -> Vec<&str> {
    query.split('.').collect()
}

/// The final segment of a dotted query (the identifier being typed).
pub(crate) fn query_prefix(query: &str) -> &str {
    query.rsplit('.').next().unwrap_or(query)
}

/// The package/class path before the final segment.
pub(crate) fn query_qualifier(query: &str) -> &str {
    match query.rsplit_once('.') {
        Some((qualifier, _)) => qualifier,
        None => "",
    }
}

#[cfg(test)]
mod query_tests {
    use super::*;

    #[test]
    fn dotted_query_splits_into_segments() {
        assert_eq!(query_segments("java.util.Ba"), vec!["java", "util", "Ba"]);
        assert_eq!(query_segments("Ba"), vec!["Ba"]);
        assert_eq!(query_segments("java."), vec!["java", ""]);
        assert_eq!(query_segments(""), vec![""]);
    }

    #[test]
    fn query_prefix_is_last_segment() {
        assert_eq!(query_prefix("java.util.Ba"), "Ba");
        assert_eq!(query_prefix("java."), "");
        assert_eq!(query_prefix("Ba"), "Ba");
    }

    #[test]
    fn query_qualifier_is_everything_before_last_dot() {
        assert_eq!(query_qualifier("java.util.Ba"), "java.util");
        assert_eq!(query_qualifier("java.util"), "java");
        assert_eq!(query_qualifier("Ba"), "");
    }
}

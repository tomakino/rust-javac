//! Completion name matching and dotted-query helpers.
//!
//! The matching implementation is a direct Rust port of IntelliJ IDEA's
//! MinusculeMatcherImpl (see matcher.rs). There is no second custom matcher.

pub(crate) use super::matcher::{
    build_matcher, match_label, IdeaMatcher, Match, MatchedFragment, MatchingMode,
};

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

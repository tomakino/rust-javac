//! Faithful Rust port of IntelliJ IDEA's completion name matcher.
//!
//! Source of truth:
//!   platform/util/text-matching/src/com/intellij/psi/codeStyle/MinusculeMatcherImpl.kt
//!   platform/util/text-matching/src/com/intellij/psi/codeStyle/MinusculeMatcher.kt
//!   platform/analysis-impl/src/com/intellij/codeInsight/completion/impl/CamelHumpMatcher.java
//!   platform/util/base/src/com/intellij/util/text/NameUtilCore.kt
//!
//! Completion uses CamelHumpMatcher's middle-matching prefix: "*prefix", with
//! '.' followed by a space. The matcher below is the MinusculeMatcherImpl core
//! that produces the fragments and the humped matching score.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MatchingMode {
    IgnoreCase,
    MatchCase,
    FirstLetter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MatchedFragment {
    pub start: usize,
    pub end: usize,
}

impl MatchedFragment {
    fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Match {
    pub fragments: Vec<MatchedFragment>,
    pub degree: i32,
}

impl Match {
    pub(crate) fn is_start_match(&self) -> bool {
        self.fragments
            .first()
            .map(|fragment| fragment.start == 0)
            .unwrap_or(true)
    }
}

const MAX_CAMEL_HUMP_MATCHING_LENGTH: usize = 100;

fn is_wildcard(c: char) -> bool {
    c == ' ' || c == '*'
}

fn is_word_separator(c: char) -> bool {
    c.is_whitespace() || c == '_' || c == '-' || c == ':' || c == '+' || c == '.'
}

pub(crate) fn is_word_start(text: &[char], index: usize) -> bool {
    if index >= text.len() {
        return false;
    }
    let cur = text[index];
    let prev = index.checked_sub(1).and_then(|i| text.get(i).copied());
    if cur.is_uppercase() {
        if let Some(prev) = prev {
            if prev.is_uppercase() {
                let next = index + 1;
                return next < text.len() && text[next].is_lowercase();
            }
        }
        return true;
    }
    if cur.is_numeric() {
        return true;
    }
    if !cur.is_alphabetic() {
        return false;
    }
    if is_ideographic(cur) {
        return true;
    }
    index == 0
        || prev.map(|c| !c.is_alphanumeric()).unwrap_or(true)
        || prev.map(|c| is_kana_break(cur, c)).unwrap_or(false)
}

fn is_ideographic(c: char) -> bool {
    matches!(
        c as u32,
        0x3006
            | 0x3007
            | 0x3021..=0x3029
            | 0x3038..=0x303A
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2FA1F
    )
}

fn is_kana(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9D)
}

fn is_hiragana(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x309F)
}

fn is_katakana(c: char) -> bool {
    matches!(c as u32, 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9D)
}

fn is_kana_break(cur: char, prev: char) -> bool {
    if !is_kana(cur) && !is_kana(prev) {
        return false;
    }
    let cur_kata = is_katakana(cur);
    let prev_kata = is_katakana(prev);
    let cur_hira = is_hiragana(cur);
    let prev_hira = is_hiragana(prev);
    if (cur_kata && prev_kata) || (cur_hira && prev_hira) {
        return false;
    }
    cur_kata || cur_hira || prev_kata || prev_hira
}

pub(crate) fn next_word(text: &[char], start: usize) -> usize {
    if start >= text.len() {
        return text.len();
    }
    let ch = text[start];
    if !ch.is_alphanumeric() {
        return start + 1;
    }

    let mut i = start;
    while i < text.len() && text[i].is_numeric() {
        i += 1;
    }
    if i > start {
        return i;
    }

    while i < text.len() && text[i].is_uppercase() {
        i += 1;
    }
    if i > start + 1 {
        if i == text.len() || !text[i].is_alphabetic() {
            return i;
        }
        return i - 1;
    }

    if i == start {
        i += 1;
    }
    while i < text.len() && text[i].is_alphabetic() && !is_word_start(text, i) {
        i += 1;
    }
    i
}

fn name_contains_all_meaningful_chars_in_order(name: &[char], meaningful: &[char]) -> bool {
    let mut meaningful_index = 0usize;
    let mut name_index = 0usize;
    while meaningful_index + 1 < meaningful.len() {
        if name_index >= name.len() {
            return false;
        }
        let c1 = meaningful[meaningful_index];
        let index_of_1 = name.iter().skip(name_index).position(|c| *c == c1).map(|p| p + name_index);
        match index_of_1 {
            Some(index_of_1) if index_of_1 == name_index => {
                name_index = index_of_1 + 1;
            }
            _ => {
                let c2 = meaningful[meaningful_index + 1];
                if c1 == c2 {
                    if index_of_1.is_none() {
                        return false;
                    }
                    name_index = index_of_1.unwrap() + 1;
                } else {
                    let index_of_2 = name
                        .iter()
                        .skip(name_index)
                        .position(|c| *c == c2)
                        .map(|p| p + name_index);
                    name_index = match (index_of_1, index_of_2) {
                        (Some(a), Some(b)) => a.min(b) + 1,
                        (Some(a), None) => a + 1,
                        (None, Some(b)) => b + 1,
                        (None, None) => return false,
                    };
                }
            }
        }
        meaningful_index += 2;
    }
    true
}

pub(crate) struct IdeaMatcher {
    pattern: Vec<char>,
    mode: MatchingMode,
    hard_separators: Vec<char>,
    is_lower_case: Vec<bool>,
    is_upper_case: Vec<bool>,
    is_word_separator: Vec<bool>,
    to_upper_case: Vec<char>,
    to_lower_case: Vec<char>,
    mixed_case: bool,
    has_separators: bool,
    has_dots: bool,
    meaningful_characters: Vec<char>,
}

impl IdeaMatcher {
    pub(crate) fn new(pattern: &str, mode: MatchingMode, hard_separators: &str) -> Self {
        let pattern: Vec<char> = pattern.strip_suffix("* ").unwrap_or(pattern).chars().collect();
        let len = pattern.len();
        let mut is_lower_case = vec![false; len];
        let mut is_upper_case = vec![false; len];
        let mut word_separator_flags = vec![false; len];
        let mut to_upper_case = vec!['\0'; len];
        let mut to_lower_case = vec!['\0'; len];
        let mut meaningful = Vec::new();
        let mut seen_non_wildcard = false;
        let mut seen_lower_case = false;
        let mut seen_upper_case_not_immediately_after_wildcard = false;
        let mut has_dots = false;
        let mut has_separators = false;

        for (k, &c) in pattern.iter().enumerate() {
            let word_separator = is_word_separator(c);
            let upper = c.is_uppercase();
            let lower = c.is_lowercase();
            let upper_char = c.to_uppercase().next().unwrap_or(c);
            let lower_char = c.to_lowercase().next().unwrap_or(c);
            if lower {
                seen_lower_case = true;
            }
            if c == '.' {
                has_dots = true;
            }
            if seen_non_wildcard && upper {
                seen_upper_case_not_immediately_after_wildcard = true;
            }
            if !is_wildcard(c) {
                seen_non_wildcard = true;
                meaningful.push(lower_char);
                meaningful.push(upper_char);
            }
            if seen_non_wildcard && word_separator {
                has_separators = true;
            }
            word_separator_flags[k] = word_separator;
            is_upper_case[k] = upper;
            is_lower_case[k] = lower;
            to_upper_case[k] = upper_char;
            to_lower_case[k] = lower_char;
        }

        Self {
            pattern,
            mode,
            hard_separators: hard_separators.chars().collect(),
            is_lower_case,
            is_upper_case,
            is_word_separator: word_separator_flags,
            to_upper_case,
            to_lower_case,
            mixed_case: seen_lower_case && seen_upper_case_not_immediately_after_wildcard,
            has_separators,
            has_dots,
            meaningful_characters: meaningful,
        }
    }

    pub(crate) fn match_name(&self, name: &str) -> Option<Vec<MatchedFragment>> {
        let name: Vec<char> = name.chars().collect();
        if name.len() < self.meaningful_characters.len() / 2 {
            return None;
        }
        if self.pattern.len() > MAX_CAMEL_HUMP_MATCHING_LENGTH {
            return self.match_by_substring(&name);
        }
        if !name_contains_all_meaningful_chars_in_order(&name, &self.meaningful_characters) {
            return None;
        }
        let mut ranges = self.match_wildcards(&name, 0, 0)?;
        ranges.reverse();
        Some(ranges)
    }

    pub(crate) fn matching_degree(&self, name: &str, value_start_case_match: bool) -> i32 {
        let chars: Vec<char> = name.chars().collect();
        let fragments = self.match_chars(&chars);
        self.calculate_humped_matching_score(&chars, value_start_case_match, fragments.as_deref())
    }

    pub(crate) fn is_start_match(&self, name: &str) -> bool {
        self.match_name(name)
            .map(|fragments| fragments.first().map(|f| f.start == 0).unwrap_or(true))
            .unwrap_or(false)
    }

    fn match_chars(&self, name: &[char]) -> Option<Vec<MatchedFragment>> {
        if name.len() < self.meaningful_characters.len() / 2 {
            return None;
        }
        if self.pattern.len() > MAX_CAMEL_HUMP_MATCHING_LENGTH {
            return self.match_by_substring(name);
        }
        if !name_contains_all_meaningful_chars_in_order(name, &self.meaningful_characters) {
            return None;
        }
        let mut ranges = self.match_wildcards(name, 0, 0)?;
        ranges.reverse();
        Some(ranges)
    }

    fn match_by_substring(&self, name: &[char]) -> Option<Vec<MatchedFragment>> {
        let meaningful_count = self.meaningful_characters.len() / 2;
        if name.len() < meaningful_count {
            return None;
        }
        if self.is_pattern_char_signed(0, '*') {
            for i in 0..=name.len() - meaningful_count {
                let length = self.meaningful_chars_match_at(name, i);
                if length != 0 {
                    return Some(vec![MatchedFragment::new(i, i + length)]);
                }
            }
            return None;
        }
        let length = self.meaningful_chars_match_at(name, 0);
        if length != 0 {
            Some(vec![MatchedFragment::new(0, length)])
        } else {
            None
        }
    }

    fn meaningful_chars_match_at(&self, name: &[char], name_index: usize) -> usize {
        let mut meaningful_index = 0usize;
        let mut name_pos = name_index;
        while name_pos < name.len() && meaningful_index + 1 < self.meaningful_characters.len() {
            let c = name[name_pos];
            if c == self.meaningful_characters[meaningful_index]
                || c == self.meaningful_characters[meaningful_index + 1]
            {
                meaningful_index += 2;
                name_pos += 1;
            } else if is_wildcard(c) {
                name_pos += 1;
            } else {
                return 0;
            }
        }
        name_pos - name_index
    }

    fn match_wildcards(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: isize,
    ) -> Option<Vec<MatchedFragment>> {
        if name_index < 0 {
            return None;
        }
        if !self.is_wildcard(pattern_index) {
            return if pattern_index == self.pattern.len() {
                Some(Vec::new())
            } else {
                self.match_fragment(name, pattern_index, name_index as usize)
            };
        }

        let mut pattern_index = pattern_index;
        loop {
            pattern_index += 1;
            if !self.is_wildcard(pattern_index) {
                break;
            }
        }

        if pattern_index == self.pattern.len() {
            if self.is_trailing_space_pattern()
                && name_index as usize != name.len()
                && (pattern_index < 2 || !self.is_upper_case_or_digit(pattern_index - 2))
            {
                let space_index = name
                    .iter()
                    .skip(name_index as usize)
                    .position(|c| *c == ' ')
                    .map(|p| p + name_index as usize);
                return space_index.map(|i| vec![MatchedFragment::new(i, i + 1)]);
            }
            return Some(Vec::new());
        }

        let next = self.find_next_pattern_char_occurrence(name, name_index, pattern_index);
        self.match_skipping_words(name, pattern_index, next, true)
    }

    fn is_trailing_space_pattern(&self) -> bool {
        self.pattern.last().copied() == Some(' ')
    }

    fn is_upper_case_or_digit(&self, pattern_index: usize) -> bool {
        self.is_upper_case[pattern_index] || self.pattern[pattern_index].is_numeric()
    }

    fn match_skipping_words(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: isize,
        allow_special_chars: bool,
    ) -> Option<Vec<MatchedFragment>> {
        let mut name_index = name_index;
        let mut max_found_length = 0usize;
        while name_index >= 0 {
            let index = name_index as usize;
            let fragment_length = if self.seems_like_fragment_start(name, pattern_index, index) {
                self.max_matching_fragment(name, pattern_index, index)
            } else {
                0
            };

            if fragment_length > max_found_length
                || (index + fragment_length == name.len() && self.is_trailing_space_pattern())
            {
                if !self.is_middle_match(name, pattern_index, index) {
                    max_found_length = fragment_length;
                }
                if let Some(ranges) =
                    self.match_inside_fragment(name, pattern_index, index, fragment_length)
                {
                    return Some(ranges);
                }
            }
            let next = self.find_next_pattern_char_occurrence(name, name_index + 1, pattern_index);
            name_index = if allow_special_chars {
                next
            } else {
                self.check_for_special_chars(name, index + 1, next, pattern_index)
            };
        }
        None
    }

    fn find_next_pattern_char_occurrence(
        &self,
        name: &[char],
        start_at: isize,
        pattern_index: usize,
    ) -> isize {
        if !self.is_pattern_char_signed(pattern_index as isize - 1, '*')
            && !self.is_word_separator[pattern_index]
        {
            self.index_of_word_start(name, pattern_index, start_at)
        } else {
            self.index_of_ignore_case(name, start_at, pattern_index)
        }
    }

    fn check_for_special_chars(
        &self,
        name: &[char],
        start: usize,
        end: isize,
        pattern_index: usize,
    ) -> isize {
        if end < 0 {
            return -1;
        }
        if !self.has_separators
            && !self.mixed_case
            && self.index_of_any(name, &self.hard_separators, start, end as usize) != -1
        {
            return -1;
        }
        if self.has_dots
            && !self.is_pattern_char_signed(pattern_index as isize - 1, '.')
            && self.index_of(name, '.', start, end as usize, false) != -1
        {
            return -1;
        }
        end
    }

    fn seems_like_fragment_start(
        &self,
        name: &[char],
        pattern_index: usize,
        next_occurrence: usize,
    ) -> bool {
        if !self.is_upper_case[pattern_index] {
            return true;
        }
        if next_occurrence >= name.len() {
            return false;
        }
        name[next_occurrence].is_uppercase()
            || is_word_start(name, next_occurrence)
            || (!self.mixed_case && self.mode != MatchingMode::MatchCase)
    }

    fn char_equals(
        &self,
        pattern_char: char,
        pattern_index: usize,
        c: char,
        ignore_case: bool,
    ) -> bool {
        pattern_char == c
            || (ignore_case
                && (self.to_lower_case[pattern_index] == c || self.to_upper_case[pattern_index] == c))
    }

    fn match_fragment(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: usize,
    ) -> Option<Vec<MatchedFragment>> {
        let fragment_length = self.max_matching_fragment(name, pattern_index, name_index);
        if fragment_length == 0 {
            None
        } else {
            self.match_inside_fragment(name, pattern_index, name_index, fragment_length)
        }
    }

    fn max_matching_fragment(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: usize,
    ) -> usize {
        if !self.is_first_char_matching(name, name_index, pattern_index) {
            return 0;
        }
        let mut i = 1usize;
        let ignore_case = self.mode != MatchingMode::MatchCase;
        while name_index + i < name.len() && pattern_index + i < self.pattern.len() {
            let name_char = name[name_index + i];
            if !self.char_equals(self.pattern[pattern_index + i], pattern_index + i, name_char, ignore_case) {
                if self.is_skipping_digit_between_pattern_digits(pattern_index + i, name_char) {
                    return 0;
                }
                break;
            }
            i += 1;
        }
        i
    }

    fn is_skipping_digit_between_pattern_digits(&self, pattern_index: usize, name_char: char) -> bool {
        self.pattern[pattern_index].is_numeric()
            && pattern_index > 0
            && self.pattern[pattern_index - 1].is_numeric()
            && name_char.is_numeric()
    }

    fn match_inside_fragment(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: usize,
        fragment_length: usize,
    ) -> Option<Vec<MatchedFragment>> {
        let min_fragment =
            if self.is_middle_match(name, pattern_index, name_index) { 3 } else { 1 };
        self.improve_camel_humps(name, pattern_index, name_index, fragment_length, min_fragment)
            .or_else(|| {
                self.find_longest_matching_prefix(
                    name,
                    pattern_index,
                    name_index,
                    fragment_length,
                    min_fragment,
                )
            })
    }

    fn is_middle_match(&self, name: &[char], pattern_index: usize, name_index: usize) -> bool {
        self.is_pattern_char_signed(pattern_index as isize - 1, '*')
            && !self.is_wildcard(pattern_index + 1)
            && name
                .get(name_index)
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false)
            && !is_word_start(name, name_index)
    }

    fn find_longest_matching_prefix(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: usize,
        fragment_length: usize,
        min_fragment: usize,
    ) -> Option<Vec<MatchedFragment>> {
        if pattern_index + fragment_length >= self.pattern.len() {
            return Some(vec![MatchedFragment::new(name_index, name_index + fragment_length)]);
        }

        let mut i = fragment_length;
        loop {
            if i < min_fragment && !(i > 0 && self.is_wildcard(pattern_index + i)) {
                break;
            }
            let ranges = if self.is_wildcard(pattern_index + i) {
                self.match_wildcards(name, pattern_index + i, (name_index + i) as isize)
            } else {
                let mut next = self.find_next_pattern_char_occurrence(
                    name,
                    (name_index + i + 1) as isize,
                    pattern_index + i,
                );
                next = self.check_for_special_chars(
                    name,
                    name_index + i,
                    next,
                    pattern_index + i,
                );
                if next >= 0 {
                    self.match_skipping_words(name, pattern_index + i, next, false)
                } else {
                    None
                }
            };
            if let Some(ranges) = ranges {
                return Some(self.append_range(ranges, name_index, i));
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
        None
    }

    fn improve_camel_humps(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: usize,
        max_fragment: usize,
        min_fragment: usize,
    ) -> Option<Vec<MatchedFragment>> {
        for i in min_fragment..max_fragment {
            if self.is_uppercase_pattern_vs_lowercase_name_char(
                name,
                pattern_index + i,
                name_index + i,
            ) {
                if let Some(ranges) =
                    self.find_uppercase_match_further(name, pattern_index + i, name_index + i)
                {
                    return Some(self.append_range(ranges, name_index, i));
                }
            }
        }
        None
    }

    fn is_uppercase_pattern_vs_lowercase_name_char(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: usize,
    ) -> bool {
        self.is_upper_case[pattern_index] && self.pattern[pattern_index] != name[name_index]
    }

    fn find_uppercase_match_further(
        &self,
        name: &[char],
        pattern_index: usize,
        name_index: usize,
    ) -> Option<Vec<MatchedFragment>> {
        let next_word_start =
            self.index_of_word_start(name, pattern_index, name_index as isize);
        self.match_wildcards(name, pattern_index, next_word_start)
    }

    fn is_first_char_matching(&self, name: &[char], name_index: usize, pattern_index: usize) -> bool {
        if name_index >= name.len() {
            return false;
        }
        let ignore_case = self.mode != MatchingMode::MatchCase;
        let pattern_char = self.pattern[pattern_index];
        if !self.char_equals(pattern_char, pattern_index, name[name_index], ignore_case) {
            return false;
        }
        !(self.mode == MatchingMode::FirstLetter
            && (pattern_index == 0 || (pattern_index == 1 && self.is_wildcard(0)))
            && self.has_case(pattern_index)
            && self.is_upper_case[pattern_index] != name[0].is_uppercase())
    }

    fn has_case(&self, pattern_index: usize) -> bool {
        self.is_upper_case[pattern_index] || self.is_lower_case[pattern_index]
    }

    fn is_wildcard(&self, pattern_index: usize) -> bool {
        pattern_index < self.pattern.len() && is_wildcard(self.pattern[pattern_index])
    }

    fn is_pattern_char_signed(&self, pattern_index: isize, c: char) -> bool {
        pattern_index >= 0
            && self
                .pattern
                .get(pattern_index as usize)
                .copied()
                .map(|pc| pc == c)
                .unwrap_or(false)
    }

    fn index_of_word_start(
        &self,
        name: &[char],
        pattern_index: usize,
        start_from: isize,
    ) -> isize {
        let p = self.pattern[pattern_index];
        if start_from < 0
            || start_from as usize >= name.len()
            || (self.mixed_case
                && self.is_lower_case[pattern_index]
                && !(pattern_index > 0 && self.is_word_separator[pattern_index - 1]))
        {
            return -1;
        }
        let is_special_symbol = !p.is_alphanumeric();
        let mut i = start_from;
        loop {
            i = self.index_of_ignore_case(name, i, pattern_index);
            if i < 0 {
                return -1;
            }
            if is_special_symbol || is_word_start(name, i as usize) {
                return i;
            }
            i += 1;
        }
    }

    fn index_of_ignore_case(&self, name: &[char], from_index: isize, pattern_index: usize) -> isize {
        if from_index < 0 {
            return -1;
        }
        let p = self.pattern[pattern_index];
        if p.is_ascii() {
            let upper = self.to_upper_case[pattern_index];
            let lower = self.to_lower_case[pattern_index];
            for (i, c) in name.iter().enumerate().skip(from_index as usize) {
                if *c == upper || *c == lower {
                    return i as isize;
                }
            }
            return -1;
        }
        name.iter()
            .enumerate()
            .skip(from_index as usize)
            .find(|(_, c)| **c == p || c.to_lowercase().eq(p.to_lowercase()))
            .map(|(i, _)| i as isize)
            .unwrap_or(-1)
    }

    fn index_of_any(&self, name: &[char], chars: &[char], start: usize, end: usize) -> isize {
        if chars.is_empty() || start >= end || start >= name.len() {
            return -1;
        }
        for i in start..end.min(name.len()) {
            if chars.contains(&name[i]) {
                return i as isize;
            }
        }
        -1
    }

    fn index_of(&self, name: &[char], c: char, start: usize, end: usize, ignore_case: bool) -> isize {
        if start >= end || start >= name.len() {
            return -1;
        }
        for i in start..end.min(name.len()) {
            let equal = if ignore_case {
                name[i] == c || name[i].to_lowercase().eq(c.to_lowercase())
            } else {
                name[i] == c
            };
            if equal {
                return i as isize;
            }
        }
        -1
    }

    fn append_range(
        &self,
        mut ranges: Vec<MatchedFragment>,
        from: usize,
        length: usize,
    ) -> Vec<MatchedFragment> {
        if ranges.is_empty() {
            return vec![MatchedFragment::new(from, from + length)];
        }
        let last = *ranges.last().unwrap();
        if last.start == from + length {
            let last_index = ranges.len() - 1;
            ranges[last_index] = MatchedFragment::new(from, last.end);
        } else {
            ranges.push(MatchedFragment::new(from, from + length));
        }
        ranges
    }

    fn calculate_humped_matching_score(
        &self,
        name: &[char],
        value_start_case_match: bool,
        fragments: Option<&[MatchedFragment]>,
    ) -> i32 {
        let Some(fragments) = fragments else {
            return i32::MIN;
        };
        if fragments.is_empty() {
            return 0;
        }

        let first = fragments[0];
        let start_match = first.start == 0;
        let valued_start_match = start_match && value_start_case_match;

        let mut matching_case = 0i32;
        let mut p: isize = -1;
        let mut skipped_humps = 0i32;
        let mut next_hump_start = 0usize;
        let mut hump_start_matched_upper = false;

        for (fragment_index, range) in fragments.iter().enumerate() {
            for i in range.start..range.end {
                let after_gap = i == range.start && fragment_index != 0;
                let mut is_hump_start = false;
                while next_hump_start <= i {
                    if next_hump_start == i {
                        is_hump_start = true;
                    } else if after_gap {
                        skipped_humps += 1;
                    }
                    next_hump_start = if next_hump_start < name.len()
                        && name[next_hump_start].is_numeric()
                    {
                        next_hump_start + 1
                    } else {
                        next_word(name, next_hump_start)
                    };
                }

                let c = name[i];
                p = self.index_of_pattern_from(self.pattern.as_slice(), c, p + 1);
                if p < 0 {
                    break;
                }
                if is_hump_start {
                    hump_start_matched_upper = c == self.pattern[p as usize]
                        && self.is_upper_case[p as usize];
                }
                matching_case += self.evaluate_case_matching(
                    p as usize,
                    valued_start_match,
                    hump_start_matched_upper,
                    i,
                    after_gap,
                    is_hump_start,
                    c,
                );
            }
        }

        let start_index = first.start;
        let after_separator =
            self.index_of_any(name, &self.hard_separators, 0, start_index) >= 0;
        let word_start = start_index == 0
            || (is_word_start(name, start_index) && !is_word_start(name, start_index - 1));
        let final_match = fragments.last().unwrap().end == name.len();

        (if word_start { 1000 } else { 0 }) + matching_case - fragments.len() as i32
            - skipped_humps * 10
            + (if after_separator { 0 } else { 2 })
            + (if start_match { 1 } else { 0 })
            + (if final_match { 1 } else { 0 })
    }

    fn index_of_pattern_from(&self, pattern: &[char], c: char, from: isize) -> isize {
        if from < 0 {
            return -1;
        }
        pattern
            .iter()
            .enumerate()
            .skip(from as usize)
            .find(|(_, p)| **p == c || p.to_lowercase().eq(c.to_lowercase()))
            .map(|(i, _)| i as isize)
            .unwrap_or(-1)
    }

    fn evaluate_case_matching(
        &self,
        pattern_index: usize,
        valued_start_match: bool,
        hump_start_matched_upper: bool,
        name_index: usize,
        _after_gap: bool,
        is_hump_start: bool,
        name_char: char,
    ) -> i32 {
        if _after_gap && is_hump_start && self.is_lower_case[pattern_index] {
            -10
        } else if name_char == self.pattern[pattern_index] {
            if self.is_upper_case[pattern_index] {
                50
            } else if name_index == 0 && valued_start_match {
                150
            } else if is_hump_start {
                1
            } else {
                0
            }
        } else if is_hump_start {
            -1
        } else if self.is_lower_case[pattern_index] && hump_start_matched_upper {
            -1
        } else {
            0
        }
    }
}

pub(crate) fn apply_middle_matching(prefix: &str) -> String {
    if prefix.is_empty() {
        return prefix.to_string();
    }
    format!("*{}", prefix.replace('.', ". ").trim())
}

pub(crate) fn build_matcher(prefix: &str) -> IdeaMatcher {
    IdeaMatcher::new(&apply_middle_matching(prefix), MatchingMode::IgnoreCase, "")
}

pub(crate) fn match_label(prefix: &str, label: &str) -> Option<Match> {
    let matcher = build_matcher(prefix);
    let fragments = matcher.match_name(label)?;
    let degree = matcher.matching_degree(label, true);
    Some(Match { fragments, degree })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, name: &str) -> bool {
        IdeaMatcher::new(pattern, MatchingMode::IgnoreCase, "")
            .match_name(name)
            .is_some()
    }

    fn completion_matches(pattern: &str, name: &str) -> bool {
        build_matcher(pattern).match_name(name).is_some()
    }

    fn fragments(pattern: &str, name: &str) -> Vec<MatchedFragment> {
        IdeaMatcher::new(pattern, MatchingMode::IgnoreCase, "")
            .match_name(name)
            .unwrap()
    }

    #[test]
    fn simple_cases() {
        assert!(matches("N", "NameUtilTest"));
        assert!(matches("NU", "NameUtilTest"));
        assert!(matches("NUT", "NameUtilTest"));
        assert!(matches("NaUT", "NameUtilTest"));
        assert!(!matches("NeUT", "NameUtilTest"));
        assert!(!matches("NaUTa", "NameUtilTest"));
        assert!(matches("NaUtT", "NameUtilTest"));
        assert!(matches("NaUtTe", "NameUtilTest"));
        assert!(matches("AACl", "AAClass"));
        assert!(matches("ZZZ", "ZZZZZZZZZZ"));
    }

    #[test]
    fn empty_prefix() {
        assert!(matches("", ""));
        assert!(matches("", "asdfs"));
    }

    #[test]
    fn skip_words() {
        assert!(matches("nt", "NameUtilTest"));
        assert!(matches("repl map", "ReplacePathToMacroMap"));
        assert!(matches("replmap", "ReplacePathToMacroMap"));
        assert!(matches("CertificateEx", "CertificateEncodingException"));
        assert!(!matches("ABCD", "AbstractButton.DISABLED_ICON_CHANGED_PROPERTY"));
        assert!(matches("templipa", "template_impl_template_list_panel"));
        assert!(matches("templistpa", "template_impl_template_list_panel"));
    }

    #[test]
    fn middle_matching_minimum_three_letters() {
        assert!(matches("*fo", "reformat"));
        assert!(matches("*f", "reFormat"));
        assert!(matches("*f", "format"));
        assert!(matches("*Stri", "string"));
        assert!(!matches("*fc", "reformatCode"));
        assert!(!matches("*foc", "reformatCode"));
        assert!(matches("*forc", "reformatCode"));
        assert!(!matches("*sTC", "LazyClassTypeConstructor"));
        assert!(!matches("*Icon", "LEADING_CONSTRUCTOR"));
        assert!(matches("*I", "LEADING_CONSTRUCTOR"));
        assert!(matches("*in", "LEADING_CONSTRUCTOR"));
        assert!(matches("*ing", "LEADING_CONSTRUCTOR"));
        assert!(!matches("*inc", "LEADING_CONSTRUCTOR"));
        assert!(matches("*l", "AppDelegate"));
        assert!(matches("*leg", "AppDelegate"));
    }

    #[test]
    fn middle_matching() {
        assert!(matches("*zz*", "ListConfigzzKey"));
        assert!(matches("*zz", "ListConfigzzKey"));
        assert!(matches("*old", "folder"));
        assert!(matches("SWU*H*7", "SWUpgradeHdlrFSPR7Test"));
        assert!(matches("SWU*H*R", "SWUpgradeHdlrFSPR7Test"));
        assert!(matches("SWU*H*R", "SWUPGRADEHDLRFSPR7TEST"));
        assert!(matches("*git", "GitBlaBla"));
        assert!(matches("*Git", "GitBlaBla"));
        assert!(!matches("*get*A", "getClass"));
        assert!(matches("*git", "BlaGitBla"));
        assert!(matches("*Git*", "AtpGenerationItem"));
        assert!(matches("Collec*Util*", "CollectionUtils"));
        assert!(matches("Collec*Util*", "CollectionUtilsTest"));
        assert!(matches("*us", "usage"));
        assert!(matches(" us", "usage"));
        assert!(matches(" fo. ba", "getFoo.getBar"));
        assert!(matches(" File. sepa", "File.separator"));
        assert!(matches(" File. sepa", "File._separator"));
        assert!(matches(" File. _sepa", "File._separator"));
        assert!(matches(" _fo", "_foo"));
        assert!(matches("*BComp", "BaseComponent"));
    }

    #[test]
    fn zip_behavior() {
        assert!(completion_matches("zip", "ZipFile"));
        assert!(completion_matches("zip", "GZIPInputStream"));
        assert!(completion_matches("zip", "GZIPOutputStream"));
        assert!(!completion_matches("zip", "SQLInvalidAuthorizationSpecException"));
        assert!(!completion_matches("zip", "SerializationException"));
        assert!(!completion_matches("zip", "SizeLimitExceededException"));
    }

    #[test]
    fn matching_fragments() {
        assert_eq!(
            fragments("ncldfou*ion", "NoClassDefFoundException"),
            vec![
                MatchedFragment::new(0, 1),
                MatchedFragment::new(2, 4),
                MatchedFragment::new(7, 8),
                MatchedFragment::new(10, 13),
                MatchedFragment::new(21, 24),
            ]
        );
        assert_eq!(
            fragments("SWU*H*R", "SWUPGRADEHDLRFSPR7TEST"),
            vec![
                MatchedFragment::new(0, 3),
                MatchedFragment::new(9, 10),
                MatchedFragment::new(12, 13),
            ]
        );
    }

    #[test]
    fn matching_degree_preferences() {
        let matcher = build_matcher("jscote");
        assert!(
            matcher.matching_degree("JsfCompletionTest", true)
                < matcher.matching_degree("JSCompletionTest", true)
        );
        let matcher = build_matcher("getCU");
        assert!(
            matcher.matching_degree("getCurrentUser", true)
                > matcher.matching_degree("getCurrentSomething", true)
        );
        let matcher = build_matcher("cL");
        assert!(
            matcher.matching_degree("class", true) < matcher.matching_degree("coreLoader", true)
        );
    }

    #[test]
    fn idea_space_delimiters() {
        assert!(matches("Na Ut Te", "name util test"));
        assert!(matches("Na Ut Te", "name Util Test"));
        assert!(!matches("Na Ut Ta", "name Util Test"));
        assert!(matches("na ut te", "name util test"));
        assert!(matches("na ut", "name_util_test"));
        assert!(matches("na te", "name_util_test"));
        assert!(!matches("na ti", "name_util_test"));
    }

    #[test]
    fn idea_underscore_style() {
        assert!(matches("N_U_T", "NAME_UTIL_TEST"));
        assert!(matches("NUT", "NAME_UTIL_TEST"));
        assert!(!matches("NUT", "NameutilTest"));
    }

    #[test]
    fn idea_all_uppercase() {
        assert!(matches("NOS", "NetOutputStream"));
    }

    #[test]
    fn idea_lowercase_humps() {
        assert!(matches("foo", "foo"));
        assert!(!matches("foo", "fxoo"));
        assert!(matches("foo", "fOo"));
        assert!(matches("foo", "fxOo"));
        assert!(matches("foo", "fXOo"));
        assert!(matches("fOo", "foo"));
        assert!(!matches("fOo", "FaOaOaXXXX"));
        assert!(matches("ncdfoe", "NoClassDefFoundException"));
        assert!(matches("fob", "FOO_BAR"));
        assert!(matches("fo_b", "FOO_BAR"));
        assert!(matches("fob", "FOO BAR"));
        assert!(matches("fo b", "FOO BAR"));
        assert!(matches("em", "emptyList"));
        assert!(matches("bui", "BuildConfig.groovy"));
        assert!(matches("buico", "BuildConfig.groovy"));
        assert!(matches("buico.gr", "BuildConfig.groovy"));
        assert!(matches("bui.gr", "BuildConfig.groovy"));
        assert!(matches("Wlo", "WebLogic"));
    }

    #[test]
    fn idea_final_space() {
        assert!(matches("a ", "alpha + beta"));
        assert!(matches("a ", "a "));
        assert!(matches("a ", "a"));
        assert!(matches("Foo ", "Foo"));
        assert!(!matches("Foo ", "FooBar"));
        assert!(!matches("Collections ", "CollectionSplitter"));
        assert!(matches("CollectionS ", "CollectionSplitter"));
        assert!(matches("*run ", "in Runnable.run"));
        assert!(!matches("*l ", "AppDelegate"));
    }

    #[test]
    fn idea_digits() {
        assert!(matches("foba4", "FooBar4"));
        assert!(matches("foba", "Foo4Bar"));
        assert!(matches("*TEST-* ", "TEST-001"));
        assert!(matches("*v2 ", "VARCHAR2"));
        assert!(matches("smart8co", "SmartType18CompletionTest"));
        assert!(matches("smart8co", "smart18completion"));
        assert!(!matches("*012", "001122"));
        assert!(matches("012", "0a1_22"));
    }

    #[test]
    fn idea_fragments_extra() {
        assert_eq!(
            fragments("d*st", "doGet(HttpServletRequest, HttpServletResponse):void"),
            vec![MatchedFragment::new(0, 1), MatchedFragment::new(22, 24)]
        );
        assert_eq!(
            fragments("doge*st", "doGet(HttpServletRequest, HttpServletResponse):void"),
            vec![MatchedFragment::new(0, 4), MatchedFragment::new(22, 24)]
        );
        assert_eq!(fragments("_", "_test"), vec![MatchedFragment::new(0, 1)]);
        assert_eq!(fragments("_t", "_test"), vec![MatchedFragment::new(0, 2)]);
    }

    fn degree(pattern: &str, name: &str, mode: MatchingMode) -> i32 {
        // The direct matcher tests call matchingDegree(name), i.e. valueStartCaseMatch=false.
        IdeaMatcher::new(pattern, mode, "").matching_degree(name, false)
    }

    #[test]
    fn idea_matching_degree_preferences_full() {
        use MatchingMode::*;
        assert!(degree("jscote", "JsfCompletionTest", IgnoreCase) < degree("jscote", "JSCompletionTest", IgnoreCase));
        assert!(degree("OCO", "OneCoolObject", FirstLetter) < degree("OCO", "OCObject", FirstLetter));
        assert!(degree("MUp", "MavenUmlProvider", FirstLetter) < degree("MUp", "MarkUp", FirstLetter));
        assert!(degree("MUP", "MarkUp", FirstLetter) < degree("MUP", "MavenUmlProvider", FirstLetter));
        assert!(
            degree("CertificateExce", "CertificateEncodingException", FirstLetter)
                < degree("CertificateExce", "CertificateException", FirstLetter)
        );
        assert!(degree("boo", "Boolean", IgnoreCase) < degree("boo", "boolean", IgnoreCase));
        assert!(degree("Boo", "boolean", IgnoreCase) < degree("Boo", "Boolean", IgnoreCase));
        assert!(
            degree("getCU", "getCurrentSomething", FirstLetter)
                < degree("getCU", "getCurrentUser", FirstLetter)
        );
        assert!(degree("cL", "class", FirstLetter) < degree("cL", "coreLoader", FirstLetter));
        assert!(degree("cL", "class", FirstLetter) < degree("cL", "classLoader", FirstLetter));
        assert!(
            degree("inse", "InstrumentationError", IgnoreCase)
                < degree("inse", "intSet", IgnoreCase)
        );
        assert!(degree("String", "STRING", IgnoreCase) < degree("String", "String", IgnoreCase));
        assert!(degree("*String", "STRING", IgnoreCase) < degree("*String", "String", IgnoreCase));
    }

    #[test]
    fn idea_extra_completion_matches() {
        assert!(completion_matches("Fileinp", "FileInputStream"));
        assert!(completion_matches("Fileinp", "FileImageInputStream"));
        assert!(completion_matches("Fileinp", "FileCacheImageInputStream"));
        assert!(!completion_matches("Fileinp", "File"));
        assert!(!completion_matches("Fileinp", "FileHandler"));
        assert!(completion_matches("gstream", "GZIPInputStream"));
        assert!(completion_matches("gstream", "LogStream"));
        assert!(completion_matches("gstream", "LongStream"));
        assert!(completion_matches("gstream", "RecordingStream"));
        assert!(completion_matches("gstream", "RemoteRecordingStream"));
        assert!(!completion_matches("gstream", "SQLInvalidAuthorizationSpecException"));
    }

    #[test]
    fn idea_matching_fragments_sorted_and_parts() {
        assert_eq!(
            fragments("ncldfou*ion", "NoClassDefFoundException"),
            vec![
                MatchedFragment::new(0, 1),
                MatchedFragment::new(2, 4),
                MatchedFragment::new(7, 8),
                MatchedFragment::new(10, 13),
                MatchedFragment::new(21, 24),
            ]
        );
        assert_eq!(
            fragments("getCU", "getCurrentUser"),
            vec![MatchedFragment::new(0, 4), MatchedFragment::new(10, 11)]
        );
    }
}

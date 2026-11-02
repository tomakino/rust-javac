//! A class that defines source code positions as simple character
//! offsets from the beginning of the file. The first character
//! is at position 0.
//!
//! Support is also provided for (line,column) coordinates, but tab
//! expansion is optional and no Unicode escape translation is considered.
//! The first character is at location (1,1).
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! Java's `Position` class is a namespace (private constructor); it maps to free
//! functions and constants here.
//! overloads:
//! - LineMap.getStartPosition(int)  -> get_start_position
//! - LineMap.getStartPosition(long) -> get_start_position_long
//! - LineMap.getPosition(int,int)   -> get_position
//! - LineMap.getPosition(long,long) -> get_position_long
//! - LineMap.getLineNumber(int)     -> get_line_number
//! - LineMap.getLineNumber(long)    -> get_line_number_long
//! - LineMap.getColumnNumber(int)   -> get_column_number
//! - LineMap.getColumnNumber(long)  -> get_column_number_long
//!
//! LineTabMapImpl extends LineMapImpl; per PLAN.md Porting rules the Java subclass fields are
//! modelled with an enum kind in the outer struct, and the virtual call to setTabPosition is a
//! match on that kind. Neither LineMapImpl nor LineTabMapImpl is instantiated outside
//! Position.makeLineMap in javac (verified by grep over the upstream tree).

use std::cell::Cell;

use super::layout_characters::{CR, LF, TAB, tabulate};

pub const NOPOS: i32 = -1;

pub const FIRSTPOS: i32 = 0;
pub const FIRSTLINE: i32 = 1;
pub const FIRSTCOLUMN: i32 = 1;

pub const LINESHIFT: u32 = 10;
pub const MAXCOLUMN: i32 = (1 << LINESHIFT) - 1;
pub const MAXLINE: i32 = (1 << (32 - LINESHIFT)) - 1;

pub const MAXPOS: i32 = i32::MAX;

/// A two-way map between line/column numbers and positions,
/// derived from a scan done at creation time.  Tab expansion is
/// optionally supported via a character map.  Text content
/// is not retained.
/// <p>
/// Notes:  The first character position FIRSTPOS is at
/// (FIRSTLINE,FIRSTCOLUMN).  No account is taken of Unicode escapes.
pub trait LineMap {
    /// Find the start position of a line.
    ///
    /// @param line number of line (first is 1)
    /// @return     position of first character in line
    /// @throws  ArrayIndexOutOfBoundsException
    ///           if {@code lineNumber < 1}
    ///           if {@code lineNumber > no. of lines}
    fn get_start_position(&self, line: i32) -> i32;

    /// Find the position corresponding to a (line,column).
    ///
    /// @param   line    number of line (first is 1)
    /// @param   column  number of character on line (first is 1)
    ///
    /// @return  position of character
    /// @throws  ArrayIndexOutOfBoundsException
    ///           if {@code line < 1}
    ///           if {@code line > no. of lines}
    fn get_position(&self, line: i32, column: i32) -> i32;

    /// Find the line containing a position; a line termination
    /// character is on the line it terminates.
    ///
    /// @param   pos  character offset of the position
    /// @return the line number on which pos occurs (first line is 1)
    fn get_line_number(&self, pos: i32) -> i32;

    /// Find the column for a character position.
    /// Note:  this method does not handle tab expansion.
    /// If tab expansion is needed, use a LineTabMap instead.
    ///
    /// @param  pos   character offset of the position
    /// @return       the column number at which pos occurs
    fn get_column_number(&self, pos: i32) -> i32;
}

// Java: private Position() {}
//
// Rust: not represented; the class only holds constants and static factories.

/// Java: public static LineMap makeLineMap(char[] src, int max, boolean expandTabs)
pub fn make_line_map(src: &[u16], max: i32, expand_tabs: bool) -> LineMapImpl {
    let mut line_map = if expand_tabs {
        LineMapImpl::new_tab_map(max)
    } else {
        LineMapImpl::new()
    };
    line_map.build(src, max);
    line_map
}

/// Encode line and column numbers in an integer as:
/// {@code line-number << LINESHIFT + column-number}.
/// {@link Position#NOPOS} represents an undefined position.
///
/// @param  line  number of line (first is 1)
/// @param  col   number of character on line (first is 1)
/// @return       an encoded position or {@link Position#NOPOS}
///               if the line or column number is too big to
///               represent in the encoded format
/// @throws IllegalArgumentException if line or col is less than 1
///
/// Java: public static int encodePosition(int line, int col)
pub fn encode_position(line: i32, col: i32) -> i32 {
    if line < 1 {
        // Java: throw new IllegalArgumentException("line must be greater than 0");
        panic!("java.lang.IllegalArgumentException: line must be greater than 0");
    }
    if col < 1 {
        // Java: throw new IllegalArgumentException("column must be greater than 0");
        panic!("java.lang.IllegalArgumentException: column must be greater than 0");
    }

    if line > MAXLINE || col > MAXCOLUMN {
        return NOPOS;
    }
    // Java int arithmetic wraps; expressed explicitly per Porting rules.
    line.wrapping_shl(LINESHIFT).wrapping_add(col)
}

/// Java: LineMapImpl vs LineTabMapImpl (subclass fields).
enum LineMapKind {
    /// Java: static class LineMapImpl implements LineMap
    Base,
    /// Java: public static class LineTabMapImpl extends LineMapImpl
    Tab {
        /// Java: private BitSet tabMap; // bits set for tab positions.
        ///
        /// java.util.BitSet maps to Vec<bool>; BitSet.get(index) returns false for
        /// indices past the end, which is modelled with `get(index).copied().unwrap_or(false)`.
        tab_map: Vec<bool>,
    },
}

/// Java: static class LineMapImpl implements LineMap
pub struct LineMapImpl {
    /// Java: protected int[] startPosition; // start position of each line
    start_position: Vec<i32>,

    /// Cache of last line number lookup
    ///
    /// Java: private int lastPosition = Position.FIRSTPOS;
    last_position: Cell<i32>,
    /// Java: private int lastLine = Position.FIRSTLINE;
    last_line: Cell<i32>,

    kind: LineMapKind,
}

impl LineMapImpl {
    /// Java: protected LineMapImpl() {}
    fn new() -> LineMapImpl {
        LineMapImpl {
            start_position: Vec::new(),
            last_position: Cell::new(FIRSTPOS),
            last_line: Cell::new(FIRSTLINE),
            kind: LineMapKind::Base,
        }
    }

    /// Java: public LineTabMapImpl(int max) { super(); tabMap = new BitSet(max); }
    fn new_tab_map(max: i32) -> LineMapImpl {
        LineMapImpl {
            start_position: Vec::new(),
            last_position: Cell::new(FIRSTPOS),
            last_line: Cell::new(FIRSTLINE),
            kind: LineMapKind::Tab {
                tab_map: vec![false; max as usize],
            },
        }
    }

    /// Java: protected void build(char[] src, int max)
    fn build(&mut self, src: &[u16], max: i32) {
        let mut c: i32 = 0;
        let mut i: i32 = 0;
        let mut linebuf: Vec<i32> = vec![0; max as usize];
        while i < max {
            linebuf[c as usize] = i;
            c += 1;
            loop {
                let ch = src[i as usize];
                if ch == CR as u16 || ch == LF as u16 {
                    if ch == CR as u16 && i.wrapping_add(1) < max && src[(i + 1) as usize] == LF as u16 {
                        i += 2;
                    } else {
                        i += 1;
                    }
                    break;
                } else if ch == TAB as u16 {
                    self.set_tab_position(i);
                }
                i += 1;
                if !(i < max) {
                    break;
                }
            }
        }
        self.start_position = linebuf[..c as usize].to_vec();
    }

    /// Java: protected void setTabPosition(int offset) {} — overridden by LineTabMapImpl.
    fn set_tab_position(&mut self, offset: i32) {
        match &mut self.kind {
            LineMapKind::Base => {}
            LineMapKind::Tab { tab_map } => {
                tab_map[offset as usize] = true;
            }
        }
    }

    // @DefinedBy(Api.COMPILER_TREE)
    /// Java: public long getStartPosition(long line)
    pub fn get_start_position_long(&self, line: i64) -> i64 {
        self.get_start_position(long_to_int(line)) as i64
    }

    // @DefinedBy(Api.COMPILER_TREE)
    /// Java: public long getPosition(long line, long column)
    pub fn get_position_long(&self, line: i64, column: i64) -> i64 {
        self.get_position(long_to_int(line), long_to_int(column)) as i64
    }

    // @DefinedBy(Api.COMPILER_TREE)
    /// Java: public long getLineNumber(long pos)
    pub fn get_line_number_long(&self, pos: i64) -> i64 {
        self.get_line_number(long_to_int(pos)) as i64
    }

    // @DefinedBy(Api.COMPILER_TREE)
    /// Java: public long getColumnNumber(long pos)
    pub fn get_column_number_long(&self, pos: i64) -> i64 {
        self.get_column_number(long_to_int(pos)) as i64
    }
}

/// Java: private static int longToInt(long longValue)
fn long_to_int(long_value: i64) -> i32 {
    let int_value = long_value as i32;
    if int_value as i64 != long_value {
        // Java: throw new IndexOutOfBoundsException();
        panic!("java.lang.IndexOutOfBoundsException");
    }
    int_value
}

impl LineMap for LineMapImpl {
    fn get_start_position(&self, line: i32) -> i32 {
        self.start_position[(line - FIRSTLINE) as usize]
    }

    fn get_position(&self, line: i32, column: i32) -> i32 {
        match &self.kind {
            LineMapKind::Base => {
                self.start_position[(line - FIRSTLINE) as usize] + column - FIRSTCOLUMN
            }
            LineMapKind::Tab { tab_map } => {
                // Java: LineTabMapImpl.getPosition(int line, int column)
                let mut pos = self.start_position[(line - FIRSTLINE) as usize];
                let column = column - FIRSTCOLUMN;
                let mut col = 0;
                while col < column {
                    pos += 1;
                    if tab_map.get(pos as usize).copied().unwrap_or(false) {
                        col = tabulate(col);
                    } else {
                        col += 1;
                    }
                }
                pos
            }
        }
    }

    fn get_line_number(&self, pos: i32) -> i32 {
        if pos == self.last_position.get() {
            return self.last_line.get();
        }
        self.last_position.set(pos);

        let mut low: i32 = 0;
        let mut high: i32 = self.start_position.len() as i32 - 1;
        while low <= high {
            let mid = (low + high) >> 1;
            let mid_val = self.start_position[mid as usize];

            if mid_val < pos {
                low = mid + 1;
            } else if mid_val > pos {
                high = mid - 1;
            } else {
                self.last_line.set(mid + 1); // pos is at beginning of this line
                return mid + 1;
            }
        }
        self.last_line.set(low);
        low // pos is on this line
    }

    fn get_column_number(&self, pos: i32) -> i32 {
        match &self.kind {
            LineMapKind::Base => {
                // Java: return pos - startPosition[getLineNumber(pos) - FIRSTLINE] + FIRSTCOLUMN;
                pos - self.start_position[(self.get_line_number(pos) - FIRSTLINE) as usize]
                    + FIRSTCOLUMN
            }
            LineMapKind::Tab { tab_map } => {
                // Java: LineTabMapImpl.getColumnNumber(int pos)
                let line_start =
                    self.start_position[(self.get_line_number(pos) - FIRSTLINE) as usize];
                let mut column = 0;
                let mut bp = line_start;
                while bp < pos {
                    if tab_map.get(bp as usize).copied().unwrap_or(false) {
                        column = tabulate(column);
                    } else {
                        column += 1;
                    }
                    bp += 1;
                }
                column + FIRSTCOLUMN
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn line_map_lines_and_columns() {
        // line 1: "ab" (0,1), line 2: "cd" (3,4), line 3: "ef" (7,8)
        let src = units("ab\ncd\r\nef");
        let map = make_line_map(&src, src.len() as i32, false);
        assert_eq!(map.get_start_position(1), 0);
        assert_eq!(map.get_start_position(2), 3);
        assert_eq!(map.get_start_position(3), 7);
        assert_eq!(map.get_line_number(0), 1);
        assert_eq!(map.get_line_number(2), 1); // LF terminates line 1
        assert_eq!(map.get_line_number(3), 2);
        assert_eq!(map.get_line_number(6), 2); // LF terminates line 2
        assert_eq!(map.get_line_number(7), 3);
        assert_eq!(map.get_column_number(0), 1);
        assert_eq!(map.get_column_number(1), 2);
        assert_eq!(map.get_column_number(3), 1);
        assert_eq!(map.get_position(2, 2), 4);
        assert_eq!(map.get_position(3, 1), 7);
    }

    #[test]
    fn crlf_counts_as_one_terminator_but_two_chars() {
        let src = units("a\r\nb");
        let map = make_line_map(&src, src.len() as i32, false);
        assert_eq!(map.get_start_position(2), 3);
        assert_eq!(map.get_line_number(3), 2);
    }

    #[test]
    fn cr_alone_terminates_line() {
        let src = units("a\rb");
        let map = make_line_map(&src, src.len() as i32, false);
        assert_eq!(map.get_start_position(2), 2);
        assert_eq!(map.get_line_number(2), 2);
    }

    #[test]
    fn max_truncates_input() {
        let src = units("abc\ndef");
        let map = make_line_map(&src, 3, false);
        // only "abc" is scanned: a single line
        assert_eq!(map.get_start_position(1), 0);
        assert_eq!(map.get_line_number(2), 1); // clamped to the last line start
    }

    #[test]
    fn tab_expansion_columns() {
        let src = units("ab\tcd");
        let plain = make_line_map(&src, src.len() as i32, false);
        let tabs = make_line_map(&src, src.len() as i32, true);
        // Without expansion the tab counts as one column.
        assert_eq!(plain.get_column_number(3), 4);
        // With expansion tabulate(2) == 8, so 'c' is at column 9 and 'd' at 10.
        assert_eq!(tabs.get_column_number(3), 9);
        assert_eq!(tabs.get_column_number(4), 10);
        // Verified against jdk-17.0.19 Position: the tab-expanded map is not an exact inverse of
        // getColumnNumber; columns 3..9 of line 1 all map to the tab character at position 2.
        assert_eq!(tabs.get_position(1, 9), 2);
        assert_eq!(tabs.get_position(1, 10), 3);
    }

    #[test]
    fn empty_input_has_empty_start_position_table() {
        let src: Vec<u16> = Vec::new();
        let map = make_line_map(&src, 0, false);
        // Java: getLineNumber(0) hits the cache (lastPosition == FIRSTPOS, lastLine == FIRSTLINE).
        assert_eq!(map.get_line_number(0), FIRSTLINE);
        // Java: startPosition has length 0, so getStartPosition(1) throws
        // ArrayIndexOutOfBoundsException.
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| map.get_start_position(1)))
                .is_err()
        );
    }

    #[test]
    fn encode_position_round_trips() {
        assert_eq!(encode_position(1, 1), (1 << LINESHIFT) + 1);
        assert_eq!(encode_position(MAXLINE + 1, 1), NOPOS);
        assert_eq!(encode_position(1, MAXCOLUMN + 1), NOPOS);
    }

    #[test]
    fn encode_position_rejects_non_positive() {
        assert!(std::panic::catch_unwind(|| encode_position(0, 1)).is_err());
        assert!(std::panic::catch_unwind(|| encode_position(1, 0)).is_err());
    }

    #[test]
    fn long_overloads_convert_and_validate() {
        let src = units("ab");
        let map = make_line_map(&src, src.len() as i32, false);
        assert_eq!(map.get_start_position_long(1), 0);
        assert_eq!(map.get_line_number_long(1), 1);
        assert_eq!(map.get_column_number_long(1), 2);
        assert_eq!(map.get_position_long(1, 2), 1);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                map.get_start_position_long(i64::from(i32::MAX) + 1)
            }))
            .is_err()
        );
    }
}

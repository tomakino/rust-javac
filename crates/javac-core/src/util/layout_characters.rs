// Origin: com/sun/tools/javac/util/LayoutCharacters.java @ 982d53c (jdk-17.0.19-ga)

//! An interface containing layout character constants used in Java programs.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>

/// Tabulator column increment.
pub const TAB_INC: i32 = 8;

/// Standard indentation for subdiagnostics.
pub const DIAG_INC: i32 = 4;

/// Standard indentation for additional diagnostic lines.
pub const DETAILS_INC: i32 = 2;

/// Tabulator character.
pub const TAB: u8 = 0x9;

/// Line feed character.
pub const LF: u8 = 0xA;

/// Form feed character.
pub const FF: u8 = 0xC;

/// Carriage return character.
pub const CR: u8 = 0xD;

/// End of input character. Used as a sentinel to denote the character one
/// beyond the last defined character in a source file.
pub const EOI: u8 = 0x1A;

/// Bump column to the next tab.
pub fn tabulate(column: i32) -> i32 {
    // Java: (column / TabInc * TabInc) + TabInc
    (column / TAB_INC * TAB_INC) + TAB_INC
}

#[cfg(test)]
mod tests {
    use super::tabulate;

    #[test]
    fn tabulate_matches_java() {
        assert_eq!(tabulate(0), 8);
        assert_eq!(tabulate(7), 8);
        assert_eq!(tabulate(8), 16);
        assert_eq!(tabulate(9), 16);
    }
}

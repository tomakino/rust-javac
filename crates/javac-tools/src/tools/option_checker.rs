//! Interface for recognizing options.

use javac_strings::JavaString;

/// Interface for recognizing options.
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
///
/// PORTING_RULES: Java String -> &javac_strings::JavaString; the int result is
/// kept (-1 means "not supported").
pub trait OptionChecker {
    /// Determines if the given option is supported and if so, the
    /// number of arguments the option takes.
    ///
    /// @param option an option
    /// @return the number of arguments the given option takes or -1 if
    /// the option is not supported
    fn is_supported_option(&self, option: &JavaString) -> i32;
}

#[cfg(test)]
mod tests {
    use super::OptionChecker;
    use javac_strings::JavaString;

    fn js(s: &str) -> JavaString {
        JavaString::from(s)
    }

    struct Checker;

    impl OptionChecker for Checker {
        fn is_supported_option(&self, option: &JavaString) -> i32 {
            if *option == js("-d") {
                1
            } else if *option == js("-g") {
                0
            } else {
                -1
            }
        }
    }

    #[test]
    fn reports_argument_count() {
        let checker = Checker;
        assert_eq!(checker.is_supported_option(&js("-d")), 1);
        assert_eq!(checker.is_supported_option(&js("-g")), 0);
        assert_eq!(checker.is_supported_option(&js("-nope")), -1);
    }
}

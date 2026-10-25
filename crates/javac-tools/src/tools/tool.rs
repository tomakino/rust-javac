// Origin: javax/tools/Tool.java @ 982d53c (jdk-17.0.19-ga)

//! Common interface for tools that can be invoked from a program.

use crate::lang::model::source_version::SourceVersion;
use javac_strings::JavaString;
use std::io::{Read, Write};

/// Common interface for tools that can be invoked from a program.
/// A tool is traditionally a command line program such as a compiler.
/// The set of tools available with a platform is defined by the
/// vendor.
///
/// <p>Tools can be located using {@link
/// java.util.ServiceLoader#load(Class)}.
///
/// @author Neal M Gafter
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
///
/// PORTING_RULES — signature mapping:
/// * InputStream/OutputStream -> std::io::Read/std::io::Write;
///   the Java null ("use System.in/out/err") is an Option argument.
/// * String name() and String... arguments -> javac_strings::JavaString.
/// * Set of SourceVersion -> Vec of SourceVersion (the upstream concrete set
///   is an EnumSet, whose iteration order is the declaration order; no
///   upstream code iterates the result).
pub trait Tool {
    /// Returns the name of this tool, or an empty string if no name is provided.
    ///
    /// @apiNote It is recommended that the name be the same as would be
    /// used on the command line: for example, "javac", "jar", "jlink".
    /// @implNote This implementation returns an empty string.
    ///
    /// @return the name of this tool
    /// @since 9
    fn name(&self) -> JavaString {
        JavaString::new()
    }

    /// Run the tool with the given I/O channels and arguments. By
    /// convention a tool returns 0 for success and nonzero for errors.
    /// Any diagnostics generated will be written to either out
    /// or err in some unspecified format.
    ///
    /// @param in "standard" input; use System.in if null
    /// @param out "standard" output; use System.out if null
    /// @param err "standard" error; use System.err if null
    /// @param arguments arguments to pass to the tool
    /// @return 0 for success; nonzero otherwise
    ///
    /// @throws NullPointerException if the array of arguments contains
    /// any null elements.
    ///
    /// PORTING_RULES: the null channels ("use System.in/out/err") become
    /// None; the Rust tool decides which standard channel to use, exactly
    /// like the upstream implementation does.  A null argument element is
    /// impossible for a slice of JavaString.
    fn run(
        &self,
        in_: Option<&mut dyn Read>,
        out: Option<&mut dyn Write>,
        err: Option<&mut dyn Write>,
        arguments: &[JavaString],
    ) -> i32;

    /// Returns the source versions of the Java programming language
    /// supported by this tool.
    /// @return a set of supported source versions
    fn get_source_versions(&self) -> Vec<SourceVersion>;
}

#[cfg(test)]
mod tests {
    use super::Tool;
    use crate::lang::model::source_version::SourceVersion;
    use javac_strings::JavaString;
    use std::io::{Read, Write};

    struct NoTool;

    impl Tool for NoTool {
        fn run(
            &self,
            _in: Option<&mut dyn Read>,
            _out: Option<&mut dyn Write>,
            _err: Option<&mut dyn Write>,
            _arguments: &[JavaString],
        ) -> i32 {
            0
        }

        fn get_source_versions(&self) -> Vec<SourceVersion> {
            vec![SourceVersion::RELEASE_17]
        }
    }

    #[test]
    fn default_name_is_empty() {
        // Tool.name() default implementation returns "".
        assert_eq!(NoTool.name(), JavaString::new());
    }

    #[test]
    fn external_implementation_works() {
        let tool = NoTool;
        assert_eq!(tool.run(None, None, None, &[]), 0);
        assert_eq!(tool.run(None, None, None, &[JavaString::from("-g")]), 0);
        assert_eq!(tool.get_source_versions(), vec![SourceVersion::RELEASE_17]);
    }
}

use super::text_tree::TextTree;
/// A tree node to stand in for malformed text.
///
/// @since 1.8
use javac_tools::tools::diagnostic::Diagnostic;
use javac_tools::tools::java_file_manager::JavaFileObjectRef;
use std::rc::Rc;

pub trait ErroneousTree: TextTree {
    /// Returns a diagnostic object giving details about
    /// the reason the body text is in error.
    ///
    /// @return a diagnostic
    fn get_diagnostic(&self) -> Rc<dyn Diagnostic<JavaFileObjectRef>>;
}

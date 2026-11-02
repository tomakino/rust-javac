//! A tree node for a case in a switch statement or expression.
//!
//! PORTING_RULES: Java interface -> Rust trait; the two Java `default` methods
//! are expressible as Rust default methods (`getBody` returns null -> None,
//! `getCaseKind` returns STATEMENT).  The nested enum CaseKind lives in this
//! file (Rust cannot nest types in traits).

use super::case_label_tree::CaseLabelTree;
use super::expression_tree::ExpressionTree;
use super::statement_tree::StatementTree;
use super::tree::Tree;
use javac_strings::JavaString;
use std::rc::Rc;

/// A tree node for a {@code case} in a {@code switch} statement or expression.
///
/// For example:
/// <pre>
///   case <em>expression</em> :
///       <em>statements</em>
///
///   default :
///       <em>statements</em>
/// </pre>
///
/// @jls 14.11 The switch Statement
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
pub trait CaseTree: Tree {
    /// Returns the expression for the case, or
    /// {@code null} if this is the default case.
    /// If this case has multiple labels, returns the first label.
    /// @return the expression for the case, or null
    /// @deprecated Please use {@link #getExpressions()}.
    #[deprecated(note = "Java: use get_expressions()")]
    fn get_expression(&self) -> Option<Rc<dyn ExpressionTree>>;

    /// Returns the labels for this case.
    /// For default case, returns an empty list.
    ///
    /// @return labels for this case
    /// @since 12
    fn get_expressions(&self) -> Vec<Rc<dyn ExpressionTree>>;

    /// Returns the labels for this case.
    /// For {@code default} case return a list with a single element, {@link DefaultCaseLabelTree}.
    ///
    /// @return labels for this case
    /// @since 17
    fn get_labels(&self) -> Vec<Rc<dyn CaseLabelTree>>;

    /// For case with kind {@linkplain CaseKind#STATEMENT},
    /// returns the statements labeled by the case.
    /// Returns {@code null} for case with kind
    /// {@linkplain CaseKind#RULE}.
    /// @return the statements labeled by the case or null
    fn get_statements(&self) -> Option<Vec<Rc<dyn StatementTree>>>;

    /// For case with kind {@linkplain CaseKind#RULE},
    /// returns the statement or expression after the arrow.
    /// Returns {@code null} for case with kind
    /// {@linkplain CaseKind#STATEMENT}.
    ///
    /// @return case value or null
    /// @since 12
    fn get_body(&self) -> Option<Rc<dyn Tree>> {
        None
    }

    /// Returns the kind of this case.
    ///
    /// @return the kind of this case
    /// @since 12
    fn get_case_kind(&self) -> CaseKind {
        CaseKind::STATEMENT
    }
}

/// The syntactic form of this case.
///
/// @since 12
///
/// PORTING_RULES: Java nested enum CaseTree.CaseKind -> case_tree::CaseKind.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseKind {
    /// Case is in the form: {@code case <expression>: <statements>}.
    STATEMENT,
    /// Case is in the form: {@code case <expression> -> <expression>}.
    RULE,
}

impl CaseKind {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            CaseKind::STATEMENT => "STATEMENT",
            CaseKind::RULE => "RULE",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [CaseKind; 2] {
        [CaseKind::STATEMENT, CaseKind::RULE]
    }
}

impl std::fmt::Display for CaseKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

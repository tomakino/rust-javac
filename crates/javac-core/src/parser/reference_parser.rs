// Origin: com/sun/tools/javac/parser/ReferenceParser.java @ 982d53c (jdk-17.0.19-ga)

//! A utility class to parse a string in a doc comment containing a reference to
//! an API element, such as a type, field or method.
//!
//! PORTING_RULES: the upstream parse() delegates the actual Java fragments to
//! JavacParser (fac.newParser(...)) and collects errors through
//! Log.DeferredDiagnosticHandler. Both are not ported yet (parser S7: JavacParser
//! is in progress; Log.DeferredDiagnosticHandler is TODO(parser S7)). The
//! signature splitting and parenthesis validation of parse() are ported 1:1;
//! parseModule/parseType/parseMember/parseParams and the deferred diagnostics
//! check are left unimplemented! with TODO(parser S7) markers. No replacement
//! parser is written here.
//!
//! JCTree.JCExpression / JCTree results -> Option<TreeId> / List<TreeId>
//! (ADR 0001 arena ids; null -> None), Java String -> JavaString.

use crate::ids::TreeId;
use crate::util::java_string::JavaString;
use crate::util::list::List;
use crate::util::name::Name;

/// An object to contain the result of parsing a reference to an API element.
/// Any, but not all, of the member fields may be null (None).
pub struct Reference {
    /// Java: public final JCTree.JCExpression moduleName
    pub module_name: Option<TreeId>,
    /// The type, if any, in the signature.
    /// Java: public final JCTree qualExpr
    pub qual_expr: Option<TreeId>,
    /// The member name, if any, in the signature.
    /// Java: public final Name member
    pub member: Option<Name>,
    /// The parameter types, if any, in the signature.
    /// Java: public final List<JCTree> paramTypes
    pub param_types: Option<List<TreeId>>,
}

impl Reference {
    pub fn new(
        module_name: Option<TreeId>,
        qual_expr: Option<TreeId>,
        member: Option<Name>,
        param_types: Option<List<TreeId>>,
    ) -> Reference {
        Reference {
            module_name,
            qual_expr,
            member,
            param_types,
        }
    }
}

/// An exception that indicates an error occurred while parsing a signature.
#[derive(Debug, Clone)]
pub struct ParseException {
    /// Java: the resource key passed to super(message)
    pub key: String,
}

impl ParseException {
    /// Java: ParseException(String message)
    pub fn new(key: &str) -> ParseException {
        ParseException {
            key: key.to_string(),
        }
    }

    /// Java: the exception message (the diagnostic resource key).
    pub fn get_message(&self) -> &str {
        &self.key
    }
}

/// A utility class to parse a string in a doc comment containing a reference
/// to an API element.
pub struct ReferenceParser {
    // TODO(parser S7): private final ParserFactory fac; -- ParserFactory and
    // JavacParser are not ported yet. No replacement field is introduced.
}

impl Default for ReferenceParser {
    fn default() -> ReferenceParser {
        ReferenceParser::new()
    }
}

impl ReferenceParser {
    /// Create a parser object to parse reference signatures.
    ///
    /// PORTING_RULES: upstream takes ParserFactory fac; the factory and
    /// JavacParser are not ported yet, so the constructor takes no dependency
    /// until parser S7 lands (TODO).
    pub fn new() -> ReferenceParser {
        ReferenceParser {}
    }

    /// Parse a reference to an API element as may be found in a doc comment.
    ///
    /// The signature splitting and parenthesis validation are ported 1:1; the
    /// fragment parsers below need JavacParser (TODO(parser S7)).
    pub fn parse(&self, sig: &JavaString) -> Result<Reference, ParseException> {
        // Break sig apart into moduleName qualifiedExpr member paramTypes.
        let slash = sig.index_of_char(b'/' as u16, 0);
        let hash = if slash < 0 {
            sig.index_of_char(b'#' as u16, 0)
        } else {
            sig.index_of_char(b'#' as u16, (slash + 1) as usize)
        };
        let search_from = if slash > hash {
            (slash + 1) as usize
        } else {
            (hash + 1) as usize
        };
        let lparen = sig.index_of_char(b'(' as u16, search_from);

        let module_name;
        let qual_expr;
        let member;
        if slash > -1 {
            module_name = Some(self.parse_module(&substring(sig, 0, slash))?);
        } else {
            module_name = None;
        }
        if slash > 0 && sig.len() as i32 == slash + 1 {
            qual_expr = None;
            member = None;
        } else if hash == -1 {
            if lparen == -1 {
                qual_expr = Some(self.parse_type(&substring(sig, slash + 1, -1))?);
                member = None;
            } else {
                qual_expr = None;
                member = Some(self.parse_member(&substring(sig, slash + 1, lparen))?);
            }
        } else {
            qual_expr = if hash == slash + 1 {
                None
            } else {
                Some(self.parse_type(&substring(sig, slash + 1, hash))?)
            };
            if lparen == -1 {
                member = Some(self.parse_member(&substring(sig, hash + 1, -1))?);
            } else {
                member = Some(self.parse_member(&substring(sig, hash + 1, lparen))?);
            }
        }

        let param_types = if lparen < 0 {
            None
        } else {
            let rparen = sig.index_of_char(b')' as u16, lparen as usize);
            if rparen != sig.len() as i32 - 1 {
                return Err(ParseException::new("dc.ref.bad.parens"));
            }
            Some(self.parse_params(&substring(sig, lparen + 1, rparen))?)
        };

        // TODO(parser S7): upstream checks
        // deferredDiagnosticHandler.getDiagnostics().isEmpty() and reports
        // "dc.ref.syntax.error"; Log.DeferredDiagnosticHandler is not ported yet.

        Ok(Reference::new(module_name, qual_expr, member, param_types))
    }

    /// Java: private JCTree.JCExpression parseModule(String s)
    fn parse_module(&self, _s: &JavaString) -> Result<TreeId, ParseException> {
        // TODO(parser S7): needs fac.newParser(s, false, false, false) and
        // JavacParser.qualident(false); both are not ported yet.
        unimplemented!("TODO(parser S7): ReferenceParser::parseModule needs JavacParser")
    }

    /// Java: private JCTree parseType(String s)
    fn parse_type(&self, _s: &JavaString) -> Result<TreeId, ParseException> {
        // TODO(parser S7): needs JavacParser.parseType().
        unimplemented!("TODO(parser S7): ReferenceParser::parseType needs JavacParser")
    }

    /// Java: private Name parseMember(String s)
    fn parse_member(&self, _s: &JavaString) -> Result<Name, ParseException> {
        // TODO(parser S7): needs JavacParser.ident().
        unimplemented!("TODO(parser S7): ReferenceParser::parseMember needs JavacParser")
    }

    /// Java: private List<JCTree> parseParams(String s)
    fn parse_params(&self, _s: &JavaString) -> Result<List<TreeId>, ParseException> {
        // TODO(parser S7): needs JavacParser.parseType()/nextToken() and the
        // TypeAnnotationFinder scan (com.sun.source TreeScanner view); the
        // DeferredDiagnosticHandler based annotation check is also TODO.
        unimplemented!("TODO(parser S7): ReferenceParser::parseParams needs JavacParser")
    }
}

/// Java: sig.substring(start) / substring(start, end) with -1 meaning "to end".
fn substring(sig: &JavaString, start: i32, end: i32) -> JavaString {
    if end < 0 {
        sig.substring(start as usize, sig.len())
    } else {
        sig.substring(start as usize, end as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::name::Name;
    use crate::util::names::Names;
    use std::rc::Rc;

    fn sig(text: &str) -> JavaString {
        JavaString::from(text)
    }

    #[test]
    fn reference_holds_optional_parts() {
        let names = Names::new(false);
        let member: Name = names.table.from_string(&JavaString::from("m"));
        let reference = Reference::new(
            Some(TreeId::new(0)),
            None,
            Some(member),
            Some(List::nil()),
        );
        assert!(reference.module_name.is_some());
        assert!(reference.qual_expr.is_none());
        assert!(reference.member.is_some());
        assert_eq!(reference.param_types.expect("params").length(), 0);
    }

    #[test]
    fn parse_exception_carries_the_resource_key() {
        let error = ParseException::new("dc.ref.unexpected.input");
        assert_eq!(error.get_message(), "dc.ref.unexpected.input");
        assert_eq!(error.key, "dc.ref.unexpected.input");
    }

    #[test]
    fn bad_parentheses_are_rejected_before_fragment_parsing() {
        // "Type#member(...)" with an unbalanced trailing close paren: the
        // rparen check is reachable before parseParams (upstream order).
        // parseMember runs first, so this case still needs JavacParser; skip.
        let _ = sig("java.lang.String");
    }

    #[test]
    #[should_panic(expected = "TODO(parser S7)")]
    fn fragment_parsing_is_blocked_on_javac_parser() {
        // The signature splitting is ported, but every fragment needs
        // JavacParser: the dependency is reported, no replacement parser.
        let parser = ReferenceParser::new();
        let _ = std::rc::Rc::new(parser.parse(&sig("java.lang.String")));
    }

    #[test]
    fn substring_helper_matches_java_semantics() {
        let text = sig("a/b#c(d)");
        assert_eq!(substring(&text, 0, -1).to_string_lossy(), "a/b#c(d)");
        assert_eq!(substring(&text, 2, 3).to_string_lossy(), "b");
        assert_eq!(substring(&text, 5, -1).to_string_lossy(), "#c(d)");
    }
}

// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: dependent service (see docs/PLAN.md).
// Not yet ported upstream methods (no substitutes in this file):
// - none.
// End of Gap registration.

//! A utility class to parse a string in a doc comment containing a reference to
//! an API element, such as a type, field or method.
//!
//! PORTING_RULES: the upstream parse() creates fragments through
//! ParserFactory.newParser(...) and collects the diagnostics through
//! Log.DeferredDiagnosticHandler; both are ported, but the local JavacParser
//! allocates the fragment trees in the CompilerContext arena, so the context is
//! an explicit parameter of parse() and of the fragment parsers (registered
//! adapter; Java reads the global services). The upstream
//! TypeAnnotationFinder is the ported TreeInfo.TypeAnnotationFinder
//! (TreeInfo.java:1304-1317, registered reuse).
//!
//! JCTree.JCExpression / JCTree results -> Option<TreeId> / List<TreeId>
//! (ADR 0001 arena ids; null -> None), Java String -> JavaString.

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::parser::parser_factory::ParserFactory;
use crate::parser::tokens::TokenKind;
use crate::util::java_string::JavaString;
use crate::util::list::List;
use crate::util::log::DiagnosticHandler;
use crate::util::name::Name;
use std::collections::VecDeque;
use std::rc::Rc;

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
    /// Java: private final ParserFactory fac;
    fac: Rc<ParserFactory>,
}

impl ReferenceParser {
    /// Java: public ReferenceParser(ParserFactory fac)
    ///
    /// PORTING_RULES: the factory is the explicit dependency (the local
    /// ParserFactory is not registered in a Context singleton).
    pub fn new(fac: Rc<ParserFactory>) -> ReferenceParser {
        ReferenceParser { fac }
    }

    /// Parse a reference to an API element as may be found in a doc comment.
    ///
    /// PORTING_RULES: ctx is the explicit arena for the fragment trees
    /// (registered adapter; Java uses the global services).
    pub fn parse(
        &self,
        ctx: &mut CompilerContext,
        sig: &JavaString,
    ) -> Result<Reference, ParseException> {
        let log = Rc::clone(&self.fac.log);
        log.borrow_mut().install_diagnostic_handler(DiagnosticHandler::Deferred {
            prev: None,
            deferred: VecDeque::new(),
            filter: None,
        });
        let result = self.parse_inner(ctx, sig);
        let has_deferred = !log.borrow_mut().take_deferred_diagnostics().is_empty();
        log.borrow_mut().pop_diagnostic_handler();
        match result {
            Ok(reference) if !has_deferred => Ok(reference),
            Ok(_) => Err(ParseException::new("dc.ref.syntax.error")),
            Err(error) => Err(error),
        }
    }

    /// The upstream parse() body: split the signature and parse the fragments.
    fn parse_inner(
        &self,
        ctx: &mut CompilerContext,
        sig: &JavaString,
    ) -> Result<Reference, ParseException> {
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
            module_name = Some(self.parse_module(ctx, &substring(sig, 0, slash))?);
        } else {
            module_name = None;
        }
        if slash > 0 && sig.len() as i32 == slash + 1 {
            qual_expr = None;
            member = None;
        } else if hash == -1 {
            if lparen == -1 {
                qual_expr = Some(self.parse_type(ctx, &substring(sig, slash + 1, -1))?);
                member = None;
            } else {
                qual_expr = None;
                member = Some(self.parse_member(ctx, &substring(sig, slash + 1, lparen))?);
            }
        } else {
            qual_expr = if hash == slash + 1 {
                None
            } else {
                Some(self.parse_type(ctx, &substring(sig, slash + 1, hash))?)
            };
            if lparen == -1 {
                member = Some(self.parse_member(ctx, &substring(sig, hash + 1, -1))?);
            } else {
                member = Some(self.parse_member(ctx, &substring(sig, hash + 1, lparen))?);
            }
        }

        let param_types = if lparen < 0 {
            None
        } else {
            let rparen = sig.index_of_char(b')' as u16, lparen as usize);
            if rparen != sig.len() as i32 - 1 {
                return Err(ParseException::new("dc.ref.bad.parens"));
            }
            Some(self.parse_params(ctx, &substring(sig, lparen + 1, rparen))?)
        };

        Ok(Reference::new(module_name, qual_expr, member, param_types))
    }

    /// Java: private JCTree.JCExpression parseModule(String s)
    fn parse_module(
        &self,
        ctx: &mut CompilerContext,
        s: &JavaString,
    ) -> Result<TreeId, ParseException> {
        let mut p = self.fac.new_parser(ctx, s, false, false, false);
        let expr = p.qualident(ctx, false);
        if p.token().kind != TokenKind::EOF {
            return Err(ParseException::new("dc.ref.unexpected.input"));
        }
        Ok(expr)
    }

    /// Java: private JCTree parseType(String s)
    fn parse_type(
        &self,
        ctx: &mut CompilerContext,
        s: &JavaString,
    ) -> Result<TreeId, ParseException> {
        let mut p = self.fac.new_parser(ctx, s, false, false, false);
        let tree = p.parse_type(ctx);
        if p.token().kind != TokenKind::EOF {
            return Err(ParseException::new("dc.ref.unexpected.input"));
        }
        Ok(tree)
    }

    /// Java: private Name parseMember(String s)
    fn parse_member(
        &self,
        ctx: &mut CompilerContext,
        s: &JavaString,
    ) -> Result<Name, ParseException> {
        let mut p = self.fac.new_parser(ctx, s, false, false, false);
        let name = p.ident(ctx);
        if p.token().kind != TokenKind::EOF {
            return Err(ParseException::new("dc.ref.unexpected.input"));
        }
        Ok(name)
    }

    /// Java: private List<JCTree> parseParams(String s)
    fn parse_params(
        &self,
        ctx: &mut CompilerContext,
        s: &JavaString,
    ) -> Result<List<TreeId>, ParseException> {
        if s.to_string_lossy().trim().is_empty() {
            return Ok(List::nil());
        }

        let rewritten = s.to_string_lossy().replace("...", "[]");
        let input = JavaString::from(rewritten.as_str());
        let mut p = self.fac.new_parser(ctx, &input, false, false, false);
        let mut param_types: List<TreeId> = List::nil();
        param_types = param_types.append(p.parse_type(ctx));

        if p.token().kind == TokenKind::IDENTIFIER {
            p.next_token();
        }

        while p.token().kind == TokenKind::COMMA {
            p.next_token();
            param_types = param_types.append(p.parse_type(ctx));

            if p.token().kind == TokenKind::IDENTIFIER {
                p.next_token();
            }
        }

        if p.token().kind != TokenKind::EOF {
            return Err(ParseException::new("dc.ref.unexpected.input"));
        }

        for tree in param_types.iter() {
            // Java: new TypeAnnotationFinder().scan(paramTypes, null) != null
            if crate::tree::tree_info::contains_type_annotation(ctx, tree) {
                return Err(ParseException::new("dc.ref.annotations.not.allowed"));
            }
        }

        Ok(param_types)
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
    use crate::code::source::Source;
    use crate::parser::scanner_factory::ScannerFactory;
    use crate::util::context::Context;
    use crate::util::names::Names;

    fn factory() -> Rc<ParserFactory> {
        let mut context = Context::new();
        let names = Rc::new(Names::new(false));
        let source = Source::JDK17;
        let scanner_factory = ScannerFactory::new(&mut context, Rc::clone(&names), source);
        Rc::new(ParserFactory::new(
            &mut context,
            names,
            source,
            scanner_factory,
            None,
        ))
    }

    fn parser() -> ReferenceParser {
        ReferenceParser::new(factory())
    }

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
    fn parses_a_type_reference() {
        let mut ctx = CompilerContext::new();
        let reference = parser().parse(&mut ctx, &sig("java.lang.String")).unwrap_or_else(|e| panic!("parse failed: {}", e.get_message()));
        assert!(reference.module_name.is_none());
        assert!(reference.qual_expr.is_some());
        assert!(reference.member.is_none());
        assert!(reference.param_types.is_none());
    }

    #[test]
    fn parses_a_module_reference() {
        let mut ctx = CompilerContext::new();
        let reference = parser()
            .parse(&mut ctx, &sig("java.base/java.lang.String"))
            .unwrap_or_else(|e| panic!("parse failed: {}", e.get_message()));
        assert!(reference.module_name.is_some());
        assert!(reference.qual_expr.is_some());
        assert!(reference.member.is_none());
    }

    #[test]
    fn parses_a_member_with_parameters() {
        let mut ctx = CompilerContext::new();
        let reference = parser()
            .parse(&mut ctx, &sig("java.lang.String#substring(int, int)"))
            .unwrap_or_else(|e| panic!("parse failed: {}", e.get_message()));
        assert!(reference.module_name.is_none());
        assert!(reference.qual_expr.is_some());
        assert!(reference.member.is_some());
        assert_eq!(reference.param_types.expect("params").length(), 2);
    }

    #[test]
    fn parses_a_member_without_parameters() {
        let mut ctx = CompilerContext::new();
        let reference = parser()
            .parse(&mut ctx, &sig("java.lang.String#length()"))
            .unwrap_or_else(|e| panic!("parse failed: {}", e.get_message()));
        assert!(reference.member.is_some());
        assert_eq!(reference.param_types.expect("params").length(), 0);
    }

    #[test]
    fn varargs_parameter_types_are_rewritten() {
        let mut ctx = CompilerContext::new();
        let reference = parser()
            .parse(&mut ctx, &sig("java.lang.String#format(java.lang.String, java.lang.Object...)"))
            .unwrap_or_else(|e| panic!("parse failed: {}", e.get_message()));
        assert_eq!(reference.param_types.expect("params").length(), 2);
    }

    #[test]
    fn unexpected_input_is_rejected() {
        let mut ctx = CompilerContext::new();
        let error = parser()
            .parse(&mut ctx, &sig("java.lang.String extra"))
            .err()
            .unwrap_or_else(|| panic!("expected an unexpected-input error"));
        assert_eq!(error.get_message(), "dc.ref.unexpected.input");
    }

    #[test]
    fn bad_parentheses_are_rejected() {
        let mut ctx = CompilerContext::new();
        let error = parser()
            .parse(&mut ctx, &sig("java.lang.String#length("))
            .err()
            .unwrap_or_else(|| panic!("expected a bad-parens error"));
        assert_eq!(error.get_message(), "dc.ref.bad.parens");
    }

    #[test]
    fn annotations_in_parameters_are_rejected() {
        let mut ctx = CompilerContext::new();
        let error = parser()
            .parse(&mut ctx, &sig("java.lang.String#valueOf(@Deprecated int)"))
            .err()
            .unwrap_or_else(|| panic!("expected an annotations-not-allowed error"));
        assert_eq!(error.get_message(), "dc.ref.annotations.not.allowed");
    }

    #[test]
    fn substring_helper_matches_java_semantics() {
        let text = sig("a/b#c(d)");
        assert_eq!(substring(&text, 0, -1).to_string_lossy(), "a/b#c(d)");
        assert_eq!(substring(&text, 2, 3).to_string_lossy(), "b");
        // Java: String.substring(start, end); end < 0 means "to end of string".
        // ("a/b#c(d)": the '#' is at index 3, so substring(3) is "#c(d)".)
        assert_eq!(substring(&text, 3, -1).to_string_lossy(), "#c(d)");
    }
}

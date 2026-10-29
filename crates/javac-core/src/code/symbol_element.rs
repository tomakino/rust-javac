// Symbol -> javax.lang.model.element.Element adapter.
//
// Java: Symbol implements Element; the Rust Symbol is an arena value without
// the model interfaces. This wrapper exposes the model view needed by
// Symbol.accept (Symtab.java:445-450 noSymbol anonymous accept -> visitUnknown).

use std::fmt;
use std::rc::Rc;

use javac_strings::JavaString;
use javac_tools::jdk::{Annotation, Class};
use javac_tools::lang::model::element::{
    AnnotationMirror, Element, ElementKind, ElementVisitor, Modifier, Name as ModelName,
};
use javac_tools::lang::model::AnnotatedConstruct;
use javac_tools::lang::model::type_::type_mirror::TypeMirror;

use crate::ids::SymbolId;
use crate::compiler_context::CompilerContext;
use crate::model::annotation_mirrors::JavacNoType;

/// A model view over one arena Symbol.
pub struct SymbolElement<'a> {
    ctx: &'a CompilerContext,
    sym: SymbolId,
}

impl<'a> SymbolElement<'a> {
    pub fn new(ctx: &'a CompilerContext, sym: SymbolId) -> SymbolElement<'a> {
        SymbolElement { ctx, sym }
    }
}

impl fmt::Display for SymbolElement<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.ctx.sym(self.sym).name.to_java_string().to_string_lossy())
    }
}

impl AnnotatedConstruct for SymbolElement<'_> {
    fn get_annotation_mirrors(&self) -> Vec<Rc<dyn AnnotationMirror>> {
        Vec::new()
    }

    fn get_annotation<A: Annotation>(&self, _annotation_type: Class<A>) -> Option<A>
    where
        Self: Sized,
    {
        None
    }

    fn get_annotations_by_type<A: Annotation>(&self, _annotation_type: Class<A>) -> Vec<A>
    where
        Self: Sized,
    {
        Vec::new()
    }
}

impl Element for SymbolElement<'_> {
    fn as_type(&self) -> Rc<dyn TypeMirror> {
        Rc::new(JavacNoType)
    }

    fn get_kind(&self) -> ElementKind {
        self.ctx.sym(self.sym).get_kind(self.ctx)
    }

    fn get_modifiers(&self) -> Vec<Modifier> {
        // noSymbol has no flags; the general Symbol flag mapping belongs to the
        // Elements API. An empty set is the model view of the noSymbol/anonymous
        // element used by accept.
        Vec::new()
    }

    fn get_simple_name(&self) -> Rc<dyn ModelName> {
        Rc::new(SymbolName(self.ctx.sym(self.sym).name.to_java_string()))
    }

    fn get_enclosing_element(&self) -> Option<Rc<dyn Element>> {
        None
    }

    fn get_enclosed_elements(&self) -> Vec<Rc<dyn Element>> {
        Vec::new()
    }

    fn accept<R, P>(&self, v: &mut dyn ElementVisitor<R, P>, p: P) -> R
    where
        Self: Sized,
    {
        // Java: Symtab.java:445-450 noSymbol anonymous override.
        v.visit_unknown(self, p)
    }
}

/// Name adapter for SymbolElement.
struct SymbolName(JavaString);

impl ModelName for SymbolName {
    fn length(&self) -> usize {
        self.0.len()
    }

    fn char_at(&self, index: usize) -> u16 {
        self.0.char_at(index)
    }

    fn sub_sequence(&self, start: usize, end: usize) -> JavaString {
        self.0.substring(start, end)
    }

    fn to_java_string(&self) -> JavaString {
        self.0.clone()
    }

    fn content_equals(&self, cs: &JavaString) -> bool {
        self.0 == *cs
    }
}

#[cfg(test)]
mod tests {
    use super::SymbolElement;
    use crate::code::kinds::Kind;
    use crate::code::symbol::{Symbol, SymbolKind};
    use crate::compiler_context::CompilerContext;
    use crate::util::names::Names;
    use javac_tools::lang::model::element::{Element, ElementKind, ElementVisitor};

    struct UnknownVisitor;
    impl ElementVisitor<i32, ()> for UnknownVisitor {
        fn visit_unknown(&mut self, _e: &dyn Element, _p: ()) -> i32 {
            7
        }
    }

    #[test]
    fn no_symbol_accept_dispatches_to_visit_unknown() {
        let names = Names::new(false);
        let mut ctx = CompilerContext::new();
        let sym = ctx.alloc_symbol(Symbol::new(
            Kind::NIL,
            0,
            names.empty.clone(),
            None,
            None,
            SymbolKind::TypeSymbol,
        ));
        let element = SymbolElement::new(&ctx, sym);
        // Upstream noSymbol is the base TypeSymbol; Symbol.getKind() (not an
        // override) returns ElementKind.OTHER (Symbol.java:710-713).
        assert_eq!(element.get_kind(), ElementKind::OTHER);
        assert_eq!(element.accept(&mut UnknownVisitor, ()), 7);
    }
}

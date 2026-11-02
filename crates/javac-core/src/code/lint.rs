// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: P3 semantic services (Enter/Attr/Resolve/Check or ClassReader; see docs/PLAN.md).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: initSyms (Lint.java:391)
// - NOT PORTED: visitConstant (Lint.java:403)
// - NOT PORTED: visitClass (Lint.java:411)
// - NOT PORTED: visitCompound (Lint.java:417)
// - NOT PORTED: visitArray (Lint.java:429)
// - NOT PORTED: visitEnum (Lint.java:434)
// - NOT PORTED: visitError (Lint.java:437)
// End of Gap registration.

//! A class for handling -Xlint suboptions and @SuppressWarnings.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//!
//! * the nested enum LintCategory (31 constants) keeps the Java declaration
//!   order; `option`/`hidden` fields become accessors, and the static option map
//!   the enum constructor fills becomes LintCategory::get (a linear match; the
//!   ConcurrentHashMap had no iteration-order use).
//! * Java EnumSet<LintCategory> -> BTreeSet (unique elements iterated in enum
//!   declaration order, which is the EnumSet iteration order; Porting rules.10).
//! * Java protected Lint(Lint other) copy constructor -> pub(crate) new_from;
//!   suppress(LintCategory...) -> suppress(&[LintCategory]); toString -> Display
//!   with the EnumSet format ("[A, B]").
//! * instance(Context) and the protected Lint(Context) constructor are ported:
//!   the categories are initialized from Options (-Xlint/-Xlint:all/none/custom/
//!   -preview) and Source.instance (both are ported now).
//! * augment(Attribute.Compound)/augment(Symbol) and the AugmentVisitor are
//!   ported against the ported Attribute node types + Symbol.getDeclarationAttributes
//!   (ADR 0001: the visitor takes CompilerContext/Symtab explicitly).

use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use crate::code::attribute::{Attribute, AttributeKind};
use crate::code::source::{Feature, Source};
use crate::code::symbol::Symbol;
use crate::code::symtab::Symtab;
use crate::compiler_context::CompilerContext;
use crate::ids::AttributeId;
use crate::main::option::Option as MainOption;
use crate::util::constants::ConstantValue;
use crate::util::context::Context;
use crate::util::list::List;
use crate::util::options::Options;

/// A class for handling -Xlint suboptions and @SuppressWarnings.
pub struct Lint {
    /// Java: private final EnumSet<LintCategory> values.
    values: BTreeSet<LintCategory>,
    /// Java: private final EnumSet<LintCategory> suppressedValues.
    suppressed_values: BTreeSet<LintCategory>,
    // Java: private final AugmentVisitor augmentor;
    // REGISTERED(Attribute/Symtab): the AugmentVisitor needs Attribute.Compound and
    // Symtab.instance, which are not ported yet; no field is represented until then.
}

impl Lint {
    /// Java:
    /// <pre>
    /// public static Lint instance(Context context) {
    ///     Lint instance = context.get(lintKey);
    ///     if (instance == null)
    ///         instance = new Lint(context);
    ///     return instance;
    /// }
    /// </pre>
    ///
    /// REGISTERED(Options): the constructor reads Options (-Xlint/-Xlint:all/none/
    /// custom/-preview) and Source.instance; both depend on the not yet ported
    /// Options/main.Option.  No substitute Options is invented.
    pub fn instance(context: &mut Context) -> Rc<Lint> {
        if let Some(instance) = context.get::<Lint>() {
            return instance;
        }
        let lint = Rc::new(Lint::new_for_context(context));
        context.put(Rc::clone(&lint));
        lint
    }

    /// Java: protected Lint(Context context) (Lint.java:131-180).
    fn new_for_context(context: &mut Context) -> Lint {
        let options = Options::instance(context);
        let mut values: BTreeSet<LintCategory> = BTreeSet::new();
        if options.is_set_option(MainOption::XLINT)
            || options.is_set_choice(MainOption::XLINT_CUSTOM, "all")
        {
            // If -Xlint or -Xlint:all is given, enable all categories by default.
            values = LintCategory::values().into_iter().collect();
        } else if options.is_set_choice(MainOption::XLINT_CUSTOM, "none") {
            // if -Xlint:none is given, disable all categories by default
        } else {
            let source = Source::instance(context);
            if *source >= Source::JDK9 {
                values.insert(LintCategory::DEP_ANN);
            }
            if Feature::REDUNDANT_STRICTFP.allowed_in_source(*source) {
                values.insert(LintCategory::STRICTFP);
            }
            values.insert(LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC);
            values.insert(LintCategory::OPENS);
            values.insert(LintCategory::MODULE);
            values.insert(LintCategory::REMOVAL);
            if !options.is_set_option(MainOption::PREVIEW) {
                values.insert(LintCategory::PREVIEW);
            }
            values.insert(LintCategory::SYNCHRONIZATION);
        }
        // Look for specific overrides
        for lc in LintCategory::values() {
            if options.is_set_choice(MainOption::XLINT_CUSTOM, lc.option()) {
                values.insert(lc);
            } else if options.is_set_choice(MainOption::XLINT_CUSTOM, &format!("-{}", lc.option()))
            {
                values.remove(&lc);
            }
        }
        Lint {
            values,
            suppressed_values: BTreeSet::new(),
        }
    }

    /// Java: protected Lint(Lint other).
    pub(crate) fn new_from(other: &Lint) -> Lint {
        Lint {
            values: other.values.clone(),
            suppressed_values: other.suppressed_values.clone(),
        }
    }

    /// Returns a new Lint that has the given LintCategorys suppressed.
    /// @param lc one or more categories to be suppressed
    ///
    /// Java: public Lint suppress(LintCategory... lc).
    pub fn suppress(&self, lc: &[LintCategory]) -> Lint {
        let mut lint = Lint::new_from(self);
        for category in lc {
            lint.values.remove(category);
            lint.suppressed_values.insert(*category);
        }
        lint
    }

    /// Returns the result of combining the values in this object with
    /// the given annotation.
    ///
    /// Java: public Lint augment(Attribute.Compound attr) (Lint.java:65-67).
    /// Porting rules adapter: the upstream AugmentVisitor is a context
    /// singleton; the ported visitor takes the compiler context and symtab
    /// explicitly (ADR 0001).
    pub fn augment_compound(
        &self,
        ctx: &CompilerContext,
        syms: &Symtab,
        attr: AttributeId,
    ) -> Lint {
        augment_visitor_augment_compound(ctx, syms, self, attr)
    }

    /// Returns the result of combining the values in this object with
    /// the metadata on the given symbol.
    ///
    /// Java: public Lint augment(Symbol sym) (Lint.java:73-81).
    pub fn augment(&self, ctx: &CompilerContext, syms: &Symtab, sym: &Symbol) -> Lint {
        let mut lint =
            augment_visitor_augment_list(ctx, syms, self, &sym.get_declaration_attributes());
        if sym.is_deprecated() {
            // Java: if (l == this) l = new Lint(this); then remove/add
            // DEPRECATION. The ported augmentor always yields a copy
            // (Lint::new_from), which the adapter adapter treats as the
            // "l != this" branch; the resulting category sets are identical.
            let mut values = lint.values.clone();
            let mut suppressed = lint.suppressed_values.clone();
            values.remove(&LintCategory::DEPRECATION);
            suppressed.insert(LintCategory::DEPRECATION);
            lint.values = values;
            lint.suppressed_values = suppressed;
        }
        lint
    }

    /// Checks if a warning category is enabled. A warning category may be enabled
    /// on the command line, or by default, and can be temporarily disabled with
    /// the SuppressWarnings annotation.
    pub fn is_enabled(&self, lc: LintCategory) -> bool {
        self.values.contains(&lc)
    }

    /// Checks is a warning category has been specifically suppressed, by means
    /// of the SuppressWarnings annotation, or, in the case of the deprecated
    /// category, whether it has been implicitly suppressed by virtue of the
    /// current entity being itself deprecated.
    pub fn is_suppressed(&self, lc: LintCategory) -> bool {
        self.suppressed_values.contains(&lc)
    }
}

impl fmt::Display for Lint {
    /// Java: public String toString() {
    ///           return "Lint:[values" + values + " suppressedValues" + suppressedValues + "]";
    ///       }
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Lint:[values{} suppressedValues{}]",
            format_categories(&self.values),
            format_categories(&self.suppressed_values)
        )
    }
}

/// Java: EnumSet.toString() ("[A, B]" in enum declaration order).
fn format_categories(categories: &BTreeSet<LintCategory>) -> String {
    let names: Vec<&str> = categories.iter().map(|category| category.name()).collect();
    format!("[{}]", names.join(", "))
}

/// Categories of warnings that can be generated by the compiler.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LintCategory {
    /// Warn when code refers to a auxiliary class that is hidden in a source file
    /// (ie source file name is different from the class name, and the type is not
    /// properly nested) and the referring code is not located in the same source file.
    AUXILIARYCLASS,
    /// Warn about use of unnecessary casts.
    CAST,
    /// Warn about issues related to classfile contents
    CLASSFILE,
    /// Warn about use of deprecated items.
    DEPRECATION,
    /// Warn about items which are documented with an {@code @deprecated} JavaDoc
    /// comment, but which do not have {@code @Deprecated} annotation.
    DEP_ANN,
    /// Warn about division by constant integer 0.
    DIVZERO,
    /// Warn about empty statement after if.
    EMPTY,
    /// Warn about issues regarding module exports.
    EXPORTS,
    /// Warn about falling through from one case of a switch statement to the next.
    FALLTHROUGH,
    /// Warn about finally clauses that do not terminate normally.
    FINALLY,
    /// Warn about compiler generation of a default constructor.
    MISSING_EXPLICIT_CTOR,
    /// Warn about module system related issues.
    MODULE,
    /// Warn about issues regarding module opens.
    OPENS,
    /// Warn about issues relating to use of command line options
    OPTIONS,
    /// Warn about issues regarding method overloads.
    OVERLOADS,
    /// Warn about issues regarding method overrides.
    OVERRIDES,
    /// Warn about invalid path elements on the command line.
    /// Such warnings cannot be suppressed with the SuppressWarnings
    /// annotation.
    PATH,
    /// Warn about issues regarding annotation processing.
    PROCESSING,
    /// Warn about unchecked operations on raw types.
    RAW,
    /// Warn about use of deprecated-for-removal items.
    REMOVAL,
    /// Warn about use of automatic modules in the requires clauses.
    REQUIRES_AUTOMATIC,
    /// Warn about automatic modules in requires transitive.
    REQUIRES_TRANSITIVE_AUTOMATIC,
    /// Warn about Serializable classes that do not provide a serial version ID.
    SERIAL,
    /// Warn about issues relating to use of statics
    STATIC,
    /// Warn about unnecessary uses of the strictfp modifier
    STRICTFP,
    /// Warn about synchronization attempts on instances of @ValueBased classes.
    SYNCHRONIZATION,
    /// Warn about issues relating to use of text blocks
    TEXT_BLOCKS,
    /// Warn about issues relating to use of try blocks (i.e. try-with-resources)
    TRY,
    /// Warn about unchecked operations on raw types.
    UNCHECKED,
    /// Warn about potentially unsafe vararg methods
    VARARGS,
    /// Warn about use of preview features.
    PREVIEW,
}

impl LintCategory {
    /// Java: LintCategory.values().
    pub fn values() -> [LintCategory; 31] {
        [
            LintCategory::AUXILIARYCLASS,
            LintCategory::CAST,
            LintCategory::CLASSFILE,
            LintCategory::DEPRECATION,
            LintCategory::DEP_ANN,
            LintCategory::DIVZERO,
            LintCategory::EMPTY,
            LintCategory::EXPORTS,
            LintCategory::FALLTHROUGH,
            LintCategory::FINALLY,
            LintCategory::MISSING_EXPLICIT_CTOR,
            LintCategory::MODULE,
            LintCategory::OPENS,
            LintCategory::OPTIONS,
            LintCategory::OVERLOADS,
            LintCategory::OVERRIDES,
            LintCategory::PATH,
            LintCategory::PROCESSING,
            LintCategory::RAW,
            LintCategory::REMOVAL,
            LintCategory::REQUIRES_AUTOMATIC,
            LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC,
            LintCategory::SERIAL,
            LintCategory::STATIC,
            LintCategory::STRICTFP,
            LintCategory::SYNCHRONIZATION,
            LintCategory::TEXT_BLOCKS,
            LintCategory::TRY,
            LintCategory::UNCHECKED,
            LintCategory::VARARGS,
            LintCategory::PREVIEW,
        ]
    }

    /// Java: {@code public final String option} (the -Xlint suboption name).
    pub const fn option(self) -> &'static str {
        match self {
            LintCategory::AUXILIARYCLASS => "auxiliaryclass",
            LintCategory::CAST => "cast",
            LintCategory::CLASSFILE => "classfile",
            LintCategory::DEPRECATION => "deprecation",
            LintCategory::DEP_ANN => "dep-ann",
            LintCategory::DIVZERO => "divzero",
            LintCategory::EMPTY => "empty",
            LintCategory::EXPORTS => "exports",
            LintCategory::FALLTHROUGH => "fallthrough",
            LintCategory::FINALLY => "finally",
            LintCategory::MISSING_EXPLICIT_CTOR => "missing-explicit-ctor",
            LintCategory::MODULE => "module",
            LintCategory::OPENS => "opens",
            LintCategory::OPTIONS => "options",
            LintCategory::OVERLOADS => "overloads",
            LintCategory::OVERRIDES => "overrides",
            LintCategory::PATH => "path",
            LintCategory::PROCESSING => "processing",
            LintCategory::RAW => "rawtypes",
            LintCategory::REMOVAL => "removal",
            LintCategory::REQUIRES_AUTOMATIC => "requires-automatic",
            LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC => "requires-transitive-automatic",
            LintCategory::SERIAL => "serial",
            LintCategory::STATIC => "static",
            LintCategory::STRICTFP => "strictfp",
            LintCategory::SYNCHRONIZATION => "synchronization",
            LintCategory::TEXT_BLOCKS => "text-blocks",
            LintCategory::TRY => "try",
            LintCategory::UNCHECKED => "unchecked",
            LintCategory::VARARGS => "varargs",
            LintCategory::PREVIEW => "preview",
        }
    }

    /// Java: {@code public final boolean hidden}.
    ///
    /// every upstream constant is created with the one-argument
    /// constructor (hidden = false); the field is kept as an accessor.
    pub const fn hidden(self) -> bool {
        false
    }

    /// Java: static LintCategory get(String option) — the static option map.
    pub fn get(option: &str) -> Option<LintCategory> {
        let category = match option {
            "auxiliaryclass" => LintCategory::AUXILIARYCLASS,
            "cast" => LintCategory::CAST,
            "classfile" => LintCategory::CLASSFILE,
            "deprecation" => LintCategory::DEPRECATION,
            "dep-ann" => LintCategory::DEP_ANN,
            "divzero" => LintCategory::DIVZERO,
            "empty" => LintCategory::EMPTY,
            "exports" => LintCategory::EXPORTS,
            "fallthrough" => LintCategory::FALLTHROUGH,
            "finally" => LintCategory::FINALLY,
            "missing-explicit-ctor" => LintCategory::MISSING_EXPLICIT_CTOR,
            "module" => LintCategory::MODULE,
            "opens" => LintCategory::OPENS,
            "options" => LintCategory::OPTIONS,
            "overloads" => LintCategory::OVERLOADS,
            "overrides" => LintCategory::OVERRIDES,
            "path" => LintCategory::PATH,
            "processing" => LintCategory::PROCESSING,
            "rawtypes" => LintCategory::RAW,
            "removal" => LintCategory::REMOVAL,
            "requires-automatic" => LintCategory::REQUIRES_AUTOMATIC,
            "requires-transitive-automatic" => LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC,
            "serial" => LintCategory::SERIAL,
            "static" => LintCategory::STATIC,
            "strictfp" => LintCategory::STRICTFP,
            "synchronization" => LintCategory::SYNCHRONIZATION,
            "text-blocks" => LintCategory::TEXT_BLOCKS,
            "try" => LintCategory::TRY,
            "unchecked" => LintCategory::UNCHECKED,
            "varargs" => LintCategory::VARARGS,
            "preview" => LintCategory::PREVIEW,
            _ => return None,
        };
        Some(category)
    }

    /// Java: enum#name() — the constant name (Enum.toString()).
    pub const fn name(self) -> &'static str {
        match self {
            LintCategory::AUXILIARYCLASS => "AUXILIARYCLASS",
            LintCategory::CAST => "CAST",
            LintCategory::CLASSFILE => "CLASSFILE",
            LintCategory::DEPRECATION => "DEPRECATION",
            LintCategory::DEP_ANN => "DEP_ANN",
            LintCategory::DIVZERO => "DIVZERO",
            LintCategory::EMPTY => "EMPTY",
            LintCategory::EXPORTS => "EXPORTS",
            LintCategory::FALLTHROUGH => "FALLTHROUGH",
            LintCategory::FINALLY => "FINALLY",
            LintCategory::MISSING_EXPLICIT_CTOR => "MISSING_EXPLICIT_CTOR",
            LintCategory::MODULE => "MODULE",
            LintCategory::OPENS => "OPENS",
            LintCategory::OPTIONS => "OPTIONS",
            LintCategory::OVERLOADS => "OVERLOADS",
            LintCategory::OVERRIDES => "OVERRIDES",
            LintCategory::PATH => "PATH",
            LintCategory::PROCESSING => "PROCESSING",
            LintCategory::RAW => "RAW",
            LintCategory::REMOVAL => "REMOVAL",
            LintCategory::REQUIRES_AUTOMATIC => "REQUIRES_AUTOMATIC",
            LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC => "REQUIRES_TRANSITIVE_AUTOMATIC",
            LintCategory::SERIAL => "SERIAL",
            LintCategory::STATIC => "STATIC",
            LintCategory::STRICTFP => "STRICTFP",
            LintCategory::SYNCHRONIZATION => "SYNCHRONIZATION",
            LintCategory::TEXT_BLOCKS => "TEXT_BLOCKS",
            LintCategory::TRY => "TRY",
            LintCategory::UNCHECKED => "UNCHECKED",
            LintCategory::VARARGS => "VARARGS",
            LintCategory::PREVIEW => "PREVIEW",
        }
    }
}

impl fmt::Display for LintCategory {
    /// Java: Enum.toString() -> the constant name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Java: protected static class Lint.AugmentVisitor implements
/// Attribute.Visitor (Lint.java:337-445).
struct AugmentVisitor<'a> {
    ctx: &'a CompilerContext,
    syms: &'a Symtab,
    parent: &'a Lint,
    lint: Option<Lint>,
}

impl<'a> AugmentVisitor<'a> {
    fn new(ctx: &'a CompilerContext, syms: &'a Symtab, parent: &'a Lint) -> AugmentVisitor<'a> {
        AugmentVisitor {
            ctx,
            syms,
            parent,
            lint: None,
        }
    }

    /// Java: private void suppress(LintCategory lc) (Lint.java:378-384)
    fn suppress(&mut self, lc: LintCategory) {
        if self.lint.is_none() {
            self.lint = Some(Lint::new_from(self.parent));
        }
        let lint = self.lint.as_mut().expect("augmented lint");
        lint.suppressed_values.insert(lc);
        lint.values.remove(&lc);
    }

    /// Java: Attribute.Visitor.visitArray (Lint.java:420-425)
    fn walk_array(&mut self, array: &Attribute) {
        if let AttributeKind::Array { values } = &array.kind {
            let mut index = 0;
            while index < values.len() {
                let value = values[index];
                let attribute = self.ctx.attr(value);
                self.walk(attribute);
                index += 1;
            }
        }
    }

    fn walk(&mut self, attribute: &Attribute) {
        match &attribute.kind {
            AttributeKind::Constant { value } => {
                // Java: visitConstant (Lint.java:388-393)
                let string_tsym = self.ctx.typ(self.syms.string_type).tsym;
                let attribute_tsym = attribute.ty.and_then(|ty| self.ctx.typ(ty).tsym);
                if attribute_tsym == string_tsym
                    && let ConstantValue::String(text) = value
                    && let Some(lc) = LintCategory::get(&text.to_string_lossy())
                {
                    self.suppress(lc);
                }
            }
            AttributeKind::Compound { values, .. } | AttributeKind::TypeCompound { values, .. } => {
                // Java: visitCompound (Lint.java:405-418)
                let compound_tsym = attribute.ty.and_then(|ty| self.ctx.typ(ty).tsym);
                let suppress_warnings_tsym = self.ctx.typ(self.syms.suppress_warnings_type).tsym;
                if compound_tsym == suppress_warnings_tsym {
                    let values = values.clone();
                    // Iterate instead of length()/get(index): both are O(n) per
                    // step, which made this loop O(n^2).
                    for pair in values.iter() {
                        let method = self.ctx.sym(pair.fst);
                        if method.name.to_string() == "value" {
                            let value = self.ctx.attr(pair.snd);
                            self.walk(value);
                        }
                    }
                }
            }
            AttributeKind::Array { .. } => {
                // Java: visitArray (Lint.java:420-425)
                self.walk_array(attribute);
            }
            AttributeKind::Class { .. } => {
                // Java: visitClass (Lint.java:395-396): no-op.
            }
            AttributeKind::Enum { .. } => {
                // Java: visitEnum (Lint.java:427-428): no-op.
            }
            AttributeKind::Error | AttributeKind::UnresolvedClass { .. } => {
                // Java: visitError (Lint.java:430-431): no-op.
            }
        }
    }
}

/// Java: AugmentVisitor.augment(Lint parent, Attribute.Compound attr)
/// (Lint.java:351-358)
fn augment_visitor_augment_compound(
    ctx: &CompilerContext,
    syms: &Symtab,
    parent: &Lint,
    attr: AttributeId,
) -> Lint {
    let mut visitor = AugmentVisitor::new(ctx, syms, parent);
    visitor.walk(ctx.attr(attr));
    visitor.lint.unwrap_or_else(|| Lint::new_from(parent))
}

/// Java: AugmentVisitor.augment(Lint parent, List<Attribute.Compound> attrs)
/// (Lint.java:360-368)
fn augment_visitor_augment_list(
    ctx: &CompilerContext,
    syms: &Symtab,
    parent: &Lint,
    attrs: &List<AttributeId>,
) -> Lint {
    let mut visitor = AugmentVisitor::new(ctx, syms, parent);
    for attr in attrs.iter() {
        visitor.walk(ctx.attr(attr));
    }
    visitor.lint.unwrap_or_else(|| Lint::new_from(parent))
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::{Lint, LintCategory};

    fn make_lint(values: &[LintCategory], suppressed: &[LintCategory]) -> Lint {
        Lint {
            values: values.iter().copied().collect(),
            suppressed_values: suppressed.iter().copied().collect(),
        }
    }

    #[test]
    fn categories_are_complete_and_in_declaration_order() {
        let values = LintCategory::values();
        assert_eq!(values.len(), 31);
        assert_eq!(values[0], LintCategory::AUXILIARYCLASS);
        assert_eq!(values[1], LintCategory::CAST);
        assert_eq!(values[values.len() - 1], LintCategory::PREVIEW);
    }

    #[test]
    fn option_strings_match_the_xlint_names() {
        assert_eq!(LintCategory::AUXILIARYCLASS.option(), "auxiliaryclass");
        assert_eq!(LintCategory::DEP_ANN.option(), "dep-ann");
        assert_eq!(LintCategory::RAW.option(), "rawtypes");
        assert_eq!(
            LintCategory::REQUIRES_TRANSITIVE_AUTOMATIC.option(),
            "requires-transitive-automatic"
        );
        assert_eq!(LintCategory::PREVIEW.option(), "preview");
    }

    #[test]
    fn get_maps_every_option_back_to_its_category() {
        for category in LintCategory::values() {
            assert_eq!(LintCategory::get(category.option()), Some(category));
            // The 31 option strings are unique (the Java map would otherwise
            // silently drop an entry).
            let count = LintCategory::values()
                .iter()
                .filter(|other| other.option() == category.option())
                .count();
            assert_eq!(count, 1);
        }
        assert_eq!(LintCategory::get("unknown"), None);
        assert_eq!(LintCategory::get(""), None);
        assert_eq!(LintCategory::get("RAW"), None);
    }

    #[test]
    fn hidden_is_false_for_all_categories() {
        for category in LintCategory::values() {
            assert!(!category.hidden());
        }
    }

    #[test]
    fn names_are_the_enum_constant_names() {
        assert_eq!(LintCategory::AUXILIARYCLASS.name(), "AUXILIARYCLASS");
        assert_eq!(
            LintCategory::MISSING_EXPLICIT_CTOR.name(),
            "MISSING_EXPLICIT_CTOR"
        );
        assert_eq!(LintCategory::PREVIEW.to_string(), "PREVIEW");
    }

    #[test]
    fn is_enabled_and_is_suppressed_follow_the_sets() {
        let lint = make_lint(
            &[LintCategory::CAST, LintCategory::RAW],
            &[LintCategory::DEPRECATION],
        );
        assert!(lint.is_enabled(LintCategory::CAST));
        assert!(lint.is_enabled(LintCategory::RAW));
        assert!(!lint.is_enabled(LintCategory::DEPRECATION));
        assert!(lint.is_suppressed(LintCategory::DEPRECATION));
        assert!(!lint.is_suppressed(LintCategory::CAST));
    }

    #[test]
    fn suppress_returns_a_new_lint_and_keeps_the_original() {
        let original = make_lint(&[LintCategory::CAST, LintCategory::RAW], &[]);
        let suppressed = original.suppress(&[LintCategory::CAST]);
        // the original is untouched (Java copy constructor semantics)
        assert!(original.is_enabled(LintCategory::CAST));
        assert!(!original.is_suppressed(LintCategory::CAST));
        // the copy has the category suppressed and no longer enabled
        assert!(!suppressed.is_enabled(LintCategory::CAST));
        assert!(suppressed.is_suppressed(LintCategory::CAST));
        assert!(suppressed.is_enabled(LintCategory::RAW));
    }

    #[test]
    fn display_uses_the_enumset_format() {
        let lint = make_lint(
            &[LintCategory::CAST, LintCategory::RAW],
            &[LintCategory::DEPRECATION],
        );
        // EnumSet iteration order = enum declaration order (DEPRECATION < CAST < RAW).
        assert_eq!(
            lint.to_string(),
            "Lint:[values[CAST, RAW] suppressedValues[DEPRECATION]]"
        );
        assert_eq!(
            make_lint(&[], &[]).to_string(),
            "Lint:[values[] suppressedValues[]]"
        );
    }

    #[test]
    fn instance_is_a_singleton_from_the_context() {
        let mut context = crate::util::context::Context::new();
        let first = Lint::instance(&mut context);
        let second = Lint::instance(&mut context);
        assert!(Rc::ptr_eq(&first, &second));
        // Defaults: -Xlint not set -> only the on-by-default categories are
        // enabled (Source default is the latest; DEP_ANN/STRICTFP included).
        assert!(first.is_enabled(LintCategory::DEP_ANN));
        assert!(first.is_enabled(LintCategory::MODULE));
        assert!(first.is_enabled(LintCategory::OPENS));
        assert!(first.is_enabled(LintCategory::REMOVAL));
        assert!(first.is_enabled(LintCategory::SYNCHRONIZATION));
        assert!(first.is_enabled(LintCategory::PREVIEW));
        assert!(!first.is_enabled(LintCategory::CAST));
        assert!(!first.is_enabled(LintCategory::RAW));
        assert!(!first.is_suppressed(LintCategory::CAST));
    }
}

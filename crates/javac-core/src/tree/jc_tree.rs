//! The abstract syntax tree (JCTree) of the javac compiler.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own
//! risk.  This code and its internal interfaces are subject to change
//! or deletion without notice.</b></p>
//!
//! PORTING_RULES: ADR 0001 (arena + IDs) defines the representation of the tree
//! nodes:
//! - a node is a JCTree value in the tree arena (TreeId is the arena index);
//! - child references are Option<TreeId> (Java null -> Option), child lists are
//!   List<TreeId> (util::list, List<JCTree> in Java);
//! - symbols/types/scopes are Option<SymbolId> / Option<TypeId> / Option<ScopeId>;
//! - the upstream concrete node classes map 1:1 to TreeKind variants; the JC
//!   prefix is dropped in the variant name (e.g. TreeKind::ClassDecl is the
//!   upstream JCTree.JCClassDecl; the doc comment records the upstream class);
//! - the abstract node classes (JCExpression, JCStatement, JCPolyExpression,
//!   JCFunctionalExpression, JCOperatorExpression, JCPattern, JCDirective) have
//!   no variants of their own: their fields are merged into the concrete
//!   variants (polyKind, target, operator);
//! - Java Tag is the internal tag (105 values); the com.sun.source Tree.Kind
//!   mapping lives in the tree layer's later steps (ADR 0005).
//!
//! - Annotation.attribute and Exports/Opens/Requires.directive are arena
//!   references (Option<AttributeId> / Option<DirectiveId>): code/Attribute and
//!   code/Directive are "immutable but identity-semantic" objects, so they are
//!   arena values (ADR 0001 supplement), not Box-embedded values.
//!
//! The 67 concrete node variants (upstream JCTree concrete classes) are:
//! CompilationUnit, PackageDecl, Import, ClassDecl, MethodDecl, VariableDecl,
//! Skip, Block, DoWhileLoop, WhileLoop, ForLoop, EnhancedForLoop,
//! LabeledStatement, Switch, Case, SwitchExpression, Synchronized, Try, Catch,
//! Conditional, If, ExpressionStatement, Break, Yield, Continue, Return, Throw,
//! Assert, MethodInvocation, NewClass, NewArray, Lambda, Parens, Assign,
//! AssignOp, Unary, Binary, TypeCast, InstanceOf, BindingPattern,
//! DefaultCaseLabel, ParenthesizedPattern, GuardPattern, ArrayAccess,
//! FieldAccess, MemberReference, Ident, Literal, PrimitiveTypeTree, ArrayTypeTree,
//! TypeApply, TypeUnion, TypeIntersection, TypeParameter, Wildcard,
//! TypeBoundKind, Annotation, Modifiers, AnnotatedType, ModuleDecl, Exports,
//! Opens, Provides, Requires, Uses, Erroneous, LetExpr
//! (the 8 upstream abstract node classes have no variants: their fields are
//! merged into the concrete variants).
//!
//! Not ported in this step (tracked in docs/STATUS.md):
//! - Visitor/Factory interfaces, Pretty-based toString, DiagnosticPosition
//!   methods (they need TreeInfo/Pretty, ADR 0005 S1b).

use std::rc::Rc;

use javac_source::tree::case_tree::CaseKind;
pub use javac_source::tree::member_reference_tree::ReferenceMode;
use javac_source::tree::module_tree::ModuleKind;
use javac_tools::tools::java_file_manager::Location;
use javac_tools::tools::java_file_object::JavaFileObject;

use crate::code::bound_kind::BoundKind;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::{AttributeId, DirectiveId, ScopeId, SymbolId, TreeId, TypeId};
use crate::tree::doc_comment_table::DocCommentTable;
use crate::tree::end_pos_table::EndPosTable;
use crate::util::java_string::JavaString;
use crate::util::list::List;
use crate::util::name::Name;
use crate::util::position::LineMap;

/// Tree tag values, identifying kinds of trees.
///
/// Java: public enum Tag (JCTree.Tag). PORTING_RULES: the Java enum keeps the
/// upstream constant names; the enum constructor argument noAssignTag of the
/// assignment operators is expressed by the no_assign_op() method.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i32)]
pub enum Tag {
    /// For methods that return an invalid tag if a given condition is not met
    NO_TAG,
    /// Toplevel nodes, of type TopLevel, representing entire source files.
    TOPLEVEL,
    /// Package level definitions.
    PACKAGEDEF,
    /// Import clauses, of type Import.
    IMPORT,
    /// Class definitions, of type ClassDef.
    CLASSDEF,
    /// Method definitions, of type MethodDef.
    METHODDEF,
    /// Variable definitions, of type VarDef.
    VARDEF,
    /// The no-op statement ";", of type Skip
    SKIP,
    /// Blocks, of type Block.
    BLOCK,
    /// Do-while loops, of type DoLoop.
    DOLOOP,
    /// While-loops, of type WhileLoop.
    WHILELOOP,
    /// For-loops, of type ForLoop.
    FORLOOP,
    /// Foreach-loops, of type ForeachLoop.
    FOREACHLOOP,
    /// Labelled statements, of type Labelled.
    LABELLED,
    /// Switch statements, of type Switch.
    SWITCH,
    /// Case parts in switch statements/expressions, of type Case.
    CASE,
    /// Switch expression statements, of type Switch.
    SWITCH_EXPRESSION,
    /// Synchronized statements, of type Synchronized.
    SYNCHRONIZED,
    /// Try statements, of type Try.
    TRY,
    /// Catch clauses in try statements, of type Catch.
    CATCH,
    /// Conditional expressions, of type Conditional.
    CONDEXPR,
    /// Conditional statements, of type If.
    IF,
    /// Expression statements, of type Exec.
    EXEC,
    /// Break statements, of type Break.
    BREAK,
    /// Yield statements, of type Yield.
    YIELD,
    /// Continue statements, of type Continue.
    CONTINUE,
    /// Return statements, of type Return.
    RETURN,
    /// Throw statements, of type Throw.
    THROW,
    /// Assert statements, of type Assert.
    ASSERT,
    /// Method invocation expressions, of type Apply.
    APPLY,
    /// Class instance creation expressions, of type NewClass.
    NEWCLASS,
    /// Array creation expressions, of type NewArray.
    NEWARRAY,
    /// Lambda expression, of type Lambda.
    LAMBDA,
    /// Parenthesized subexpressions, of type Parens.
    PARENS,
    /// Assignment expressions, of type Assign.
    ASSIGN,
    /// Type cast expressions, of type TypeCast.
    TYPECAST,
    /// Type test expressions, of type TypeTest.
    TYPETEST,
    /// Patterns.
    BINDINGPATTERN,
    DEFAULTCASELABEL,
    GUARDPATTERN,
    PARENTHESIZEDPATTERN,
    /// Indexed array expressions, of type Indexed.
    INDEXED,
    /// Selections, of type Select.
    SELECT,
    /// Member references, of type Reference.
    REFERENCE,
    /// Simple identifiers, of type Ident.
    IDENT,
    /// Literals, of type Literal.
    LITERAL,
    /// Basic type identifiers, of type TypeIdent.
    TYPEIDENT,
    /// Array types, of type TypeArray.
    TYPEARRAY,
    /// Parameterized types, of type TypeApply.
    TYPEAPPLY,
    /// Union types, of type TypeUnion.
    TYPEUNION,
    /// Intersection types, of type TypeIntersection.
    TYPEINTERSECTION,
    /// Formal type parameters, of type TypeParameter.
    TYPEPARAMETER,
    /// Type argument.
    WILDCARD,
    /// Bound kind: extends, super, exact, or unbound
    TYPEBOUNDKIND,
    /// metadata: Annotation.
    ANNOTATION,
    /// metadata: Type annotation.
    TYPE_ANNOTATION,
    /// metadata: Modifiers
    MODIFIERS,
    /// An annotated type tree.
    ANNOTATED_TYPE,
    /// Error trees, of type Erroneous.
    ERRONEOUS,
    /// Unary operators, of type Unary.
    POS, // +
    NEG,     // -
    NOT,     // !
    COMPL,   // ~
    PREINC,  // ++ _
    PREDEC,  // -- _
    POSTINC, // _ ++
    POSTDEC, // _ --
    /// unary operator for null reference checks, only used internally.
    NULLCHK,
    /// Binary operators, of type Binary.
    OR, // ||
    AND,    // &&
    BITOR,  // |
    BITXOR, // ^
    BITAND, // &
    EQ,     // ==
    NE,     // !=
    LT,     // <
    GT,     // >
    LE,     // <=
    GE,     // >=
    SL,     // <<
    SR,     // >>
    USR,    // >>>
    PLUS,   // +
    MINUS,  // -
    MUL,    // *
    DIV,    // /
    MOD,    // %
    /// Assignment operators, of type Assignop.
    BITOR_ASG, // |=
    BITXOR_ASG, // ^=
    BITAND_ASG, // &=
    SL_ASG, // <<=
    SR_ASG, // >>=
    USR_ASG, // >>>=
    PLUS_ASG, // +=
    MINUS_ASG, // -=
    MUL_ASG, // *=
    DIV_ASG, // /=
    MOD_ASG, // %=
    MODULEDEF,
    EXPORTS,
    OPENS,
    PROVIDES,
    REQUIRES,
    USES,
    /// A synthetic let expression, of type LetExpr.
    LETEXPR, // ala scheme
}

impl Tag {
    /// Java: private static final int numberOfOperators = MOD.ordinal() - POS.ordinal() + 1;
    const NUMBER_OF_OPERATORS: i32 = 28;

    /// Java: public static int getNumberOfOperators()
    pub const fn get_number_of_operators() -> i32 {
        Tag::NUMBER_OF_OPERATORS
    }

    /// Java: public Tag noAssignOp()
    ///
    /// PORTING_RULES: the upstream AssertionError for non-assignment tags is a
    /// panic with the same message.
    pub fn no_assign_op(self) -> Tag {
        match self {
            Tag::BITOR_ASG => Tag::BITOR,
            Tag::BITXOR_ASG => Tag::BITXOR,
            Tag::BITAND_ASG => Tag::BITAND,
            Tag::SL_ASG => Tag::SL,
            Tag::SR_ASG => Tag::SR,
            Tag::USR_ASG => Tag::USR,
            Tag::PLUS_ASG => Tag::PLUS,
            Tag::MINUS_ASG => Tag::MINUS,
            Tag::MUL_ASG => Tag::MUL,
            Tag::DIV_ASG => Tag::DIV,
            Tag::MOD_ASG => Tag::MOD,
            _ => panic!("noAssignOp() method is not available for non assignment tags"),
        }
    }

    /// Java: public boolean isPostUnaryOp()
    pub fn is_post_unary_op(self) -> bool {
        matches!(self, Tag::POSTINC | Tag::POSTDEC)
    }

    /// Java: public boolean isIncOrDecUnaryOp()
    pub fn is_inc_or_dec_unary_op(self) -> bool {
        matches!(
            self,
            Tag::PREINC | Tag::PREDEC | Tag::POSTINC | Tag::POSTDEC
        )
    }

    /// Java: public boolean isAssignop()
    pub fn is_assignop(self) -> bool {
        matches!(
            self,
            Tag::BITOR_ASG
                | Tag::BITXOR_ASG
                | Tag::BITAND_ASG
                | Tag::SL_ASG
                | Tag::SR_ASG
                | Tag::USR_ASG
                | Tag::PLUS_ASG
                | Tag::MINUS_ASG
                | Tag::MUL_ASG
                | Tag::DIV_ASG
                | Tag::MOD_ASG
        )
    }

    /// Java: public int operatorIndex() { return (this.ordinal() - POS.ordinal()); }
    pub fn operator_index(self) -> i32 {
        self as i32 - Tag::POS as i32
    }

    /// PORTING_RULES adapter: Java enum ordinal() is not available in Rust for
    /// fieldless enums without a match; the declaration order is the ordinal
    /// (ADR 0005: the value order must mirror the upstream enum).
    pub fn ordinal(self) -> usize {
        self as usize
    }
}

/// Java: public abstract class JCTree implements Tree, Cloneable, DiagnosticPosition
///
/// PORTING_RULES: the Java object reference (identity) becomes an arena ID; the
/// public fields of JCTree are the position, the type (filled during Attr) and
/// the tag; the node-specific fields live in the TreeKind variant.
#[allow(clippy::upper_case_acronyms)]
pub struct JCTree {
    /// The (encoded) position in the source file. @see util.Position.
    pub pos: i32,
    /// The type of this node (Java: public Type type; null -> None, ADR 0001).
    pub ty: Option<TypeId>,
    /// The tag of this node -- one of the constants declared in Tag.
    pub tag: Tag,
    /// The node-specific fields (one variant per upstream JCTree subclass).
    pub kind: TreeKind,
}

impl JCTree {
    /// Java: public abstract Tag getTag()
    pub fn get_tag(&self) -> Tag {
        self.tag
    }

    /// Java: public boolean hasTag(Tag tag)
    pub fn has_tag(&self, tag: Tag) -> bool {
        tag == self.get_tag()
    }

    /// Java: public JCTree setPos(int pos)
    pub fn set_pos(&mut self, pos: i32) -> &mut JCTree {
        self.pos = pos;
        self
    }

    /// Java: public JCTree setType(Type type)
    pub fn set_type(&mut self, ty: Option<TypeId>) -> &mut JCTree {
        self.ty = ty;
        self
    }
}

/// Java: JCTree.JCPolyExpression.PolyKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PolyKind {
    /// poly expression to be treated as a standalone expression
    STANDALONE,
    /// true poly expression
    POLY,
}

/// Java: JCTree.JCLambda.ParameterKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParameterKind {
    IMPLICIT,
    EXPLICIT,
}

/// Java: JCTree.JCMemberReference.OverloadKind (JCTree.java:2420-2450)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverloadKind {
    UNOVERLOADED,
    OVERLOADED,
    ERROR,
}

/// Java: JCTree.JCMemberReference.ReferenceKind
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    /// super # instMethod
    SUPER,
    /// Type # instMethod
    UNBOUND,
    /// Type # staticMethod
    STATIC,
    /// Expr # instMethod
    BOUND,
    /// Inner # new
    IMPLICIT_INNER,
    /// Toplevel # new
    TOPLEVEL,
    /// ArrayType # new
    ARRAY_CTOR,
}

impl ReferenceKind {
    /// Java: final ReferenceMode mode (enum constructor argument).
    pub fn mode(self) -> ReferenceMode {
        match self {
            ReferenceKind::SUPER
            | ReferenceKind::UNBOUND
            | ReferenceKind::STATIC
            | ReferenceKind::BOUND => ReferenceMode::INVOKE,
            ReferenceKind::IMPLICIT_INNER | ReferenceKind::TOPLEVEL | ReferenceKind::ARRAY_CTOR => {
                ReferenceMode::NEW
            }
        }
    }

    /// Java: public boolean isUnbound()
    pub fn is_unbound(self) -> bool {
        matches!(self, ReferenceKind::UNBOUND)
    }
}

/// Java: JCTree.JCLiteral.value is an Object holding one of the literal wrapper
/// values (Integer/Long/Float/Double/Character/Boolean/String).
/// PORTING_RULES: Java Object literal values map to this enum (registered in
/// PORTING_RULES section 9).
#[derive(Clone, Debug, PartialEq)]
pub enum LiteralValue {
    Integer(i32),
    /// Java Byte wrapper (the JCLiteral BYTE value domain).
    Byte(i8),
    /// Java Short wrapper (the JCLiteral SHORT value domain).
    Short(i16),
    Long(i64),
    Float(f32),
    Double(f64),
    /// Java Character (UTF-16 code unit).
    Character(u16),
    Boolean(bool),
    String(JavaString),
}

/// The node-specific fields of the JCTree subclasses.
///
/// PORTING_RULES: one variant per upstream concrete class (the JC prefix is
/// dropped, see the module doc); fields keep the upstream declaration order and
/// names (snake_case); references are Option<TreeId>, lists List<TreeId>;
/// inherited fields of the abstract base classes are merged into the variants.
#[allow(clippy::large_enum_variant)]
#[allow(non_camel_case_types)]
#[derive(Clone)]
pub enum TreeKind {
    /// Java: JCTree.JCCompilationUnit
    CompilationUnit {
        /// All definitions in this file (ClassDef, Import, and Skip)
        defs: List<TreeId>,
        /// The source file name.
        sourcefile: Option<Rc<dyn JavaFileObject>>,
        /// The module to which this compilation unit belongs.
        modle: Option<SymbolId>,
        /// The location in which this compilation unit was found.
        locn: Option<Rc<dyn Location>>,
        /// The package to which this compilation unit belongs.
        packge: Option<SymbolId>,
        /// A scope containing top level classes.
        toplevel_scope: Option<ScopeId>,
        /// A scope for all named imports.
        named_import_scope: Option<ScopeId>,
        /// A scope for all import-on-demands.
        star_import_scope: Option<ScopeId>,
        /// Line starting positions, defined only if option -g is set.
        line_map: Option<Rc<dyn LineMap>>,
        /// A table that stores all documentation comments indexed by the tree
        /// nodes they refer to. defined only if option -s is set.
        doc_comments: Option<Rc<std::cell::RefCell<dyn DocCommentTable>>>,
        /// An object encapsulating ending positions of source ranges indexed by
        /// the tree nodes they belong to. Defined only if option -Xjcov is set.
        end_positions: Option<Rc<dyn EndPosTable>>,
    },
    /// Java: JCTree.JCPackageDecl
    PackageDecl {
        /// The annotations on the package declaration.
        annotations: List<TreeId>,
        /// The qualified name of the package.
        pid: Option<TreeId>,
        /// The package symbol (filled during Enter).
        packge: Option<SymbolId>,
    },
    /// Java: JCTree.JCImport
    Import {
        /// Is this import a static import?
        static_import: bool,
        /// The qualified identifier of the imported member or import-on-demand.
        qualid: Option<TreeId>,
        /// The import scope (filled during Enter).
        import_scope: Option<ScopeId>,
    },
    /// Java: JCTree.JCClassDecl
    ClassDecl {
        /// the modifiers
        mods: Option<TreeId>,
        /// the name of the class
        name: Name,
        /// formal class parameters
        typarams: List<TreeId>,
        /// the classes this class extends
        extending: Option<TreeId>,
        /// the interfaces implemented by this class
        implementing: List<TreeId>,
        /// the subclasses allowed to extend this class, if sealed
        permitting: List<TreeId>,
        /// all variables and methods defined in this class
        defs: List<TreeId>,
        /// the symbol
        sym: Option<SymbolId>,
    },
    /// Java: JCTree.JCMethodDecl
    MethodDecl {
        /// the modifiers
        mods: Option<TreeId>,
        /// the name of the method
        name: Name,
        /// the return type
        restype: Option<TreeId>,
        /// formal type parameters
        typarams: List<TreeId>,
        /// receiver parameter
        recvparam: Option<TreeId>,
        /// value parameter
        params: List<TreeId>,
        /// exceptions thrown by the method
        thrown: List<TreeId>,
        /// method body, or null to indicate abstract methods
        body: Option<TreeId>,
        /// default value
        default_value: Option<TreeId>,
        /// the symbol
        sym: Option<SymbolId>,
        /// true if this method can complete normally (JLS 14.21)
        completes_normally: bool,
    },
    /// Java: JCTree.JCVariableDecl
    VariableDecl {
        /// the modifiers
        mods: Option<TreeId>,
        /// the name of the variable
        name: Name,
        /// the nameexpr of a var declaration
        nameexpr: Option<TreeId>,
        /// the type of the variable
        vartype: Option<TreeId>,
        /// the initializer of the variable
        init: Option<TreeId>,
        /// the symbol
        sym: Option<SymbolId>,
        /// declaration position
        start_pos: i32,
        /// Java: JCVariableDecl.declaredUsingVar (declared using 'var')
        declared_using_var: bool,
    },
    /// Java: JCTree.JCSkip
    Skip {},
    /// Java: JCTree.JCBlock
    Block {
        /// flags, as in JCModifiers
        flags: i64,
        /// the statements in the block
        stats: List<TreeId>,
        /// end position of the block (only set during parsing)
        endpos: i32,
    },
    /// Java: JCTree.JCDoWhileLoop
    DoWhileLoop {
        /// the loop body
        body: Option<TreeId>,
        /// the loop condition
        cond: Option<TreeId>,
    },
    /// Java: JCTree.JCWhileLoop
    WhileLoop {
        /// the loop condition
        cond: Option<TreeId>,
        /// the loop body
        body: Option<TreeId>,
    },
    /// Java: JCTree.JCForLoop
    ForLoop {
        /// the initializer statements
        init: List<TreeId>,
        /// the loop condition
        cond: Option<TreeId>,
        /// the step
        step: List<TreeId>,
        /// the loop body
        body: Option<TreeId>,
    },
    /// Java: JCTree.JCEnhancedForLoop
    EnhancedForLoop {
        /// the variable
        var: Option<TreeId>,
        /// the expression
        expr: Option<TreeId>,
        /// the loop body
        body: Option<TreeId>,
    },
    /// Java: JCTree.JCLabeledStatement
    LabeledStatement {
        /// the label
        label: Name,
        /// the labeled statement
        body: Option<TreeId>,
    },
    /// Java: JCTree.JCSwitch
    Switch {
        /// the selector
        selector: Option<TreeId>,
        /// the switch cases
        cases: List<TreeId>,
        /// end position of the switch (only set during parsing)
        endpos: i32,
        /// true if the switch has a total pattern
        has_total_pattern: bool,
        /// true if the switch is a pattern switch
        pattern_switch: bool,
    },
    /// Java: JCTree.JCCase
    Case {
        /// the case kind (statement or rule)
        case_kind: CaseKind,
        /// the labels
        labels: List<TreeId>,
        /// the statements
        stats: List<TreeId>,
        /// the body
        body: Option<TreeId>,
        /// true if this case can complete normally (JLS 14.21)
        completes_normally: bool,
    },
    /// Java: JCTree.JCSwitchExpression
    SwitchExpression {
        /// the selector
        selector: Option<TreeId>,
        /// the switch cases
        cases: List<TreeId>,
        /// end position of the switch (only set during parsing)
        endpos: i32,
        /// true if the switch has a total pattern
        has_total_pattern: bool,
        /// true if the switch is a pattern switch
        pattern_switch: bool,
        /// is this poly expression a 'true' poly expression?
        poly_kind: PolyKind,
    },
    /// Java: JCTree.JCSynchronized
    Synchronized {
        /// the lock
        lock: Option<TreeId>,
        /// the synchronized block
        body: Option<TreeId>,
    },
    /// Java: JCTree.JCTry
    Try {
        /// the try block
        body: Option<TreeId>,
        /// the catch clauses
        catchers: List<TreeId>,
        /// the finally block
        finalizer: Option<TreeId>,
        /// the resources
        resources: List<TreeId>,
        /// true if the finally block can complete normally (JLS 14.21)
        finally_can_complete_normally: bool,
    },
    /// Java: JCTree.JCCatch
    Catch {
        /// the catch parameter
        param: Option<TreeId>,
        /// the catch block
        body: Option<TreeId>,
    },
    /// Java: JCTree.JCConditional
    Conditional {
        /// the condition
        cond: Option<TreeId>,
        /// the true part
        truepart: Option<TreeId>,
        /// the false part
        falsepart: Option<TreeId>,
        /// is this poly expression a 'true' poly expression?
        poly_kind: PolyKind,
    },
    /// Java: JCTree.JCIf
    If {
        /// the condition
        cond: Option<TreeId>,
        /// the then part
        thenpart: Option<TreeId>,
        /// the else part
        elsepart: Option<TreeId>,
    },
    /// Java: JCTree.JCExpressionStatement
    ExpressionStatement {
        /// the expression
        expr: Option<TreeId>,
    },
    /// Java: JCTree.JCBreak
    Break {
        /// the label, or null
        label: Option<Name>,
        /// the target of the break (filled during Attr)
        target: Option<TreeId>,
    },
    /// Java: JCTree.JCYield
    Yield {
        /// the value
        value: Option<TreeId>,
        /// the target of the yield (filled during Attr)
        target: Option<TreeId>,
    },
    /// Java: JCTree.JCContinue
    Continue {
        /// the label, or null
        label: Option<Name>,
        /// the target of the continue (filled during Attr)
        target: Option<TreeId>,
    },
    /// Java: JCTree.JCReturn
    Return {
        /// the return expression
        expr: Option<TreeId>,
    },
    /// Java: JCTree.JCThrow
    Throw {
        /// the thrown expression
        expr: Option<TreeId>,
    },
    /// Java: JCTree.JCAssert
    Assert {
        /// the condition
        cond: Option<TreeId>,
        /// the detail expression
        detail: Option<TreeId>,
    },
    /// Java: JCTree.JCMethodInvocation
    MethodInvocation {
        /// the type arguments
        typeargs: List<TreeId>,
        /// the method expression
        meth: Option<TreeId>,
        /// the arguments
        args: List<TreeId>,
        /// the varargs element type (filled during Attr)
        varargs_element: Option<TypeId>,
        /// is this poly expression a 'true' poly expression?
        poly_kind: PolyKind,
    },
    /// Java: JCTree.JCNewClass
    NewClass {
        /// the enclosing expression
        encl: Option<TreeId>,
        /// the type arguments
        typeargs: List<TreeId>,
        /// the class expression
        clazz: Option<TreeId>,
        /// the arguments
        args: List<TreeId>,
        /// the anonymous class definition, or null
        def: Option<TreeId>,
        /// the constructor (filled during Attr/Resolve)
        constructor: Option<SymbolId>,
        /// the varargs element type (filled during Attr)
        varargs_element: Option<TypeId>,
        /// the constructor type (filled during Attr)
        constructor_type: Option<TypeId>,
        /// is this poly expression a 'true' poly expression?
        poly_kind: PolyKind,
        /// Java: JCNewClass.classDeclRemoved (the speculative new class marker).
        class_decl_removed: bool,
    },
    /// Java: JCTree.JCNewArray
    NewArray {
        /// the element type
        elemtype: Option<TreeId>,
        /// the dimension expressions
        dims: List<TreeId>,
        /// the annotations on the element type
        annotations: List<TreeId>,
        /// the annotations on the dimensions
        dim_annotations: List<List<TreeId>>,
        /// the initializer elements (null when the array is created with
        /// dimension expressions only; Java null -> None)
        elems: Option<List<TreeId>>,
    },
    /// Java: JCTree.JCLambda
    Lambda {
        /// the parameters
        params: List<TreeId>,
        /// the body
        body: Option<TreeId>,
        /// true if this lambda can complete normally (JLS 14.21)
        can_complete_normally: bool,
        /// the parameter kind (implicit or explicit)
        param_kind: ParameterKind,
        /// is this poly expression a 'true' poly expression?
        poly_kind: PolyKind,
        /// list of target types inferred for this functional expression.
        target: Option<TypeId>,
    },
    /// Java: JCTree.JCParens
    Parens {
        /// the parenthesized expression
        expr: Option<TreeId>,
    },
    /// Java: JCTree.JCAssign
    Assign {
        /// the left-hand side
        lhs: Option<TreeId>,
        /// the right-hand side
        rhs: Option<TreeId>,
    },
    /// Java: JCTree.JCAssignOp
    AssignOp {
        /// the left-hand side
        lhs: Option<TreeId>,
        /// the right-hand side
        rhs: Option<TreeId>,
        /// the operator symbol (JCOperatorExpression.operator)
        operator: Option<SymbolId>,
    },
    /// Java: JCTree.JCUnary
    Unary {
        /// the operand
        arg: Option<TreeId>,
        /// the operator symbol (JCOperatorExpression.operator)
        operator: Option<SymbolId>,
    },
    /// Java: JCTree.JCBinary
    Binary {
        /// the left-hand side
        lhs: Option<TreeId>,
        /// the right-hand side
        rhs: Option<TreeId>,
        /// the operator symbol (JCOperatorExpression.operator)
        operator: Option<SymbolId>,
    },
    /// Java: JCTree.JCTypeCast
    TypeCast {
        /// the type
        clazz: Option<TreeId>,
        /// the expression
        expr: Option<TreeId>,
    },
    /// Java: JCTree.JCInstanceOf
    InstanceOf {
        /// the expression
        expr: Option<TreeId>,
        /// the pattern or type
        pattern: Option<TreeId>,
    },
    /// Java: JCTree.JCBindingPattern
    BindingPattern {
        /// the binding variable
        var: Option<TreeId>,
    },
    /// Java: JCTree.JCDefaultCaseLabel
    DefaultCaseLabel {},
    /// Java: JCTree.JCParenthesizedPattern
    ParenthesizedPattern {
        /// the parenthesized pattern
        pattern: Option<TreeId>,
    },
    /// Java: JCTree.JCGuardPattern
    GuardPattern {
        /// the pattern
        patt: Option<TreeId>,
        /// the guard expression
        expr: Option<TreeId>,
    },
    /// Java: JCTree.JCArrayAccess
    ArrayAccess {
        /// the indexed expression
        indexed: Option<TreeId>,
        /// the index
        index: Option<TreeId>,
    },
    /// Java: JCTree.JCFieldAccess
    FieldAccess {
        /// the selected expression
        selected: Option<TreeId>,
        /// the field name
        name: Name,
        /// the symbol (filled during Enter/Attr)
        sym: Option<SymbolId>,
    },
    /// Java: JCTree.JCMemberReference
    MemberReference {
        /// the reference mode (invoke or new)
        mode: ReferenceMode,
        /// the reference kind (Java null before Attr -> None)
        kind: Option<ReferenceKind>,
        /// Java: JCMemberReference.overloadKind (ArgumentAttr fills it; the
        /// DeferredAttr stuck policies read it).
        overload_kind: Option<OverloadKind>,
        /// the referenced name
        name: Name,
        /// the expression
        expr: Option<TreeId>,
        /// the type arguments (null when absent; Java null -> None)
        typeargs: Option<List<TreeId>>,
        /// the symbol (filled during Attr)
        sym: Option<SymbolId>,
        /// the varargs element type (filled during Attr)
        varargs_element: Option<TypeId>,
        /// the poly kind of the reference
        ref_poly_kind: PolyKind,
        /// true if the referenced owner is accessible
        owner_accessible: bool,
        /// the referent type (filled during Attr)
        referent_type: Option<TypeId>,
        /// is this poly expression a 'true' poly expression?
        poly_kind: PolyKind,
        /// list of target types inferred for this functional expression.
        target: Option<TypeId>,
    },
    /// Java: JCTree.JCIdent
    Ident {
        /// the identifier name
        name: Name,
        /// the symbol (filled during Enter)
        sym: Option<SymbolId>,
    },
    /// Java: JCTree.JCLiteral
    Literal {
        /// the literal type tag
        typetag: TypeTag,
        /// the literal value (Java Object; null -> None)
        value: Option<LiteralValue>,
    },
    /// Java: JCTree.JCPrimitiveTypeTree
    PrimitiveTypeTree {
        /// the primitive type tag
        typetag: TypeTag,
    },
    /// Java: JCTree.JCArrayTypeTree
    ArrayTypeTree {
        /// the element type
        elemtype: Option<TreeId>,
    },
    /// Java: JCTree.JCTypeApply
    TypeApply {
        /// the type expression
        clazz: Option<TreeId>,
        /// the type arguments
        arguments: List<TreeId>,
    },
    /// Java: JCTree.JCTypeUnion
    TypeUnion {
        /// the union alternatives
        alternatives: List<TreeId>,
    },
    /// Java: JCTree.JCTypeIntersection
    TypeIntersection {
        /// the intersection bounds
        bounds: List<TreeId>,
    },
    /// Java: JCTree.JCTypeParameter
    TypeParameter {
        /// the type parameter name
        name: Name,
        /// the bounds
        bounds: List<TreeId>,
        /// the annotations
        annotations: List<TreeId>,
    },
    /// Java: JCTree.JCWildcard
    Wildcard {
        /// the wildcard bound kind tree (JCTree.TypeBoundKind)
        kind: Option<TreeId>,
        /// the inner expression, or null
        inner: Option<TreeId>,
    },
    /// Java: JCTree.TypeBoundKind
    TypeBoundKind {
        /// the bound kind
        kind: BoundKind,
    },
    /// Java: JCTree.JCAnnotation (the private tag field is expressed by the
    /// JCTree.tag field: Tag.ANNOTATION or Tag.TYPE_ANNOTATION).
    Annotation {
        /// the annotation type
        annotation_type: Option<TreeId>,
        /// the annotation arguments
        args: List<TreeId>,
        /// the attribute (filled during annotation processing; an arena value)
        attribute: Option<AttributeId>,
    },
    /// Java: JCTree.JCModifiers
    Modifiers {
        /// the modifier flags
        flags: i64,
        /// the annotations
        annotations: List<TreeId>,
    },
    /// Java: JCTree.JCAnnotatedType
    AnnotatedType {
        /// the annotations
        annotations: List<TreeId>,
        /// the underlying type
        underlying_type: Option<TreeId>,
    },
    /// Java: JCTree.JCModuleDecl
    ModuleDecl {
        /// the modifiers
        mods: Option<TreeId>,
        /// the module type (Java: ModuleType, filled during Attr)
        ty: Option<TypeId>,
        /// the module kind (open or strong)
        kind: ModuleKind,
        /// the module name
        qual_id: Option<TreeId>,
        /// the directives (null when the module declaration has no body;
        /// Java null -> None)
        directives: Option<List<TreeId>>,
        /// the module symbol (filled during Enter)
        sym: Option<SymbolId>,
    },
    /// Java: JCTree.JCExports ('exports' Package ';' / 'exports' Package 'to'
    /// ModuleList ';')
    Exports {
        /// the exported package
        qualid: Option<TreeId>,
        /// the target module names (null for an unqualified directive;
        /// Java null -> None)
        module_names: Option<List<TreeId>>,
        /// the directive (filled during Enter; an arena value)
        directive: Option<DirectiveId>,
    },
    /// Java: JCTree.JCOpens ('opens' Package ';' / 'opens' Package 'to'
    /// ModuleList ';')
    Opens {
        /// the opened package
        qualid: Option<TreeId>,
        /// the target module names (null for an unqualified directive;
        /// Java null -> None)
        module_names: Option<List<TreeId>>,
        /// the directive (filled during Enter; an arena value)
        directive: Option<DirectiveId>,
    },
    /// Java: JCTree.JCProvides
    Provides {
        /// the service name
        service_name: Option<TreeId>,
        /// the implementation names
        impl_names: List<TreeId>,
    },
    /// Java: JCTree.JCRequires ('requires' ('static' | 'transitive')* ModuleName
    /// ';')
    Requires {
        /// true for a transitive requires
        is_transitive: bool,
        /// true for a static-phase requires
        is_static_phase: bool,
        /// the required module name
        module_name: Option<TreeId>,
        /// the directive (filled during Enter; an arena value)
        directive: Option<DirectiveId>,
    },
    /// Java: JCTree.JCUses
    Uses {
        /// the service name
        qualid: Option<TreeId>,
    },
    /// Java: JCTree.JCErroneous
    Erroneous {
        /// the erroneous trees
        errs: List<TreeId>,
    },
    /// Java: JCTree.LetExpr (a synthetic let expression, ala scheme)
    LetExpr {
        /// the definitions
        defs: List<TreeId>,
        /// the expression
        expr: Option<TreeId>,
        /// true if a expr should be run through Gen.genCond
        needs_cond: bool,
    },
}

/// The number of concrete node variants (upstream JCTree concrete classes; the
/// 1:1 list is documented in the module doc). Used by the tests to assert the
/// coverage of the upstream node set.
pub const TREE_KIND_VARIANT_COUNT: usize = 67;

/// Java: JCTree.Visitor
///
/// PORTING_RULES: ADR 0001 (arena + IDs):
/// - the upstream "public void visitXxx(JCXxx that) { visitTree(that); }"
///   methods become trait methods with the CompilerContext passed explicitly
///   (the visitor needs the arena to read the node);
/// - visitTree is the default: it raises the upstream Assert.error();
/// - the accept dispatch (tree.accept(visitor)) becomes the free function
///   accept(ctx, tree, visitor), matching the node's Tag.
pub trait TreeVisitor {
    /// Java: public void visitTree(JCTree that) { Assert.error(); }
    fn visit_tree(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        panic!(
            "AssertionError: visitTree tag={:?} pos={}",
            ctx.tree(tree).tag,
            ctx.tree(tree).pos
        );
    }

    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_skip(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_default_case_label(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_type_bound_kind(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_erroneous(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_tree(ctx, tree);
    }
}

thread_local! {
    static ACCEPT_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct AcceptDepthGuard;
impl Drop for AcceptDepthGuard {
    fn drop(&mut self) {
        ACCEPT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Java: tree.accept(visitor) - dispatch by the node tag.
pub fn accept(ctx: &mut CompilerContext, tree: TreeId, visitor: &mut dyn TreeVisitor) {
    let depth = ACCEPT_DEPTH.with(|d| {
        let next = d.get() + 1;
        d.set(next);
        next
    });
    let _depth_guard = AcceptDepthGuard;
    if depth > 1000
        && let Ok(limit) = std::env::var("DSH_RECURSION_LIMIT")
        && let Ok(limit) = limit.parse::<usize>()
        && depth > limit
    {
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        let frames: Vec<&str> = bt.lines().take(30).collect();
        eprintln!(
            "[RECURSION] depth={} tag={:?} pos={}\n{}",
            depth,
            ctx.tree(tree).tag,
            ctx.tree(tree).pos,
            frames.join("\n")
        );
        panic!("recursion limit exceeded");
    }
    match ctx.tree(tree).tag {
        Tag::TOPLEVEL => visitor.visit_top_level(ctx, tree),
        Tag::PACKAGEDEF => visitor.visit_package_def(ctx, tree),
        Tag::IMPORT => visitor.visit_import(ctx, tree),
        Tag::CLASSDEF => visitor.visit_class_def(ctx, tree),
        Tag::METHODDEF => visitor.visit_method_def(ctx, tree),
        Tag::VARDEF => visitor.visit_var_def(ctx, tree),
        Tag::SKIP => visitor.visit_skip(ctx, tree),
        Tag::BLOCK => visitor.visit_block(ctx, tree),
        Tag::DOLOOP => visitor.visit_do_loop(ctx, tree),
        Tag::WHILELOOP => visitor.visit_while_loop(ctx, tree),
        Tag::FORLOOP => visitor.visit_for_loop(ctx, tree),
        Tag::FOREACHLOOP => visitor.visit_foreach_loop(ctx, tree),
        Tag::LABELLED => visitor.visit_labelled(ctx, tree),
        Tag::SWITCH => visitor.visit_switch(ctx, tree),
        Tag::CASE => visitor.visit_case(ctx, tree),
        Tag::SWITCH_EXPRESSION => visitor.visit_switch_expression(ctx, tree),
        Tag::SYNCHRONIZED => visitor.visit_synchronized(ctx, tree),
        Tag::TRY => visitor.visit_try(ctx, tree),
        Tag::CATCH => visitor.visit_catch(ctx, tree),
        Tag::CONDEXPR => visitor.visit_conditional(ctx, tree),
        Tag::IF => visitor.visit_if(ctx, tree),
        Tag::EXEC => visitor.visit_exec(ctx, tree),
        Tag::BREAK => visitor.visit_break(ctx, tree),
        Tag::YIELD => visitor.visit_yield(ctx, tree),
        Tag::CONTINUE => visitor.visit_continue(ctx, tree),
        Tag::RETURN => visitor.visit_return(ctx, tree),
        Tag::THROW => visitor.visit_throw(ctx, tree),
        Tag::ASSERT => visitor.visit_assert(ctx, tree),
        Tag::APPLY => visitor.visit_apply(ctx, tree),
        Tag::NEWCLASS => visitor.visit_new_class(ctx, tree),
        Tag::NEWARRAY => visitor.visit_new_array(ctx, tree),
        Tag::LAMBDA => visitor.visit_lambda(ctx, tree),
        Tag::PARENS => visitor.visit_parens(ctx, tree),
        Tag::ASSIGN => visitor.visit_assign(ctx, tree),
        Tag::BITOR_ASG
        | Tag::BITXOR_ASG
        | Tag::BITAND_ASG
        | Tag::SL_ASG
        | Tag::SR_ASG
        | Tag::USR_ASG
        | Tag::PLUS_ASG
        | Tag::MINUS_ASG
        | Tag::MUL_ASG
        | Tag::DIV_ASG
        | Tag::MOD_ASG => visitor.visit_assignop(ctx, tree),
        Tag::POS
        | Tag::NEG
        | Tag::NOT
        | Tag::COMPL
        | Tag::PREINC
        | Tag::PREDEC
        | Tag::POSTINC
        | Tag::POSTDEC
        | Tag::NULLCHK => visitor.visit_unary(ctx, tree),
        Tag::OR
        | Tag::AND
        | Tag::BITOR
        | Tag::BITXOR
        | Tag::BITAND
        | Tag::EQ
        | Tag::NE
        | Tag::LT
        | Tag::GT
        | Tag::LE
        | Tag::GE
        | Tag::SL
        | Tag::SR
        | Tag::USR
        | Tag::PLUS
        | Tag::MINUS
        | Tag::MUL
        | Tag::DIV
        | Tag::MOD => visitor.visit_binary(ctx, tree),
        Tag::TYPECAST => visitor.visit_type_cast(ctx, tree),
        Tag::TYPETEST => visitor.visit_type_test(ctx, tree),
        Tag::BINDINGPATTERN => visitor.visit_binding_pattern(ctx, tree),
        Tag::DEFAULTCASELABEL => visitor.visit_default_case_label(ctx, tree),
        Tag::PARENTHESIZEDPATTERN => visitor.visit_parenthesized_pattern(ctx, tree),
        Tag::GUARDPATTERN => visitor.visit_guard_pattern(ctx, tree),
        Tag::INDEXED => visitor.visit_indexed(ctx, tree),
        Tag::SELECT => visitor.visit_select(ctx, tree),
        Tag::REFERENCE => visitor.visit_reference(ctx, tree),
        Tag::IDENT => visitor.visit_ident(ctx, tree),
        Tag::LITERAL => visitor.visit_literal(ctx, tree),
        Tag::TYPEIDENT => visitor.visit_type_ident(ctx, tree),
        Tag::TYPEARRAY => visitor.visit_type_array(ctx, tree),
        Tag::TYPEAPPLY => visitor.visit_type_apply(ctx, tree),
        Tag::TYPEUNION => visitor.visit_type_union(ctx, tree),
        Tag::TYPEINTERSECTION => visitor.visit_type_intersection(ctx, tree),
        Tag::TYPEPARAMETER => visitor.visit_type_parameter(ctx, tree),
        Tag::WILDCARD => visitor.visit_wildcard(ctx, tree),
        Tag::TYPEBOUNDKIND => visitor.visit_type_bound_kind(ctx, tree),
        Tag::ANNOTATION | Tag::TYPE_ANNOTATION => visitor.visit_annotation(ctx, tree),
        Tag::MODIFIERS => visitor.visit_modifiers(ctx, tree),
        Tag::ANNOTATED_TYPE => visitor.visit_annotated_type(ctx, tree),
        Tag::ERRONEOUS => visitor.visit_erroneous(ctx, tree),
        Tag::MODULEDEF => visitor.visit_module_def(ctx, tree),
        Tag::EXPORTS => visitor.visit_exports(ctx, tree),
        Tag::OPENS => visitor.visit_opens(ctx, tree),
        Tag::PROVIDES => visitor.visit_provides(ctx, tree),
        Tag::REQUIRES => visitor.visit_requires(ctx, tree),
        Tag::USES => visitor.visit_uses(ctx, tree),
        Tag::LETEXPR => visitor.visit_let_expr(ctx, tree),
        Tag::NO_TAG => visitor.visit_tree(ctx, tree),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::name::NameTableHandle;

    fn name(text: &[u16]) -> Name {
        let table: Rc<NameTableHandle> = NameTableHandle::unshared();
        table.from_chars(text, 0, text.len() as i32)
    }

    fn empty() -> List<TreeId> {
        List::nil()
    }

    #[test]
    fn tag_values_are_complete() {
        const ALL: &[Tag] = &[
            Tag::NO_TAG,
            Tag::TOPLEVEL,
            Tag::PACKAGEDEF,
            Tag::IMPORT,
            Tag::CLASSDEF,
            Tag::METHODDEF,
            Tag::VARDEF,
            Tag::SKIP,
            Tag::BLOCK,
            Tag::DOLOOP,
            Tag::WHILELOOP,
            Tag::FORLOOP,
            Tag::FOREACHLOOP,
            Tag::LABELLED,
            Tag::SWITCH,
            Tag::CASE,
            Tag::SWITCH_EXPRESSION,
            Tag::SYNCHRONIZED,
            Tag::TRY,
            Tag::CATCH,
            Tag::CONDEXPR,
            Tag::IF,
            Tag::EXEC,
            Tag::BREAK,
            Tag::YIELD,
            Tag::CONTINUE,
            Tag::RETURN,
            Tag::THROW,
            Tag::ASSERT,
            Tag::APPLY,
            Tag::NEWCLASS,
            Tag::NEWARRAY,
            Tag::LAMBDA,
            Tag::PARENS,
            Tag::ASSIGN,
            Tag::TYPECAST,
            Tag::TYPETEST,
            Tag::BINDINGPATTERN,
            Tag::DEFAULTCASELABEL,
            Tag::GUARDPATTERN,
            Tag::PARENTHESIZEDPATTERN,
            Tag::INDEXED,
            Tag::SELECT,
            Tag::REFERENCE,
            Tag::IDENT,
            Tag::LITERAL,
            Tag::TYPEIDENT,
            Tag::TYPEARRAY,
            Tag::TYPEAPPLY,
            Tag::TYPEUNION,
            Tag::TYPEINTERSECTION,
            Tag::TYPEPARAMETER,
            Tag::WILDCARD,
            Tag::TYPEBOUNDKIND,
            Tag::ANNOTATION,
            Tag::TYPE_ANNOTATION,
            Tag::MODIFIERS,
            Tag::ANNOTATED_TYPE,
            Tag::ERRONEOUS,
            Tag::POS,
            Tag::NEG,
            Tag::NOT,
            Tag::COMPL,
            Tag::PREINC,
            Tag::PREDEC,
            Tag::POSTINC,
            Tag::POSTDEC,
            Tag::NULLCHK,
            Tag::OR,
            Tag::AND,
            Tag::BITOR,
            Tag::BITXOR,
            Tag::BITAND,
            Tag::EQ,
            Tag::NE,
            Tag::LT,
            Tag::GT,
            Tag::LE,
            Tag::GE,
            Tag::SL,
            Tag::SR,
            Tag::USR,
            Tag::PLUS,
            Tag::MINUS,
            Tag::MUL,
            Tag::DIV,
            Tag::MOD,
            Tag::BITOR_ASG,
            Tag::BITXOR_ASG,
            Tag::BITAND_ASG,
            Tag::SL_ASG,
            Tag::SR_ASG,
            Tag::USR_ASG,
            Tag::PLUS_ASG,
            Tag::MINUS_ASG,
            Tag::MUL_ASG,
            Tag::DIV_ASG,
            Tag::MOD_ASG,
            Tag::MODULEDEF,
            Tag::EXPORTS,
            Tag::OPENS,
            Tag::PROVIDES,
            Tag::REQUIRES,
            Tag::USES,
            Tag::LETEXPR,
        ];
        assert_eq!(ALL.len(), 105, "upstream JCTree.Tag has 105 values");
        for (index, tag) in ALL.iter().enumerate() {
            assert_eq!(tag.ordinal(), index, "unexpected ordinal for {tag:?}");
        }
        assert_eq!(Tag::POS.ordinal(), 59);
        assert_eq!(Tag::MOD.ordinal(), 86);
        assert_eq!(Tag::LETEXPR.ordinal(), 104);
    }

    #[test]
    fn tag_operator_semantics() {
        assert_eq!(Tag::get_number_of_operators(), 28);
        assert_eq!(Tag::POS.operator_index(), 0);
        assert_eq!(Tag::MOD.operator_index(), 27);
        assert_eq!(Tag::PLUS.operator_index(), 23);
        assert_eq!(Tag::BITOR_ASG.no_assign_op(), Tag::BITOR);
        assert_eq!(Tag::MOD_ASG.no_assign_op(), Tag::MOD);
        assert!(Tag::BITOR_ASG.is_assignop());
        assert!(!Tag::PLUS.is_assignop());
        assert!(Tag::POSTINC.is_post_unary_op());
        assert!(Tag::POSTDEC.is_post_unary_op());
        assert!(!Tag::PREINC.is_post_unary_op());
        assert!(Tag::PREINC.is_inc_or_dec_unary_op());
        assert!(Tag::POSTDEC.is_inc_or_dec_unary_op());
        assert!(!Tag::POS.is_inc_or_dec_unary_op());
    }

    #[test]
    #[should_panic(expected = "noAssignOp() method is not available for non assignment tags")]
    fn tag_no_assign_op_panics_for_non_assignment_tags() {
        let _ = Tag::PLUS.no_assign_op();
    }

    #[test]
    fn tree_fields_and_setters() {
        let mut tree = JCTree {
            pos: 7,
            ty: None,
            tag: Tag::IDENT,
            kind: TreeKind::Ident {
                name: name(&[0x41]),
                sym: None,
            },
        };
        assert_eq!(tree.get_tag(), Tag::IDENT);
        assert!(tree.has_tag(Tag::IDENT));
        assert!(!tree.has_tag(Tag::LITERAL));
        tree.set_pos(3).set_type(Some(TypeId::new(1)));
        assert_eq!(tree.pos, 3);
        assert_eq!(tree.ty, Some(TypeId::new(1)));
        assert_eq!(tree.set_pos(9).pos, 9);
    }

    #[test]
    fn tree_kind_variant_count_and_annotation_directive_kinds() {
        assert_eq!(TREE_KIND_VARIANT_COUNT, 67);
        let annotation = JCTree {
            pos: 1,
            ty: None,
            tag: Tag::ANNOTATION,
            kind: TreeKind::Annotation {
                annotation_type: Some(TreeId::new(1)),
                args: empty(),
                attribute: Some(AttributeId::new(0)),
            },
        };
        assert!(annotation.has_tag(Tag::ANNOTATION));
        assert!(matches!(
            annotation.kind,
            TreeKind::Annotation {
                attribute: Some(_),
                ..
            }
        ));
        let exports = TreeKind::Exports {
            qualid: Some(TreeId::new(2)),
            module_names: Some(empty()),
            directive: Some(DirectiveId::new(0)),
        };
        assert!(matches!(
            exports,
            TreeKind::Exports {
                directive: Some(_),
                ..
            }
        ));
        let opens = TreeKind::Opens {
            qualid: None,
            module_names: Some(empty()),
            directive: None,
        };
        assert!(matches!(opens, TreeKind::Opens { .. }));
        let requires = TreeKind::Requires {
            is_transitive: true,
            is_static_phase: false,
            module_name: Some(TreeId::new(3)),
            directive: None,
        };
        assert!(matches!(
            requires,
            TreeKind::Requires {
                is_transitive: true,
                is_static_phase: false,
                ..
            }
        ));
    }

    #[test]
    fn representative_tree_kinds() {
        let class = TreeKind::ClassDecl {
            mods: None,
            name: name(&[0x43]),
            typarams: empty(),
            extending: None,
            implementing: empty(),
            permitting: empty(),
            defs: empty(),
            sym: None,
        };
        assert!(matches!(class, TreeKind::ClassDecl { .. }));

        let literal = TreeKind::Literal {
            typetag: TypeTag::INT,
            value: Some(LiteralValue::Integer(42)),
        };
        match literal {
            TreeKind::Literal { typetag, value } => {
                assert_eq!(typetag, TypeTag::INT);
                assert_eq!(value, Some(LiteralValue::Integer(42)));
            }
            _ => panic!("expected a literal"),
        }

        let case = TreeKind::Case {
            case_kind: CaseKind::RULE,
            labels: empty(),
            stats: empty(),
            body: None,
            completes_normally: false,
        };
        assert!(matches!(
            case,
            TreeKind::Case {
                case_kind: CaseKind::RULE,
                ..
            }
        ));

        let bound = TreeKind::TypeBoundKind {
            kind: BoundKind::EXTENDS,
        };
        assert!(matches!(
            bound,
            TreeKind::TypeBoundKind {
                kind: BoundKind::EXTENDS
            }
        ));

        let unit = TreeKind::CompilationUnit {
            defs: empty(),
            sourcefile: None,
            modle: None,
            locn: None,
            packge: None,
            toplevel_scope: None,
            named_import_scope: None,
            star_import_scope: None,
            line_map: None,
            doc_comments: None,
            end_positions: None,
        };
        assert!(matches!(
            unit,
            TreeKind::CompilationUnit {
                doc_comments: None,
                end_positions: None,
                ..
            }
        ));

        let lambda = TreeKind::Lambda {
            params: empty(),
            body: None,
            can_complete_normally: true,
            param_kind: ParameterKind::IMPLICIT,
            poly_kind: PolyKind::POLY,
            target: None,
        };
        assert!(matches!(
            lambda,
            TreeKind::Lambda {
                poly_kind: PolyKind::POLY,
                ..
            }
        ));

        let reference = TreeKind::MemberReference {
            mode: ReferenceMode::INVOKE,
            kind: Some(ReferenceKind::UNBOUND),
            name: name(&[0x4d]),
            expr: None,
            typeargs: Some(empty()),
            sym: None,
            varargs_element: None,
            ref_poly_kind: PolyKind::POLY,
            owner_accessible: false,
            referent_type: None,
            overload_kind: None,
            poly_kind: PolyKind::POLY,
            target: None,
        };
        assert!(matches!(
            reference,
            TreeKind::MemberReference {
                mode: ReferenceMode::INVOKE,
                kind: Some(ReferenceKind::UNBOUND),
                ..
            }
        ));

        let module = TreeKind::ModuleDecl {
            mods: None,
            ty: None,
            kind: ModuleKind::STRONG,
            qual_id: None,
            directives: Some(empty()),
            sym: None,
        };
        assert!(matches!(
            module,
            TreeKind::ModuleDecl {
                kind: ModuleKind::STRONG,
                ..
            }
        ));

        let erroneous = TreeKind::Erroneous { errs: empty() };
        assert!(matches!(erroneous, TreeKind::Erroneous { .. }));

        let let_expr = TreeKind::LetExpr {
            defs: empty(),
            expr: None,
            needs_cond: false,
        };
        assert!(matches!(let_expr, TreeKind::LetExpr { .. }));
    }

    #[test]
    fn literal_value_variants() {
        let values = [
            LiteralValue::Integer(1),
            LiteralValue::Long(2_i64),
            LiteralValue::Float(1.5),
            LiteralValue::Double(2.5),
            LiteralValue::Character(0x41),
            LiteralValue::Boolean(true),
            LiteralValue::String(JavaString::from("s")),
        ];
        assert_eq!(values.len(), 7);
        assert_eq!(values[0], LiteralValue::Integer(1));
        assert_eq!(values[3], LiteralValue::Double(2.5));
        assert_eq!(values[6], LiteralValue::String(JavaString::from("s")));
    }

    #[test]
    fn reference_kind_modes() {
        assert_eq!(ReferenceKind::SUPER.mode(), ReferenceMode::INVOKE);
        assert_eq!(ReferenceKind::BOUND.mode(), ReferenceMode::INVOKE);
        assert_eq!(ReferenceKind::IMPLICIT_INNER.mode(), ReferenceMode::NEW);
        assert_eq!(ReferenceKind::ARRAY_CTOR.mode(), ReferenceMode::NEW);
        assert!(ReferenceKind::UNBOUND.is_unbound());
        assert!(!ReferenceKind::STATIC.is_unbound());
    }

    #[test]
    fn poly_and_parameter_kinds() {
        assert_ne!(PolyKind::POLY, PolyKind::STANDALONE);
        assert_ne!(ParameterKind::IMPLICIT, ParameterKind::EXPLICIT);
    }
}

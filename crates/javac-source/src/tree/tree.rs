//! Common interface for all nodes in an abstract syntax tree.
//!
//!
//! * Java interface -> Rust trait.  The nested `Tree.Kind` enum is declared in
//!   this module, complete (111 constants) and in the Java
//!   declaration order (javac's code/TypeTag.getKindLiteral depends on it).
//! * `Kind.asInterface()` returns `Class<? extends Tree>` upstream; the Rust
//!   method returns the associated interface name as a `&'static str` (no
//!   reflection in the port, registered).
//! * Java `Tree.accept(TreeVisitor, D)` is generic and cannot be called on
//!   `&dyn Tree`; it keeps `where Self: Sized`, and the util layer reproduces
//!   the dispatch through getKind() plus the object-conversion accessors below
//!   (docs/PORTING_RULES.md §13/§14).
//! * The `as_xxx` accessors are the Rust form of `instanceof`/class cast for
//!   trees (same contract as javax.lang.model): default None, concrete
//!   implementations override the applicable one.

/// Common interface for all nodes in an abstract syntax tree.
///
/// <p><b>WARNING:</b> This interface and its sub-interfaces are
/// subject to change as the Java programming language evolves.
/// These interfaces are implemented by the JDK Java compiler (javac)
/// and should not be implemented either directly or indirectly by
/// other applications.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
///
/// @since 1.6
use javac_strings::JavaString;
use super::annotated_type_tree::AnnotatedTypeTree;
use super::annotation_tree::AnnotationTree;
use super::array_access_tree::ArrayAccessTree;
use super::array_type_tree::ArrayTypeTree;
use super::assert_tree::AssertTree;
use super::assignment_tree::AssignmentTree;
use super::binary_tree::BinaryTree;
use super::binding_pattern_tree::BindingPatternTree;
use super::block_tree::BlockTree;
use super::break_tree::BreakTree;
use super::case_label_tree::CaseLabelTree;
use super::case_tree::CaseTree;
use super::catch_tree::CatchTree;
use super::class_tree::ClassTree;
use super::compilation_unit_tree::CompilationUnitTree;
use super::compound_assignment_tree::CompoundAssignmentTree;
use super::conditional_expression_tree::ConditionalExpressionTree;
use super::continue_tree::ContinueTree;
use super::default_case_label_tree::DefaultCaseLabelTree;
use super::directive_tree::DirectiveTree;
use super::do_while_loop_tree::DoWhileLoopTree;
use super::empty_statement_tree::EmptyStatementTree;
use super::enhanced_for_loop_tree::EnhancedForLoopTree;
use super::erroneous_tree::ErroneousTree;
use super::exports_tree::ExportsTree;
use super::expression_statement_tree::ExpressionStatementTree;
use super::expression_tree::ExpressionTree;
use super::for_loop_tree::ForLoopTree;
use super::guarded_pattern_tree::GuardedPatternTree;
use super::identifier_tree::IdentifierTree;
use super::if_tree::IfTree;
use super::import_tree::ImportTree;
use super::instance_of_tree::InstanceOfTree;
use super::intersection_type_tree::IntersectionTypeTree;
use super::labeled_statement_tree::LabeledStatementTree;
use super::lambda_expression_tree::LambdaExpressionTree;
use super::literal_tree::LiteralTree;
use super::member_reference_tree::MemberReferenceTree;
use super::member_select_tree::MemberSelectTree;
use super::method_invocation_tree::MethodInvocationTree;
use super::method_tree::MethodTree;
use super::modifiers_tree::ModifiersTree;
use super::module_tree::ModuleTree;
use super::new_array_tree::NewArrayTree;
use super::new_class_tree::NewClassTree;
use super::opens_tree::OpensTree;
use super::package_tree::PackageTree;
use super::parameterized_type_tree::ParameterizedTypeTree;
use super::parenthesized_pattern_tree::ParenthesizedPatternTree;
use super::parenthesized_tree::ParenthesizedTree;
use super::pattern_tree::PatternTree;
use super::primitive_type_tree::PrimitiveTypeTree;
use super::provides_tree::ProvidesTree;
use super::requires_tree::RequiresTree;
use super::return_tree::ReturnTree;
use super::statement_tree::StatementTree;
use super::switch_expression_tree::SwitchExpressionTree;
use super::switch_tree::SwitchTree;
use super::synchronized_tree::SynchronizedTree;
use super::throw_tree::ThrowTree;
use super::tree_visitor::TreeVisitor;
use super::try_tree::TryTree;
use super::type_cast_tree::TypeCastTree;
use super::type_parameter_tree::TypeParameterTree;
use super::unary_tree::UnaryTree;
use super::union_type_tree::UnionTypeTree;
use super::uses_tree::UsesTree;
use super::variable_tree::VariableTree;
use super::while_loop_tree::WhileLoopTree;
use super::wildcard_tree::WildcardTree;
use super::yield_tree::YieldTree;

pub trait Tree {
    /// Returns the kind of this tree.
    ///
    /// @return the kind of this tree
    fn get_kind(&self) -> Kind;

    /// Accept method used to implement the visitor pattern.  The
    /// visitor pattern is used to implement operations on trees.
    ///
    /// @param <R> the result type of this operation
    /// @param <D> the type of additional data
    /// @param visitor the visitor to be called
    /// @param data a value to be passed to the visitor
    /// @return the result returned from calling the visitor
    ///
    /// `where Self: Sized` keeps Tree object safe; the dispatch is
    /// reproduced by the util layer.
    fn accept<R, D>(&self, visitor: &mut dyn TreeVisitor<R, D>, data: D) -> R
    where
        Self: Sized;

    /// object conversion for `instanceof AnnotatedTypeTree` (see file header).
    fn as_annotated_type_tree(&self) -> Option<&dyn AnnotatedTypeTree> {
        None
    }

    /// object conversion for `instanceof AnnotationTree` (see file header).
    fn as_annotation_tree(&self) -> Option<&dyn AnnotationTree> {
        None
    }

    /// object conversion for `instanceof ArrayAccessTree` (see file header).
    fn as_array_access_tree(&self) -> Option<&dyn ArrayAccessTree> {
        None
    }

    /// object conversion for `instanceof ArrayTypeTree` (see file header).
    fn as_array_type_tree(&self) -> Option<&dyn ArrayTypeTree> {
        None
    }

    /// object conversion for `instanceof AssertTree` (see file header).
    fn as_assert_tree(&self) -> Option<&dyn AssertTree> {
        None
    }

    /// object conversion for `instanceof AssignmentTree` (see file header).
    fn as_assignment_tree(&self) -> Option<&dyn AssignmentTree> {
        None
    }

    /// object conversion for `instanceof BinaryTree` (see file header).
    fn as_binary_tree(&self) -> Option<&dyn BinaryTree> {
        None
    }

    /// object conversion for `instanceof BindingPatternTree` (see file header).
    fn as_binding_pattern_tree(&self) -> Option<&dyn BindingPatternTree> {
        None
    }

    /// object conversion for `instanceof BlockTree` (see file header).
    fn as_block_tree(&self) -> Option<&dyn BlockTree> {
        None
    }

    /// object conversion for `instanceof BreakTree` (see file header).
    fn as_break_tree(&self) -> Option<&dyn BreakTree> {
        None
    }

    /// object conversion for `instanceof CaseLabelTree` (see file header).
    fn as_case_label_tree(&self) -> Option<&dyn CaseLabelTree> {
        None
    }

    /// object conversion for `instanceof CaseTree` (see file header).
    fn as_case_tree(&self) -> Option<&dyn CaseTree> {
        None
    }

    /// object conversion for `instanceof CatchTree` (see file header).
    fn as_catch_tree(&self) -> Option<&dyn CatchTree> {
        None
    }

    /// object conversion for `instanceof ClassTree` (see file header).
    fn as_class_tree(&self) -> Option<&dyn ClassTree> {
        None
    }

    /// object conversion for `instanceof CompilationUnitTree` (see file header).
    fn as_compilation_unit_tree(&self) -> Option<&dyn CompilationUnitTree> {
        None
    }

    /// object conversion for `instanceof CompoundAssignmentTree` (see file header).
    fn as_compound_assignment_tree(&self) -> Option<&dyn CompoundAssignmentTree> {
        None
    }

    /// object conversion for `instanceof ConditionalExpressionTree` (see file header).
    fn as_conditional_expression_tree(&self) -> Option<&dyn ConditionalExpressionTree> {
        None
    }

    /// object conversion for `instanceof ContinueTree` (see file header).
    fn as_continue_tree(&self) -> Option<&dyn ContinueTree> {
        None
    }

    /// object conversion for `instanceof DefaultCaseLabelTree` (see file header).
    fn as_default_case_label_tree(&self) -> Option<&dyn DefaultCaseLabelTree> {
        None
    }

    /// object conversion for `instanceof DirectiveTree` (see file header).
    fn as_directive_tree(&self) -> Option<&dyn DirectiveTree> {
        None
    }

    /// object conversion for `instanceof DoWhileLoopTree` (see file header).
    fn as_do_while_loop_tree(&self) -> Option<&dyn DoWhileLoopTree> {
        None
    }

    /// object conversion for `instanceof EmptyStatementTree` (see file header).
    fn as_empty_statement_tree(&self) -> Option<&dyn EmptyStatementTree> {
        None
    }

    /// object conversion for `instanceof EnhancedForLoopTree` (see file header).
    fn as_enhanced_for_loop_tree(&self) -> Option<&dyn EnhancedForLoopTree> {
        None
    }

    /// object conversion for `instanceof ErroneousTree` (see file header).
    fn as_erroneous_tree(&self) -> Option<&dyn ErroneousTree> {
        None
    }

    /// object conversion for `instanceof ExportsTree` (see file header).
    fn as_exports_tree(&self) -> Option<&dyn ExportsTree> {
        None
    }

    /// object conversion for `instanceof ExpressionStatementTree` (see file header).
    fn as_expression_statement_tree(&self) -> Option<&dyn ExpressionStatementTree> {
        None
    }

    /// object conversion for `instanceof ExpressionTree` (see file header).
    fn as_expression_tree(&self) -> Option<&dyn ExpressionTree> {
        None
    }

    /// object conversion for `instanceof ForLoopTree` (see file header).
    fn as_for_loop_tree(&self) -> Option<&dyn ForLoopTree> {
        None
    }

    /// object conversion for `instanceof GuardedPatternTree` (see file header).
    fn as_guarded_pattern_tree(&self) -> Option<&dyn GuardedPatternTree> {
        None
    }

    /// object conversion for `instanceof IdentifierTree` (see file header).
    fn as_identifier_tree(&self) -> Option<&dyn IdentifierTree> {
        None
    }

    /// object conversion for `instanceof IfTree` (see file header).
    fn as_if_tree(&self) -> Option<&dyn IfTree> {
        None
    }

    /// object conversion for `instanceof ImportTree` (see file header).
    fn as_import_tree(&self) -> Option<&dyn ImportTree> {
        None
    }

    /// object conversion for `instanceof InstanceOfTree` (see file header).
    fn as_instance_of_tree(&self) -> Option<&dyn InstanceOfTree> {
        None
    }

    /// object conversion for `instanceof IntersectionTypeTree` (see file header).
    fn as_intersection_type_tree(&self) -> Option<&dyn IntersectionTypeTree> {
        None
    }

    /// object conversion for `instanceof LabeledStatementTree` (see file header).
    fn as_labeled_statement_tree(&self) -> Option<&dyn LabeledStatementTree> {
        None
    }

    /// object conversion for `instanceof LambdaExpressionTree` (see file header).
    fn as_lambda_expression_tree(&self) -> Option<&dyn LambdaExpressionTree> {
        None
    }

    /// object conversion for `instanceof LiteralTree` (see file header).
    fn as_literal_tree(&self) -> Option<&dyn LiteralTree> {
        None
    }

    /// object conversion for `instanceof MemberReferenceTree` (see file header).
    fn as_member_reference_tree(&self) -> Option<&dyn MemberReferenceTree> {
        None
    }

    /// object conversion for `instanceof MemberSelectTree` (see file header).
    fn as_member_select_tree(&self) -> Option<&dyn MemberSelectTree> {
        None
    }

    /// object conversion for `instanceof MethodInvocationTree` (see file header).
    fn as_method_invocation_tree(&self) -> Option<&dyn MethodInvocationTree> {
        None
    }

    /// object conversion for `instanceof MethodTree` (see file header).
    fn as_method_tree(&self) -> Option<&dyn MethodTree> {
        None
    }

    /// object conversion for `instanceof ModifiersTree` (see file header).
    fn as_modifiers_tree(&self) -> Option<&dyn ModifiersTree> {
        None
    }

    /// object conversion for `instanceof ModuleTree` (see file header).
    fn as_module_tree(&self) -> Option<&dyn ModuleTree> {
        None
    }

    /// object conversion for `instanceof NewArrayTree` (see file header).
    fn as_new_array_tree(&self) -> Option<&dyn NewArrayTree> {
        None
    }

    /// object conversion for `instanceof NewClassTree` (see file header).
    fn as_new_class_tree(&self) -> Option<&dyn NewClassTree> {
        None
    }

    /// object conversion for `instanceof OpensTree` (see file header).
    fn as_opens_tree(&self) -> Option<&dyn OpensTree> {
        None
    }

    /// object conversion for `instanceof PackageTree` (see file header).
    fn as_package_tree(&self) -> Option<&dyn PackageTree> {
        None
    }

    /// object conversion for `instanceof ParameterizedTypeTree` (see file header).
    fn as_parameterized_type_tree(&self) -> Option<&dyn ParameterizedTypeTree> {
        None
    }

    /// object conversion for `instanceof ParenthesizedPatternTree` (see file header).
    fn as_parenthesized_pattern_tree(&self) -> Option<&dyn ParenthesizedPatternTree> {
        None
    }

    /// object conversion for `instanceof ParenthesizedTree` (see file header).
    fn as_parenthesized_tree(&self) -> Option<&dyn ParenthesizedTree> {
        None
    }

    /// object conversion for `instanceof PatternTree` (see file header).
    fn as_pattern_tree(&self) -> Option<&dyn PatternTree> {
        None
    }

    /// object conversion for `instanceof PrimitiveTypeTree` (see file header).
    fn as_primitive_type_tree(&self) -> Option<&dyn PrimitiveTypeTree> {
        None
    }

    /// object conversion for `instanceof ProvidesTree` (see file header).
    fn as_provides_tree(&self) -> Option<&dyn ProvidesTree> {
        None
    }

    /// object conversion for `instanceof RequiresTree` (see file header).
    fn as_requires_tree(&self) -> Option<&dyn RequiresTree> {
        None
    }

    /// object conversion for `instanceof ReturnTree` (see file header).
    fn as_return_tree(&self) -> Option<&dyn ReturnTree> {
        None
    }

    /// object conversion for `instanceof StatementTree` (see file header).
    fn as_statement_tree(&self) -> Option<&dyn StatementTree> {
        None
    }

    /// object conversion for `instanceof SwitchExpressionTree` (see file header).
    fn as_switch_expression_tree(&self) -> Option<&dyn SwitchExpressionTree> {
        None
    }

    /// object conversion for `instanceof SwitchTree` (see file header).
    fn as_switch_tree(&self) -> Option<&dyn SwitchTree> {
        None
    }

    /// object conversion for `instanceof SynchronizedTree` (see file header).
    fn as_synchronized_tree(&self) -> Option<&dyn SynchronizedTree> {
        None
    }

    /// object conversion for `instanceof ThrowTree` (see file header).
    fn as_throw_tree(&self) -> Option<&dyn ThrowTree> {
        None
    }

    /// object conversion for `instanceof TryTree` (see file header).
    fn as_try_tree(&self) -> Option<&dyn TryTree> {
        None
    }

    /// object conversion for `instanceof TypeCastTree` (see file header).
    fn as_type_cast_tree(&self) -> Option<&dyn TypeCastTree> {
        None
    }

    /// object conversion for `instanceof TypeParameterTree` (see file header).
    fn as_type_parameter_tree(&self) -> Option<&dyn TypeParameterTree> {
        None
    }

    /// object conversion for `instanceof UnaryTree` (see file header).
    fn as_unary_tree(&self) -> Option<&dyn UnaryTree> {
        None
    }

    /// object conversion for `instanceof UnionTypeTree` (see file header).
    fn as_union_type_tree(&self) -> Option<&dyn UnionTypeTree> {
        None
    }

    /// object conversion for `instanceof UsesTree` (see file header).
    fn as_uses_tree(&self) -> Option<&dyn UsesTree> {
        None
    }

    /// object conversion for `instanceof VariableTree` (see file header).
    fn as_variable_tree(&self) -> Option<&dyn VariableTree> {
        None
    }

    /// object conversion for `instanceof WhileLoopTree` (see file header).
    fn as_while_loop_tree(&self) -> Option<&dyn WhileLoopTree> {
        None
    }

    /// object conversion for `instanceof WildcardTree` (see file header).
    fn as_wildcard_tree(&self) -> Option<&dyn WildcardTree> {
        None
    }

    /// object conversion for `instanceof YieldTree` (see file header).
    fn as_yield_tree(&self) -> Option<&dyn YieldTree> {
        None
    }
}

/// Enumerates all kinds of trees.
///
/// Java enum constants keep their Java names (UPPER_SNAKE_CASE)
/// and declaration order.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Used for instances of {@link AnnotatedTypeTree}
    /// representing annotated types.
    ANNOTATED_TYPE,

    /// Used for instances of {@link AnnotationTree}
    /// representing declaration annotations.
    ANNOTATION,

    /// Used for instances of {@link AnnotationTree}
    /// representing type annotations.
    TYPE_ANNOTATION,

    /// Used for instances of {@link ArrayAccessTree}.
    ARRAY_ACCESS,

    /// Used for instances of {@link ArrayTypeTree}.
    ARRAY_TYPE,

    /// Used for instances of {@link AssertTree}.
    ASSERT,

    /// Used for instances of {@link AssignmentTree}.
    ASSIGNMENT,

    /// Used for instances of {@link BlockTree}.
    BLOCK,

    /// Used for instances of {@link BreakTree}.
    BREAK,

    /// Used for instances of {@link CaseTree}.
    CASE,

    /// Used for instances of {@link CatchTree}.
    CATCH,

    /// Used for instances of {@link ClassTree} representing classes.
    CLASS,

    /// Used for instances of {@link CompilationUnitTree}.
    COMPILATION_UNIT,

    /// Used for instances of {@link ConditionalExpressionTree}.
    CONDITIONAL_EXPRESSION,

    /// Used for instances of {@link ContinueTree}.
    CONTINUE,

    /// Used for instances of {@link DoWhileLoopTree}.
    DO_WHILE_LOOP,

    /// Used for instances of {@link EnhancedForLoopTree}.
    ENHANCED_FOR_LOOP,

    /// Used for instances of {@link ExpressionStatementTree}.
    EXPRESSION_STATEMENT,

    /// Used for instances of {@link MemberSelectTree}.
    MEMBER_SELECT,

    /// Used for instances of {@link MemberReferenceTree}.
    MEMBER_REFERENCE,

    /// Used for instances of {@link ForLoopTree}.
    FOR_LOOP,

    /// Used for instances of {@link IdentifierTree}.
    IDENTIFIER,

    /// Used for instances of {@link IfTree}.
    IF,

    /// Used for instances of {@link ImportTree}.
    IMPORT,

    /// Used for instances of {@link InstanceOfTree}.
    INSTANCE_OF,

    /// Used for instances of {@link LabeledStatementTree}.
    LABELED_STATEMENT,

    /// Used for instances of {@link MethodTree}.
    METHOD,

    /// Used for instances of {@link MethodInvocationTree}.
    METHOD_INVOCATION,

    /// Used for instances of {@link ModifiersTree}.
    MODIFIERS,

    /// Used for instances of {@link NewArrayTree}.
    NEW_ARRAY,

    /// Used for instances of {@link NewClassTree}.
    NEW_CLASS,

    /// Used for instances of {@link LambdaExpressionTree}.
    LAMBDA_EXPRESSION,

    /// Used for instances of {@link PackageTree}.
    /// @since 9
    PACKAGE,

    /// Used for instances of {@link ParenthesizedTree}.
    PARENTHESIZED,

    /// Used for instances of {@link BindingPatternTree}.
    ///
    /// @since 16
    BINDING_PATTERN,

    /// Used for instances of {@link GuardedPatternTree}.
    ///
    /// @since 17
    GUARDED_PATTERN,

    /// Used for instances of {@link ParenthesizedPatternTree}.
    ///
    /// @since 17
    PARENTHESIZED_PATTERN,

    /// Used for instances of {@link DefaultCaseLabelTree}.
    ///
    /// @since 17
    DEFAULT_CASE_LABEL,

    /// Used for instances of {@link PrimitiveTypeTree}.
    PRIMITIVE_TYPE,

    /// Used for instances of {@link ReturnTree}.
    RETURN,

    /// Used for instances of {@link EmptyStatementTree}.
    EMPTY_STATEMENT,

    /// Used for instances of {@link SwitchTree}.
    SWITCH,

    /// Used for instances of {@link SwitchExpressionTree}.
    ///
    /// @since 12
    SWITCH_EXPRESSION,

    /// Used for instances of {@link SynchronizedTree}.
    SYNCHRONIZED,

    /// Used for instances of {@link ThrowTree}.
    THROW,

    /// Used for instances of {@link TryTree}.
    TRY,

    /// Used for instances of {@link ParameterizedTypeTree}.
    PARAMETERIZED_TYPE,

    /// Used for instances of {@link UnionTypeTree}.
    UNION_TYPE,

    /// Used for instances of {@link IntersectionTypeTree}.
    INTERSECTION_TYPE,

    /// Used for instances of {@link TypeCastTree}.
    TYPE_CAST,

    /// Used for instances of {@link TypeParameterTree}.
    TYPE_PARAMETER,

    /// Used for instances of {@link VariableTree}.
    VARIABLE,

    /// Used for instances of {@link WhileLoopTree}.
    WHILE_LOOP,

    /// Used for instances of {@link UnaryTree} representing postfix
    /// increment operator {@code ++}.
    POSTFIX_INCREMENT,

    /// Used for instances of {@link UnaryTree} representing postfix
    /// decrement operator {@code --}.
    POSTFIX_DECREMENT,

    /// Used for instances of {@link UnaryTree} representing prefix
    /// increment operator {@code ++}.
    PREFIX_INCREMENT,

    /// Used for instances of {@link UnaryTree} representing prefix
    /// decrement operator {@code --}.
    PREFIX_DECREMENT,

    /// Used for instances of {@link UnaryTree} representing unary plus
    /// operator {@code +}.
    UNARY_PLUS,

    /// Used for instances of {@link UnaryTree} representing unary minus
    /// operator {@code -}.
    UNARY_MINUS,

    /// Used for instances of {@link UnaryTree} representing bitwise
    /// complement operator {@code ~}.
    BITWISE_COMPLEMENT,

    /// Used for instances of {@link UnaryTree} representing logical
    /// complement operator {@code !}.
    LOGICAL_COMPLEMENT,

    /// Used for instances of {@link BinaryTree} representing
    /// multiplication {@code *}.
    MULTIPLY,

    /// Used for instances of {@link BinaryTree} representing
    /// division {@code /}.
    DIVIDE,

    /// Used for instances of {@link BinaryTree} representing
    /// remainder {@code %}.
    REMAINDER,

    /// Used for instances of {@link BinaryTree} representing
    /// addition or string concatenation {@code +}.
    PLUS,

    /// Used for instances of {@link BinaryTree} representing
    /// subtraction {@code -}.
    MINUS,

    /// Used for instances of {@link BinaryTree} representing
    /// left shift {@code <<}.
    LEFT_SHIFT,

    /// Used for instances of {@link BinaryTree} representing
    /// right shift {@code >>}.
    RIGHT_SHIFT,

    /// Used for instances of {@link BinaryTree} representing
    /// unsigned right shift {@code >>>}.
    UNSIGNED_RIGHT_SHIFT,

    /// Used for instances of {@link BinaryTree} representing
    /// less-than {@code <}.
    LESS_THAN,

    /// Used for instances of {@link BinaryTree} representing
    /// greater-than {@code >}.
    GREATER_THAN,

    /// Used for instances of {@link BinaryTree} representing
    /// less-than-equal {@code <=}.
    LESS_THAN_EQUAL,

    /// Used for instances of {@link BinaryTree} representing
    /// greater-than-equal {@code >=}.
    GREATER_THAN_EQUAL,

    /// Used for instances of {@link BinaryTree} representing
    /// equal-to {@code ==}.
    EQUAL_TO,

    /// Used for instances of {@link BinaryTree} representing
    /// not-equal-to {@code !=}.
    NOT_EQUAL_TO,

    /// Used for instances of {@link BinaryTree} representing
    /// bitwise and logical "and" {@code &}.
    AND,

    /// Used for instances of {@link BinaryTree} representing
    /// bitwise and logical "xor" {@code ^}.
    XOR,

    /// Used for instances of {@link BinaryTree} representing
    /// bitwise and logical "or" {@code |}.
    OR,

    /// Used for instances of {@link BinaryTree} representing
    /// conditional-and {@code &&}.
    CONDITIONAL_AND,

    /// Used for instances of {@link BinaryTree} representing
    /// conditional-or {@code ||}.
    CONDITIONAL_OR,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// multiplication assignment {@code *=}.
    MULTIPLY_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// division assignment {@code /=}.
    DIVIDE_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// remainder assignment {@code %=}.
    REMAINDER_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// addition or string concatenation assignment {@code +=}.
    PLUS_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// subtraction assignment {@code -=}.
    MINUS_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// left shift assignment {@code <<=}.
    LEFT_SHIFT_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// right shift assignment {@code >>=}.
    RIGHT_SHIFT_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// unsigned right shift assignment {@code >>>=}.
    UNSIGNED_RIGHT_SHIFT_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// bitwise and logical "and" assignment {@code &=}.
    AND_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// bitwise and logical "xor" assignment {@code ^=}.
    XOR_ASSIGNMENT,

    /// Used for instances of {@link CompoundAssignmentTree} representing
    /// bitwise and logical "or" assignment {@code |=}.
    OR_ASSIGNMENT,

    /// Used for instances of {@link LiteralTree} representing
    /// an integral literal expression of type {@code int}.
    INT_LITERAL,

    /// Used for instances of {@link LiteralTree} representing
    /// an integral literal expression of type {@code long}.
    LONG_LITERAL,

    /// Used for instances of {@link LiteralTree} representing
    /// a floating-point literal expression of type {@code float}.
    FLOAT_LITERAL,

    /// Used for instances of {@link LiteralTree} representing
    /// a floating-point literal expression of type {@code double}.
    DOUBLE_LITERAL,

    /// Used for instances of {@link LiteralTree} representing
    /// a boolean literal expression of type {@code boolean}.
    BOOLEAN_LITERAL,

    /// Used for instances of {@link LiteralTree} representing
    /// a character literal expression of type {@code char}.
    CHAR_LITERAL,

    /// Used for instances of {@link LiteralTree} representing
    /// a string literal expression of type {@link String}.
    STRING_LITERAL,

    /// Used for instances of {@link LiteralTree} representing
    /// the use of {@code null}.
    NULL_LITERAL,

    /// Used for instances of {@link WildcardTree} representing
    /// an unbounded wildcard type argument.
    UNBOUNDED_WILDCARD,

    /// Used for instances of {@link WildcardTree} representing
    /// an extends bounded wildcard type argument.
    EXTENDS_WILDCARD,

    /// Used for instances of {@link WildcardTree} representing
    /// a super bounded wildcard type argument.
    SUPER_WILDCARD,

    /// Used for instances of {@link ErroneousTree}.
    ERRONEOUS,

    /// Used for instances of {@link ClassTree} representing interfaces.
    INTERFACE,

    /// Used for instances of {@link ClassTree} representing enums.
    ENUM,

    /// Used for instances of {@link ClassTree} representing annotation types.
    ANNOTATION_TYPE,

    /// Used for instances of {@link ModuleTree} representing module declarations.
    MODULE,

    /// Used for instances of {@link ExportsTree} representing
    /// exports directives in a module declaration.
    EXPORTS,

    /// Used for instances of {@link ExportsTree} representing
    /// opens directives in a module declaration.
    OPENS,

    /// Used for instances of {@link ProvidesTree} representing
    /// provides directives in a module declaration.
    PROVIDES,

    /// Used for instances of {@link ClassTree} representing records.
    /// @since 16
    RECORD,

    /// Used for instances of {@link RequiresTree} representing
    /// requires directives in a module declaration.
    REQUIRES,

    /// Used for instances of {@link UsesTree} representing
    /// uses directives in a module declaration.
    USES,

    /// An implementation-reserved node. This is the not the node
    /// you are looking for.
    OTHER,

    /// Used for instances of {@link YieldTree}.
    ///
    /// @since 13
    YIELD,

}

impl Kind {
    /// Java: enum#name() (returns a Java String).
    pub fn name(&self) -> JavaString {
        JavaString::from(match self {
            Kind::ANNOTATED_TYPE => "ANNOTATED_TYPE",
            Kind::ANNOTATION => "ANNOTATION",
            Kind::TYPE_ANNOTATION => "TYPE_ANNOTATION",
            Kind::ARRAY_ACCESS => "ARRAY_ACCESS",
            Kind::ARRAY_TYPE => "ARRAY_TYPE",
            Kind::ASSERT => "ASSERT",
            Kind::ASSIGNMENT => "ASSIGNMENT",
            Kind::BLOCK => "BLOCK",
            Kind::BREAK => "BREAK",
            Kind::CASE => "CASE",
            Kind::CATCH => "CATCH",
            Kind::CLASS => "CLASS",
            Kind::COMPILATION_UNIT => "COMPILATION_UNIT",
            Kind::CONDITIONAL_EXPRESSION => "CONDITIONAL_EXPRESSION",
            Kind::CONTINUE => "CONTINUE",
            Kind::DO_WHILE_LOOP => "DO_WHILE_LOOP",
            Kind::ENHANCED_FOR_LOOP => "ENHANCED_FOR_LOOP",
            Kind::EXPRESSION_STATEMENT => "EXPRESSION_STATEMENT",
            Kind::MEMBER_SELECT => "MEMBER_SELECT",
            Kind::MEMBER_REFERENCE => "MEMBER_REFERENCE",
            Kind::FOR_LOOP => "FOR_LOOP",
            Kind::IDENTIFIER => "IDENTIFIER",
            Kind::IF => "IF",
            Kind::IMPORT => "IMPORT",
            Kind::INSTANCE_OF => "INSTANCE_OF",
            Kind::LABELED_STATEMENT => "LABELED_STATEMENT",
            Kind::METHOD => "METHOD",
            Kind::METHOD_INVOCATION => "METHOD_INVOCATION",
            Kind::MODIFIERS => "MODIFIERS",
            Kind::NEW_ARRAY => "NEW_ARRAY",
            Kind::NEW_CLASS => "NEW_CLASS",
            Kind::LAMBDA_EXPRESSION => "LAMBDA_EXPRESSION",
            Kind::PACKAGE => "PACKAGE",
            Kind::PARENTHESIZED => "PARENTHESIZED",
            Kind::BINDING_PATTERN => "BINDING_PATTERN",
            Kind::GUARDED_PATTERN => "GUARDED_PATTERN",
            Kind::PARENTHESIZED_PATTERN => "PARENTHESIZED_PATTERN",
            Kind::DEFAULT_CASE_LABEL => "DEFAULT_CASE_LABEL",
            Kind::PRIMITIVE_TYPE => "PRIMITIVE_TYPE",
            Kind::RETURN => "RETURN",
            Kind::EMPTY_STATEMENT => "EMPTY_STATEMENT",
            Kind::SWITCH => "SWITCH",
            Kind::SWITCH_EXPRESSION => "SWITCH_EXPRESSION",
            Kind::SYNCHRONIZED => "SYNCHRONIZED",
            Kind::THROW => "THROW",
            Kind::TRY => "TRY",
            Kind::PARAMETERIZED_TYPE => "PARAMETERIZED_TYPE",
            Kind::UNION_TYPE => "UNION_TYPE",
            Kind::INTERSECTION_TYPE => "INTERSECTION_TYPE",
            Kind::TYPE_CAST => "TYPE_CAST",
            Kind::TYPE_PARAMETER => "TYPE_PARAMETER",
            Kind::VARIABLE => "VARIABLE",
            Kind::WHILE_LOOP => "WHILE_LOOP",
            Kind::POSTFIX_INCREMENT => "POSTFIX_INCREMENT",
            Kind::POSTFIX_DECREMENT => "POSTFIX_DECREMENT",
            Kind::PREFIX_INCREMENT => "PREFIX_INCREMENT",
            Kind::PREFIX_DECREMENT => "PREFIX_DECREMENT",
            Kind::UNARY_PLUS => "UNARY_PLUS",
            Kind::UNARY_MINUS => "UNARY_MINUS",
            Kind::BITWISE_COMPLEMENT => "BITWISE_COMPLEMENT",
            Kind::LOGICAL_COMPLEMENT => "LOGICAL_COMPLEMENT",
            Kind::MULTIPLY => "MULTIPLY",
            Kind::DIVIDE => "DIVIDE",
            Kind::REMAINDER => "REMAINDER",
            Kind::PLUS => "PLUS",
            Kind::MINUS => "MINUS",
            Kind::LEFT_SHIFT => "LEFT_SHIFT",
            Kind::RIGHT_SHIFT => "RIGHT_SHIFT",
            Kind::UNSIGNED_RIGHT_SHIFT => "UNSIGNED_RIGHT_SHIFT",
            Kind::LESS_THAN => "LESS_THAN",
            Kind::GREATER_THAN => "GREATER_THAN",
            Kind::LESS_THAN_EQUAL => "LESS_THAN_EQUAL",
            Kind::GREATER_THAN_EQUAL => "GREATER_THAN_EQUAL",
            Kind::EQUAL_TO => "EQUAL_TO",
            Kind::NOT_EQUAL_TO => "NOT_EQUAL_TO",
            Kind::AND => "AND",
            Kind::XOR => "XOR",
            Kind::OR => "OR",
            Kind::CONDITIONAL_AND => "CONDITIONAL_AND",
            Kind::CONDITIONAL_OR => "CONDITIONAL_OR",
            Kind::MULTIPLY_ASSIGNMENT => "MULTIPLY_ASSIGNMENT",
            Kind::DIVIDE_ASSIGNMENT => "DIVIDE_ASSIGNMENT",
            Kind::REMAINDER_ASSIGNMENT => "REMAINDER_ASSIGNMENT",
            Kind::PLUS_ASSIGNMENT => "PLUS_ASSIGNMENT",
            Kind::MINUS_ASSIGNMENT => "MINUS_ASSIGNMENT",
            Kind::LEFT_SHIFT_ASSIGNMENT => "LEFT_SHIFT_ASSIGNMENT",
            Kind::RIGHT_SHIFT_ASSIGNMENT => "RIGHT_SHIFT_ASSIGNMENT",
            Kind::UNSIGNED_RIGHT_SHIFT_ASSIGNMENT => "UNSIGNED_RIGHT_SHIFT_ASSIGNMENT",
            Kind::AND_ASSIGNMENT => "AND_ASSIGNMENT",
            Kind::XOR_ASSIGNMENT => "XOR_ASSIGNMENT",
            Kind::OR_ASSIGNMENT => "OR_ASSIGNMENT",
            Kind::INT_LITERAL => "INT_LITERAL",
            Kind::LONG_LITERAL => "LONG_LITERAL",
            Kind::FLOAT_LITERAL => "FLOAT_LITERAL",
            Kind::DOUBLE_LITERAL => "DOUBLE_LITERAL",
            Kind::BOOLEAN_LITERAL => "BOOLEAN_LITERAL",
            Kind::CHAR_LITERAL => "CHAR_LITERAL",
            Kind::STRING_LITERAL => "STRING_LITERAL",
            Kind::NULL_LITERAL => "NULL_LITERAL",
            Kind::UNBOUNDED_WILDCARD => "UNBOUNDED_WILDCARD",
            Kind::EXTENDS_WILDCARD => "EXTENDS_WILDCARD",
            Kind::SUPER_WILDCARD => "SUPER_WILDCARD",
            Kind::ERRONEOUS => "ERRONEOUS",
            Kind::INTERFACE => "INTERFACE",
            Kind::ENUM => "ENUM",
            Kind::ANNOTATION_TYPE => "ANNOTATION_TYPE",
            Kind::MODULE => "MODULE",
            Kind::EXPORTS => "EXPORTS",
            Kind::OPENS => "OPENS",
            Kind::PROVIDES => "PROVIDES",
            Kind::RECORD => "RECORD",
            Kind::REQUIRES => "REQUIRES",
            Kind::USES => "USES",
            Kind::OTHER => "OTHER",
            Kind::YIELD => "YIELD",
        })
    }

    /// Java: enum#values().
    pub fn values() -> [Kind; 115] {
        [
            Kind::ANNOTATED_TYPE,
            Kind::ANNOTATION,
            Kind::TYPE_ANNOTATION,
            Kind::ARRAY_ACCESS,
            Kind::ARRAY_TYPE,
            Kind::ASSERT,
            Kind::ASSIGNMENT,
            Kind::BLOCK,
            Kind::BREAK,
            Kind::CASE,
            Kind::CATCH,
            Kind::CLASS,
            Kind::COMPILATION_UNIT,
            Kind::CONDITIONAL_EXPRESSION,
            Kind::CONTINUE,
            Kind::DO_WHILE_LOOP,
            Kind::ENHANCED_FOR_LOOP,
            Kind::EXPRESSION_STATEMENT,
            Kind::MEMBER_SELECT,
            Kind::MEMBER_REFERENCE,
            Kind::FOR_LOOP,
            Kind::IDENTIFIER,
            Kind::IF,
            Kind::IMPORT,
            Kind::INSTANCE_OF,
            Kind::LABELED_STATEMENT,
            Kind::METHOD,
            Kind::METHOD_INVOCATION,
            Kind::MODIFIERS,
            Kind::NEW_ARRAY,
            Kind::NEW_CLASS,
            Kind::LAMBDA_EXPRESSION,
            Kind::PACKAGE,
            Kind::PARENTHESIZED,
            Kind::BINDING_PATTERN,
            Kind::GUARDED_PATTERN,
            Kind::PARENTHESIZED_PATTERN,
            Kind::DEFAULT_CASE_LABEL,
            Kind::PRIMITIVE_TYPE,
            Kind::RETURN,
            Kind::EMPTY_STATEMENT,
            Kind::SWITCH,
            Kind::SWITCH_EXPRESSION,
            Kind::SYNCHRONIZED,
            Kind::THROW,
            Kind::TRY,
            Kind::PARAMETERIZED_TYPE,
            Kind::UNION_TYPE,
            Kind::INTERSECTION_TYPE,
            Kind::TYPE_CAST,
            Kind::TYPE_PARAMETER,
            Kind::VARIABLE,
            Kind::WHILE_LOOP,
            Kind::POSTFIX_INCREMENT,
            Kind::POSTFIX_DECREMENT,
            Kind::PREFIX_INCREMENT,
            Kind::PREFIX_DECREMENT,
            Kind::UNARY_PLUS,
            Kind::UNARY_MINUS,
            Kind::BITWISE_COMPLEMENT,
            Kind::LOGICAL_COMPLEMENT,
            Kind::MULTIPLY,
            Kind::DIVIDE,
            Kind::REMAINDER,
            Kind::PLUS,
            Kind::MINUS,
            Kind::LEFT_SHIFT,
            Kind::RIGHT_SHIFT,
            Kind::UNSIGNED_RIGHT_SHIFT,
            Kind::LESS_THAN,
            Kind::GREATER_THAN,
            Kind::LESS_THAN_EQUAL,
            Kind::GREATER_THAN_EQUAL,
            Kind::EQUAL_TO,
            Kind::NOT_EQUAL_TO,
            Kind::AND,
            Kind::XOR,
            Kind::OR,
            Kind::CONDITIONAL_AND,
            Kind::CONDITIONAL_OR,
            Kind::MULTIPLY_ASSIGNMENT,
            Kind::DIVIDE_ASSIGNMENT,
            Kind::REMAINDER_ASSIGNMENT,
            Kind::PLUS_ASSIGNMENT,
            Kind::MINUS_ASSIGNMENT,
            Kind::LEFT_SHIFT_ASSIGNMENT,
            Kind::RIGHT_SHIFT_ASSIGNMENT,
            Kind::UNSIGNED_RIGHT_SHIFT_ASSIGNMENT,
            Kind::AND_ASSIGNMENT,
            Kind::XOR_ASSIGNMENT,
            Kind::OR_ASSIGNMENT,
            Kind::INT_LITERAL,
            Kind::LONG_LITERAL,
            Kind::FLOAT_LITERAL,
            Kind::DOUBLE_LITERAL,
            Kind::BOOLEAN_LITERAL,
            Kind::CHAR_LITERAL,
            Kind::STRING_LITERAL,
            Kind::NULL_LITERAL,
            Kind::UNBOUNDED_WILDCARD,
            Kind::EXTENDS_WILDCARD,
            Kind::SUPER_WILDCARD,
            Kind::ERRONEOUS,
            Kind::INTERFACE,
            Kind::ENUM,
            Kind::ANNOTATION_TYPE,
            Kind::MODULE,
            Kind::EXPORTS,
            Kind::OPENS,
            Kind::PROVIDES,
            Kind::RECORD,
            Kind::REQUIRES,
            Kind::USES,
            Kind::OTHER,
            Kind::YIELD,
        ]
    }

    /// Returns the associated interface type that uses this kind.
    /// @return the associated interface
    ///
    /// Java: `Class<? extends Tree> asInterface()` returns null for
    /// `Kind.OTHER`; the Rust port returns the interface name (no reflection,
    /// registered) and `None` for OTHER, matching the Java null.
    pub fn as_interface(&self) -> Option<&'static str> {
        match self {
            Kind::ANNOTATED_TYPE => Some("AnnotatedTypeTree"),
            Kind::ANNOTATION => Some("AnnotationTree"),
            Kind::TYPE_ANNOTATION => Some("AnnotationTree"),
            Kind::ARRAY_ACCESS => Some("ArrayAccessTree"),
            Kind::ARRAY_TYPE => Some("ArrayTypeTree"),
            Kind::ASSERT => Some("AssertTree"),
            Kind::ASSIGNMENT => Some("AssignmentTree"),
            Kind::BLOCK => Some("BlockTree"),
            Kind::BREAK => Some("BreakTree"),
            Kind::CASE => Some("CaseTree"),
            Kind::CATCH => Some("CatchTree"),
            Kind::CLASS => Some("ClassTree"),
            Kind::COMPILATION_UNIT => Some("CompilationUnitTree"),
            Kind::CONDITIONAL_EXPRESSION => Some("ConditionalExpressionTree"),
            Kind::CONTINUE => Some("ContinueTree"),
            Kind::DO_WHILE_LOOP => Some("DoWhileLoopTree"),
            Kind::ENHANCED_FOR_LOOP => Some("EnhancedForLoopTree"),
            Kind::EXPRESSION_STATEMENT => Some("ExpressionStatementTree"),
            Kind::MEMBER_SELECT => Some("MemberSelectTree"),
            Kind::MEMBER_REFERENCE => Some("MemberReferenceTree"),
            Kind::FOR_LOOP => Some("ForLoopTree"),
            Kind::IDENTIFIER => Some("IdentifierTree"),
            Kind::IF => Some("IfTree"),
            Kind::IMPORT => Some("ImportTree"),
            Kind::INSTANCE_OF => Some("InstanceOfTree"),
            Kind::LABELED_STATEMENT => Some("LabeledStatementTree"),
            Kind::METHOD => Some("MethodTree"),
            Kind::METHOD_INVOCATION => Some("MethodInvocationTree"),
            Kind::MODIFIERS => Some("ModifiersTree"),
            Kind::NEW_ARRAY => Some("NewArrayTree"),
            Kind::NEW_CLASS => Some("NewClassTree"),
            Kind::LAMBDA_EXPRESSION => Some("LambdaExpressionTree"),
            Kind::PACKAGE => Some("PackageTree"),
            Kind::PARENTHESIZED => Some("ParenthesizedTree"),
            Kind::BINDING_PATTERN => Some("BindingPatternTree"),
            Kind::GUARDED_PATTERN => Some("GuardedPatternTree"),
            Kind::PARENTHESIZED_PATTERN => Some("ParenthesizedPatternTree"),
            Kind::DEFAULT_CASE_LABEL => Some("DefaultCaseLabelTree"),
            Kind::PRIMITIVE_TYPE => Some("PrimitiveTypeTree"),
            Kind::RETURN => Some("ReturnTree"),
            Kind::EMPTY_STATEMENT => Some("EmptyStatementTree"),
            Kind::SWITCH => Some("SwitchTree"),
            Kind::SWITCH_EXPRESSION => Some("SwitchExpressionTree"),
            Kind::SYNCHRONIZED => Some("SynchronizedTree"),
            Kind::THROW => Some("ThrowTree"),
            Kind::TRY => Some("TryTree"),
            Kind::PARAMETERIZED_TYPE => Some("ParameterizedTypeTree"),
            Kind::UNION_TYPE => Some("UnionTypeTree"),
            Kind::INTERSECTION_TYPE => Some("IntersectionTypeTree"),
            Kind::TYPE_CAST => Some("TypeCastTree"),
            Kind::TYPE_PARAMETER => Some("TypeParameterTree"),
            Kind::VARIABLE => Some("VariableTree"),
            Kind::WHILE_LOOP => Some("WhileLoopTree"),
            Kind::POSTFIX_INCREMENT => Some("UnaryTree"),
            Kind::POSTFIX_DECREMENT => Some("UnaryTree"),
            Kind::PREFIX_INCREMENT => Some("UnaryTree"),
            Kind::PREFIX_DECREMENT => Some("UnaryTree"),
            Kind::UNARY_PLUS => Some("UnaryTree"),
            Kind::UNARY_MINUS => Some("UnaryTree"),
            Kind::BITWISE_COMPLEMENT => Some("UnaryTree"),
            Kind::LOGICAL_COMPLEMENT => Some("UnaryTree"),
            Kind::MULTIPLY => Some("BinaryTree"),
            Kind::DIVIDE => Some("BinaryTree"),
            Kind::REMAINDER => Some("BinaryTree"),
            Kind::PLUS => Some("BinaryTree"),
            Kind::MINUS => Some("BinaryTree"),
            Kind::LEFT_SHIFT => Some("BinaryTree"),
            Kind::RIGHT_SHIFT => Some("BinaryTree"),
            Kind::UNSIGNED_RIGHT_SHIFT => Some("BinaryTree"),
            Kind::LESS_THAN => Some("BinaryTree"),
            Kind::GREATER_THAN => Some("BinaryTree"),
            Kind::LESS_THAN_EQUAL => Some("BinaryTree"),
            Kind::GREATER_THAN_EQUAL => Some("BinaryTree"),
            Kind::EQUAL_TO => Some("BinaryTree"),
            Kind::NOT_EQUAL_TO => Some("BinaryTree"),
            Kind::AND => Some("BinaryTree"),
            Kind::XOR => Some("BinaryTree"),
            Kind::OR => Some("BinaryTree"),
            Kind::CONDITIONAL_AND => Some("BinaryTree"),
            Kind::CONDITIONAL_OR => Some("BinaryTree"),
            Kind::MULTIPLY_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::DIVIDE_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::REMAINDER_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::PLUS_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::MINUS_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::LEFT_SHIFT_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::RIGHT_SHIFT_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::UNSIGNED_RIGHT_SHIFT_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::AND_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::XOR_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::OR_ASSIGNMENT => Some("CompoundAssignmentTree"),
            Kind::INT_LITERAL => Some("LiteralTree"),
            Kind::LONG_LITERAL => Some("LiteralTree"),
            Kind::FLOAT_LITERAL => Some("LiteralTree"),
            Kind::DOUBLE_LITERAL => Some("LiteralTree"),
            Kind::BOOLEAN_LITERAL => Some("LiteralTree"),
            Kind::CHAR_LITERAL => Some("LiteralTree"),
            Kind::STRING_LITERAL => Some("LiteralTree"),
            Kind::NULL_LITERAL => Some("LiteralTree"),
            Kind::UNBOUNDED_WILDCARD => Some("WildcardTree"),
            Kind::EXTENDS_WILDCARD => Some("WildcardTree"),
            Kind::SUPER_WILDCARD => Some("WildcardTree"),
            Kind::ERRONEOUS => Some("ErroneousTree"),
            Kind::INTERFACE => Some("ClassTree"),
            Kind::ENUM => Some("ClassTree"),
            Kind::ANNOTATION_TYPE => Some("ClassTree"),
            Kind::MODULE => Some("ModuleTree"),
            Kind::EXPORTS => Some("ExportsTree"),
            Kind::OPENS => Some("OpensTree"),
            Kind::PROVIDES => Some("ProvidesTree"),
            Kind::RECORD => Some("ClassTree"),
            Kind::REQUIRES => Some("RequiresTree"),
            Kind::USES => Some("UsesTree"),
            Kind::OTHER => None,
            Kind::YIELD => Some("YieldTree"),
        }
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name().to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, Tree};
    use crate::tree::tree_visitor::TreeVisitor;

    struct Leaf {
        kind: Kind,
    }

    impl Tree for Leaf {
        fn get_kind(&self) -> Kind {
            self.kind
        }
        fn accept<R, D>(&self, _visitor: &mut dyn TreeVisitor<R, D>, _data: D) -> R {
            panic!("UnsupportedOperationException: accept: test fixture accept")
        }
    }

    #[test]
    fn kind_enum_is_complete_and_ordered() {
        // Tree.Kind must be complete (111 constants) and in the Java order
        // (code/TypeTag.getKindLiteral depends on it).
        let values = Kind::values();
        assert_eq!(values.len(), 115);
        assert_eq!(values[0], Kind::ANNOTATED_TYPE);
        assert_eq!(values[1], Kind::ANNOTATION);
        assert_eq!(values[2], Kind::TYPE_ANNOTATION);
        assert_eq!(values[values.len() - 1], Kind::YIELD);
        // spot-check ordering further down the list
        assert_eq!(values[3], Kind::ARRAY_ACCESS);
        assert_eq!(values[4], Kind::ARRAY_TYPE);
        assert_eq!(Kind::CLASS.name().to_string_lossy(), "CLASS");
        assert_eq!(Kind::CLASS.to_string(), "CLASS");
    }

    #[test]
    fn kind_associated_interfaces() {
        // Java: Kind.asInterface() returns the associated Tree interface.
        assert_eq!(Kind::CLASS.as_interface(), Some("ClassTree"));
        assert_eq!(Kind::IDENTIFIER.as_interface(), Some("IdentifierTree"));
        assert_eq!(Kind::METHOD.as_interface(), Some("MethodTree"));
        assert_eq!(Kind::OTHER.as_interface(), None);
        assert_eq!(Kind::YIELD.as_interface(), Some("YieldTree"));
    }

    #[test]
    fn tree_is_object_safe_with_conversion_accessors() {
        let leaf = Leaf { kind: Kind::CLASS };
        let tree: &dyn Tree = &leaf;
        assert_eq!(tree.get_kind(), Kind::CLASS);
        // the object-conversion accessors default to None; concrete impls override.
        assert!(tree.as_class_tree().is_none());
        assert!(tree.as_identifier_tree().is_none());
        let _ = tree.as_annotation_tree();
    }
}

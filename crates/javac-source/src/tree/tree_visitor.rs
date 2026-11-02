//! A visitor of trees, in the style of the visitor design pattern.
//!
//! (docs/PORTING_RULES.md §14): visitor interface methods are
//! abstract in Java; in Rust they carry a panicking default so that the util
//! class layers (TreeScanner, SimpleTreeVisitor, ...) can supply the real
//! defaults through subtraits.  Methods take `&mut self` and the visited node as
//! `&dyn XxxTree`.

/// A visitor of trees, in the style of the visitor design pattern.
/// Classes implementing this interface are used to operate
/// on a tree when the kind of tree is unknown at compile time.
/// When a visitor is passed to an tree's {@link Tree#accept
/// accept} method, the <code>visit<i>Xyz</i></code> method most applicable
/// to that tree is invoked.
///
/// <p> Classes implementing this interface may or may not throw a
/// {@code NullPointerException} if the additional parameter {@code p}
/// is {@code null}; see documentation of the implementing class for
/// details.
///
/// <p> <b>WARNING:</b> It is possible that methods will be added to
/// this interface to accommodate new, currently unknown, language
/// structures added to future versions of the Java programming
/// language.  Therefore, visitor classes directly implementing this
/// interface may be source incompatible with future versions of the
/// platform.
///
/// @param <R> the return type of this visitor's methods.  Use {@link
///            Void} for visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's
///            methods.  Use {@code Void} for visitors that do not need an
///            additional parameter.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
///
/// @since 1.6
use super::tree::Tree;
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
use super::case_tree::CaseTree;
use super::catch_tree::CatchTree;
use super::class_tree::ClassTree;
use super::compilation_unit_tree::CompilationUnitTree;
use super::compound_assignment_tree::CompoundAssignmentTree;
use super::conditional_expression_tree::ConditionalExpressionTree;
use super::continue_tree::ContinueTree;
use super::default_case_label_tree::DefaultCaseLabelTree;
use super::do_while_loop_tree::DoWhileLoopTree;
use super::empty_statement_tree::EmptyStatementTree;
use super::enhanced_for_loop_tree::EnhancedForLoopTree;
use super::erroneous_tree::ErroneousTree;
use super::exports_tree::ExportsTree;
use super::expression_statement_tree::ExpressionStatementTree;
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
use super::primitive_type_tree::PrimitiveTypeTree;
use super::provides_tree::ProvidesTree;
use super::requires_tree::RequiresTree;
use super::return_tree::ReturnTree;
use super::switch_expression_tree::SwitchExpressionTree;
use super::switch_tree::SwitchTree;
use super::synchronized_tree::SynchronizedTree;
use super::throw_tree::ThrowTree;
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

pub trait TreeVisitor<R, P> {
    /// Visits an AnnotatedTypeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_annotated_type(&mut self, _node: &dyn AnnotatedTypeTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_annotated_type: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an AnnotatedTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_annotation(&mut self, _node: &dyn AnnotationTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_annotation: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a MethodInvocationTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_method_invocation(&mut self, _node: &dyn MethodInvocationTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_method_invocation: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an AssertTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_assert(&mut self, _node: &dyn AssertTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_assert: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an AssignmentTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_assignment(&mut self, _node: &dyn AssignmentTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_assignment: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a CompoundAssignmentTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_compound_assignment(&mut self, _node: &dyn CompoundAssignmentTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_compound_assignment: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a BinaryTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_binary(&mut self, _node: &dyn BinaryTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_binary: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a BlockTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_block(&mut self, _node: &dyn BlockTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_block: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a BreakTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_break(&mut self, _node: &dyn BreakTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_break: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a CaseTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_case(&mut self, _node: &dyn CaseTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_case: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a CatchTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_catch(&mut self, _node: &dyn CatchTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_catch: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ClassTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_class(&mut self, _node: &dyn ClassTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_class: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ConditionalExpressionTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_conditional_expression(&mut self, _node: &dyn ConditionalExpressionTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_conditional_expression: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ContinueTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_continue(&mut self, _node: &dyn ContinueTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_continue: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a DoWhileTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_do_while_loop(&mut self, _node: &dyn DoWhileLoopTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_do_while_loop: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an ErroneousTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_erroneous(&mut self, _node: &dyn ErroneousTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_erroneous: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an ExpressionStatementTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_expression_statement(&mut self, _node: &dyn ExpressionStatementTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_expression_statement: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an EnhancedForLoopTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_enhanced_for_loop(&mut self, _node: &dyn EnhancedForLoopTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_enhanced_for_loop: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ForLoopTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_for_loop(&mut self, _node: &dyn ForLoopTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_for_loop: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an IdentifierTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_identifier(&mut self, _node: &dyn IdentifierTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_identifier: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an IfTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_if(&mut self, _node: &dyn IfTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_if: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an ImportTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_import(&mut self, _node: &dyn ImportTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_import: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an ArrayAccessTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_array_access(&mut self, _node: &dyn ArrayAccessTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_array_access: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a LabeledStatementTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_labeled_statement(&mut self, _node: &dyn LabeledStatementTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_labeled_statement: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a LiteralTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_literal(&mut self, _node: &dyn LiteralTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_literal: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an BindingPattern node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 16
    fn visit_binding_pattern(&mut self, _node: &dyn BindingPatternTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_binding_pattern: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a DefaultCaseLabelTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 17
    fn visit_default_case_label(&mut self, _node: &dyn DefaultCaseLabelTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_default_case_label: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a MethodTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_method(&mut self, _node: &dyn MethodTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_method: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ModifiersTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_modifiers(&mut self, _node: &dyn ModifiersTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_modifiers: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a NewArrayTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_new_array(&mut self, _node: &dyn NewArrayTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_new_array: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a GuardPatternTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 17
    fn visit_guarded_pattern(&mut self, _node: &dyn GuardedPatternTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_guarded_pattern: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ParenthesizedPatternTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 17
    fn visit_parenthesized_pattern(&mut self, _node: &dyn ParenthesizedPatternTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_parenthesized_pattern: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a NewClassTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_new_class(&mut self, _node: &dyn NewClassTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_new_class: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a LambdaExpressionTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_lambda_expression(&mut self, _node: &dyn LambdaExpressionTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_lambda_expression: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a PackageTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_package(&mut self, _node: &dyn PackageTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_package: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ParenthesizedTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_parenthesized(&mut self, _node: &dyn ParenthesizedTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_parenthesized: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ReturnTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_return(&mut self, _node: &dyn ReturnTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_return: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a MemberSelectTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_member_select(&mut self, _node: &dyn MemberSelectTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_member_select: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a MemberReferenceTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_member_reference(&mut self, _node: &dyn MemberReferenceTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_member_reference: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an EmptyStatementTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_empty_statement(&mut self, _node: &dyn EmptyStatementTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_empty_statement: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a SwitchTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_switch(&mut self, _node: &dyn SwitchTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_switch: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a SwitchExpressionTree node.
    ///
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 12
    fn visit_switch_expression(&mut self, _node: &dyn SwitchExpressionTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_switch_expression: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a SynchronizedTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_synchronized(&mut self, _node: &dyn SynchronizedTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_synchronized: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ThrowTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_throw(&mut self, _node: &dyn ThrowTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_throw: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a CompilationUnitTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_compilation_unit(&mut self, _node: &dyn CompilationUnitTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_compilation_unit: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a TryTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_try(&mut self, _node: &dyn TryTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_try: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ParameterizedTypeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_parameterized_type(&mut self, _node: &dyn ParameterizedTypeTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_parameterized_type: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a UnionTypeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_union_type(&mut self, _node: &dyn UnionTypeTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_union_type: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an IntersectionTypeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_intersection_type(&mut self, _node: &dyn IntersectionTypeTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_intersection_type: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an ArrayTypeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_array_type(&mut self, _node: &dyn ArrayTypeTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_array_type: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a TypeCastTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_type_cast(&mut self, _node: &dyn TypeCastTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_type_cast: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a PrimitiveTypeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_primitive_type(&mut self, _node: &dyn PrimitiveTypeTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_primitive_type: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a TypeParameterTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_type_parameter(&mut self, _node: &dyn TypeParameterTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_type_parameter: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an InstanceOfTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_instance_of(&mut self, _node: &dyn InstanceOfTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_instance_of: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a UnaryTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_unary(&mut self, _node: &dyn UnaryTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_unary: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a VariableTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_variable(&mut self, _node: &dyn VariableTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_variable: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a WhileLoopTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_while_loop(&mut self, _node: &dyn WhileLoopTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_while_loop: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a WildcardTypeTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_wildcard(&mut self, _node: &dyn WildcardTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_wildcard: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ModuleTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_module(&mut self, _node: &dyn ModuleTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_module: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an ExportsTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_exports(&mut self, _node: &dyn ExportsTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_exports: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an OpensTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_opens(&mut self, _node: &dyn OpensTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_opens: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a ProvidesTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_provides(&mut self, _node: &dyn ProvidesTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_provides: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a RequiresTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_requires(&mut self, _node: &dyn RequiresTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_requires: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a UsesTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_uses(&mut self, _node: &dyn UsesTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_uses: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits an unknown type of Tree node.
    /// This can occur if the language evolves and new kinds
    /// of nodes are added to the {@code Tree} hierarchy.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    fn visit_other(&mut self, _node: &dyn Tree, _p: P) -> R {
        panic!("AbstractMethodError: visit_other: PORTING_RULES: visitor interface method not overridden")
    }

    /// Visits a YieldTree node.
    /// @param node the node being visited
    /// @param p a parameter value
    /// @return a result value
    /// @since 13
    fn visit_yield(&mut self, _node: &dyn YieldTree, _p: P) -> R {
        panic!("AbstractMethodError: visit_yield: PORTING_RULES: visitor interface method not overridden")
    }

}

#[cfg(test)]
mod tests {
    use super::TreeVisitor;
    use crate::tree::identifier_tree::IdentifierTree;
    use javac_tools::lang::model::element::Name;
    use crate::tree::tree::{Kind, Tree};
    use javac_strings::JavaString;

    struct TestName;

    impl Name for TestName {
        fn length(&self) -> usize {
            1
        }
        fn char_at(&self, _index: usize) -> u16 {
            'x' as u16
        }
        fn sub_sequence(&self, _start: usize, _end: usize) -> JavaString {
            JavaString::from("x")
        }
        fn to_java_string(&self) -> JavaString {
            JavaString::from("x")
        }
        fn content_equals(&self, cs: &JavaString) -> bool {
            *cs == JavaString::from("x")
        }
    }

    struct Ident;

    impl Tree for Ident {
        fn get_kind(&self) -> Kind {
            Kind::IDENTIFIER
        }
        fn accept<R, D>(&self, _visitor: &mut dyn TreeVisitor<R, D>, _data: D) -> R {
            panic!("UnsupportedOperationException: accept: test fixture accept")
        }
    }

    impl crate::tree::case_label_tree::CaseLabelTree for Ident {}

    impl crate::tree::expression_tree::ExpressionTree for Ident {}

    impl IdentifierTree for Ident {
        fn get_name(&self) -> std::rc::Rc<dyn Name> {
            std::rc::Rc::new(TestName)
        }
    }

    struct CountingVisitor {
        hits: usize,
    }

    impl TreeVisitor<usize, ()> for CountingVisitor {
        fn visit_identifier(&mut self, _node: &dyn IdentifierTree, _p: ()) -> usize {
            self.hits += 1;
            self.hits
        }
    }

    #[test]
    fn visitor_is_object_safe_and_dispatches() {
        let ident = Ident;
        let mut visitor = CountingVisitor { hits: 0 };
        let dyn_visitor: &mut dyn TreeVisitor<usize, ()> = &mut visitor;
        let result = dyn_visitor.visit_identifier(&ident, ());
        assert_eq!(result, 1);
        assert_eq!(visitor.hits, 1);
    }

    #[test]
    #[should_panic(expected = "not overridden")]
    fn unimplemented_visit_method_panics() {
        // Java: abstract interface method -> Rust panicking default (§14).
        struct Empty;
        impl TreeVisitor<(), ()> for Empty {}
        let mut visitor = Empty;
        let ident = Ident;
        visitor.visit_other(&ident, ());
    }
}

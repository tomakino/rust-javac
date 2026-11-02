//! A simple visitor for tree nodes.
//!
//! PORTING_RULES (§14.5):
//! * Java class SimpleTreeVisitor<R,P> implements TreeVisitor<R,P> -> Rust trait
//!   SimpleTreeVisitor<R: Clone, P: Clone>: TreeVisitor<R, P>; the Java nullable
//!   DEFAULT_VALUE field -> default_value() -> Option<&R> (the no-arg
//!   constructor's null has no Rust form; default_action panics if it would
//!   return null) and the protected defaultAction(Tree, P) ->
//!   default_action(&mut self, tree: &dyn Tree, p: P) -> R (approved).
//! * the 66 visit methods are mechanically generated from the Java source; each
//!   calls defaultAction exactly like upstream.
//! * the final convenience methods visit(Tree, P) / visit(Iterable, P) (they
//!   dispatch and may return null) -> visit(Option<&dyn Tree>, P) -> Option<R>
//!   and visit_all(&[Rc<dyn Tree>], P) -> Option<R>; the Java single-argument
//!   convenience overloads are not ported (as registered).
//! * the double dispatch goes through crate::util::dispatch::
//!   dispatch_simple_tree_visitor (Tree.accept is generic and not callable on
//!   &dyn Tree); implementations also write the empty impl TreeVisitor<R, P>
//!   for X {} because the interface methods only keep their §14 panicking
//!   defaults.
//!
//! @author Peter von der Ah&eacute;
//! @since 1.6

use crate::tree::tree::Tree;
use crate::tree::tree_visitor::TreeVisitor;
use crate::util::dispatch::dispatch_simple_tree_visitor;
use std::rc::Rc;

use crate::tree::annotated_type_tree::AnnotatedTypeTree;
use crate::tree::annotation_tree::AnnotationTree;
use crate::tree::array_access_tree::ArrayAccessTree;
use crate::tree::array_type_tree::ArrayTypeTree;
use crate::tree::assert_tree::AssertTree;
use crate::tree::assignment_tree::AssignmentTree;
use crate::tree::binary_tree::BinaryTree;
use crate::tree::binding_pattern_tree::BindingPatternTree;
use crate::tree::block_tree::BlockTree;
use crate::tree::break_tree::BreakTree;
use crate::tree::case_tree::CaseTree;
use crate::tree::catch_tree::CatchTree;
use crate::tree::class_tree::ClassTree;
use crate::tree::compilation_unit_tree::CompilationUnitTree;
use crate::tree::compound_assignment_tree::CompoundAssignmentTree;
use crate::tree::conditional_expression_tree::ConditionalExpressionTree;
use crate::tree::continue_tree::ContinueTree;
use crate::tree::default_case_label_tree::DefaultCaseLabelTree;
use crate::tree::do_while_loop_tree::DoWhileLoopTree;
use crate::tree::empty_statement_tree::EmptyStatementTree;
use crate::tree::enhanced_for_loop_tree::EnhancedForLoopTree;
use crate::tree::erroneous_tree::ErroneousTree;
use crate::tree::exports_tree::ExportsTree;
use crate::tree::expression_statement_tree::ExpressionStatementTree;
use crate::tree::for_loop_tree::ForLoopTree;
use crate::tree::guarded_pattern_tree::GuardedPatternTree;
use crate::tree::identifier_tree::IdentifierTree;
use crate::tree::if_tree::IfTree;
use crate::tree::import_tree::ImportTree;
use crate::tree::instance_of_tree::InstanceOfTree;
use crate::tree::intersection_type_tree::IntersectionTypeTree;
use crate::tree::labeled_statement_tree::LabeledStatementTree;
use crate::tree::lambda_expression_tree::LambdaExpressionTree;
use crate::tree::literal_tree::LiteralTree;
use crate::tree::member_reference_tree::MemberReferenceTree;
use crate::tree::member_select_tree::MemberSelectTree;
use crate::tree::method_invocation_tree::MethodInvocationTree;
use crate::tree::method_tree::MethodTree;
use crate::tree::modifiers_tree::ModifiersTree;
use crate::tree::module_tree::ModuleTree;
use crate::tree::new_array_tree::NewArrayTree;
use crate::tree::new_class_tree::NewClassTree;
use crate::tree::opens_tree::OpensTree;
use crate::tree::package_tree::PackageTree;
use crate::tree::parameterized_type_tree::ParameterizedTypeTree;
use crate::tree::parenthesized_pattern_tree::ParenthesizedPatternTree;
use crate::tree::parenthesized_tree::ParenthesizedTree;
use crate::tree::primitive_type_tree::PrimitiveTypeTree;
use crate::tree::provides_tree::ProvidesTree;
use crate::tree::requires_tree::RequiresTree;
use crate::tree::return_tree::ReturnTree;
use crate::tree::switch_expression_tree::SwitchExpressionTree;
use crate::tree::switch_tree::SwitchTree;
use crate::tree::synchronized_tree::SynchronizedTree;
use crate::tree::throw_tree::ThrowTree;
use crate::tree::try_tree::TryTree;
use crate::tree::type_cast_tree::TypeCastTree;
use crate::tree::type_parameter_tree::TypeParameterTree;
use crate::tree::unary_tree::UnaryTree;
use crate::tree::union_type_tree::UnionTypeTree;
use crate::tree::uses_tree::UsesTree;
use crate::tree::variable_tree::VariableTree;
use crate::tree::while_loop_tree::WhileLoopTree;
use crate::tree::wildcard_tree::WildcardTree;
use crate::tree::yield_tree::YieldTree;

/// A simple visitor for tree nodes.
///
/// @param <R> the return type of this visitor's methods.  Use {@link Void} for
///            visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's methods.
///            Use {@code Void} for visitors that do not need an additional parameter.
///
/// @author Peter von der Ah&eacute;
/// @since 1.6
pub trait SimpleTreeVisitor<R: Clone, P: Clone>: TreeVisitor<R, P> {
    /// The default value, returned by the default action.
    ///
    /// Java: protected final R DEFAULT_VALUE (null for the no-arg constructor,
    /// which has no Rust form; the accessor returns None for it).
    fn default_value(&self) -> Option<&R>;

    /// The default action, used by all visit methods that are not overridden.
    /// @param node the node being visited
    /// @param p the parameter value passed to the visit method
    /// @return the result value to be returned from the visit method
    fn default_action(&mut self, _node: &dyn Tree, _p: P) -> R {
        match self.default_value() {
            Some(value) => value.clone(),
            None => panic!(
                "SimpleTreeVisitor.defaultAction: DEFAULT_VALUE is null (Java); \
                 the Rust implementation must return Some(value) from default_value()"
            ),
        }
    }

    /// Invokes the appropriate visit method specific to the type of the node.
    /// @param node the node on which to dispatch (Java null -> None)
    /// @param p a parameter to be passed to the appropriate visit method
    /// @return the value returned from the appropriate visit method
    fn visit(&mut self, node: Option<&dyn Tree>, p: P) -> Option<R> {
        match node {
            Some(node) => Some(dispatch_simple_tree_visitor(self, node, p)),
            None => None,
        }
    }

    /// Invokes the appropriate visit method on each of a sequence of nodes.
    /// @param nodes the nodes on which to dispatch
    /// @param p a parameter value to be passed to each appropriate visit method
    /// @return the value returned from the last of the visit methods, or null
    ///      if none were called
    fn visit_all(&mut self, nodes: &[Rc<dyn Tree>], p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for node in nodes {
            r = self.visit(Some(node.as_ref()), p.clone());
        }
        r
    }

    /// A simple visitor for tree nodes.
    ///
    /// @param <R> the return type of this visitor's methods.  Use {@link
    ///            Void} for visitors that do not need to return results.
    /// @param <P> the type of the additional parameter to this visitor's
    ///            methods.  Use {@code Void} for visitors that do not need an
    ///            additional parameter.
    ///
    /// @author Peter von der Ah&eacute;
    /// @since 1.6
    /// /
    /// public class SimpleTreeVisitor <R,P> implements TreeVisitor<R,P> {
    ///     /**
    /// The default value, returned by the {@link #defaultAction default action}.
    /// /
    ///     protected final R DEFAULT_VALUE;
    ///
    ///     /**
    /// Creates a visitor, with a DEFAULT_VALUE of {@code null}.
    /// /
    ///     protected SimpleTreeVisitor() {
    ///         DEFAULT_VALUE = null;
    ///     }
    ///
    ///     /**
    /// Creates a visitor, with a specified DEFAULT_VALUE.
    /// @param defaultValue the default value to be returned by the default action
    /// /
    ///     protected SimpleTreeVisitor(R defaultValue) {
    ///         DEFAULT_VALUE = defaultValue;
    ///     }
    ///
    ///     /**
    /// The default action, used by all visit methods that are not overridden.
    /// @param node the node being visited
    /// @param p the parameter value passed to the visit method
    /// @return the result value to be returned from the visit method
    /// /
    ///     protected R defaultAction(Tree node, P p) {
    ///         return DEFAULT_VALUE;
    ///     }
    ///
    ///     /**
    /// Invokes the appropriate visit method specific to the type of the node.
    /// @param node the node on which to dispatch
    /// @param p a parameter to be passed to the appropriate visit method
    /// @return the value returns from the appropriate visit method
    /// /
    ///     public final R visit(Tree node, P p) {
    ///         return (node == null) ? null : node.accept(this, p);
    ///     }
    ///
    ///     /**
    /// Invokes the appropriate visit method on each of a sequence of nodes.
    /// @param nodes the nodes on which to dispatch
    /// @param p a parameter value to be passed to each appropriate visit method
    /// @return the value return from the last of the visit methods, or null
    ///      if none were called
    /// /
    ///     public final R visit(Iterable<? extends Tree> nodes, P p) {
    ///         R r = null;
    ///         if (nodes != null)
    ///             for (Tree node : nodes)
    ///                 r = visit(node, p);
    ///         return r;
    ///     }
    ///
    ///     /**
    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_compilation_unit(&mut self, node: &dyn CompilationUnitTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_package(&mut self, node: &dyn PackageTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_import(&mut self, node: &dyn ImportTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_class(&mut self, node: &dyn ClassTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_method(&mut self, node: &dyn MethodTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_variable(&mut self, node: &dyn VariableTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_empty_statement(&mut self, node: &dyn EmptyStatementTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_block(&mut self, node: &dyn BlockTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_do_while_loop(&mut self, node: &dyn DoWhileLoopTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_while_loop(&mut self, node: &dyn WhileLoopTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_for_loop(&mut self, node: &dyn ForLoopTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_enhanced_for_loop(&mut self, node: &dyn EnhancedForLoopTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_labeled_statement(&mut self, node: &dyn LabeledStatementTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_switch(&mut self, node: &dyn SwitchTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_switch_expression(&mut self, node: &dyn SwitchExpressionTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_case(&mut self, node: &dyn CaseTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_synchronized(&mut self, node: &dyn SynchronizedTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_try(&mut self, node: &dyn TryTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_catch(&mut self, node: &dyn CatchTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_conditional_expression(&mut self, node: &dyn ConditionalExpressionTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_if(&mut self, node: &dyn IfTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_expression_statement(&mut self, node: &dyn ExpressionStatementTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_break(&mut self, node: &dyn BreakTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_continue(&mut self, node: &dyn ContinueTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_return(&mut self, node: &dyn ReturnTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_throw(&mut self, node: &dyn ThrowTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_assert(&mut self, node: &dyn AssertTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_method_invocation(&mut self, node: &dyn MethodInvocationTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_new_class(&mut self, node: &dyn NewClassTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_new_array(&mut self, node: &dyn NewArrayTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_lambda_expression(&mut self, node: &dyn LambdaExpressionTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_parenthesized(&mut self, node: &dyn ParenthesizedTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_assignment(&mut self, node: &dyn AssignmentTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_compound_assignment(&mut self, node: &dyn CompoundAssignmentTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_unary(&mut self, node: &dyn UnaryTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_binary(&mut self, node: &dyn BinaryTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_type_cast(&mut self, node: &dyn TypeCastTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_instance_of(&mut self, node: &dyn InstanceOfTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    /// @since 14
    fn visit_binding_pattern(&mut self, node: &dyn BindingPatternTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    /// @since 17
    fn visit_default_case_label(&mut self, node: &dyn DefaultCaseLabelTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_array_access(&mut self, node: &dyn ArrayAccessTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_member_select(&mut self, node: &dyn MemberSelectTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    /// @since 17
    fn visit_parenthesized_pattern(&mut self, node: &dyn ParenthesizedPatternTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    /// @since 17
    fn visit_guarded_pattern(&mut self, node: &dyn GuardedPatternTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_member_reference(&mut self, node: &dyn MemberReferenceTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_identifier(&mut self, node: &dyn IdentifierTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_literal(&mut self, node: &dyn LiteralTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_primitive_type(&mut self, node: &dyn PrimitiveTypeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_array_type(&mut self, node: &dyn ArrayTypeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_parameterized_type(&mut self, node: &dyn ParameterizedTypeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_union_type(&mut self, node: &dyn UnionTypeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_intersection_type(&mut self, node: &dyn IntersectionTypeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_type_parameter(&mut self, node: &dyn TypeParameterTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_wildcard(&mut self, node: &dyn WildcardTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_modifiers(&mut self, node: &dyn ModifiersTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_annotation(&mut self, node: &dyn AnnotationTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_annotated_type(&mut self, node: &dyn AnnotatedTypeTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_module(&mut self, node: &dyn ModuleTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_exports(&mut self, node: &dyn ExportsTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_opens(&mut self, node: &dyn OpensTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_provides(&mut self, node: &dyn ProvidesTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_requires(&mut self, node: &dyn RequiresTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_uses(&mut self, node: &dyn UsesTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_erroneous(&mut self, node: &dyn ErroneousTree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_other(&mut self, node: &dyn Tree, p: P) -> R {
        self.default_action(node, p)
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation calls {@code defaultAction}.
    ///
    /// @param node {@inheritDoc}
    /// @param p {@inheritDoc}
    /// @return  the result of {@code defaultAction}
    fn visit_yield(&mut self, node: &dyn YieldTree, p: P) -> R {
        self.default_action(node, p)
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleTreeVisitor;
    use crate::tree::identifier_tree::IdentifierTree;
    use crate::tree::tree::Tree;
    use crate::tree::tree_visitor::TreeVisitor;
    use crate::util::test_support::{BinaryTreeFixture, IdentTree};
    use std::rc::Rc;

    struct DefaultVisitor {
        value: usize,
    }

    impl TreeVisitor<usize, ()> for DefaultVisitor {}

    impl SimpleTreeVisitor<usize, ()> for DefaultVisitor {
        fn default_value(&self) -> Option<&usize> {
            Some(&self.value)
        }
    }

    struct CustomVisitor;

    impl TreeVisitor<usize, ()> for CustomVisitor {}

    impl SimpleTreeVisitor<usize, ()> for CustomVisitor {
        fn default_value(&self) -> Option<&usize> {
            None
        }
        fn visit_identifier(&mut self, _node: &dyn IdentifierTree, _p: ()) -> usize {
            7
        }
    }

    #[test]
    fn default_action_returns_the_default_value() {
        let mut visitor = DefaultVisitor { value: 42 };
        assert_eq!(
            SimpleTreeVisitor::visit_identifier(&mut visitor, &IdentTree, ()),
            42
        );
        assert_eq!(visitor.visit(Some(&IdentTree), ()), Some(42));
        assert_eq!(visitor.visit(None, ()), None);
    }

    #[test]
    fn dispatch_uses_the_overridden_visit_method() {
        let mut visitor = CustomVisitor;
        assert_eq!(visitor.visit(Some(&IdentTree), ()), Some(7));
    }

    #[test]
    fn visit_all_iterates_and_keeps_the_last_result() {
        let mut visitor = CustomVisitor;
        let nodes: Vec<Rc<dyn Tree>> = vec![Rc::new(IdentTree), Rc::new(IdentTree)];
        assert_eq!(visitor.visit_all(&nodes, ()), Some(7));
    }

    #[test]
    #[should_panic(expected = "DEFAULT_VALUE is null")]
    fn default_action_panics_when_default_value_is_null() {
        let mut visitor = CustomVisitor;
        let left: Rc<dyn crate::tree::expression_tree::ExpressionTree> = Rc::new(IdentTree);
        let right: Rc<dyn crate::tree::expression_tree::ExpressionTree> = Rc::new(IdentTree);
        let binary = BinaryTreeFixture { left, right };
        let _ = SimpleTreeVisitor::visit_binary(&mut visitor, &binary, ());
    }
}

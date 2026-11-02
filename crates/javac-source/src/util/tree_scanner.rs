//! A TreeVisitor that visits all the child tree nodes.
//!
//! (§14):
//! * Java class TreeScanner<R,P> implements TreeVisitor<R,P> -> Rust trait
//!   `TreeScanner<R, P>: TreeVisitor<Option<R>, P>`: Java's nullable R results
//!   (the default visit methods and scan() return null) are represented by
//!   Option<R>.  Implementations write an empty
//!   `impl TreeVisitor<Option<R>, P> for X {}` (the interface methods keep their
//!   §14 panicking defaults; the scanner dispatch calls the TreeScanner methods
//!   instead).
//! * the Java scan(Tree, P) double dispatch goes through
//!   `crate::util::dispatch::dispatch_tree_scanner` (Tree.accept is generic and
//!   not callable on &dyn Tree); a null tree is expressed by
//!   `scan_opt(Option<Rc<dyn Tree>>, P)`.
//! * `P: Clone` is required because the Java default visit methods pass the same
//!   parameter value to several child scans (Rust values are moved; registered
//!   adaptation, same as the javax.lang.model scanners).
//! * Java scan(Iterable<? extends Tree>, P) -> scan_all(&[Rc<dyn Tree>], P).
//! * visitCase / visitInstanceOf / visitNewArray are ported verbatim (the
//!   getDimAnnotations loop is written out explicitly).
//!
//! @author Peter von der Ah&eacute;
//! @author Jonathan Gibbons
//! @since 1.6

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
use crate::tree::identifier_tree::IdentifierTree;
use crate::tree::if_tree::IfTree;
use crate::tree::import_tree::ImportTree;
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
use crate::tree::tree::Tree;
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
use crate::tree::tree_visitor::TreeVisitor;
use crate::util::dispatch::dispatch_tree_scanner;
use std::rc::Rc;

/// A TreeVisitor that visits all the child tree nodes.
///
/// @param <R> the return type of this visitor's methods.  Use {@link Void} for
///            visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's methods.
///            Use {@code Void} for visitors that do not need an additional parameter.
///
/// @author Peter von der Ah&eacute;
/// @author Jonathan Gibbons
/// @since 1.6
pub trait TreeScanner<R, P: Clone>: TreeVisitor<Option<R>, P> {
    /// Scans a single node.
    /// @param tree the node to be scanned
    /// @param p a parameter value passed to the visit method
    /// @return the result value from the visit method
    fn scan(&mut self, tree: &dyn Tree, p: P) -> Option<R> {
        dispatch_tree_scanner(self, tree, p)
    }

    /// Java: scan(Tree tree, P p) for a possibly-null tree.
    fn scan_opt(&mut self, tree: Option<Rc<dyn Tree>>, p: P) -> Option<R> {
        match tree {
            Some(tree) => self.scan(tree.as_ref(), p),
            None => None,
        }
    }

    /// Scans a sequence of nodes.
    /// @param nodes the nodes to be scanned
    /// @param p a parameter value to be passed to the visit method for each node
    /// @return the combined return value from the visit methods.
    ///      The values are combined using the {@link #reduce reduce} method.
    fn scan_all(&mut self, nodes: &[Rc<dyn Tree>], p: P) -> Option<R> {
        let mut r: Option<R> = None;
        let mut first = true;
        for node in nodes {
            r = if first {
                self.scan(node.as_ref(), p.clone())
            } else {
                self.scan_and_reduce(node.as_ref(), p.clone(), r)
            };
            first = false;
        }
        r
    }

    /// Java: private R scanAndReduce(Tree node, P p, R r).
    fn scan_and_reduce(&mut self, node: &dyn Tree, p: P, r: Option<R>) -> Option<R> {
        let scanned = self.scan(node, p);
        self.reduce(scanned, r)
    }

    /// Reduces two results into a combined result.
    /// The default implementation is to return the first parameter.
    /// @param r1 the first of the values to be combined
    /// @param r2 the second of the values to be combined
    /// @return the result of combining the two parameters
    fn reduce(&mut self, r1: Option<R>, _r2: Option<R>) -> Option<R> {
        r1
    }

    fn visit_compilation_unit(&mut self, node: &dyn CompilationUnitTree, p: P) -> Option<R> {
        let mut r = match node.get_package() { Some(child) => { let child: Rc<dyn Tree> = child; self.scan(child.as_ref(), p.clone()) }, None => None };
        for child in node.get_imports() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_type_decls() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        if let Some(child) = node.get_module() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_package(&mut self, node: &dyn PackageTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_package_name().as_ref(), p.clone(), r);
        r
    }

    fn visit_import(&mut self, node: &dyn ImportTree, p: P) -> Option<R> {
        return self.scan(node.get_qualified_identifier().as_ref(), p.clone());
    }

    fn visit_class(&mut self, node: &dyn ClassTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_modifiers().as_ref(), p.clone());
        for child in node.get_type_parameters() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        if let Some(child) = node.get_extends_clause() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_implements_clause() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_permits_clause() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_members() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_method(&mut self, node: &dyn MethodTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_modifiers().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_return_type().as_ref(), p.clone(), r);
        for child in node.get_type_parameters() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_parameters() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        if let Some(child) = node.get_receiver_parameter() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_throws() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        if let Some(child) = node.get_body() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_default_value().as_ref(), p.clone(), r);
        r
    }

    fn visit_variable(&mut self, node: &dyn VariableTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_modifiers().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_type().as_ref(), p.clone(), r);
        r = self.scan_and_reduce(node.get_name_expression().as_ref(), p.clone(), r);
        if let Some(child) = node.get_initializer() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_empty_statement(&mut self, _node: &dyn EmptyStatementTree, _p: P) -> Option<R> {
        None
    }

    fn visit_block(&mut self, node: &dyn BlockTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_statements() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_do_while_loop(&mut self, node: &dyn DoWhileLoopTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_statement().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_condition().as_ref(), p.clone(), r);
        r
    }

    fn visit_while_loop(&mut self, node: &dyn WhileLoopTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_condition().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_statement().as_ref(), p.clone(), r);
        r
    }

    fn visit_for_loop(&mut self, node: &dyn ForLoopTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_initializer() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_condition().as_ref(), p.clone(), r);
        for child in node.get_update() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_statement().as_ref(), p.clone(), r);
        r
    }

    fn visit_enhanced_for_loop(&mut self, node: &dyn EnhancedForLoopTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_variable().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_expression().as_ref(), p.clone(), r);
        r = self.scan_and_reduce(node.get_statement().as_ref(), p.clone(), r);
        r
    }

    fn visit_labeled_statement(&mut self, node: &dyn LabeledStatementTree, p: P) -> Option<R> {
        return self.scan(node.get_statement().as_ref(), p.clone());
    }

    fn visit_switch(&mut self, node: &dyn SwitchTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_expression().as_ref(), p.clone());
        for child in node.get_cases() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_switch_expression(&mut self, node: &dyn SwitchExpressionTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_expression().as_ref(), p.clone());
        for child in node.get_cases() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_synchronized(&mut self, node: &dyn SynchronizedTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_expression().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_block().as_ref(), p.clone(), r);
        r
    }

    fn visit_try(&mut self, node: &dyn TryTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_resources() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_block().as_ref(), p.clone(), r);
        for child in node.get_catches() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        if let Some(child) = node.get_finally_block() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_catch(&mut self, node: &dyn CatchTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_parameter().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_block().as_ref(), p.clone(), r);
        r
    }

    fn visit_conditional_expression(&mut self, node: &dyn ConditionalExpressionTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_condition().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_true_expression().as_ref(), p.clone(), r);
        r = self.scan_and_reduce(node.get_false_expression().as_ref(), p.clone(), r);
        r
    }

    fn visit_if(&mut self, node: &dyn IfTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_condition().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_then_statement().as_ref(), p.clone(), r);
        if let Some(child) = node.get_else_statement() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_expression_statement(&mut self, node: &dyn ExpressionStatementTree, p: P) -> Option<R> {
        return self.scan(node.get_expression().as_ref(), p.clone());
    }

    fn visit_break(&mut self, _node: &dyn BreakTree, _p: P) -> Option<R> {
        None
    }

    fn visit_continue(&mut self, _node: &dyn ContinueTree, _p: P) -> Option<R> {
        None
    }

    fn visit_return(&mut self, node: &dyn ReturnTree, p: P) -> Option<R> {
        return self.scan(node.get_expression().as_ref(), p.clone());
    }

    fn visit_throw(&mut self, node: &dyn ThrowTree, p: P) -> Option<R> {
        return self.scan(node.get_expression().as_ref(), p.clone());
    }

    fn visit_assert(&mut self, node: &dyn AssertTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_condition().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_detail().as_ref(), p.clone(), r);
        r
    }

    fn visit_method_invocation(&mut self, node: &dyn MethodInvocationTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_type_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_method_select().as_ref(), p.clone(), r);
        for child in node.get_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_new_class(&mut self, node: &dyn NewClassTree, p: P) -> Option<R> {
        let mut r = match node.get_enclosing_expression() { Some(child) => { let child: Rc<dyn Tree> = child; self.scan(child.as_ref(), p.clone()) }, None => None };
        r = self.scan_and_reduce(node.get_identifier().as_ref(), p.clone(), r);
        for child in node.get_type_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_class_body().as_ref(), p.clone(), r);
        r
    }

    fn visit_lambda_expression(&mut self, node: &dyn LambdaExpressionTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_parameters() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_body().as_ref(), p.clone(), r);
        r
    }

    fn visit_parenthesized(&mut self, node: &dyn ParenthesizedTree, p: P) -> Option<R> {
        return self.scan(node.get_expression().as_ref(), p.clone());
    }

    fn visit_assignment(&mut self, node: &dyn AssignmentTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_variable().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_expression().as_ref(), p.clone(), r);
        r
    }

    fn visit_compound_assignment(&mut self, node: &dyn CompoundAssignmentTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_variable().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_expression().as_ref(), p.clone(), r);
        r
    }

    fn visit_unary(&mut self, node: &dyn UnaryTree, p: P) -> Option<R> {
        return self.scan(node.get_expression().as_ref(), p.clone());
    }

    fn visit_binary(&mut self, node: &dyn BinaryTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_left_operand().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_right_operand().as_ref(), p.clone(), r);
        r
    }

    fn visit_type_cast(&mut self, node: &dyn TypeCastTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_type().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_expression().as_ref(), p.clone(), r);
        r
    }

    fn visit_binding_pattern(&mut self, node: &dyn BindingPatternTree, p: P) -> Option<R> {
        return self.scan(node.get_variable().as_ref(), p.clone());
    }

    fn visit_default_case_label(&mut self, _node: &dyn DefaultCaseLabelTree, _p: P) -> Option<R> {
        None
    }

    fn visit_array_access(&mut self, node: &dyn ArrayAccessTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_expression().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_index().as_ref(), p.clone(), r);
        r
    }

    fn visit_member_select(&mut self, node: &dyn MemberSelectTree, p: P) -> Option<R> {
        return self.scan(node.get_expression().as_ref(), p.clone());
    }

    fn visit_parenthesized_pattern(&mut self, node: &dyn ParenthesizedPatternTree, p: P) -> Option<R> {
        return self.scan(node.get_pattern().as_ref(), p.clone());
    }

    fn visit_member_reference(&mut self, node: &dyn MemberReferenceTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_qualifier_expression().as_ref(), p.clone());
        for child in node.get_type_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_identifier(&mut self, _node: &dyn IdentifierTree, _p: P) -> Option<R> {
        None
    }

    fn visit_literal(&mut self, _node: &dyn LiteralTree, _p: P) -> Option<R> {
        None
    }

    fn visit_primitive_type(&mut self, _node: &dyn PrimitiveTypeTree, _p: P) -> Option<R> {
        None
    }

    fn visit_array_type(&mut self, node: &dyn ArrayTypeTree, p: P) -> Option<R> {
        return self.scan(node.get_type().as_ref(), p.clone());
    }

    fn visit_parameterized_type(&mut self, node: &dyn ParameterizedTypeTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_type().as_ref(), p.clone());
        for child in node.get_type_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_union_type(&mut self, node: &dyn UnionTypeTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_type_alternatives() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_intersection_type(&mut self, node: &dyn IntersectionTypeTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_bounds() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_type_parameter(&mut self, node: &dyn TypeParameterTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_bounds() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_wildcard(&mut self, node: &dyn WildcardTree, p: P) -> Option<R> {
        return self.scan(node.get_bound().as_ref(), p.clone());
    }

    fn visit_modifiers(&mut self, node: &dyn ModifiersTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_annotation(&mut self, node: &dyn AnnotationTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_annotation_type().as_ref(), p.clone());
        for child in node.get_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_annotated_type(&mut self, node: &dyn AnnotatedTypeTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_underlying_type().as_ref(), p.clone(), r);
        r
    }

    fn visit_module(&mut self, node: &dyn ModuleTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_name().as_ref(), p.clone(), r);
        for child in node.get_directives() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_exports(&mut self, node: &dyn ExportsTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_package_name().as_ref(), p.clone());
        for child in node.get_module_names() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_opens(&mut self, node: &dyn OpensTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_package_name().as_ref(), p.clone());
        for child in node.get_module_names() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_provides(&mut self, node: &dyn ProvidesTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_service_name().as_ref(), p.clone());
        for child in node.get_implementation_names() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        r
    }

    fn visit_requires(&mut self, node: &dyn RequiresTree, p: P) -> Option<R> {
        return self.scan(node.get_module_name().as_ref(), p.clone());
    }

    fn visit_uses(&mut self, node: &dyn UsesTree, p: P) -> Option<R> {
        return self.scan(node.get_service_name().as_ref(), p.clone());
    }

    fn visit_other(&mut self, _node: &dyn Tree, _p: P) -> Option<R> {
        None
    }

    fn visit_erroneous(&mut self, _node: &dyn ErroneousTree, _p: P) -> Option<R> {
        None
    }

    fn visit_yield(&mut self, node: &dyn YieldTree, p: P) -> Option<R> {
        return self.scan(node.get_value().as_ref(), p.clone());
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the children in left to right order.
    fn visit_guarded_pattern(
        &mut self,
        node: &dyn crate::tree::guarded_pattern_tree::GuardedPatternTree,
        p: P,
    ) -> Option<R> {
        let mut r = self.scan(node.get_pattern().as_ref(), p.clone());
        r = self.scan_and_reduce(node.get_expression().as_ref(), p.clone(), r);
        r
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the children in left to right order.
    fn visit_case(&mut self, node: &dyn crate::tree::case_tree::CaseTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_expressions() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        if node.get_case_kind() == crate::tree::case_tree::CaseKind::RULE {
            if let Some(child) = node.get_body() {
                let child: Rc<dyn Tree> = child;
                r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
            }
        } else if let Some(statements) = node.get_statements() {
            for child in statements {
                let child: Rc<dyn Tree> = child;
                r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
            }
        }
        r
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the children in left to right order.
    fn visit_instance_of(
        &mut self,
        node: &dyn crate::tree::instance_of_tree::InstanceOfTree,
        p: P,
    ) -> Option<R> {
        let mut r = self.scan(node.get_expression().as_ref(), p.clone());
        if let Some(child) = node.get_pattern() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        } else {
            r = self.scan_and_reduce(node.get_type().as_ref(), p.clone(), r);
        }
        r
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the children in left to right order.
    fn visit_new_array(
        &mut self,
        node: &dyn crate::tree::new_array_tree::NewArrayTree,
        p: P,
    ) -> Option<R> {
        let mut r = match node.get_type() {
            Some(child) => {
                let child: Rc<dyn Tree> = child;
                self.scan(child.as_ref(), p.clone())
            }
            None => None,
        };
        for child in node.get_dimensions() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_initializers() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
        }
        for dim_anno in node.get_dim_annotations() {
            for child in dim_anno {
                let child: Rc<dyn Tree> = child;
                r = self.scan_and_reduce(child.as_ref(), p.clone(), r);
            }
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::TreeScanner;
    use crate::tree::identifier_tree::IdentifierTree;
    use crate::tree::tree::Tree;
    use crate::tree::tree_visitor::TreeVisitor;
    use crate::util::test_support::{BinaryTreeFixture, IdentTree};
    use std::rc::Rc;

    struct CountingScanner {
        count: usize,
    }

    impl TreeVisitor<Option<usize>, ()> for CountingScanner {}

    impl TreeScanner<usize, ()> for CountingScanner {
        fn visit_identifier(&mut self, _node: &dyn IdentifierTree, _p: ()) -> Option<usize> {
            self.count += 1;
            Some(self.count)
        }
    }

    #[test]
    fn scan_dispatches_through_visit_methods() {
        let ident = IdentTree;
        let mut scanner = CountingScanner { count: 0 };
        assert_eq!(scanner.scan(&ident, ()), Some(1));
        assert_eq!(scanner.count, 1);
    }

    #[test]
    fn default_visit_scans_children_in_order_and_reduces() {
        let left: Rc<dyn crate::tree::expression_tree::ExpressionTree> = Rc::new(IdentTree);
        let right: Rc<dyn crate::tree::expression_tree::ExpressionTree> = Rc::new(IdentTree);
        let binary = BinaryTreeFixture { left, right };
        let mut scanner = CountingScanner { count: 0 };
        // default visitBinary scans left then right; reduce keeps the first
        // argument (the last scanned result) per the Java default.
        assert_eq!(scanner.scan(&binary, ()), Some(2));
        assert_eq!(scanner.count, 2);
    }

    #[test]
    fn scan_opt_handles_null_trees() {
        let mut scanner = CountingScanner { count: 0 };
        assert_eq!(scanner.scan_opt(None, ()), None);
        let ident: Rc<dyn Tree> = Rc::new(IdentTree);
        assert_eq!(scanner.scan_opt(Some(ident), ()), Some(1));
    }
}

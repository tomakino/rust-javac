//! A TreeVisitor that visits all the child tree nodes, and provides support for
//! maintaining a path for the parent nodes.  To visit nodes of a particular
//! type, just override the corresponding visitXYZ method.  Inside your method,
//! call super.visitXYZ to visit descendant nodes.
//!
//! PORTING_RULES (§14.4):
//! * Java `TreePathScanner<R,P> extends TreeScanner<R,P>` is ported as an
//!   **independent** trait (not a TreeScanner subtrait): the Java scan(Tree, P)
//!   override must be visible to the default visit methods that call `this.scan`,
//!   which Rust trait inheritance cannot express (a TreeScanner default method
//!   always resolves `self.scan` to TreeScanner::scan).  The 66 visit defaults are
//!   mechanically generated from TreeScanner.java with the child scans going
//!   through this trait's scan; the difference is registered.
//! * required get_current_path/set_current_path let implementations provide the
//!   Java private `TreePath path` field.
//! * a Java null result is Option<R>; the Java `R scan(TreePath, P)` is
//!   scan_path(Rc<TreePath>, P); scan(Rc<dyn Tree>, P) keeps the Java null check
//!   (Option input) and the prev/current path exchange verbatim.
//! * P: Clone (same adaptation as TreeScanner).
//!
//! @author Jonathan Gibbons
//! @since 1.6

use super::dispatch::dispatch_tree_path_scanner;
use super::tree_path::TreePath;
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
use crate::tree::tree::Tree;
use crate::tree::tree_visitor::TreeVisitor;
use std::rc::Rc;

/// A TreeVisitor that visits all the child tree nodes, and provides
/// support for maintaining a path for the parent nodes.
///
/// @param <R> the return type of this visitor's methods.  Use {@link Void} for
///            visitors that do not need to return results.
/// @param <P> the type of the additional parameter to this visitor's methods.
///            Use {@code Void} for visitors that do not need an additional parameter.
///
/// @author Jonathan Gibbons
/// @since 1.6
pub trait TreePathScanner<R, P: Clone>: TreeVisitor<Option<R>, P> {
    /// Returns the current path for the node, as built up by the currently
    /// active set of scan calls.
    /// @return the current path
    ///
    /// Java: private TreePath path.  Implementations provide the field.
    fn get_current_path(&self) -> Option<Rc<TreePath>>;

    /// Java: this.path = path.
    fn set_current_path(&mut self, path: Option<Rc<TreePath>>);

    /// Scans a tree from a position identified by a TreePath.
    /// @param path the path identifying the node to be scanned
    /// @param p a parameter value passed to visit methods
    /// @return the result value from the visit method
    ///
    /// Java: public R scan(TreePath path, P p) — this.path = path; try
    /// return path.getLeaf().accept(this, p); finally this.path = null;
    fn scan_path(&mut self, path: Rc<TreePath>, p: P) -> Option<R> {
        self.set_current_path(Some(path.clone()));
        let leaf = path.get_leaf();
        let result = dispatch_tree_path_scanner(self, leaf.as_ref(), p);
        self.set_current_path(None);
        result
    }

    /// Scans a single node.
    /// The current path is updated for the duration of the scan.
    ///
    /// @apiNote This method should normally only be called by the
    /// scanner's {@code visit} methods, as part of an ongoing scan
    /// initiated by {@link #scan(TreePath,Object) scan(TreePath, P)}.
    /// The one exception is that it may also be called to initiate
    /// a full scan of a {@link com.sun.source.tree.CompilationUnitTree}.
    ///
    /// @return the result value from the visit method
    ///
    /// Java: public R scan(Tree tree, P p) — if (tree == null) return null;
    /// TreePath prev = path; path = new TreePath(path, tree); try return
    /// tree.accept(this, p); finally path = prev;
    fn scan(&mut self, tree: Rc<dyn Tree>, p: P) -> Option<R> {
        let prev = self.get_current_path();
        let current = match &prev {
            Some(prev) => Rc::new(TreePath::new_child(prev.clone(), tree.clone())),
            None => {
                // Java: new TreePath(null, tree) — only legal for a compilation
                // unit root; otherwise the Java constructor throws NPE.
                if tree.get_kind() != crate::tree::tree::Kind::COMPILATION_UNIT {
                    panic!("NullPointerException: TreePathScanner.scan without a current path");
                }
                Rc::new(TreePath::new_root_tree(tree.clone()))
            }
        };
        self.set_current_path(Some(current));
        let result = dispatch_tree_path_scanner(self, tree.as_ref(), p);
        self.set_current_path(prev);
        result
    }

    /// Java: scan(Tree tree, P p) with a possibly-null tree.
    fn scan_opt(&mut self, tree: Option<Rc<dyn Tree>>, p: P) -> Option<R> {
        match tree {
            Some(tree) => self.scan(tree, p),
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
                self.scan(node.clone(), p.clone())
            } else {
                self.scan_and_reduce(node.clone(), p.clone(), r)
            };
            first = false;
        }
        r
    }

    /// Java: private R scanAndReduce(Tree node, P p, R r).
    fn scan_and_reduce(&mut self, node: Rc<dyn Tree>, p: P, r: Option<R>) -> Option<R> {
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
        let mut r = match node.get_package() { Some(child) => { let child: Rc<dyn Tree> = child; self.scan(child, p.clone()) }, None => None };
        for child in node.get_imports() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_type_decls() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        if let Some(child) = node.get_module() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_package(&mut self, node: &dyn PackageTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_package_name(), p.clone(), r);
        r
    }

    fn visit_import(&mut self, node: &dyn ImportTree, p: P) -> Option<R> {
        self.scan(node.get_qualified_identifier(), p.clone())
    }

    fn visit_class(&mut self, node: &dyn ClassTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_modifiers(), p.clone());
        for child in node.get_type_parameters() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        if let Some(child) = node.get_extends_clause() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_implements_clause() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_permits_clause() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_members() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_method(&mut self, node: &dyn MethodTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_modifiers(), p.clone());
        r = self.scan_and_reduce(node.get_return_type(), p.clone(), r);
        for child in node.get_type_parameters() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_parameters() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        if let Some(child) = node.get_receiver_parameter() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_throws() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        if let Some(child) = node.get_body() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_default_value(), p.clone(), r);
        r
    }

    fn visit_variable(&mut self, node: &dyn VariableTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_modifiers(), p.clone());
        r = self.scan_and_reduce(node.get_type(), p.clone(), r);
        r = self.scan_and_reduce(node.get_name_expression(), p.clone(), r);
        if let Some(child) = node.get_initializer() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
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
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_do_while_loop(&mut self, node: &dyn DoWhileLoopTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_statement(), p.clone());
        r = self.scan_and_reduce(node.get_condition(), p.clone(), r);
        r
    }

    fn visit_while_loop(&mut self, node: &dyn WhileLoopTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_condition(), p.clone());
        r = self.scan_and_reduce(node.get_statement(), p.clone(), r);
        r
    }

    fn visit_for_loop(&mut self, node: &dyn ForLoopTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_initializer() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_condition(), p.clone(), r);
        for child in node.get_update() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_statement(), p.clone(), r);
        r
    }

    fn visit_enhanced_for_loop(&mut self, node: &dyn EnhancedForLoopTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_variable(), p.clone());
        r = self.scan_and_reduce(node.get_expression(), p.clone(), r);
        r = self.scan_and_reduce(node.get_statement(), p.clone(), r);
        r
    }

    fn visit_labeled_statement(&mut self, node: &dyn LabeledStatementTree, p: P) -> Option<R> {
        self.scan(node.get_statement(), p.clone())
    }

    fn visit_switch(&mut self, node: &dyn SwitchTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_expression(), p.clone());
        for child in node.get_cases() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_switch_expression(&mut self, node: &dyn SwitchExpressionTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_expression(), p.clone());
        for child in node.get_cases() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_synchronized(&mut self, node: &dyn SynchronizedTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_expression(), p.clone());
        r = self.scan_and_reduce(node.get_block(), p.clone(), r);
        r
    }

    fn visit_try(&mut self, node: &dyn TryTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_resources() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_block(), p.clone(), r);
        for child in node.get_catches() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        if let Some(child) = node.get_finally_block() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_catch(&mut self, node: &dyn CatchTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_parameter(), p.clone());
        r = self.scan_and_reduce(node.get_block(), p.clone(), r);
        r
    }

    fn visit_conditional_expression(&mut self, node: &dyn ConditionalExpressionTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_condition(), p.clone());
        r = self.scan_and_reduce(node.get_true_expression(), p.clone(), r);
        r = self.scan_and_reduce(node.get_false_expression(), p.clone(), r);
        r
    }

    fn visit_if(&mut self, node: &dyn IfTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_condition(), p.clone());
        r = self.scan_and_reduce(node.get_then_statement(), p.clone(), r);
        if let Some(child) = node.get_else_statement() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_expression_statement(&mut self, node: &dyn ExpressionStatementTree, p: P) -> Option<R> {
        self.scan(node.get_expression(), p.clone())
    }

    fn visit_break(&mut self, _node: &dyn BreakTree, _p: P) -> Option<R> {
        None
    }

    fn visit_continue(&mut self, _node: &dyn ContinueTree, _p: P) -> Option<R> {
        None
    }

    fn visit_return(&mut self, node: &dyn ReturnTree, p: P) -> Option<R> {
        self.scan(node.get_expression(), p.clone())
    }

    fn visit_throw(&mut self, node: &dyn ThrowTree, p: P) -> Option<R> {
        self.scan(node.get_expression(), p.clone())
    }

    fn visit_assert(&mut self, node: &dyn AssertTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_condition(), p.clone());
        r = self.scan_and_reduce(node.get_detail(), p.clone(), r);
        r
    }

    fn visit_method_invocation(&mut self, node: &dyn MethodInvocationTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_type_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_method_select(), p.clone(), r);
        for child in node.get_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_new_class(&mut self, node: &dyn NewClassTree, p: P) -> Option<R> {
        let mut r = match node.get_enclosing_expression() { Some(child) => { let child: Rc<dyn Tree> = child; self.scan(child, p.clone()) }, None => None };
        r = self.scan_and_reduce(node.get_identifier(), p.clone(), r);
        for child in node.get_type_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_class_body(), p.clone(), r);
        r
    }

    fn visit_lambda_expression(&mut self, node: &dyn LambdaExpressionTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_parameters() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_body(), p.clone(), r);
        r
    }

    fn visit_parenthesized(&mut self, node: &dyn ParenthesizedTree, p: P) -> Option<R> {
        self.scan(node.get_expression(), p.clone())
    }

    fn visit_assignment(&mut self, node: &dyn AssignmentTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_variable(), p.clone());
        r = self.scan_and_reduce(node.get_expression(), p.clone(), r);
        r
    }

    fn visit_compound_assignment(&mut self, node: &dyn CompoundAssignmentTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_variable(), p.clone());
        r = self.scan_and_reduce(node.get_expression(), p.clone(), r);
        r
    }

    fn visit_unary(&mut self, node: &dyn UnaryTree, p: P) -> Option<R> {
        self.scan(node.get_expression(), p.clone())
    }

    fn visit_binary(&mut self, node: &dyn BinaryTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_left_operand(), p.clone());
        r = self.scan_and_reduce(node.get_right_operand(), p.clone(), r);
        r
    }

    fn visit_type_cast(&mut self, node: &dyn TypeCastTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_type(), p.clone());
        r = self.scan_and_reduce(node.get_expression(), p.clone(), r);
        r
    }

    fn visit_binding_pattern(&mut self, node: &dyn BindingPatternTree, p: P) -> Option<R> {
        self.scan(node.get_variable(), p.clone())
    }

    fn visit_default_case_label(&mut self, _node: &dyn DefaultCaseLabelTree, _p: P) -> Option<R> {
        None
    }

    fn visit_array_access(&mut self, node: &dyn ArrayAccessTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_expression(), p.clone());
        r = self.scan_and_reduce(node.get_index(), p.clone(), r);
        r
    }

    fn visit_member_select(&mut self, node: &dyn MemberSelectTree, p: P) -> Option<R> {
        self.scan(node.get_expression(), p.clone())
    }

    fn visit_parenthesized_pattern(&mut self, node: &dyn ParenthesizedPatternTree, p: P) -> Option<R> {
        self.scan(node.get_pattern(), p.clone())
    }

    fn visit_member_reference(&mut self, node: &dyn MemberReferenceTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_qualifier_expression(), p.clone());
        for child in node.get_type_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
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
        self.scan(node.get_type(), p.clone())
    }

    fn visit_parameterized_type(&mut self, node: &dyn ParameterizedTypeTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_type(), p.clone());
        for child in node.get_type_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_union_type(&mut self, node: &dyn UnionTypeTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_type_alternatives() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_intersection_type(&mut self, node: &dyn IntersectionTypeTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_bounds() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_type_parameter(&mut self, node: &dyn TypeParameterTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_bounds() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_wildcard(&mut self, node: &dyn WildcardTree, p: P) -> Option<R> {
        self.scan(node.get_bound(), p.clone())
    }

    fn visit_modifiers(&mut self, node: &dyn ModifiersTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_annotation(&mut self, node: &dyn AnnotationTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_annotation_type(), p.clone());
        for child in node.get_arguments() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_annotated_type(&mut self, node: &dyn AnnotatedTypeTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_underlying_type(), p.clone(), r);
        r
    }

    fn visit_module(&mut self, node: &dyn ModuleTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r = self.scan_and_reduce(node.get_name(), p.clone(), r);
        for child in node.get_directives() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_exports(&mut self, node: &dyn ExportsTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_package_name(), p.clone());
        for child in node.get_module_names() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_opens(&mut self, node: &dyn OpensTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_package_name(), p.clone());
        for child in node.get_module_names() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_provides(&mut self, node: &dyn ProvidesTree, p: P) -> Option<R> {
        let mut r = self.scan(node.get_service_name(), p.clone());
        for child in node.get_implementation_names() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        r
    }

    fn visit_requires(&mut self, node: &dyn RequiresTree, p: P) -> Option<R> {
        self.scan(node.get_module_name(), p.clone())
    }

    fn visit_uses(&mut self, node: &dyn UsesTree, p: P) -> Option<R> {
        self.scan(node.get_service_name(), p.clone())
    }

    fn visit_other(&mut self, _node: &dyn Tree, _p: P) -> Option<R> {
        None
    }

    fn visit_erroneous(&mut self, _node: &dyn ErroneousTree, _p: P) -> Option<R> {
        None
    }

    fn visit_yield(&mut self, node: &dyn YieldTree, p: P) -> Option<R> {
        self.scan(node.get_value(), p.clone())
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the children in left to right order.
    fn visit_guarded_pattern(
        &mut self,
        node: &dyn crate::tree::guarded_pattern_tree::GuardedPatternTree,
        p: P,
    ) -> Option<R> {
        let mut r = self.scan(node.get_pattern(), p.clone());
        r = self.scan_and_reduce(node.get_expression(), p.clone(), r);
        r
    }

    /// {@inheritDoc}
    ///
    /// @implSpec This implementation scans the children in left to right order.
    fn visit_case(&mut self, node: &dyn crate::tree::case_tree::CaseTree, p: P) -> Option<R> {
        let mut r: Option<R> = None;
        for child in node.get_expressions() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        if node.get_case_kind() == crate::tree::case_tree::CaseKind::RULE {
            if let Some(child) = node.get_body() {
                let child: Rc<dyn Tree> = child;
                r = self.scan_and_reduce(child, p.clone(), r);
            }
        } else if let Some(statements) = node.get_statements() {
            for child in statements {
                let child: Rc<dyn Tree> = child;
                r = self.scan_and_reduce(child, p.clone(), r);
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
        let mut r = self.scan(node.get_expression(), p.clone());
        if let Some(child) = node.get_pattern() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        } else {
            r = self.scan_and_reduce(node.get_type(), p.clone(), r);
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
            Some(child) => self.scan(child, p.clone()),
            None => None,
        };
        for child in node.get_dimensions() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_initializers() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for child in node.get_annotations() {
            let child: Rc<dyn Tree> = child;
            r = self.scan_and_reduce(child, p.clone(), r);
        }
        for dim_anno in node.get_dim_annotations() {
            for child in dim_anno {
                let child: Rc<dyn Tree> = child;
                r = self.scan_and_reduce(child, p.clone(), r);
            }
        }
        r
    }
}

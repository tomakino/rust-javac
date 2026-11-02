// PORTING ADAPTER (not a 1:1 port): the Rust form of the Java double dispatch
// Tree.accept(TreeVisitor, D).  Tree.accept is generic and therefore not
// callable on &dyn Tree; this function reproduces the dispatch from getKind()
// plus the §13 object-conversion accessors (docs/porting adapter.md §14), exactly
// as javax.lang.model's dispatch_element/dispatch_type do for elements/types.
// The Kind-to-visit-method mapping follows the Java TreeVisitor method set; the
// Kind values that share one Java accept method (e.g. the four class kinds ->
// visitClass, the eight unary kinds -> visitUnary) are grouped like javac's
// JCTree.accept.

use crate::doctree::doc_tree::DocTree;
use crate::tree::tree::{Kind, Tree};
use crate::tree::tree_visitor::TreeVisitor;
use crate::util::doc_tree_path_scanner::DocTreePathScanner;
use crate::util::doc_tree_scanner::DocTreeScanner;
use crate::util::simple_doc_tree_visitor::SimpleDocTreeVisitor;
use crate::util::simple_tree_visitor::SimpleTreeVisitor;
use crate::util::tree_path_scanner::TreePathScanner;
use crate::util::tree_scanner::TreeScanner;

#[allow(dead_code)] // used by the TreeScanner/SimpleTreeVisitor batch
pub(crate) fn dispatch_tree<R, P, V: TreeVisitor<R, P> + ?Sized>(
    visitor: &mut V,
    tree: &dyn Tree,
    p: P,
) -> R {
    match tree.get_kind() {
        Kind::ANNOTATED_TYPE => visitor.visit_annotated_type(
            tree.as_annotated_type_tree()
                .expect("ClassCastException: AnnotatedTypeTree"),
            p,
        ),
        Kind::ANNOTATION => visitor.visit_annotation(
            tree.as_annotation_tree()
                .expect("ClassCastException: AnnotationTree"),
            p,
        ),
        Kind::TYPE_ANNOTATION => visitor.visit_annotation(
            tree.as_annotation_tree()
                .expect("ClassCastException: AnnotationTree"),
            p,
        ),
        Kind::ARRAY_ACCESS => visitor.visit_array_access(
            tree.as_array_access_tree()
                .expect("ClassCastException: ArrayAccessTree"),
            p,
        ),
        Kind::ARRAY_TYPE => visitor.visit_array_type(
            tree.as_array_type_tree()
                .expect("ClassCastException: ArrayTypeTree"),
            p,
        ),
        Kind::ASSERT => visitor.visit_assert(
            tree.as_assert_tree()
                .expect("ClassCastException: AssertTree"),
            p,
        ),
        Kind::ASSIGNMENT => visitor.visit_assignment(
            tree.as_assignment_tree()
                .expect("ClassCastException: AssignmentTree"),
            p,
        ),
        Kind::BLOCK => visitor.visit_block(
            tree.as_block_tree().expect("ClassCastException: BlockTree"),
            p,
        ),
        Kind::BREAK => visitor.visit_break(
            tree.as_break_tree().expect("ClassCastException: BreakTree"),
            p,
        ),
        Kind::CASE => visitor.visit_case(
            tree.as_case_tree().expect("ClassCastException: CaseTree"),
            p,
        ),
        Kind::CATCH => visitor.visit_catch(
            tree.as_catch_tree().expect("ClassCastException: CatchTree"),
            p,
        ),
        Kind::CLASS => visitor.visit_class(
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::COMPILATION_UNIT => visitor.visit_compilation_unit(
            tree.as_compilation_unit_tree()
                .expect("ClassCastException: CompilationUnitTree"),
            p,
        ),
        Kind::CONDITIONAL_EXPRESSION => visitor.visit_conditional_expression(
            tree.as_conditional_expression_tree()
                .expect("ClassCastException: ConditionalExpressionTree"),
            p,
        ),
        Kind::CONTINUE => visitor.visit_continue(
            tree.as_continue_tree()
                .expect("ClassCastException: ContinueTree"),
            p,
        ),
        Kind::DO_WHILE_LOOP => visitor.visit_do_while_loop(
            tree.as_do_while_loop_tree()
                .expect("ClassCastException: DoWhileLoopTree"),
            p,
        ),
        Kind::ENHANCED_FOR_LOOP => visitor.visit_enhanced_for_loop(
            tree.as_enhanced_for_loop_tree()
                .expect("ClassCastException: EnhancedForLoopTree"),
            p,
        ),
        Kind::EXPRESSION_STATEMENT => visitor.visit_expression_statement(
            tree.as_expression_statement_tree()
                .expect("ClassCastException: ExpressionStatementTree"),
            p,
        ),
        Kind::MEMBER_SELECT => visitor.visit_member_select(
            tree.as_member_select_tree()
                .expect("ClassCastException: MemberSelectTree"),
            p,
        ),
        Kind::MEMBER_REFERENCE => visitor.visit_member_reference(
            tree.as_member_reference_tree()
                .expect("ClassCastException: MemberReferenceTree"),
            p,
        ),
        Kind::FOR_LOOP => visitor.visit_for_loop(
            tree.as_for_loop_tree()
                .expect("ClassCastException: ForLoopTree"),
            p,
        ),
        Kind::IDENTIFIER => visitor.visit_identifier(
            tree.as_identifier_tree()
                .expect("ClassCastException: IdentifierTree"),
            p,
        ),
        Kind::IF => visitor.visit_if(tree.as_if_tree().expect("ClassCastException: IfTree"), p),
        Kind::IMPORT => visitor.visit_import(
            tree.as_import_tree()
                .expect("ClassCastException: ImportTree"),
            p,
        ),
        Kind::INSTANCE_OF => visitor.visit_instance_of(
            tree.as_instance_of_tree()
                .expect("ClassCastException: InstanceOfTree"),
            p,
        ),
        Kind::LABELED_STATEMENT => visitor.visit_labeled_statement(
            tree.as_labeled_statement_tree()
                .expect("ClassCastException: LabeledStatementTree"),
            p,
        ),
        Kind::METHOD => visitor.visit_method(
            tree.as_method_tree()
                .expect("ClassCastException: MethodTree"),
            p,
        ),
        Kind::METHOD_INVOCATION => visitor.visit_method_invocation(
            tree.as_method_invocation_tree()
                .expect("ClassCastException: MethodInvocationTree"),
            p,
        ),
        Kind::MODIFIERS => visitor.visit_modifiers(
            tree.as_modifiers_tree()
                .expect("ClassCastException: ModifiersTree"),
            p,
        ),
        Kind::NEW_ARRAY => visitor.visit_new_array(
            tree.as_new_array_tree()
                .expect("ClassCastException: NewArrayTree"),
            p,
        ),
        Kind::NEW_CLASS => visitor.visit_new_class(
            tree.as_new_class_tree()
                .expect("ClassCastException: NewClassTree"),
            p,
        ),
        Kind::LAMBDA_EXPRESSION => visitor.visit_lambda_expression(
            tree.as_lambda_expression_tree()
                .expect("ClassCastException: LambdaExpressionTree"),
            p,
        ),
        Kind::PACKAGE => visitor.visit_package(
            tree.as_package_tree()
                .expect("ClassCastException: PackageTree"),
            p,
        ),
        Kind::PARENTHESIZED => visitor.visit_parenthesized(
            tree.as_parenthesized_tree()
                .expect("ClassCastException: ParenthesizedTree"),
            p,
        ),
        Kind::BINDING_PATTERN => visitor.visit_binding_pattern(
            tree.as_binding_pattern_tree()
                .expect("ClassCastException: BindingPatternTree"),
            p,
        ),
        Kind::GUARDED_PATTERN => visitor.visit_guarded_pattern(
            tree.as_guarded_pattern_tree()
                .expect("ClassCastException: GuardedPatternTree"),
            p,
        ),
        Kind::PARENTHESIZED_PATTERN => visitor.visit_parenthesized_pattern(
            tree.as_parenthesized_pattern_tree()
                .expect("ClassCastException: ParenthesizedPatternTree"),
            p,
        ),
        Kind::DEFAULT_CASE_LABEL => visitor.visit_default_case_label(
            tree.as_default_case_label_tree()
                .expect("ClassCastException: DefaultCaseLabelTree"),
            p,
        ),
        Kind::PRIMITIVE_TYPE => visitor.visit_primitive_type(
            tree.as_primitive_type_tree()
                .expect("ClassCastException: PrimitiveTypeTree"),
            p,
        ),
        Kind::RETURN => visitor.visit_return(
            tree.as_return_tree()
                .expect("ClassCastException: ReturnTree"),
            p,
        ),
        Kind::EMPTY_STATEMENT => visitor.visit_empty_statement(
            tree.as_empty_statement_tree()
                .expect("ClassCastException: EmptyStatementTree"),
            p,
        ),
        Kind::SWITCH => visitor.visit_switch(
            tree.as_switch_tree()
                .expect("ClassCastException: SwitchTree"),
            p,
        ),
        Kind::SWITCH_EXPRESSION => visitor.visit_switch_expression(
            tree.as_switch_expression_tree()
                .expect("ClassCastException: SwitchExpressionTree"),
            p,
        ),
        Kind::SYNCHRONIZED => visitor.visit_synchronized(
            tree.as_synchronized_tree()
                .expect("ClassCastException: SynchronizedTree"),
            p,
        ),
        Kind::THROW => visitor.visit_throw(
            tree.as_throw_tree().expect("ClassCastException: ThrowTree"),
            p,
        ),
        Kind::TRY => visitor.visit_try(tree.as_try_tree().expect("ClassCastException: TryTree"), p),
        Kind::PARAMETERIZED_TYPE => visitor.visit_parameterized_type(
            tree.as_parameterized_type_tree()
                .expect("ClassCastException: ParameterizedTypeTree"),
            p,
        ),
        Kind::UNION_TYPE => visitor.visit_union_type(
            tree.as_union_type_tree()
                .expect("ClassCastException: UnionTypeTree"),
            p,
        ),
        Kind::INTERSECTION_TYPE => visitor.visit_intersection_type(
            tree.as_intersection_type_tree()
                .expect("ClassCastException: IntersectionTypeTree"),
            p,
        ),
        Kind::TYPE_CAST => visitor.visit_type_cast(
            tree.as_type_cast_tree()
                .expect("ClassCastException: TypeCastTree"),
            p,
        ),
        Kind::TYPE_PARAMETER => visitor.visit_type_parameter(
            tree.as_type_parameter_tree()
                .expect("ClassCastException: TypeParameterTree"),
            p,
        ),
        Kind::VARIABLE => visitor.visit_variable(
            tree.as_variable_tree()
                .expect("ClassCastException: VariableTree"),
            p,
        ),
        Kind::WHILE_LOOP => visitor.visit_while_loop(
            tree.as_while_loop_tree()
                .expect("ClassCastException: WhileLoopTree"),
            p,
        ),
        Kind::MODULE => visitor.visit_module(
            tree.as_module_tree()
                .expect("ClassCastException: ModuleTree"),
            p,
        ),
        Kind::EXPORTS => visitor.visit_exports(
            tree.as_exports_tree()
                .expect("ClassCastException: ExportsTree"),
            p,
        ),
        Kind::OPENS => visitor.visit_opens(
            tree.as_opens_tree().expect("ClassCastException: OpensTree"),
            p,
        ),
        Kind::PROVIDES => visitor.visit_provides(
            tree.as_provides_tree()
                .expect("ClassCastException: ProvidesTree"),
            p,
        ),
        Kind::REQUIRES => visitor.visit_requires(
            tree.as_requires_tree()
                .expect("ClassCastException: RequiresTree"),
            p,
        ),
        Kind::USES => visitor.visit_uses(
            tree.as_uses_tree().expect("ClassCastException: UsesTree"),
            p,
        ),
        Kind::YIELD => visitor.visit_yield(
            tree.as_yield_tree().expect("ClassCastException: YieldTree"),
            p,
        ),
        Kind::INTERFACE => visitor.visit_class(
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ENUM => visitor.visit_class(
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ANNOTATION_TYPE => visitor.visit_class(
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::RECORD => visitor.visit_class(
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ERRONEOUS => visitor.visit_erroneous(
            tree.as_erroneous_tree()
                .expect("ClassCastException: ErroneousTree"),
            p,
        ),
        Kind::POSTFIX_INCREMENT => visitor.visit_unary(
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::POSTFIX_DECREMENT => visitor.visit_unary(
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::PREFIX_INCREMENT => visitor.visit_unary(
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::PREFIX_DECREMENT => visitor.visit_unary(
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::UNARY_PLUS => visitor.visit_unary(
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::UNARY_MINUS => visitor.visit_unary(
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::BITWISE_COMPLEMENT => visitor.visit_unary(
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::LOGICAL_COMPLEMENT => visitor.visit_unary(
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::MULTIPLY => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::DIVIDE => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::REMAINDER => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::PLUS => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::MINUS => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LEFT_SHIFT => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::RIGHT_SHIFT => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::UNSIGNED_RIGHT_SHIFT => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LESS_THAN => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::GREATER_THAN => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LESS_THAN_EQUAL => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::GREATER_THAN_EQUAL => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::EQUAL_TO => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::NOT_EQUAL_TO => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::AND => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::XOR => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::OR => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::CONDITIONAL_AND => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::CONDITIONAL_OR => visitor.visit_binary(
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::MULTIPLY_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::DIVIDE_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::REMAINDER_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::PLUS_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::MINUS_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::LEFT_SHIFT_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::RIGHT_SHIFT_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::UNSIGNED_RIGHT_SHIFT_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::AND_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::XOR_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::OR_ASSIGNMENT => visitor.visit_compound_assignment(
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::INT_LITERAL => visitor.visit_literal(
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::LONG_LITERAL => visitor.visit_literal(
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::FLOAT_LITERAL => visitor.visit_literal(
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::DOUBLE_LITERAL => visitor.visit_literal(
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::BOOLEAN_LITERAL => visitor.visit_literal(
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::CHAR_LITERAL => visitor.visit_literal(
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::STRING_LITERAL => visitor.visit_literal(
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::NULL_LITERAL => visitor.visit_literal(
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::UNBOUNDED_WILDCARD => visitor.visit_wildcard(
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::EXTENDS_WILDCARD => visitor.visit_wildcard(
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::SUPER_WILDCARD => visitor.visit_wildcard(
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::OTHER => visitor.visit_other(tree, p),
    }
}

/// PORTING ADAPTER: the TreeScanner form of the double dispatch.  The default
/// visit methods live on the TreeScanner trait (the Java TreeScanner class), so
/// this variant calls the scanner methods instead of the TreeVisitor interface
/// methods (Rust cannot override another trait's default methods).  The
/// Kind-to-visit-method mapping is the same as dispatch_tree.
pub(crate) fn dispatch_tree_scanner<R, P: Clone, V: TreeScanner<R, P> + ?Sized>(
    scanner: &mut V,
    tree: &dyn Tree,
    p: P,
) -> Option<R> {
    match tree.get_kind() {
        Kind::ANNOTATED_TYPE => <V as TreeScanner<R, P>>::visit_annotated_type(
            scanner,
            tree.as_annotated_type_tree()
                .expect("ClassCastException: AnnotatedTypeTree"),
            p,
        ),
        Kind::ANNOTATION => <V as TreeScanner<R, P>>::visit_annotation(
            scanner,
            tree.as_annotation_tree()
                .expect("ClassCastException: AnnotationTree"),
            p,
        ),
        Kind::TYPE_ANNOTATION => <V as TreeScanner<R, P>>::visit_annotation(
            scanner,
            tree.as_annotation_tree()
                .expect("ClassCastException: AnnotationTree"),
            p,
        ),
        Kind::ARRAY_ACCESS => <V as TreeScanner<R, P>>::visit_array_access(
            scanner,
            tree.as_array_access_tree()
                .expect("ClassCastException: ArrayAccessTree"),
            p,
        ),
        Kind::ARRAY_TYPE => <V as TreeScanner<R, P>>::visit_array_type(
            scanner,
            tree.as_array_type_tree()
                .expect("ClassCastException: ArrayTypeTree"),
            p,
        ),
        Kind::ASSERT => <V as TreeScanner<R, P>>::visit_assert(
            scanner,
            tree.as_assert_tree()
                .expect("ClassCastException: AssertTree"),
            p,
        ),
        Kind::ASSIGNMENT => <V as TreeScanner<R, P>>::visit_assignment(
            scanner,
            tree.as_assignment_tree()
                .expect("ClassCastException: AssignmentTree"),
            p,
        ),
        Kind::BLOCK => <V as TreeScanner<R, P>>::visit_block(
            scanner,
            tree.as_block_tree().expect("ClassCastException: BlockTree"),
            p,
        ),
        Kind::BREAK => <V as TreeScanner<R, P>>::visit_break(
            scanner,
            tree.as_break_tree().expect("ClassCastException: BreakTree"),
            p,
        ),
        Kind::CASE => <V as TreeScanner<R, P>>::visit_case(
            scanner,
            tree.as_case_tree().expect("ClassCastException: CaseTree"),
            p,
        ),
        Kind::CATCH => <V as TreeScanner<R, P>>::visit_catch(
            scanner,
            tree.as_catch_tree().expect("ClassCastException: CatchTree"),
            p,
        ),
        Kind::CLASS => <V as TreeScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::COMPILATION_UNIT => <V as TreeScanner<R, P>>::visit_compilation_unit(
            scanner,
            tree.as_compilation_unit_tree()
                .expect("ClassCastException: CompilationUnitTree"),
            p,
        ),
        Kind::CONDITIONAL_EXPRESSION => <V as TreeScanner<R, P>>::visit_conditional_expression(
            scanner,
            tree.as_conditional_expression_tree()
                .expect("ClassCastException: ConditionalExpressionTree"),
            p,
        ),
        Kind::CONTINUE => <V as TreeScanner<R, P>>::visit_continue(
            scanner,
            tree.as_continue_tree()
                .expect("ClassCastException: ContinueTree"),
            p,
        ),
        Kind::DO_WHILE_LOOP => <V as TreeScanner<R, P>>::visit_do_while_loop(
            scanner,
            tree.as_do_while_loop_tree()
                .expect("ClassCastException: DoWhileLoopTree"),
            p,
        ),
        Kind::ENHANCED_FOR_LOOP => <V as TreeScanner<R, P>>::visit_enhanced_for_loop(
            scanner,
            tree.as_enhanced_for_loop_tree()
                .expect("ClassCastException: EnhancedForLoopTree"),
            p,
        ),
        Kind::EXPRESSION_STATEMENT => <V as TreeScanner<R, P>>::visit_expression_statement(
            scanner,
            tree.as_expression_statement_tree()
                .expect("ClassCastException: ExpressionStatementTree"),
            p,
        ),
        Kind::MEMBER_SELECT => <V as TreeScanner<R, P>>::visit_member_select(
            scanner,
            tree.as_member_select_tree()
                .expect("ClassCastException: MemberSelectTree"),
            p,
        ),
        Kind::MEMBER_REFERENCE => <V as TreeScanner<R, P>>::visit_member_reference(
            scanner,
            tree.as_member_reference_tree()
                .expect("ClassCastException: MemberReferenceTree"),
            p,
        ),
        Kind::FOR_LOOP => <V as TreeScanner<R, P>>::visit_for_loop(
            scanner,
            tree.as_for_loop_tree()
                .expect("ClassCastException: ForLoopTree"),
            p,
        ),
        Kind::IDENTIFIER => <V as TreeScanner<R, P>>::visit_identifier(
            scanner,
            tree.as_identifier_tree()
                .expect("ClassCastException: IdentifierTree"),
            p,
        ),
        Kind::IF => <V as TreeScanner<R, P>>::visit_if(
            scanner,
            tree.as_if_tree().expect("ClassCastException: IfTree"),
            p,
        ),
        Kind::IMPORT => <V as TreeScanner<R, P>>::visit_import(
            scanner,
            tree.as_import_tree()
                .expect("ClassCastException: ImportTree"),
            p,
        ),
        Kind::INSTANCE_OF => <V as TreeScanner<R, P>>::visit_instance_of(
            scanner,
            tree.as_instance_of_tree()
                .expect("ClassCastException: InstanceOfTree"),
            p,
        ),
        Kind::LABELED_STATEMENT => <V as TreeScanner<R, P>>::visit_labeled_statement(
            scanner,
            tree.as_labeled_statement_tree()
                .expect("ClassCastException: LabeledStatementTree"),
            p,
        ),
        Kind::METHOD => <V as TreeScanner<R, P>>::visit_method(
            scanner,
            tree.as_method_tree()
                .expect("ClassCastException: MethodTree"),
            p,
        ),
        Kind::METHOD_INVOCATION => <V as TreeScanner<R, P>>::visit_method_invocation(
            scanner,
            tree.as_method_invocation_tree()
                .expect("ClassCastException: MethodInvocationTree"),
            p,
        ),
        Kind::MODIFIERS => <V as TreeScanner<R, P>>::visit_modifiers(
            scanner,
            tree.as_modifiers_tree()
                .expect("ClassCastException: ModifiersTree"),
            p,
        ),
        Kind::NEW_ARRAY => <V as TreeScanner<R, P>>::visit_new_array(
            scanner,
            tree.as_new_array_tree()
                .expect("ClassCastException: NewArrayTree"),
            p,
        ),
        Kind::NEW_CLASS => <V as TreeScanner<R, P>>::visit_new_class(
            scanner,
            tree.as_new_class_tree()
                .expect("ClassCastException: NewClassTree"),
            p,
        ),
        Kind::LAMBDA_EXPRESSION => <V as TreeScanner<R, P>>::visit_lambda_expression(
            scanner,
            tree.as_lambda_expression_tree()
                .expect("ClassCastException: LambdaExpressionTree"),
            p,
        ),
        Kind::PACKAGE => <V as TreeScanner<R, P>>::visit_package(
            scanner,
            tree.as_package_tree()
                .expect("ClassCastException: PackageTree"),
            p,
        ),
        Kind::PARENTHESIZED => <V as TreeScanner<R, P>>::visit_parenthesized(
            scanner,
            tree.as_parenthesized_tree()
                .expect("ClassCastException: ParenthesizedTree"),
            p,
        ),
        Kind::BINDING_PATTERN => <V as TreeScanner<R, P>>::visit_binding_pattern(
            scanner,
            tree.as_binding_pattern_tree()
                .expect("ClassCastException: BindingPatternTree"),
            p,
        ),
        Kind::GUARDED_PATTERN => <V as TreeScanner<R, P>>::visit_guarded_pattern(
            scanner,
            tree.as_guarded_pattern_tree()
                .expect("ClassCastException: GuardedPatternTree"),
            p,
        ),
        Kind::PARENTHESIZED_PATTERN => <V as TreeScanner<R, P>>::visit_parenthesized_pattern(
            scanner,
            tree.as_parenthesized_pattern_tree()
                .expect("ClassCastException: ParenthesizedPatternTree"),
            p,
        ),
        Kind::DEFAULT_CASE_LABEL => <V as TreeScanner<R, P>>::visit_default_case_label(
            scanner,
            tree.as_default_case_label_tree()
                .expect("ClassCastException: DefaultCaseLabelTree"),
            p,
        ),
        Kind::PRIMITIVE_TYPE => <V as TreeScanner<R, P>>::visit_primitive_type(
            scanner,
            tree.as_primitive_type_tree()
                .expect("ClassCastException: PrimitiveTypeTree"),
            p,
        ),
        Kind::RETURN => <V as TreeScanner<R, P>>::visit_return(
            scanner,
            tree.as_return_tree()
                .expect("ClassCastException: ReturnTree"),
            p,
        ),
        Kind::EMPTY_STATEMENT => <V as TreeScanner<R, P>>::visit_empty_statement(
            scanner,
            tree.as_empty_statement_tree()
                .expect("ClassCastException: EmptyStatementTree"),
            p,
        ),
        Kind::SWITCH => <V as TreeScanner<R, P>>::visit_switch(
            scanner,
            tree.as_switch_tree()
                .expect("ClassCastException: SwitchTree"),
            p,
        ),
        Kind::SWITCH_EXPRESSION => <V as TreeScanner<R, P>>::visit_switch_expression(
            scanner,
            tree.as_switch_expression_tree()
                .expect("ClassCastException: SwitchExpressionTree"),
            p,
        ),
        Kind::SYNCHRONIZED => <V as TreeScanner<R, P>>::visit_synchronized(
            scanner,
            tree.as_synchronized_tree()
                .expect("ClassCastException: SynchronizedTree"),
            p,
        ),
        Kind::THROW => <V as TreeScanner<R, P>>::visit_throw(
            scanner,
            tree.as_throw_tree().expect("ClassCastException: ThrowTree"),
            p,
        ),
        Kind::TRY => <V as TreeScanner<R, P>>::visit_try(
            scanner,
            tree.as_try_tree().expect("ClassCastException: TryTree"),
            p,
        ),
        Kind::PARAMETERIZED_TYPE => <V as TreeScanner<R, P>>::visit_parameterized_type(
            scanner,
            tree.as_parameterized_type_tree()
                .expect("ClassCastException: ParameterizedTypeTree"),
            p,
        ),
        Kind::UNION_TYPE => <V as TreeScanner<R, P>>::visit_union_type(
            scanner,
            tree.as_union_type_tree()
                .expect("ClassCastException: UnionTypeTree"),
            p,
        ),
        Kind::INTERSECTION_TYPE => <V as TreeScanner<R, P>>::visit_intersection_type(
            scanner,
            tree.as_intersection_type_tree()
                .expect("ClassCastException: IntersectionTypeTree"),
            p,
        ),
        Kind::TYPE_CAST => <V as TreeScanner<R, P>>::visit_type_cast(
            scanner,
            tree.as_type_cast_tree()
                .expect("ClassCastException: TypeCastTree"),
            p,
        ),
        Kind::TYPE_PARAMETER => <V as TreeScanner<R, P>>::visit_type_parameter(
            scanner,
            tree.as_type_parameter_tree()
                .expect("ClassCastException: TypeParameterTree"),
            p,
        ),
        Kind::VARIABLE => <V as TreeScanner<R, P>>::visit_variable(
            scanner,
            tree.as_variable_tree()
                .expect("ClassCastException: VariableTree"),
            p,
        ),
        Kind::WHILE_LOOP => <V as TreeScanner<R, P>>::visit_while_loop(
            scanner,
            tree.as_while_loop_tree()
                .expect("ClassCastException: WhileLoopTree"),
            p,
        ),
        Kind::MODULE => <V as TreeScanner<R, P>>::visit_module(
            scanner,
            tree.as_module_tree()
                .expect("ClassCastException: ModuleTree"),
            p,
        ),
        Kind::EXPORTS => <V as TreeScanner<R, P>>::visit_exports(
            scanner,
            tree.as_exports_tree()
                .expect("ClassCastException: ExportsTree"),
            p,
        ),
        Kind::OPENS => <V as TreeScanner<R, P>>::visit_opens(
            scanner,
            tree.as_opens_tree().expect("ClassCastException: OpensTree"),
            p,
        ),
        Kind::PROVIDES => <V as TreeScanner<R, P>>::visit_provides(
            scanner,
            tree.as_provides_tree()
                .expect("ClassCastException: ProvidesTree"),
            p,
        ),
        Kind::REQUIRES => <V as TreeScanner<R, P>>::visit_requires(
            scanner,
            tree.as_requires_tree()
                .expect("ClassCastException: RequiresTree"),
            p,
        ),
        Kind::USES => <V as TreeScanner<R, P>>::visit_uses(
            scanner,
            tree.as_uses_tree().expect("ClassCastException: UsesTree"),
            p,
        ),
        Kind::YIELD => <V as TreeScanner<R, P>>::visit_yield(
            scanner,
            tree.as_yield_tree().expect("ClassCastException: YieldTree"),
            p,
        ),
        Kind::INTERFACE => <V as TreeScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ENUM => <V as TreeScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ANNOTATION_TYPE => <V as TreeScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::RECORD => <V as TreeScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ERRONEOUS => <V as TreeScanner<R, P>>::visit_erroneous(
            scanner,
            tree.as_erroneous_tree()
                .expect("ClassCastException: ErroneousTree"),
            p,
        ),
        Kind::POSTFIX_INCREMENT => <V as TreeScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::POSTFIX_DECREMENT => <V as TreeScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::PREFIX_INCREMENT => <V as TreeScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::PREFIX_DECREMENT => <V as TreeScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::UNARY_PLUS => <V as TreeScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::UNARY_MINUS => <V as TreeScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::BITWISE_COMPLEMENT => <V as TreeScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::LOGICAL_COMPLEMENT => <V as TreeScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::MULTIPLY => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::DIVIDE => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::REMAINDER => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::PLUS => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::MINUS => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LEFT_SHIFT => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::RIGHT_SHIFT => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::UNSIGNED_RIGHT_SHIFT => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LESS_THAN => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::GREATER_THAN => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LESS_THAN_EQUAL => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::GREATER_THAN_EQUAL => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::EQUAL_TO => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::NOT_EQUAL_TO => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::AND => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::XOR => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::OR => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::CONDITIONAL_AND => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::CONDITIONAL_OR => <V as TreeScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::MULTIPLY_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::DIVIDE_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::REMAINDER_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::PLUS_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::MINUS_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::LEFT_SHIFT_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::RIGHT_SHIFT_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::UNSIGNED_RIGHT_SHIFT_ASSIGNMENT => {
            <V as TreeScanner<R, P>>::visit_compound_assignment(
                scanner,
                tree.as_compound_assignment_tree()
                    .expect("ClassCastException: CompoundAssignmentTree"),
                p,
            )
        }
        Kind::AND_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::XOR_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::OR_ASSIGNMENT => <V as TreeScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::INT_LITERAL => <V as TreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::LONG_LITERAL => <V as TreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::FLOAT_LITERAL => <V as TreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::DOUBLE_LITERAL => <V as TreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::BOOLEAN_LITERAL => <V as TreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::CHAR_LITERAL => <V as TreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::STRING_LITERAL => <V as TreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::NULL_LITERAL => <V as TreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::UNBOUNDED_WILDCARD => <V as TreeScanner<R, P>>::visit_wildcard(
            scanner,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::EXTENDS_WILDCARD => <V as TreeScanner<R, P>>::visit_wildcard(
            scanner,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::SUPER_WILDCARD => <V as TreeScanner<R, P>>::visit_wildcard(
            scanner,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::OTHER => <V as TreeScanner<R, P>>::visit_other(scanner, tree, p),
    }
}

pub(crate) fn dispatch_tree_path_scanner<R, P: Clone, V: TreePathScanner<R, P> + ?Sized>(
    scanner: &mut V,
    tree: &dyn Tree,
    p: P,
) -> Option<R> {
    match tree.get_kind() {
        Kind::ANNOTATED_TYPE => <V as TreePathScanner<R, P>>::visit_annotated_type(
            scanner,
            tree.as_annotated_type_tree()
                .expect("ClassCastException: AnnotatedTypeTree"),
            p,
        ),
        Kind::ANNOTATION => <V as TreePathScanner<R, P>>::visit_annotation(
            scanner,
            tree.as_annotation_tree()
                .expect("ClassCastException: AnnotationTree"),
            p,
        ),
        Kind::TYPE_ANNOTATION => <V as TreePathScanner<R, P>>::visit_annotation(
            scanner,
            tree.as_annotation_tree()
                .expect("ClassCastException: AnnotationTree"),
            p,
        ),
        Kind::ARRAY_ACCESS => <V as TreePathScanner<R, P>>::visit_array_access(
            scanner,
            tree.as_array_access_tree()
                .expect("ClassCastException: ArrayAccessTree"),
            p,
        ),
        Kind::ARRAY_TYPE => <V as TreePathScanner<R, P>>::visit_array_type(
            scanner,
            tree.as_array_type_tree()
                .expect("ClassCastException: ArrayTypeTree"),
            p,
        ),
        Kind::ASSERT => <V as TreePathScanner<R, P>>::visit_assert(
            scanner,
            tree.as_assert_tree()
                .expect("ClassCastException: AssertTree"),
            p,
        ),
        Kind::ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_assignment(
            scanner,
            tree.as_assignment_tree()
                .expect("ClassCastException: AssignmentTree"),
            p,
        ),
        Kind::BLOCK => <V as TreePathScanner<R, P>>::visit_block(
            scanner,
            tree.as_block_tree().expect("ClassCastException: BlockTree"),
            p,
        ),
        Kind::BREAK => <V as TreePathScanner<R, P>>::visit_break(
            scanner,
            tree.as_break_tree().expect("ClassCastException: BreakTree"),
            p,
        ),
        Kind::CASE => <V as TreePathScanner<R, P>>::visit_case(
            scanner,
            tree.as_case_tree().expect("ClassCastException: CaseTree"),
            p,
        ),
        Kind::CATCH => <V as TreePathScanner<R, P>>::visit_catch(
            scanner,
            tree.as_catch_tree().expect("ClassCastException: CatchTree"),
            p,
        ),
        Kind::CLASS => <V as TreePathScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::COMPILATION_UNIT => <V as TreePathScanner<R, P>>::visit_compilation_unit(
            scanner,
            tree.as_compilation_unit_tree()
                .expect("ClassCastException: CompilationUnitTree"),
            p,
        ),
        Kind::CONDITIONAL_EXPRESSION => <V as TreePathScanner<R, P>>::visit_conditional_expression(
            scanner,
            tree.as_conditional_expression_tree()
                .expect("ClassCastException: ConditionalExpressionTree"),
            p,
        ),
        Kind::CONTINUE => <V as TreePathScanner<R, P>>::visit_continue(
            scanner,
            tree.as_continue_tree()
                .expect("ClassCastException: ContinueTree"),
            p,
        ),
        Kind::DO_WHILE_LOOP => <V as TreePathScanner<R, P>>::visit_do_while_loop(
            scanner,
            tree.as_do_while_loop_tree()
                .expect("ClassCastException: DoWhileLoopTree"),
            p,
        ),
        Kind::ENHANCED_FOR_LOOP => <V as TreePathScanner<R, P>>::visit_enhanced_for_loop(
            scanner,
            tree.as_enhanced_for_loop_tree()
                .expect("ClassCastException: EnhancedForLoopTree"),
            p,
        ),
        Kind::EXPRESSION_STATEMENT => <V as TreePathScanner<R, P>>::visit_expression_statement(
            scanner,
            tree.as_expression_statement_tree()
                .expect("ClassCastException: ExpressionStatementTree"),
            p,
        ),
        Kind::MEMBER_SELECT => <V as TreePathScanner<R, P>>::visit_member_select(
            scanner,
            tree.as_member_select_tree()
                .expect("ClassCastException: MemberSelectTree"),
            p,
        ),
        Kind::MEMBER_REFERENCE => <V as TreePathScanner<R, P>>::visit_member_reference(
            scanner,
            tree.as_member_reference_tree()
                .expect("ClassCastException: MemberReferenceTree"),
            p,
        ),
        Kind::FOR_LOOP => <V as TreePathScanner<R, P>>::visit_for_loop(
            scanner,
            tree.as_for_loop_tree()
                .expect("ClassCastException: ForLoopTree"),
            p,
        ),
        Kind::IDENTIFIER => <V as TreePathScanner<R, P>>::visit_identifier(
            scanner,
            tree.as_identifier_tree()
                .expect("ClassCastException: IdentifierTree"),
            p,
        ),
        Kind::IF => <V as TreePathScanner<R, P>>::visit_if(
            scanner,
            tree.as_if_tree().expect("ClassCastException: IfTree"),
            p,
        ),
        Kind::IMPORT => <V as TreePathScanner<R, P>>::visit_import(
            scanner,
            tree.as_import_tree()
                .expect("ClassCastException: ImportTree"),
            p,
        ),
        Kind::INSTANCE_OF => <V as TreePathScanner<R, P>>::visit_instance_of(
            scanner,
            tree.as_instance_of_tree()
                .expect("ClassCastException: InstanceOfTree"),
            p,
        ),
        Kind::LABELED_STATEMENT => <V as TreePathScanner<R, P>>::visit_labeled_statement(
            scanner,
            tree.as_labeled_statement_tree()
                .expect("ClassCastException: LabeledStatementTree"),
            p,
        ),
        Kind::METHOD => <V as TreePathScanner<R, P>>::visit_method(
            scanner,
            tree.as_method_tree()
                .expect("ClassCastException: MethodTree"),
            p,
        ),
        Kind::METHOD_INVOCATION => <V as TreePathScanner<R, P>>::visit_method_invocation(
            scanner,
            tree.as_method_invocation_tree()
                .expect("ClassCastException: MethodInvocationTree"),
            p,
        ),
        Kind::MODIFIERS => <V as TreePathScanner<R, P>>::visit_modifiers(
            scanner,
            tree.as_modifiers_tree()
                .expect("ClassCastException: ModifiersTree"),
            p,
        ),
        Kind::NEW_ARRAY => <V as TreePathScanner<R, P>>::visit_new_array(
            scanner,
            tree.as_new_array_tree()
                .expect("ClassCastException: NewArrayTree"),
            p,
        ),
        Kind::NEW_CLASS => <V as TreePathScanner<R, P>>::visit_new_class(
            scanner,
            tree.as_new_class_tree()
                .expect("ClassCastException: NewClassTree"),
            p,
        ),
        Kind::LAMBDA_EXPRESSION => <V as TreePathScanner<R, P>>::visit_lambda_expression(
            scanner,
            tree.as_lambda_expression_tree()
                .expect("ClassCastException: LambdaExpressionTree"),
            p,
        ),
        Kind::PACKAGE => <V as TreePathScanner<R, P>>::visit_package(
            scanner,
            tree.as_package_tree()
                .expect("ClassCastException: PackageTree"),
            p,
        ),
        Kind::PARENTHESIZED => <V as TreePathScanner<R, P>>::visit_parenthesized(
            scanner,
            tree.as_parenthesized_tree()
                .expect("ClassCastException: ParenthesizedTree"),
            p,
        ),
        Kind::BINDING_PATTERN => <V as TreePathScanner<R, P>>::visit_binding_pattern(
            scanner,
            tree.as_binding_pattern_tree()
                .expect("ClassCastException: BindingPatternTree"),
            p,
        ),
        Kind::GUARDED_PATTERN => <V as TreePathScanner<R, P>>::visit_guarded_pattern(
            scanner,
            tree.as_guarded_pattern_tree()
                .expect("ClassCastException: GuardedPatternTree"),
            p,
        ),
        Kind::PARENTHESIZED_PATTERN => <V as TreePathScanner<R, P>>::visit_parenthesized_pattern(
            scanner,
            tree.as_parenthesized_pattern_tree()
                .expect("ClassCastException: ParenthesizedPatternTree"),
            p,
        ),
        Kind::DEFAULT_CASE_LABEL => <V as TreePathScanner<R, P>>::visit_default_case_label(
            scanner,
            tree.as_default_case_label_tree()
                .expect("ClassCastException: DefaultCaseLabelTree"),
            p,
        ),
        Kind::PRIMITIVE_TYPE => <V as TreePathScanner<R, P>>::visit_primitive_type(
            scanner,
            tree.as_primitive_type_tree()
                .expect("ClassCastException: PrimitiveTypeTree"),
            p,
        ),
        Kind::RETURN => <V as TreePathScanner<R, P>>::visit_return(
            scanner,
            tree.as_return_tree()
                .expect("ClassCastException: ReturnTree"),
            p,
        ),
        Kind::EMPTY_STATEMENT => <V as TreePathScanner<R, P>>::visit_empty_statement(
            scanner,
            tree.as_empty_statement_tree()
                .expect("ClassCastException: EmptyStatementTree"),
            p,
        ),
        Kind::SWITCH => <V as TreePathScanner<R, P>>::visit_switch(
            scanner,
            tree.as_switch_tree()
                .expect("ClassCastException: SwitchTree"),
            p,
        ),
        Kind::SWITCH_EXPRESSION => <V as TreePathScanner<R, P>>::visit_switch_expression(
            scanner,
            tree.as_switch_expression_tree()
                .expect("ClassCastException: SwitchExpressionTree"),
            p,
        ),
        Kind::SYNCHRONIZED => <V as TreePathScanner<R, P>>::visit_synchronized(
            scanner,
            tree.as_synchronized_tree()
                .expect("ClassCastException: SynchronizedTree"),
            p,
        ),
        Kind::THROW => <V as TreePathScanner<R, P>>::visit_throw(
            scanner,
            tree.as_throw_tree().expect("ClassCastException: ThrowTree"),
            p,
        ),
        Kind::TRY => <V as TreePathScanner<R, P>>::visit_try(
            scanner,
            tree.as_try_tree().expect("ClassCastException: TryTree"),
            p,
        ),
        Kind::PARAMETERIZED_TYPE => <V as TreePathScanner<R, P>>::visit_parameterized_type(
            scanner,
            tree.as_parameterized_type_tree()
                .expect("ClassCastException: ParameterizedTypeTree"),
            p,
        ),
        Kind::UNION_TYPE => <V as TreePathScanner<R, P>>::visit_union_type(
            scanner,
            tree.as_union_type_tree()
                .expect("ClassCastException: UnionTypeTree"),
            p,
        ),
        Kind::INTERSECTION_TYPE => <V as TreePathScanner<R, P>>::visit_intersection_type(
            scanner,
            tree.as_intersection_type_tree()
                .expect("ClassCastException: IntersectionTypeTree"),
            p,
        ),
        Kind::TYPE_CAST => <V as TreePathScanner<R, P>>::visit_type_cast(
            scanner,
            tree.as_type_cast_tree()
                .expect("ClassCastException: TypeCastTree"),
            p,
        ),
        Kind::TYPE_PARAMETER => <V as TreePathScanner<R, P>>::visit_type_parameter(
            scanner,
            tree.as_type_parameter_tree()
                .expect("ClassCastException: TypeParameterTree"),
            p,
        ),
        Kind::VARIABLE => <V as TreePathScanner<R, P>>::visit_variable(
            scanner,
            tree.as_variable_tree()
                .expect("ClassCastException: VariableTree"),
            p,
        ),
        Kind::WHILE_LOOP => <V as TreePathScanner<R, P>>::visit_while_loop(
            scanner,
            tree.as_while_loop_tree()
                .expect("ClassCastException: WhileLoopTree"),
            p,
        ),
        Kind::MODULE => <V as TreePathScanner<R, P>>::visit_module(
            scanner,
            tree.as_module_tree()
                .expect("ClassCastException: ModuleTree"),
            p,
        ),
        Kind::EXPORTS => <V as TreePathScanner<R, P>>::visit_exports(
            scanner,
            tree.as_exports_tree()
                .expect("ClassCastException: ExportsTree"),
            p,
        ),
        Kind::OPENS => <V as TreePathScanner<R, P>>::visit_opens(
            scanner,
            tree.as_opens_tree().expect("ClassCastException: OpensTree"),
            p,
        ),
        Kind::PROVIDES => <V as TreePathScanner<R, P>>::visit_provides(
            scanner,
            tree.as_provides_tree()
                .expect("ClassCastException: ProvidesTree"),
            p,
        ),
        Kind::REQUIRES => <V as TreePathScanner<R, P>>::visit_requires(
            scanner,
            tree.as_requires_tree()
                .expect("ClassCastException: RequiresTree"),
            p,
        ),
        Kind::USES => <V as TreePathScanner<R, P>>::visit_uses(
            scanner,
            tree.as_uses_tree().expect("ClassCastException: UsesTree"),
            p,
        ),
        Kind::YIELD => <V as TreePathScanner<R, P>>::visit_yield(
            scanner,
            tree.as_yield_tree().expect("ClassCastException: YieldTree"),
            p,
        ),
        Kind::INTERFACE => <V as TreePathScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ENUM => <V as TreePathScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ANNOTATION_TYPE => <V as TreePathScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::RECORD => <V as TreePathScanner<R, P>>::visit_class(
            scanner,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ERRONEOUS => <V as TreePathScanner<R, P>>::visit_erroneous(
            scanner,
            tree.as_erroneous_tree()
                .expect("ClassCastException: ErroneousTree"),
            p,
        ),
        Kind::POSTFIX_INCREMENT => <V as TreePathScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::POSTFIX_DECREMENT => <V as TreePathScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::PREFIX_INCREMENT => <V as TreePathScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::PREFIX_DECREMENT => <V as TreePathScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::UNARY_PLUS => <V as TreePathScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::UNARY_MINUS => <V as TreePathScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::BITWISE_COMPLEMENT => <V as TreePathScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::LOGICAL_COMPLEMENT => <V as TreePathScanner<R, P>>::visit_unary(
            scanner,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::MULTIPLY => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::DIVIDE => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::REMAINDER => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::PLUS => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::MINUS => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LEFT_SHIFT => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::RIGHT_SHIFT => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::UNSIGNED_RIGHT_SHIFT => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LESS_THAN => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::GREATER_THAN => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LESS_THAN_EQUAL => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::GREATER_THAN_EQUAL => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::EQUAL_TO => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::NOT_EQUAL_TO => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::AND => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::XOR => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::OR => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::CONDITIONAL_AND => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::CONDITIONAL_OR => <V as TreePathScanner<R, P>>::visit_binary(
            scanner,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::MULTIPLY_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::DIVIDE_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::REMAINDER_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::PLUS_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::MINUS_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::LEFT_SHIFT_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::RIGHT_SHIFT_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::UNSIGNED_RIGHT_SHIFT_ASSIGNMENT => {
            <V as TreePathScanner<R, P>>::visit_compound_assignment(
                scanner,
                tree.as_compound_assignment_tree()
                    .expect("ClassCastException: CompoundAssignmentTree"),
                p,
            )
        }
        Kind::AND_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::XOR_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::OR_ASSIGNMENT => <V as TreePathScanner<R, P>>::visit_compound_assignment(
            scanner,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::INT_LITERAL => <V as TreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::LONG_LITERAL => <V as TreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::FLOAT_LITERAL => <V as TreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::DOUBLE_LITERAL => <V as TreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::BOOLEAN_LITERAL => <V as TreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::CHAR_LITERAL => <V as TreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::STRING_LITERAL => <V as TreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::NULL_LITERAL => <V as TreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::UNBOUNDED_WILDCARD => <V as TreePathScanner<R, P>>::visit_wildcard(
            scanner,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::EXTENDS_WILDCARD => <V as TreePathScanner<R, P>>::visit_wildcard(
            scanner,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::SUPER_WILDCARD => <V as TreePathScanner<R, P>>::visit_wildcard(
            scanner,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::OTHER => <V as TreePathScanner<R, P>>::visit_other(scanner, tree, p),
    }
}

/// PORTING ADAPTER: the SimpleTreeVisitor form of the double dispatch (the
/// default visit methods live on the SimpleTreeVisitor trait; Rust cannot
/// override another trait's default methods, so this variant calls the
/// SimpleTreeVisitor methods).  The Kind-to-visit-method mapping is the same as
/// dispatch_tree.
pub(crate) fn dispatch_simple_tree_visitor<
    R: Clone,
    P: Clone,
    V: SimpleTreeVisitor<R, P> + ?Sized,
>(
    visitor: &mut V,
    tree: &dyn Tree,
    p: P,
) -> R {
    match tree.get_kind() {
        Kind::ANNOTATED_TYPE => <V as SimpleTreeVisitor<R, P>>::visit_annotated_type(
            visitor,
            tree.as_annotated_type_tree()
                .expect("ClassCastException: AnnotatedTypeTree"),
            p,
        ),
        Kind::ANNOTATION => <V as SimpleTreeVisitor<R, P>>::visit_annotation(
            visitor,
            tree.as_annotation_tree()
                .expect("ClassCastException: AnnotationTree"),
            p,
        ),
        Kind::TYPE_ANNOTATION => <V as SimpleTreeVisitor<R, P>>::visit_annotation(
            visitor,
            tree.as_annotation_tree()
                .expect("ClassCastException: AnnotationTree"),
            p,
        ),
        Kind::ARRAY_ACCESS => <V as SimpleTreeVisitor<R, P>>::visit_array_access(
            visitor,
            tree.as_array_access_tree()
                .expect("ClassCastException: ArrayAccessTree"),
            p,
        ),
        Kind::ARRAY_TYPE => <V as SimpleTreeVisitor<R, P>>::visit_array_type(
            visitor,
            tree.as_array_type_tree()
                .expect("ClassCastException: ArrayTypeTree"),
            p,
        ),
        Kind::ASSERT => <V as SimpleTreeVisitor<R, P>>::visit_assert(
            visitor,
            tree.as_assert_tree()
                .expect("ClassCastException: AssertTree"),
            p,
        ),
        Kind::ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_assignment(
            visitor,
            tree.as_assignment_tree()
                .expect("ClassCastException: AssignmentTree"),
            p,
        ),
        Kind::BLOCK => <V as SimpleTreeVisitor<R, P>>::visit_block(
            visitor,
            tree.as_block_tree().expect("ClassCastException: BlockTree"),
            p,
        ),
        Kind::BREAK => <V as SimpleTreeVisitor<R, P>>::visit_break(
            visitor,
            tree.as_break_tree().expect("ClassCastException: BreakTree"),
            p,
        ),
        Kind::CASE => <V as SimpleTreeVisitor<R, P>>::visit_case(
            visitor,
            tree.as_case_tree().expect("ClassCastException: CaseTree"),
            p,
        ),
        Kind::CATCH => <V as SimpleTreeVisitor<R, P>>::visit_catch(
            visitor,
            tree.as_catch_tree().expect("ClassCastException: CatchTree"),
            p,
        ),
        Kind::CLASS => <V as SimpleTreeVisitor<R, P>>::visit_class(
            visitor,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::COMPILATION_UNIT => <V as SimpleTreeVisitor<R, P>>::visit_compilation_unit(
            visitor,
            tree.as_compilation_unit_tree()
                .expect("ClassCastException: CompilationUnitTree"),
            p,
        ),
        Kind::CONDITIONAL_EXPRESSION => {
            <V as SimpleTreeVisitor<R, P>>::visit_conditional_expression(
                visitor,
                tree.as_conditional_expression_tree()
                    .expect("ClassCastException: ConditionalExpressionTree"),
                p,
            )
        }
        Kind::CONTINUE => <V as SimpleTreeVisitor<R, P>>::visit_continue(
            visitor,
            tree.as_continue_tree()
                .expect("ClassCastException: ContinueTree"),
            p,
        ),
        Kind::DO_WHILE_LOOP => <V as SimpleTreeVisitor<R, P>>::visit_do_while_loop(
            visitor,
            tree.as_do_while_loop_tree()
                .expect("ClassCastException: DoWhileLoopTree"),
            p,
        ),
        Kind::ENHANCED_FOR_LOOP => <V as SimpleTreeVisitor<R, P>>::visit_enhanced_for_loop(
            visitor,
            tree.as_enhanced_for_loop_tree()
                .expect("ClassCastException: EnhancedForLoopTree"),
            p,
        ),
        Kind::EXPRESSION_STATEMENT => <V as SimpleTreeVisitor<R, P>>::visit_expression_statement(
            visitor,
            tree.as_expression_statement_tree()
                .expect("ClassCastException: ExpressionStatementTree"),
            p,
        ),
        Kind::MEMBER_SELECT => <V as SimpleTreeVisitor<R, P>>::visit_member_select(
            visitor,
            tree.as_member_select_tree()
                .expect("ClassCastException: MemberSelectTree"),
            p,
        ),
        Kind::MEMBER_REFERENCE => <V as SimpleTreeVisitor<R, P>>::visit_member_reference(
            visitor,
            tree.as_member_reference_tree()
                .expect("ClassCastException: MemberReferenceTree"),
            p,
        ),
        Kind::FOR_LOOP => <V as SimpleTreeVisitor<R, P>>::visit_for_loop(
            visitor,
            tree.as_for_loop_tree()
                .expect("ClassCastException: ForLoopTree"),
            p,
        ),
        Kind::IDENTIFIER => <V as SimpleTreeVisitor<R, P>>::visit_identifier(
            visitor,
            tree.as_identifier_tree()
                .expect("ClassCastException: IdentifierTree"),
            p,
        ),
        Kind::IF => <V as SimpleTreeVisitor<R, P>>::visit_if(
            visitor,
            tree.as_if_tree().expect("ClassCastException: IfTree"),
            p,
        ),
        Kind::IMPORT => <V as SimpleTreeVisitor<R, P>>::visit_import(
            visitor,
            tree.as_import_tree()
                .expect("ClassCastException: ImportTree"),
            p,
        ),
        Kind::INSTANCE_OF => <V as SimpleTreeVisitor<R, P>>::visit_instance_of(
            visitor,
            tree.as_instance_of_tree()
                .expect("ClassCastException: InstanceOfTree"),
            p,
        ),
        Kind::LABELED_STATEMENT => <V as SimpleTreeVisitor<R, P>>::visit_labeled_statement(
            visitor,
            tree.as_labeled_statement_tree()
                .expect("ClassCastException: LabeledStatementTree"),
            p,
        ),
        Kind::METHOD => <V as SimpleTreeVisitor<R, P>>::visit_method(
            visitor,
            tree.as_method_tree()
                .expect("ClassCastException: MethodTree"),
            p,
        ),
        Kind::METHOD_INVOCATION => <V as SimpleTreeVisitor<R, P>>::visit_method_invocation(
            visitor,
            tree.as_method_invocation_tree()
                .expect("ClassCastException: MethodInvocationTree"),
            p,
        ),
        Kind::MODIFIERS => <V as SimpleTreeVisitor<R, P>>::visit_modifiers(
            visitor,
            tree.as_modifiers_tree()
                .expect("ClassCastException: ModifiersTree"),
            p,
        ),
        Kind::NEW_ARRAY => <V as SimpleTreeVisitor<R, P>>::visit_new_array(
            visitor,
            tree.as_new_array_tree()
                .expect("ClassCastException: NewArrayTree"),
            p,
        ),
        Kind::NEW_CLASS => <V as SimpleTreeVisitor<R, P>>::visit_new_class(
            visitor,
            tree.as_new_class_tree()
                .expect("ClassCastException: NewClassTree"),
            p,
        ),
        Kind::LAMBDA_EXPRESSION => <V as SimpleTreeVisitor<R, P>>::visit_lambda_expression(
            visitor,
            tree.as_lambda_expression_tree()
                .expect("ClassCastException: LambdaExpressionTree"),
            p,
        ),
        Kind::PACKAGE => <V as SimpleTreeVisitor<R, P>>::visit_package(
            visitor,
            tree.as_package_tree()
                .expect("ClassCastException: PackageTree"),
            p,
        ),
        Kind::PARENTHESIZED => <V as SimpleTreeVisitor<R, P>>::visit_parenthesized(
            visitor,
            tree.as_parenthesized_tree()
                .expect("ClassCastException: ParenthesizedTree"),
            p,
        ),
        Kind::BINDING_PATTERN => <V as SimpleTreeVisitor<R, P>>::visit_binding_pattern(
            visitor,
            tree.as_binding_pattern_tree()
                .expect("ClassCastException: BindingPatternTree"),
            p,
        ),
        Kind::GUARDED_PATTERN => <V as SimpleTreeVisitor<R, P>>::visit_guarded_pattern(
            visitor,
            tree.as_guarded_pattern_tree()
                .expect("ClassCastException: GuardedPatternTree"),
            p,
        ),
        Kind::PARENTHESIZED_PATTERN => <V as SimpleTreeVisitor<R, P>>::visit_parenthesized_pattern(
            visitor,
            tree.as_parenthesized_pattern_tree()
                .expect("ClassCastException: ParenthesizedPatternTree"),
            p,
        ),
        Kind::DEFAULT_CASE_LABEL => <V as SimpleTreeVisitor<R, P>>::visit_default_case_label(
            visitor,
            tree.as_default_case_label_tree()
                .expect("ClassCastException: DefaultCaseLabelTree"),
            p,
        ),
        Kind::PRIMITIVE_TYPE => <V as SimpleTreeVisitor<R, P>>::visit_primitive_type(
            visitor,
            tree.as_primitive_type_tree()
                .expect("ClassCastException: PrimitiveTypeTree"),
            p,
        ),
        Kind::RETURN => <V as SimpleTreeVisitor<R, P>>::visit_return(
            visitor,
            tree.as_return_tree()
                .expect("ClassCastException: ReturnTree"),
            p,
        ),
        Kind::EMPTY_STATEMENT => <V as SimpleTreeVisitor<R, P>>::visit_empty_statement(
            visitor,
            tree.as_empty_statement_tree()
                .expect("ClassCastException: EmptyStatementTree"),
            p,
        ),
        Kind::SWITCH => <V as SimpleTreeVisitor<R, P>>::visit_switch(
            visitor,
            tree.as_switch_tree()
                .expect("ClassCastException: SwitchTree"),
            p,
        ),
        Kind::SWITCH_EXPRESSION => <V as SimpleTreeVisitor<R, P>>::visit_switch_expression(
            visitor,
            tree.as_switch_expression_tree()
                .expect("ClassCastException: SwitchExpressionTree"),
            p,
        ),
        Kind::SYNCHRONIZED => <V as SimpleTreeVisitor<R, P>>::visit_synchronized(
            visitor,
            tree.as_synchronized_tree()
                .expect("ClassCastException: SynchronizedTree"),
            p,
        ),
        Kind::THROW => <V as SimpleTreeVisitor<R, P>>::visit_throw(
            visitor,
            tree.as_throw_tree().expect("ClassCastException: ThrowTree"),
            p,
        ),
        Kind::TRY => <V as SimpleTreeVisitor<R, P>>::visit_try(
            visitor,
            tree.as_try_tree().expect("ClassCastException: TryTree"),
            p,
        ),
        Kind::PARAMETERIZED_TYPE => <V as SimpleTreeVisitor<R, P>>::visit_parameterized_type(
            visitor,
            tree.as_parameterized_type_tree()
                .expect("ClassCastException: ParameterizedTypeTree"),
            p,
        ),
        Kind::UNION_TYPE => <V as SimpleTreeVisitor<R, P>>::visit_union_type(
            visitor,
            tree.as_union_type_tree()
                .expect("ClassCastException: UnionTypeTree"),
            p,
        ),
        Kind::INTERSECTION_TYPE => <V as SimpleTreeVisitor<R, P>>::visit_intersection_type(
            visitor,
            tree.as_intersection_type_tree()
                .expect("ClassCastException: IntersectionTypeTree"),
            p,
        ),
        Kind::TYPE_CAST => <V as SimpleTreeVisitor<R, P>>::visit_type_cast(
            visitor,
            tree.as_type_cast_tree()
                .expect("ClassCastException: TypeCastTree"),
            p,
        ),
        Kind::TYPE_PARAMETER => <V as SimpleTreeVisitor<R, P>>::visit_type_parameter(
            visitor,
            tree.as_type_parameter_tree()
                .expect("ClassCastException: TypeParameterTree"),
            p,
        ),
        Kind::VARIABLE => <V as SimpleTreeVisitor<R, P>>::visit_variable(
            visitor,
            tree.as_variable_tree()
                .expect("ClassCastException: VariableTree"),
            p,
        ),
        Kind::WHILE_LOOP => <V as SimpleTreeVisitor<R, P>>::visit_while_loop(
            visitor,
            tree.as_while_loop_tree()
                .expect("ClassCastException: WhileLoopTree"),
            p,
        ),
        Kind::MODULE => <V as SimpleTreeVisitor<R, P>>::visit_module(
            visitor,
            tree.as_module_tree()
                .expect("ClassCastException: ModuleTree"),
            p,
        ),
        Kind::EXPORTS => <V as SimpleTreeVisitor<R, P>>::visit_exports(
            visitor,
            tree.as_exports_tree()
                .expect("ClassCastException: ExportsTree"),
            p,
        ),
        Kind::OPENS => <V as SimpleTreeVisitor<R, P>>::visit_opens(
            visitor,
            tree.as_opens_tree().expect("ClassCastException: OpensTree"),
            p,
        ),
        Kind::PROVIDES => <V as SimpleTreeVisitor<R, P>>::visit_provides(
            visitor,
            tree.as_provides_tree()
                .expect("ClassCastException: ProvidesTree"),
            p,
        ),
        Kind::REQUIRES => <V as SimpleTreeVisitor<R, P>>::visit_requires(
            visitor,
            tree.as_requires_tree()
                .expect("ClassCastException: RequiresTree"),
            p,
        ),
        Kind::USES => <V as SimpleTreeVisitor<R, P>>::visit_uses(
            visitor,
            tree.as_uses_tree().expect("ClassCastException: UsesTree"),
            p,
        ),
        Kind::YIELD => <V as SimpleTreeVisitor<R, P>>::visit_yield(
            visitor,
            tree.as_yield_tree().expect("ClassCastException: YieldTree"),
            p,
        ),
        Kind::INTERFACE => <V as SimpleTreeVisitor<R, P>>::visit_class(
            visitor,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ENUM => <V as SimpleTreeVisitor<R, P>>::visit_class(
            visitor,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ANNOTATION_TYPE => <V as SimpleTreeVisitor<R, P>>::visit_class(
            visitor,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::RECORD => <V as SimpleTreeVisitor<R, P>>::visit_class(
            visitor,
            tree.as_class_tree().expect("ClassCastException: ClassTree"),
            p,
        ),
        Kind::ERRONEOUS => <V as SimpleTreeVisitor<R, P>>::visit_erroneous(
            visitor,
            tree.as_erroneous_tree()
                .expect("ClassCastException: ErroneousTree"),
            p,
        ),
        Kind::POSTFIX_INCREMENT => <V as SimpleTreeVisitor<R, P>>::visit_unary(
            visitor,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::POSTFIX_DECREMENT => <V as SimpleTreeVisitor<R, P>>::visit_unary(
            visitor,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::PREFIX_INCREMENT => <V as SimpleTreeVisitor<R, P>>::visit_unary(
            visitor,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::PREFIX_DECREMENT => <V as SimpleTreeVisitor<R, P>>::visit_unary(
            visitor,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::UNARY_PLUS => <V as SimpleTreeVisitor<R, P>>::visit_unary(
            visitor,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::UNARY_MINUS => <V as SimpleTreeVisitor<R, P>>::visit_unary(
            visitor,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::BITWISE_COMPLEMENT => <V as SimpleTreeVisitor<R, P>>::visit_unary(
            visitor,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::LOGICAL_COMPLEMENT => <V as SimpleTreeVisitor<R, P>>::visit_unary(
            visitor,
            tree.as_unary_tree().expect("ClassCastException: UnaryTree"),
            p,
        ),
        Kind::MULTIPLY => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::DIVIDE => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::REMAINDER => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::PLUS => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::MINUS => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LEFT_SHIFT => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::RIGHT_SHIFT => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::UNSIGNED_RIGHT_SHIFT => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LESS_THAN => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::GREATER_THAN => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::LESS_THAN_EQUAL => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::GREATER_THAN_EQUAL => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::EQUAL_TO => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::NOT_EQUAL_TO => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::AND => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::XOR => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::OR => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::CONDITIONAL_AND => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::CONDITIONAL_OR => <V as SimpleTreeVisitor<R, P>>::visit_binary(
            visitor,
            tree.as_binary_tree()
                .expect("ClassCastException: BinaryTree"),
            p,
        ),
        Kind::MULTIPLY_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::DIVIDE_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::REMAINDER_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::PLUS_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::MINUS_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::LEFT_SHIFT_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::RIGHT_SHIFT_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::UNSIGNED_RIGHT_SHIFT_ASSIGNMENT => {
            <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
                visitor,
                tree.as_compound_assignment_tree()
                    .expect("ClassCastException: CompoundAssignmentTree"),
                p,
            )
        }
        Kind::AND_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::XOR_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::OR_ASSIGNMENT => <V as SimpleTreeVisitor<R, P>>::visit_compound_assignment(
            visitor,
            tree.as_compound_assignment_tree()
                .expect("ClassCastException: CompoundAssignmentTree"),
            p,
        ),
        Kind::INT_LITERAL => <V as SimpleTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::LONG_LITERAL => <V as SimpleTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::FLOAT_LITERAL => <V as SimpleTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::DOUBLE_LITERAL => <V as SimpleTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::BOOLEAN_LITERAL => <V as SimpleTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::CHAR_LITERAL => <V as SimpleTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::STRING_LITERAL => <V as SimpleTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::NULL_LITERAL => <V as SimpleTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        Kind::UNBOUNDED_WILDCARD => <V as SimpleTreeVisitor<R, P>>::visit_wildcard(
            visitor,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::EXTENDS_WILDCARD => <V as SimpleTreeVisitor<R, P>>::visit_wildcard(
            visitor,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::SUPER_WILDCARD => <V as SimpleTreeVisitor<R, P>>::visit_wildcard(
            visitor,
            tree.as_wildcard_tree()
                .expect("ClassCastException: WildcardTree"),
            p,
        ),
        Kind::OTHER => <V as SimpleTreeVisitor<R, P>>::visit_other(visitor, tree, p),
    }
}

/// PORTING ADAPTER: the DocTreeScanner form of the doc-tree double dispatch
/// (DocTree.accept is generic).  Same Kind-to-visit-method mapping as the
/// DocTreeVisitor interface.
pub(crate) fn dispatch_doc_tree_scanner<R, P: Clone, V: DocTreeScanner<R, P> + ?Sized>(
    scanner: &mut V,
    tree: &dyn DocTree,
    p: P,
) -> Option<R> {
    use crate::doctree::doc_tree::Kind as DocKind;
    match tree.get_kind() {
        DocKind::ATTRIBUTE => <V as DocTreeScanner<R, P>>::visit_attribute(
            scanner,
            tree.as_attribute_tree()
                .expect("ClassCastException: AttributeTree"),
            p,
        ),
        DocKind::AUTHOR => <V as DocTreeScanner<R, P>>::visit_author(
            scanner,
            tree.as_author_tree()
                .expect("ClassCastException: AuthorTree"),
            p,
        ),
        DocKind::CODE => <V as DocTreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        DocKind::COMMENT => <V as DocTreeScanner<R, P>>::visit_comment(
            scanner,
            tree.as_comment_tree()
                .expect("ClassCastException: CommentTree"),
            p,
        ),
        DocKind::DEPRECATED => <V as DocTreeScanner<R, P>>::visit_deprecated(
            scanner,
            tree.as_deprecated_tree()
                .expect("ClassCastException: DeprecatedTree"),
            p,
        ),
        DocKind::DOC_COMMENT => <V as DocTreeScanner<R, P>>::visit_doc_comment(
            scanner,
            tree.as_doc_comment_tree()
                .expect("ClassCastException: DocCommentTree"),
            p,
        ),
        DocKind::DOC_ROOT => <V as DocTreeScanner<R, P>>::visit_doc_root(
            scanner,
            tree.as_doc_root_tree()
                .expect("ClassCastException: DocRootTree"),
            p,
        ),
        DocKind::DOC_TYPE => <V as DocTreeScanner<R, P>>::visit_doc_type(
            scanner,
            tree.as_doc_type_tree()
                .expect("ClassCastException: DocTypeTree"),
            p,
        ),
        DocKind::END_ELEMENT => <V as DocTreeScanner<R, P>>::visit_end_element(
            scanner,
            tree.as_end_element_tree()
                .expect("ClassCastException: EndElementTree"),
            p,
        ),
        DocKind::ENTITY => <V as DocTreeScanner<R, P>>::visit_entity(
            scanner,
            tree.as_entity_tree()
                .expect("ClassCastException: EntityTree"),
            p,
        ),
        DocKind::ERRONEOUS => <V as DocTreeScanner<R, P>>::visit_erroneous(
            scanner,
            tree.as_erroneous_tree()
                .expect("ClassCastException: ErroneousTree"),
            p,
        ),
        DocKind::EXCEPTION => <V as DocTreeScanner<R, P>>::visit_throws(
            scanner,
            tree.as_throws_tree()
                .expect("ClassCastException: ThrowsTree"),
            p,
        ),
        DocKind::HIDDEN => <V as DocTreeScanner<R, P>>::visit_hidden(
            scanner,
            tree.as_hidden_tree()
                .expect("ClassCastException: HiddenTree"),
            p,
        ),
        DocKind::IDENTIFIER => <V as DocTreeScanner<R, P>>::visit_identifier(
            scanner,
            tree.as_identifier_tree()
                .expect("ClassCastException: IdentifierTree"),
            p,
        ),
        DocKind::INDEX => <V as DocTreeScanner<R, P>>::visit_index(
            scanner,
            tree.as_index_tree().expect("ClassCastException: IndexTree"),
            p,
        ),
        DocKind::INHERIT_DOC => <V as DocTreeScanner<R, P>>::visit_inherit_doc(
            scanner,
            tree.as_inherit_doc_tree()
                .expect("ClassCastException: InheritDocTree"),
            p,
        ),
        DocKind::LINK => <V as DocTreeScanner<R, P>>::visit_link(
            scanner,
            tree.as_link_tree().expect("ClassCastException: LinkTree"),
            p,
        ),
        DocKind::LINK_PLAIN => <V as DocTreeScanner<R, P>>::visit_link(
            scanner,
            tree.as_link_tree().expect("ClassCastException: LinkTree"),
            p,
        ),
        DocKind::LITERAL => <V as DocTreeScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        DocKind::PARAM => <V as DocTreeScanner<R, P>>::visit_param(
            scanner,
            tree.as_param_tree().expect("ClassCastException: ParamTree"),
            p,
        ),
        DocKind::PROVIDES => <V as DocTreeScanner<R, P>>::visit_provides(
            scanner,
            tree.as_provides_tree()
                .expect("ClassCastException: ProvidesTree"),
            p,
        ),
        DocKind::REFERENCE => <V as DocTreeScanner<R, P>>::visit_reference(
            scanner,
            tree.as_reference_tree()
                .expect("ClassCastException: ReferenceTree"),
            p,
        ),
        DocKind::RETURN => <V as DocTreeScanner<R, P>>::visit_return(
            scanner,
            tree.as_return_tree()
                .expect("ClassCastException: ReturnTree"),
            p,
        ),
        DocKind::SEE => <V as DocTreeScanner<R, P>>::visit_see(
            scanner,
            tree.as_see_tree().expect("ClassCastException: SeeTree"),
            p,
        ),
        DocKind::SERIAL => <V as DocTreeScanner<R, P>>::visit_serial(
            scanner,
            tree.as_serial_tree()
                .expect("ClassCastException: SerialTree"),
            p,
        ),
        DocKind::SERIAL_DATA => <V as DocTreeScanner<R, P>>::visit_serial_data(
            scanner,
            tree.as_serial_data_tree()
                .expect("ClassCastException: SerialDataTree"),
            p,
        ),
        DocKind::SERIAL_FIELD => <V as DocTreeScanner<R, P>>::visit_serial_field(
            scanner,
            tree.as_serial_field_tree()
                .expect("ClassCastException: SerialFieldTree"),
            p,
        ),
        DocKind::SINCE => <V as DocTreeScanner<R, P>>::visit_since(
            scanner,
            tree.as_since_tree().expect("ClassCastException: SinceTree"),
            p,
        ),
        DocKind::START_ELEMENT => <V as DocTreeScanner<R, P>>::visit_end_element(
            scanner,
            tree.as_end_element_tree()
                .expect("ClassCastException: EndElementTree"),
            p,
        ),
        DocKind::SYSTEM_PROPERTY => <V as DocTreeScanner<R, P>>::visit_system_property(
            scanner,
            tree.as_system_property_tree()
                .expect("ClassCastException: SystemPropertyTree"),
            p,
        ),
        DocKind::SUMMARY => <V as DocTreeScanner<R, P>>::visit_summary(
            scanner,
            tree.as_summary_tree()
                .expect("ClassCastException: SummaryTree"),
            p,
        ),
        DocKind::TEXT => <V as DocTreeScanner<R, P>>::visit_text(
            scanner,
            tree.as_text_tree().expect("ClassCastException: TextTree"),
            p,
        ),
        DocKind::THROWS => <V as DocTreeScanner<R, P>>::visit_throws(
            scanner,
            tree.as_throws_tree()
                .expect("ClassCastException: ThrowsTree"),
            p,
        ),
        DocKind::UNKNOWN_BLOCK_TAG => <V as DocTreeScanner<R, P>>::visit_unknown_block_tag(
            scanner,
            tree.as_unknown_block_tag_tree()
                .expect("ClassCastException: UnknownBlockTagTree"),
            p,
        ),
        DocKind::UNKNOWN_INLINE_TAG => <V as DocTreeScanner<R, P>>::visit_unknown_inline_tag(
            scanner,
            tree.as_unknown_inline_tag_tree()
                .expect("ClassCastException: UnknownInlineTagTree"),
            p,
        ),
        DocKind::USES => <V as DocTreeScanner<R, P>>::visit_uses(
            scanner,
            tree.as_uses_tree().expect("ClassCastException: UsesTree"),
            p,
        ),
        DocKind::VALUE => <V as DocTreeScanner<R, P>>::visit_value(
            scanner,
            tree.as_value_tree().expect("ClassCastException: ValueTree"),
            p,
        ),
        DocKind::VERSION => <V as DocTreeScanner<R, P>>::visit_version(
            scanner,
            tree.as_version_tree()
                .expect("ClassCastException: VersionTree"),
            p,
        ),
        DocKind::OTHER => <V as DocTreeScanner<R, P>>::visit_other(scanner, tree, p),
    }
}

/// PORTING ADAPTER: the SimpleDocTreeVisitor form of the doc-tree double dispatch.
pub(crate) fn dispatch_simple_doc_tree<
    R: Clone,
    P: Clone,
    V: SimpleDocTreeVisitor<R, P> + ?Sized,
>(
    visitor: &mut V,
    tree: &dyn DocTree,
    p: P,
) -> R {
    use crate::doctree::doc_tree::Kind as DocKind;
    match tree.get_kind() {
        DocKind::ATTRIBUTE => <V as SimpleDocTreeVisitor<R, P>>::visit_attribute(
            visitor,
            tree.as_attribute_tree()
                .expect("ClassCastException: AttributeTree"),
            p,
        ),
        DocKind::AUTHOR => <V as SimpleDocTreeVisitor<R, P>>::visit_author(
            visitor,
            tree.as_author_tree()
                .expect("ClassCastException: AuthorTree"),
            p,
        ),
        DocKind::CODE => <V as SimpleDocTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        DocKind::COMMENT => <V as SimpleDocTreeVisitor<R, P>>::visit_comment(
            visitor,
            tree.as_comment_tree()
                .expect("ClassCastException: CommentTree"),
            p,
        ),
        DocKind::DEPRECATED => <V as SimpleDocTreeVisitor<R, P>>::visit_deprecated(
            visitor,
            tree.as_deprecated_tree()
                .expect("ClassCastException: DeprecatedTree"),
            p,
        ),
        DocKind::DOC_COMMENT => <V as SimpleDocTreeVisitor<R, P>>::visit_doc_comment(
            visitor,
            tree.as_doc_comment_tree()
                .expect("ClassCastException: DocCommentTree"),
            p,
        ),
        DocKind::DOC_ROOT => <V as SimpleDocTreeVisitor<R, P>>::visit_doc_root(
            visitor,
            tree.as_doc_root_tree()
                .expect("ClassCastException: DocRootTree"),
            p,
        ),
        DocKind::DOC_TYPE => <V as SimpleDocTreeVisitor<R, P>>::visit_doc_type(
            visitor,
            tree.as_doc_type_tree()
                .expect("ClassCastException: DocTypeTree"),
            p,
        ),
        DocKind::END_ELEMENT => <V as SimpleDocTreeVisitor<R, P>>::visit_end_element(
            visitor,
            tree.as_end_element_tree()
                .expect("ClassCastException: EndElementTree"),
            p,
        ),
        DocKind::ENTITY => <V as SimpleDocTreeVisitor<R, P>>::visit_entity(
            visitor,
            tree.as_entity_tree()
                .expect("ClassCastException: EntityTree"),
            p,
        ),
        DocKind::ERRONEOUS => <V as SimpleDocTreeVisitor<R, P>>::visit_erroneous(
            visitor,
            tree.as_erroneous_tree()
                .expect("ClassCastException: ErroneousTree"),
            p,
        ),
        DocKind::EXCEPTION => <V as SimpleDocTreeVisitor<R, P>>::visit_throws(
            visitor,
            tree.as_throws_tree()
                .expect("ClassCastException: ThrowsTree"),
            p,
        ),
        DocKind::HIDDEN => <V as SimpleDocTreeVisitor<R, P>>::visit_hidden(
            visitor,
            tree.as_hidden_tree()
                .expect("ClassCastException: HiddenTree"),
            p,
        ),
        DocKind::IDENTIFIER => <V as SimpleDocTreeVisitor<R, P>>::visit_identifier(
            visitor,
            tree.as_identifier_tree()
                .expect("ClassCastException: IdentifierTree"),
            p,
        ),
        DocKind::INDEX => <V as SimpleDocTreeVisitor<R, P>>::visit_index(
            visitor,
            tree.as_index_tree().expect("ClassCastException: IndexTree"),
            p,
        ),
        DocKind::INHERIT_DOC => <V as SimpleDocTreeVisitor<R, P>>::visit_inherit_doc(
            visitor,
            tree.as_inherit_doc_tree()
                .expect("ClassCastException: InheritDocTree"),
            p,
        ),
        DocKind::LINK => <V as SimpleDocTreeVisitor<R, P>>::visit_link(
            visitor,
            tree.as_link_tree().expect("ClassCastException: LinkTree"),
            p,
        ),
        DocKind::LINK_PLAIN => <V as SimpleDocTreeVisitor<R, P>>::visit_link(
            visitor,
            tree.as_link_tree().expect("ClassCastException: LinkTree"),
            p,
        ),
        DocKind::LITERAL => <V as SimpleDocTreeVisitor<R, P>>::visit_literal(
            visitor,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        DocKind::PARAM => <V as SimpleDocTreeVisitor<R, P>>::visit_param(
            visitor,
            tree.as_param_tree().expect("ClassCastException: ParamTree"),
            p,
        ),
        DocKind::PROVIDES => <V as SimpleDocTreeVisitor<R, P>>::visit_provides(
            visitor,
            tree.as_provides_tree()
                .expect("ClassCastException: ProvidesTree"),
            p,
        ),
        DocKind::REFERENCE => <V as SimpleDocTreeVisitor<R, P>>::visit_reference(
            visitor,
            tree.as_reference_tree()
                .expect("ClassCastException: ReferenceTree"),
            p,
        ),
        DocKind::RETURN => <V as SimpleDocTreeVisitor<R, P>>::visit_return(
            visitor,
            tree.as_return_tree()
                .expect("ClassCastException: ReturnTree"),
            p,
        ),
        DocKind::SEE => <V as SimpleDocTreeVisitor<R, P>>::visit_see(
            visitor,
            tree.as_see_tree().expect("ClassCastException: SeeTree"),
            p,
        ),
        DocKind::SERIAL => <V as SimpleDocTreeVisitor<R, P>>::visit_serial(
            visitor,
            tree.as_serial_tree()
                .expect("ClassCastException: SerialTree"),
            p,
        ),
        DocKind::SERIAL_DATA => <V as SimpleDocTreeVisitor<R, P>>::visit_serial_data(
            visitor,
            tree.as_serial_data_tree()
                .expect("ClassCastException: SerialDataTree"),
            p,
        ),
        DocKind::SERIAL_FIELD => <V as SimpleDocTreeVisitor<R, P>>::visit_serial_field(
            visitor,
            tree.as_serial_field_tree()
                .expect("ClassCastException: SerialFieldTree"),
            p,
        ),
        DocKind::SINCE => <V as SimpleDocTreeVisitor<R, P>>::visit_since(
            visitor,
            tree.as_since_tree().expect("ClassCastException: SinceTree"),
            p,
        ),
        DocKind::START_ELEMENT => <V as SimpleDocTreeVisitor<R, P>>::visit_end_element(
            visitor,
            tree.as_end_element_tree()
                .expect("ClassCastException: EndElementTree"),
            p,
        ),
        DocKind::SYSTEM_PROPERTY => <V as SimpleDocTreeVisitor<R, P>>::visit_system_property(
            visitor,
            tree.as_system_property_tree()
                .expect("ClassCastException: SystemPropertyTree"),
            p,
        ),
        DocKind::SUMMARY => <V as SimpleDocTreeVisitor<R, P>>::visit_summary(
            visitor,
            tree.as_summary_tree()
                .expect("ClassCastException: SummaryTree"),
            p,
        ),
        DocKind::TEXT => <V as SimpleDocTreeVisitor<R, P>>::visit_text(
            visitor,
            tree.as_text_tree().expect("ClassCastException: TextTree"),
            p,
        ),
        DocKind::THROWS => <V as SimpleDocTreeVisitor<R, P>>::visit_throws(
            visitor,
            tree.as_throws_tree()
                .expect("ClassCastException: ThrowsTree"),
            p,
        ),
        DocKind::UNKNOWN_BLOCK_TAG => <V as SimpleDocTreeVisitor<R, P>>::visit_unknown_block_tag(
            visitor,
            tree.as_unknown_block_tag_tree()
                .expect("ClassCastException: UnknownBlockTagTree"),
            p,
        ),
        DocKind::UNKNOWN_INLINE_TAG => <V as SimpleDocTreeVisitor<R, P>>::visit_unknown_inline_tag(
            visitor,
            tree.as_unknown_inline_tag_tree()
                .expect("ClassCastException: UnknownInlineTagTree"),
            p,
        ),
        DocKind::USES => <V as SimpleDocTreeVisitor<R, P>>::visit_uses(
            visitor,
            tree.as_uses_tree().expect("ClassCastException: UsesTree"),
            p,
        ),
        DocKind::VALUE => <V as SimpleDocTreeVisitor<R, P>>::visit_value(
            visitor,
            tree.as_value_tree().expect("ClassCastException: ValueTree"),
            p,
        ),
        DocKind::VERSION => <V as SimpleDocTreeVisitor<R, P>>::visit_version(
            visitor,
            tree.as_version_tree()
                .expect("ClassCastException: VersionTree"),
            p,
        ),
        DocKind::OTHER => <V as SimpleDocTreeVisitor<R, P>>::visit_other(visitor, tree, p),
    }
}

pub(crate) fn dispatch_doc_tree_path_scanner<R, P: Clone, V: DocTreePathScanner<R, P> + ?Sized>(
    scanner: &mut V,
    tree: &dyn DocTree,
    p: P,
) -> Option<R> {
    use crate::doctree::doc_tree::Kind as DocKind;
    match tree.get_kind() {
        DocKind::ATTRIBUTE => <V as DocTreePathScanner<R, P>>::visit_attribute(
            scanner,
            tree.as_attribute_tree()
                .expect("ClassCastException: AttributeTree"),
            p,
        ),
        DocKind::AUTHOR => <V as DocTreePathScanner<R, P>>::visit_author(
            scanner,
            tree.as_author_tree()
                .expect("ClassCastException: AuthorTree"),
            p,
        ),
        DocKind::CODE => <V as DocTreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        DocKind::COMMENT => <V as DocTreePathScanner<R, P>>::visit_comment(
            scanner,
            tree.as_comment_tree()
                .expect("ClassCastException: CommentTree"),
            p,
        ),
        DocKind::DEPRECATED => <V as DocTreePathScanner<R, P>>::visit_deprecated(
            scanner,
            tree.as_deprecated_tree()
                .expect("ClassCastException: DeprecatedTree"),
            p,
        ),
        DocKind::DOC_COMMENT => <V as DocTreePathScanner<R, P>>::visit_doc_comment(
            scanner,
            tree.as_doc_comment_tree()
                .expect("ClassCastException: DocCommentTree"),
            p,
        ),
        DocKind::DOC_ROOT => <V as DocTreePathScanner<R, P>>::visit_doc_root(
            scanner,
            tree.as_doc_root_tree()
                .expect("ClassCastException: DocRootTree"),
            p,
        ),
        DocKind::DOC_TYPE => <V as DocTreePathScanner<R, P>>::visit_doc_type(
            scanner,
            tree.as_doc_type_tree()
                .expect("ClassCastException: DocTypeTree"),
            p,
        ),
        DocKind::END_ELEMENT => <V as DocTreePathScanner<R, P>>::visit_end_element(
            scanner,
            tree.as_end_element_tree()
                .expect("ClassCastException: EndElementTree"),
            p,
        ),
        DocKind::ENTITY => <V as DocTreePathScanner<R, P>>::visit_entity(
            scanner,
            tree.as_entity_tree()
                .expect("ClassCastException: EntityTree"),
            p,
        ),
        DocKind::ERRONEOUS => <V as DocTreePathScanner<R, P>>::visit_erroneous(
            scanner,
            tree.as_erroneous_tree()
                .expect("ClassCastException: ErroneousTree"),
            p,
        ),
        DocKind::EXCEPTION => <V as DocTreePathScanner<R, P>>::visit_throws(
            scanner,
            tree.as_throws_tree()
                .expect("ClassCastException: ThrowsTree"),
            p,
        ),
        DocKind::HIDDEN => <V as DocTreePathScanner<R, P>>::visit_hidden(
            scanner,
            tree.as_hidden_tree()
                .expect("ClassCastException: HiddenTree"),
            p,
        ),
        DocKind::IDENTIFIER => <V as DocTreePathScanner<R, P>>::visit_identifier(
            scanner,
            tree.as_identifier_tree()
                .expect("ClassCastException: IdentifierTree"),
            p,
        ),
        DocKind::INDEX => <V as DocTreePathScanner<R, P>>::visit_index(
            scanner,
            tree.as_index_tree().expect("ClassCastException: IndexTree"),
            p,
        ),
        DocKind::INHERIT_DOC => <V as DocTreePathScanner<R, P>>::visit_inherit_doc(
            scanner,
            tree.as_inherit_doc_tree()
                .expect("ClassCastException: InheritDocTree"),
            p,
        ),
        DocKind::LINK => <V as DocTreePathScanner<R, P>>::visit_link(
            scanner,
            tree.as_link_tree().expect("ClassCastException: LinkTree"),
            p,
        ),
        DocKind::LINK_PLAIN => <V as DocTreePathScanner<R, P>>::visit_link(
            scanner,
            tree.as_link_tree().expect("ClassCastException: LinkTree"),
            p,
        ),
        DocKind::LITERAL => <V as DocTreePathScanner<R, P>>::visit_literal(
            scanner,
            tree.as_literal_tree()
                .expect("ClassCastException: LiteralTree"),
            p,
        ),
        DocKind::PARAM => <V as DocTreePathScanner<R, P>>::visit_param(
            scanner,
            tree.as_param_tree().expect("ClassCastException: ParamTree"),
            p,
        ),
        DocKind::PROVIDES => <V as DocTreePathScanner<R, P>>::visit_provides(
            scanner,
            tree.as_provides_tree()
                .expect("ClassCastException: ProvidesTree"),
            p,
        ),
        DocKind::REFERENCE => <V as DocTreePathScanner<R, P>>::visit_reference(
            scanner,
            tree.as_reference_tree()
                .expect("ClassCastException: ReferenceTree"),
            p,
        ),
        DocKind::RETURN => <V as DocTreePathScanner<R, P>>::visit_return(
            scanner,
            tree.as_return_tree()
                .expect("ClassCastException: ReturnTree"),
            p,
        ),
        DocKind::SEE => <V as DocTreePathScanner<R, P>>::visit_see(
            scanner,
            tree.as_see_tree().expect("ClassCastException: SeeTree"),
            p,
        ),
        DocKind::SERIAL => <V as DocTreePathScanner<R, P>>::visit_serial(
            scanner,
            tree.as_serial_tree()
                .expect("ClassCastException: SerialTree"),
            p,
        ),
        DocKind::SERIAL_DATA => <V as DocTreePathScanner<R, P>>::visit_serial_data(
            scanner,
            tree.as_serial_data_tree()
                .expect("ClassCastException: SerialDataTree"),
            p,
        ),
        DocKind::SERIAL_FIELD => <V as DocTreePathScanner<R, P>>::visit_serial_field(
            scanner,
            tree.as_serial_field_tree()
                .expect("ClassCastException: SerialFieldTree"),
            p,
        ),
        DocKind::SINCE => <V as DocTreePathScanner<R, P>>::visit_since(
            scanner,
            tree.as_since_tree().expect("ClassCastException: SinceTree"),
            p,
        ),
        DocKind::START_ELEMENT => <V as DocTreePathScanner<R, P>>::visit_end_element(
            scanner,
            tree.as_end_element_tree()
                .expect("ClassCastException: EndElementTree"),
            p,
        ),
        DocKind::SYSTEM_PROPERTY => <V as DocTreePathScanner<R, P>>::visit_system_property(
            scanner,
            tree.as_system_property_tree()
                .expect("ClassCastException: SystemPropertyTree"),
            p,
        ),
        DocKind::SUMMARY => <V as DocTreePathScanner<R, P>>::visit_summary(
            scanner,
            tree.as_summary_tree()
                .expect("ClassCastException: SummaryTree"),
            p,
        ),
        DocKind::TEXT => <V as DocTreePathScanner<R, P>>::visit_text(
            scanner,
            tree.as_text_tree().expect("ClassCastException: TextTree"),
            p,
        ),
        DocKind::THROWS => <V as DocTreePathScanner<R, P>>::visit_throws(
            scanner,
            tree.as_throws_tree()
                .expect("ClassCastException: ThrowsTree"),
            p,
        ),
        DocKind::UNKNOWN_BLOCK_TAG => <V as DocTreePathScanner<R, P>>::visit_unknown_block_tag(
            scanner,
            tree.as_unknown_block_tag_tree()
                .expect("ClassCastException: UnknownBlockTagTree"),
            p,
        ),
        DocKind::UNKNOWN_INLINE_TAG => <V as DocTreePathScanner<R, P>>::visit_unknown_inline_tag(
            scanner,
            tree.as_unknown_inline_tag_tree()
                .expect("ClassCastException: UnknownInlineTagTree"),
            p,
        ),
        DocKind::USES => <V as DocTreePathScanner<R, P>>::visit_uses(
            scanner,
            tree.as_uses_tree().expect("ClassCastException: UsesTree"),
            p,
        ),
        DocKind::VALUE => <V as DocTreePathScanner<R, P>>::visit_value(
            scanner,
            tree.as_value_tree().expect("ClassCastException: ValueTree"),
            p,
        ),
        DocKind::VERSION => <V as DocTreePathScanner<R, P>>::visit_version(
            scanner,
            tree.as_version_tree()
                .expect("ClassCastException: VersionTree"),
            p,
        ),
        DocKind::OTHER => <V as DocTreePathScanner<R, P>>::visit_other(scanner, tree, p),
    }
}

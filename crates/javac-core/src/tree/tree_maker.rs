// Origin: com/sun/tools/javac/tree/TreeMaker.java @ 982d53c (jdk-17.0.19-ga)

//! A factory for JCTree nodes: the upstream TreeMaker keeps the current position and the
//! toplevel tree, and its makeXxx methods allocate JCxxx values; the JCTree.Factory interface
//! is represented by the TreeFactory trait implemented by TreeMaker.
//!
//! <p><b>This is NOT part of any supported API.</b>
//!
//! PORTING_RULES (S3 batch):
//! * JCTree is a value in the CompilerContext tree arena: Java "new JCxxx(); tree.pos = pos"
//!   becomes ctx.alloc_tree(JCTree { pos, ty: None, tag, kind }) (TreeId returned; see ADR 0001).
//! * Java null child references are Option<TreeId>; child lists are List<TreeId>.  A Java
//!   null list argument (e.g. the JCExpression extending of a class) maps to Option too, but
//!   the makeXxx list parameters are kept as List<TreeId> (callers pass List::nil()).
//! * TreeMaker is a mutable shared object in javac (Parser keeps one); the Rust instance is
//!   Rc<RefCell<TreeMaker>> (the interior-mutability whitelist used for Options/Log; registered).
//! * Names is ported (util::names::Names), but the Java Names.instance(Context) adapter is not;
//!   TreeMaker::instance reads an Option<Rc<Names>> from the Context (registered by the
//!   caller when available) and the field stays None until then.  Types and Symtab are not
//!   ported yet (code/types.rs is a placeholder for the Type class, not the Types service;
//!   Symtab::new needs many unfinished services): the upstream TreeMaker fields types/syms are
//!   NOT declared here (TODO(Symtab/Types batch), reported; no substitute values are invented).
//!   Convenience methods that take a Symbol/Type/Attribute (Ident(Symbol), VarDef(VarSymbol),
//!   MethodDef(MethodSymbol), Type(Type), Annotation(Attribute), TypeParam(Symbol), ...) are
//!   therefore TODO panics (unimplemented!) until those services land; the tree-only Factory
//!   surface is complete.
//! * at(DiagnosticPosition) takes Option<&dyn DiagnosticPosition> (Java null means NOPOS);
//!   getStartPosition -> DiagnosticPosition::get_start_position.
//! * JCLambda/conditional/member-reference constructor defaults: the Java constructors set
//!   PolyKind.STANDALONE and ParameterKind.EXPLICIT (JCLambda(..., body) constructor); these
//!   are reproduced.  JCMemberReference keeps its upstream uninitialized-kind limitation:
//!   the Rust ReferenceKind is not optional (TODO, reported) so the reference factories are
//!   not provided yet.

// The upstream Factory signatures are kept 1:1 (MethodDef has 9 arguments, plus the arena
// context); the clippy argument-count thresholds do not apply to a direct port.
#![allow(clippy::too_many_arguments)]

use std::cell::RefCell;
use std::option::Option as StdOption;
use std::rc::Rc;

use javac_source::tree::case_tree::CaseKind;
use javac_source::tree::module_tree::ModuleKind;

use crate::code::bound_kind::BoundKind;
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::tree::jc_tree::{JCTree, LiteralValue, ParameterKind, PolyKind, Tag, TreeKind};
use crate::util::jc_diagnostic::DiagnosticPosition;
use crate::util::list::List;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::position;

/// Java: public class TreeMaker implements JCTree.Factory.
pub struct TreeMaker {
    /// Java: public int pos = Position.NOPOS.
    pub pos: i32,
    /// Java: public JCCompilationUnit toplevel.
    pub toplevel: StdOption<TreeId>,
    /// Java: Names names. (See the file header for the Option adaptation.)
    pub names: StdOption<Rc<Names>>,
    // Java: Types types; Symtab syms; — not declared yet, see the file header (TODO).
}

impl TreeMaker {
    /// Java: protected TreeMaker(Context context).
    pub fn new(names: StdOption<Rc<Names>>) -> TreeMaker {
        TreeMaker {
            pos: position::NOPOS,
            toplevel: None,
            names,
        }
    }

    /// Java: protected TreeMaker(JCCompilationUnit toplevel, Names names, Types types, Symtab syms).
    pub fn new_for_toplevel(toplevel: StdOption<TreeId>, names: StdOption<Rc<Names>>) -> TreeMaker {
        TreeMaker {
            pos: position::FIRSTPOS,
            toplevel,
            names,
        }
    }

    /// Java: public static TreeMaker instance(Context context) — cached in the Context.
    pub fn instance(context: &mut crate::util::context::Context) -> Rc<RefCell<TreeMaker>> {
        if let Some(instance) = context.get::<RefCell<TreeMaker>>() {
            return instance;
        }
        // Java: names = Names.instance(context); Types/Symtab are not ported yet (file header).
        let names = context.get::<Names>();
        let maker = Rc::new(RefCell::new(TreeMaker::new(names)));
        context.put::<RefCell<TreeMaker>>(Rc::clone(&maker));
        maker
    }

    /// Java: public TreeMaker forToplevel(JCCompilationUnit toplevel).
    pub fn for_toplevel(&self, toplevel: StdOption<TreeId>) -> TreeMaker {
        TreeMaker::new_for_toplevel(toplevel, self.names.clone())
    }

    /// Java: public TreeMaker at(int pos).
    pub fn at(&mut self, pos: i32) -> &mut TreeMaker {
        self.pos = pos;
        self
    }

    /// Java: public TreeMaker at(DiagnosticPosition pos).
    ///
    /// PORTING_RULES: Java null -> NOPOS; the Rust parameter is Option<&dyn DiagnosticPosition>.
    pub fn at_diagnostic(&mut self, pos: StdOption<&dyn DiagnosticPosition>) -> &mut TreeMaker {
        self.pos = match pos {
            Some(pos) => pos.get_start_position(),
            None => position::NOPOS,
        };
        self
    }

    /// The shared node allocation: Java "new JCxxx(...); tree.pos = pos; return tree;".
    fn make(&mut self, ctx: &mut CompilerContext, tag: Tag, kind: TreeKind) -> TreeId {
        ctx.alloc_tree(JCTree {
            pos: self.pos,
            ty: None,
            tag,
            kind,
        })
    }

    /// Java: public JCCompilationUnit TopLevel(List<JCTree> defs).
    pub fn top_level(&mut self, ctx: &mut CompilerContext, defs: List<TreeId>) -> TreeId {
        for def in defs.iter() {
            let ok = match ctx.tree(def).tag {
                Tag::CLASSDEF
                | Tag::PACKAGEDEF
                | Tag::IMPORT
                | Tag::MODULEDEF
                | Tag::SKIP
                | Tag::ERRONEOUS => true,
                Tag::EXEC => match &ctx.tree(def).kind {
                    TreeKind::ExpressionStatement { expr: Some(expr) } => {
                        ctx.tree(*expr).tag == Tag::ERRONEOUS
                    }
                    _ => false,
                },
                _ => false,
            };
            assert!(ok, "TopLevel: unexpected node kind");
        }
        let tree = self.make(
            ctx,
            Tag::TOPLEVEL,
            TreeKind::CompilationUnit {
                defs,
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
            },
        );
        self.toplevel = Some(tree);
        tree
    }

    /// Java: Names.empty (the empty name used by AnonymousClassDef).
    fn empty_name(&self) -> Name {
        match &self.names {
            Some(names) => names.from_utf(&[]),
            None => panic!(
                "TODO(Names batch): TreeMaker.names is None (Names.instance adapter not ported)"
            ),
        }
    }
}

/// Java: JCTree.Factory — the node factory interface implemented by TreeMaker.
///
/// PORTING_RULES: Java nested interface JCTree.Factory -> trait TreeFactory (Rust traits cannot
/// be nested); every method takes the CompilerContext arena first, and returns TreeId.  Java
/// overloads are renamed with a suffix (docs/PORTING_RULES.md section 3): class_def (6 args) /
/// class_def_permitting (7), method_def (with recvparam) / method_def_simple (without),
/// var_def / var_def_declared_using_var, try_block / try_with_resources, modifiers_with_annotations,
/// erroneous / erroneous_empty, new_class / new_class_speculative.
#[allow(clippy::too_many_arguments)]
pub trait TreeFactory {
    fn top_level(&mut self, ctx: &mut CompilerContext, defs: List<TreeId>) -> TreeId;
    fn package_decl(
        &mut self,
        ctx: &mut CompilerContext,
        annotations: List<TreeId>,
        pid: TreeId,
    ) -> TreeId;
    fn import(&mut self, ctx: &mut CompilerContext, qualid: TreeId, static_import: bool) -> TreeId;
    fn class_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        typarams: List<TreeId>,
        extending: StdOption<TreeId>,
        implementing: List<TreeId>,
        defs: List<TreeId>,
    ) -> TreeId;
    fn class_def_permitting(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        typarams: List<TreeId>,
        extending: StdOption<TreeId>,
        implementing: List<TreeId>,
        permitting: List<TreeId>,
        defs: List<TreeId>,
    ) -> TreeId;
    fn method_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        restype: StdOption<TreeId>,
        typarams: List<TreeId>,
        recvparam: StdOption<TreeId>,
        params: List<TreeId>,
        thrown: List<TreeId>,
        body: StdOption<TreeId>,
        default_value: StdOption<TreeId>,
    ) -> TreeId;
    fn method_def_simple(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        restype: StdOption<TreeId>,
        typarams: List<TreeId>,
        params: List<TreeId>,
        thrown: List<TreeId>,
        body: StdOption<TreeId>,
        default_value: StdOption<TreeId>,
    ) -> TreeId;
    fn var_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        vartype: StdOption<TreeId>,
        init: StdOption<TreeId>,
    ) -> TreeId;
    fn var_def_declared_using_var(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        vartype: StdOption<TreeId>,
        init: StdOption<TreeId>,
        declared_using_var: bool,
    ) -> TreeId;
    fn skip(&mut self, ctx: &mut CompilerContext) -> TreeId;
    fn block(&mut self, ctx: &mut CompilerContext, flags: i64, stats: List<TreeId>) -> TreeId;
    fn do_loop(&mut self, ctx: &mut CompilerContext, body: TreeId, cond: TreeId) -> TreeId;
    fn while_loop(&mut self, ctx: &mut CompilerContext, cond: TreeId, body: TreeId) -> TreeId;
    fn for_loop(
        &mut self,
        ctx: &mut CompilerContext,
        init: List<TreeId>,
        cond: StdOption<TreeId>,
        step: List<TreeId>,
        body: TreeId,
    ) -> TreeId;
    fn foreach_loop(
        &mut self,
        ctx: &mut CompilerContext,
        var: TreeId,
        expr: TreeId,
        body: TreeId,
    ) -> TreeId;
    fn labelled(&mut self, ctx: &mut CompilerContext, label: Name, body: TreeId) -> TreeId;
    fn switch(
        &mut self,
        ctx: &mut CompilerContext,
        selector: TreeId,
        cases: List<TreeId>,
    ) -> TreeId;
    fn switch_expression(
        &mut self,
        ctx: &mut CompilerContext,
        selector: TreeId,
        cases: List<TreeId>,
    ) -> TreeId;
    fn case(
        &mut self,
        ctx: &mut CompilerContext,
        case_kind: CaseKind,
        labels: List<TreeId>,
        stats: List<TreeId>,
        body: StdOption<TreeId>,
    ) -> TreeId;
    fn synchronized(&mut self, ctx: &mut CompilerContext, lock: TreeId, body: TreeId) -> TreeId;
    fn try_block(
        &mut self,
        ctx: &mut CompilerContext,
        body: TreeId,
        catchers: List<TreeId>,
        finalizer: StdOption<TreeId>,
    ) -> TreeId;
    fn try_with_resources(
        &mut self,
        ctx: &mut CompilerContext,
        resources: List<TreeId>,
        body: TreeId,
        catchers: List<TreeId>,
        finalizer: StdOption<TreeId>,
    ) -> TreeId;
    fn catch(&mut self, ctx: &mut CompilerContext, param: TreeId, body: TreeId) -> TreeId;
    fn conditional(
        &mut self,
        ctx: &mut CompilerContext,
        cond: TreeId,
        thenpart: TreeId,
        elsepart: TreeId,
    ) -> TreeId;
    fn if_(
        &mut self,
        ctx: &mut CompilerContext,
        cond: TreeId,
        thenpart: TreeId,
        elsepart: StdOption<TreeId>,
    ) -> TreeId;
    fn exec(&mut self, ctx: &mut CompilerContext, expr: TreeId) -> TreeId;
    fn break_(&mut self, ctx: &mut CompilerContext, label: StdOption<Name>) -> TreeId;
    fn yield_(&mut self, ctx: &mut CompilerContext, value: TreeId) -> TreeId;
    fn continue_(&mut self, ctx: &mut CompilerContext, label: StdOption<Name>) -> TreeId;
    fn return_(&mut self, ctx: &mut CompilerContext, expr: StdOption<TreeId>) -> TreeId;
    fn throw_(&mut self, ctx: &mut CompilerContext, expr: TreeId) -> TreeId;
    fn assert_(
        &mut self,
        ctx: &mut CompilerContext,
        cond: TreeId,
        detail: StdOption<TreeId>,
    ) -> TreeId;
    fn apply(
        &mut self,
        ctx: &mut CompilerContext,
        typeargs: List<TreeId>,
        fn_: TreeId,
        args: List<TreeId>,
    ) -> TreeId;
    fn new_class(
        &mut self,
        ctx: &mut CompilerContext,
        encl: StdOption<TreeId>,
        typeargs: List<TreeId>,
        clazz: TreeId,
        args: List<TreeId>,
        def: StdOption<TreeId>,
    ) -> TreeId;
    fn new_class_speculative(
        &mut self,
        ctx: &mut CompilerContext,
        encl: StdOption<TreeId>,
        typeargs: List<TreeId>,
        clazz: TreeId,
        args: List<TreeId>,
        def: StdOption<TreeId>,
        class_def_removed: bool,
    ) -> TreeId;
    fn new_array(
        &mut self,
        ctx: &mut CompilerContext,
        elemtype: StdOption<TreeId>,
        dims: List<TreeId>,
        elems: StdOption<List<TreeId>>,
    ) -> TreeId;
    fn parens(&mut self, ctx: &mut CompilerContext, expr: TreeId) -> TreeId;
    fn assign(&mut self, ctx: &mut CompilerContext, lhs: TreeId, rhs: TreeId) -> TreeId;
    fn assignop(
        &mut self,
        ctx: &mut CompilerContext,
        opcode: Tag,
        lhs: TreeId,
        rhs: TreeId,
    ) -> TreeId;
    fn unary(&mut self, ctx: &mut CompilerContext, opcode: Tag, arg: TreeId) -> TreeId;
    fn binary(
        &mut self,
        ctx: &mut CompilerContext,
        opcode: Tag,
        lhs: TreeId,
        rhs: TreeId,
    ) -> TreeId;
    fn type_cast(&mut self, ctx: &mut CompilerContext, expr: TreeId, type_: TreeId) -> TreeId;
    fn type_test(&mut self, ctx: &mut CompilerContext, expr: TreeId, clazz: TreeId) -> TreeId;
    fn binding_pattern(&mut self, ctx: &mut CompilerContext, var: TreeId) -> TreeId;
    fn default_case_label(&mut self, ctx: &mut CompilerContext) -> TreeId;
    fn parenthesized_pattern(&mut self, ctx: &mut CompilerContext, pattern: TreeId) -> TreeId;
    fn guard_pattern(&mut self, ctx: &mut CompilerContext, patt: TreeId, expr: TreeId) -> TreeId;
    fn indexed(&mut self, ctx: &mut CompilerContext, indexed: TreeId, index: TreeId) -> TreeId;
    fn select(&mut self, ctx: &mut CompilerContext, selected: TreeId, selector: Name) -> TreeId;
    fn ident(&mut self, ctx: &mut CompilerContext, name: Name) -> TreeId;
    fn literal(
        &mut self,
        ctx: &mut CompilerContext,
        typetag: TypeTag,
        value: StdOption<LiteralValue>,
    ) -> TreeId;
    fn type_ident(&mut self, ctx: &mut CompilerContext, typetag: TypeTag) -> TreeId;
    fn type_array(&mut self, ctx: &mut CompilerContext, elemtype: TreeId) -> TreeId;
    fn type_apply(
        &mut self,
        ctx: &mut CompilerContext,
        clazz: TreeId,
        arguments: List<TreeId>,
    ) -> TreeId;
    fn type_parameter(
        &mut self,
        ctx: &mut CompilerContext,
        name: Name,
        bounds: List<TreeId>,
    ) -> TreeId;
    fn wildcard(
        &mut self,
        ctx: &mut CompilerContext,
        kind: TreeId,
        type_: StdOption<TreeId>,
    ) -> TreeId;
    fn type_bound_kind(&mut self, ctx: &mut CompilerContext, kind: BoundKind) -> TreeId;
    fn annotation(
        &mut self,
        ctx: &mut CompilerContext,
        annotation_type: TreeId,
        args: List<TreeId>,
    ) -> TreeId;
    fn type_annotation(
        &mut self,
        ctx: &mut CompilerContext,
        annotation_type: TreeId,
        args: List<TreeId>,
    ) -> TreeId;
    fn modifiers(&mut self, ctx: &mut CompilerContext, flags: i64) -> TreeId;
    fn modifiers_with_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        flags: i64,
        annotations: List<TreeId>,
    ) -> TreeId;
    fn annotated_type(
        &mut self,
        ctx: &mut CompilerContext,
        annotations: List<TreeId>,
        underlying_type: TreeId,
    ) -> TreeId;
    fn module_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        kind: ModuleKind,
        qual_id: StdOption<TreeId>,
        directives: List<TreeId>,
    ) -> TreeId;
    fn exports(
        &mut self,
        ctx: &mut CompilerContext,
        qual_id: TreeId,
        module_names: StdOption<List<TreeId>>,
    ) -> TreeId;
    fn opens(
        &mut self,
        ctx: &mut CompilerContext,
        qual_id: TreeId,
        module_names: StdOption<List<TreeId>>,
    ) -> TreeId;
    fn provides(
        &mut self,
        ctx: &mut CompilerContext,
        service_name: TreeId,
        impl_names: List<TreeId>,
    ) -> TreeId;
    fn requires(
        &mut self,
        ctx: &mut CompilerContext,
        is_transitive: bool,
        is_static_phase: bool,
        qual_id: TreeId,
    ) -> TreeId;
    fn uses(&mut self, ctx: &mut CompilerContext, qual_id: TreeId) -> TreeId;
    fn erroneous(&mut self, ctx: &mut CompilerContext, errs: List<TreeId>) -> TreeId;
    fn erroneous_empty(&mut self, ctx: &mut CompilerContext) -> TreeId;
    fn let_expr(&mut self, ctx: &mut CompilerContext, defs: List<TreeId>, expr: TreeId) -> TreeId;
    fn type_union(&mut self, ctx: &mut CompilerContext, components: List<TreeId>) -> TreeId;
    fn type_intersection(&mut self, ctx: &mut CompilerContext, components: List<TreeId>) -> TreeId;
    fn anonymous_class_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        defs: List<TreeId>,
    ) -> TreeId;
    fn lambda(&mut self, ctx: &mut CompilerContext, params: List<TreeId>, body: TreeId) -> TreeId;
}

#[allow(clippy::too_many_arguments)]
impl TreeFactory for TreeMaker {
    fn top_level(&mut self, ctx: &mut CompilerContext, defs: List<TreeId>) -> TreeId {
        TreeMaker::top_level(self, ctx, defs)
    }

    fn package_decl(
        &mut self,
        ctx: &mut CompilerContext,
        annotations: List<TreeId>,
        pid: TreeId,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::PACKAGEDEF,
            TreeKind::PackageDecl {
                annotations,
                pid: Some(pid),
                packge: None,
            },
        )
    }

    fn import(&mut self, ctx: &mut CompilerContext, qualid: TreeId, static_import: bool) -> TreeId {
        self.make(
            ctx,
            Tag::IMPORT,
            TreeKind::Import {
                static_import,
                qualid: Some(qualid),
                import_scope: None,
            },
        )
    }

    fn class_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        typarams: List<TreeId>,
        extending: StdOption<TreeId>,
        implementing: List<TreeId>,
        defs: List<TreeId>,
    ) -> TreeId {
        self.class_def_permitting(
            ctx,
            mods,
            name,
            typarams,
            extending,
            implementing,
            List::nil(),
            defs,
        )
    }

    fn class_def_permitting(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        typarams: List<TreeId>,
        extending: StdOption<TreeId>,
        implementing: List<TreeId>,
        permitting: List<TreeId>,
        defs: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::CLASSDEF,
            TreeKind::ClassDecl {
                mods: Some(mods),
                name,
                typarams,
                extending,
                implementing,
                permitting,
                defs,
                sym: None,
            },
        )
    }

    fn method_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        restype: StdOption<TreeId>,
        typarams: List<TreeId>,
        recvparam: StdOption<TreeId>,
        params: List<TreeId>,
        thrown: List<TreeId>,
        body: StdOption<TreeId>,
        default_value: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::METHODDEF,
            TreeKind::MethodDecl {
                mods: Some(mods),
                name,
                restype,
                typarams,
                recvparam,
                params,
                thrown,
                body,
                default_value,
                sym: None,
                completes_normally: false,
            },
        )
    }

    fn method_def_simple(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        restype: StdOption<TreeId>,
        typarams: List<TreeId>,
        params: List<TreeId>,
        thrown: List<TreeId>,
        body: StdOption<TreeId>,
        default_value: StdOption<TreeId>,
    ) -> TreeId {
        self.method_def(
            ctx,
            mods,
            name,
            restype,
            typarams,
            None,
            params,
            thrown,
            body,
            default_value,
        )
    }

    fn var_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        vartype: StdOption<TreeId>,
        init: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::VARDEF,
            TreeKind::VariableDecl {
                mods: Some(mods),
                name,
                nameexpr: None,
                vartype,
                init,
                sym: None,
                start_pos: position::NOPOS,
            },
        )
    }

    fn var_def_declared_using_var(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        name: Name,
        vartype: StdOption<TreeId>,
        init: StdOption<TreeId>,
        declared_using_var: bool,
    ) -> TreeId {
        // Java: JCVariableDecl.declaredUsingVar has no Rust field yet (TODO(Symtab batch));
        // the flag is accepted and ignored until the field is added (reported).
        let _ = declared_using_var;
        self.var_def(ctx, mods, name, vartype, init)
    }

    fn skip(&mut self, ctx: &mut CompilerContext) -> TreeId {
        self.make(ctx, Tag::SKIP, TreeKind::Skip {})
    }

    fn block(&mut self, ctx: &mut CompilerContext, flags: i64, stats: List<TreeId>) -> TreeId {
        self.make(
            ctx,
            Tag::BLOCK,
            TreeKind::Block {
                flags,
                stats,
                endpos: position::NOPOS,
            },
        )
    }

    fn do_loop(&mut self, ctx: &mut CompilerContext, body: TreeId, cond: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::DOLOOP,
            TreeKind::DoWhileLoop {
                body: Some(body),
                cond: Some(cond),
            },
        )
    }

    fn while_loop(&mut self, ctx: &mut CompilerContext, cond: TreeId, body: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::WHILELOOP,
            TreeKind::WhileLoop {
                cond: Some(cond),
                body: Some(body),
            },
        )
    }

    fn for_loop(
        &mut self,
        ctx: &mut CompilerContext,
        init: List<TreeId>,
        cond: StdOption<TreeId>,
        step: List<TreeId>,
        body: TreeId,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::FORLOOP,
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body: Some(body),
            },
        )
    }

    fn foreach_loop(
        &mut self,
        ctx: &mut CompilerContext,
        var: TreeId,
        expr: TreeId,
        body: TreeId,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::FOREACHLOOP,
            TreeKind::EnhancedForLoop {
                var: Some(var),
                expr: Some(expr),
                body: Some(body),
            },
        )
    }

    fn labelled(&mut self, ctx: &mut CompilerContext, label: Name, body: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::LABELLED,
            TreeKind::LabeledStatement {
                label,
                body: Some(body),
            },
        )
    }

    fn switch(
        &mut self,
        ctx: &mut CompilerContext,
        selector: TreeId,
        cases: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::SWITCH,
            TreeKind::Switch {
                selector: Some(selector),
                cases,
                endpos: position::NOPOS,
                has_total_pattern: false,
                pattern_switch: false,
            },
        )
    }

    fn switch_expression(
        &mut self,
        ctx: &mut CompilerContext,
        selector: TreeId,
        cases: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::SWITCH_EXPRESSION,
            TreeKind::SwitchExpression {
                selector: Some(selector),
                cases,
                endpos: position::NOPOS,
                has_total_pattern: false,
                pattern_switch: false,
                poly_kind: PolyKind::STANDALONE,
            },
        )
    }

    fn case(
        &mut self,
        ctx: &mut CompilerContext,
        case_kind: CaseKind,
        labels: List<TreeId>,
        stats: List<TreeId>,
        body: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::CASE,
            TreeKind::Case {
                case_kind,
                labels,
                stats,
                body,
                completes_normally: false,
            },
        )
    }

    fn synchronized(&mut self, ctx: &mut CompilerContext, lock: TreeId, body: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::SYNCHRONIZED,
            TreeKind::Synchronized {
                lock: Some(lock),
                body: Some(body),
            },
        )
    }

    fn try_block(
        &mut self,
        ctx: &mut CompilerContext,
        body: TreeId,
        catchers: List<TreeId>,
        finalizer: StdOption<TreeId>,
    ) -> TreeId {
        self.try_with_resources(ctx, List::nil(), body, catchers, finalizer)
    }

    fn try_with_resources(
        &mut self,
        ctx: &mut CompilerContext,
        resources: List<TreeId>,
        body: TreeId,
        catchers: List<TreeId>,
        finalizer: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::TRY,
            TreeKind::Try {
                body: Some(body),
                catchers,
                finalizer,
                resources,
                finally_can_complete_normally: false,
            },
        )
    }

    fn catch(&mut self, ctx: &mut CompilerContext, param: TreeId, body: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::CATCH,
            TreeKind::Catch {
                param: Some(param),
                body: Some(body),
            },
        )
    }

    fn conditional(
        &mut self,
        ctx: &mut CompilerContext,
        cond: TreeId,
        thenpart: TreeId,
        elsepart: TreeId,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::CONDEXPR,
            TreeKind::Conditional {
                cond: Some(cond),
                truepart: Some(thenpart),
                falsepart: Some(elsepart),
                poly_kind: PolyKind::STANDALONE,
            },
        )
    }

    fn if_(
        &mut self,
        ctx: &mut CompilerContext,
        cond: TreeId,
        thenpart: TreeId,
        elsepart: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::IF,
            TreeKind::If {
                cond: Some(cond),
                thenpart: Some(thenpart),
                elsepart,
            },
        )
    }

    fn exec(&mut self, ctx: &mut CompilerContext, expr: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::EXEC,
            TreeKind::ExpressionStatement { expr: Some(expr) },
        )
    }

    fn break_(&mut self, ctx: &mut CompilerContext, label: StdOption<Name>) -> TreeId {
        self.make(
            ctx,
            Tag::BREAK,
            TreeKind::Break {
                label,
                target: None,
            },
        )
    }

    fn yield_(&mut self, ctx: &mut CompilerContext, value: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::YIELD,
            TreeKind::Yield {
                value: Some(value),
                target: None,
            },
        )
    }

    fn continue_(&mut self, ctx: &mut CompilerContext, label: StdOption<Name>) -> TreeId {
        self.make(
            ctx,
            Tag::CONTINUE,
            TreeKind::Continue {
                label,
                target: None,
            },
        )
    }

    fn return_(&mut self, ctx: &mut CompilerContext, expr: StdOption<TreeId>) -> TreeId {
        self.make(ctx, Tag::RETURN, TreeKind::Return { expr })
    }

    fn throw_(&mut self, ctx: &mut CompilerContext, expr: TreeId) -> TreeId {
        self.make(ctx, Tag::THROW, TreeKind::Throw { expr: Some(expr) })
    }

    fn assert_(
        &mut self,
        ctx: &mut CompilerContext,
        cond: TreeId,
        detail: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::ASSERT,
            TreeKind::Assert {
                cond: Some(cond),
                detail,
            },
        )
    }

    fn apply(
        &mut self,
        ctx: &mut CompilerContext,
        typeargs: List<TreeId>,
        fn_: TreeId,
        args: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::APPLY,
            TreeKind::MethodInvocation {
                typeargs,
                meth: Some(fn_),
                args,
                varargs_element: None,
                poly_kind: PolyKind::STANDALONE,
            },
        )
    }

    fn new_class(
        &mut self,
        ctx: &mut CompilerContext,
        encl: StdOption<TreeId>,
        typeargs: List<TreeId>,
        clazz: TreeId,
        args: List<TreeId>,
        def: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::NEWCLASS,
            TreeKind::NewClass {
                encl,
                typeargs,
                clazz: Some(clazz),
                args,
                def,
                constructor: None,
                varargs_element: None,
                constructor_type: None,
                poly_kind: PolyKind::STANDALONE,
            },
        )
    }

    fn new_class_speculative(
        &mut self,
        ctx: &mut CompilerContext,
        encl: StdOption<TreeId>,
        typeargs: List<TreeId>,
        clazz: TreeId,
        args: List<TreeId>,
        def: StdOption<TreeId>,
        class_def_removed: bool,
    ) -> TreeId {
        let tree = self.new_class(ctx, encl, typeargs, clazz, args, def);
        if class_def_removed && let TreeKind::NewClass { def, .. } = &mut ctx.tree_mut(tree).kind {
            *def = None;
        }
        tree
    }

    fn new_array(
        &mut self,
        ctx: &mut CompilerContext,
        elemtype: StdOption<TreeId>,
        dims: List<TreeId>,
        elems: StdOption<List<TreeId>>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::NEWARRAY,
            TreeKind::NewArray {
                elemtype,
                dims,
                annotations: List::nil(),
                dim_annotations: List::<List<TreeId>>::nil(),
                elems,
            },
        )
    }

    fn parens(&mut self, ctx: &mut CompilerContext, expr: TreeId) -> TreeId {
        self.make(ctx, Tag::PARENS, TreeKind::Parens { expr: Some(expr) })
    }

    fn assign(&mut self, ctx: &mut CompilerContext, lhs: TreeId, rhs: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::ASSIGN,
            TreeKind::Assign {
                lhs: Some(lhs),
                rhs: Some(rhs),
            },
        )
    }

    fn assignop(
        &mut self,
        ctx: &mut CompilerContext,
        opcode: Tag,
        lhs: TreeId,
        rhs: TreeId,
    ) -> TreeId {
        self.make(
            ctx,
            opcode,
            TreeKind::AssignOp {
                lhs: Some(lhs),
                rhs: Some(rhs),
                operator: None,
            },
        )
    }

    fn unary(&mut self, ctx: &mut CompilerContext, opcode: Tag, arg: TreeId) -> TreeId {
        self.make(
            ctx,
            opcode,
            TreeKind::Unary {
                arg: Some(arg),
                operator: None,
            },
        )
    }

    fn binary(
        &mut self,
        ctx: &mut CompilerContext,
        opcode: Tag,
        lhs: TreeId,
        rhs: TreeId,
    ) -> TreeId {
        self.make(
            ctx,
            opcode,
            TreeKind::Binary {
                lhs: Some(lhs),
                rhs: Some(rhs),
                operator: None,
            },
        )
    }

    fn type_cast(&mut self, ctx: &mut CompilerContext, expr: TreeId, type_: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::TYPECAST,
            TreeKind::TypeCast {
                clazz: Some(type_),
                expr: Some(expr),
            },
        )
    }

    fn type_test(&mut self, ctx: &mut CompilerContext, expr: TreeId, clazz: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::TYPETEST,
            TreeKind::InstanceOf {
                expr: Some(expr),
                pattern: Some(clazz),
            },
        )
    }

    fn binding_pattern(&mut self, ctx: &mut CompilerContext, var: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::BINDINGPATTERN,
            TreeKind::BindingPattern { var: Some(var) },
        )
    }

    fn default_case_label(&mut self, ctx: &mut CompilerContext) -> TreeId {
        self.make(ctx, Tag::DEFAULTCASELABEL, TreeKind::DefaultCaseLabel {})
    }

    fn parenthesized_pattern(&mut self, ctx: &mut CompilerContext, pattern: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::PARENTHESIZEDPATTERN,
            TreeKind::ParenthesizedPattern {
                pattern: Some(pattern),
            },
        )
    }

    fn guard_pattern(&mut self, ctx: &mut CompilerContext, patt: TreeId, expr: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::GUARDPATTERN,
            TreeKind::GuardPattern {
                patt: Some(patt),
                expr: Some(expr),
            },
        )
    }

    fn indexed(&mut self, ctx: &mut CompilerContext, indexed: TreeId, index: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::INDEXED,
            TreeKind::ArrayAccess {
                indexed: Some(indexed),
                index: Some(index),
            },
        )
    }

    fn select(&mut self, ctx: &mut CompilerContext, selected: TreeId, selector: Name) -> TreeId {
        self.make(
            ctx,
            Tag::SELECT,
            TreeKind::FieldAccess {
                selected: Some(selected),
                name: selector,
                sym: None,
            },
        )
    }

    fn ident(&mut self, ctx: &mut CompilerContext, name: Name) -> TreeId {
        self.make(ctx, Tag::IDENT, TreeKind::Ident { name, sym: None })
    }

    fn literal(
        &mut self,
        ctx: &mut CompilerContext,
        typetag: TypeTag,
        value: StdOption<LiteralValue>,
    ) -> TreeId {
        self.make(ctx, Tag::LITERAL, TreeKind::Literal { typetag, value })
    }

    fn type_ident(&mut self, ctx: &mut CompilerContext, typetag: TypeTag) -> TreeId {
        self.make(ctx, Tag::TYPEIDENT, TreeKind::PrimitiveTypeTree { typetag })
    }

    fn type_array(&mut self, ctx: &mut CompilerContext, elemtype: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::TYPEARRAY,
            TreeKind::ArrayTypeTree {
                elemtype: Some(elemtype),
            },
        )
    }

    fn type_apply(
        &mut self,
        ctx: &mut CompilerContext,
        clazz: TreeId,
        arguments: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::TYPEAPPLY,
            TreeKind::TypeApply {
                clazz: Some(clazz),
                arguments,
            },
        )
    }

    fn type_parameter(
        &mut self,
        ctx: &mut CompilerContext,
        name: Name,
        bounds: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::TYPEPARAMETER,
            TreeKind::TypeParameter {
                name,
                bounds,
                annotations: List::nil(),
            },
        )
    }

    fn wildcard(
        &mut self,
        ctx: &mut CompilerContext,
        kind: TreeId,
        type_: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::WILDCARD,
            TreeKind::Wildcard {
                kind: Some(kind),
                inner: type_,
            },
        )
    }

    fn type_bound_kind(&mut self, ctx: &mut CompilerContext, kind: BoundKind) -> TreeId {
        self.make(ctx, Tag::TYPEBOUNDKIND, TreeKind::TypeBoundKind { kind })
    }

    fn annotation(
        &mut self,
        ctx: &mut CompilerContext,
        annotation_type: TreeId,
        args: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::ANNOTATION,
            TreeKind::Annotation {
                annotation_type: Some(annotation_type),
                args,
                attribute: None,
            },
        )
    }

    fn type_annotation(
        &mut self,
        ctx: &mut CompilerContext,
        annotation_type: TreeId,
        args: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::TYPE_ANNOTATION,
            TreeKind::Annotation {
                annotation_type: Some(annotation_type),
                args,
                attribute: None,
            },
        )
    }

    fn modifiers(&mut self, ctx: &mut CompilerContext, flags: i64) -> TreeId {
        self.modifiers_with_annotations(ctx, flags, List::nil())
    }

    fn modifiers_with_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        flags: i64,
        annotations: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::MODIFIERS,
            TreeKind::Modifiers { flags, annotations },
        )
    }

    fn annotated_type(
        &mut self,
        ctx: &mut CompilerContext,
        annotations: List<TreeId>,
        underlying_type: TreeId,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::ANNOTATED_TYPE,
            TreeKind::AnnotatedType {
                annotations,
                underlying_type: Some(underlying_type),
            },
        )
    }

    fn module_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        kind: ModuleKind,
        qual_id: StdOption<TreeId>,
        directives: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::MODULEDEF,
            TreeKind::ModuleDecl {
                mods: Some(mods),
                ty: None,
                kind,
                qual_id,
                directives: Some(directives),
                sym: None,
            },
        )
    }

    fn exports(
        &mut self,
        ctx: &mut CompilerContext,
        qual_id: TreeId,
        module_names: StdOption<List<TreeId>>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::EXPORTS,
            TreeKind::Exports {
                qualid: Some(qual_id),
                module_names,
                directive: None,
            },
        )
    }

    fn opens(
        &mut self,
        ctx: &mut CompilerContext,
        qual_id: TreeId,
        module_names: StdOption<List<TreeId>>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::OPENS,
            TreeKind::Opens {
                qualid: Some(qual_id),
                module_names,
                directive: None,
            },
        )
    }

    fn provides(
        &mut self,
        ctx: &mut CompilerContext,
        service_name: TreeId,
        impl_names: List<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::PROVIDES,
            TreeKind::Provides {
                service_name: Some(service_name),
                impl_names,
            },
        )
    }

    fn requires(
        &mut self,
        ctx: &mut CompilerContext,
        is_transitive: bool,
        is_static_phase: bool,
        qual_id: TreeId,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::REQUIRES,
            TreeKind::Requires {
                is_transitive,
                is_static_phase,
                module_name: Some(qual_id),
                directive: None,
            },
        )
    }

    fn uses(&mut self, ctx: &mut CompilerContext, qual_id: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::USES,
            TreeKind::Uses {
                qualid: Some(qual_id),
            },
        )
    }

    fn erroneous(&mut self, ctx: &mut CompilerContext, errs: List<TreeId>) -> TreeId {
        self.make(ctx, Tag::ERRONEOUS, TreeKind::Erroneous { errs })
    }

    fn erroneous_empty(&mut self, ctx: &mut CompilerContext) -> TreeId {
        self.erroneous(ctx, List::nil())
    }

    fn let_expr(&mut self, ctx: &mut CompilerContext, defs: List<TreeId>, expr: TreeId) -> TreeId {
        self.make(
            ctx,
            Tag::LETEXPR,
            TreeKind::LetExpr {
                defs,
                expr: Some(expr),
                needs_cond: false,
            },
        )
    }

    fn type_union(&mut self, ctx: &mut CompilerContext, components: List<TreeId>) -> TreeId {
        self.make(
            ctx,
            Tag::TYPEUNION,
            TreeKind::TypeUnion {
                alternatives: components,
            },
        )
    }

    fn type_intersection(&mut self, ctx: &mut CompilerContext, components: List<TreeId>) -> TreeId {
        self.make(
            ctx,
            Tag::TYPEINTERSECTION,
            TreeKind::TypeIntersection { bounds: components },
        )
    }

    fn anonymous_class_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        defs: List<TreeId>,
    ) -> TreeId {
        // Java: ClassDef(mods, names.empty, List.nil(), null, List.nil(), List.nil(), defs)
        let name = self.empty_name();
        self.class_def_permitting(
            ctx,
            mods,
            name,
            List::nil(),
            None,
            List::nil(),
            List::nil(),
            defs,
        )
    }

    fn lambda(&mut self, ctx: &mut CompilerContext, params: List<TreeId>, body: TreeId) -> TreeId {
        // Java: new JCLambda(pos, params, body): ParameterKind.EXPLICIT/PolyKind.STANDALONE.
        self.make(
            ctx,
            Tag::LAMBDA,
            TreeKind::Lambda {
                params,
                body: Some(body),
                can_complete_normally: false,
                param_kind: ParameterKind::EXPLICIT,
                poly_kind: PolyKind::STANDALONE,
                target: None,
            },
        )
    }
}

impl TreeMaker {
    /// Java: public JCMethodInvocation App(JCExpression meth).
    pub fn app(&mut self, ctx: &mut CompilerContext, meth: TreeId) -> TreeId {
        self.apply(ctx, List::nil(), meth, List::nil())
    }

    /// Java: public JCMethodInvocation App(JCExpression meth, List<JCExpression> args).
    pub fn app_with_args(
        &mut self,
        ctx: &mut CompilerContext,
        meth: TreeId,
        args: List<TreeId>,
    ) -> TreeId {
        self.apply(ctx, List::nil(), meth, args)
    }

    /// Java: public JCLiteral Literal(Object value) — TypeTag inferred from the wrapper type.
    ///
    /// TODO(Log batch): the upstream invalid literal case reports a compiler error
    /// (Errors.InvalidLiteral) and returns a null literal; that diagnostic is not ported, so the
    /// unsupported value panics (only LiteralValue can reach here, all are supported).
    pub fn literal_object(&mut self, ctx: &mut CompilerContext, value: LiteralValue) -> TreeId {
        let typetag = match &value {
            LiteralValue::Integer(_) => TypeTag::INT,
            LiteralValue::Long(_) => TypeTag::LONG,
            LiteralValue::Float(_) => TypeTag::FLOAT,
            LiteralValue::Double(_) => TypeTag::DOUBLE,
            LiteralValue::Character(_) => TypeTag::CHAR,
            LiteralValue::Boolean(_) => TypeTag::BOOLEAN,
            LiteralValue::String(_) => TypeTag::CLASS,
        };
        self.literal(ctx, typetag, Some(value))
    }

    /// Java: public JCExpression Type(Type t) — needs the Types service (TODO(Types batch)).
    pub fn type_expr(&mut self, _ctx: &mut CompilerContext, _t: crate::ids::TypeId) -> TreeId {
        unimplemented!("TODO(Types batch): TreeMaker.Type(Type) needs the Types service")
    }

    /// Java: public JCExpression Ident(Symbol sym) / Ident(JCVariableDecl tree) / ClassLiteral /
    /// This / Super / QualThis / QualIdent / Create — Symbol-based convenience factories need the
    /// symbol/type model (TODO(Symtab/Types batch)).
    pub fn ident_symbol(
        &mut self,
        _ctx: &mut CompilerContext,
        _sym: crate::ids::SymbolId,
    ) -> TreeId {
        unimplemented!("TODO(Symtab batch): TreeMaker.Ident(Symbol) needs symbol flags/names")
    }

    /// Java: public JCVariableDecl VarDef(VarSymbol v, JCExpression init).
    pub fn var_def_symbol(
        &mut self,
        _ctx: &mut CompilerContext,
        _v: crate::ids::SymbolId,
        _init: StdOption<TreeId>,
    ) -> TreeId {
        unimplemented!("TODO(Symtab batch): TreeMaker.VarDef(VarSymbol, init)")
    }

    /// Java: public JCMethodDecl MethodDef(MethodSymbol m).
    pub fn method_def_symbol(
        &mut self,
        _ctx: &mut CompilerContext,
        _m: crate::ids::SymbolId,
    ) -> TreeId {
        unimplemented!("TODO(Symtab batch): TreeMaker.MethodDef(MethodSymbol)")
    }

    /// Java: public JCTypeParameter TypeParam(Symbol sym) / TypeParams / Param / Params.
    pub fn type_param_symbol(
        &mut self,
        _ctx: &mut CompilerContext,
        _sym: crate::ids::SymbolId,
    ) -> TreeId {
        unimplemented!("TODO(Symtab batch): TreeMaker.TypeParam(Symbol)")
    }

    /// Java: public JCExpression Call / Assignment / Indexed(Symbol) / TypeCast(Type) helpers.
    pub fn call(
        &mut self,
        _ctx: &mut CompilerContext,
        _meth: crate::ids::SymbolId,
        _args: List<TreeId>,
    ) -> TreeId {
        unimplemented!("TODO(Symtab batch): TreeMaker.Call(Symbol, args) needs MethodSymbol")
    }
}

#[cfg(test)]
mod tests {
    use super::{TreeFactory, TreeMaker};
    use crate::code::type_tag::TypeTag;
    use crate::compiler_context::CompilerContext;
    use crate::tree::jc_tree::{LiteralValue, ParameterKind, PolyKind, Tag, TreeKind};
    use crate::tree::pretty::Pretty;
    use crate::util::list::List;
    use crate::util::name::Name;
    use crate::util::names::Names;
    use javac_strings::JavaString;
    use std::rc::Rc;

    fn new_maker() -> (CompilerContext, TreeMaker) {
        let names = Rc::new(Names::new(false));
        (CompilerContext::new(), TreeMaker::new(Some(names)))
    }

    fn name(maker: &TreeMaker, s: &str) -> Name {
        maker
            .names
            .as_ref()
            .expect("names in the test maker")
            .from_utf(s.as_bytes())
    }

    /// Builds: class C { int x; int f() { return 42; } } with a source position per node.
    fn build_unit(
        ctx: &mut CompilerContext,
        maker: &mut TreeMaker,
    ) -> (crate::ids::TreeId, crate::ids::TreeId) {
        let mods = maker.at(0).modifiers(ctx, 0);
        let int = maker.at(5).type_ident(ctx, TypeTag::INT);
        let lit = maker.at(10).literal_object(ctx, LiteralValue::Integer(42));
        let ret = maker.at(8).return_(ctx, Some(lit));
        let block = maker.at(6).block(ctx, 0, List::of(ret));
        let f = name(maker, "f");
        let mdef = maker.at(4).method_def_simple(
            ctx,
            mods,
            f,
            Some(int),
            List::nil(),
            List::nil(),
            List::nil(),
            Some(block),
            None,
        );
        let x = name(maker, "x");
        let var = maker.at(3).var_def(ctx, mods, x, Some(int), None);
        let cname = name(maker, "C");
        let cdef = maker.at(2).class_def(
            ctx,
            mods,
            cname,
            List::nil(),
            None,
            List::nil(),
            List::of(mdef).prepend(var),
        );
        let unit = maker.top_level(ctx, List::of(cdef));
        (unit, cdef)
    }

    #[test]
    fn top_level_and_class_members_keep_pos_and_shape() {
        let (mut ctx, mut maker) = new_maker();
        let (unit, cdef) = build_unit(&mut ctx, &mut maker);
        assert_eq!(maker.toplevel, Some(unit));
        assert_eq!(ctx.tree(unit).tag, Tag::TOPLEVEL);
        assert_eq!(ctx.tree(unit).pos, 2);
        let TreeKind::CompilationUnit { defs, .. } = &ctx.tree(unit).kind else {
            panic!("compilation unit");
        };
        assert_eq!(defs.length(), 1);
        let TreeKind::ClassDecl {
            name,
            mods,
            defs: members,
            ..
        } = &ctx.tree(cdef).kind
        else {
            panic!("class def");
        };
        assert_eq!(ctx.tree(cdef).pos, 2);
        assert!(name.content_equals(&JavaString::from("C")));
        assert_eq!(ctx.tree(mods.expect("mods")).tag, Tag::MODIFIERS);
        assert_eq!(members.length(), 2);
        // The two members are the variable and the method, in order.
        let TreeKind::ClassDecl { defs: members, .. } = &ctx.tree(cdef).kind else {
            unreachable!()
        };
        assert_eq!(
            ctx.tree(members.iter().next().expect("var")).tag,
            Tag::VARDEF
        );
        assert_eq!(
            ctx.tree(members.iter().nth(1).expect("method")).tag,
            Tag::METHODDEF
        );
    }

    #[test]
    fn method_body_expression_positions_literals_and_lambda() {
        let (mut ctx, mut maker) = new_maker();
        let lit = maker
            .at(10)
            .literal_object(&mut ctx, LiteralValue::Integer(42));
        assert_eq!(ctx.tree(lit).tag, Tag::LITERAL);
        assert_eq!(ctx.tree(lit).pos, 10);
        let TreeKind::Literal { typetag, value } = &ctx.tree(lit).kind else {
            panic!("literal");
        };
        assert_eq!(*typetag, TypeTag::INT);
        assert_eq!(*value, Some(LiteralValue::Integer(42)));

        let block = maker.at(6).block(&mut ctx, 0, List::nil());
        assert_eq!(ctx.tree(block).tag, Tag::BLOCK);
        let params = List::nil();
        let lambda = maker.at(11).lambda(&mut ctx, params, lit);
        assert_eq!(ctx.tree(lambda).tag, Tag::LAMBDA);
        let TreeKind::Lambda {
            param_kind,
            poly_kind,
            body,
            ..
        } = &ctx.tree(lambda).kind
        else {
            panic!("lambda");
        };
        assert_eq!(*param_kind, ParameterKind::EXPLICIT);
        assert_eq!(*poly_kind, PolyKind::STANDALONE);
        assert_eq!(*body, Some(lit));

        let cond = maker.at(12).conditional(&mut ctx, lit, lit, lit);
        assert_eq!(ctx.tree(cond).tag, Tag::CONDEXPR);
        let binary = maker.at(13).binary(&mut ctx, Tag::PLUS, lit, lit);
        assert_eq!(ctx.tree(binary).tag, Tag::PLUS);
        let TreeKind::Binary { operator, .. } = &ctx.tree(binary).kind else {
            panic!("binary");
        };
        assert_eq!(*operator, None);
    }

    #[test]
    fn annotations_and_modifiers() {
        let (mut ctx, mut maker) = new_maker();
        let int = maker.at(5).type_ident(&mut ctx, TypeTag::INT);
        let lit = maker
            .at(10)
            .literal_object(&mut ctx, LiteralValue::Integer(1));
        let ann = maker.at(9).annotation(&mut ctx, int, List::of(lit));
        assert_eq!(ctx.tree(ann).tag, Tag::ANNOTATION);
        let mods = maker
            .at(1)
            .modifiers_with_annotations(&mut ctx, 0x0004, List::of(ann));
        let TreeKind::Modifiers { flags, annotations } = &ctx.tree(mods).kind else {
            panic!("modifiers");
        };
        assert_eq!(*flags, 0x0004);
        assert_eq!(annotations.length(), 1);
    }

    #[test]
    fn pretty_prints_maker_built_expressions_and_statements() {
        // NOTE: Pretty.printUnit is not used here: its import filtering needs isUsed/TreeScanner
        // (unimplemented in the pretty batch, not part of this task).  The expression/statement
        // printers are complete, so the integration test covers those.
        let (mut ctx, mut maker) = new_maker();
        let one = maker
            .at(10)
            .literal_object(&mut ctx, LiteralValue::Integer(1));
        let two = maker
            .at(12)
            .literal_object(&mut ctx, LiteralValue::Integer(2));
        let sum = maker.at(11).binary(&mut ctx, Tag::PLUS, one, two);
        let mut pretty = Pretty::new(false);
        pretty.print_expr(&ctx, sum);
        let printed = pretty.get_out().to_string_lossy();
        assert!(printed.contains("1 + 2"), "{printed}");

        let (mut ctx, mut maker) = new_maker();
        let lit = maker
            .at(10)
            .literal_object(&mut ctx, LiteralValue::Integer(42));
        let ret = maker.at(8).return_(&mut ctx, Some(lit));
        let mut pretty = Pretty::new(false);
        pretty.print_stat(&ctx, ret);
        let printed = pretty.get_out().to_string_lossy();
        assert!(printed.contains("return 42"), "{printed}");
    }
}

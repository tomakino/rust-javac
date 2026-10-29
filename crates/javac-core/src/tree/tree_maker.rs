// Origin: com/sun/tools/javac/tree/TreeMaker.java @ 982d53c (jdk-17.0.19-ga)
// Gap registration (method-level audit of the ported upstream class):
// Dependency/phase: tree (tree nodes/scanners for the corresponding upstream feature).
// Not yet ported upstream methods (no substitutes in this file):
// - NOT PORTED: SpeculativeNewClass (TreeMaker.java) / classDeclRemoved
// (TreeMaker.java): the anonymous JCNewClass subclass override has no arena
// mapping yet (registered; the JCNewClass node has no classDeclRemoved flag).
// - NOT PORTED: Literal(Object) Byte/Short branches (TreeMaker.java):
// the ported tree::jc_tree::LiteralValue value domain has no Byte/Short
// variants (registered: literal value domain); the remaining wrapper
// types (Integer/Long/Float/Double/Character/Boolean/String) are ported.
// - The AnnotationBuilder (TreeMaker.java) is expressed as TreeMaker
// visitor methods (visit_constant_attribute/visit_class_attribute/...), the
// Rust equivalent of the Java inner class (registered adapter); the visitor
// dispatch is the AttributeKind match.
// End of Gap registration.

//! A factory for JCTree nodes: the upstream TreeMaker keeps the current position and the
//! toplevel tree, and its makeXxx methods allocate JCxxx values; the JCTree.Factory interface
//! is represented by the TreeFactory trait implemented by TreeMaker.
//!
//! <p><b>This is NOT part of any supported API.</b>
//!
//! PORTING_RULES (S3):
//! * JCTree is a value in the CompilerContext tree arena: Java "new JCxxx(); tree.pos = pos"
//!   becomes ctx.alloc_tree(JCTree { pos, ty: None, tag, kind }) (TreeId returned; see ADR 0001).
//! * Java null child references are Option<TreeId>; child lists are List<TreeId>. A Java
//!   null list argument (e.g. the JCExpression extending of a class) maps to Option too, but
//!   the makeXxx list parameters are kept as List<TreeId> (callers pass List::nil()).
//! * TreeMaker is a mutable shared object in javac (Parser keeps one); the Rust instance is
//!   Rc<RefCell<TreeMaker>> (the interior-mutability whitelist used for Options/Log; registered).
//! * Names is ported (util::names::Names), but the Java Names.instance(Context) adapter is not;
//!   TreeMaker::instance reads an Option<Rc<Names>> from the Context (registered by the
//!   caller when available) and the field stays None until then. Types and Symtab are now
//!   ported services but they are passed explicitly (the Types service is stateful and Symtab
//!   is &mut at the call sites; ADR 0001); the upstream TreeMaker fields types/syms are
//!   therefore not stored, and the Symbol/Type convenience factories (Ident(Symbol),
//!   QualIdent, Type(Type), VarDef(VarSymbol), MethodDef(MethodSymbol), TypeParam, Param(s),
//!   This/Super/ClassLiteral, Create,...) take ctx/syms (and Types where required) as
//!   explicit parameters (see TreeMaker.java:703-1144; the Attribute/constType based helpers
//!   remain registered NOT PORTED, see the Registration list above).
//! * at(DiagnosticPosition) takes Option<&dyn DiagnosticPosition> (Java null means NOPOS);
//!   getStartPosition -> DiagnosticPosition::get_start_position.
//! * JCLambda/conditional/member-reference constructor defaults: the Java constructors set
//!   PolyKind.STANDALONE and ParameterKind.EXPLICIT (JCLambda(..., body) constructor); these
//!   are reproduced. JCMemberReference.kind is null until Attr in the upstream parser
//!   (JCMemberReference constructor does not set it); Rust models the null as Option::None.

// The upstream Factory signatures are kept 1:1 (MethodDef has 9 arguments, plus the arena
// context); the clippy argument-count thresholds do not apply to a direct port.
#![allow(clippy::too_many_arguments)]

use std::cell::RefCell;
use std::option::Option as StdOption;
use std::rc::Rc;

use javac_source::tree::case_tree::CaseKind;
use javac_source::tree::member_reference_tree::ReferenceMode;
use javac_source::tree::module_tree::ModuleKind;

use crate::code::bound_kind::BoundKind;
use crate::code::flags;
use crate::code::types::Types;
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
    /// Java: Types types (the shared type service; registered adapter: linked
    /// in at wiring time instead of read from Context).
    pub types: StdOption<Rc<Types>>,
    /// Java: Symtab syms (the shared symbol table; registered adapter: linked
    /// in at wiring time instead of read from Context).
    pub syms: StdOption<Rc<RefCell<crate::code::symtab::Symtab>>>,
    /// Java: Symtab.rootPackage (captured at wiring so qual_ident does not
    /// need to borrow the shared Symtab; registered adapter).
    pub root_package: StdOption<crate::ids::SymbolId>,
// Java: Types types; Symtab syms; — not declared yet, see the file header (REGISTERED).
}

impl TreeMaker {
    /// Java: protected TreeMaker(Context context).
    pub fn new(names: StdOption<Rc<Names>>) -> TreeMaker {
        TreeMaker {
            pos: position::NOPOS,
            toplevel: None,
            names,
            types: None,
            syms: None,
            root_package: None,
        }
    }

    /// Java: protected TreeMaker(JCCompilationUnit toplevel, Names names, Types types, Symtab syms).
    pub fn new_for_toplevel(
        toplevel: StdOption<TreeId>,
        names: StdOption<Rc<Names>>,
        types: StdOption<Rc<Types>>,
        syms: StdOption<Rc<RefCell<crate::code::symtab::Symtab>>>, 
    ) -> TreeMaker {
        TreeMaker {
            pos: position::FIRSTPOS,
            toplevel,
            names,
            types,
            syms,
            root_package: None,
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
        let mut maker = TreeMaker::new_for_toplevel(
            toplevel,
            self.names.clone(),
            self.types.clone(),
            self.syms.clone(),
        );
        maker.root_package = self.root_package;
        maker
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
                "TreeMaker.names is not installed (Names adapter REGISTERED)"
            ),
        }
    }
}

/// Java: JCTree.Factory — the node factory interface implemented by TreeMaker.
///
/// PORTING_RULES: Java nested interface JCTree.Factory -> trait TreeFactory (Rust traits cannot
/// be nested); every method takes the CompilerContext arena first, and returns TreeId. Java
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
    /// Java: JCTree.Factory.ReceiverVarDef(JCModifiers mods, JCExpression nameexpr,
    /// JCExpression vartype) — the receiver parameter (name is Names.empty).
    fn receiver_var_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        nameexpr: TreeId,
        vartype: StdOption<TreeId>,
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
    fn yield_(
        &mut self,
        ctx: &mut CompilerContext,
        value: StdOption<TreeId>,
    ) -> TreeId;
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
    fn type_cast(&mut self, ctx: &mut CompilerContext, type_: TreeId, expr: TreeId) -> TreeId;
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
    fn reference(
        &mut self,
        ctx: &mut CompilerContext,
        mode: ReferenceMode,
        name: Name,
        expr: TreeId,
        typeargs: StdOption<List<TreeId>>,
    ) -> TreeId;
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
                declared_using_var: false,
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
                declared_using_var,
            },
        )
    }

    fn receiver_var_def(
        &mut self,
        ctx: &mut CompilerContext,
        mods: TreeId,
        nameexpr: TreeId,
        vartype: StdOption<TreeId>,
    ) -> TreeId {
        // Java: JCVariableDecl.Factory.ReceiverVarDef -> new JCVariableDecl(mods,
        // names.empty, nameexpr, vartype, null).
        let name = self.empty_name();
        self.make(
            ctx,
            Tag::VARDEF,
            TreeKind::VariableDecl {
                mods: Some(mods),
                name,
                nameexpr: Some(nameexpr),
                vartype,
                init: None,
                sym: None,
                start_pos: position::NOPOS,
                declared_using_var: false,
            },
        )
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

    fn yield_(
        &mut self,
        ctx: &mut CompilerContext,
        value: StdOption<TreeId>,
    ) -> TreeId {
        self.make(
            ctx,
            Tag::YIELD,
            TreeKind::Yield { value, target: None },
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
                class_decl_removed: false,
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
        if let TreeKind::NewClass { class_decl_removed, .. } = &mut ctx.tree_mut(tree).kind {
            *class_decl_removed = class_def_removed;
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

    /// Upstream: TreeMaker.TypeCast(Type type, JCExpression expr).
    fn type_cast(&mut self, ctx: &mut CompilerContext, type_: TreeId, expr: TreeId) -> TreeId {
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
        let tree = self.make(
            ctx,
            Tag::ANNOTATION,
            TreeKind::Annotation {
                annotation_type: Some(annotation_type),
                args,
                attribute: None,
            },
        );
        // Java: JCAnnotation(ANNOTATION, a, a.type, List.nil())
        // (TreeMaker.Annotation): the synthesized annotation carries its type.
        let annotation_ty = ctx.tree(annotation_type).ty;
        ctx.tree_mut(tree).ty = annotation_ty;
        tree
    }

    fn type_annotation(
        &mut self,
        ctx: &mut CompilerContext,
        annotation_type: TreeId,
        args: List<TreeId>,
    ) -> TreeId {
        let tree = self.make(
            ctx,
            Tag::TYPE_ANNOTATION,
            TreeKind::Annotation {
                annotation_type: Some(annotation_type),
                args,
                attribute: None,
            },
        );
        // Java: JCAnnotation(TYPE_ANNOTATION, a, a.type, List.nil())
        // (TreeMaker.TypeAnnotation): the synthesized container annotation
        // carries the container type.
        let annotation_ty = ctx.tree(annotation_type).ty;
        ctx.tree_mut(tree).ty = annotation_ty;
        tree
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
        // Java: new JCLambda(pos, params, body): paramKind is EXPLICIT when the
        // list is empty or the first parameter has a declared type, IMPLICIT
        // otherwise; PolyKind.STANDALONE (JCLambda constructor).
        let param_kind = if params.is_empty() {
            ParameterKind::EXPLICIT
        } else {
            let first = *params.head();
            match &ctx.tree(first).kind {
                TreeKind::VariableDecl {
                    vartype: Some(_), ..
                } => ParameterKind::EXPLICIT,
                _ => ParameterKind::IMPLICIT,
            }
        };
        self.make(
            ctx,
            Tag::LAMBDA,
            TreeKind::Lambda {
                params,
                body: Some(body),
                can_complete_normally: false,
                param_kind,
                poly_kind: PolyKind::STANDALONE,
                target: None,
            },
        )
    }

    fn reference(
        &mut self,
        ctx: &mut CompilerContext,
        mode: ReferenceMode,
        name: Name,
        expr: TreeId,
        typeargs: StdOption<List<TreeId>>,
    ) -> TreeId {
        // Java: new JCMemberReference(mode, name, expr, typeargs): kind stays
        // null (None) until Attr; refPolyKind/polyKind default STANDALONE.
        self.make(
            ctx,
            Tag::REFERENCE,
            TreeKind::MemberReference {
                mode,
                kind: None,
                name,
                expr: Some(expr),
                typeargs,
                sym: None,
                varargs_element: None,
                ref_poly_kind: PolyKind::STANDALONE,
                owner_accessible: false,
                overload_kind: None,
                referent_type: None,
                poly_kind: PolyKind::STANDALONE,
                target: None,
            },
        )
    }
}

impl TreeMaker {
    // ------------------------------------------------------------------
    // Java: TreeMaker convenience factories (JCTree.Factory surface is in the
    // trait above; these are the class-level helpers, TreeMaker.java:703-1144).
    // The Attribute/constType based helpers (Annotations(Attribute),
    // Annotation(Attribute), TypeAnnotation(Attribute), Literal(Object) and the
    // AnnotationBuilder visitor) stay registered as NOT PORTED: they need the
    // Attribute model (see the file header).
    // ------------------------------------------------------------------

    /// Java: public JCIdent Ident(Symbol sym)
    pub fn ident_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        sym: crate::ids::SymbolId,
    ) -> TreeId {
        let name = {
            let symbol = ctx.sym(sym);
            let empty = self
                .names
                .as_ref()
                .map(|names| names.empty.clone())
                .expect("TreeMaker.names");
            if name_equals(&symbol.name, &empty) {
                symbol.flat_name()
            } else {
                symbol.name.clone()
            }
        };
        let ty = ctx.sym(sym).ty;
        let tree = self.ident(ctx, name);
        if let TreeKind::Ident { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(sym);
        }
        ctx.tree_mut(tree).ty = ty;
        tree
    }

    /// Java: public JCExpression Select(JCExpression base, Symbol sym)
    pub fn select_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        base: TreeId,
        sym: crate::ids::SymbolId,
    ) -> TreeId {
        let name = ctx.sym(sym).name.clone();
        let ty = ctx.sym(sym).ty;
        let tree = self.select(ctx, base, name);
        if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = Some(sym);
        }
        ctx.tree_mut(tree).ty = ty;
        tree
    }

    /// Java: public JCExpression QualIdent(Symbol sym)
    pub fn qual_ident(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: crate::ids::SymbolId,
    ) -> TreeId {
        if self.is_unqualifiable(ctx, syms, sym) {
            self.ident_symbol(ctx, sym)
        } else {
            let owner = ctx.sym(sym).owner.expect("qualified symbol owner");
            let base = self.qual_ident(ctx, syms, owner);
            self.select_symbol(ctx, base, sym)
        }
    }

    /// Java: public JCExpression Ident(JCVariableDecl param)
    pub fn ident_param(
        &mut self,
        ctx: &mut CompilerContext,
        param: TreeId,
    ) -> TreeId {
        let sym = match &ctx.tree(param).kind {
            TreeKind::VariableDecl { sym, .. } => sym.expect("variable declaration symbol"),
            _ => panic!("ClassCastException: JCVariableDecl expected"),
        };
        self.ident_symbol(ctx, sym)
    }

    /// Java: public List<JCExpression> Idents(List<JCVariableDecl> params)
    pub fn idents(&mut self, ctx: &mut CompilerContext, params: List<TreeId>) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        let mut current = params;
        while current.non_empty() {
            let param = *current.head();
            let ident = self.ident_param(ctx, param);
            result = result.append(ident);
            current = current.tail();
        }
        result
    }


    /// Java: public JCExpression This(Type t)
    pub fn this_expr(
        &mut self,
        ctx: &mut CompilerContext,
        t: crate::ids::TypeId,
    ) -> TreeId {
        let tsym = ctx.typ(t).tsym.expect("type symbol");
        let names = self.names.as_ref().expect("TreeMaker.names");
        let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            flags::FINAL as i64,
            names._this.clone(),
            t,
            tsym,
        ));
        self.ident_symbol(ctx, sym)
    }

    /// Java: public JCExpression QualThis(Type t)
    pub fn qual_this(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: crate::ids::TypeId,
    ) -> TreeId {
        let tsym = ctx.typ(t).tsym.expect("type symbol");
        let names = self.names.as_ref().expect("TreeMaker.names");
        let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            flags::FINAL as i64,
            names._this.clone(),
            t,
            tsym,
        ));
        let base = self.type_expr(ctx, syms, t);
        self.select_symbol(ctx, base, sym)
    }

    /// Java: public JCExpression ClassLiteral(ClassSymbol clazz)
    pub fn class_literal_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        clazz: crate::ids::SymbolId,
    ) -> TreeId {
        let ty = ctx.sym(clazz).ty.expect("class symbol type");
        self.class_literal(ctx, syms, ty)
    }

    /// Java: public JCExpression ClassLiteral(Type t)
    pub fn class_literal(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: crate::ids::TypeId,
    ) -> TreeId {
        let tsym = ctx.typ(t).tsym.expect("type symbol");
        let names = self.names.as_ref().expect("TreeMaker.names");
        let lit = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            (flags::STATIC | flags::PUBLIC | flags::FINAL) as i64,
            names._class.clone(),
            t,
            tsym,
        ));
        let base = self.type_expr(ctx, syms, t);
        self.select_symbol(ctx, base, lit)
    }

    /// Java: public JCIdent Super(Type t, TypeSymbol owner)
    pub fn super_expr(
        &mut self,
        ctx: &mut CompilerContext,
        t: crate::ids::TypeId,
        owner: crate::ids::SymbolId,
    ) -> TreeId {
        let names = self.names.as_ref().expect("TreeMaker.names");
        let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            flags::FINAL as i64,
            names._super.clone(),
            t,
            owner,
        ));
        self.ident_symbol(ctx, sym)
    }

    /// Java: public JCMethodInvocation App(JCExpression meth)
    pub fn app(&mut self, ctx: &mut CompilerContext, meth: TreeId) -> TreeId {
        self.app_with_args(ctx, meth, List::nil())
    }

    /// Java: public JCMethodInvocation App(JCExpression meth, List<JCExpression> args)
    pub fn app_with_args(
        &mut self,
        ctx: &mut CompilerContext,
        meth: TreeId,
        args: List<TreeId>,
    ) -> TreeId {
        let tree = self.apply(ctx, List::nil(), meth, args);
        let return_type = ctx
            .tree(meth)
            .ty
            .and_then(|ty| ctx.typ(ty).get_return_type(ctx));
        ctx.tree_mut(tree).ty = return_type;
        tree
    }

    /// Java: public JCExpression Create(Symbol ctor, List<JCExpression> args)
    pub fn create(
        &mut self,
        ctx: &mut CompilerContext,
        types: &Types,
        syms: &crate::code::symtab::Symtab,
        ctor: crate::ids::SymbolId,
        args: List<TreeId>,
    ) -> TreeId {
        let owner = ctx.sym(ctor).owner.expect("constructor owner");
        let owner_type = ctx.sym(owner).ty.expect("constructor owner type");
        let t = types.erasure(ctx, syms, owner_type);
        let class_expr = self.type_expr(ctx, syms, t);
        let tree = self.new_class(ctx, None, List::nil(), class_expr, args, None);
        if let TreeKind::NewClass { constructor, .. } = &mut ctx.tree_mut(tree).kind {
            *constructor = Some(ctor);
        }
        ctx.tree_mut(tree).ty = Some(t);
        tree
    }

    /// Java: public JCExpression Type(Type t)
    pub fn type_expr(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        t: crate::ids::TypeId,
    ) -> TreeId {
        let tag = ctx.typ(t).get_tag();
        let tp = match tag {
            TypeTag::BYTE
            | TypeTag::CHAR
            | TypeTag::SHORT
            | TypeTag::INT
            | TypeTag::LONG
            | TypeTag::FLOAT
            | TypeTag::DOUBLE
            | TypeTag::BOOLEAN
            | TypeTag::VOID => self.type_ident(ctx, tag),
            TypeTag::TYPEVAR => {
                let tsym = ctx.typ(t).tsym.expect("type variable symbol");
                self.ident_symbol(ctx, tsym)
            }
            TypeTag::WILDCARD => {
                let (kind, bound) = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::WildcardType {
                        type_, kind, ..
                    } => (*kind, *type_),
                    _ => panic!("ClassCastException: WildcardType expected"),
                };
                let kind_tree = self.type_bound_kind(ctx, kind);
                let bound_tree = if kind == BoundKind::UNBOUND {
                    None
                } else {
                    Some(self.type_expr(ctx, syms, bound.expect("wildcard bound")))
                };
                self.wildcard(ctx, kind_tree, bound_tree)
            }
            TypeTag::CLASS => match &ctx.typ(t).kind {
                crate::code::type_::TypeKindData::UnionClassType {
                    alternatives_field,
                    ..
                } => {
                    let alternatives = alternatives_field.clone();
                    let mut trees: List<TreeId> = List::nil();
                    let mut current = alternatives;
                    while current.non_empty() {
                        let alt = *current.head();
                        let alt_tree = self.type_expr(ctx, syms, alt);
                        trees = trees.append(alt_tree);
                        current = current.tail();
                    }
                    self.type_union(ctx, trees)
                }
                crate::code::type_::TypeKindData::IntersectionClassType {
                    all_interfaces,
                    interfaces_field,
                    supertype_field,
                    ..
                } => {
                    let components = if *all_interfaces {
                        interfaces_field.clone().unwrap_or_else(List::nil)
                    } else {
                        interfaces_field
                            .clone()
                            .unwrap_or_else(List::nil)
                            .prepend(supertype_field.expect("intersection supertype"))
                    };
                    let mut trees: List<TreeId> = List::nil();
                    let mut current = components;
                    while current.non_empty() {
                        let component = *current.head();
                        let component_tree = self.type_expr(ctx, syms, component);
                        trees = trees.append(component_tree);
                        current = current.tail();
                    }
                    self.type_intersection(ctx, trees)
                }
                _ => {
                    let outer = ctx.typ(t).get_enclosing_type();
                    let clazz = match outer {
                        Some(outer) if ctx.typ(outer).has_tag(TypeTag::CLASS) => {
                            let tsym = ctx.typ(t).tsym.expect("class type symbol");
                            let owner = ctx.sym(tsym).owner.expect("type owner");
                            if ctx.sym(owner).kind == crate::code::kinds::Kind::TYP {
                                let outer_tree = self.type_expr(ctx, syms, outer);
                                self.select_symbol(ctx, outer_tree, tsym)
                            } else {
                                self.qual_ident(ctx, syms, tsym)
                            }
                        }
                        _ => {
                            let tsym = ctx.typ(t).tsym.expect("class type symbol");
                            self.qual_ident(ctx, syms, tsym)
                        }
                    };
                    let args = class_type_arguments(ctx, t);
                    if args.is_empty() {
                        clazz
                    } else {
                        let mut type_trees: List<TreeId> = List::nil();
                        let mut current = args;
                        while current.non_empty() {
                            let arg = *current.head();
                            let arg_tree = self.type_expr(ctx, syms, arg);
                            type_trees = type_trees.append(arg_tree);
                            current = current.tail();
                        }
                        self.type_apply(ctx, clazz, type_trees)
                    }
                }
            },
            TypeTag::ARRAY => {
                let elemtype = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => {
                        elemtype.expect("array element type")
                    }
                    _ => panic!("ClassCastException: ArrayType expected"),
                };
                let elem_tree = self.type_expr(ctx, syms, elemtype);
                self.type_array(ctx, elem_tree)
            }
            TypeTag::ERROR => self.type_ident(ctx, TypeTag::ERROR),
            _ => panic!("AssertionError: unexpected type"),
        };
        ctx.tree_mut(tp).ty = Some(t);
        tp
    }

    /// Java: public List<JCExpression> Types(List<Type> ts)
    pub fn types(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        ts: List<crate::ids::TypeId>,
    ) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        let mut current = ts;
        while current.non_empty() {
            let t = *current.head();
            let tree = self.type_expr(ctx, syms, t);
            result = result.append(tree);
            current = current.tail();
        }
        result
    }

    /// Java: public JCVariableDecl VarDef(VarSymbol v, JCExpression init)
    pub fn var_def_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        v: crate::ids::SymbolId,
        init: StdOption<TreeId>,
    ) -> TreeId {
        let ty = ctx.sym(v).ty.expect("variable type");
        let annotations = self.raw_annotations(ctx, syms, v);
        let mods = self.modifiers_with_annotations(ctx, ctx.sym(v).flags_field, annotations);
        let name = ctx.sym(v).name.clone();
        let vartype = self.type_expr(ctx, syms, ty);
        let tree = self.var_def(ctx, mods, name, Some(vartype), init);
        if let TreeKind::VariableDecl { sym, .. } = &mut ctx.tree_mut(tree).kind {
            *sym = Some(v);
        }
        ctx.tree_mut(tree).ty = Some(ty);
        tree
    }

    /// Java: public JCMethodDecl MethodDef(MethodSymbol m, JCBlock body)
    pub fn method_def_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        m: crate::ids::SymbolId,
        body: StdOption<TreeId>,
    ) -> TreeId {
        let ty = ctx.sym(m).ty.expect("method type");
        self.method_def_symbol_with_type(ctx, syms, m, ty, body)
    }

    /// Java: public JCMethodDecl MethodDef(MethodSymbol m, Type mtype, JCBlock body)
    pub fn method_def_symbol_with_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        m: crate::ids::SymbolId,
        mtype: crate::ids::TypeId,
        body: StdOption<TreeId>,
    ) -> TreeId {
        let annotations = self.raw_annotations(ctx, syms, m);
        let mods = self.modifiers_with_annotations(ctx, ctx.sym(m).flags_field, annotations);
        let name = ctx.sym(m).name.clone();
        let names = self.names.as_ref().expect("TreeMaker.names").clone();
        let restype = if !name_equals(&name, &names.init) {
            ctx.typ(mtype)
                .get_return_type(ctx)
                .map(|restype| self.type_expr(ctx, syms, restype))
        } else {
            None
        };
        let typarams = ctx
            .typ(mtype)
            .get_type_arguments()
            .iter()
            .map(|tvar| {
                let name = ctx
                    .typ(tvar)
                    .tsym
                    .map(|tsym| ctx.sym(tsym).name.clone())
                    .expect("type variable symbol");
                let bounds = ctx.typ(tvar).get_bounds();
                let mut bound_trees: List<TreeId> = List::nil();
                let mut current = bounds;
                while current.non_empty() {
                    let bound = *current.head();
                    let bound_tree = self.type_expr(ctx, syms, bound);
                    bound_trees = bound_trees.append(bound_tree);
                    current = current.tail();
                }
                let typaram = self.type_parameter(ctx, name, bound_trees);
                ctx.tree_mut(typaram).ty = Some(tvar);
                typaram
            })
            .collect::<Vec<TreeId>>();
        let typarams = List::from_iterable(typarams);
        let param_types = ctx.typ(mtype).get_parameter_types(ctx);
        let params = self.params(ctx, syms, param_types, m);
        let thrown_types = ctx.typ(mtype).get_thrown_types(ctx);
        let thrown = self.types(ctx, syms, thrown_types);
        let tree = self.method_def(
            ctx,
            mods,
            name,
            restype,
            typarams,
            None,
            params,
            thrown,
            body,
            None,
        );
        if let TreeKind::MethodDecl { sym, .. } = &mut ctx.tree_mut(tree).kind {
            *sym = Some(m);
        }
        ctx.tree_mut(tree).ty = Some(mtype);
        tree
    }

    /// Java: public JCTypeParameter TypeParam(Name name, TypeVar tvar)
    pub fn type_param(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        name: Name,
        tvar: crate::ids::TypeId,
    ) -> TreeId {
        let bounds = ctx.typ(tvar).get_bounds();
        let bound_trees = self.types(ctx, syms, bounds);
        let tree = self.type_parameter(ctx, name, bound_trees);
        ctx.tree_mut(tree).ty = Some(tvar);
        tree
    }

    /// Java: public List<JCTypeParameter> TypeParams(List<Type> typarams)
    pub fn type_params(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        typarams: List<crate::ids::TypeId>,
    ) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        let mut current = typarams;
        while current.non_empty() {
            let tvar = *current.head();
            let name = ctx
                .typ(tvar)
                .tsym
                .map(|tsym| ctx.sym(tsym).name.clone())
                .expect("type variable symbol");
            let tree = self.type_param(ctx, syms, name, tvar);
            result = result.append(tree);
            current = current.tail();
        }
        result
    }

    /// Java: public JCVariableDecl Param(Name name, Type argtype, Symbol owner)
    pub fn param(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        name: Name,
        argtype: crate::ids::TypeId,
        owner: crate::ids::SymbolId,
    ) -> TreeId {
        let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            flags::PARAMETER,
            name,
            argtype,
            owner,
        ));
        self.var_def_symbol(ctx, syms, sym, None)
    }

    /// Java: public List<JCVariableDecl> Params(List<Type> argtypes, Symbol owner)
    pub fn params(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        argtypes: List<crate::ids::TypeId>,
        owner: crate::ids::SymbolId,
    ) -> List<TreeId> {
        let owner_sym = ctx.sym(owner);
        let method_params = match &owner_sym.kind_data {
            crate::code::symbol::SymbolKind::MethodSymbol { params, .. } => Some(params.clone()),
            _ => None,
        };
        let matching_params = match method_params {
            Some(method_params) if method_params.length() == argtypes.length() => {
                Some(method_params)
            }
            _ => None,
        };
        if let Some(method_params) = matching_params {
            let mut result: List<TreeId> = List::nil();
            let mut current = method_params;
            while current.non_empty() {
                let param = *current.head();
                let tree = self.var_def_symbol(ctx, syms, param, None);
                result = result.append(tree);
                current = current.tail();
            }
            return result;
        }
        let mut result: List<TreeId> = List::nil();
        let mut index = 0;
        let mut current = argtypes;
        while current.non_empty() {
            let argtype = *current.head();
            let name = self.param_name(index);
            index += 1;
            let tree = self.param(ctx, syms, name, argtype, owner);
            result = result.append(tree);
            current = current.tail();
        }
        result
    }

    /// Java: public JCStatement Call(JCExpression apply)
    pub fn call_statement(&mut self, ctx: &mut CompilerContext, apply: TreeId) -> TreeId {
        let is_void = ctx
            .tree(apply)
            .ty
            .map(|ty| ctx.typ(ty).has_tag(TypeTag::VOID))
            .unwrap_or(false);
        if is_void {
            self.exec(ctx, apply)
        } else {
            self.return_(ctx, Some(apply))
        }
    }

    /// Java: public JCStatement Assignment(Symbol v, JCExpression rhs)
    pub fn assignment(
        &mut self,
        ctx: &mut CompilerContext,
        v: crate::ids::SymbolId,
        rhs: TreeId,
    ) -> TreeId {
        let lhs = self.ident_symbol(ctx, v);
        let assign = self.assign(ctx, lhs, rhs);
        let ty = ctx.sym(v).ty;
        ctx.tree_mut(assign).ty = ty;
        self.exec(ctx, assign)
    }

    /// Java: public JCArrayAccess Indexed(Symbol v, JCExpression index)
    pub fn indexed_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        v: crate::ids::SymbolId,
        index: TreeId,
    ) -> TreeId {
        let array = self.qual_ident(ctx, syms, v);
        let tree = self.indexed(ctx, array, index);
        let elemtype = ctx
            .sym(v)
            .ty
            .and_then(|ty| match &ctx.typ(ty).kind {
                crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => *elemtype,
                _ => None,
            });
        ctx.tree_mut(tree).ty = elemtype;
        tree
    }

    /// Java: public JCTypeCast TypeCast(Type type, JCExpression expr)
    pub fn type_cast_typed(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        ty: crate::ids::TypeId,
        expr: TreeId,
    ) -> TreeId {
        let clazz = self.type_expr(ctx, syms, ty);
        let tree = self.type_cast(ctx, clazz, expr);
        ctx.tree_mut(tree).ty = Some(ty);
        tree
    }

    /// Java: public JCLiteral Literal(Object value) — TypeTag inferred from the
    /// wrapper type (the Attribute/constType part of the upstream method is
    /// registered NOT PORTED, see the file header; the constant type is not set
    /// until the Types.constType).
    pub fn literal_object(&mut self, ctx: &mut CompilerContext, value: LiteralValue) -> TreeId {
        let typetag = match &value {
            LiteralValue::Integer(_) => TypeTag::INT,
            LiteralValue::Byte(_) => TypeTag::BYTE,
            LiteralValue::Short(_) => TypeTag::SHORT,
            LiteralValue::Long(_) => TypeTag::LONG,
            LiteralValue::Float(_) => TypeTag::FLOAT,
            LiteralValue::Double(_) => TypeTag::DOUBLE,
            LiteralValue::Character(_) => TypeTag::CHAR,
            LiteralValue::Boolean(_) => TypeTag::BOOLEAN,
            LiteralValue::String(_) => TypeTag::CLASS,
        };
        let tree = self.literal(ctx, typetag, Some(value.clone()));
        // Java: Literal(Object) sets the constant type from the value
        // (TreeMaker.java): String -> stringType, Integer -> intType,
        // Long -> longType, Byte -> byteType, Character -> charType, Double ->
        // doubleType, Float -> floatType, Short -> shortType, Boolean ->
        // booleanType.constType(0/1), each via constType(value).
        let (type_id, constant) = {
            let syms_rc = self.syms.as_ref().expect("TreeMaker.syms").clone();
            let syms = syms_rc.borrow();
            match &value {
                LiteralValue::Integer(v) => {
                    (syms.int_type, crate::util::constants::ConstantValue::Integer(*v))
                }
                LiteralValue::Byte(v) => {
                    (syms.byte_type, crate::util::constants::ConstantValue::Byte(*v))
                }
                LiteralValue::Short(v) => {
                    (syms.short_type, crate::util::constants::ConstantValue::Short(*v))
                }
                LiteralValue::Long(v) => {
                    (syms.long_type, crate::util::constants::ConstantValue::Long(*v))
                }
                LiteralValue::Float(v) => {
                    (syms.float_type, crate::util::constants::ConstantValue::Float(*v))
                }
                LiteralValue::Double(v) => {
                    (syms.double_type, crate::util::constants::ConstantValue::Double(*v))
                }
                LiteralValue::Character(v) => {
                    (syms.char_type, crate::util::constants::ConstantValue::Character(*v))
                }
                LiteralValue::Boolean(v) => (
                    syms.boolean_type,
                    crate::util::constants::ConstantValue::Integer(if *v { 1 } else { 0 }),
                ),
                LiteralValue::String(v) => (
                    syms.string_type,
                    crate::util::constants::ConstantValue::String(v.clone()),
                ),
            }
        };
        let const_type = {
            let typ = ctx.typ(type_id).const_type(constant);
            ctx.alloc_type(typ)
        };
        ctx.tree_mut(tree).ty = Some(const_type);
        tree
    }

    // ------------------------------------------------------------------
    // Helper methods (TreeMaker.java).
    // ------------------------------------------------------------------

    /// Java: boolean isUnqualifiable(Symbol sym)
    fn is_unqualifiable(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: crate::ids::SymbolId,
    ) -> bool {
        let names = self.names.as_ref().expect("TreeMaker.names").clone();
        let symbol_name = ctx.sym(sym).name.clone();
        let owner = ctx.sym(sym).owner;
        let owner_kind = owner.map(|owner| ctx.sym(owner).kind);
        if name_equals(&symbol_name, &names.empty) || owner.is_none() {
            return true;
        }
        let root_package = self.root_package.expect("TreeMaker.rootPackage");
        if owner == Some(root_package) {
            return true;
        }
        if matches!(
            owner_kind,
            Some(crate::code::kinds::Kind::MTH) | Some(crate::code::kinds::Kind::VAR)
        ) {
            return true;
        }
        if ctx.sym(sym).kind == crate::code::kinds::Kind::TYP && self.toplevel.is_some() {
            // Java: the named-import, package-member and star-import scopes are
            // queried in that order; the first scope with a candidate decides
            // (the symbol must be the only candidate).
            let toplevel = self.toplevel.expect("toplevel");
            let types = self
                .types
                .as_ref()
                .expect("AssertionError: TreeMaker.types")
                .clone();
            let (named_import_scope, star_import_scope, packge) =
                match &ctx.tree(toplevel).kind {
                    TreeKind::CompilationUnit {
                        named_import_scope,
                        star_import_scope,
                        packge,
                        ..
                    } => (*named_import_scope, *star_import_scope, *packge),
                    _ => panic!("ClassCastException: JCCompilationUnit expected"),
                };
            let name = ctx.sym(sym).name.clone();
            let lookup = |ctx: &mut CompilerContext,
                              scope: crate::ids::ScopeId|
             -> StdOption<bool> {
                // Java: Scope.getSymbolsByName reads the Context Types/Symtab
                // services (Scope.java); the port passes them explicitly
                // so a FilterImportScope never borrows a shared Symtab.
                let candidates = ctx.scope_get_symbols_by_name_ctx(
                    scope,
                    &name,
                    crate::code::scope::LookupKind::NON_RECURSIVE,
                    &|_| true,
                    &types,
                    syms,
                    None,
                );
                if candidates.is_empty() {
                    return None;
                }
                Some(candidates.len() == 1 && candidates[0] == sym)
            };
            if let StdOption::Some(scope) = named_import_scope
                && let StdOption::Some(result) = lookup(ctx, scope) {
                    return result;
                }
            if let StdOption::Some(packge) = packge
                && let StdOption::Some(members) = ctx.sym(packge).members() {
                    if let StdOption::Some(result) = lookup(ctx, members) {
                        return result;
                    }
                }
            if let StdOption::Some(scope) = star_import_scope
                && let StdOption::Some(result) = lookup(ctx, scope) {
                    return result;
                }
        }
        false
    }

    /// Java: public Name paramName(int i) { return names.fromString("x" + i); }
    pub fn param_name(&mut self, i: i32) -> Name {
        self.str_name(&format!("x{i}"))
    }

    /// Java: public Name typaramName(int i) { return names.fromString("A" + i); }
    pub fn typaram_name(&mut self, i: i32) -> Name {
        self.str_name(&format!("A{i}"))
    }

    fn str_name(&mut self, text: &str) -> Name {
        let names = self.names.as_ref().expect("TreeMaker.names");
        names.from_utf(text.as_bytes())
    }

    /// Java: Symbol.getRawAttributes() -> Annotations(List<Attribute.Compound>)
    fn raw_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        sym: crate::ids::SymbolId,
    ) -> List<TreeId> {
        let attributes = ctx.sym(sym).get_raw_attributes();
        self.annotations_attributes(ctx, syms, attributes)
    }

    // ------------------------------------------------------------------
    // Java: TreeMaker.AnnotationBuilder + Annotations/Annotation/
    // TypeAnnotation/Literal(Object) (TreeMaker.java).
    // ------------------------------------------------------------------

    /// Java: public List<JCAnnotation> Annotations(List<Attribute.Compound>
    /// attributes)
    pub fn annotations_attributes(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attributes: List<crate::ids::AttributeId>,
    ) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        let mut current = attributes;
        while current.non_empty() {
            let attribute = *current.head();
            let annotation = self.annotation_attribute(ctx, syms, attribute);
            result = result.append(annotation);
            current = current.tail();
        }
        result
    }

    /// Java: public JCAnnotation Annotation(Attribute a) —
    /// annotationBuilder.translate((Attribute.Compound) a)
    pub fn annotation_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        self.visit_compound_internal(ctx, syms, attribute)
    }

    /// Java: public JCAnnotation TypeAnnotation(Attribute a) —
    /// annotationBuilder.translate((Attribute.TypeCompound) a)
    pub fn type_annotation_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        self.visit_type_compound_internal(ctx, syms, attribute)
    }

    /// Java: AnnotationBuilder.translate(Attribute a) / the Visitor dispatch.
    pub fn translate_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        use crate::code::attribute::AttributeKind;
        match &ctx.attr(attribute).kind {
            AttributeKind::Constant { .. } => self.visit_constant_attribute(ctx, attribute),
            AttributeKind::Class { .. } => self.visit_class_attribute(ctx, syms, attribute),
            AttributeKind::Enum { .. } => self.visit_enum_attribute(ctx, syms, attribute),
            AttributeKind::Error | AttributeKind::UnresolvedClass { .. } => {
                self.visit_error_attribute(ctx, syms, attribute)
            }
            AttributeKind::Compound { .. } => self.visit_compound_attribute(ctx, syms, attribute),
            AttributeKind::TypeCompound { .. } => {
                self.visit_type_compound_internal(ctx, syms, attribute)
            }
            AttributeKind::Array { .. } => self.visit_array_attribute(ctx, syms, attribute),
        }
    }

    /// Java: public void visitConstant(Attribute.Constant v) (AnnotationBuilder)
    fn visit_constant_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        use crate::util::constants::ConstantValue;
        let (typetag, value) = match &ctx.attr(attribute).kind {
            crate::code::attribute::AttributeKind::Constant { value } => match value {
                ConstantValue::Integer(value) => (TypeTag::INT, LiteralValue::Integer(*value)),
                ConstantValue::Byte(value) => (TypeTag::BYTE, LiteralValue::Byte(*value)),
                ConstantValue::Short(value) => (TypeTag::SHORT, LiteralValue::Short(*value)),
                ConstantValue::Long(value) => (TypeTag::LONG, LiteralValue::Long(*value)),
                ConstantValue::Float(value) => (TypeTag::FLOAT, LiteralValue::Float(*value)),
                ConstantValue::Double(value) => (TypeTag::DOUBLE, LiteralValue::Double(*value)),
                ConstantValue::Character(value) => {
                    (TypeTag::CHAR, LiteralValue::Character(*value))
                }
                ConstantValue::Boolean(value) => {
                    (TypeTag::BOOLEAN, LiteralValue::Boolean(*value))
                }
                ConstantValue::String(value) => (TypeTag::CLASS, LiteralValue::String(value.clone())),
            },
            _ => panic!("ClassCastException: not a constant attribute"),
        };
        self.literal(ctx, typetag, Some(value))
    }

    /// Java: public void visitClass(Attribute.Class clazz) (AnnotationBuilder)
    fn visit_class_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        let class_type = match &ctx.attr(attribute).kind {
            crate::code::attribute::AttributeKind::Class { class_type } => {
                class_type.expect("class attribute type")
            }
            _ => panic!("ClassCastException: not a class attribute"),
        };
        let tree = self.class_literal(ctx, syms, class_type);
        let class_type = syms.class_type;
        ctx.tree_mut(tree).ty = Some(class_type);
        tree
    }

    /// Java: public void visitEnum(Attribute.Enum e) (AnnotationBuilder)
    fn visit_enum_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        let value = match &ctx.attr(attribute).kind {
            crate::code::attribute::AttributeKind::Enum { value } => value.expect("enum value"),
            _ => panic!("ClassCastException: not an enum attribute"),
        };
        self.qual_ident(ctx, syms, value)
    }

    /// Java: public void visitError(Attribute.Error e) (AnnotationBuilder)
    fn visit_error_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        match &ctx.attr(attribute).kind {
            crate::code::attribute::AttributeKind::UnresolvedClass { class_type } => {
                let class_type = class_type.expect("unresolved class type");
                self.class_literal(ctx, syms, class_type)
            }
            _ => self.erroneous(ctx, List::nil()),
        }
    }

    /// Java: public void visitCompound(Attribute.Compound compound)
    /// (AnnotationBuilder dispatch)
    fn visit_compound_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        self.visit_compound_internal(ctx, syms, attribute)
    }

    /// Java: public JCAnnotation visitCompoundInternal(Attribute.Compound
    /// compound) (AnnotationBuilder)
    fn visit_compound_internal(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        let (values, compound_type) = {
            let attr = ctx.attr(attribute);
            let values = match &attr.kind {
                crate::code::attribute::AttributeKind::Compound { values, .. } => values.clone(),
                _ => panic!("ClassCastException: not a compound attribute"),
            };
            (values, attr.ty.expect("compound type"))
        };
        let mut args: List<TreeId> = List::nil();
        let mut current = values;
        while current.non_empty() {
            let pair = current.head();
            let value_tree = self.translate_attribute(ctx, syms, pair.snd);
            let method = self.ident_symbol(ctx, pair.fst);
            let assign = self.assign(ctx, method, value_tree);
            let value_type = ctx.tree(value_tree).ty;
            ctx.tree_mut(assign).ty = value_type;
            args = args.append(assign);
            current = current.tail();
        }
        let annotation_type = self.type_expr(ctx, syms, compound_type);
        self.annotation(ctx, annotation_type, args)
    }

    /// Java: public JCAnnotation visitTypeCompoundInternal(Attribute.TypeCompound
    /// compound) (AnnotationBuilder)
    fn visit_type_compound_internal(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        let (values, compound_type) = {
            let attr = ctx.attr(attribute);
            let values = match &attr.kind {
                crate::code::attribute::AttributeKind::TypeCompound { values, .. } => {
                    values.clone()
                }
                _ => panic!("ClassCastException: not a type compound attribute"),
            };
            (values, attr.ty.expect("type compound type"))
        };
        let mut args: List<TreeId> = List::nil();
        let mut current = values;
        while current.non_empty() {
            let pair = current.head();
            let value_tree = self.translate_attribute(ctx, syms, pair.snd);
            let method = self.ident_symbol(ctx, pair.fst);
            let assign = self.assign(ctx, method, value_tree);
            let value_type = ctx.tree(value_tree).ty;
            ctx.tree_mut(assign).ty = value_type;
            args = args.append(assign);
            current = current.tail();
        }
        let annotation_type = self.type_expr(ctx, syms, compound_type);
        self.type_annotation(ctx, annotation_type, args)
    }

    /// Java: public void visitArray(Attribute.Array array) (AnnotationBuilder)
    fn visit_array_attribute(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        attribute: crate::ids::AttributeId,
    ) -> TreeId {
        let (values, array_type) = {
            let attr = ctx.attr(attribute);
            let values = match &attr.kind {
                crate::code::attribute::AttributeKind::Array { values } => values.clone(),
                _ => panic!("ClassCastException: not an array attribute"),
            };
            (values, attr.ty.expect("array type"))
        };
        let mut elems: List<TreeId> = List::nil();
        for value in values {
            let elem = self.translate_attribute(ctx, syms, value);
            elems = elems.append(elem);
        }
        let new_array = self.new_array(ctx, None, List::nil(), Some(elems));
        ctx.tree_mut(new_array).ty = Some(array_type);
        new_array
    }

    /// Java: public static JCLiteral Literal(Object value) (TreeMaker.java) —
    /// the TypeTag/type inference part; see the file header for the Byte/Short
    /// registration.
    pub fn literal_constant(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,
        value: crate::util::constants::ConstantValue,
    ) -> TreeId {
        use crate::util::constants::ConstantValue;
        let (typetag, literal_value, base_type) = match &value {
            ConstantValue::Integer(value) => {
                (TypeTag::INT, LiteralValue::Integer(*value), syms.int_type)
            }
            ConstantValue::Byte(value) => {
                (TypeTag::BYTE, LiteralValue::Byte(*value), syms.byte_type)
            }
            ConstantValue::Short(value) => {
                (TypeTag::SHORT, LiteralValue::Short(*value), syms.short_type)
            }
            ConstantValue::Long(value) => (TypeTag::LONG, LiteralValue::Long(*value), syms.long_type),
            ConstantValue::Float(value) => {
                (TypeTag::FLOAT, LiteralValue::Float(*value), syms.float_type)
            }
            ConstantValue::Double(value) => {
                (TypeTag::DOUBLE, LiteralValue::Double(*value), syms.double_type)
            }
            ConstantValue::Character(value) => {
                (TypeTag::CHAR, LiteralValue::Character(*value), syms.char_type)
            }
            ConstantValue::Boolean(value) => {
                (TypeTag::BOOLEAN, LiteralValue::Boolean(*value), syms.boolean_type)
            }
            ConstantValue::String(value) => {
                (TypeTag::CLASS, LiteralValue::String(value.clone()), syms.string_type)
            }
        };
        let tree = self.literal(ctx, typetag, Some(literal_value));
        let constant_type = ctx.type_const_type(base_type, value);
        ctx.tree_mut(tree).ty = Some(constant_type);
        tree
    }

}

/// Java: Name equality (Name.contentEquals).
fn name_equals(a: &Name, b: &Name) -> bool {
    a.content_equals(&b.to_java_string())
}

/// Java: ClassType.getTypeArguments() / UnionClassType / IntersectionClassType
/// (the local ClassType stores Option<List<TypeId>>).
fn class_type_arguments(ctx: &CompilerContext, t: crate::ids::TypeId) -> List<crate::ids::TypeId> {
    match &ctx.typ(t).kind {
        crate::code::type_::TypeKindData::ClassType { typarams_field, .. }
        | crate::code::type_::TypeKindData::UnionClassType { typarams_field, .. }
        | crate::code::type_::TypeKindData::IntersectionClassType { typarams_field, .. } => {
            typarams_field.clone().unwrap_or_else(List::nil)
        }
        _ => List::nil(),
    }
}


impl TreeMaker {
    /// adapter helper: public wrapper of the private block factory (JCTree.java).
    pub fn block_pub(&mut self, ctx: &mut CompilerContext, flags: i64, stats: List<TreeId>) -> TreeId {
        self.block(ctx, flags, stats)
    }

    /// adapter helper: public wrapper of the private lambda factory (JCTree.java).
    pub fn lambda_pub(&mut self, ctx: &mut CompilerContext, params: List<TreeId>, body: TreeId) -> TreeId {
        self.lambda(ctx, params, body)
    }
}


impl TreeMaker {
    /// Java: public JCExpression Type(Type t) (TreeMaker.java).
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - t: the represented type.
    ///
    /// # Returns
    ///   - the type tree for t.
    ///
    /// # Panics
    ///   - non-class/array/typevar/wildcard/primitive tags assert.
    ///
    /// Upstream: TreeMaker.java:790-860.
    pub fn type_from_type(
        &mut self,
        ctx: &mut CompilerContext,
        syms: &crate::code::symtab::Symtab,

        t: crate::ids::TypeId,
    ) -> TreeId {
        let tag = ctx.typ(t).get_tag();
        let result = match tag {
            TypeTag::BYTE | TypeTag::CHAR | TypeTag::SHORT | TypeTag::INT | TypeTag::LONG
            | TypeTag::FLOAT | TypeTag::DOUBLE | TypeTag::BOOLEAN | TypeTag::VOID => {
                self.type_ident(ctx, tag)
            }
            TypeTag::TYPEVAR => {
                // The identifier publishes the type-variable symbol (unlike an
                // identifier created from a name, which leaves tree.sym null and
                // later breaks Lower.visitIdent).
                let tsym = ctx.typ(t).tsym.expect("type variable symbol");
                self.ident_symbol(ctx, tsym)
            }
            TypeTag::WILDCARD => {
                let (inner, kind) = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::WildcardType { type_, kind, .. } => (*type_, *kind),
                    _ => (None, crate::code::bound_kind::BoundKind::UNBOUND),
                };
                let kind_tree = self.type_bound_kind(ctx, kind);
                let inner_tree = if kind == crate::code::bound_kind::BoundKind::UNBOUND {
                    None
                } else {
                    inner.map(|inner| self.type_from_type(ctx, syms, inner))
                };
                self.wildcard(ctx, kind_tree, inner_tree)
            }
            TypeTag::CLASS => {
                if ctx.typ(t).is_union() {
                    let alternatives = ctx.typ(t).get_alternative_types();
                    let mut components: List<TreeId> = List::nil();
                    let mut current = alternatives;
                    while current.non_empty() {
                        let alternative = *current.head();
                        let tree = self.type_from_type(ctx, syms, alternative);
                        components = components.append(tree);
                        current = current.tail();
                    }
                    let tree = self.type_union(ctx, components);
                    ctx.tree_mut(tree).ty = Some(t);
                    return tree;
                }
                if ctx.typ(t).is_intersection() {
                    let components_types = ctx.typ(t).get_explicit_components();
                    let mut components: List<TreeId> = List::nil();
                    let mut current = components_types;
                    while current.non_empty() {
                        let component = *current.head();
                        let tree = self.type_from_type(ctx, syms, component);
                        components = components.append(tree);
                        current = current.tail();
                    }
                    let tree = self.type_intersection(ctx, components);
                    ctx.tree_mut(tree).ty = Some(t);
                    return tree;
                }
                let tsym = ctx.typ(t).tsym.expect("class type symbol");
                let outer = ctx.typ(t).get_enclosing_type();
                let owner_kind = ctx.sym(tsym).owner.map(|owner| ctx.sym(owner).kind);
                let clazz = match outer {
                    Some(outer)
                        if ctx.typ(outer).has_tag(TypeTag::CLASS)
                            && owner_kind == Some(crate::code::kinds::Kind::TYP) =>
                    {
                        // Java: Select(Type(outer), t.tsym)
                        let selected = self.type_from_type(ctx, syms, outer);
                        let name = ctx.sym(tsym).name.clone();
                        self.select(ctx, selected, name)
                    }
                    _ => self.qual_ident(ctx, syms, tsym),
                };
                let args = ctx.typ(t).get_type_arguments();
                if args.is_empty() {
                    clazz
                } else {
                    let mut argument_trees: List<TreeId> = List::nil();
                    let mut current = args;
                    while current.non_empty() {
                        let argument = *current.head();
                        let tree = self.type_from_type(ctx, syms, argument);
                        argument_trees = argument_trees.append(tree);
                        current = current.tail();
                    }
                    self.type_apply(ctx, clazz, argument_trees)
                }
            }
            TypeTag::ARRAY => {
                let elemtype = match &ctx.typ(t).kind {
                    crate::code::type_::TypeKindData::ArrayType { elemtype, .. } => {
                        elemtype.expect("array element type")
                    }
                    _ => panic!("AssertionError: array type expected"),
                };
                let elem_tree = self.type_from_type(ctx, syms, elemtype);
                self.type_array(ctx, elem_tree)
            }
            other => panic!("AssertionError: unexpected type tag: {:?}", other),
        };
        // The tree type is set on the returned identifier.
        ctx.tree_mut(result).ty = Some(t);
        result
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
        // Java: TreeMaker holds the Symtab used by the Attribute/constType
        // helpers (TreeMaker.java); the upstream constructor always
        // receives the context services, so the test maker is wired with syms.
        let (mut ctx, _syms, _types, mut maker) = new_maker_with_syms();
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
            crate::util::assert::error()
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
        let (mut ctx, _syms, _types, mut maker) = new_maker_with_syms();
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
        let (mut ctx, _syms, _types, mut maker) = new_maker_with_syms();
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
        // (unimplemented in the pretty , not part of this task). The expression/statement
        // printers are complete, so the integration test covers those.
        let (mut ctx, _syms, _types, mut maker) = new_maker_with_syms();
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

        let (mut ctx, _syms, _types, mut maker) = new_maker_with_syms();
        let lit = maker
            .at(10)
            .literal_object(&mut ctx, LiteralValue::Integer(42));
        let ret = maker.at(8).return_(&mut ctx, Some(lit));
        let mut pretty = Pretty::new(false);
        pretty.print_stat(&ctx, ret);
        let printed = pretty.get_out().to_string_lossy();
        assert!(printed.contains("return 42"), "{printed}");
    }
    fn new_maker_with_syms() -> (
        CompilerContext,
        Rc<std::cell::RefCell<crate::code::symtab::Symtab>>,
        Rc<crate::code::types::Types>,
        TreeMaker,
    ) {
        let names = Rc::new(Names::new(false));
        let mut ctx = CompilerContext::new();
        let syms = Rc::new(std::cell::RefCell::new(crate::code::symtab::Symtab::new(
            &mut ctx,
            Rc::clone(&names),
            crate::jvm::target::Target::JDK1_17,
            crate::code::source::Source::JDK17,
        )));
        let types = Rc::new(crate::code::types::Types::with_names(Rc::clone(&names)));
        let mut maker = TreeMaker::new(Some(names));
        maker.syms = Some(Rc::clone(&syms));
        maker.root_package = Some(syms.borrow().root_package);
        maker.types = Some(Rc::clone(&types));
        (ctx, syms, types, maker)
    }

    #[test]
    fn symbol_type_and_statement_convenience_factories() {
        let (mut ctx, syms_rc, types, mut maker) = new_maker_with_syms();
        // Java: Ident(Symbol) — the ident keeps the symbol name/sym/type.
        let var = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            0,
            name(&maker, "v"),
            syms_rc.borrow().int_type,
            syms_rc.borrow().java_base,
        ));
        let ident = maker.ident_symbol(&mut ctx, var);
        let TreeKind::Ident { name: ident_name, sym } = &ctx.tree(ident).kind else {
            panic!("ident");
        };
        assert_eq!(ident_name.to_java_string(), name(&maker, "v").to_java_string());
        assert_eq!(*sym, Some(var));
        assert_eq!(ctx.tree(ident).ty, Some(syms_rc.borrow().int_type));

        // Java: Type(Type) — primitive, class and array types.
        let borrowed_int_type = syms_rc.borrow().int_type;
        let int_tree = maker.type_expr(&mut ctx, &syms_rc.borrow(), borrowed_int_type);
        assert_eq!(ctx.tree(int_tree).tag, Tag::TYPEIDENT);
        let TreeKind::PrimitiveTypeTree { typetag } = &ctx.tree(int_tree).kind else {
            panic!("primitive");
        };
        assert_eq!(*typetag, TypeTag::INT);
        assert_eq!(ctx.tree(int_tree).ty, Some(syms_rc.borrow().int_type));

        let array_class = syms_rc.borrow().array_class;
        let int_array = ctx.alloc_type(crate::code::type_::Type::new_array_type(
            syms_rc.borrow().int_type,
            array_class,
        ));
        let array_tree = maker.type_expr(&mut ctx, &syms_rc.borrow(), int_array);
        assert_eq!(ctx.tree(array_tree).tag, Tag::TYPEARRAY);
        let _ = &types;

        // Java: This(Type)/ClassLiteral(Type) — the field selection keeps sym/type.
        let borrowed_object_type = syms_rc.borrow().object_type;
        let this_tree = maker.this_expr(&mut ctx, borrowed_object_type);
        assert_eq!(ctx.tree(this_tree).tag, Tag::IDENT);
        let borrowed_object_type = syms_rc.borrow().object_type;
        let class_lit = maker.class_literal(&mut ctx, &syms_rc.borrow(), borrowed_object_type);
        assert_eq!(ctx.tree(class_lit).tag, Tag::SELECT);

        // Java: App/Assignment/Indexed/TypeCast convenience helpers.
        let meth_sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_method_symbol(
            0,
            name(&maker, "m"),
            syms_rc.borrow().int_type,
            syms_rc.borrow().java_base,
        ));
        let meth = maker.ident_symbol(&mut ctx, meth_sym);
        ctx.tree_mut(meth).ty = Some(syms_rc.borrow().class_type);
        let app = maker.app(&mut ctx, meth);
        assert_eq!(ctx.tree(app).tag, Tag::APPLY);
        let assignment = maker.assignment(&mut ctx, var, ident);
        assert_eq!(ctx.tree(assignment).tag, Tag::EXEC);
        let borrowed_int_type = syms_rc.borrow().int_type;
        let casted = maker.type_cast_typed(&mut ctx, &syms_rc.borrow(), borrowed_int_type, ident);
        assert_eq!(ctx.tree(casted).tag, Tag::TYPECAST);
        let array_class = syms_rc.borrow().array_class;
        let array_type = ctx.alloc_type(crate::code::type_::Type::new_array_type(
            syms_rc.borrow().int_type,
            array_class,
        ));
        let array_var = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            0,
            name(&maker, "a"),
            array_type,
            syms_rc.borrow().java_base,
        ));
        let index = maker.literal_object(&mut ctx, LiteralValue::Integer(0));
        let indexed = maker.indexed_symbol(&mut ctx, &syms_rc.borrow(), array_var, index);
        assert_eq!(ctx.tree(indexed).tag, Tag::INDEXED);
        assert_eq!(ctx.tree(indexed).ty, Some(syms_rc.borrow().int_type));

        // Java: paramName/typaramName.
        assert_eq!(maker.param_name(2).to_java_string(), name(&maker, "x2").to_java_string());
        assert_eq!(maker.typaram_name(0).to_java_string(), name(&maker, "A0").to_java_string());
    }

    #[test]
    fn attribute_annotation_trees_and_literal_object() {
        use crate::code::attribute::{Attribute, AttributeKind};
        use crate::util::constants::ConstantValue;
        let (mut ctx, syms_rc, _types, mut maker) = new_maker_with_syms();
        // Java: Literal(Object) — the constant type is attached to the tree.
        let literal = maker.literal_constant(&mut ctx, &syms_rc.borrow(), ConstantValue::Integer(7));
        assert_eq!(ctx.tree(literal).tag, Tag::LITERAL);
        let constant_type = ctx.tree(literal).ty.expect("constant type");
        assert_eq!(ctx.type_const_value(constant_type), Some(&ConstantValue::Integer(7)));
        assert_eq!(ctx.type_base_type(constant_type), syms_rc.borrow().int_type);
        let string_literal = maker.literal_constant(
            &mut ctx,
            &syms_rc.borrow(),
            ConstantValue::String(javac_strings::JavaString::from("s")),
        );
        assert_eq!(
            ctx.type_base_type(ctx.tree(string_literal).ty.expect("string const type")),
            syms_rc.borrow().string_type
        );

        // Java: Annotations(List<Attribute.Compound>) — a compound annotation
        // becomes Annotation(Type, [Assign(Ident(method), value)]).
        let method = ctx.alloc_symbol(crate::code::symbol::Symbol::new_method_symbol(
            0,
            maker_at_name(&maker, "m"),
            syms_rc.borrow().int_type,
            syms_rc.borrow().java_base,
        ));
        let constant = ctx.alloc_attribute(Attribute::new(
            Some(syms_rc.borrow().int_type),
            AttributeKind::Constant {
                value: ConstantValue::Integer(7),
            },
        ));
        let compound = ctx.alloc_attribute(Attribute::new(
            Some(syms_rc.borrow().int_type),
            AttributeKind::Compound {
                values: List::of(crate::util::pair::Pair::of(method, constant)),
                position: None,
                synthesized: false,
            },
        ));
        let annotations = maker.annotations_attributes(&mut ctx, &syms_rc.borrow(), List::of(compound));
        assert_eq!(annotations.length(), 1);
        let annotation = *annotations.head();
        assert_eq!(ctx.tree(annotation).tag, Tag::ANNOTATION);
        let TreeKind::Annotation { args, .. } = &ctx.tree(annotation).kind else {
            panic!("annotation");
        };
        assert_eq!(args.length(), 1);
        let assign = *args.head();
        assert_eq!(ctx.tree(assign).tag, Tag::ASSIGN);
        let TreeKind::Assign { lhs, rhs } = &ctx.tree(assign).kind else {
            panic!("assign");
        };
        assert_eq!(ctx.tree(lhs.expect("lhs")).tag, Tag::IDENT);
        assert_eq!(ctx.tree(rhs.expect("rhs")).tag, Tag::LITERAL);

        // Java: Symbol.getRawAttributes() -> Annotations; the metadata creation
        // goes through the symbol arena id.
        let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            0,
            maker_at_name(&maker, "v"),
            syms_rc.borrow().int_type,
            syms_rc.borrow().java_base,
        ));
        let metadata = ctx.symbol_inited_metadata(sym);
        metadata.set_declaration_attributes(List::of(compound));
        let raw = maker.raw_annotations(&mut ctx, &syms_rc.borrow(), sym);
        assert_eq!(raw.length(), 1);
        assert_eq!(ctx.tree(*raw.head()).tag, Tag::ANNOTATION);
    }

    fn maker_at_name(maker: &TreeMaker, text: &str) -> crate::util::name::Name {
        maker
            .names
            .as_ref()
            .expect("names")
            .from_utf(text.as_bytes())
    }

}

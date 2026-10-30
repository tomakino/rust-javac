// Origin: com/sun/tools/javac/comp/LambdaToMethod.java @ 982d53c (jdk-17.0.19-ga)

//! This pass desugars lambda expressions into static methods
//! (LambdaToMethod.java).
//!
//! PORTING_RULES: LambdaToMethod extends TreeTranslator; the port keeps an
//! embedded TreeTranslator base for the default navigation (registered adapter
//! for Java inheritance) and implements the lambda-specific visitors itself.
//! The Java inner classes (DedupedLambda, KlassInfo, LambdaAnalyzerPreprocessor,
//! Frame, TranslationContext and its subclasses, LambdaSymbolKind) are ported as
//! Rust structs/enums in this file; the analyzer anonymous
//! BasicFreeVarCollector subclass maps to a BasicFreeVarCollectorVisitor impl.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::too_many_arguments,
    clippy::let_and_return,
    clippy::needless_return,
    clippy::explicit_counter_loop
)]

// Port note: the upstream translator keeps deeply nested control flow and
// wide service signatures; the corresponding style lints are suppressed.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::code::flags;
use crate::code::kinds::Kind;
use crate::code::symbol::{self, Symbol, SymbolKind};
use crate::code::symtab::Symtab;
use crate::code::type_::Type;
use crate::code::type_tag::TypeTag;
use crate::code::types::{SignatureGenerator, Types};
use crate::comp::attr::Attr;
use crate::comp::attr_context::AttrContext;
use crate::comp::enter::Enter;
use crate::comp::env::Env;
use crate::comp::lower::{self, Lower};
use crate::comp::operators::Operators;
use crate::comp::resolve::Resolve;
use crate::comp::trans_types::TransTypes;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::jvm::pool_constant::{BasicConstant, LoadableConstantRef};
use crate::tree::jc_tree::{LiteralValue, Tag, TreeKind, TreeVisitor};
use crate::tree::tree_info;
use crate::tree::tree_maker::{TreeFactory, TreeMaker};
use crate::tree::tree_translator::TreeTranslator;
use crate::util::list::List;
use crate::util::name::Name;
use crate::util::names::Names;
    /// The altMetafactory flag that marks a serializable lambda.
pub const FLAG_SERIALIZABLE: i32 = 1 << 0;
    /// The altMetafactory flag that is followed by the number of marker interfaces.
pub const FLAG_MARKERS: i32 = 1 << 1;
    /// The altMetafactory flag that is followed by the number of bridge methods.
pub const FLAG_BRIDGES: i32 = 1 << 2;
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
/// Port note: the upstream enum constants keep their Java spelling.
#[allow(non_camel_case_types)]
/// The categories of lambda symbols the analyzer translates (upstream
/// LambdaSymbolKind).
pub enum LambdaSymbolKind {
    PARAM,
    LOCAL_VAR,
    CAPTURED_VAR,
    CAPTURED_THIS,
    CAPTURED_OUTER_THIS,
}

impl LambdaSymbolKind {
    fn propagate_annotations(self) -> bool {
        !matches!(
            self,
            LambdaSymbolKind::CAPTURED_VAR
                | LambdaSymbolKind::CAPTURED_THIS
                | LambdaSymbolKind::CAPTURED_OUTER_THIS
        )
    }
    fn values() -> [LambdaSymbolKind; 5] {
        [
            LambdaSymbolKind::PARAM,
            LambdaSymbolKind::LOCAL_VAR,
            LambdaSymbolKind::CAPTURED_VAR,
            LambdaSymbolKind::CAPTURED_THIS,
            LambdaSymbolKind::CAPTURED_OUTER_THIS,
        ]
    }
}
#[derive(Clone)]
    /// One deduplicated lambda body: its method symbol, its body tree and its
    /// hash code.
pub struct DedupedLambda {
    pub symbol: SymbolId,
    pub tree: TreeId,
    pub hash_code: i32,
}
    /// The per-class translation state (upstream KlassInfo): the methods appended
    /// to the class, the deduplicated lambdas and the deserialization state.
pub struct KlassInfo {
    pub appended_method_list: Vec<TreeId>,
    pub deduped_lambdas: Vec<DedupedLambda>,
    pub dyn_meth_syms: Vec<((SymbolId, Vec<LoadableConstantRef>), SymbolId)>,
    pub deserialize_cases: Vec<(String, Vec<TreeId>)>,
    pub deser_method_sym: SymbolId,
    pub deser_param_sym: SymbolId,
    pub clazz: TreeId,
}
#[derive(Clone)]
    /// One navigation frame of the analyzer (upstream Frame): the tree being
    /// visited and the local variable symbols declared so far.
pub struct Frame {
    pub tree: TreeId,
    pub locals: Vec<SymbolId>,
}

impl Frame {
    /// Creates a frame for the given tree with an empty local set.
    pub fn new(tree: TreeId) -> Frame {
        Frame {
            tree,
            locals: Vec::new(),
        }
    }
    /// Records one local variable declaration in this frame.
    pub fn add_local(&mut self, sym: SymbolId) {
        self.locals.insert(0, sym);
    }
    /// Returns whether this frame declared the given symbol.
    pub fn contains(&self, sym: SymbolId) -> bool {
        self.locals.contains(&sym)
    }
}
    /// The translation context of one lambda (upstream
    /// LambdaTranslationContext): its target type, the synthetic method it is
    /// translated to and the symbol mapping of its captured variables.
pub struct LambdaTranslationContext {
    pub target: TypeId,
    /// Whether this lambda was synthesized from a member reference
    /// (upstream MemberReferenceToLambda); the "lambda.stat" note is only
    /// reported for source lambdas.
    pub from_member_reference: bool,
    pub self_sym: Option<SymbolId>,
    pub assigned_to: Option<SymbolId>,
    pub translated_symbols: HashMap<LambdaSymbolKind, Vec<(SymbolId, SymbolId)>>,
    pub translated_sym: SymbolId,
    pub synthetic_params: Option<Vec<TreeId>>,
    pub free_var_processed_local_classes: HashSet<SymbolId>,
    pub method_reference_receiver: Option<TreeId>,
}
    /// The translation context of one member reference (upstream
    /// ReferenceTranslationContext): whether it is a super reference and its
    /// target type.
pub struct ReferenceTranslationContext {
    pub is_super: bool,
    pub target: TypeId,
}
    /// The context of one translated lambda or member reference (upstream
    /// TranslationContext): the tree, its owning symbol, the capture depth, the
    /// bridge method symbols and the subclass payload.
pub struct TranslationContext {
    pub tree: TreeId,
    pub owner: SymbolId,
    pub depth: i32,
    pub bridges: Vec<SymbolId>,
    pub prev_context: Option<TreeId>,
    pub kind: TranslationContextKind,
}
    /// The kind discriminator of a translation context (registration adapter for
    /// the upstream generic TranslationContext family).
pub enum TranslationContextKind {
    Lambda(LambdaTranslationContext),
    Reference(ReferenceTranslationContext),
}
    /// This pass desugars lambda expressions into static methods.
    ///
    /// The Rust port keeps the upstream TreeTranslator shape: a TreeVisitor whose
    /// default navigation descends into every child and whose overridden visitors
    /// replace lambda and member reference trees with invokedynamic calls.
pub struct LambdaToMethod {
    pub attr: Rc<RefCell<Attr>>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
    pub log: Rc<RefCell<crate::util::log::Log>>,
    pub lower: Rc<RefCell<Lower>>,
    pub names: Rc<Names>,
    pub syms: Rc<RefCell<Symtab>>,
    pub rs: Rc<RefCell<Resolve>>,
    pub operators: Rc<Operators>,
    pub make: Rc<RefCell<TreeMaker>>,
    pub types: Rc<Types>,
    pub trans_types: Rc<RefCell<TransTypes>>,
    pub enter: Rc<RefCell<Enter>>,
    pub attr_env: Option<Rc<Env<AttrContext>>>,
    pub analyzer: LambdaAnalyzerPreprocessor,
    pub context_map: Rc<RefCell<HashMap<TreeId, TranslationContext>>>,
    pub context: Option<TreeId>,
    pub k_info: Option<KlassInfo>,
    pub dump_lambda_to_method_stats: bool,
    pub force_serializable: bool,
    pub debug_lines_or_vars: bool,
    pub verbose_deduplication: bool,
    pub deduplicate_lambdas: bool,
    pub nestmate_lambdas: bool,
    pub result: Option<TreeId>,
    pub base: TreeTranslator,
}

impl LambdaToMethod {
    /// # Arguments
    ///   - the ported service handles;
    ///   - the option flags read by the upstream constructor.
    /// # Returns
    ///   - the pass.
    /// # Panics
    ///   - none.
    ///
    /// Creates the pass with the compiler services it drives (Attr, Log, Lower,
    /// Names, Symtab, Resolve, Operators, TreeMaker, Types, TransTypes, Enter) and
    /// the option flags.
    #[allow(clippy::too_many_arguments)]
    ///
    /// # Arguments
    /// - `attr` — the attribution service used for synthetic variables
    /// - `diags` — the diagnostic factory
    /// - `log` — the compiler log
    /// - `lower` — the Lower pass (free variable collection)
    /// - `names` — the shared name table
    /// - `syms` — the symbol table
    /// - `resolve` — the resolver
    /// - `operators` — the operator table
    /// - `make` — the tree maker
    /// - `types` — the type services
    /// - `trans_types` — the TransTypes pass (coercion)
    /// - `enter` — the Enter pass
    /// - the remaining flags select serialization, debug and deduplication modes
    pub fn new(
        attr: Rc<RefCell<Attr>>,
        diags: Rc<crate::util::jc_diagnostic::Factory>,
        log: Rc<RefCell<crate::util::log::Log>>,
        lower: Rc<RefCell<Lower>>,
        names: Rc<Names>,
        syms: Rc<RefCell<Symtab>>,
        rs: Rc<RefCell<Resolve>>,
        operators: Rc<Operators>,
        make: Rc<RefCell<TreeMaker>>,
        types: Rc<Types>,
        trans_types: Rc<RefCell<TransTypes>>,
        enter: Rc<RefCell<Enter>>,
        dump_lambda_to_method_stats: bool,
        force_serializable: bool,
        debug_lines_or_vars: bool,
        verbose_deduplication: bool,
        deduplicate_lambdas: bool,
        nestmate_lambdas: bool,
    ) -> LambdaToMethod {
        let context_map: Rc<RefCell<HashMap<TreeId, TranslationContext>>> =
            Rc::new(RefCell::new(HashMap::new()));
        let mut analyzer = LambdaAnalyzerPreprocessor::new(
            Rc::clone(&context_map),
            Rc::clone(&names),
            Rc::clone(&syms),
            Rc::clone(&types),
            Rc::clone(&attr),
            Rc::clone(&lower),
            Rc::clone(&log),
            Rc::clone(&trans_types),
            Rc::clone(&make),
            Rc::clone(&diags),
            None,
            dump_lambda_to_method_stats,
        );
        analyzer.force_serializable = force_serializable;
        analyzer.nestmate_lambdas = nestmate_lambdas;
        LambdaToMethod {
            attr,
            diags,
            log,
            lower,
            names,
            syms,
            rs,
            operators,
            make,
            types,
            trans_types,
            enter,
            attr_env: None,
            analyzer,
            context_map,
            context: None,
            k_info: None,
            dump_lambda_to_method_stats,
            force_serializable,
            debug_lines_or_vars,
            verbose_deduplication,
            deduplicate_lambdas,
            nestmate_lambdas,
            result: None,
            base: TreeTranslator::new(),
        }
    }
    /// Translates one tree, switching to the translation context registered for
    /// lambda and member reference trees.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `tree` — the tree to translate
    ///
    /// # Returns
    /// The translated tree, or None when the input was None
    pub fn translate(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        let tree = tree?;
        let new_context = if self.context_map.borrow().contains_key(&tree) {
            Some(tree)
        } else {
            self.context
        };
        self.translate_with_context(ctx, tree, new_context)
    }
    /// Translates one tree under the given translation context, restoring the
    /// previous context afterwards.
    pub fn translate_with_context(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        new_context: Option<TreeId>,
    ) -> Option<TreeId> {
        let previous_context = self.context;
        self.context = new_context;
        self.result = None;
        crate::tree::jc_tree::accept(ctx, tree, self);
        let result = self.result.take().or(Some(tree));
        self.context = previous_context;
        result
    }
    /// Translates a list of trees under the given translation context.
    pub fn translate_list_with_context(
        &mut self,
        ctx: &mut CompilerContext,
        trees: &List<TreeId>,
        new_context: Option<TreeId>,
    ) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        for tree in trees.iter() {
            let translated = self
                .translate_with_context(ctx, tree, new_context)
                .expect("translated tree");
            result = result.append(translated);
        }
        result
    }
    /// Entry point of the pass: resets the context map, analyzes the class and
    /// translates it.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `env` — the attribution environment of the class
    /// - `cdef` — the class declaration tree
    /// - `make` — the tree maker
    ///
    /// # Returns
    /// The translated class declaration
    pub fn translate_top_level_class(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        cdef: TreeId,
        make: Rc<RefCell<TreeMaker>>,
    ) -> TreeId {
        self.make = make;
        self.attr_env = Some(Rc::clone(env));
        self.analyzer.attr_env = Some(Rc::clone(env));
        self.context = None;
        self.context_map.borrow_mut().clear();
        self.translate(ctx, Some(cdef)).expect("translated class")
    }
}


impl LambdaToMethod {
    fn visit_class_def_l2m(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut tree = tree;
        let class_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        if ctx.sym(class_sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::PCK) {
            tree = self.analyzer.analyze_and_preprocess_class(ctx, tree);
        }
        let previous_klass_info = self.k_info.take();
        let klass_info = self.new_klass_info(ctx, tree, class_sym);
        self.k_info = Some(klass_info);
        self.default_visit_class_def(ctx, tree);
        let translated = self.result.take().expect("translated class");
        let has_deserialize_cases = self
            .k_info
            .as_ref()
            .map(|info| !info.deserialize_cases.is_empty())
            .unwrap_or(false);
        if has_deserialize_cases {
            let previous_pos = self.make.borrow().pos;
            {
                let mut make = self.make.borrow_mut();
                make.at(ctx.tree(translated).pos);
            }
            let deser = self.make_deserialize_method(ctx, class_sym);
            {
                let mut make = self.make.borrow_mut();
                make.at(previous_pos);
            }
            if let Some(info) = self.k_info.as_mut() {
                info.appended_method_list.insert(0, deser);
            }
        }
        let new_methods: Vec<TreeId> = self
            .k_info
            .as_ref()
            .map(|info| info.appended_method_list.clone())
            .unwrap_or_default();
        if let TreeKind::ClassDecl { defs: slot, .. } = &mut ctx.tree_mut(translated).kind {
            *slot = slot.append_list(&List::from_iterable(new_methods.clone()));
        }
        for lambda in &new_methods {
            if let TreeKind::MethodDecl { sym: Some(method_sym), .. } = &ctx.tree(*lambda).kind {
                let members = match &ctx.sym(class_sym).kind_data {
                    SymbolKind::ClassSymbol { members_field, .. } => *members_field,
                    _ => None,
                };
                if let Some(members) = members {
                    ctx.scope_enter(members, *method_sym);
                }
            }
        }
        self.result = Some(translated);
        self.k_info = previous_klass_info;
    }
    fn new_klass_info(&mut self, ctx: &mut CompilerContext, clazz: TreeId, class_sym: SymbolId) -> KlassInfo {
        let serialized_lambda_type = self.syms.borrow().serialized_lambda_type;
        let object_type = self.syms.borrow().object_type;
        let method_class = self.syms.borrow().method_class;
        let method_type = ctx.alloc_type(Type::new_method_type(
            List::of(serialized_lambda_type),
            object_type,
            List::nil(),
            method_class,
        ));
        let deser_method_sym = self.make_private_synthetic_method(
            ctx,
            flags::STATIC as i64,
            self.names.deserializeLambda.clone(),
            Some(method_type),
            class_sym,
        );
        let lambda_name = self.names.from_string(&javac_strings::JavaString::from("lambda"));
        let deser_param_sym = self.new_var_symbol(
            ctx,
            flags::FINAL as i64,
            lambda_name,
            serialized_lambda_type,
            deser_method_sym,
        );
        KlassInfo {
            appended_method_list: Vec::new(),
            deduped_lambdas: Vec::new(),
            dyn_meth_syms: Vec::new(),
            deserialize_cases: Vec::new(),
            deser_method_sym,
            deser_param_sym,
            clazz,
        }
    }
    fn visit_lambda_l2m(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if self.context.is_none()
            && std::env::var_os("DSH_DEBUG_L2M").is_some()
        {
            let pos = ctx.tree(tree).pos;
            let keys: Vec<(i32, u32)> = self
                .context_map
                .borrow()
                .keys()
                .map(|t| (ctx.tree(*t).pos, t.index()))
                .collect();
            eprintln!("[L2M-NOCTX] lambda_pos={pos} contexts={keys:?}");
        }
        let context_tree = match self.context {
            Some(context_tree) => context_tree,
            None => {
                // A speculative/translated copy of a lambda has a fresh tree id
                // but keeps the source position of the analyzed lambda.
                let pos = ctx.tree(tree).pos;
                // A reference and the lambda converted from it share the same
                // source position. Prefer the Lambda context; matching the
                // first context by position picked the Reference context and
                // visit_lambda_l2m panicked with "expected a lambda context".
                let candidates: Vec<TreeId> =
                    self.context_map.borrow().keys().copied().collect();
                let found = candidates.into_iter().find(|candidate| {
                    ctx.tree(*candidate).pos == pos
                        && matches!(
                            self.context_map
                                .borrow()
                                .get(candidate)
                                .map(|context| &context.kind),
                            Some(TranslationContextKind::Lambda(_))
                        )
                });
                found.unwrap_or_else(|| panic!("current translation context"))
            }
        };
        let (mut sym, owner, self_sym, method_reference_receiver, prev_context, synthetic_params) = {
            let context_map = self.context_map.borrow();
            let context = context_map
                .get(&context_tree)
                .expect("lambda translation context");
            let TranslationContextKind::Lambda(lambda) = &context.kind else {
                panic!("expected a lambda translation context");
            };
            (
                lambda.translated_sym,
                context.owner,
                lambda.self_sym,
                lambda.method_reference_receiver,
                self.map_prev_context(ctx),
                lambda.synthetic_params.clone(),
            )
        };
        let lambda_type = ctx.sym(sym).ty.expect("lambda method type");
        self.apportion_type_annotations_lambda(ctx, tree, owner, sym);
        let owner_name = ctx.sym(owner).name.clone();
        let init = owner_name == self.names.init;
        let clinit = owner_name == self.names.clinit;
        let annotation_owner = owner;
        let _ = annotation_owner;
        if init || clinit {
            let annotation_owner = ctx.sym(owner).owner.expect("owner of constructor");
            self.apportion_type_annotations_owner(ctx, tree, annotation_owner, init, sym);
        }
        if let Some(self_sym) = self_sym {
            if is_field_symbol(ctx, self_sym) {
                self.apportion_type_annotations_owner(ctx, tree, self_sym, false, sym);
            }
        }
        let _ = synthetic_params;
        let lambda_decl = self.make_lambda_method_decl(ctx, tree, sym, lambda_type);
        let lambda_body = self.make_lambda_body(ctx, tree, lambda_decl);
        let translated_body = self
            .translate_with_context(ctx, lambda_body, Some(tree))
            .expect("translated lambda body");
        if let TreeKind::MethodDecl { body: slot, .. } = &mut ctx.tree_mut(lambda_decl).kind {
            *slot = Some(translated_body);
        }
        let serializable = self.context_is_serializable(ctx, context_tree);
        let mut dedupe = false;
        if self.deduplicate_lambdas && !self.debug_lines_or_vars && !serializable {
            if let Some(existing) = self.dedup_lambda(ctx, sym, translated_body) {
                sym = existing;
                dedupe = true;
            }
        }
        if dedupe && self.verbose_deduplication {
            let name = crate::comp::resolve::symbol_display_name(ctx, sym);
            let note = crate::util::jc_diagnostic::Note::from(
                crate::resources::compiler_properties::notes::verbose_l2m_deduplicate(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(name),
                ),
            );
            // The note points at the rewritten lambda; ensure the enclosing
            // class source is active so the diagnostic carries file:line.
            let owner = crate::code::symbol::encl_class(ctx, sym);
            let sourcefile = owner.and_then(|owner| match &ctx.sym(owner).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => {
                    sourcefile.clone()
                }
                _ => None,
            });
            let prev_source = crate::util::abstract_log::AbstractLog::use_source(
                &mut *self.log.borrow_mut(),
                sourcefile,
            );
            crate::util::abstract_log::AbstractLog::note_at(
                &mut *self.log.borrow_mut(),
                ctx.tree(tree).pos,
                note,
            );
            crate::util::abstract_log::AbstractLog::use_source(
                &mut *self.log.borrow_mut(),
                prev_source,
            );
        }
        if !dedupe {
            if let Some(info) = self.k_info.as_mut() {
                info.appended_method_list.insert(0, lambda_decl);
            }
        }
        let mut synthetic_inits: Vec<TreeId> = Vec::new();
        if let Some(receiver) = method_reference_receiver {
            synthetic_inits.push(receiver);
        } else {
            let is_static = (ctx.sym(sym).flags_field & flags::STATIC as i64) != 0;
            if !is_static {
                let sym_encl = symbol::encl_class(ctx, sym)
                    .and_then(|class| ctx.sym(class).ty)
                    .expect("enclosing class of lambda method");
                synthetic_inits.push(self.make_this(ctx, sym_encl, crate::code::symbol::encl_class(ctx, owner).expect("owner class")));
            }
        }
        {
            let context_map = self.context_map.borrow();
            let context = context_map
                .get(&context_tree)
                .expect("lambda translation context");
            let TranslationContextKind::Lambda(lambda) = &context.kind else {
                panic!("expected a lambda translation context");
            };
            let self_sym = lambda.self_sym;
            for fv in captured_symbols(lambda, LambdaSymbolKind::CAPTURED_VAR) {
                if Some(fv) != self_sym {
                    let captured = {
                        let mut make = self.make.borrow_mut();
                        make.at(ctx.tree(tree).pos);
                        make.ident_symbol(ctx, fv)
                    };
                    ctx.tree_mut(captured).ty = ctx.sym(fv).ty;
                    synthetic_inits.push(captured);
                }
            }
            for fv in captured_symbols(lambda, LambdaSymbolKind::CAPTURED_OUTER_THIS) {
                let captured = {
                    let mut make = self.make.borrow_mut();
                    make.at(ctx.tree(tree).pos);
                    make.qual_this(ctx, &mut self.syms.borrow_mut(), ctx.sym(fv).ty.expect("outer this type"))
                };
                synthetic_inits.push(captured);
            }
        }
        let indy_args =
            self.translate_list_with_context(ctx, &List::from_iterable(synthetic_inits), prev_context);
        let handle = symbol::as_method_handle(ctx, sym, false);
        let result = self.make_metafactory_indy_call(ctx, context_tree, handle, indy_args);
        let from_member_reference = self
            .context_map
            .borrow()
            .get(&context_tree)
            .map(|context| match &context.kind {
                TranslationContextKind::Lambda(lambda) => lambda.from_member_reference,
                _ => false,
            })
            .unwrap_or(false);
        if self.dump_lambda_to_method_stats && !from_member_reference {
            let name = crate::comp::resolve::symbol_display_name(ctx, sym);
            let note = crate::util::jc_diagnostic::Note::from(
                crate::resources::compiler_properties::notes::lambda_stat(
                    false,
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(name),
                ),
            );
            let owner = crate::code::symbol::encl_class(ctx, sym);
            let sourcefile = owner.and_then(|owner| match &ctx.sym(owner).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => {
                    sourcefile.clone()
                }
                _ => None,
            });
            let prev_source = crate::util::abstract_log::AbstractLog::use_source(
                &mut *self.log.borrow_mut(),
                sourcefile,
            );
            crate::util::abstract_log::AbstractLog::note_at(
                &mut *self.log.borrow_mut(),
                ctx.tree(tree).pos,
                note,
            );
            crate::util::abstract_log::AbstractLog::use_source(
                &mut *self.log.borrow_mut(),
                prev_source,
            );
        }
        self.result = Some(result);
    }
    fn make_this(&mut self, ctx: &mut CompilerContext, ty: TypeId, owner: SymbolId) -> TreeId {
        let flags_field = flags::PARAMETER | flags::FINAL as i64 | flags::SYNTHETIC as i64;
        let this_sym = self.new_var_symbol(ctx, flags_field, self.names._this.clone(), ty, owner);
        let ident = {
            let mut make = self.make.borrow_mut();
            make.ident_symbol(ctx, this_sym)
        };
        ctx.tree_mut(ident).ty = Some(ty);
        ident
    }
    fn visit_reference_l2m(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        // Java: the analyzer registers a ReferenceTranslationContext for every
        // member reference, and Translator.visitReference uses it even outside
        // a lambda body. Look the reference context up by its tree when there
        // is no enclosing lambda context.
        let context_tree = match self.context {
            Some(context_tree) => context_tree,
            None => {
                let is_reference = matches!(
                    self.context_map
                        .borrow()
                        .get(&tree)
                        .map(|context| &context.kind),
                    Some(TranslationContextKind::Reference(_))
                );
                if !is_reference {
                    self.default_visit_reference(ctx, tree);
                    return;
                }
                tree
            }
        };
        let (owner, prev_context) = {
            let context_map = self.context_map.borrow();
            let context = context_map
                .get(&context_tree)
                .expect("reference translation context");
            (context.owner, self.map_prev_context(ctx))
        };
        let ref_sym = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { sym: Some(sym), .. } => *sym,
            _ => panic!("expected an attributed member reference"),
        };
        let mode = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { mode, .. } => *mode,
            _ => panic!("ClassCastException: JCMemberReference expected"),
        };
        let kind = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { kind, .. } => *kind,
            _ => panic!("ClassCastException: JCMemberReference expected"),
        };
        let init: Option<TreeId> = match (mode, kind) {
            (crate::tree::jc_tree::ReferenceMode::INVOKE, Some(crate::tree::jc_tree::ReferenceKind::IMPLICIT_INNER))
            | (crate::tree::jc_tree::ReferenceMode::INVOKE, Some(crate::tree::jc_tree::ReferenceKind::SUPER)) => {
                let owner_encl = symbol::encl_class(ctx, owner)
                    .and_then(|class| ctx.sym(class).ty)
                    .expect("enclosing class of reference owner");
                Some(self.make_this(ctx, owner_encl, crate::code::symbol::encl_class(ctx, owner).expect("owner class")))
            }
            (crate::tree::jc_tree::ReferenceMode::INVOKE, Some(crate::tree::jc_tree::ReferenceKind::BOUND)) => {
                let qualifier = match &ctx.tree(tree).kind {
                    TreeKind::MemberReference { expr: Some(expr), .. } => *expr,
                    _ => panic!("bound member reference without qualifier"),
                };
                let sym_owner = ctx.sym(ref_sym).owner.expect("reference symbol owner");
                let target = ctx.sym(sym_owner).ty.expect("reference owner type");
                let target = {
                    let syms = self.syms.borrow();
                    self.types.erasure(ctx, &syms, target)
                };
                let coerced = {
                    let attr_env = self.attr_env.clone().expect("attrEnv");
                    let mut trans_types = self.trans_types.borrow_mut();
                    trans_types.coerce(ctx, &attr_env, qualifier, target)
                };
                let attr = Rc::clone(&self.attr);
                let checked = {
                    let mut attr = attr.borrow_mut();
                    let mut syms = self.syms.borrow_mut();
                    attr.make_null_check(ctx, &mut syms, coerced)
                };
                Some(checked)
            }
            _ => None,
        };
        let indy_args = match init {
            None => List::nil(),
            Some(init) => self.translate_list_with_context(ctx, &List::of(init), prev_context),
        };
        let handle = symbol::as_method_handle(ctx, ref_sym, false);
        let result = self.make_metafactory_indy_call(ctx, context_tree, handle, indy_args);
        let (ref_bridges, is_super) = match self.context_map.borrow().get(&context_tree) {
            Some(context) => match &context.kind {
                TranslationContextKind::Reference(reference) => {
                    (context.bridges.clone(), reference.is_super)
                }
                _ => (Vec::new(), false),
            },
            None => (Vec::new(), false),
        };
        let super_qualifier = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr: Some(expr), .. } => {
                crate::tree::tree_info::symbol(ctx, *expr)
                    .map(|symbol| ctx.sym(symbol).name == self.names._super)
                    .unwrap_or(false)
            }
            _ => false,
        };
        let _ = is_super;
        let converts_to_lambda = matches!(
            kind,
            Some(crate::tree::jc_tree::ReferenceKind::SUPER)
                | Some(crate::tree::jc_tree::ReferenceKind::IMPLICIT_INNER)
        ) || super_qualifier
            || !ref_bridges.is_empty();
        if self.dump_lambda_to_method_stats && !converts_to_lambda {
            let note = crate::util::jc_diagnostic::Note::from(
                crate::resources::compiler_properties::notes::mref_stat(false, ()),
            );
            let owner = crate::code::symbol::encl_class(ctx, ref_sym);
            let sourcefile = owner.and_then(|owner| match &ctx.sym(owner).kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => {
                    sourcefile.clone()
                }
                _ => None,
            });
            let prev_source = crate::util::abstract_log::AbstractLog::use_source(
                &mut *self.log.borrow_mut(),
                sourcefile,
            );
            crate::util::abstract_log::AbstractLog::note_at(
                &mut *self.log.borrow_mut(),
                ctx.tree(tree).pos,
                note,
            );
            crate::util::abstract_log::AbstractLog::use_source(
                &mut *self.log.borrow_mut(),
                prev_source,
            );
        }
        self.result = Some(result);
    }
    fn visit_ident_l2m(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::Ident { sym, .. } => *sym,
            _ => None,
        };
        let filter = sym
            .map(|sym| self.analyzer.lambda_ident_symbol_filter(ctx, sym))
            .unwrap_or(false);
        if self.context.is_none() || !filter {
            self.default_visit_ident(ctx, tree);
            return;
        }
        let previous_pos = self.make.borrow().pos;
        {
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
        }
        let translated = self.translate_ident_in_lambda(ctx, tree);
        if let Some(translated) = translated {
            self.result = Some(translated);
        } else {
            self.default_visit_ident(ctx, tree);
        }
        {
            let mut make = self.make.borrow_mut();
            make.at(previous_pos);
        }
    }
    fn visit_select_l2m(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let filter = self.analyzer.lambda_field_access_filter(ctx, tree);
        if self.context.is_none() || !filter {
            self.default_visit_select(ctx, tree);
            return;
        }
        let previous_pos = self.make.borrow().pos;
        {
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
        }
        let translated = self.translate_select_in_lambda(ctx, tree);
        if let Some(translated) = translated {
            self.result = Some(translated);
        } else {
            self.default_visit_select(ctx, tree);
        }
        {
            let mut make = self.make.borrow_mut();
            make.at(previous_pos);
        }
    }
    fn visit_new_class_l2m(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let filter = self.analyzer.lambda_new_class_filter(ctx, tree);
        if self.context.is_none() || !filter {
            self.default_visit_new_class(ctx, tree);
            return;
        }
        let previous_pos = self.make.borrow().pos;
        {
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(tree).pos);
        }
        let translated = self.translate_new_class_in_lambda(ctx, tree);
        self.default_visit_new_class(ctx, translated);
        {
            let mut make = self.make.borrow_mut();
            make.at(previous_pos);
        }
    }
    fn visit_var_def_l2m(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut translated = None;
        if let Some(context_tree) = self.context {
            let context_map = self.context_map.borrow();
            if let Some(context) = context_map.get(&context_tree) {
                if let TranslationContextKind::Lambda(lambda) = &context.kind {
                    if let TreeKind::VariableDecl { sym: Some(sym), .. } = &ctx.tree(tree).kind {
                        if let Some(mapped) = context_symbol(lambda, LambdaSymbolKind::LOCAL_VAR, *sym) {
                            translated = Some((mapped, *sym));
                        }
                    }
                }
            }
        }
        let Some((mapped_sym, original_sym)) = translated else {
            self.default_visit_var_def(ctx, tree);
            return;
        };
        let _ = original_sym;
        let init = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { init, .. } => *init,
            _ => panic!("expected a variable declaration"),
        };
        // Java: visitVarDef only translates the initializer when it exists
        // (Lower.visitVarDef / LambdaToMethod.translate). Passing the VARDEF
        // itself as a fallback re-enters visit_var_def_l2m forever.
        let translated_init = match init {
            Some(init) => Some(
                self.translate_with_context(ctx, init, self.context)
                    .expect("translated initializer"),
            ),
            None => None,
        };
        if let TreeKind::VariableDecl { init: slot, sym, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = translated_init;
            *sym = Some(mapped_sym);
        }
        self.result = Some(tree);
    }
}
/// This visitor collects information about translation of a lambda expression.
/// More specifically, it keeps track of the enclosing contexts and captured locals
/// accessed by the lambda being translated (as well as other useful info).
/// It also translates away problems for LambdaToMethod.
/// PORTING_RULES: the analyzer shares the translation contexts and the compiler
/// services with the enclosing pass through Rc/RefCell (registered adapter).
pub struct LambdaAnalyzerPreprocessor {
    pub context_map: Rc<RefCell<HashMap<TreeId, TranslationContext>>>,
    pub names: Rc<Names>,
    pub syms: Rc<RefCell<Symtab>>,
    pub types: Rc<Types>,
    pub attr: Rc<RefCell<Attr>>,
    pub lower: Rc<RefCell<Lower>>,
    pub log: Rc<RefCell<crate::util::log::Log>>,
    pub trans_types: Rc<RefCell<TransTypes>>,
    pub make: Rc<RefCell<TreeMaker>>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
    pub frame_stack: Vec<Frame>,
    pub lambda_count: i32,
    pub types_under_construction: Vec<SymbolId>,
    pub synthetic_method_name_counts: HashMap<String, i32>,
    pub local_class_defs: HashMap<SymbolId, TreeId>,
    pub clinits: HashMap<SymbolId, SymbolId>,
    pub attr_env: Option<Rc<Env<AttrContext>>>,
    pub dump_lambda_to_method_stats: bool,
    pub force_serializable: bool,
    pub nestmate_lambdas: bool,
    pub result: Option<TreeId>,
    pub base: TreeTranslator,
}

impl LambdaToMethod {
    /// Translates a list of trees (TreeTranslator view of the pass).
    pub fn translate_list(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list_with_context(ctx, trees, self.context)
    }
    /// Translates a list of variable declarations.
    pub fn translate_var_defs(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// Translates a list of type parameters.
    pub fn translate_type_params(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// Translates a list of switch cases.
    pub fn translate_cases(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// Translates a list of catch clauses.
    pub fn translate_catchers(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// Translates a list of annotations.
    pub fn translate_annotations(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
}

impl LambdaAnalyzerPreprocessor {
    /// Translates a list of trees (analyzer view of the pass).
    pub fn translate_list(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        for tree in trees.iter() {
            let translated = self.translate(ctx, Some(tree)).expect("translated tree");
            result = result.append(translated);
        }
        result
    }
    /// Translates a list of variable declarations (analyzer view).
    pub fn translate_var_defs(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// Translates a list of type parameters (analyzer view).
    pub fn translate_type_params(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// Translates a list of switch cases (analyzer view).
    pub fn translate_cases(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// Translates a list of catch clauses (analyzer view).
    pub fn translate_catchers(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// Translates a list of annotations (analyzer view).
    pub fn translate_annotations(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    /// # Arguments
    ///   - the shared services/context map of the enclosing pass.
    /// # Returns
    ///   - the analyzer.
    /// # Panics
    ///   - none.
    ///
    /// Creates the analyzer with the compiler services it uses.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context_map: Rc<RefCell<HashMap<TreeId, TranslationContext>>>,
        names: Rc<Names>,
        syms: Rc<RefCell<Symtab>>,
        types: Rc<Types>,
        attr: Rc<RefCell<Attr>>,
        lower: Rc<RefCell<Lower>>,
        log: Rc<RefCell<crate::util::log::Log>>,
        trans_types: Rc<RefCell<TransTypes>>,
        make: Rc<RefCell<TreeMaker>>,
        diags: Rc<crate::util::jc_diagnostic::Factory>,
        attr_env: Option<Rc<Env<AttrContext>>>,
        dump_lambda_to_method_stats: bool,
    ) -> LambdaAnalyzerPreprocessor {
        LambdaAnalyzerPreprocessor {
            context_map,
            names,
            syms,
            types,
            attr,
            lower,
            log,
            trans_types,
            make,
            diags,
            frame_stack: Vec::new(),
            lambda_count: 0,
            types_under_construction: Vec::new(),
            synthetic_method_name_counts: HashMap::new(),
            local_class_defs: HashMap::new(),
            clinits: HashMap::new(),
            attr_env,
            dump_lambda_to_method_stats,
            force_serializable: false,
            nestmate_lambdas: false,
            result: None,
            base: TreeTranslator::new(),
        }
    }
    /// Translates one tree with the analyzer's visitor (dispatches to the
    /// overridden analyzer visitors).
    pub fn translate(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        let tree = tree?;
        self.result = None;
        crate::tree::jc_tree::accept(ctx, tree, self);
        self.result.take().or(Some(tree))
    }
    /// Analyzes and preprocesses one class: registers the lambda/reference
    /// translation contexts and captures local classes.
    ///
    /// # Arguments
    /// - `ctx` — the compiler context
    /// - `tree` — the class declaration
    ///
    /// # Returns
    /// The class declaration with the analyzer side effects recorded
    pub fn analyze_and_preprocess_class(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
    ) -> TreeId {
        self.frame_stack = Vec::new();
        self.types_under_construction = Vec::new();
        self.local_class_defs.clear();
        if let TreeKind::CompilationUnit { .. } = &ctx.tree(tree).kind {
            panic!("expected a class declaration");
        }
        self.translate(ctx, Some(tree)).expect("translated class")
    }
}

impl LambdaAnalyzerPreprocessor {
    fn analyzer_visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let previous_nascent_types = self.types_under_construction.clone();
        let meth_name = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth: Some(meth), .. } => tree_info::name(ctx, *meth),
            _ => None,
        };
        if let Some(name) = meth_name {
            if name == self.names._this || name == self.names._super {
                if let Some(current_class) = self.current_class(ctx) {
                    self.types_under_construction.insert(0, current_class);
                }
            }
        }
        self.analyzer_default_visit_apply(ctx, tree);
        self.types_under_construction = previous_nascent_types;
    }
    fn analyzer_visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let previous_stack = self.frame_stack.clone();
        if let Some(head) = self.frame_stack.first() {
            if ctx.tree(head.tree).has_tag(Tag::CLASSDEF) {
                self.frame_stack.insert(0, Frame::new(tree));
            }
        }
        self.analyzer_default_visit_block(ctx, tree);
        self.frame_stack = previous_stack;
    }
    fn analyzer_visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let previous_stack = self.frame_stack.clone();
        let previous_lambda_count = self.lambda_count;
        let previous_synthetic_counts = self.synthetic_method_name_counts.clone();
        let previous_clinits = self.clinits.clone();
        let class_sym = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("expected a class declaration"),
        };
        self.lambda_count = 0;
        self.synthetic_method_name_counts = HashMap::new();
        self.clinits = HashMap::new();
        if ctx.sym(class_sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH) {
            self.local_class_defs.insert(class_sym, tree);
        }
        if let Some(enclosing_lambda) = self.directly_enclosing_lambda(ctx) {
            let owner = self.owner(ctx, false).expect("owner");
            if let TreeKind::ClassDecl { sym: Some(slot), .. } = &mut ctx.tree_mut(tree).kind {
                let _ = slot;
            }
            self.set_class_owner(ctx, class_sym, owner);
            if ctx.sym(class_sym).has_outer_instance(ctx) {
                let class_ty = ctx.sym(class_sym).ty.expect("class type");
                let outer_instance_symbol = ctx.typ(class_ty).get_enclosing_type().and_then(|encl| ctx.typ(encl).tsym);
                let mut local_context = self.context_key(ctx);
                while let Some(context_tree) = local_context {
                    let context_owner = self
                        .context_map
                        .borrow()
                        .get(&context_tree)
                        .map(|context| context.owner)
                        .expect("context owner");
                    if ctx.sym(context_owner).flags_field & flags::STATIC as i64 != 0 {
                        break;
                    }
                    if ctx.tree(context_tree).has_tag(Tag::LAMBDA) {
                        if let Some(outer) = outer_instance_symbol {
                            let depth = self
                                .context_map
                                .borrow()
                                .get(&context_tree)
                                .map(|context| context.depth)
                                .expect("context depth");
                            if self.captured_decl(ctx, depth, outer).is_none() {
                                break;
                            }
                            self.add_lambda_symbol(ctx, context_tree, outer, LambdaSymbolKind::CAPTURED_THIS);
                        }
                    }
                    local_context = self
                        .context_map
                        .borrow()
                        .get(&context_tree)
                        .and_then(|context| context.prev_context);
                }
                let _ = enclosing_lambda;
            }
        }
        self.frame_stack.insert(0, Frame::new(tree));
        self.analyzer_default_visit_class_def(ctx, tree);
        self.frame_stack = previous_stack;
        self.lambda_count = previous_lambda_count;
        self.synthetic_method_name_counts = previous_synthetic_counts;
        self.clinits = previous_clinits;
    }
    fn analyzer_visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::Ident { sym, .. } => *sym,
            _ => None,
        };
        if let Some(sym) = sym {
            if std::env::var_os("DSH_DEBUG_L2M").is_some()
                && ctx.sym(sym).name.to_java_string().to_string_lossy() == "gson"
            {
                eprintln!(
                    "[CAP] name=gson key={:?} filter={} owner={:?} flags={}",
                    self.context_key(ctx),
                    self.lambda_ident_symbol_filter(ctx, sym),
                    ctx.sym(sym).owner.map(|o| ctx.sym(o).kind),
                    ctx.sym(sym).flags_field,
                );
            }
            if self.context_key(ctx).is_some() && self.lambda_ident_symbol_filter(ctx, sym) {
                let const_value = ctx
                    .sym(sym)
                    .ty
                    .and_then(|ty| ctx.typ(ty).const_value().cloned());
                if ctx.sym(sym).kind == Kind::VAR
                    && ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH)
                    && const_value.is_none()
                {
                    self.capture_symbol(ctx, sym, LambdaSymbolKind::CAPTURED_VAR);
                } else if ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::TYP) {
                    self.capture_this_symbol(ctx, sym);
                }
            }
        }
        self.analyzer_default_visit_ident(ctx, tree);
    }
    fn analyzer_visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyze_lambda(ctx, tree, None);
    }
    fn analyze_lambda_with_receiver(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        method_reference_receiver: Option<TreeId>,
    ) {
        let receiver = method_reference_receiver
            .and_then(|receiver| self.translate(ctx, Some(receiver)));
        let context = self.analyze_lambda(ctx, tree, receiver);
        let _ = context;
        if let Some(TranslationContext {
            kind: TranslationContextKind::Lambda(lambda),
            ..
        }) = self.context_map.borrow_mut().get_mut(&tree)
        {
            lambda.from_member_reference = true;
        }
        self.dump_mref_stat1(ctx, tree);
    }
    /// Java: LambdaToMethod.analyzeLambda (LambdaToMethod.java:1422-1424) -
    /// reports "mref.stat.1" for a member reference converted to a lambda.
    fn dump_mref_stat1(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if !self.dump_lambda_to_method_stats {
            return;
        }
        let (needs_alt_metafactory, target, translated_sym) = {
            let contexts = self.context_map.borrow();
            let Some(context) = contexts.get(&tree) else {
                return;
            };
            let TranslationContextKind::Lambda(lambda) = &context.kind else {
                return;
            };
            (
                ctx.typ(lambda.target).is_intersection() || context.bridges.len() > 1,
                lambda.target,
                lambda.translated_sym,
            )
        };
        let needs_alt_metafactory =
            needs_alt_metafactory || self.lambda_context_is_serializable(ctx, target);
        let name = crate::comp::resolve::symbol_display_name(ctx, translated_sym);
        let note = crate::util::jc_diagnostic::Note::from(
            crate::resources::compiler_properties::notes::mref_stat1(
                needs_alt_metafactory,
                crate::resources::diagnostic_contract::SymbolArg::placeholder(name),
            ),
        );
        let owner = crate::code::symbol::encl_class(ctx, translated_sym);
        let sourcefile = owner.and_then(|owner| match &ctx.sym(owner).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => {
                sourcefile.clone()
            }
            _ => None,
        });
        let prev_source = crate::util::abstract_log::AbstractLog::use_source(
            &mut *self.log.borrow_mut(),
            sourcefile,
        );
        crate::util::abstract_log::AbstractLog::note_at(
            &mut *self.log.borrow_mut(),
            ctx.tree(tree).pos,
            note,
        );
        crate::util::abstract_log::AbstractLog::use_source(
            &mut *self.log.borrow_mut(),
            prev_source,
        );
    }
    fn analyze_lambda(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        method_reference_receiver: Option<TreeId>,
    ) -> Option<()> {
        let previous_stack = self.frame_stack.clone();
        let context = self.new_lambda_context(ctx, tree);
        self.frame_stack.insert(0, Frame::new(tree));
        // The context must be in the map before its parameters are added:
        // add_symbol_to_context looks the context up by tree id.
        self.context_map.borrow_mut().insert(tree, context);
        let params = match &ctx.tree(tree).kind {
            TreeKind::Lambda { params, .. } => params.clone(),
            _ => panic!("expected a lambda"),
        };
        for param in params.iter() {
            let sym = match &ctx.tree(param).kind {
                TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                _ => panic!("expected a lambda parameter"),
            };
            self.add_symbol_to_context(ctx, tree, sym, LambdaSymbolKind::PARAM);
            if let Some(frame) = self.frame_stack.first_mut() {
                frame.add_local(sym);
            }
        }
        self.analyzer_default_visit_lambda(ctx, tree);
        self.complete_lambda_context(ctx, tree, method_reference_receiver);
        self.frame_stack = previous_stack;
        Some(())
    }
    fn analyzer_visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let previous_stack = self.frame_stack.clone();
        self.frame_stack.insert(0, Frame::new(tree));
        self.analyzer_default_visit_method_def(ctx, tree);
        self.frame_stack = previous_stack;
    }
    fn analyzer_visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let class_ty = ctx.tree(tree).ty.expect("new class type");
        let def = ctx.typ(class_ty).tsym.expect("new class symbol");
        let in_referenced_class = self.currently_in_class(ctx, def);
        let is_local = crate::code::symbol::Symbol::is_directly_or_indirectly_local(ctx, def);
        if std::env::var_os("DSH_DEBUG_ANON").is_some() {
            eprintln!(
                "[ANON] new_pos={} ty_sym={} def={} def_kind={:?} is_local={} in_ref={} has_def={}",
                ctx.tree(tree).pos,
                ctx.typ(class_ty).tsym.map(|s| ctx.sym(s).name.to_java_string().to_string_lossy().to_string()).unwrap_or_default(),
                ctx.sym(def).name.to_java_string().to_string_lossy(),
                ctx.sym(def).kind,
                is_local,
                in_referenced_class,
                matches!(ctx.tree(tree).kind, TreeKind::NewClass { def: Some(_), .. })
            );
        }
        if (in_referenced_class && is_local) || self.lambda_new_class_filter(ctx, tree) {
            let enclosing_ty = ctx.typ(class_ty).get_enclosing_type();
            let outer_instance_symbol = enclosing_ty.and_then(|encl| ctx.typ(encl).tsym);
            let mut local_context = self.context_key(ctx);
            while let Some(context_tree) = local_context {
                let context_owner = self
                    .context_map
                    .borrow()
                    .get(&context_tree)
                    .map(|context| context.owner)
                    .expect("context owner");
                if ctx.sym(context_owner).flags_field & flags::STATIC as i64 != 0 {
                    break;
                }
                if ctx.tree(context_tree).has_tag(Tag::LAMBDA) {
                    if let Some(outer) = outer_instance_symbol {
                        let depth = self
                            .context_map
                            .borrow()
                            .get(&context_tree)
                            .map(|context| context.depth)
                            .expect("context depth");
                        if self.captured_decl(ctx, depth, outer).is_none() {
                            break;
                        }
                    }
                    if let Some(outer) = outer_instance_symbol {
                        self.add_lambda_symbol(ctx, context_tree, outer, LambdaSymbolKind::CAPTURED_THIS);
                    }
                }
                local_context = self
                    .context_map
                    .borrow()
                    .get(&context_tree)
                    .and_then(|context| context.prev_context);
            }
        }
        self.analyzer_default_visit_new_class(ctx, tree);
        if self.context_key(ctx).is_some() && !in_referenced_class && is_local {
            self.capture_local_class_defs(ctx, def, self.context_key(ctx).expect("context"));
        }
    }
    fn capture_local_class_defs(
        &mut self,
        ctx: &mut CompilerContext,
        class_sym: SymbolId,
        lambda_context: TreeId,
    ) {
        // A local/anonymous class defined in a lambda body is owned by the
        // synthetic lambda method (upstream Lower then collects the lambda
        // parameter as a free variable of the class). Leaving the owner at the
        // enclosing <clinit> loses that capture and emits an empty constructor.
        let translated = {
            let contexts = self.context_map.borrow();
            match contexts.get(&lambda_context).map(|context| &context.kind) {
                Some(TranslationContextKind::Lambda(lambda)) => lambda.translated_sym,
                _ => return,
            }
        };
        ctx.sym_mut(class_sym).owner = Some(translated);
        let local_class_defs = self.local_class_defs.clone();
        let lower = Rc::clone(&self.lower);
        let frame_stack = self.frame_stack.clone();
        let names = Rc::clone(&self.names);
        let syms = Rc::clone(&self.syms);
        let types = Rc::clone(&self.types);
        capture_local_class_defs_static(
            ctx,
            &self.context_map,
            &local_class_defs,
            &lower,
            &names,
            &syms,
            &types,
            &frame_stack,
            class_sym,
            lambda_context,
        );
    }
    fn currently_in_class(&self, ctx: &CompilerContext, csym: SymbolId) -> bool {
        for frame in &self.frame_stack {
            if !ctx.tree(frame.tree).has_tag(Tag::CLASSDEF) {
                continue;
            }
            if let TreeKind::ClassDecl { sym: Some(class_sym), .. } = &ctx.tree(frame.tree).kind {
                if *class_sym == csym {
                    return true;
                }
            }
        }
        false
    }
    fn analyzer_visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        // reference (needsConversionToLambda decides whether it becomes a lambda).
        let owner = self.owner(ctx, true).expect("owner");
        let depth = self.frame_stack.len() as i32 - 1;
        let prev_context = self.context_key(ctx);
        let target = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { target: Some(target), .. } => *target,
            _ => panic!("expected an attributed member reference"),
        };
        let is_super = matches!(
            &ctx.tree(tree).kind,
            TreeKind::MemberReference { kind: Some(crate::tree::jc_tree::ReferenceKind::SUPER), .. }
        );
        let context = TranslationContext {
            tree,
            owner,
            depth,
            bridges: Vec::new(),
            prev_context,
            kind: TranslationContextKind::Reference(ReferenceTranslationContext {
                is_super,
                target,
            }),
        };
        self.context_map.borrow_mut().insert(tree, context);
        let needs_conversion = self.reference_needs_conversion(ctx, tree);
        if needs_conversion {
            let receiver = self.member_reference_receiver(ctx, tree);
            let lambda = self.member_reference_to_lambda(ctx, tree);
            self.analyze_lambda_with_receiver(ctx, lambda, receiver);
        } else {
            self.analyzer_default_visit_reference(ctx, tree);
        }
    }
    fn analyzer_visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (sym, name) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { sym, name, .. } => (*sym, name.clone()),
            _ => panic!("expected a field access"),
        };
        if self.context_key(ctx).is_some()
            && sym.map(|sym| ctx.sym(sym).kind) == Some(Kind::VAR)
            && (name == self.names._this || name == self.names._super)
        {
            let mut local_context = self.context_key(ctx);
            while let Some(context_tree) = local_context {
                let context_owner = self
                    .context_map
                    .borrow()
                    .get(&context_tree)
                    .map(|context| context.owner)
                    .expect("context owner");
                if ctx.sym(context_owner).flags_field & flags::STATIC as i64 != 0 {
                    break;
                }
                if ctx.tree(context_tree).has_tag(Tag::LAMBDA) {
                    let depth = self
                        .context_map
                        .borrow()
                        .get(&context_tree)
                        .map(|context| context.depth)
                        .expect("context depth");
                    let decl = sym.and_then(|sym| self.captured_decl(ctx, depth, sym));
                    let Some(decl) = decl else {
                        break;
                    };
                    let class_sym = match &ctx.tree(decl).kind {
                        TreeKind::ClassDecl { sym: Some(class_sym), .. } => *class_sym,
                        _ => break,
                    };
                    self.add_lambda_symbol(ctx, context_tree, class_sym, LambdaSymbolKind::CAPTURED_THIS);
                }
                local_context = self
                    .context_map
                    .borrow()
                    .get(&context_tree)
                    .and_then(|context| context.prev_context);
            }
        }
        self.analyzer_default_visit_select(ctx, tree);
    }
    fn analyzer_visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let context = self.context_key(ctx);
        let sym = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl { sym, .. } => *sym,
            _ => None,
        };
        if let Some(context_tree) = context {
            let is_lambda = matches!(
                self.context_map.borrow().get(&context_tree).map(|context| &context.kind),
                Some(TranslationContextKind::Lambda(_))
            );
            if is_lambda {
                if let Some(sym) = sym {
                    let frames = self.frame_stack.clone();
                    for frame in &frames {
                        if ctx.tree(frame.tree).has_tag(Tag::VARDEF) {
                            continue;
                        } else if ctx.tree(frame.tree).has_tag(Tag::LAMBDA) {
                            self.add_lambda_symbol(ctx, frame.tree, sym, LambdaSymbolKind::LOCAL_VAR);
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        if let Some(sym) = sym {
            if ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH) {
                if std::env::var_os("DSH_DEBUG_L2M").is_some()
                    && ctx.sym(sym).name.to_java_string().to_string_lossy() == "gson"
                {
                    eprintln!(
                        "[VARDEF] gson frames={:?}",
                        self.frame_stack
                            .iter()
                            .map(|f| ctx.tree(f.tree).tag)
                            .collect::<Vec<_>>(),
                    );
                }
                if let Some(frame) = self.frame_stack.first_mut() {
                    frame.add_local(sym);
                }
            }
        }
        // Clone after recording the declaration so leaving the var-def frame
        // keeps the local visible to the enclosing method/lambda frames.
        let previous_stack = self.frame_stack.clone();
        self.frame_stack.insert(0, Frame::new(tree));
        self.analyzer_default_visit_var_def(ctx, tree);
        self.frame_stack = previous_stack;
    }
}

impl LambdaAnalyzerPreprocessor {
    /// Return a valid owner given the current declaration stack
    /// (required to skip synthetic lambda symbols)
    fn owner(&mut self, ctx: &mut CompilerContext, skip_lambda: bool) -> Option<SymbolId> {
        let mut index = 0;
        while index < self.frame_stack.len() {
            let frame_tree = self.frame_stack[index].tree;
            let tag = ctx.tree(frame_tree).tag;
            match tag {
                Tag::VARDEF => {
                    let var_sym = match &ctx.tree(frame_tree).kind {
                        TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                        _ => panic!("expected a variable declaration"),
                    };
                    if crate::code::symbol::Symbol::is_directly_or_indirectly_local(ctx, var_sym) {
                        index += 1;
                        continue;
                    }
                    let class_tree = self.frame_stack[index + 1].tree;
                    let class_sym = match &ctx.tree(class_tree).kind {
                        TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                        _ => panic!("expected a class declaration"),
                    };
                    return Some(self.init_sym(ctx, class_sym, ctx.sym(var_sym).flags_field & flags::STATIC as i64));
                }
                Tag::BLOCK => {
                    let class_tree = self.frame_stack[index + 1].tree;
                    let class_sym = match &ctx.tree(class_tree).kind {
                        TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                        _ => panic!("expected a class declaration"),
                    };
                    let block_flags = match &ctx.tree(frame_tree).kind {
                        TreeKind::Block { flags, .. } => *flags,
                        _ => panic!("expected a block"),
                    };
                    return Some(self.init_sym(ctx, class_sym, block_flags & flags::STATIC as i64));
                }
                Tag::CLASSDEF => {
                    return match &ctx.tree(frame_tree).kind {
                        TreeKind::ClassDecl { sym: Some(sym), .. } => Some(*sym),
                        _ => None,
                    };
                }
                Tag::METHODDEF => {
                    return match &ctx.tree(frame_tree).kind {
                        TreeKind::MethodDecl { sym: Some(sym), .. } => Some(*sym),
                        _ => None,
                    };
                }
                Tag::LAMBDA if !skip_lambda => {
                    return self
                        .context_map
                        .borrow()
                        .get(&frame_tree)
                        .map(|context| context.owner);
                }
                _ => index += 1,
            }
        }
        panic!("AssertionError: owner not found")
    }
    fn init_sym(&mut self, ctx: &mut CompilerContext, class_sym: SymbolId, class_flags: i64) -> SymbolId {
        if class_flags & flags::STATIC as i64 != 0 {
            let attr = Rc::clone(&self.attr);
            let attr = attr.borrow();
            if let Some(clinit) = attr.remove_clinit(class_sym) {
                self.clinits.insert(class_sym, clinit);
                return clinit;
            }
            if let Some(clinit) = self.clinits.get(&class_sym).copied() {
                return clinit;
            }
            let void_type = self.syms.borrow().void_type;
            let method_class = self.syms.borrow().method_class;
            let method_type = ctx.alloc_type(Type::new_method_type(
                List::nil(),
                void_type,
                List::nil(),
                method_class,
            ));
            let clinit = self.make_private_synthetic_method(
                ctx,
                flags::STATIC as i64,
                self.names.clinit.clone(),
                Some(method_type),
                class_sym,
            );
            self.clinits.insert(class_sym, clinit);
            return clinit;
        }
        let members = match &ctx.sym(class_sym).kind_data {
            SymbolKind::ClassSymbol { members_field, .. } => *members_field,
            _ => None,
        };
        if let Some(members) = members {
            let init = self.names.init.clone();
            let symbols = ctx.scope_get_symbols_by_name(
                members,
                &init,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
            );
            if let Some(first) = symbols.first() {
                return *first;
            }
        }
        panic!("AssertionError: init not found")
    }
    fn directly_enclosing_lambda(&self, ctx: &CompilerContext) -> Option<TreeId> {
        for frame in &self.frame_stack {
            let tag = ctx.tree(frame.tree).tag;
            match tag {
                Tag::CLASSDEF | Tag::METHODDEF => return None,
                Tag::LAMBDA => return Some(frame.tree),
                _ => {}
            }
        }
        None
    }
    /// Port note: upstream defines this helper but never calls it;
    /// it is kept for 1:1 traceability.
    #[allow(dead_code)]
    fn in_class_within_lambda(&self, ctx: &CompilerContext) -> bool {
        let mut class_found = false;
        for frame in &self.frame_stack {
            match ctx.tree(frame.tree).tag {
                Tag::LAMBDA => return class_found,
                Tag::CLASSDEF => class_found = true,
                _ => {}
            }
        }
        false
    }
    /// Return the declaration corresponding to a symbol in the enclosing
    /// scope; the depth parameter is used to filter out symbols defined
    /// in nested scopes (which do not need to undergo capture).
    fn captured_decl(&self, ctx: &mut CompilerContext, depth: i32, sym: SymbolId) -> Option<TreeId> {
        let total = self.frame_stack.len() as i32;
        for (index, frame) in self.frame_stack.iter().enumerate() {
            // Depth grows outwards from the class (index len-1) towards the
            // innermost lambda (index 0), matching LambdaTranslationContext.depth.
            let current_depth = total - 1 - index as i32;
            match ctx.tree(frame.tree).tag {
                Tag::CLASSDEF => {
                    let class_sym = match &ctx.tree(frame.tree).kind {
                        TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                        _ => continue,
                    };
                    let is_member = ctx
                        .sym(sym)
                        .owner
                        .map(|owner| owner == class_sym)
                        .unwrap_or(false);
                    if is_member {
                        return if current_depth > depth { None } else { Some(frame.tree) };
                    }
                }
                Tag::VARDEF => {
                    let var_sym = match &ctx.tree(frame.tree).kind {
                        TreeKind::VariableDecl { sym, .. } => *sym,
                        _ => None,
                    };
                    if (var_sym == Some(sym) && ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH))
                        || frame.contains(sym)
                    {
                        return if current_depth > depth { None } else { Some(frame.tree) };
                    }
                }
                Tag::BLOCK | Tag::METHODDEF | Tag::LAMBDA => {
                    if frame.contains(sym) {
                        return if current_depth > depth { None } else { Some(frame.tree) };
                    }
                }
                _ => panic!("AssertionError: bad decl kind"),
            }
        }
        None
    }
    fn context_key(&self, _ctx: &CompilerContext) -> Option<TreeId> {
        for frame in &self.frame_stack {
            if self.context_map.borrow().contains_key(&frame.tree) {
                return Some(frame.tree);
            }
        }
        None
    }
    fn current_class(&self, ctx: &CompilerContext) -> Option<SymbolId> {
        for frame in &self.frame_stack {
            if ctx.tree(frame.tree).has_tag(Tag::CLASSDEF) {
                if let TreeKind::ClassDecl { sym: Some(sym), .. } = &ctx.tree(frame.tree).kind {
                    return Some(*sym);
                }
            }
        }
        None
    }
    /// This is used to filter out those identifiers that needs to be adjusted
    /// when translating away lambda expressions
    fn lambda_ident_symbol_filter(&self, ctx: &CompilerContext, sym: SymbolId) -> bool {
        let symbol = ctx.sym(sym);
        (symbol.kind == Kind::VAR || symbol.kind == Kind::MTH)
            && symbol.flags_field & flags::STATIC as i64 == 0
            && symbol.name != self.names.init
    }
    /// This is used to filter out those select nodes that need to be adjusted
    /// when translating away lambda expressions - at the moment, this is the
    /// set of nodes that select this (qualified this)
    fn lambda_field_access_filter(&self, ctx: &mut CompilerContext, tree: TreeId) -> bool {
        let is_context_lambda = self
            .context_key(ctx)
            .map(|context| {
                matches!(
                    self.context_map.borrow().get(&context).map(|context| &context.kind),
                    Some(TranslationContextKind::Lambda(_))
                )
            })
            .unwrap_or(false);
        if !is_context_lambda {
            return false;
        }
        let (sym, name) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { sym, name, .. } => (*sym, name.clone()),
            _ => return false,
        };
        let Some(sym) = sym else {
            return false;
        };
        if name != self.names._this {
            return false;
        }
        if ctx.sym(sym).flags_field & flags::STATIC as i64 != 0 {
            return false;
        }
        if ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind) != Some(Kind::TYP) {
            return false;
        }
        let context = self.context_key(ctx).expect("context");
        let outer_count = match &self.context_map.borrow().get(&context).expect("context").kind {
            TranslationContextKind::Lambda(lambda) => lambda
                .translated_symbols
                .get(&LambdaSymbolKind::CAPTURED_OUTER_THIS)
                .map(|map| map.len())
                .unwrap_or(0),
            _ => 0,
        };
        outer_count > 0
    }
    /// This is used to filter out those new class expressions that need to be
    /// qualified with an enclosing tree
    fn lambda_new_class_filter(&self, ctx: &mut CompilerContext, tree: TreeId) -> bool {
        let context = self.context_key(ctx);
        let Some(context) = context else {
            return false;
        };
        let context_owner = self
            .context_map
            .borrow()
            .get(&context)
            .map(|context| context.owner)
            .expect("context owner");
        let TreeKind::NewClass { encl, def, clazz, .. } = &ctx.tree(tree).kind else {
            return false;
        };
        if encl.is_some() || def.is_some() {
            return false;
        }
        let Some(clazz) = clazz else {
            return false;
        };
        let class_ty = ctx.tree(*clazz).ty.expect("new class type");
        let enclosing = ctx.typ(class_ty).get_enclosing_type();
        let Some(enclosing) = enclosing else {
            return false;
        };
        let mut current = symbol::encl_class(ctx, context_owner)
            .and_then(|class| ctx.sym(class).ty)
            .expect("context owner class type");
        loop {
            if ctx.typ(current).get_tag() == TypeTag::NONE {
                return false;
            }
            let enclosing_tsym = ctx.typ(enclosing).tsym;
            let current_tsym = ctx.typ(current).tsym;
            if let (Some(current_tsym), Some(enclosing_tsym)) = (current_tsym, enclosing_tsym) {
                if crate::code::symbol::is_sub_class(ctx, &self.syms.borrow(), &self.types, current_tsym, enclosing_tsym) {
                    return true;
                }
            }
            let Some(parent) = ctx.typ(current).get_enclosing_type() else {
                return false;
            };
            current = parent;
        }
    }
}


impl LambdaAnalyzerPreprocessor {
    /// Completes the translated lambda method symbol: flags, synthetic
    /// parameters, name and signature (upstream LambdaTranslationContext.complete).
    fn complete_lambda_context(&mut self, ctx: &mut CompilerContext, tree: TreeId, method_reference_receiver: Option<TreeId>) {
        let (owner, translated_sym, target, assigned_to, self_sym, already, mut_receiver) = {
            let contexts = self.context_map.borrow();
            let Some(TranslationContext {
                owner,
                kind: TranslationContextKind::Lambda(lambda),
                ..
            }) = contexts.get(&tree)
            else {
                return;
            };
            (
                *owner,
                lambda.translated_sym,
                lambda.target,
                lambda.assigned_to,
                lambda.self_sym,
                lambda.synthetic_params.is_some(),
                lambda.method_reference_receiver,
            )
        };
        let _ = mut_receiver;
        if !already {
            let (param_orig, captured_orig, outer_this_orig, this_orig) = {
                let contexts = self.context_map.borrow();
                let lambda = match contexts.get(&tree).map(|context| &context.kind) {
                    Some(TranslationContextKind::Lambda(lambda)) => lambda,
                    _ => return,
                };
                (
                    captured_symbols(lambda, LambdaSymbolKind::PARAM),
                    captured_symbols(lambda, LambdaSymbolKind::CAPTURED_VAR)
                        .into_iter()
                        .map(|sym| (sym, context_symbol(lambda, LambdaSymbolKind::CAPTURED_VAR, sym).expect("captured var")))
                        .collect::<Vec<(SymbolId, SymbolId)>>(),
                    captured_symbols(lambda, LambdaSymbolKind::CAPTURED_OUTER_THIS)
                        .into_iter()
                        .map(|sym| (sym, context_symbol(lambda, LambdaSymbolKind::CAPTURED_OUTER_THIS, sym).expect("outer this")))
                        .collect::<Vec<(SymbolId, SymbolId)>>(),
                    captured_symbols(lambda, LambdaSymbolKind::CAPTURED_THIS),
                )
            };
            if std::env::var_os("DSH_DEBUG_L2M").is_some() {
                eprintln!(
                    "[COMPLETE] tree={tree:?} params={} captured={} outer_this={} this={}",
                    param_orig.len(),
                    captured_orig.len(),
                    outer_this_orig.len(),
                    this_orig.len(),
                );
            }
            let owner_flags = ctx.sym(owner).flags_field;
            let owner_owner_flags = ctx.sym(owner).owner.map(|encl| ctx.sym(encl).flags_field).unwrap_or(0);
            let in_interface = owner_flags & flags::INTERFACE as i64 != 0;
            let this_referenced = !this_orig.is_empty();
            let mut new_flags = flags::SYNTHETIC as i64
                | flags::LAMBDA_METHOD
                | (owner_flags & flags::STRICTFP as i64)
                | (owner_owner_flags & flags::STRICTFP as i64)
                | flags::PRIVATE as i64;
            if this_referenced {
                if in_interface {
                    new_flags |= flags::DEFAULT;
                }
            } else {
                new_flags |= flags::STATIC as i64;
            }
            ctx.sym_mut(translated_sym).flags_field = new_flags;
            let mut param_defs: List<TreeId> = List::nil();
            let mut param_symbols: List<SymbolId> = List::nil();
            let mut param_types: List<TypeId> = List::nil();
            for (_, translated) in captured_orig.iter().chain(outer_this_orig.iter()) {
                let var_def = {
                    let mut syms = self.syms.borrow_mut();
                    let mut make = self.make.borrow_mut();
                    make.var_def_symbol(ctx, &mut syms, *translated, None)
                };
                param_defs = param_defs.append(var_def);
                param_symbols = param_symbols.append(*translated);
                param_types = param_types.append(ctx.sym(*translated).ty.expect("synthetic param type"));
            }
            for sym in param_orig.iter() {
                let translated = {
                    let contexts = self.context_map.borrow();
                    match contexts.get(&tree).map(|context| &context.kind) {
                        Some(TranslationContextKind::Lambda(lambda)) => context_symbol(lambda, LambdaSymbolKind::PARAM, *sym).expect("translated param"),
                        _ => continue,
                    }
                };
                let var_def = {
                    let mut syms = self.syms.borrow_mut();
                    let mut make = self.make.borrow_mut();
                    make.var_def_symbol(ctx, &mut syms, translated, None)
                };
                param_defs = param_defs.append(var_def);
                param_symbols = param_symbols.append(translated);
                param_types = param_types.append(ctx.sym(translated).ty.expect("synthetic param type"));
            }
            let serializable = self.lambda_context_is_serializable(ctx, target);
            let name = if serializable {
                self.serialized_lambda_name(ctx, tree, owner, target, assigned_to, self_sym, &captured_orig)
            } else {
                self.lambda_name(ctx, owner)
            };
            ctx.sym_mut(translated_sym).name = name;
            if let SymbolKind::MethodSymbol { params: slot, .. } = &mut ctx.sym_mut(translated_sym).kind_data {
                *slot = param_symbols;
            }
            let descriptor = self
                .find_descriptor_type_checked(ctx, target)
                .expect("descriptor type");
            let generated = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, descriptor)
            };
            let new_type = self
                .types
                .create_method_type_with_parameters(ctx, generated, param_types);
            ctx.sym_mut(translated_sym).ty = Some(new_type);
            if let Some(context) = self.context_map.borrow_mut().get_mut(&tree) {
                if let TranslationContextKind::Lambda(lambda) = &mut context.kind {
                    lambda.synthetic_params = Some(param_defs.iter().collect::<Vec<TreeId>>());
                }
            }
        }
        if let Some(receiver) = method_reference_receiver {
            if let Some(context) = self.context_map.borrow_mut().get_mut(&tree) {
                if let TranslationContextKind::Lambda(lambda) = &mut context.kind {
                    lambda.method_reference_receiver = Some(receiver);
                }
            }
        }
    }

    /// Whether the lambda target requires serialization support.
    fn lambda_context_is_serializable(&self, ctx: &mut CompilerContext, target: TypeId) -> bool {
        if self.force_serializable {
            return true;
        }
        let serializable_tsym = self.syms.borrow().serializable_type;
        let serializable_tsym = ctx.typ(serializable_tsym).tsym.expect("serializable type symbol");
        let syms = self.syms.borrow();
        self.types.as_super(ctx, &syms, target, serializable_tsym).is_some()
    }

    /// The name of the enclosing method folded into synthetic lambda names.
    fn enclosing_method_name(&self, ctx: &CompilerContext, owner: SymbolId) -> String {
        self.synthetic_method_name_component(ctx, ctx.sym(owner).name.clone())
    }

    /// Method name in a form that can be folded into a synthetic method name.
    fn synthetic_method_name_component(&self, _ctx: &CompilerContext, name: Name) -> String {
        let method_name = name.to_java_string().to_string_lossy().to_string();
        if method_name == self.names.clinit.to_java_string().to_string_lossy().as_ref() {
            "static".to_string()
        } else if method_name == self.names.init.to_java_string().to_string_lossy().as_ref() {
            "new".to_string()
        } else {
            method_name
        }
    }

    /// Increments and returns the per-prefix synthetic method name count.
    fn synthetic_method_name_index(&mut self, key: &str) -> i32 {
        let index = self.synthetic_method_name_counts.entry(key.to_string()).or_insert(0);
        let result = *index;
        *index += 1;
        result
    }

    /// A simple synthetic lambda method name (upstream lambdaName).
    fn lambda_name(&mut self, ctx: &CompilerContext, owner: SymbolId) -> Name {
        let component = self.enclosing_method_name(ctx, owner);
        let count = self.lambda_count;
        self.lambda_count += 1;
        let suffix = self.names.from_string(&javac_strings::JavaString::from(
            format!("{}{}{}", component, "$", count).as_str(),
        ));
        self.names.lambda.append(&suffix)
    }

    /// The disambiguating string of a serializable lambda (upstream
    /// serializedLambdaDisambiguation).
    fn serialized_lambda_disambiguation(
        &self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        owner: SymbolId,
        target: TypeId,
        assigned_to: Option<SymbolId>,
        self_sym: Option<SymbolId>,
        captured_vars: &[(SymbolId, SymbolId)],
    ) -> String {
        let mut buffer = String::new();
        match ctx.sym(owner).ty {
            Some(owner_type) => {
                buffer.push_str(&self.type_sig(ctx, owner_type, true));
                buffer.push(':');
            }
            None => {
                assert!(
                    self.directly_enclosing_lambda(ctx).is_some(),
                    "AssertionError: owners without a type must be enclosed in a lambda"
                );
            }
        }
        let target_tsym = ctx.typ(target).tsym.expect("lambda target symbol");
        let descriptor = self
            .find_descriptor_symbol_checked(ctx, target_tsym)
            .expect("descriptor symbol");
        let descriptor_owner = ctx.sym(descriptor).owner.expect("descriptor owner");
        buffer.push_str(&ctx.sym(descriptor_owner).flat_name().to_java_string().to_string_lossy());
        buffer.push(' ');
        if let Some(assigned_to) = assigned_to {
            buffer.push_str(&ctx.sym(assigned_to).flat_name().to_java_string().to_string_lossy());
            buffer.push('=');
        }
        for (original, _) in captured_vars {
            if Some(*original) == self_sym {
                continue;
            }
            if let Some(type_) = ctx.sym(*original).ty {
                buffer.push_str(&self.type_sig(ctx, type_, true));
                buffer.push(' ');
            }
            buffer.push_str(&ctx.sym(*original).flat_name().to_java_string().to_string_lossy());
            buffer.push(',');
        }
        let _ = tree;
        buffer
    }

    /// A name that maximizes stability across deserialization (upstream
    /// serializedLambdaName).
    fn serialized_lambda_name(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        owner: SymbolId,
        target: TypeId,
        assigned_to: Option<SymbolId>,
        self_sym: Option<SymbolId>,
        captured_vars: &[(SymbolId, SymbolId)],
    ) -> Name {
        let mut buffer = self.names.lambda.to_java_string().to_string_lossy().to_string();
        buffer.push_str(&self.enclosing_method_name(ctx, owner));
        buffer.push('$');
        let disambiguation = self.serialized_lambda_disambiguation(
            ctx,
            tree,
            owner,
            target,
            assigned_to,
            self_sym,
            captured_vars,
        );
        let hash = disambiguation
            .encode_utf16()
            .fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as i32));
        buffer.push_str(&format!("{:x}", hash as u32));
        buffer.push('$');
        let index = self.synthetic_method_name_index(&buffer);
        buffer.push_str(&format!("{}", index));
        self.names.from_string(&javac_strings::JavaString::from(buffer.as_str()))
    }

    /// Translates an identifier inside a lambda to the mapped identifier
    /// (upstream LambdaTranslationContext.translate(JCIdent)).
    fn translate_lambda_ident(&mut self, ctx: &mut CompilerContext, context_tree: TreeId, tree: TreeId) -> Option<TreeId> {
        let (sym, ty) = match &ctx.tree(tree).kind {
            TreeKind::Ident { sym: Some(sym), .. } => (*sym, ctx.tree(tree).ty),
            _ => return None,
        };
        let symbols: Vec<(LambdaSymbolKind, Vec<(SymbolId, SymbolId)>)> = {
            let contexts = self.context_map.borrow();
            let lambda = match contexts.get(&context_tree).map(|context| &context.kind) {
                Some(TranslationContextKind::Lambda(lambda)) => lambda,
                _ => return None,
            };
            LambdaSymbolKind::values()
                .iter()
                .map(|kind| (*kind, lambda.symbol_map(*kind).clone()))
                .collect()
        };
        for (kind, map) in symbols {
            if kind == LambdaSymbolKind::CAPTURED_OUTER_THIS {
                let target_tsym = ctx.typ(ctx.sym(sym).owner.and_then(|owner| ctx.sym(owner).ty).expect("identifier owner type"))
                    .tsym;
                let mut proxy: Option<SymbolId> = None;
                for (original, _) in map.iter() {
                    let out_tsym = ctx.typ(ctx.sym(*original).ty.expect("outer this type")).tsym;
                    if let (Some(target_tsym), Some(out_tsym)) = (target_tsym, out_tsym) {
                        if crate::code::symbol::is_member_of(ctx, &self.syms.borrow(), &self.types, sym, out_tsym) {
                            proxy = match proxy {
                                None => Some(*original),
                                Some(current) => {
                                    let current_tsym = ctx.typ(ctx.sym(current).ty.expect("outer this type")).tsym;
                                    match current_tsym {
                                        Some(current_tsym) => {
                                            if crate::code::symbol::is_enclosed_by(ctx, *original, current_tsym) {
                                                Some(*original)
                                            } else {
                                                Some(current)
                                            }
                                        }
                                        None => Some(*original),
                                    }
                                }
                            };
                        }
                        let _ = target_tsym;
                    }
                }
                if let Some(proxy) = proxy {
                    let translated = map.iter().find(|(original, _)| *original == proxy).map(|(_, translated)| *translated).expect("translated outer this");
                    let owner_ty = ctx.sym(sym).owner.and_then(|owner| ctx.sym(owner).ty).expect("identifier owner type");
                    let outer = {
                        let mut make = self.make.borrow_mut();
                        let mut ident = make.ident_symbol(ctx, translated);
                        ctx.tree_mut(ident).ty = Some(owner_ty);
                        ident = {
                            let name = ctx.sym(sym).name.clone();
                            let mut make2 = self.make.borrow_mut();
                            make2.select(ctx, ident, name)
                        };
                        ident
                    };
                    ctx.tree_mut(outer).ty = ty;
                    if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(outer).kind {
                        *slot = Some(sym);
                    }
                    return Some(outer);
                }
            } else if let Some((_, translated)) = map.iter().find(|(original, _)| *original == sym) {
                let ident = {
                    let mut make = self.make.borrow_mut();
                    make.ident_symbol(ctx, *translated)
                };
                ctx.tree_mut(ident).ty = ty;
                return Some(ident);
            }
        }
        None
    }

    /// Translates qualified this references to the captured outer instance
    /// (upstream LambdaTranslationContext.translate(JCFieldAccess)).
    fn translate_lambda_select(&mut self, ctx: &mut CompilerContext, context_tree: TreeId, tree: TreeId) -> Option<TreeId> {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { sym: Some(sym), .. } => *sym,
            _ => return None,
        };
        let name = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { name, .. } => name.clone(),
            _ => return None,
        };
        assert!(name == self.names._this, "AssertionError: qualified this expected");
        let owner = ctx.sym(sym).owner.expect("qualified this owner");
        let translated = {
            let contexts = self.context_map.borrow();
            let lambda = match contexts.get(&context_tree).map(|context| &context.kind) {
                Some(TranslationContextKind::Lambda(lambda)) => lambda,
                _ => return None,
            };
            context_symbol(lambda, LambdaSymbolKind::CAPTURED_OUTER_THIS, owner)
        };
        let translated = translated?;
        let owner_ty = ctx.sym(owner).ty.expect("qualified this owner type");
        let ident = {
            let mut make = self.make.borrow_mut();
            make.ident_symbol(ctx, translated)
        };
        ctx.tree_mut(ident).ty = Some(owner_ty);
        Some(ident)
    }

    /// Anchors an instance creation with an implicit enclosing instance to the
    /// captured outer this (upstream LambdaTranslationContext.translate(JCNewClass)).
    fn translate_lambda_new_class(&mut self, ctx: &mut CompilerContext, context_tree: TreeId, tree: TreeId) -> TreeId {
        let (clazz, encl) = match &ctx.tree(tree).kind {
            TreeKind::NewClass { clazz, encl, .. } => (*clazz, *encl),
            _ => panic!("expected an instance creation"),
        };
        let clazz = clazz.expect("instance creation class");
        let clazz_ty = ctx.tree(clazz).ty.expect("instance creation class type");
        let clazz_tsym = ctx.typ(clazz_ty).tsym.expect("instance creation class symbol");
        assert!(
            ctx.sym(clazz_tsym).has_outer_instance(ctx) && encl.is_none(),
            "AssertionError: implicit enclosing instance expected"
        );
        let enclosing = ctx.typ(clazz_ty).get_enclosing_type().expect("enclosing type");
        let enclosing_tsym = ctx.typ(enclosing).tsym.expect("enclosing type symbol");
        let translated = {
            let contexts = self.context_map.borrow();
            let lambda = match contexts.get(&context_tree).map(|context| &context.kind) {
                Some(TranslationContextKind::Lambda(lambda)) => lambda,
                _ => return tree,
            };
            context_symbol(lambda, LambdaSymbolKind::CAPTURED_OUTER_THIS, enclosing_tsym)
        };
        if let Some(translated) = translated {
            let ident = {
                let mut make = self.make.borrow_mut();
                make.ident_symbol(ctx, translated)
            };
            ctx.tree_mut(ident).ty = Some(enclosing);
            if let TreeKind::NewClass { encl: slot, .. } = &mut ctx.tree_mut(tree).kind {
                *slot = Some(ident);
            }
        }
        tree
    }
    fn add_symbol_to_context(
        &mut self,
        ctx: &mut CompilerContext,
        context_tree: TreeId,
        sym: SymbolId,
        skind: LambdaSymbolKind,
    ) {
        let mut skind = skind;
        if skind == LambdaSymbolKind::CAPTURED_THIS
            && ctx.sym(sym).kind == Kind::TYP
            && !self.types_under_construction.is_empty()
        {
            if let Some(current_class) = self.current_class(ctx) {
                if self.types_under_construction.contains(&current_class) && sym != current_class {
                    skind = LambdaSymbolKind::CAPTURED_OUTER_THIS;
                }
            }
        }
        let translated = self.translate_lambda_symbol(ctx, context_tree, sym, skind);
        if let Some(translated) = translated {
            if let Some(context) = self.context_map.borrow_mut().get_mut(&context_tree) {
                if let TranslationContextKind::Lambda(lambda) = &mut context.kind {
                    lambda
                        .translated_symbols
                        .entry(skind)
                        .or_default()
                        .push((sym, translated));
                }
            }
        }
    }
    fn translate_lambda_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        context_tree: TreeId,
        sym: SymbolId,
        skind: LambdaSymbolKind,
    ) -> Option<SymbolId> {
        let translated_sym = match self.context_map.borrow().get(&context_tree) {
            Some(TranslationContext {
                kind: TranslationContextKind::Lambda(lambda),
                ..
            }) => lambda.translated_sym,
            _ => return None,
        };
        let sym_name = ctx.sym(sym).name.clone();
        let result = match skind {
            LambdaSymbolKind::CAPTURED_THIS => sym,
            LambdaSymbolKind::CAPTURED_VAR => {
                let syms = self.syms.borrow();
                let erased = self.types.erasure(ctx, &syms, ctx.sym(sym).ty.expect("captured var type"));
                drop(syms);
                self.new_var_symbol(
                    ctx,
                    flags::SYNTHETIC as i64 | flags::FINAL as i64 | flags::PARAMETER,
                    sym_name,
                    erased,
                    translated_sym,
                )
            }
            LambdaSymbolKind::CAPTURED_OUTER_THIS => {
                let flat = ctx.sym(sym).flat_name();
                let name = self.names.from_string(&javac_strings::JavaString::from(
                    format!("{}{}", flat.to_java_string().to_string_lossy().replace('.', "$"), self.names.dollarThis.to_java_string().to_string_lossy()).as_str(),
                ));
                let syms = self.syms.borrow();
                let erased = self.types.erasure(ctx, &syms, ctx.sym(sym).ty.expect("outer this type"));
                drop(syms);
                self.new_var_symbol(
                    ctx,
                    flags::SYNTHETIC as i64 | flags::FINAL as i64 | flags::PARAMETER,
                    name,
                    erased,
                    translated_sym,
                )
            }
            LambdaSymbolKind::LOCAL_VAR => {
                let syms = self.syms.borrow();
                let erased = self.types.erasure(ctx, &syms, ctx.sym(sym).ty.expect("local var type"));
                drop(syms);
                let var = self.new_var_symbol(
                    ctx,
                    ctx.sym(sym).flags_field & flags::FINAL as i64,
                    sym_name,
                    erased,
                    translated_sym,
                );
                if let Some(pos) = ctx.sym(sym).var_pos() {
                    ctx.sym_mut(var).set_var_pos(pos);
                }
                var
            }
            LambdaSymbolKind::PARAM => {
                let syms = self.syms.borrow();
                let erased = self.types.erasure(ctx, &syms, ctx.sym(sym).ty.expect("param type"));
                drop(syms);
                let var = self.new_var_symbol(
                    ctx,
                    (ctx.sym(sym).flags_field & flags::FINAL as i64) | flags::PARAMETER,
                    sym_name,
                    erased,
                    translated_sym,
                );
                if let Some(pos) = ctx.sym(sym).var_pos() {
                    ctx.sym_mut(var).set_var_pos(pos);
                }
                var
            }
        };
        if skind.propagate_annotations() && result != sym {
            let raw = ctx.sym(sym).get_raw_attributes();
            let type_raw = ctx.sym(sym).get_raw_attributes();
            ctx.sym_mut(result).set_declaration_attributes(raw);
            ctx.sym_mut(result).set_type_attributes(type_raw);
        }
        Some(result)
    }
    fn add_lambda_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        context_tree: TreeId,
        sym: SymbolId,
        skind: LambdaSymbolKind,
    ) {
        let already = self
            .context_map
            .borrow()
            .get(&context_tree)
            .map(|context| match &context.kind {
                TranslationContextKind::Lambda(lambda) => lambda
                    .translated_symbols
                    .get(&skind)
                    .map(|map| map.iter().any(|(original, _)| *original == sym))
                    .unwrap_or(false),
                _ => false,
            })
            .unwrap_or(false);
        if !already {
            self.add_symbol_to_context(ctx, context_tree, sym, skind);
        }
    }
    fn capture_symbol(&mut self, ctx: &mut CompilerContext, sym: SymbolId, skind: LambdaSymbolKind) {
        let mut local_context = self.context_key(ctx);
        while let Some(context_tree) = local_context {
            if ctx.tree(context_tree).has_tag(Tag::LAMBDA) {
                let depth = self
                    .context_map
                    .borrow()
                    .get(&context_tree)
                    .map(|context| context.depth)
                    .expect("context depth");
                if std::env::var_os("DSH_DEBUG_L2M").is_some() {
                    eprintln!(
                        "[CAPSYM] tree={} depth={} decl={:?} frames={:?}",
                        ctx.sym(sym).name.to_java_string().to_string_lossy(),
                        depth,
                        self.captured_decl(ctx, depth, sym),
                        self.frame_stack
                            .iter()
                            .map(|f| (ctx.tree(f.tree).tag, f.contains(sym)))
                            .collect::<Vec<_>>(),
                    );
                }
                if self.captured_decl(ctx, depth, sym).is_none() {
                    break;
                }
                self.add_lambda_symbol(ctx, context_tree, sym, skind);
            }
            local_context = self
                .context_map
                .borrow()
                .get(&context_tree)
                .and_then(|context| context.prev_context);
        }
    }
    fn capture_this_symbol(&mut self, ctx: &mut CompilerContext, sym: SymbolId) {
        let mut local_context = self.context_key(ctx);
        while let Some(context_tree) = local_context {
            let context_owner = self
                .context_map
                .borrow()
                .get(&context_tree)
                .map(|context| context.owner)
                .expect("context owner");
            if ctx.sym(context_owner).flags_field & flags::STATIC as i64 != 0 {
                break;
            }
            if ctx.tree(context_tree).has_tag(Tag::LAMBDA) {
                let depth = self
                    .context_map
                    .borrow()
                    .get(&context_tree)
                    .map(|context| context.depth)
                    .expect("context depth");
                let decl = self.captured_decl(ctx, depth, sym);
                let Some(decl) = decl else {
                    break;
                };
                let class_sym = match &ctx.tree(decl).kind {
                    TreeKind::ClassDecl { sym: Some(class_sym), .. } => *class_sym,
                    _ => break,
                };
                self.add_lambda_symbol(ctx, context_tree, class_sym, LambdaSymbolKind::CAPTURED_THIS);
            }
            local_context = self
                .context_map
                .borrow()
                .get(&context_tree)
                .and_then(|context| context.prev_context);
        }
    }
    fn set_class_owner(&mut self, ctx: &mut CompilerContext, class_sym: SymbolId, owner: SymbolId) {
        let mut symbol = ctx.sym(class_sym).clone();
        symbol.owner = Some(owner);
        symbol.id.set(Some(class_sym));
        *ctx.sym_mut(class_sym) = symbol;
    }
    fn new_lambda_context(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TranslationContext {
        let owner = self.owner(ctx, true).expect("owner");
        let depth = self.frame_stack.len() as i32 - 1;
        let prev_context = self.context_key(ctx);
        let attr_env = self.attr_env.clone().expect("attrEnv");
        let target = match &ctx.tree(tree).kind {
            TreeKind::Lambda { target: Some(target), .. } => *target,
            _ => panic!("expected an attributed lambda"),
        };
        let (mut syms, chk, diags) = {
            let attr = self.attr.borrow();
            let chk = Rc::clone(&attr.chk);
            let _ = &chk;
            (self.syms.borrow_mut(), chk, Rc::clone(&self.diags))
        };
        let empty = self.names.empty.clone();
        let csym = self
            .types
            .make_functional_interface_class(
                ctx,
                &mut syms,
                &diags,
                &chk,
                &attr_env,
                empty,
                target,
                flags::ABSTRACT as i64 | flags::INTERFACE as i64,
            )
            .expect("functional interface class");
        let bridges = {
            let trans_types = self.trans_types.borrow();
            let enter_rc = Rc::clone(&trans_types.enter);
            let enter = enter_rc.borrow();
            let chk2 = Rc::clone(&trans_types.chk);
            self.types
                .functional_interface_bridges(ctx, &mut syms, &diags, &chk2, &enter, csym)
        };
        drop(syms);
        let (self_sym, assigned_to) = self.assigned_symbols(ctx);
        let class_sym = symbol::encl_class(ctx, owner).expect("enclosing class");
        let translated_sym = if ctx.sym(owner).kind == Kind::MTH {
            let original_owner = ctx.alloc_symbol(Symbol::new_method_symbol(
                ctx.sym(owner).flags_field,
                ctx.sym(owner).name.clone(),
                ctx.sym(owner).ty.expect("owner type"),
                ctx.sym(owner).owner.expect("owner owner"),
            ));
            ctx.alloc_symbol(Symbol::new(
                Kind::MTH,
                flags::SYNTHETIC as i64 | flags::PRIVATE as i64,
                self.names.empty.clone(),
                None,
                Some(class_sym),
                SymbolKind::MethodSymbol {
                    extra_params: List::nil(),
                    captured_locals: List::nil(),
                    params: List::nil(),
                    default_value: None,
                    original_enclosing_method: Some(original_owner),
                },
            ))
        } else {
            self.make_private_synthetic_method(ctx, 0, self.names.empty.clone(), None, class_sym)
        };
        let mut translated_symbols: HashMap<LambdaSymbolKind, Vec<(SymbolId, SymbolId)>> = HashMap::new();
        for kind in LambdaSymbolKind::values() {
            translated_symbols.insert(kind, Vec::new());
        }
        TranslationContext {
            tree,
            owner,
            depth,
            bridges,
            prev_context,
            kind: TranslationContextKind::Lambda(LambdaTranslationContext {
                target,
                from_member_reference: false,
                self_sym,
                assigned_to,
                translated_symbols,
                translated_sym,
                synthetic_params: None,
                free_var_processed_local_classes: HashSet::new(),
                method_reference_receiver: None,
            }),
        }
    }
    fn assigned_symbols(&self, ctx: &mut CompilerContext) -> (Option<SymbolId>, Option<SymbolId>) {
        let Some(frame) = self.frame_stack.first() else {
            return (None, None);
        };
        match ctx.tree(frame.tree).tag {
            Tag::VARDEF => match &ctx.tree(frame.tree).kind {
                TreeKind::VariableDecl { sym, .. } => (*sym, *sym),
                _ => (None, None),
            },
            Tag::ASSIGN => match &ctx.tree(frame.tree).kind {
                TreeKind::Assign { lhs: Some(lhs), .. } => (None, tree_info::symbol(ctx, *lhs)),
                _ => (None, None),
            },
            _ => (None, None),
        }
    }
}


/// Creates a synthetic variable symbol with the given services (static
/// capture adapter for newVarSymbol).
fn new_var_symbol_static(
    ctx: &mut CompilerContext,
    flags_field: i64,
    name: Name,
    ty: TypeId,
    owner: SymbolId,
) -> SymbolId {
    ctx.alloc_symbol(Symbol::new(
        Kind::VAR,
        flags_field,
        name,
        Some(ty),
        Some(owner),
        SymbolKind::VarSymbol {
            lazy_const_value: None,
            pos: 0,
            adr: 0,
            data_kind: None,
            data: None,
        },
    ))
}

/// Static capture adapter for LambdaTranslationContext.addSymbol.
fn add_lambda_symbol_ro(
    ctx: &mut CompilerContext,
    context_map: &Rc<RefCell<HashMap<TreeId, TranslationContext>>>,
    names: &Rc<Names>,
    syms: &Rc<RefCell<Symtab>>,
    types: &Rc<Types>,
    context_tree: TreeId,
    sym: SymbolId,
    skind: LambdaSymbolKind,
) {
    let already = {
        let contexts = context_map.borrow();
        contexts
            .get(&context_tree)
            .map(|context| match &context.kind {
                TranslationContextKind::Lambda(lambda) => lambda
                    .symbol_map(skind)
                    .iter()
                    .any(|(original, _)| *original == sym),
                _ => false,
            })
            .unwrap_or(false)
    };
    if already {
        return;
    }
    let translated_sym = {
        let contexts = context_map.borrow();
        match contexts.get(&context_tree).map(|context| &context.kind) {
            Some(TranslationContextKind::Lambda(lambda)) => lambda.translated_sym,
            _ => return,
        }
    };
    let sym_name = ctx.sym(sym).name.clone();
    let translated = match skind {
        LambdaSymbolKind::CAPTURED_THIS => sym,
        LambdaSymbolKind::CAPTURED_VAR => {
            let erased = {
                let syms_borrow = syms.borrow();
                types.erasure(ctx, &syms_borrow, ctx.sym(sym).ty.expect("captured var type"))
            };
            new_var_symbol_static(
                ctx,
                flags::SYNTHETIC as i64 | flags::FINAL as i64 | flags::PARAMETER,
                sym_name,
                erased,
                translated_sym,
            )
        }
        LambdaSymbolKind::CAPTURED_OUTER_THIS => {
            let flat = ctx.sym(sym).flat_name();
            let flat_name = flat.to_java_string().to_string_lossy().replace('.', "$");
            let name = names.from_string(&javac_strings::JavaString::from(
                format!("{}{}", flat_name, names.dollarThis.to_java_string().to_string_lossy()).as_str(),
            ));
            let erased = {
                let syms_borrow = syms.borrow();
                types.erasure(ctx, &syms_borrow, ctx.sym(sym).ty.expect("outer this type"))
            };
            new_var_symbol_static(
                ctx,
                flags::SYNTHETIC as i64 | flags::FINAL as i64 | flags::PARAMETER,
                name,
                erased,
                translated_sym,
            )
        }
        LambdaSymbolKind::LOCAL_VAR => {
            let erased = {
                let syms_borrow = syms.borrow();
                types.erasure(ctx, &syms_borrow, ctx.sym(sym).ty.expect("local var type"))
            };
            let var = new_var_symbol_static(
                ctx,
                ctx.sym(sym).flags_field & flags::FINAL as i64,
                sym_name,
                erased,
                translated_sym,
            );
            if let Some(pos) = ctx.sym(sym).var_pos() {
                ctx.sym_mut(var).set_var_pos(pos);
            }
            var
        }
        LambdaSymbolKind::PARAM => {
            let erased = {
                let syms_borrow = syms.borrow();
                types.erasure(ctx, &syms_borrow, ctx.sym(sym).ty.expect("param type"))
            };
            let var = new_var_symbol_static(
                ctx,
                (ctx.sym(sym).flags_field & flags::FINAL as i64) | flags::PARAMETER,
                sym_name,
                erased,
                translated_sym,
            );
            if let Some(pos) = ctx.sym(sym).var_pos() {
                ctx.sym_mut(var).set_var_pos(pos);
            }
            var
        }
    };
    if skind.propagate_annotations() && translated != sym {
        let raw = ctx.sym(sym).get_raw_attributes();
        let type_raw = ctx.sym(sym).get_raw_type_attributes();
        ctx.sym_mut(translated).set_declaration_attributes(raw);
        ctx.sym_mut(translated).set_type_attributes(type_raw);
    }
    if let Some(context) = context_map.borrow_mut().get_mut(&context_tree) {
        if let TranslationContextKind::Lambda(lambda) = &mut context.kind {
            lambda.symbol_map_mut(skind).push((sym, translated));
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn capture_local_class_defs_static(
    ctx: &mut CompilerContext,
    context_map: &Rc<RefCell<HashMap<TreeId, TranslationContext>>>,
    local_class_defs: &HashMap<SymbolId, TreeId>,
    lower: &Rc<RefCell<Lower>>,
    names: &Rc<Names>,
    syms: &Rc<RefCell<Symtab>>,
    types: &Rc<Types>,
    frame_stack: &[Frame],
    class_sym: SymbolId,
    lambda_context: TreeId,
) {
    let Some(local_cdef) = local_class_defs.get(&class_sym).copied() else {
        return;
    };
    {
        let mut contexts = context_map.borrow_mut();
        let Some(context) = contexts.get_mut(&lambda_context) else {
            return;
        };
        let TranslationContextKind::Lambda(lambda) = &mut context.kind else {
            return;
        };
        if !lambda.free_var_processed_local_classes.insert(class_sym) {
            return;
        }
    }
    struct CaptureVisitor<'a> {
        context_map: &'a Rc<RefCell<HashMap<TreeId, TranslationContext>>>,
        local_class_defs: &'a HashMap<SymbolId, TreeId>,
        lower: &'a Rc<RefCell<Lower>>,
        names: &'a Rc<Names>,
        syms: &'a Rc<RefCell<Symtab>>,
        types: &'a Rc<Types>,
        frame_stack: &'a [Frame],
        lambda_context: TreeId,
    }
    impl<'a> lower::BasicFreeVarCollectorVisitor for CaptureVisitor<'a> {
        fn add_free_vars(&mut self, ctx: &mut CompilerContext, class: SymbolId) {
            capture_local_class_defs_static(
                ctx,
                self.context_map,
                self.local_class_defs,
                self.lower,
                self.names,
                self.syms,
                self.types,
                self.frame_stack,
                class,
                self.lambda_context,
            );
        }

        fn visit_symbol(&mut self, ctx: &mut CompilerContext, sym: SymbolId) {
            let const_value = ctx
                .sym(sym)
                .ty
                .and_then(|ty| ctx.typ(ty).const_value().cloned());
            if ctx.sym(sym).kind == Kind::VAR
                && ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH)
                && const_value.is_none()
            {
                let mut local_context = context_key_ro(self.frame_stack, self.context_map);
                while let Some(context_tree) = local_context {
                    if ctx.tree(context_tree).has_tag(Tag::LAMBDA) {
                        let depth = self
                            .context_map
                            .borrow()
                            .get(&context_tree)
                            .map(|context| context.depth)
                            .expect("context depth");
                        if captured_decl_ro(ctx, self.frame_stack, depth, sym).is_none() {
                            break;
                        }
                        add_lambda_symbol_ro(
                            ctx,
                            self.context_map,
                            self.names,
                            self.syms,
                            self.types,
                            context_tree,
                            sym,
                            LambdaSymbolKind::CAPTURED_VAR,
                        );
                    }
                    local_context = self
                        .context_map
                        .borrow()
                        .get(&context_tree)
                        .and_then(|context| context.prev_context);
                }
            }
        }
    }
    let owner = ctx.sym(class_sym).owner.expect("local class owner");
    let lower_borrow = lower.borrow();
    let mut visitor = CaptureVisitor {
        context_map,
        local_class_defs,
        lower,
        names,
        syms,
        types,
        frame_stack,
        lambda_context,
    };
    lower::collect_free_vars(ctx, &lower_borrow, class_sym, owner, local_cdef, &mut visitor);
}

fn context_key_ro(
    frame_stack: &[Frame],
    context_map: &Rc<RefCell<HashMap<TreeId, TranslationContext>>>,
) -> Option<TreeId> {
    for frame in frame_stack {
        if context_map.borrow().contains_key(&frame.tree) {
            return Some(frame.tree);
        }
    }
    None
}

/// Return the declaration corresponding to a symbol in the enclosing
/// scope; the depth parameter is used to filter out symbols defined
/// in nested scopes (which do not need to undergo capture).
fn captured_decl_ro(
    ctx: &mut CompilerContext,
    frame_stack: &[Frame],
    depth: i32,
    sym: SymbolId,
) -> Option<TreeId> {
    let mut current_depth = frame_stack.len() as i32 - 1;
    for frame in frame_stack {
        match ctx.tree(frame.tree).tag {
            Tag::CLASSDEF => {
                let class_sym = match &ctx.tree(frame.tree).kind {
                    TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
                    _ => {
                        current_depth -= 1;
                        continue;
                    }
                };
                let is_member = ctx
                    .sym(sym)
                    .owner
                    .map(|owner| owner == class_sym)
                    .unwrap_or(false);
                if is_member {
                    return if current_depth > depth { None } else { Some(frame.tree) };
                }
            }
            Tag::VARDEF => {
                let var_sym = match &ctx.tree(frame.tree).kind {
                    TreeKind::VariableDecl { sym, .. } => *sym,
                    _ => None,
                };
                if (var_sym == Some(sym) && ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind) == Some(Kind::MTH))
                    || frame.contains(sym)
                {
                    return if current_depth > depth { None } else { Some(frame.tree) };
                }
            }
            Tag::BLOCK | Tag::METHODDEF | Tag::LAMBDA => {
                if frame.contains(sym) {
                    return if current_depth > depth { None } else { Some(frame.tree) };
                }
            }
            _ => {}
        }
        current_depth -= 1;
    }
    None
}

impl LambdaAnalyzerPreprocessor {

    fn interface_parameter_is_intersection_or_union_type(&self, ctx: &mut CompilerContext, tree: TreeId) -> bool {
        let target = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { target: Some(target), .. } => *target,
            _ => return false,
        };
        let Some(descriptor) = self.find_descriptor_type_checked(ctx, target) else {
            return false;
        };
        let params = ctx.typ(descriptor).get_parameter_types(ctx);
        // Port note: the upstream loop returns from its first iteration (only the
        // first type argument is inspected).
        if params.non_empty() {
            return self.is_intersection_or_union_type(ctx, *params.head());
        }
        false
    }
    fn is_intersection_or_union_type(&self, ctx: &CompilerContext, ty: TypeId) -> bool {
        match ctx.typ(ty).get_tag() {
            TypeTag::CLASS => {
                return ctx.typ(ty).is_intersection() || ctx.typ(ty).is_union();
            }
            TypeTag::TYPEVAR => {
                let upper = ctx.typ(ty).get_upper_bound();
                upper.map(|upper| self.is_intersection_or_union_type(ctx, upper)).unwrap_or(false)
            }
            _ => false,
        }
    }
}

impl LambdaToMethod {
    fn translate_ident_in_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> Option<TreeId> {
        let context_tree = self.context?;
        self.analyzer.translate_lambda_ident(ctx, context_tree, tree)
    }
    fn translate_select_in_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> Option<TreeId> {
        let context_tree = self.context?;
        self.analyzer.translate_lambda_select(ctx, context_tree, tree)
    }
    fn translate_new_class_in_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let context_tree = self.context.expect("current context");
        self.analyzer.translate_lambda_new_class(ctx, context_tree, tree)
    }
    /// Supplier<List<Attribute.TypeCompound>> source, Consumer owner,
    fn apportion_type_annotations_lambda(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        owner: SymbolId,
        lambda_sym: SymbolId,
    ) {
        let source = ctx.sym(owner).get_raw_attributes();
        self.apportion_type_annotations(ctx, tree, &source, owner, TypeAttrKind::Raw, false, lambda_sym, false);
    }
    fn apportion_type_annotations_owner(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        owner: SymbolId,
        init: bool,
        lambda_sym: SymbolId,
    ) {
        let source = if init {
            ctx.sym(owner).get_init_type_attributes()
        } else {
            ctx.sym(owner).get_class_init_type_attributes()
        };
        let kind = if init { TypeAttrKind::Init } else { TypeAttrKind::ClassInit };
        self.apportion_type_annotations(ctx, tree, &source, owner, kind, true, lambda_sym, true);
    }
    #[allow(clippy::too_many_arguments)]
    fn apportion_type_annotations(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        source: &List<crate::ids::AttributeId>,
        owner: SymbolId,
        owner_kind: TypeAttrKind,
        owner_append_unique: bool,
        lambda_sym: SymbolId,
        lambda_append_unique: bool,
    ) {
        let mut owner_attrs: List<crate::ids::AttributeId> = List::nil();
        let mut lambda_attrs: List<crate::ids::AttributeId> = List::nil();
        for attribute in source.iter() {
            let on_lambda = ctx
                .attr(attribute)
                .type_compound_on_lambda();
            if on_lambda == Some(tree) {
                lambda_attrs = lambda_attrs.append(attribute);
            } else {
                owner_attrs = owner_attrs.append(attribute);
            }
        }
        if lambda_attrs.is_empty() {
            return;
        }
        match owner_kind {
            TypeAttrKind::Raw => {
                if owner_append_unique {
                    ctx.sym_mut(owner).append_unique_type_attributes(owner_attrs);
                } else {
                    ctx.sym_mut(owner).set_type_attributes(owner_attrs);
                }
            }
            TypeAttrKind::Init => {
                if owner_append_unique {
                    let existing = ctx.sym(owner).get_init_type_attributes();
                    ctx.sym_mut(owner).set_init_type_attributes(existing.append_list(&owner_attrs));
                } else {
                    ctx.sym_mut(owner).set_init_type_attributes(owner_attrs);
                }
            }
            TypeAttrKind::ClassInit => {
                if owner_append_unique {
                    let existing = ctx.sym(owner).get_class_init_type_attributes();
                    ctx.sym_mut(owner).set_class_init_type_attributes(existing.append_list(&owner_attrs));
                } else {
                    ctx.sym_mut(owner).set_class_init_type_attributes(owner_attrs);
                }
            }
        }
        if lambda_append_unique {
            ctx.sym_mut(lambda_sym).append_unique_type_attributes(lambda_attrs);
        } else {
            ctx.sym_mut(lambda_sym).set_type_attributes(lambda_attrs);
        }
    }
    fn map_prev_context(&self, _ctx: &mut CompilerContext) -> Option<TreeId> {
        self.context
            .and_then(|context| self.context_map.borrow().get(&context).map(|context| context.prev_context))
            .flatten()
    }
}
#[derive(Clone, Copy)]
enum TypeAttrKind {
    Raw,
    Init,
    ClassInit,
}

impl LambdaToMethod {
    fn make_metafactory_indy_call(
        &mut self,
        ctx: &mut CompilerContext,
        context_tree: TreeId,
        ref_sym: SymbolId,
        indy_args: List<TreeId>,
    ) -> TreeId {
        let (target, tree, bridges, is_serializable, is_reference) = {
            let context_map = self.context_map.borrow();
            let context = context_map.get(&context_tree).expect("translation context");
            match &context.kind {
                TranslationContextKind::Lambda(lambda) => (
                    lambda.target,
                    context.tree,
                    context.bridges.clone(),
                    self.context_is_serializable(ctx, context_tree),
                    false,
                ),
                TranslationContextKind::Reference(reference) => (
                    reference.target,
                    context.tree,
                    context.bridges.clone(),
                    self.context_is_serializable(ctx, context_tree),
                    true,
                ),
            }
        };
        let target_tsym = ctx.typ(target).tsym.expect("target type symbol");
        let sam_sym = self
            .find_descriptor_symbol_checked(ctx, target_tsym)
            .expect("sam descriptor symbol");
        let sam_type = ctx.sym(sam_sym).ty.expect("sam type");
        let ref_type = ctx.sym(ref_sym).ty.expect("reference type");
        let target_type = ctx.tree(tree).ty.expect("functional expression type");
        let descriptor = self
            .find_descriptor_type_checked(ctx, target)
            .expect("descriptor type");
        let mut static_args: Vec<LoadableConstantRef> = Vec::new();
        static_args.push(LoadableConstantRef::MethodType(self.type_to_method_type(ctx, sam_type)));
        static_args.push(LoadableConstantRef::MethodHandle(ref_sym));
        static_args.push(LoadableConstantRef::MethodType(self.type_to_method_type(ctx, descriptor)));
        let _ = ref_type;
        let mut indy_arg_types: List<TypeId> = List::nil();
        for arg in indy_args.iter() {
            if let Some(ty) = ctx.tree(arg).ty {
                indy_arg_types = indy_arg_types.append(ty);
            }
        }
        let method_class = self.syms.borrow().method_class;
        let indy_type = ctx.alloc_type(Type::new_method_type(indy_arg_types, target_type, List::nil(), method_class));
        let needs_alt = target_tsym_is_intersection(ctx, target)
            || is_serializable
            || bridges.len() > 1;
        let metafactory_name = if needs_alt {
            self.names.altMetafactory.clone()
        } else {
            self.names.metafactory.clone()
        };
        if needs_alt {
            let mut markers: Vec<TypeId> = Vec::new();
            let targets = if ctx.typ(target).is_intersection() {
                let syms = self.syms.borrow();
                self.types.direct_supertypes(ctx, &syms, target)
            } else {
                List::nil()
            };
            let serializable_type = self.syms.borrow().serializable_type;
            let serializable_tsym = ctx.typ(serializable_type).tsym;
            let target_tsym2 = ctx.typ(target).tsym;
            let object_tsym = ctx.typ(self.syms.borrow().object_type).tsym;
            for marker in targets.iter() {
                let syms = self.syms.borrow();
                let erased = self.types.erasure(ctx, &syms, marker);
                drop(syms);
                let marker_tsym = ctx.typ(erased).tsym;
                if marker_tsym != serializable_tsym && marker_tsym != target_tsym2 && marker_tsym != object_tsym {
                    markers.push(erased);
                }
            }
            let mut flags_value = if is_serializable { FLAG_SERIALIZABLE } else { 0 };
            let has_markers = !markers.is_empty();
            let has_bridges = !bridges.is_empty();
            if has_markers {
                flags_value |= FLAG_MARKERS;
            }
            if has_bridges {
                flags_value |= FLAG_BRIDGES;
            }
            static_args.push(LoadableConstantRef::Basic(BasicConstant::int(flags_value)));
            if has_markers {
                static_args.push(LoadableConstantRef::Basic(BasicConstant::int(markers.len() as i32)));
                for marker in &markers {
                    static_args.push(LoadableConstantRef::Class(*marker));
                }
            }
            if has_bridges {
                static_args.push(LoadableConstantRef::Basic(BasicConstant::int(bridges.len() as i32 - 1)));
                let sam_erasure = {
                    let syms = self.syms.borrow();
                    self.types.erasure(ctx, &syms, sam_type)
                };
                for bridge in &bridges {
                    let bridge_ty = ctx.sym(*bridge).ty.expect("bridge type");
                    let bridge_erasure = {
                        let syms = self.syms.borrow();
                        self.types.erasure(ctx, &syms, bridge_ty)
                    };
                    let syms = self.syms.borrow();
                    let same = self.types.is_same_type(ctx, &syms, bridge_erasure, sam_erasure);
                    drop(syms);
                    if !same {
                        static_args.push(LoadableConstantRef::MethodType(self.type_to_method_type(ctx, bridge_erasure)));
                    }
                }
            }
            if is_serializable {
                let previous_pos = self.make.borrow().pos;
                let k_info_class = self.k_info.as_ref().map(|info| info.clazz);
                if let Some(clazz) = k_info_class {
                    {
                        let mut make = self.make.borrow_mut();
                        make.at(ctx.tree(clazz).pos);
                    }
                    self.add_deserialization_case(ctx, ref_sym, target_type, sam_sym, tree, &static_args, indy_type);
                    {
                        let mut make = self.make.borrow_mut();
                        make.at(previous_pos);
                    }
                }
            }
        }
        let _ = is_reference;
        let sam_name = ctx.sym(sam_sym).name.clone();
        let lambda_metafactory = self.syms.borrow().lambda_metafactory;
        self.make_indy_call(ctx, tree, lambda_metafactory, metafactory_name, static_args, indy_type, indy_args, sam_name)
    }
    /// Name bsmName, List<LoadableConstant> staticArgs, MethodType indyType,
    /// Generate an indy method call with given name, type and static bootstrap
    /// arguments types
    fn make_indy_call(
        &mut self,
        ctx: &mut CompilerContext,
        pos_tree: TreeId,
        site: TypeId,
        bsm_name: Name,
        static_args: Vec<LoadableConstantRef>,
        indy_type: TypeId,
        indy_args: List<TreeId>,
        meth_name: Name,
    ) -> TreeId {
        let previous_pos = self.make.borrow().pos;
        let pos = ctx.tree(pos_tree).pos;
        {
            let mut make = self.make.borrow_mut();
            make.at(pos);
        }
        let bsm_static_args = {
            let syms = self.syms.borrow();
            let mut args: List<TypeId> = List::of(syms.method_handle_lookup_type)
                .append(syms.string_type)
                .append(syms.method_type_type);
            drop(syms);
            for argument in &static_args {
                let ty = self.types.constant_type(ctx, &self.syms.borrow(), argument);
                args = args.append(ty);
            }
            args
        };
        let attr_env = self.attr_env.clone().expect("attrEnv");
        let site_tsym = ctx.typ(site).tsym.expect("bootstrap site symbol");
        let attr_rc = Rc::clone(&self.attr);
        let (chk, infer) = {
            let attr = attr_rc.borrow();
            (Rc::clone(&attr.chk), Rc::clone(&attr.infer))
        };
        let chk_borrow = chk;
        let bsm = {
            let mut attr = attr_rc.borrow_mut();
            let mut syms = self.syms.borrow_mut();
            self.rs.borrow_mut().resolve_internal_method(
                ctx,
                &mut syms,
                &self.types,
                &mut attr,
                &chk_borrow,
                &infer,
                None,
                &attr_env,
                site,
                bsm_name.clone(),
                bsm_static_args,
                None,
            )
        };
        let _ = site_tsym;
        let bsm_handle = symbol::as_method_handle(ctx, bsm, false);
        let dyn_sym = symbol::new_dynamic_method_symbol(
            ctx,
            meth_name,
            self.syms.borrow().no_symbol,
            bsm_handle,
            indy_type,
            static_args.clone(),
        );
        let key = (bsm, static_args.clone());
        let existing = self
            .k_info
            .as_ref()
            .and_then(|info| info.dyn_meth_syms.iter().find(|(candidate, _)| *candidate == key).map(|(_, sym)| *sym));
        let sym_to_use = if let Some(existing) = existing {
            existing
        } else {
            if let Some(info) = self.k_info.as_mut() {
                info.dyn_meth_syms.push((key, dyn_sym));
            }
            dyn_sym
        };
        let qualifier = {
            let mut make = self.make.borrow_mut();
            make.at(pos);
            let site_tree = {
                let mut syms = self.syms.borrow_mut();
                let site_sym = ctx.typ(site).tsym.expect("site symbol");
                make.qual_ident(ctx, &mut syms, site_sym)
            };
            let select = make.select(ctx, site_tree, bsm_name.clone());
            select
        };
        if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(qualifier).kind {
            *slot = Some(sym_to_use);
        }
        let return_type = ctx.typ(indy_type).get_return_type(ctx).expect("indy return type");
        ctx.tree_mut(qualifier).ty = Some(return_type);
        let proxy_call = {
            let mut make = self.make.borrow_mut();
            make.apply(ctx, List::nil(), qualifier, indy_args)
        };
        ctx.tree_mut(proxy_call).ty = Some(return_type);
        {
            let mut make = self.make.borrow_mut();
            make.at(previous_pos);
        }
        proxy_call
    }
}

/// Whether the functional target is an intersection type
/// (LambdaToMethod.makeMetafactoryIndyCall).
fn target_tsym_is_intersection(ctx: &CompilerContext, target: TypeId) -> bool {
    ctx.typ(target).is_intersection()
}

impl LambdaToMethod {
    fn make_deserialize_method(&mut self, ctx: &mut CompilerContext, class_sym: SymbolId) -> TreeId {
        let (deser_method_sym, deser_param_sym) = self
            .k_info
            .as_ref()
            .map(|info| (info.deser_method_sym, info.deser_param_sym))
            .expect("klass info");
        let cases = self
            .k_info
            .as_ref()
            .map(|info| info.deserialize_cases.clone())
            .unwrap_or_default();
        let mut case_trees: Vec<TreeId> = Vec::new();
        let mut breaks: Vec<TreeId> = Vec::new();
        for (key, statements) in cases.iter() {
            let br = {
                let clazz = self.k_info.as_ref().map(|info| info.clazz);
                let pos = clazz.map(|clazz| ctx.tree(clazz).pos).unwrap_or(0);
                let mut make = self.make.borrow_mut();
                make.at(pos);
                make.break_(ctx, None)
            };
            breaks.push(br);
            let stats = List::from_iterable(statements.iter().copied().chain(std::iter::once(br)));
            let label = {
                let mut make = self.make.borrow_mut();
                make.literal(ctx, TypeTag::CLASS, Some(LiteralValue::String(javac_strings::JavaString::from(key.as_str()))))
            };
            {
                let label_const = crate::util::constants::ConstantValue::String(
                    javac_strings::JavaString::from(key.as_str()),
                );
                let const_ty = {
                    let string_type = self.syms.borrow().string_type;
                    ctx.typ(string_type).const_type(label_const)
                };
                ctx.tree_mut(label).ty = Some(ctx.alloc_type(const_ty));
            }
            let case = {
                let mut make = self.make.borrow_mut();
                make.case(
                    ctx,
                    javac_source::tree::case_tree::CaseKind::STATEMENT,
                    List::of(label),
                    stats,
                    None,
                )
            };
            case_trees.push(case);
        }
        let string_type = self.syms.borrow().string_type;
        let selector = self.deser_getter(ctx, "getImplMethodName", string_type);
        let switch = {
            let mut make = self.make.borrow_mut();
            make.switch(ctx, selector, List::from_iterable(case_trees))
        };
        for br in &breaks {
            if let TreeKind::Break { target: slot, .. } = &mut ctx.tree_mut(*br).kind {
                *slot = Some(switch);
            }
        }
        let illegal_argument_exception_type = self.syms.borrow().illegal_argument_exception_type;
        let invalid_message = {
            let mut make = self.make.borrow_mut();
            make.literal(ctx, TypeTag::CLASS, Some(LiteralValue::String(javac_strings::JavaString::from("Invalid lambda deserialization"))))
        };
        ctx.tree_mut(invalid_message).ty = Some(self.syms.borrow().string_type);
        let throw = {
            let new_class = self.make_new_class(
                ctx,
                illegal_argument_exception_type,
                List::of(invalid_message),
                None,
            );
            let mut make = self.make.borrow_mut();
            make.throw_(ctx, new_class)
        };
        let body = {
            let mut make = self.make.borrow_mut();
            make.block_pub(ctx, 0, List::from_iterable(vec![switch, throw]))
        };
        let mods = {
            let mut make = self.make.borrow_mut();
            make.modifiers(ctx, ctx.sym(deser_method_sym).flags_field)
        };
        let restype = ctx
            .sym(deser_method_sym)
            .ty
            .and_then(|ty| ctx.typ(ty).get_return_type(ctx))
            .expect("deserialize method return type");
        let restype_tsym = ctx.typ(restype).tsym.expect("deserialize return type symbol");
        let qual_ident = {
            let mut make = self.make.borrow_mut();
            make.qual_ident(ctx, &mut self.syms.borrow_mut(), restype_tsym)
        };
        let param_def = {
            let mut make = self.make.borrow_mut();
            make.var_def_symbol(ctx, &mut self.syms.borrow_mut(), deser_param_sym, None)
        };
        let deser = {
            let mut make = self.make.borrow_mut();
            make.method_def(
                ctx,
                mods,
                self.names.deserializeLambda.clone(),
                Some(qual_ident),
                List::nil(),
                None,
                List::of(param_def),
                List::nil(),
                Some(body),
                None,
            )
        };
        if let TreeKind::MethodDecl { sym: slot, .. } = &mut ctx.tree_mut(deser).kind {
            *slot = Some(deser_method_sym);
        }
        ctx.tree_mut(deser).ty = ctx.sym(deser_method_sym).ty;
        let _ = class_sym;
        deser
    }
    /// Make an attributed class instance creation expression.
    /// @param ctype    The class type.
    /// @param args     The constructor arguments.
    /// @param cons     The constructor symbol
    fn make_new_class(
        &mut self,
        ctx: &mut CompilerContext,
        ctype: TypeId,
        args: List<TreeId>,
        cons: Option<SymbolId>,
    ) -> TreeId {
        let clazz = {
            let mut make = self.make.borrow_mut();
            let ctype_tsym = ctx.typ(ctype).tsym.expect("new class type symbol");
            make.qual_ident(ctx, &mut self.syms.borrow_mut(), ctype_tsym)
        };
        let tree = {
            let mut make = self.make.borrow_mut();
            make.new_class(ctx, None, List::nil(), clazz, args.clone(), None)
        };
        let constructor = match cons {
            Some(cons) => Some(cons),
            None => {
                let arg_types: List<TypeId> = {
                    let mut types_list: List<TypeId> = List::nil();
                    for arg in args.iter() {
                        if let Some(ty) = ctx.tree(arg).ty {
                            types_list = types_list.append(ty);
                        }
                    }
                    types_list
                };
                let attr_env = self.attr_env.clone().expect("attrEnv");
                let attr_rc = Rc::clone(&self.attr);
                let mut attr = attr_rc.borrow_mut();
                let chk = Rc::clone(&attr.chk);
                let chk = chk;
                let infer = Rc::clone(&attr.infer);
                Some(self.rs.borrow_mut().resolve_constructor(
                    ctx,
                    &mut self.syms.borrow_mut(),
                    &self.types,
                    &mut attr,
                    &chk,
                    &infer,
                    None,
                    &attr_env,
                    ctype,
                    arg_types,
                    Some(List::nil()),
                ))
            }
        };
        if let TreeKind::NewClass { constructor: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = constructor;
        }
        ctx.tree_mut(tree).ty = Some(ctype);
        tree
    }
    /// Type targetType, MethodSymbol samSym, DiagnosticPosition pos,
    /// List<LoadableConstant> staticArgs, MethodType indyType)
    fn add_deserialization_case(
        &mut self,
        ctx: &mut CompilerContext,
        ref_sym: SymbolId,
        target_type: TypeId,
        sam_sym: SymbolId,
        pos_tree: TreeId,
        static_args: &[LoadableConstantRef],
        indy_type: TypeId,
    ) {
        let functional_interface_class = self.class_sig_of_type(ctx, target_type);
        let functional_interface_method_name = ctx.sym(sam_sym).name.to_java_string().to_string_lossy().to_string();
        let sam_ty = ctx.sym(sam_sym).ty.expect("sam type");
        let sam_erasure = {
            let syms = self.syms.borrow();
            self.types.erasure(ctx, &syms, sam_ty)
        };
        let functional_interface_method_signature = self.type_sig(ctx, sam_erasure, false);
        let impl_owner = ctx.sym(ref_sym).owner.expect("impl owner");
        let impl_owner_ty = ctx.sym(impl_owner).ty.expect("impl owner type");
        let impl_class = self.class_sig_of_type(ctx, impl_owner_ty);
        let impl_method_name = ctx.sym(ref_sym).name.to_java_string().to_string_lossy().to_string();
        let ref_ty = ctx.sym(ref_sym).ty.expect("ref type");
        let impl_method_signature = self.type_sig(ctx, ref_ty, false);
        let int_type = self.syms.borrow().int_type;
        let method_kind_getter = self.deser_getter(ctx, "getImplMethodKind", int_type);
        let kind_literal = {
            let mut make = self.make.borrow_mut();
            make.literal(ctx, TypeTag::INT, Some(LiteralValue::Integer(reference_kind_of(ctx, pos_tree))))
        };
        ctx.tree_mut(kind_literal).ty = Some(int_type);
        let kind_test = self.eq_test(ctx, int_type, method_kind_getter, kind_literal);
        let mut ser_args: Vec<TreeId> = Vec::new();
        let param_types = ctx.typ(indy_type).get_parameter_types(ctx);
        let mut index = 0;
        for param in param_types.iter() {
            let index_arg = {
                let mut make = self.make.borrow_mut();
                make.literal(ctx, TypeTag::INT, Some(LiteralValue::Integer(index)))
            };
            let int_type = self.syms.borrow().int_type;
            ctx.tree_mut(index_arg).ty = Some(int_type);
            let object_type = self.syms.borrow().object_type;
            let arg_types = List::of(int_type);
            let getter = self.deser_getter_with_args(
                ctx,
                "getCapturedArg",
                object_type,
                arg_types,
                List::of(index_arg),
            );
            let erased_param = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, param)
            };
            let type_expr = {
                let mut make = self.make.borrow_mut();
                make.type_from_type(ctx, &mut self.syms.borrow_mut(), erased_param)
            };
            let cast = {
                let mut make = self.make.borrow_mut();
                make.type_cast(ctx, type_expr, getter)
            };
            ser_args.push(cast);
            index += 1;
        }
        let lambda_metafactory = self.syms.borrow().lambda_metafactory;
        let sam_name = ctx.sym(sam_sym).name.clone();
        let indy_call = self.make_indy_call(
            ctx,
            pos_tree,
            lambda_metafactory,
            self.names.altMetafactory.clone(),
            static_args.to_vec(),
            indy_type,
            List::from_iterable(ser_args),
            sam_name,
        );
        let ret = {
            let mut make = self.make.borrow_mut();
            make.return_(ctx, Some(indy_call))
        };
        let mut test = kind_test;
        test = self.deser_test(ctx, test, "getFunctionalInterfaceClass", &functional_interface_class);
        test = self.deser_test(ctx, test, "getFunctionalInterfaceMethodName", &functional_interface_method_name);
        test = self.deser_test(ctx, test, "getFunctionalInterfaceMethodSignature", &functional_interface_method_signature);
        test = self.deser_test(ctx, test, "getImplClass", &impl_class);
        test = self.deser_test(ctx, test, "getImplMethodSignature", &impl_method_signature);
        let stmt = {
            let mut make = self.make.borrow_mut();
            make.if_(ctx, test, ret, None)
        };
        if let Some(info) = self.k_info.as_mut() {
            if let Some(entry) = info
                .deserialize_cases
                .iter_mut()
                .find(|(name, _)| *name == impl_method_name)
            {
                entry.1.push(stmt);
            } else {
                info.deserialize_cases.push((impl_method_name, vec![stmt]));
            }
        }
        let _ = pos_tree;
        let _ = static_args;
    }
    fn eq_test(&mut self, ctx: &mut CompilerContext, arg_type: TypeId, arg1: TreeId, arg2: TreeId) -> TreeId {
        let test_expr = {
            let mut make = self.make.borrow_mut();
            make.binary(ctx, Tag::EQ, arg1, arg2)
        };
        let operator = {
            let mut syms = self.syms.borrow_mut();
            self.operators
                .resolve_binary(ctx, &mut syms, &self.types, ctx.tree(test_expr).pos, Tag::EQ, arg_type, arg_type)
        };
        if let TreeKind::Binary { operator: slot, .. } = &mut ctx.tree_mut(test_expr).kind {
            *slot = Some(operator);
        }
        ctx.tree_mut(test_expr).ty = Some(self.syms.borrow().boolean_type);
        test_expr
    }
    fn deser_test(&mut self, ctx: &mut CompilerContext, prev: TreeId, func: &str, lit: &str) -> TreeId {
        let eqmt = {
            let syms = self.syms.borrow();
            ctx.alloc_type(Type::new_method_type(
                List::of(syms.object_type),
                syms.boolean_type,
                List::nil(),
                syms.method_class,
            ))
        };
        let object_type = self.syms.borrow().object_type;
        let attr_env = self.attr_env.clone().expect("attrEnv");
        let location = ctx.typ(object_type).tsym.expect("equals site symbol");
        let attr_rc = Rc::clone(&self.attr);
        let mut attr = attr_rc.borrow_mut();
        let chk = Rc::clone(&attr.chk);
        let chk = chk;
        let infer = Rc::clone(&attr.infer);
        let equals_sym = self.rs.borrow_mut().resolve_qualified_method(
            ctx,
            &mut self.syms.borrow_mut(),
            &self.types,
            &mut attr,
            &chk,
            &infer,
            None,
            &attr_env,
            location,
            object_type,
            self.names.equals.clone(),
            List::of(object_type),
            Some(List::nil()),
        );
        // Release the Attr RefCell borrow before deser_getter borrows it again.
        drop(attr);
        let string_type = self.syms.borrow().string_type;
        let getter = self.deser_getter(ctx, func, string_type);
        let literal = {
            let mut make = self.make.borrow_mut();
            make.literal(ctx, TypeTag::CLASS, Some(LiteralValue::String(javac_strings::JavaString::from(lit))))
        };
        ctx.tree_mut(literal).ty = Some(string_type);
        let select = {
            let mut make = self.make.borrow_mut();
            make.select(ctx, getter, ctx.sym(equals_sym).name.clone())
        };
        if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(select).kind {
            *slot = Some(equals_sym);
        }
        ctx.tree_mut(select).ty = Some(eqmt);
        let eqtest = {
            let mut make = self.make.borrow_mut();
            make.apply(ctx, List::nil(), select, List::of(literal))
        };
        ctx.tree_mut(eqtest).ty = Some(self.syms.borrow().boolean_type);
        let compound = {
            let mut make = self.make.borrow_mut();
            make.binary(ctx, Tag::AND, prev, eqtest)
        };
        let boolean_type = self.syms.borrow().boolean_type;
        let operator = {
            let mut syms = self.syms.borrow_mut();
            self.operators
                .resolve_binary(ctx, &mut syms, &self.types, ctx.tree(compound).pos, Tag::AND, boolean_type, boolean_type)
        };
        if let TreeKind::Binary { operator: slot, .. } = &mut ctx.tree_mut(compound).kind {
            *slot = Some(operator);
        }
        ctx.tree_mut(compound).ty = Some(boolean_type);
        compound
    }
    fn deser_getter(&mut self, ctx: &mut CompilerContext, func: &str, ty: TypeId) -> TreeId {
        self.deser_getter_with_args(ctx, func, ty, List::nil(), List::nil())
    }
    fn deser_getter_with_args(
        &mut self,
        ctx: &mut CompilerContext,
        func: &str,
        ty: TypeId,
        arg_types: List<TypeId>,
        args: List<TreeId>,
    ) -> TreeId {
        let method_class = self.syms.borrow().method_class;
        let getmt = ctx.alloc_type(Type::new_method_type(arg_types.clone(), ty, List::nil(), method_class));
        let serialized_lambda_type = self.syms.borrow().serialized_lambda_type;
        let name = self.names.from_string(&javac_strings::JavaString::from(func));
        let getsym = {
            let attr_env = self.attr_env.clone().expect("attrEnv");
            let location = ctx.typ(serialized_lambda_type).tsym.expect("serialized lambda site");
            let attr_rc = Rc::clone(&self.attr);
            let mut attr = attr_rc.borrow_mut();
            let chk = Rc::clone(&attr.chk);
            let chk = chk;
            let infer = Rc::clone(&attr.infer);
            self.rs.borrow_mut().resolve_qualified_method(
                ctx,
                &mut self.syms.borrow_mut(),
                &self.types,
                &mut attr,
                &chk,
                &infer,
                None,
                &attr_env,
                location,
                serialized_lambda_type,
                name.clone(),
                arg_types.clone(),
                Some(List::nil()),
            )
        };
        let deser_param_sym = self
            .k_info
            .as_ref()
            .map(|info| info.deser_param_sym)
            .expect("klass info");
        let ident = {
            let mut make = self.make.borrow_mut();
            make.ident_symbol(ctx, deser_param_sym)
        };
        ctx.tree_mut(ident).ty = Some(serialized_lambda_type);
        let select = {
            let mut make = self.make.borrow_mut();
            make.select(ctx, ident, name)
        };
        if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(select).kind {
            *slot = Some(getsym);
        }
        ctx.tree_mut(select).ty = Some(getmt);
        let apply = {
            let mut make = self.make.borrow_mut();
            make.apply(ctx, List::nil(), select, args)
        };
        ctx.tree_mut(apply).ty = Some(ty);
        apply
    }
    fn class_sig_of_type(&self, ctx: &mut CompilerContext, ty: TypeId) -> String {
        self.class_sig(ctx, ty)
    }
}

fn reference_kind_of(ctx: &CompilerContext, tree: TreeId) -> i32 {
    let kind = match &ctx.tree(tree).kind {
        TreeKind::MemberReference { kind, mode, .. } => (*kind, *mode),
        _ => (None, crate::tree::jc_tree::ReferenceMode::INVOKE),
    };
    let kind = kind.0;
    match kind {
        Some(crate::tree::jc_tree::ReferenceKind::SUPER) => 6,
        Some(crate::tree::jc_tree::ReferenceKind::IMPLICIT_INNER) => 7,
        Some(crate::tree::jc_tree::ReferenceKind::BOUND) => 5,
        Some(crate::tree::jc_tree::ReferenceKind::UNBOUND) => 9,
        Some(crate::tree::jc_tree::ReferenceKind::STATIC) => 6,
        Some(crate::tree::jc_tree::ReferenceKind::TOPLEVEL) => 5,
        Some(crate::tree::jc_tree::ReferenceKind::ARRAY_CTOR) => 8,
        _ => 5,
    }
}


#[allow(dead_code)]
// Port adapter: unused default-navigation entries are kept for 1:1 traceability.
impl LambdaToMethod {
    /// Adapter: the TreeTranslator default navigation for this node kind, used
    /// where the upstream visitor calls super (registered adapter for super).
    fn default_visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::CompilationUnit { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = defs;
        }
        self.result = Some(tree);
    }


    fn default_visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        let annotations = self.translate_list(ctx, &annotations);
        let pid = self.translate(ctx, pid);
        if let TreeKind::PackageDecl {
            annotations: a,
            pid: p,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *p = pid;
        }
        self.result = Some(tree);
    }


    fn default_visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Import { qualid: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn default_visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, typarams, extending, implementing, defs) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl {
                mods,
                typarams,
                extending,
                implementing,
                defs,
                ..
            } => (
                *mods,
                typarams.clone(),
                *extending,
                implementing.clone(),
                defs.clone(),
            ),
            _ => panic!("expected a class declaration"),
        };
        let mods = self.translate(ctx, mods);
        let typarams = self.translate_type_params(ctx, &typarams);
        let extending = self.translate(ctx, extending);
        let implementing = self.translate_list(ctx, &implementing);
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::ClassDecl {
            mods: m,
            typarams: t,
            extending: e,
            implementing: i,
            defs: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *t = typarams;
            *e = extending;
            *i = implementing;
            *d = defs;
        }
        self.result = Some(tree);
    }


    fn default_visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, body) = match &ctx.tree(tree).kind
        {
            TreeKind::MethodDecl {
                mods,
                restype,
                typarams,
                recvparam,
                params,
                thrown,
                body,
                ..
            } => (
                *mods,
                *restype,
                typarams.clone(),
                *recvparam,
                params.clone(),
                thrown.clone(),
                *body,
            ),
            _ => panic!("expected a method declaration"),
        };
        let mods = self.translate(ctx, mods);
        let restype = self.translate(ctx, restype);
        let typarams = self.translate_type_params(ctx, &typarams);
        let recvparam = self.translate(ctx, recvparam);
        let params = self.translate_var_defs(ctx, &params);
        let thrown = self.translate_list(ctx, &thrown);
        let body = self.translate(ctx, body);
        if let TreeKind::MethodDecl {
            mods: m,
            restype: r,
            typarams: tp,
            recvparam: rp,
            params: ps,
            thrown: th,
            body: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *r = restype;
            *tp = typarams;
            *rp = recvparam;
            *ps = params;
            *th = thrown;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, nameexpr, vartype, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                nameexpr,
                vartype,
                init,
                ..
            } => (*mods, *nameexpr, *vartype, *init),
            _ => panic!("expected a variable declaration"),
        };
        let mods = self.translate(ctx, mods);
        let nameexpr = self.translate(ctx, nameexpr);
        let vartype = self.translate(ctx, vartype);
        let init = self.translate(ctx, init);
        if let TreeKind::VariableDecl {
            mods: m,
            nameexpr: ne,
            vartype: vt,
            init: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *ne = nameexpr;
            *vt = vartype;
            *i = init;
        }
        self.result = Some(tree);
    }


    fn default_visit_skip(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = stats;
        }
        self.result = Some(tree);
    }


    fn default_visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let body = self.translate(ctx, body);
        let cond = self.translate(ctx, cond);
        if let TreeKind::DoWhileLoop { body: b, cond: c } = &mut ctx.tree_mut(tree).kind {
            *b = body;
            *c = cond;
        }
        self.result = Some(tree);
    }


    fn default_visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let cond = self.translate(ctx, cond);
        let body = self.translate(ctx, body);
        if let TreeKind::WhileLoop { cond: c, body: b } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            } => (init.clone(), *cond, step.clone(), *body),
            _ => panic!("expected a for loop"),
        };
        let init = self.translate_list(ctx, &init);
        let cond = self.translate(ctx, cond);
        let step = self.translate_list(ctx, &step);
        let body = self.translate(ctx, body);
        if let TreeKind::ForLoop {
            init: i,
            cond: c,
            step: s,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = init;
            *c = cond;
            *s = step;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let var = self.translate(ctx, var);
        let expr = self.translate(ctx, expr);
        let body = self.translate(ctx, body);
        if let TreeKind::EnhancedForLoop {
            var: v,
            expr: e,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *v = var;
            *e = expr;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let body = self.translate(ctx, body);
        if let TreeKind::LabeledStatement { body: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::Switch {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn default_visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        let labels = self.translate_list(ctx, &labels);
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Case {
            labels: l,
            stats: s,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *l = labels;
            *s = stats;
        }
        self.result = Some(tree);
    }


    fn default_visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::SwitchExpression {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn default_visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        let lock = self.translate(ctx, lock);
        let body = self.translate(ctx, body);
        if let TreeKind::Synchronized { lock: l, body: b } = &mut ctx.tree_mut(tree).kind {
            *l = lock;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (resources, body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                resources,
                body,
                catchers,
                finalizer,
                ..
            } => (resources.clone(), *body, catchers.clone(), *finalizer),
            _ => panic!("expected a try statement"),
        };
        let resources = self.translate_list(ctx, &resources);
        let body = self.translate(ctx, body);
        let catchers = self.translate_catchers(ctx, &catchers);
        let finalizer = self.translate(ctx, finalizer);
        if let TreeKind::Try {
            resources: r,
            body: b,
            catchers: c,
            finalizer: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *r = resources;
            *b = body;
            *c = catchers;
            *f = finalizer;
        }
        self.result = Some(tree);
    }


    fn default_visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        let param = self.translate(ctx, param);
        let body = self.translate(ctx, body);
        if let TreeKind::Catch { param: p, body: b } = &mut ctx.tree_mut(tree).kind {
            *p = param;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (*cond, *truepart, *falsepart),
            _ => panic!("expected a conditional"),
        };
        let cond = self.translate(ctx, cond);
        let truepart = self.translate(ctx, truepart);
        let falsepart = self.translate(ctx, falsepart);
        if let TreeKind::Conditional {
            cond: c,
            truepart: t,
            falsepart: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = truepart;
            *f = falsepart;
        }
        self.result = Some(tree);
    }


    fn default_visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            } => (*cond, *thenpart, *elsepart),
            _ => panic!("expected an if statement"),
        };
        let cond = self.translate(ctx, cond);
        let thenpart = self.translate(ctx, thenpart);
        let elsepart = self.translate(ctx, elsepart);
        if let TreeKind::If {
            cond: c,
            thenpart: t,
            elsepart: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = thenpart;
            *e = elsepart;
        }
        self.result = Some(tree);
    }


    fn default_visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::ExpressionStatement { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn default_visit_break(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        let value = self.translate(ctx, value);
        if let TreeKind::Yield { value: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = value;
        }
        self.result = Some(tree);
    }


    fn default_visit_continue(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Return { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn default_visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Throw { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn default_visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        let cond = self.translate(ctx, cond);
        let detail = self.translate(ctx, detail);
        if let TreeKind::Assert { cond: c, detail: d } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *d = detail;
        }
        self.result = Some(tree);
    }


    fn default_visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        let meth = self.translate(ctx, meth);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::MethodInvocation {
            meth: m, args: a, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = meth;
            *a = args;
        }
        self.result = Some(tree);
    }


    fn default_visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                clazz,
                args,
                def,
                ..
            } => (*encl, *clazz, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        let encl = self.translate(ctx, encl);
        let clazz = self.translate(ctx, clazz);
        let args = self.translate_list(ctx, &args);
        let def = self.translate(ctx, def);
        if let TreeKind::NewClass {
            encl: e,
            clazz: c,
            args: a,
            def: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = encl;
            *c = clazz;
            *a = args;
            *d = def;
        }
        self.result = Some(tree);
    }


    fn default_visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, orig_dim_annotations_list, elemtype, dims, elems) =
            match &ctx.tree(tree).kind {
                TreeKind::NewArray {
                    annotations,
                    dim_annotations,
                    elemtype,
                    dims,
                    elems,
                } => (
                    annotations.clone(),
                    dim_annotations.clone(),
                    *elemtype,
                    dims.clone(),
                    elems.clone(),
                ),
                _ => panic!("expected a new array"),
            };
        let annotations = self.translate_annotations(ctx, &annotations);
        let mut dim_annotations = List::nil();
        for orig_dim_annotations in orig_dim_annotations_list.iter() {
            dim_annotations =
                dim_annotations.append(self.translate_annotations(ctx, &orig_dim_annotations));
        }
        let elemtype = self.translate(ctx, elemtype);
        let dims = self.translate_list(ctx, &dims);
        let elems = elems.map(|elems| self.translate_list(ctx, &elems));
        if let TreeKind::NewArray {
            annotations: a,
            dim_annotations: da,
            elemtype: et,
            dims: d,
            elems: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *da = dim_annotations;
            *et = elemtype;
            *d = dims;
            *e = elems;
        }
        self.result = Some(tree);
    }


    fn default_visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (params, body) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { params, body, .. } => (params.clone(), *body),
            _ => panic!("expected a lambda"),
        };
        let params = self.translate_list(ctx, &params);
        let body = self.translate(ctx, body);
        if let TreeKind::Lambda {
            params: p, body: b, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *p = params;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn default_visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Parens { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn default_visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Assign { lhs: l, rhs: r } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn default_visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::AssignOp { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn default_visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        let arg = self.translate(ctx, arg);
        if let TreeKind::Unary { arg: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = arg;
        }
        self.result = Some(tree);
    }


    fn default_visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Binary { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn default_visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        let clazz = self.translate(ctx, clazz);
        let expr = self.translate(ctx, expr);
        if let TreeKind::TypeCast { clazz: c, expr: e } = &mut ctx.tree_mut(tree).kind {
            *c = clazz;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn default_visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        let expr = self.translate(ctx, expr);
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::InstanceOf {
            expr: e,
            pattern: p,
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = expr;
            *p = pattern;
        }
        self.result = Some(tree);
    }


    fn default_visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        let var = self.translate(ctx, var);
        if let TreeKind::BindingPattern { var: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = var;
        }
        self.result = Some(tree);
    }


    fn default_visit_default_case_label(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::ParenthesizedPattern { pattern: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = pattern;
        }
        self.result = Some(tree);
    }


    fn default_visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        let patt = self.translate(ctx, patt);
        let expr = self.translate(ctx, expr);
        if let TreeKind::GuardPattern { patt: p, expr: e } = &mut ctx.tree_mut(tree).kind {
            *p = patt;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn default_visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        let indexed = self.translate(ctx, indexed);
        let index = self.translate(ctx, index);
        if let TreeKind::ArrayAccess {
            indexed: i,
            index: n,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = indexed;
            *n = index;
        }
        self.result = Some(tree);
    }


    fn default_visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        let selected = self.translate(ctx, selected);
        if let TreeKind::FieldAccess { selected: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = selected;
        }
        self.result = Some(tree);
    }


    fn default_visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, .. } => *expr,
            _ => panic!("expected a member reference"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::MemberReference { expr: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn default_visit_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_literal(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_type_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        let elemtype = self.translate(ctx, elemtype);
        if let TreeKind::ArrayTypeTree { elemtype: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = elemtype;
        }
        self.result = Some(tree);
    }


    fn default_visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        let clazz = self.translate(ctx, clazz);
        let arguments = self.translate_list(ctx, &arguments);
        if let TreeKind::TypeApply {
            clazz: c,
            arguments: a,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = clazz;
            *a = arguments;
        }
        self.result = Some(tree);
    }


    fn default_visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        let alternatives = self.translate_list(ctx, &alternatives);
        if let TreeKind::TypeUnion { alternatives: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = alternatives;
        }
        self.result = Some(tree);
    }


    fn default_visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeIntersection { bounds: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = bounds;
        }
        self.result = Some(tree);
    }


    fn default_visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                annotations,
                bounds,
                ..
            } => (annotations.clone(), bounds.clone()),
            _ => panic!("expected a type parameter"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeParameter {
            annotations: a,
            bounds: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *b = bounds;
        }
        self.result = Some(tree);
    }


    fn default_visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        let kind = self.translate(ctx, kind);
        let inner = self.translate(ctx, inner);
        if let TreeKind::Wildcard { kind: k, inner: i } = &mut ctx.tree_mut(tree).kind {
            *k = kind;
            *i = inner;
        }
        self.result = Some(tree);
    }


    fn default_visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        if let TreeKind::Modifiers {
            annotations: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = annotations;
        }
        self.result = Some(tree);
    }


    fn default_visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotation_type, args) = match &ctx.tree(tree).kind {
            TreeKind::Annotation {
                annotation_type,
                args,
                ..
            } => (*annotation_type, args.clone()),
            _ => panic!("expected an annotation"),
        };
        let annotation_type = self.translate(ctx, annotation_type);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::Annotation {
            annotation_type: a,
            args: ar,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotation_type;
            *ar = args;
        }
        self.result = Some(tree);
    }


    fn default_visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let underlying_type = self.translate(ctx, underlying_type);
        if let TreeKind::AnnotatedType {
            annotations: a,
            underlying_type: u,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *u = underlying_type;
        }
        self.result = Some(tree);
    }


    fn default_visit_erroneous(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn default_visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        let defs = self.translate_list(ctx, &defs);
        let expr = self.translate(ctx, expr);
        if let TreeKind::LetExpr {
            defs: d, expr: e, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *d = defs;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn default_visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, qual_id, directives) = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                qual_id,
                directives,
                ..
            } => (*mods, *qual_id, directives.clone()),
            _ => panic!("expected a module declaration"),
        };
        let mods = self.translate(ctx, mods);
        let qual_id = self.translate(ctx, qual_id);
        let directives = directives.map(|directives| self.translate_list(ctx, &directives));
        if let TreeKind::ModuleDecl {
            mods: m,
            qual_id: q,
            directives: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *q = qual_id;
            *d = directives;
        }
        self.result = Some(tree);
    }


    fn default_visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an exports directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Exports {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn default_visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an opens directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Opens {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn default_visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (service_name, impl_names) = match &ctx.tree(tree).kind {
            TreeKind::Provides {
                service_name,
                impl_names,
                ..
            } => (*service_name, impl_names.clone()),
            _ => panic!("expected a provides directive"),
        };
        let service_name = self.translate(ctx, service_name);
        let impl_names = self.translate_list(ctx, &impl_names);
        if let TreeKind::Provides {
            service_name: s,
            impl_names: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = service_name;
            *i = impl_names;
        }
        self.result = Some(tree);
    }


    fn default_visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        let module_name = self.translate(ctx, module_name);
        if let TreeKind::Requires {
            module_name: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = module_name;
        }
        self.result = Some(tree);
    }


    fn default_visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Uses { qualid: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn default_visit_tree(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        panic!("AssertionError: visitTree {:?}", tree);
    }

}

impl TreeVisitor for LambdaToMethod {
    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::CompilationUnit { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = defs;
        }
        self.result = Some(tree);
    }


    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        let annotations = self.translate_list(ctx, &annotations);
        let pid = self.translate(ctx, pid);
        if let TreeKind::PackageDecl {
            annotations: a,
            pid: p,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *p = pid;
        }
        self.result = Some(tree);
    }


    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Import { qualid: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_class_def_l2m(ctx, tree);
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, body) = match &ctx.tree(tree).kind
        {
            TreeKind::MethodDecl {
                mods,
                restype,
                typarams,
                recvparam,
                params,
                thrown,
                body,
                ..
            } => (
                *mods,
                *restype,
                typarams.clone(),
                *recvparam,
                params.clone(),
                thrown.clone(),
                *body,
            ),
            _ => panic!("expected a method declaration"),
        };
        let mods = self.translate(ctx, mods);
        let restype = self.translate(ctx, restype);
        let typarams = self.translate_type_params(ctx, &typarams);
        let recvparam = self.translate(ctx, recvparam);
        let params = self.translate_var_defs(ctx, &params);
        let thrown = self.translate_list(ctx, &thrown);
        let body = self.translate(ctx, body);
        if let TreeKind::MethodDecl {
            mods: m,
            restype: r,
            typarams: tp,
            recvparam: rp,
            params: ps,
            thrown: th,
            body: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *r = restype;
            *tp = typarams;
            *rp = recvparam;
            *ps = params;
            *th = thrown;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_var_def_l2m(ctx, tree);
    }

    fn visit_skip(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = stats;
        }
        self.result = Some(tree);
    }


    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let body = self.translate(ctx, body);
        let cond = self.translate(ctx, cond);
        if let TreeKind::DoWhileLoop { body: b, cond: c } = &mut ctx.tree_mut(tree).kind {
            *b = body;
            *c = cond;
        }
        self.result = Some(tree);
    }


    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let cond = self.translate(ctx, cond);
        let body = self.translate(ctx, body);
        if let TreeKind::WhileLoop { cond: c, body: b } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            } => (init.clone(), *cond, step.clone(), *body),
            _ => panic!("expected a for loop"),
        };
        let init = self.translate_list(ctx, &init);
        let cond = self.translate(ctx, cond);
        let step = self.translate_list(ctx, &step);
        let body = self.translate(ctx, body);
        if let TreeKind::ForLoop {
            init: i,
            cond: c,
            step: s,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = init;
            *c = cond;
            *s = step;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let var = self.translate(ctx, var);
        let expr = self.translate(ctx, expr);
        let body = self.translate(ctx, body);
        if let TreeKind::EnhancedForLoop {
            var: v,
            expr: e,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *v = var;
            *e = expr;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let body = self.translate(ctx, body);
        if let TreeKind::LabeledStatement { body: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = body;
        }
        self.result = Some(tree);
    }


    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::Switch {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        let labels = self.translate_list(ctx, &labels);
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Case {
            labels: l,
            stats: s,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *l = labels;
            *s = stats;
        }
        self.result = Some(tree);
    }


    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::SwitchExpression {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        let lock = self.translate(ctx, lock);
        let body = self.translate(ctx, body);
        if let TreeKind::Synchronized { lock: l, body: b } = &mut ctx.tree_mut(tree).kind {
            *l = lock;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (resources, body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                resources,
                body,
                catchers,
                finalizer,
                ..
            } => (resources.clone(), *body, catchers.clone(), *finalizer),
            _ => panic!("expected a try statement"),
        };
        let resources = self.translate_list(ctx, &resources);
        let body = self.translate(ctx, body);
        let catchers = self.translate_catchers(ctx, &catchers);
        let finalizer = self.translate(ctx, finalizer);
        if let TreeKind::Try {
            resources: r,
            body: b,
            catchers: c,
            finalizer: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *r = resources;
            *b = body;
            *c = catchers;
            *f = finalizer;
        }
        self.result = Some(tree);
    }


    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        let param = self.translate(ctx, param);
        let body = self.translate(ctx, body);
        if let TreeKind::Catch { param: p, body: b } = &mut ctx.tree_mut(tree).kind {
            *p = param;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (*cond, *truepart, *falsepart),
            _ => panic!("expected a conditional"),
        };
        let cond = self.translate(ctx, cond);
        let truepart = self.translate(ctx, truepart);
        let falsepart = self.translate(ctx, falsepart);
        if let TreeKind::Conditional {
            cond: c,
            truepart: t,
            falsepart: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = truepart;
            *f = falsepart;
        }
        self.result = Some(tree);
    }


    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            } => (*cond, *thenpart, *elsepart),
            _ => panic!("expected an if statement"),
        };
        let cond = self.translate(ctx, cond);
        let thenpart = self.translate(ctx, thenpart);
        let elsepart = self.translate(ctx, elsepart);
        if let TreeKind::If {
            cond: c,
            thenpart: t,
            elsepart: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = thenpart;
            *e = elsepart;
        }
        self.result = Some(tree);
    }


    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::ExpressionStatement { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_break(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        let value = self.translate(ctx, value);
        if let TreeKind::Yield { value: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = value;
        }
        self.result = Some(tree);
    }


    fn visit_continue(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Return { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Throw { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        let cond = self.translate(ctx, cond);
        let detail = self.translate(ctx, detail);
        if let TreeKind::Assert { cond: c, detail: d } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *d = detail;
        }
        self.result = Some(tree);
    }


    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        let meth = self.translate(ctx, meth);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::MethodInvocation {
            meth: m, args: a, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = meth;
            *a = args;
        }
        self.result = Some(tree);
    }


    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_new_class_l2m(ctx, tree);
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, orig_dim_annotations_list, elemtype, dims, elems) =
            match &ctx.tree(tree).kind {
                TreeKind::NewArray {
                    annotations,
                    dim_annotations,
                    elemtype,
                    dims,
                    elems,
                } => (
                    annotations.clone(),
                    dim_annotations.clone(),
                    *elemtype,
                    dims.clone(),
                    elems.clone(),
                ),
                _ => panic!("expected a new array"),
            };
        let annotations = self.translate_annotations(ctx, &annotations);
        let mut dim_annotations = List::nil();
        for orig_dim_annotations in orig_dim_annotations_list.iter() {
            dim_annotations =
                dim_annotations.append(self.translate_annotations(ctx, &orig_dim_annotations));
        }
        let elemtype = self.translate(ctx, elemtype);
        let dims = self.translate_list(ctx, &dims);
        let elems = elems.map(|elems| self.translate_list(ctx, &elems));
        if let TreeKind::NewArray {
            annotations: a,
            dim_annotations: da,
            elemtype: et,
            dims: d,
            elems: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *da = dim_annotations;
            *et = elemtype;
            *d = dims;
            *e = elems;
        }
        self.result = Some(tree);
    }


    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_lambda_l2m(ctx, tree);
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Parens { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Assign { lhs: l, rhs: r } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::AssignOp { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        let arg = self.translate(ctx, arg);
        if let TreeKind::Unary { arg: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = arg;
        }
        self.result = Some(tree);
    }


    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Binary { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        let clazz = self.translate(ctx, clazz);
        let expr = self.translate(ctx, expr);
        if let TreeKind::TypeCast { clazz: c, expr: e } = &mut ctx.tree_mut(tree).kind {
            *c = clazz;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        let expr = self.translate(ctx, expr);
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::InstanceOf {
            expr: e,
            pattern: p,
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = expr;
            *p = pattern;
        }
        self.result = Some(tree);
    }


    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        let var = self.translate(ctx, var);
        if let TreeKind::BindingPattern { var: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = var;
        }
        self.result = Some(tree);
    }


    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::ParenthesizedPattern { pattern: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = pattern;
        }
        self.result = Some(tree);
    }


    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        let patt = self.translate(ctx, patt);
        let expr = self.translate(ctx, expr);
        if let TreeKind::GuardPattern { patt: p, expr: e } = &mut ctx.tree_mut(tree).kind {
            *p = patt;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        let indexed = self.translate(ctx, indexed);
        let index = self.translate(ctx, index);
        if let TreeKind::ArrayAccess {
            indexed: i,
            index: n,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = indexed;
            *n = index;
        }
        self.result = Some(tree);
    }


    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_select_l2m(ctx, tree);
    }

    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_reference_l2m(ctx, tree);
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_ident_l2m(ctx, tree);
    }

    fn visit_literal(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_type_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        let elemtype = self.translate(ctx, elemtype);
        if let TreeKind::ArrayTypeTree { elemtype: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = elemtype;
        }
        self.result = Some(tree);
    }


    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        let clazz = self.translate(ctx, clazz);
        let arguments = self.translate_list(ctx, &arguments);
        if let TreeKind::TypeApply {
            clazz: c,
            arguments: a,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = clazz;
            *a = arguments;
        }
        self.result = Some(tree);
    }


    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        let alternatives = self.translate_list(ctx, &alternatives);
        if let TreeKind::TypeUnion { alternatives: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = alternatives;
        }
        self.result = Some(tree);
    }


    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeIntersection { bounds: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = bounds;
        }
        self.result = Some(tree);
    }


    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                annotations,
                bounds,
                ..
            } => (annotations.clone(), bounds.clone()),
            _ => panic!("expected a type parameter"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeParameter {
            annotations: a,
            bounds: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *b = bounds;
        }
        self.result = Some(tree);
    }


    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        let kind = self.translate(ctx, kind);
        let inner = self.translate(ctx, inner);
        if let TreeKind::Wildcard { kind: k, inner: i } = &mut ctx.tree_mut(tree).kind {
            *k = kind;
            *i = inner;
        }
        self.result = Some(tree);
    }


    fn visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        if let TreeKind::Modifiers {
            annotations: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = annotations;
        }
        self.result = Some(tree);
    }


    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotation_type, args) = match &ctx.tree(tree).kind {
            TreeKind::Annotation {
                annotation_type,
                args,
                ..
            } => (*annotation_type, args.clone()),
            _ => panic!("expected an annotation"),
        };
        let annotation_type = self.translate(ctx, annotation_type);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::Annotation {
            annotation_type: a,
            args: ar,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotation_type;
            *ar = args;
        }
        self.result = Some(tree);
    }


    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let underlying_type = self.translate(ctx, underlying_type);
        if let TreeKind::AnnotatedType {
            annotations: a,
            underlying_type: u,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *u = underlying_type;
        }
        self.result = Some(tree);
    }


    fn visit_erroneous(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        let defs = self.translate_list(ctx, &defs);
        let expr = self.translate(ctx, expr);
        if let TreeKind::LetExpr {
            defs: d, expr: e, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *d = defs;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, qual_id, directives) = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                qual_id,
                directives,
                ..
            } => (*mods, *qual_id, directives.clone()),
            _ => panic!("expected a module declaration"),
        };
        let mods = self.translate(ctx, mods);
        let qual_id = self.translate(ctx, qual_id);
        let directives = directives.map(|directives| self.translate_list(ctx, &directives));
        if let TreeKind::ModuleDecl {
            mods: m,
            qual_id: q,
            directives: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *q = qual_id;
            *d = directives;
        }
        self.result = Some(tree);
    }


    fn visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an exports directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Exports {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an opens directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Opens {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (service_name, impl_names) = match &ctx.tree(tree).kind {
            TreeKind::Provides {
                service_name,
                impl_names,
                ..
            } => (*service_name, impl_names.clone()),
            _ => panic!("expected a provides directive"),
        };
        let service_name = self.translate(ctx, service_name);
        let impl_names = self.translate_list(ctx, &impl_names);
        if let TreeKind::Provides {
            service_name: s,
            impl_names: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = service_name;
            *i = impl_names;
        }
        self.result = Some(tree);
    }


    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        let module_name = self.translate(ctx, module_name);
        if let TreeKind::Requires {
            module_name: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = module_name;
        }
        self.result = Some(tree);
    }


    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Uses { qualid: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn visit_tree(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        panic!("AssertionError: visitTree {:?}", tree);
    }

}
#[allow(dead_code)]
// Port adapter: unused default-navigation entries are kept for 1:1 traceability.
impl LambdaAnalyzerPreprocessor {
    /// Adapter: the TreeTranslator default navigation of the analyzer, used
    /// where the upstream analyzer visitor calls super (registered adapter).
    fn analyzer_default_visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::CompilationUnit { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = defs;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        let annotations = self.translate_list(ctx, &annotations);
        let pid = self.translate(ctx, pid);
        if let TreeKind::PackageDecl {
            annotations: a,
            pid: p,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *p = pid;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Import { qualid: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, typarams, extending, implementing, defs) = match &ctx.tree(tree).kind {
            TreeKind::ClassDecl {
                mods,
                typarams,
                extending,
                implementing,
                defs,
                ..
            } => (
                *mods,
                typarams.clone(),
                *extending,
                implementing.clone(),
                defs.clone(),
            ),
            _ => panic!("expected a class declaration"),
        };
        let mods = self.translate(ctx, mods);
        let typarams = self.translate_type_params(ctx, &typarams);
        let extending = self.translate(ctx, extending);
        let implementing = self.translate_list(ctx, &implementing);
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::ClassDecl {
            mods: m,
            typarams: t,
            extending: e,
            implementing: i,
            defs: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *t = typarams;
            *e = extending;
            *i = implementing;
            *d = defs;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, body) = match &ctx.tree(tree).kind
        {
            TreeKind::MethodDecl {
                mods,
                restype,
                typarams,
                recvparam,
                params,
                thrown,
                body,
                ..
            } => (
                *mods,
                *restype,
                typarams.clone(),
                *recvparam,
                params.clone(),
                thrown.clone(),
                *body,
            ),
            _ => panic!("expected a method declaration"),
        };
        let mods = self.translate(ctx, mods);
        let restype = self.translate(ctx, restype);
        let typarams = self.translate_type_params(ctx, &typarams);
        let recvparam = self.translate(ctx, recvparam);
        let params = self.translate_var_defs(ctx, &params);
        let thrown = self.translate_list(ctx, &thrown);
        let body = self.translate(ctx, body);
        if let TreeKind::MethodDecl {
            mods: m,
            restype: r,
            typarams: tp,
            recvparam: rp,
            params: ps,
            thrown: th,
            body: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *r = restype;
            *tp = typarams;
            *rp = recvparam;
            *ps = params;
            *th = thrown;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, nameexpr, vartype, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                nameexpr,
                vartype,
                init,
                ..
            } => (*mods, *nameexpr, *vartype, *init),
            _ => panic!("expected a variable declaration"),
        };
        let mods = self.translate(ctx, mods);
        let nameexpr = self.translate(ctx, nameexpr);
        let vartype = self.translate(ctx, vartype);
        let init = self.translate(ctx, init);
        if let TreeKind::VariableDecl {
            mods: m,
            nameexpr: ne,
            vartype: vt,
            init: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *ne = nameexpr;
            *vt = vartype;
            *i = init;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_skip(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = stats;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let body = self.translate(ctx, body);
        let cond = self.translate(ctx, cond);
        if let TreeKind::DoWhileLoop { body: b, cond: c } = &mut ctx.tree_mut(tree).kind {
            *b = body;
            *c = cond;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let cond = self.translate(ctx, cond);
        let body = self.translate(ctx, body);
        if let TreeKind::WhileLoop { cond: c, body: b } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            } => (init.clone(), *cond, step.clone(), *body),
            _ => panic!("expected a for loop"),
        };
        let init = self.translate_list(ctx, &init);
        let cond = self.translate(ctx, cond);
        let step = self.translate_list(ctx, &step);
        let body = self.translate(ctx, body);
        if let TreeKind::ForLoop {
            init: i,
            cond: c,
            step: s,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = init;
            *c = cond;
            *s = step;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let var = self.translate(ctx, var);
        let expr = self.translate(ctx, expr);
        let body = self.translate(ctx, body);
        if let TreeKind::EnhancedForLoop {
            var: v,
            expr: e,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *v = var;
            *e = expr;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let body = self.translate(ctx, body);
        if let TreeKind::LabeledStatement { body: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = body;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::Switch {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        let labels = self.translate_list(ctx, &labels);
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Case {
            labels: l,
            stats: s,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *l = labels;
            *s = stats;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::SwitchExpression {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        let lock = self.translate(ctx, lock);
        let body = self.translate(ctx, body);
        if let TreeKind::Synchronized { lock: l, body: b } = &mut ctx.tree_mut(tree).kind {
            *l = lock;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (resources, body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                resources,
                body,
                catchers,
                finalizer,
                ..
            } => (resources.clone(), *body, catchers.clone(), *finalizer),
            _ => panic!("expected a try statement"),
        };
        let resources = self.translate_list(ctx, &resources);
        let body = self.translate(ctx, body);
        let catchers = self.translate_catchers(ctx, &catchers);
        let finalizer = self.translate(ctx, finalizer);
        if let TreeKind::Try {
            resources: r,
            body: b,
            catchers: c,
            finalizer: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *r = resources;
            *b = body;
            *c = catchers;
            *f = finalizer;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        let param = self.translate(ctx, param);
        let body = self.translate(ctx, body);
        if let TreeKind::Catch { param: p, body: b } = &mut ctx.tree_mut(tree).kind {
            *p = param;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (*cond, *truepart, *falsepart),
            _ => panic!("expected a conditional"),
        };
        let cond = self.translate(ctx, cond);
        let truepart = self.translate(ctx, truepart);
        let falsepart = self.translate(ctx, falsepart);
        if let TreeKind::Conditional {
            cond: c,
            truepart: t,
            falsepart: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = truepart;
            *f = falsepart;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            } => (*cond, *thenpart, *elsepart),
            _ => panic!("expected an if statement"),
        };
        let cond = self.translate(ctx, cond);
        let thenpart = self.translate(ctx, thenpart);
        let elsepart = self.translate(ctx, elsepart);
        if let TreeKind::If {
            cond: c,
            thenpart: t,
            elsepart: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = thenpart;
            *e = elsepart;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::ExpressionStatement { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_break(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        let value = self.translate(ctx, value);
        if let TreeKind::Yield { value: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = value;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_continue(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Return { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Throw { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        let cond = self.translate(ctx, cond);
        let detail = self.translate(ctx, detail);
        if let TreeKind::Assert { cond: c, detail: d } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *d = detail;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        let meth = self.translate(ctx, meth);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::MethodInvocation {
            meth: m, args: a, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = meth;
            *a = args;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                clazz,
                args,
                def,
                ..
            } => (*encl, *clazz, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        let encl = self.translate(ctx, encl);
        let clazz = self.translate(ctx, clazz);
        let args = self.translate_list(ctx, &args);
        let def = self.translate(ctx, def);
        if let TreeKind::NewClass {
            encl: e,
            clazz: c,
            args: a,
            def: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = encl;
            *c = clazz;
            *a = args;
            *d = def;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, orig_dim_annotations_list, elemtype, dims, elems) =
            match &ctx.tree(tree).kind {
                TreeKind::NewArray {
                    annotations,
                    dim_annotations,
                    elemtype,
                    dims,
                    elems,
                } => (
                    annotations.clone(),
                    dim_annotations.clone(),
                    *elemtype,
                    dims.clone(),
                    elems.clone(),
                ),
                _ => panic!("expected a new array"),
            };
        let annotations = self.translate_annotations(ctx, &annotations);
        let mut dim_annotations = List::nil();
        for orig_dim_annotations in orig_dim_annotations_list.iter() {
            dim_annotations =
                dim_annotations.append(self.translate_annotations(ctx, &orig_dim_annotations));
        }
        let elemtype = self.translate(ctx, elemtype);
        let dims = self.translate_list(ctx, &dims);
        let elems = elems.map(|elems| self.translate_list(ctx, &elems));
        if let TreeKind::NewArray {
            annotations: a,
            dim_annotations: da,
            elemtype: et,
            dims: d,
            elems: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *da = dim_annotations;
            *et = elemtype;
            *d = dims;
            *e = elems;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (params, body) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { params, body, .. } => (params.clone(), *body),
            _ => panic!("expected a lambda"),
        };
        let params = self.translate_list(ctx, &params);
        let body = self.translate(ctx, body);
        if let TreeKind::Lambda {
            params: p, body: b, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *p = params;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Parens { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Assign { lhs: l, rhs: r } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::AssignOp { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        let arg = self.translate(ctx, arg);
        if let TreeKind::Unary { arg: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = arg;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Binary { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        let clazz = self.translate(ctx, clazz);
        let expr = self.translate(ctx, expr);
        if let TreeKind::TypeCast { clazz: c, expr: e } = &mut ctx.tree_mut(tree).kind {
            *c = clazz;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        let expr = self.translate(ctx, expr);
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::InstanceOf {
            expr: e,
            pattern: p,
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = expr;
            *p = pattern;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        let var = self.translate(ctx, var);
        if let TreeKind::BindingPattern { var: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = var;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_default_case_label(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::ParenthesizedPattern { pattern: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = pattern;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        let patt = self.translate(ctx, patt);
        let expr = self.translate(ctx, expr);
        if let TreeKind::GuardPattern { patt: p, expr: e } = &mut ctx.tree_mut(tree).kind {
            *p = patt;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        let indexed = self.translate(ctx, indexed);
        let index = self.translate(ctx, index);
        if let TreeKind::ArrayAccess {
            indexed: i,
            index: n,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = indexed;
            *n = index;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        let selected = self.translate(ctx, selected);
        if let TreeKind::FieldAccess { selected: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = selected;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, .. } => *expr,
            _ => panic!("expected a member reference"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::MemberReference { expr: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_literal(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        let elemtype = self.translate(ctx, elemtype);
        if let TreeKind::ArrayTypeTree { elemtype: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = elemtype;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        let clazz = self.translate(ctx, clazz);
        let arguments = self.translate_list(ctx, &arguments);
        if let TreeKind::TypeApply {
            clazz: c,
            arguments: a,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = clazz;
            *a = arguments;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        let alternatives = self.translate_list(ctx, &alternatives);
        if let TreeKind::TypeUnion { alternatives: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = alternatives;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeIntersection { bounds: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = bounds;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                annotations,
                bounds,
                ..
            } => (annotations.clone(), bounds.clone()),
            _ => panic!("expected a type parameter"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeParameter {
            annotations: a,
            bounds: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *b = bounds;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        let kind = self.translate(ctx, kind);
        let inner = self.translate(ctx, inner);
        if let TreeKind::Wildcard { kind: k, inner: i } = &mut ctx.tree_mut(tree).kind {
            *k = kind;
            *i = inner;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        if let TreeKind::Modifiers {
            annotations: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = annotations;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotation_type, args) = match &ctx.tree(tree).kind {
            TreeKind::Annotation {
                annotation_type,
                args,
                ..
            } => (*annotation_type, args.clone()),
            _ => panic!("expected an annotation"),
        };
        let annotation_type = self.translate(ctx, annotation_type);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::Annotation {
            annotation_type: a,
            args: ar,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotation_type;
            *ar = args;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let underlying_type = self.translate(ctx, underlying_type);
        if let TreeKind::AnnotatedType {
            annotations: a,
            underlying_type: u,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *u = underlying_type;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_erroneous(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn analyzer_default_visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        let defs = self.translate_list(ctx, &defs);
        let expr = self.translate(ctx, expr);
        if let TreeKind::LetExpr {
            defs: d, expr: e, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *d = defs;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, qual_id, directives) = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                qual_id,
                directives,
                ..
            } => (*mods, *qual_id, directives.clone()),
            _ => panic!("expected a module declaration"),
        };
        let mods = self.translate(ctx, mods);
        let qual_id = self.translate(ctx, qual_id);
        let directives = directives.map(|directives| self.translate_list(ctx, &directives));
        if let TreeKind::ModuleDecl {
            mods: m,
            qual_id: q,
            directives: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *q = qual_id;
            *d = directives;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an exports directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Exports {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an opens directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Opens {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (service_name, impl_names) = match &ctx.tree(tree).kind {
            TreeKind::Provides {
                service_name,
                impl_names,
                ..
            } => (*service_name, impl_names.clone()),
            _ => panic!("expected a provides directive"),
        };
        let service_name = self.translate(ctx, service_name);
        let impl_names = self.translate_list(ctx, &impl_names);
        if let TreeKind::Provides {
            service_name: s,
            impl_names: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = service_name;
            *i = impl_names;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        let module_name = self.translate(ctx, module_name);
        if let TreeKind::Requires {
            module_name: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = module_name;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Uses { qualid: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn analyzer_default_visit_tree(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        panic!("AssertionError: visitTree {:?}", tree);
    }

}

impl TreeVisitor for LambdaAnalyzerPreprocessor {
    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::CompilationUnit { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = defs;
        }
        self.result = Some(tree);
    }


    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        let annotations = self.translate_list(ctx, &annotations);
        let pid = self.translate(ctx, pid);
        if let TreeKind::PackageDecl {
            annotations: a,
            pid: p,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *p = pid;
        }
        self.result = Some(tree);
    }


    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Import { qualid: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_class_def(ctx, tree);
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_method_def(ctx, tree);
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_var_def(ctx, tree);
    }

    fn visit_skip(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_block(ctx, tree);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let body = self.translate(ctx, body);
        let cond = self.translate(ctx, cond);
        if let TreeKind::DoWhileLoop { body: b, cond: c } = &mut ctx.tree_mut(tree).kind {
            *b = body;
            *c = cond;
        }
        self.result = Some(tree);
    }


    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let cond = self.translate(ctx, cond);
        let body = self.translate(ctx, body);
        if let TreeKind::WhileLoop { cond: c, body: b } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            } => (init.clone(), *cond, step.clone(), *body),
            _ => panic!("expected a for loop"),
        };
        let init = self.translate_list(ctx, &init);
        let cond = self.translate(ctx, cond);
        let step = self.translate_list(ctx, &step);
        let body = self.translate(ctx, body);
        if let TreeKind::ForLoop {
            init: i,
            cond: c,
            step: s,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = init;
            *c = cond;
            *s = step;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let var = self.translate(ctx, var);
        let expr = self.translate(ctx, expr);
        let body = self.translate(ctx, body);
        if let TreeKind::EnhancedForLoop {
            var: v,
            expr: e,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *v = var;
            *e = expr;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let body = self.translate(ctx, body);
        if let TreeKind::LabeledStatement { body: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = body;
        }
        self.result = Some(tree);
    }


    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::Switch {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        let labels = self.translate_list(ctx, &labels);
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Case {
            labels: l,
            stats: s,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *l = labels;
            *s = stats;
        }
        self.result = Some(tree);
    }


    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::SwitchExpression {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        let lock = self.translate(ctx, lock);
        let body = self.translate(ctx, body);
        if let TreeKind::Synchronized { lock: l, body: b } = &mut ctx.tree_mut(tree).kind {
            *l = lock;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (resources, body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                resources,
                body,
                catchers,
                finalizer,
                ..
            } => (resources.clone(), *body, catchers.clone(), *finalizer),
            _ => panic!("expected a try statement"),
        };
        let resources = self.translate_list(ctx, &resources);
        let body = self.translate(ctx, body);
        let catchers = self.translate_catchers(ctx, &catchers);
        let finalizer = self.translate(ctx, finalizer);
        if let TreeKind::Try {
            resources: r,
            body: b,
            catchers: c,
            finalizer: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *r = resources;
            *b = body;
            *c = catchers;
            *f = finalizer;
        }
        self.result = Some(tree);
    }


    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        let param = self.translate(ctx, param);
        let body = self.translate(ctx, body);
        if let TreeKind::Catch { param: p, body: b } = &mut ctx.tree_mut(tree).kind {
            *p = param;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (*cond, *truepart, *falsepart),
            _ => panic!("expected a conditional"),
        };
        let cond = self.translate(ctx, cond);
        let truepart = self.translate(ctx, truepart);
        let falsepart = self.translate(ctx, falsepart);
        if let TreeKind::Conditional {
            cond: c,
            truepart: t,
            falsepart: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = truepart;
            *f = falsepart;
        }
        self.result = Some(tree);
    }


    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            } => (*cond, *thenpart, *elsepart),
            _ => panic!("expected an if statement"),
        };
        let cond = self.translate(ctx, cond);
        let thenpart = self.translate(ctx, thenpart);
        let elsepart = self.translate(ctx, elsepart);
        if let TreeKind::If {
            cond: c,
            thenpart: t,
            elsepart: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = thenpart;
            *e = elsepart;
        }
        self.result = Some(tree);
    }


    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::ExpressionStatement { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_break(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        let value = self.translate(ctx, value);
        if let TreeKind::Yield { value: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = value;
        }
        self.result = Some(tree);
    }


    fn visit_continue(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Return { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Throw { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        let cond = self.translate(ctx, cond);
        let detail = self.translate(ctx, detail);
        if let TreeKind::Assert { cond: c, detail: d } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *d = detail;
        }
        self.result = Some(tree);
    }


    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_apply(ctx, tree);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_new_class(ctx, tree);
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, orig_dim_annotations_list, elemtype, dims, elems) =
            match &ctx.tree(tree).kind {
                TreeKind::NewArray {
                    annotations,
                    dim_annotations,
                    elemtype,
                    dims,
                    elems,
                } => (
                    annotations.clone(),
                    dim_annotations.clone(),
                    *elemtype,
                    dims.clone(),
                    elems.clone(),
                ),
                _ => panic!("expected a new array"),
            };
        let annotations = self.translate_annotations(ctx, &annotations);
        let mut dim_annotations = List::nil();
        for orig_dim_annotations in orig_dim_annotations_list.iter() {
            dim_annotations =
                dim_annotations.append(self.translate_annotations(ctx, &orig_dim_annotations));
        }
        let elemtype = self.translate(ctx, elemtype);
        let dims = self.translate_list(ctx, &dims);
        let elems = elems.map(|elems| self.translate_list(ctx, &elems));
        if let TreeKind::NewArray {
            annotations: a,
            dim_annotations: da,
            elemtype: et,
            dims: d,
            elems: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *da = dim_annotations;
            *et = elemtype;
            *d = dims;
            *e = elems;
        }
        self.result = Some(tree);
    }


    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_lambda(ctx, tree);
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Parens { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Assign { lhs: l, rhs: r } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::AssignOp { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        let arg = self.translate(ctx, arg);
        if let TreeKind::Unary { arg: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = arg;
        }
        self.result = Some(tree);
    }


    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Binary { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        let clazz = self.translate(ctx, clazz);
        let expr = self.translate(ctx, expr);
        if let TreeKind::TypeCast { clazz: c, expr: e } = &mut ctx.tree_mut(tree).kind {
            *c = clazz;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        let expr = self.translate(ctx, expr);
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::InstanceOf {
            expr: e,
            pattern: p,
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = expr;
            *p = pattern;
        }
        self.result = Some(tree);
    }


    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        let var = self.translate(ctx, var);
        if let TreeKind::BindingPattern { var: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = var;
        }
        self.result = Some(tree);
    }


    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::ParenthesizedPattern { pattern: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = pattern;
        }
        self.result = Some(tree);
    }


    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        let patt = self.translate(ctx, patt);
        let expr = self.translate(ctx, expr);
        if let TreeKind::GuardPattern { patt: p, expr: e } = &mut ctx.tree_mut(tree).kind {
            *p = patt;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        let indexed = self.translate(ctx, indexed);
        let index = self.translate(ctx, index);
        if let TreeKind::ArrayAccess {
            indexed: i,
            index: n,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = indexed;
            *n = index;
        }
        self.result = Some(tree);
    }


    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_select(ctx, tree);
    }

    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_reference(ctx, tree);
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.analyzer_visit_ident(ctx, tree);
    }

    fn visit_literal(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_type_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        let elemtype = self.translate(ctx, elemtype);
        if let TreeKind::ArrayTypeTree { elemtype: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = elemtype;
        }
        self.result = Some(tree);
    }


    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        let clazz = self.translate(ctx, clazz);
        let arguments = self.translate_list(ctx, &arguments);
        if let TreeKind::TypeApply {
            clazz: c,
            arguments: a,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = clazz;
            *a = arguments;
        }
        self.result = Some(tree);
    }


    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        let alternatives = self.translate_list(ctx, &alternatives);
        if let TreeKind::TypeUnion { alternatives: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = alternatives;
        }
        self.result = Some(tree);
    }


    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeIntersection { bounds: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = bounds;
        }
        self.result = Some(tree);
    }


    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                annotations,
                bounds,
                ..
            } => (annotations.clone(), bounds.clone()),
            _ => panic!("expected a type parameter"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeParameter {
            annotations: a,
            bounds: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *b = bounds;
        }
        self.result = Some(tree);
    }


    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        let kind = self.translate(ctx, kind);
        let inner = self.translate(ctx, inner);
        if let TreeKind::Wildcard { kind: k, inner: i } = &mut ctx.tree_mut(tree).kind {
            *k = kind;
            *i = inner;
        }
        self.result = Some(tree);
    }


    fn visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        if let TreeKind::Modifiers {
            annotations: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = annotations;
        }
        self.result = Some(tree);
    }


    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotation_type, args) = match &ctx.tree(tree).kind {
            TreeKind::Annotation {
                annotation_type,
                args,
                ..
            } => (*annotation_type, args.clone()),
            _ => panic!("expected an annotation"),
        };
        let annotation_type = self.translate(ctx, annotation_type);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::Annotation {
            annotation_type: a,
            args: ar,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotation_type;
            *ar = args;
        }
        self.result = Some(tree);
    }


    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let underlying_type = self.translate(ctx, underlying_type);
        if let TreeKind::AnnotatedType {
            annotations: a,
            underlying_type: u,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *u = underlying_type;
        }
        self.result = Some(tree);
    }


    fn visit_erroneous(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        let defs = self.translate_list(ctx, &defs);
        let expr = self.translate(ctx, expr);
        if let TreeKind::LetExpr {
            defs: d, expr: e, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *d = defs;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, qual_id, directives) = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                qual_id,
                directives,
                ..
            } => (*mods, *qual_id, directives.clone()),
            _ => panic!("expected a module declaration"),
        };
        let mods = self.translate(ctx, mods);
        let qual_id = self.translate(ctx, qual_id);
        let directives = directives.map(|directives| self.translate_list(ctx, &directives));
        if let TreeKind::ModuleDecl {
            mods: m,
            qual_id: q,
            directives: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *q = qual_id;
            *d = directives;
        }
        self.result = Some(tree);
    }


    fn visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an exports directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Exports {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an opens directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Opens {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (service_name, impl_names) = match &ctx.tree(tree).kind {
            TreeKind::Provides {
                service_name,
                impl_names,
                ..
            } => (*service_name, impl_names.clone()),
            _ => panic!("expected a provides directive"),
        };
        let service_name = self.translate(ctx, service_name);
        let impl_names = self.translate_list(ctx, &impl_names);
        if let TreeKind::Provides {
            service_name: s,
            impl_names: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = service_name;
            *i = impl_names;
        }
        self.result = Some(tree);
    }


    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        let module_name = self.translate(ctx, module_name);
        if let TreeKind::Requires {
            module_name: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = module_name;
        }
        self.result = Some(tree);
    }


    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Uses { qualid: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn visit_tree(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        panic!("AssertionError: visitTree {:?}", tree);
    }

}
/// Signature Generation
pub struct L2MSignatureGenerator {
    sb: String,
    allow_illegal_signatures: bool,
    types: Rc<Types>,
}

impl L2MSignatureGenerator {
    /// Creates the signature generator over the given type services.
    pub fn new(types: Rc<Types>, allow_illegal_signatures: bool) -> L2MSignatureGenerator {
        L2MSignatureGenerator {
            sb: String::new(),
            allow_illegal_signatures,
            types,
        }
    }
    /// Finishes the signature: appends the trailing semicolon and returns the
    /// assembled string.
    pub fn finish(&self) -> String {
        self.sb.clone()
    }
}

impl SignatureGenerator for L2MSignatureGenerator {
    fn append_char(&mut self, ch: char) {
        self.sb.push(ch);
    }
    fn append_bytes(&mut self, ba: &[u8]) {
        for byte in ba {
            self.sb.push(*byte as char);
        }
    }
    fn append_name(&mut self, name: &Name) {
        self.sb.push_str(&name.to_java_string().to_string_lossy());
    }

    /// The Types service owning this generator.
    fn types(&self) -> &Types {
        &self.types
    }
    fn report_illegal_signature(
        &mut self,
        t: TypeId,
    ) -> Result<(), crate::code::types::InvalidSignatureException> {
        if self.allow_illegal_signatures {
            Ok(())
        } else {
            Err(crate::code::types::InvalidSignatureException { type_: t })
        }
    }
}
fn is_field_symbol(ctx: &CompilerContext, sym: SymbolId) -> bool {
    ctx.sym(sym).kind == Kind::VAR
        && ctx
            .sym(sym)
            .owner
            .map(|owner| ctx.sym(owner).kind == Kind::TYP)
            .unwrap_or(false)
}

/// The translated symbol of one original symbol in a lambda context.
fn context_symbol(
    lambda: &LambdaTranslationContext,
    kind: LambdaSymbolKind,
    sym: SymbolId,
) -> Option<SymbolId> {
    lambda
        .symbol_map(kind)
        .iter()
        .find(|(original, _)| *original == sym)
        .map(|(_, translated)| *translated)
}
/// The original captured symbols of one lambda symbol kind (TranslationContext
/// symbol maps).
fn captured_symbols(lambda: &LambdaTranslationContext, kind: LambdaSymbolKind) -> Vec<SymbolId> {
    lambda
        .symbol_map(kind)
        .iter()
        .map(|(original, _)| *original)
        .collect()
}




impl LambdaAnalyzerPreprocessor {
    /// Create new synthetic method with given flags, name, type, owner
    fn make_private_synthetic_method(
        &mut self,
        ctx: &mut CompilerContext,
        flags_field: i64,
        name: Name,
        ty: Option<TypeId>,
        owner: SymbolId,
    ) -> SymbolId {
        ctx.alloc_symbol(Symbol::new(
            Kind::MTH,
            flags_field | flags::SYNTHETIC as i64 | flags::PRIVATE as i64,
            name,
            ty,
            Some(owner),
            SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
                original_enclosing_method: None,
            },
        ))
    }
    fn new_var_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        flags_field: i64,
        name: Name,
        ty: TypeId,
        owner: SymbolId,
    ) -> SymbolId {
        ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            flags_field,
            name,
            Some(ty),
            Some(owner),
            SymbolKind::VarSymbol {
                lazy_const_value: None,
                pos: 0,
                adr: 0,
                data_kind: None,
                data: None,
            },
        ))
    }
    fn type_sig(&self, ctx: &mut CompilerContext, ty: TypeId, allow_illegal_signature: bool) -> String {
        let mut generator = L2MSignatureGenerator::new(Rc::clone(&self.types), allow_illegal_signature);
        let syms = self.syms.borrow();
        let _ = generator.assemble_sig(ctx, &syms, ty);
        drop(syms);
        generator.finish()
    }
    /// Adapter: Types.findDescriptorSymbol with the analyzer services.
    fn find_descriptor_symbol_checked(&self, ctx: &mut CompilerContext, origin: SymbolId) -> Option<SymbolId> {
        let attr = self.attr.borrow();
        let chk = Rc::clone(&attr.chk);
        let chk = chk;
        let mut syms = self.syms.borrow_mut();
        self.types
            .find_descriptor_symbol(ctx, &mut syms, &self.diags, &chk, origin)
            .ok()
    }

    /// Adapter: Types.findDescriptorType with the analyzer services.
    fn find_descriptor_type_checked(&self, ctx: &mut CompilerContext, origin: TypeId) -> Option<TypeId> {
        let attr = self.attr.borrow();
        let chk = Rc::clone(&attr.chk);
        let chk = chk;
        let mut syms = self.syms.borrow_mut();
        self.types
            .find_descriptor_type(ctx, &mut syms, &self.diags, &chk, origin)
            .ok()
    }
    fn member_reference_receiver(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
    ) -> Option<TreeId> {
        let kind = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { kind, .. } => *kind,
            _ => None,
        };
        // Java: memberReferenceReceiver returns null for STATIC/UNBOUND; only
        // BOUND references carry the receiver expression. Returning the
        // qualifier unconditionally added the class symbol T as a lambda
        // parameter for T::m.
        if !matches!(kind, Some(crate::tree::jc_tree::ReferenceKind::BOUND)) {
            return None;
        }
        let qualifier = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr: Some(expr), .. } => *expr,
            _ => return None,
        };
        let attr = Rc::clone(&self.attr);
        let mut attr = attr.borrow_mut();
        let mut syms = self.syms.borrow_mut();
        Some(attr.make_null_check(ctx, &mut syms, qualifier))
    }
    fn member_reference_to_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> TreeId {
        let mut conversion = MemberReferenceToLambda::new(
            tree,
            Rc::clone(&self.make),
            Rc::clone(&self.names),
            Rc::clone(&self.syms),
            Rc::clone(&self.types),
            Some(Rc::clone(&self.attr)),
            Rc::clone(&self.diags),
            Rc::clone(&self.trans_types),
            self.attr_env.clone(),
        );
        let owner = self.owner(ctx, false).expect("owner");
        conversion.lambda(ctx, owner)
    }
}
/// Creates a synthetic variable with the given flags, name, type and owner
/// (upstream makeSyntheticVar).
fn make_synthetic_var(
    ctx: &mut CompilerContext,
    flags_field: i64,
    name: Name,
    ty: TypeId,
    owner: SymbolId,
) -> SymbolId {
    ctx.alloc_symbol(Symbol::new(
        Kind::VAR,
        flags_field | flags::SYNTHETIC as i64,
        name,
        Some(ty),
        Some(owner),
        SymbolKind::VarSymbol {
            lazy_const_value: None,
            pos: 0,
            adr: 0,
            data_kind: None,
            data: None,
        },
    ))
}

/// Whether the reference target is a protected member of a super class of
/// the enclosing class and lives in another package (upstream
/// isProtectedInSuperClassOfEnclosingClassInOtherPackage).
fn is_protected_in_super_class_of_enclosing_class_in_other_package(
    ctx: &CompilerContext,
    target_reference: SymbolId,
    current_class: SymbolId,
) -> bool {
    let target = ctx.sym(target_reference);
    if target.flags_field & flags::PROTECTED as i64 == 0 {
        return false;
    }
    crate::code::symbol::packge(ctx, target_reference)
        != crate::code::symbol::packge(ctx, current_class)
}

/// The tree translator of one lambda body (upstream LambdaBodyTranslator).
pub struct LambdaBodyTranslator {
    pub is_target_void: bool,
    pub lambda_method_decl: TreeId,
    pub result: Option<TreeId>,
    pub base: TreeTranslator,
    pub names: Rc<Names>,
    pub make: Rc<RefCell<TreeMaker>>,
    pub syms: Rc<RefCell<Symtab>>,
}

impl LambdaBodyTranslator {
    fn translate_type_params(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    fn translate_var_defs(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    fn translate_cases(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    fn translate_catchers(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    fn translate_annotations(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        self.translate_list(ctx, trees)
    }
    pub fn translate(&mut self, ctx: &mut CompilerContext, tree: Option<TreeId>) -> Option<TreeId> {
        let tree = tree?;
        self.result = None;
        crate::tree::jc_tree::accept(ctx, tree, self);
        self.result.take().or(Some(tree))
    }
}

impl LambdaToMethod {
    /// Creates a synthetic method with the given flags, name, type and owner
    /// (upstream makePrivateSyntheticMethod).
    fn make_private_synthetic_method(
        &mut self,
        ctx: &mut CompilerContext,
        flags_field: i64,
        name: Name,
        ty: Option<TypeId>,
        owner: SymbolId,
    ) -> SymbolId {
        ctx.alloc_symbol(Symbol::new(
            Kind::MTH,
            flags_field | flags::SYNTHETIC as i64 | flags::PRIVATE as i64,
            name,
            ty,
            Some(owner),
            SymbolKind::MethodSymbol {
                extra_params: List::nil(),
                captured_locals: List::nil(),
                params: List::nil(),
                default_value: None,
                original_enclosing_method: None,
            },
        ))
    }

    /// Creates a synthetic variable with the given flags, name, type and owner
    /// (upstream makeSyntheticVar).
    fn new_var_symbol(
        &mut self,
        ctx: &mut CompilerContext,
        flags_field: i64,
        name: Name,
        ty: TypeId,
        owner: SymbolId,
    ) -> SymbolId {
        ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            flags_field,
            name,
            Some(ty),
            Some(owner),
            SymbolKind::VarSymbol {
                lazy_const_value: None,
                pos: 0,
                adr: 0,
                data_kind: None,
                data: None,
            },
        ))
    }

    /// The signature of a type (upstream typeSig).
    fn type_sig(&self, ctx: &mut CompilerContext, ty: TypeId, allow_illegal_signature: bool) -> String {
        let mut generator = L2MSignatureGenerator::new(Rc::clone(&self.types), allow_illegal_signature);
        let syms = self.syms.borrow();
        let _ = generator.assemble_sig(ctx, &syms, ty);
        drop(syms);
        generator.finish()
    }

    /// The signature of a class type (upstream classSig).
    #[allow(dead_code)]
    fn class_sig(&self, ctx: &mut CompilerContext, ty: TypeId) -> String {
        let mut generator = L2MSignatureGenerator::new(Rc::clone(&self.types), false);
        let syms = self.syms.borrow();
        let _ = generator.assemble_class_sig(ctx, &syms, ty);
        drop(syms);
        generator.finish()
    }

    /// Adapter: Types.findDescriptorSymbol with the pass services.
    fn find_descriptor_symbol_checked(&mut self, ctx: &mut CompilerContext, origin: SymbolId) -> Option<SymbolId> {
        let attr = self.attr.borrow();
        let chk = Rc::clone(&attr.chk);
        let chk = chk;
        let mut syms = self.syms.borrow_mut();
        self.types
            .find_descriptor_symbol(ctx, &mut syms, &self.diags, &chk, origin)
            .ok()
    }

    /// Adapter: Types.findDescriptorType with the pass services.
    fn find_descriptor_type_checked(&mut self, ctx: &mut CompilerContext, origin: TypeId) -> Option<TypeId> {
        let attr = self.attr.borrow();
        let chk = Rc::clone(&attr.chk);
        let chk = chk;
        let mut syms = self.syms.borrow_mut();
        self.types
            .find_descriptor_type(ctx, &mut syms, &self.diags, &chk, origin)
            .ok()
    }

    /// Converts a type to the corresponding method type constant (upstream
    /// typeToMethodType).
    fn type_to_method_type(&mut self, ctx: &mut CompilerContext, mt: TypeId) -> TypeId {
        let erased = {
            let syms = self.syms.borrow();
            self.types.erasure(ctx, &syms, mt)
        };
        let (parameter_types, return_type, thrown_types) = {
            let type_ = ctx.typ(erased);
            (
                type_.get_parameter_types(ctx),
                type_.get_return_type(ctx).expect("method return type"),
                type_.get_thrown_types(ctx),
            )
        };
        let method_class = self.syms.borrow().method_class;
        ctx.alloc_type(Type::new_method_type(
            parameter_types,
            return_type,
            thrown_types,
            method_class,
        ))
    }

    /// Creates the method declaration hoisting the lambda body.
    fn make_lambda_method_decl(&mut self, ctx: &mut CompilerContext, tree: TreeId, sym: SymbolId, lambda_type: TypeId) -> TreeId {
        let lambda_restype = ctx.typ(lambda_type).get_return_type(ctx).expect("lambda return type");
        let synthetic_params = {
            let contexts = self.context_map.borrow();
            match contexts.get(&tree).map(|context| &context.kind) {
                Some(TranslationContextKind::Lambda(lambda)) => lambda.synthetic_params.clone().unwrap_or_default(),
                _ => Vec::new(),
            }
        };
        let thrown_types = ctx.typ(lambda_type).get_thrown_types(ctx);
        let thrown_trees = {
            let mut out: List<TreeId> = List::nil();
            let mut make = self.make.borrow_mut();
            let mut syms = self.syms.borrow_mut();
            for ty in thrown_types.iter() {
                let tree_ty = make.type_from_type(ctx, &mut syms, ty);
                out = out.append(tree_ty);
            }
            out
        };
        let mods = {
            let mut make = self.make.borrow_mut();
            make.modifiers(ctx, ctx.sym(sym).flags_field)
        };
        let restype_tree = {
            let restype_tsym = ctx.typ(lambda_restype).tsym.expect("lambda return type symbol");
            let mut make = self.make.borrow_mut();
            let mut syms = self.syms.borrow_mut();
            make.qual_ident(ctx, &mut syms, restype_tsym)
        };
        let name = ctx.sym(sym).name.clone();
        let params = List::from_iterable(synthetic_params);
        let decl = {
            let mut make = self.make.borrow_mut();
            make.method_def(
                ctx,
                mods,
                name,
                Some(restype_tree),
                List::nil(),
                None,
                params,
                thrown_trees,
                None,
                None,
            )
        };
        if let TreeKind::MethodDecl { sym: slot, .. } = &mut ctx.tree_mut(decl).kind {
            *slot = Some(sym);
        }
        ctx.tree_mut(decl).ty = Some(lambda_type);
        decl
    }

    /// Creates the body block of the translated lambda method (upstream makeLambdaBody).
    fn make_lambda_body(&mut self, ctx: &mut CompilerContext, tree: TreeId, lambda_method_decl: TreeId) -> TreeId {
        let (body, can_complete_normally) = match &ctx.tree(tree).kind {
            TreeKind::Lambda { body: Some(body), can_complete_normally, .. } => (*body, *can_complete_normally),
            _ => panic!("expected a lambda body"),
        };
        if ctx.tree(body).has_tag(Tag::BLOCK) {
            self.make_lambda_statement_body(ctx, body, lambda_method_decl, can_complete_normally)
        } else {
            self.make_lambda_expression_body(ctx, body, lambda_method_decl)
        }
    }

    /// Creates the body block of a lambda with an expression body (upstream
    /// makeLambdaExpressionBody).
    fn make_lambda_expression_body(&mut self, ctx: &mut CompilerContext, expr: TreeId, lambda_method_decl: TreeId) -> TreeId {
        let decl_type = ctx.tree(lambda_method_decl).ty.expect("lambda method type");
        let restype = ctx.typ(decl_type).get_return_type(ctx).expect("lambda return type");
        let is_lambda_void = ctx.tree(expr).ty.map(|ty| ctx.typ(ty).has_tag(TypeTag::VOID)).unwrap_or(false);
        let is_target_void = ctx.typ(restype).has_tag(TypeTag::VOID);
        let void_type = self.syms.borrow().void_type;
        let boxed_void = self.types.boxed_class(ctx, &mut self.syms.borrow_mut(), void_type);
        let boxed_void_ty = ctx.sym(boxed_void).ty.expect("boxed void type");
        let is_target_void_type = {
            let syms = self.syms.borrow();
            self.types.is_same_type(ctx, &syms, restype, boxed_void_ty)
        };
        let previous_pos = self.make.borrow().pos;
        let result = if is_target_void {
            let exec = {
                let mut make = self.make.borrow_mut();
                make.exec(ctx, expr)
            };
            let block = {
                let mut make = self.make.borrow_mut();
                make.block_pub(ctx, 0, List::of(exec))
            };
            block
        } else if is_lambda_void && is_target_void_type {
            let exec = {
                let mut make = self.make.borrow_mut();
                make.exec(ctx, expr)
            };
            let bot_type = self.syms.borrow().bot_type;
            let bot = {
                let mut make = self.make.borrow_mut();
                make.literal(ctx, TypeTag::BOT, None)
            };
            ctx.tree_mut(bot).ty = Some(bot_type);
            let return_ = {
                let mut make = self.make.borrow_mut();
                make.return_(ctx, Some(bot))
            };
            let block = {
                let mut make = self.make.borrow_mut();
                make.block_pub(ctx, 0, List::from_iterable(vec![exec, return_]))
            };
            block
        } else {
            let return_ = {
                let mut make = self.make.borrow_mut();
                make.return_(ctx, Some(expr))
            };
            let block = {
                let mut make = self.make.borrow_mut();
                make.block_pub(ctx, 0, List::of(return_))
            };
            block
        };
        {
            let mut make = self.make.borrow_mut();
            make.at(previous_pos);
        }
        result
    }

    /// Creates the body block of a lambda with a statement body (upstream
    /// makeLambdaStatementBody).
    fn make_lambda_statement_body(&mut self, ctx: &mut CompilerContext, block: TreeId, lambda_method_decl: TreeId, complete_normally: bool) -> TreeId {
        let decl_type = ctx.tree(lambda_method_decl).ty.expect("lambda method type");
        let restype = ctx.typ(decl_type).get_return_type(ctx).expect("lambda return type");
        let is_target_void = ctx.typ(restype).has_tag(TypeTag::VOID);
        let void_type = self.syms.borrow().void_type;
        let boxed_void = self.types.boxed_class(ctx, &mut self.syms.borrow_mut(), void_type);
        let boxed_void_ty = ctx.sym(boxed_void).ty.expect("boxed void type");
        let is_target_void_type = {
            let syms = self.syms.borrow();
            self.types.is_same_type(ctx, &syms, restype, boxed_void_ty)
        };
        let mut translator = LambdaBodyTranslator {
            is_target_void,
            lambda_method_decl,
            result: None,
            base: TreeTranslator::new(),
            names: Rc::clone(&self.names),
            make: Rc::clone(&self.make),
            syms: Rc::clone(&self.syms),
        };
        translator.translate(ctx, Some(block));
        let trans_block = translator.result.take().or(Some(block)).expect("translated block");
        if complete_normally && is_target_void_type {
            let bot_type = self.syms.borrow().bot_type;
            let bot = {
                let mut make = self.make.borrow_mut();
                make.literal(ctx, TypeTag::BOT, None)
            };
            ctx.tree_mut(bot).ty = Some(bot_type);
            let return_ = {
                let mut make = self.make.borrow_mut();
                make.return_(ctx, Some(bot))
            };
            if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(trans_block).kind {
                *slot = slot.append(return_);
            }
        }
        trans_block
    }

    /// Does this functional expression require serialization support?
    fn context_is_serializable(&self, ctx: &mut CompilerContext, context_tree: TreeId) -> bool {
        if self.force_serializable {
            return true;
        }
        let target = self
            .context_map
            .borrow()
            .get(&context_tree)
            .map(|context| match &context.kind {
                TranslationContextKind::Lambda(lambda) => lambda.target,
                TranslationContextKind::Reference(reference) => reference.target,
            })
            .expect("context target");
        let serializable_tsym = self.syms.borrow().serializable_type;
        let serializable_tsym = ctx.typ(serializable_tsym).tsym.expect("serializable type symbol");
        let syms = self.syms.borrow();
        self.types.as_super(ctx, &syms, target, serializable_tsym).is_some()
    }

    /// Deduplicates equivalent lambda bodies (upstream DedupedLambda lookup).
    fn dedup_lambda(&mut self, ctx: &mut CompilerContext, sym: SymbolId, body: TreeId) -> Option<SymbolId> {
        let params = match &ctx.sym(sym).kind_data {
            SymbolKind::MethodSymbol { params, .. } => params.clone(),
            _ => List::nil(),
        };
        let params_vec: Vec<SymbolId> = params.iter().collect();
        let hash = crate::comp::tree_hasher::TreeHasher::hash(ctx, Some(body), &params_vec);
        let existing = self
            .k_info
            .as_ref()
            .and_then(|info| {
                info.deduped_lambdas.iter().find(|candidate| {
                    let candidate_params = match &ctx.sym(candidate.symbol).kind_data {
                        SymbolKind::MethodSymbol { params, .. } => params.clone(),
                        _ => List::nil(),
                    };
                    let candidate_params_vec: Vec<SymbolId> = candidate_params.iter().collect();
                    let sym_ty = ctx.sym(sym).ty;
                    let other_ty = ctx.sym(candidate.symbol).ty;
                    if sym_ty.is_none() || other_ty.is_none() {
                        return false;
                    }
                    let sym_ty = sym_ty.expect("method type");
                    let other_ty = other_ty.expect("method type");
                    let syms = self.syms.borrow();
                    if !self.types.is_same_type(ctx, &syms, sym_ty, other_ty) {
                        return false;
                    }
                    drop(syms);
                    let mut differ = crate::comp::tree_differ::TreeDiffer::new(&params_vec, &candidate_params_vec);
                    differ.scan(ctx, Some(body), Some(candidate.tree))
                })
            })
            .map(|candidate| candidate.symbol);
        if let Some(existing) = existing {
            return Some(existing);
        }
        if let Some(info) = self.k_info.as_mut() {
            info.deduped_lambdas.push(DedupedLambda {
                symbol: sym,
                tree: body,
                hash_code: hash,
            });
        }
        None
    }
}

impl LambdaTranslationContext {
    /// Returns the translated symbol map of the context.
    pub fn symbol_map(&self, kind: LambdaSymbolKind) -> &Vec<(SymbolId, SymbolId)> {
        self.translated_symbols
            .get(&kind)
            .expect("translated symbol map")
    }

    /// Returns the mutable translated symbol map of the context.
    pub fn symbol_map_mut(&mut self, kind: LambdaSymbolKind) -> &mut Vec<(SymbolId, SymbolId)> {
        self.translated_symbols
            .get_mut(&kind)
            .expect("translated symbol map")
    }
}

impl LambdaAnalyzerPreprocessor {
    /// Does this reference need to be converted to a lambda? (upstream
    /// ReferenceTranslationContext.needsConversionToLambda).
    fn reference_needs_conversion(&mut self, ctx: &mut CompilerContext, tree: TreeId) -> bool {
        let (kind, mode, varargs, owner_accessible, sym) = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { kind, mode, varargs_element, owner_accessible, sym, .. } => (*kind, *mode, *varargs_element, *owner_accessible, *sym),
            _ => return false,
        };
        // Java: ReferenceTranslationContext.owner is the owner recorded for
        // this reference (the analyzer registers it before this call), not the
        // enclosing lambda context: references outside any lambda (for example
        // a field initializer) still decide on their own.
        let context_owner = self
            .context_map
            .borrow()
            .get(&tree)
            .map(|context| context.owner)
            .expect("context owner");
        let interface_intersection = self.interface_parameter_is_intersection_or_union_type(ctx, tree);
        let needs_varargs = varargs.is_some();
        let is_array_op = sym
            .map(|sym| ctx.sym(sym).owner == Some(self.syms.borrow().array_class))
            .unwrap_or(false);
        let is_private_in_other_class = !self.nestmate_lambdas
            && sym
                .map(|sym| {
                    ctx.sym(sym).flags_field & flags::PRIVATE as i64 != 0
                        && symbol::encl_class(ctx, sym)
                            .map(|encl| {
                                let a = ctx.sym(encl).ty.expect("enclosing class type");
                                let b = symbol::encl_class(ctx, context_owner)
                                    .and_then(|owner| ctx.sym(owner).ty)
                                    .expect("context class type");
                                let syms = self.syms.borrow();
                                let ea = self.types.erasure(ctx, &syms, a);
                                let eb = self.types.erasure(ctx, &syms, b);
                                !self.types.is_same_type(ctx, &syms, ea, eb)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let is_protected = sym
            .map(|sym| is_protected_in_super_class_of_enclosing_class_in_other_package(ctx, sym, context_owner))
            .unwrap_or(false);
        interface_intersection
            || matches!(kind, Some(crate::tree::jc_tree::ReferenceKind::SUPER))
            || needs_varargs
            || is_array_op
            || is_private_in_other_class
            || is_protected
            || !owner_accessible
            || (mode == crate::tree::jc_tree::ReferenceMode::NEW
                && kind != Some(crate::tree::jc_tree::ReferenceKind::ARRAY_CTOR)
                && sym
                    .map(|sym| {
                        let owner = ctx.sym(sym).owner.expect("reference owner");
                        crate::code::symbol::Symbol::is_directly_or_indirectly_local(ctx, owner)
                            || ctx.sym(owner).is_inner(ctx)
                    })
                    .unwrap_or(false))
    }
}

/// Converts a method reference which cannot be used directly into a lambda.
pub struct MemberReferenceToLambda {
    pub tree: TreeId,
    pub owner: Option<SymbolId>,
    pub args: Vec<TreeId>,
    pub params: Vec<TreeId>,
    pub receiver_expression: Option<TreeId>,
    pub make: Rc<RefCell<TreeMaker>>,
    pub names: Rc<Names>,
    pub syms: Rc<RefCell<Symtab>>,
    pub types: Rc<Types>,
    pub attr: Option<Rc<RefCell<Attr>>>,
    pub diags: Rc<crate::util::jc_diagnostic::Factory>,
    pub trans_types: Rc<RefCell<TransTypes>>,
    pub attr_env: Option<Rc<Env<AttrContext>>>,
}

impl MemberReferenceToLambda {
    /// Creates the conversion for one member reference.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tree: TreeId,
        make: Rc<RefCell<TreeMaker>>,
        names: Rc<Names>,
        syms: Rc<RefCell<Symtab>>,
        types: Rc<Types>,
        attr: Option<Rc<RefCell<Attr>>>,
        diags: Rc<crate::util::jc_diagnostic::Factory>,
        trans_types: Rc<RefCell<TransTypes>>,
        attr_env: Option<Rc<Env<AttrContext>>>,
    ) -> MemberReferenceToLambda {
        MemberReferenceToLambda {
            tree,
            owner: None,
            args: Vec::new(),
            params: Vec::new(),
            receiver_expression: None,
            make,
            names,
            syms,
            types,
            attr,
            diags,
            trans_types,
            attr_env,
        }
    }

    /// Converts the member reference into a lambda expression (upstream
    /// MemberReferenceToLambda.lambda).
    pub fn lambda(&mut self, ctx: &mut CompilerContext, owner: SymbolId) -> TreeId {
        self.owner = Some(owner);
        let previous_pos = self.make.borrow().pos;
        {
            let mut make = self.make.borrow_mut();
            make.at(ctx.tree(self.tree).pos);
        }
        let rcvr = self.add_parameters_return_receiver(ctx);
        let mode = match &ctx.tree(self.tree).kind {
            TreeKind::MemberReference { mode, .. } => *mode,
            _ => panic!("expected a member reference"),
        };
        let expr = if mode == crate::tree::jc_tree::ReferenceMode::INVOKE {
            self.expression_invoke(ctx, rcvr)
        } else {
            self.expression_new(ctx)
        };
        let params = List::from_iterable(self.params.clone());
        let lambda = {
            let mut make = self.make.borrow_mut();
            make.lambda(ctx, params, expr)
        };
        let source_target = match &ctx.tree(self.tree).kind {
            TreeKind::MemberReference { target, .. } => *target,
            _ => None,
        };
        let source_ty = ctx.tree(self.tree).ty;
        let source_pos = ctx.tree(self.tree).pos;
        if let TreeKind::Lambda { target, .. } = &mut ctx.tree_mut(lambda).kind {
            *target = source_target;
        }
        ctx.tree_mut(lambda).ty = source_ty;
        ctx.tree_mut(lambda).pos = source_pos;
        {
            let mut make = self.make.borrow_mut();
            make.at(previous_pos);
        }
        lambda
    }

    /// Generates the parameter list for the converted member reference
    /// (upstream addParametersReturnReceiver).
    fn add_parameters_return_receiver(&mut self, ctx: &mut CompilerContext) -> Option<SymbolId> {
        let (kind, varargs_element, target, sym) = match &ctx.tree(self.tree).kind {
            TreeKind::MemberReference { kind, varargs_element, target, sym, .. } => (*kind, *varargs_element, *target, *sym),
            _ => panic!("expected a member reference"),
        };
        let target = target.expect("member reference target");
        let sam_desc = self.find_descriptor_type(ctx, target).expect("descriptor type");
        let sam_ptypes = ctx.typ(sam_desc).get_parameter_types(ctx);
        let desc_type = self.find_descriptor_type(ctx, target).expect("descriptor type");
        let mut desc_ptypes = ctx.typ(desc_type).get_parameter_types(ctx);
        let mut sam_ptypes = sam_ptypes;
        let sym = sym.expect("reference symbol");
        let rcvr = match kind {
            Some(crate::tree::jc_tree::ReferenceKind::BOUND) => {
                let expr = match &ctx.tree(self.tree).kind {
                    TreeKind::MemberReference { expr: Some(expr), .. } => *expr,
                    _ => panic!("bound member reference without qualifier"),
                };
                let expr_ty = ctx.tree(expr).ty.expect("receiver type");
                let rcvr = self.add_parameter(ctx, "rec$", expr_ty, false);
                let attr = Rc::clone(self.attr.as_ref().expect("attr"));
                let checked = {
                    let mut attr = attr.borrow_mut();
                    let mut syms = self.syms.borrow_mut();
                    attr.make_null_check(ctx, &mut syms, expr)
                };
                self.receiver_expression = Some(checked);
                Some(rcvr)
            }
            Some(crate::tree::jc_tree::ReferenceKind::UNBOUND) => {
                let head = *sam_ptypes.head();
                let rcvr = self.add_parameter(ctx, "rec$", head, false);
                sam_ptypes = sam_ptypes.tail();
                desc_ptypes = desc_ptypes.tail();
                Some(rcvr)
            }
            _ => None,
        };
        let impl_ptypes = {
            let sym_ty = ctx.sym(sym).ty.expect("reference symbol type");
            ctx.typ(sym_ty).get_parameter_types(ctx)
        };
        if std::env::var_os("DSH_DEBUG_SAM").is_some() {
            eprintln!(
                "[MREF-PARAMS] kind={:?} target={} desc={} impl={} sam={} params={:?} args={:?}",
                kind,
                crate::comp::attr::type_display_of(ctx, target),
                crate::comp::attr::type_display_of(ctx, desc_type),
                impl_ptypes.iter().map(|t| crate::comp::attr::type_display_of(ctx,t)).collect::<Vec<_>>().join(","),
                sam_ptypes.iter().map(|t| crate::comp::attr::type_display_of(ctx,t)).collect::<Vec<_>>().join(","),
                self.params.iter().map(|p| (ctx.tree(*p).pos, ctx.tree(*p).tag)).collect::<Vec<_>>(),
                self.args.iter().map(|a| (ctx.tree(*a).pos, ctx.tree(*a).tag)).collect::<Vec<_>>()
            );
        }
        let impl_size = impl_ptypes.length();
        let sam_size = sam_ptypes.length();
        let last = if varargs_element.is_some() { impl_size - 1 } else { impl_size };
        let check_for_intersection = varargs_element.is_some() || impl_size == desc_ptypes.length();
        let mut impl_rest = impl_ptypes;
        let mut sam_rest = sam_ptypes;
        let mut desc_rest = desc_ptypes;
        let mut i = 0;
        while impl_rest.non_empty() && i < last {
            let mut parm_type = *impl_rest.head();
            if check_for_intersection {
                let desc_head = *desc_rest.head();
                if ctx.typ(desc_head).is_intersection() {
                    parm_type = *sam_rest.head();
                }
                if ctx.typ(desc_head).has_tag(TypeTag::TYPEVAR) {
                    if let Some(upper) = ctx.typ(desc_head).get_upper_bound() {
                        if ctx.typ(upper).is_intersection() {
                            parm_type = *sam_rest.head();
                        }
                    }
                }
            }
            self.add_parameter(ctx, format!("x${}", i).as_str(), parm_type, true);
            impl_rest = impl_rest.tail();
            sam_rest = sam_rest.tail();
            desc_rest = desc_rest.tail();
            i += 1;
        }
        let mut j = last;
        while j < sam_size {
            let parm_type = varargs_element.expect("varargs element");
            self.add_parameter(ctx, format!("xva${}", j).as_str(), parm_type, true);
            j += 1;
        }
        rcvr
    }

    /// Adapter: Types.findDescriptorType with the conversion services.
    fn find_descriptor_type(&self, ctx: &mut CompilerContext, target: TypeId) -> Option<TypeId> {
        let attr = self.attr.as_ref().expect("attr").borrow();
        let chk = Rc::clone(&attr.chk);
        let chk = chk;
        let mut syms = self.syms.borrow_mut();
        self.types.find_descriptor_type(ctx, &mut syms, &self.diags, &chk, target).ok()
    }

    /// Returns the receiver expression of the converted member reference.
    pub fn get_receiver_expression(&self) -> Option<TreeId> {
        self.receiver_expression
    }

    /// Adds one parameter to the converted lambda (upstream addParameter).
    fn add_parameter(&mut self, ctx: &mut CompilerContext, name: &str, ty: TypeId, gen_arg: bool) -> SymbolId {
        let owner = self.owner.expect("conversion owner");
        let tree_pos = ctx.tree(self.tree).pos;
        let name = self.names.from_string(&javac_strings::JavaString::from(name));
        let vsym = ctx.alloc_symbol(Symbol::new(
            Kind::VAR,
            flags::PARAMETER | flags::SYNTHETIC as i64,
            name,
            Some(ty),
            Some(owner),
            SymbolKind::VarSymbol {
                lazy_const_value: None,
                pos: tree_pos,
                adr: 0,
                data_kind: None,
                data: None,
            },
        ));
        let var_def = {
            let mut make = self.make.borrow_mut();
            let mut syms = self.syms.borrow_mut();
            make.var_def_symbol(ctx, &mut syms, vsym, None)
        };
        self.params.push(var_def);
        if gen_arg {
            let ident = {
                let mut make = self.make.borrow_mut();
                make.ident_symbol(ctx, vsym)
            };
            self.args.push(ident);
        }
        vsym
    }

    /// Builds the receiver expression of the converted reference (upstream
    /// makeReceiver).
    fn make_receiver(&mut self, ctx: &mut CompilerContext, rcvr: SymbolId) -> TreeId {
        let ident = {
            let mut make = self.make.borrow_mut();
            make.ident_symbol(ctx, rcvr)
        };
        let (sym, owner_accessible, expr) = match &ctx.tree(self.tree).kind {
            TreeKind::MemberReference { sym: Some(sym), owner_accessible, expr, .. } => (*sym, *owner_accessible, *expr),
            _ => panic!("expected a member reference"),
        };
        let owner = self.owner.expect("conversion owner");
        let prot_access =
            is_protected_in_super_class_of_enclosing_class_in_other_package(ctx, sym, owner);
        let mut rcvr_type = if owner_accessible && !prot_access {
            let encl = crate::code::symbol::encl_class(ctx, sym).expect("receiver enclosing class");
            ctx.sym(encl).ty.expect("receiver enclosing type")
        } else {
            ctx.tree(expr.expect("qualifier expression"))
                .ty
                .expect("qualifier type")
        };
        let array_class_type = {
            let syms = self.syms.borrow();
            ctx.sym(syms.array_class).ty
        };
        if Some(rcvr_type) == array_class_type {
            rcvr_type = ctx.tree(expr.expect("qualifier expression")).ty.expect("qualifier type");
        }
        let rcvr_ty = ctx.sym(rcvr).ty.expect("receiver type");
        let rcvr_tsym = ctx.typ(rcvr_ty).tsym;
        let target_tsym = ctx.typ(rcvr_type).tsym;
        let is_subclass = match (rcvr_tsym, target_tsym) {
            (Some(rcvr_tsym), Some(target_tsym)) => {
                let syms = self.syms.borrow();
                crate::code::symbol::is_sub_class(ctx, &syms, &self.types, rcvr_tsym, target_tsym)
            }
            _ => false,
        };
        if !is_subclass {
            let type_tree = {
                let mut make = self.make.borrow_mut();
                let mut syms = self.syms.borrow_mut();
                make.type_from_type(ctx, &mut syms, rcvr_type)
            };
            let cast = {
                let mut make = self.make.borrow_mut();
                make.type_cast(ctx, type_tree, ident)
            };
            ctx.tree_mut(cast).ty = Some(rcvr_type);
            return cast;
        }
        ident
    }

    /// Converts the reference arguments, inserting the casts required by type
    /// erasure (upstream convertArgs).
    fn convert_args(&mut self, ctx: &mut CompilerContext, meth: SymbolId, args: List<TreeId>, varargs_element: Option<TypeId>) -> List<TreeId> {
        assert!(ctx.sym(meth).kind == Kind::MTH, "AssertionError: method symbol expected");
        let formals = {
            let meth_type = ctx.sym(meth).ty.expect("method type");
            let erased = {
                let syms = self.syms.borrow();
                self.types.erasure(ctx, &syms, meth_type)
            };
            ctx.typ(erased).get_parameter_types(ctx)
        };
        if varargs_element.is_some() {
            assert!(ctx.sym(meth).flags_field & flags::VARARGS != 0, "AssertionError: varargs method expected");
        }
        let attr_env = self.attr_env.clone().expect("attrEnv");
        let mut trans_types = self.trans_types.borrow_mut();
        trans_types.translate_args_with_env(ctx, args, formals, varargs_element, &attr_env)
    }

    /// Sets the varargs element on the converted call (upstream
    /// setVarargsIfNeeded).
    fn set_varargs_if_needed(&mut self, ctx: &mut CompilerContext, tree: TreeId, varargs_element: Option<TypeId>) {
        let Some(varargs_element) = varargs_element else { return; };
        match ctx.tree(tree).tag {
            Tag::APPLY => {
                if let TreeKind::MethodInvocation { varargs_element: slot, .. } = &mut ctx.tree_mut(tree).kind {
                    *slot = Some(varargs_element);
                }
            }
            Tag::NEWCLASS => {
                if let TreeKind::NewClass { varargs_element: slot, .. } = &mut ctx.tree_mut(tree).kind {
                    *slot = Some(varargs_element);
                }
            }
            Tag::TYPECAST => {
                if let TreeKind::TypeCast { expr: Some(expr), .. } = &ctx.tree(tree).kind {
                    let expr = *expr;
                    self.set_varargs_if_needed(ctx, expr, Some(varargs_element));
                }
            }
            _ => panic!("AssertionError: varargs element"),
        }
    }

    /// Lambda body to use for an invoke (upstream expressionInvoke).
    fn expression_invoke(&mut self, ctx: &mut CompilerContext, rcvr: Option<SymbolId>) -> TreeId {
        let qualifier = match rcvr {
            Some(rcvr) => self.make_receiver(ctx, rcvr),
            None => match &ctx.tree(self.tree).kind {
                // Java: expressionInvoke uses tree.getQualifierExpression()
                // when there is no synthetic receiver (static, unbound and
                // super references). Keeping the original "super" qualifier is
                // what makes the converted lambda capture the enclosing
                // instance.
                TreeKind::MemberReference { expr: Some(expr), .. } => *expr,
                _ => {
                    let ref_sym = match &ctx.tree(self.tree).kind {
                        TreeKind::MemberReference { sym: Some(sym), .. } => *sym,
                        _ => panic!("expected reference symbol"),
                    };
                    let owner = ctx.sym(ref_sym).owner.expect("reference owner");
                    let mut syms = self.syms.borrow_mut();
                    self.make
                        .borrow_mut()
                        .qual_ident(ctx, &mut syms, owner)
                }
            },
        };
        let (sym, varargs) = match &ctx.tree(self.tree).kind {
            TreeKind::MemberReference { sym: Some(sym), varargs_element, .. } => (*sym, *varargs_element),
            _ => panic!("expected a member reference"),
        };
        let name = ctx.sym(sym).name.clone();
        let select = {
            let mut make = self.make.borrow_mut();
            make.select(ctx, qualifier, name)
        };
        if let TreeKind::FieldAccess { sym: slot, .. } = &mut ctx.tree_mut(select).kind {
            *slot = Some(sym);
        }
        let sym_ty = ctx.sym(sym).ty.expect("reference symbol type");
        let erased = {
            let syms = self.syms.borrow();
            self.types.erasure(ctx, &syms, sym_ty)
        };
        ctx.tree_mut(select).ty = Some(erased);
        let return_type = ctx.typ(erased).get_return_type(ctx);
        let args = self.convert_args(ctx, sym, List::from_iterable(self.args.clone()), varargs);
        let apply = {
            let mut make = self.make.borrow_mut();
            make.apply(ctx, List::nil(), select, args)
        };
        ctx.tree_mut(apply).ty = return_type;
        let referent_type = match &ctx.tree(self.tree).kind {
            TreeKind::MemberReference { referent_type: Some(referent_type), .. } => *referent_type,
            _ => panic!("expected a member reference referent type"),
        };
        let target = {
            let syms = self.syms.borrow();
            self.types.erasure(ctx, &syms, referent_type)
        };
        let coerce_target = ctx.typ(target).get_return_type(ctx).expect("referent return type");
        let attr_env = self.attr_env.clone().expect("attrEnv");
        let coerced = self.trans_types.borrow_mut().coerce(ctx, &attr_env, apply, coerce_target);
        self.set_varargs_if_needed(ctx, coerced, varargs);
        coerced
    }

    /// Lambda body to use for a new (upstream expressionNew).
    fn expression_new(&mut self, ctx: &mut CompilerContext) -> TreeId {
        let kind = match &ctx.tree(self.tree).kind {
            TreeKind::MemberReference { kind, .. } => *kind,
            _ => None,
        };
        if kind == Some(crate::tree::jc_tree::ReferenceKind::ARRAY_CTOR) {
            let qualifier = match &ctx.tree(self.tree).kind {
                TreeKind::MemberReference { expr: Some(expr), .. } => *expr,
                _ => panic!("array constructor without qualifier"),
            };
            let qualifier_ty = ctx.tree(qualifier).ty.expect("array qualifier type");
            let dim = match self.params.first() {
                Some(first) => {
                    let sym = match &ctx.tree(*first).kind {
                        TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                        _ => panic!("expected parameter declaration"),
                    };
                    let ident = {
                        let mut make = self.make.borrow_mut();
                        make.ident_symbol(ctx, sym)
                    };
                    ident
                }
                None => panic!("array constructor without parameter"),
            };
            let elem_type = {
                let syms = self.syms.borrow();
                self.types.elemtype(ctx, &syms, qualifier_ty)
            };
            let array_type = {
                let mut make = self.make.borrow_mut();
                let mut syms = self.syms.borrow_mut();
                make.type_from_type(ctx, &mut syms, elem_type)
            };
            let array = {
                let mut make = self.make.borrow_mut();
                make.new_array(ctx, Some(array_type), List::of(dim), None)
            };
            ctx.tree_mut(array).ty = Some(qualifier_ty);
            array
        } else {
            let qualifier = match &ctx.tree(self.tree).kind {
                TreeKind::MemberReference { expr: Some(expr), .. } => *expr,
                _ => panic!("constructor reference without qualifier"),
            };
            let qualifier_ty = ctx.tree(qualifier).ty.expect("constructor qualifier type");
            let (sym, varargs) = match &ctx.tree(self.tree).kind {
                TreeKind::MemberReference { sym: Some(sym), varargs_element, .. } => (*sym, *varargs_element),
                _ => panic!("expected a member reference"),
            };
            let converted_args = self.convert_args(ctx, sym, List::from_iterable(self.args.clone()), varargs);
            let clazz = {
                let mut make = self.make.borrow_mut();
                let mut syms = self.syms.borrow_mut();
                make.type_from_type(ctx, &mut syms, qualifier_ty)
            };
            let new_class = {
                let mut make = self.make.borrow_mut();
                make.new_class(ctx, None, List::nil(), clazz, converted_args, None)
            };
            if let TreeKind::NewClass { constructor: slot, .. } = &mut ctx.tree_mut(new_class).kind {
                *slot = Some(sym);
            }
            ctx.tree_mut(new_class).ty = Some(qualifier_ty);
            self.set_varargs_if_needed(ctx, new_class, varargs);
            new_class
        }
    }
}

impl LambdaBodyTranslator {
    pub fn translate_list(&mut self, ctx: &mut CompilerContext, trees: &List<TreeId>) -> List<TreeId> {
        let mut result: List<TreeId> = List::nil();
        for tree in trees.iter() {
            let translated = self.translate(ctx, Some(tree)).expect("translated tree");
            result = result.append(translated);
        }
        result
    }

    /// Do not recurse into inner classes (upstream visitClassDef).
    fn visit_class_def_body(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Do not recurse into nested lambdas (upstream visitLambda).
    fn visit_lambda_body(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }

    /// Converts a value-returning return of a void-target lambda body
    /// (upstream visitReturn).
    fn visit_return_body(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr } => *expr,
            _ => panic!("expected a return"),
        };
        let is_lambda_void = expr.is_none();
        if self.is_target_void && !is_lambda_void {
            let expr = expr.expect("return expression");
            let loc_name = self.names.from_string(&javac_strings::JavaString::from("$loc"));
            let owner = match &ctx.tree(self.lambda_method_decl).kind {
                TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
                _ => panic!("expected a lambda method declaration"),
            };
            let loc = make_synthetic_var(
                ctx,
                0,
                loc_name,
                ctx.tree(expr).ty.expect("return expression type"),
                owner,
            );
            let var_def = {
                let mut make = self.make.borrow_mut();
                let mut syms = self.syms.borrow_mut();
                make.var_def_symbol(ctx, &mut syms, loc, Some(expr))
            };
            let return_ = {
                let mut make = self.make.borrow_mut();
                make.return_(ctx, None)
            };
            let block = {
                let mut make = self.make.borrow_mut();
                make.block_pub(ctx, 0, List::from_iterable(vec![var_def, return_]))
            };
            self.result = Some(block);
        } else {
            self.result = Some(tree);
        }
    }
}

impl TreeVisitor for LambdaBodyTranslator {
    fn visit_top_level(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let defs = match &ctx.tree(tree).kind {
            TreeKind::CompilationUnit { defs, .. } => defs.clone(),
            _ => panic!("expected a compilation unit"),
        };
        let defs = self.translate_list(ctx, &defs);
        if let TreeKind::CompilationUnit { defs: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = defs;
        }
        self.result = Some(tree);
    }


    fn visit_package_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, pid) = match &ctx.tree(tree).kind {
            TreeKind::PackageDecl {
                annotations, pid, ..
            } => (annotations.clone(), *pid),
            _ => panic!("expected a package declaration"),
        };
        let annotations = self.translate_list(ctx, &annotations);
        let pid = self.translate(ctx, pid);
        if let TreeKind::PackageDecl {
            annotations: a,
            pid: p,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *p = pid;
        }
        self.result = Some(tree);
    }


    fn visit_import(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Import { qualid, .. } => *qualid,
            _ => panic!("expected an import"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Import { qualid: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn visit_class_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_class_def_body(ctx, tree);
    }

    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, restype, typarams, recvparam, params, thrown, body) = match &ctx.tree(tree).kind
        {
            TreeKind::MethodDecl {
                mods,
                restype,
                typarams,
                recvparam,
                params,
                thrown,
                body,
                ..
            } => (
                *mods,
                *restype,
                typarams.clone(),
                *recvparam,
                params.clone(),
                thrown.clone(),
                *body,
            ),
            _ => panic!("expected a method declaration"),
        };
        let mods = self.translate(ctx, mods);
        let restype = self.translate(ctx, restype);
        let typarams = self.translate_type_params(ctx, &typarams);
        let recvparam = self.translate(ctx, recvparam);
        let params = self.translate_var_defs(ctx, &params);
        let thrown = self.translate_list(ctx, &thrown);
        let body = self.translate(ctx, body);
        if let TreeKind::MethodDecl {
            mods: m,
            restype: r,
            typarams: tp,
            recvparam: rp,
            params: ps,
            thrown: th,
            body: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *r = restype;
            *tp = typarams;
            *rp = recvparam;
            *ps = params;
            *th = thrown;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, nameexpr, vartype, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                mods,
                nameexpr,
                vartype,
                init,
                ..
            } => (*mods, *nameexpr, *vartype, *init),
            _ => panic!("expected a variable declaration"),
        };
        let mods = self.translate(ctx, mods);
        let nameexpr = self.translate(ctx, nameexpr);
        let vartype = self.translate(ctx, vartype);
        let init = self.translate(ctx, init);
        if let TreeKind::VariableDecl {
            mods: m,
            nameexpr: ne,
            vartype: vt,
            init: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *ne = nameexpr;
            *vt = vartype;
            *i = init;
        }
        self.result = Some(tree);
    }


    fn visit_skip(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("expected a block"),
        };
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Block { stats: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = stats;
        }
        self.result = Some(tree);
    }


    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop { body, cond } => (*body, *cond),
            _ => panic!("expected a do loop"),
        };
        let body = self.translate(ctx, body);
        let cond = self.translate(ctx, cond);
        if let TreeKind::DoWhileLoop { body: b, cond: c } = &mut ctx.tree_mut(tree).kind {
            *b = body;
            *c = cond;
        }
        self.result = Some(tree);
    }


    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop { cond, body } => (*cond, *body),
            _ => panic!("expected a while loop"),
        };
        let cond = self.translate(ctx, cond);
        let body = self.translate(ctx, body);
        if let TreeKind::WhileLoop { cond: c, body: b } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body,
            } => (init.clone(), *cond, step.clone(), *body),
            _ => panic!("expected a for loop"),
        };
        let init = self.translate_list(ctx, &init);
        let cond = self.translate(ctx, cond);
        let step = self.translate_list(ctx, &step);
        let body = self.translate(ctx, body);
        if let TreeKind::ForLoop {
            init: i,
            cond: c,
            step: s,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = init;
            *c = cond;
            *s = step;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (var, expr, body) = match &ctx.tree(tree).kind {
            TreeKind::EnhancedForLoop { var, expr, body } => (*var, *expr, *body),
            _ => panic!("expected an enhanced for loop"),
        };
        let var = self.translate(ctx, var);
        let expr = self.translate(ctx, expr);
        let body = self.translate(ctx, body);
        if let TreeKind::EnhancedForLoop {
            var: v,
            expr: e,
            body: b,
        } = &mut ctx.tree_mut(tree).kind
        {
            *v = var;
            *e = expr;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement { body, .. } => *body,
            _ => panic!("expected a labelled statement"),
        };
        let body = self.translate(ctx, body);
        if let TreeKind::LabeledStatement { body: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = body;
        }
        self.result = Some(tree);
    }


    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::Switch {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn visit_case(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (labels, stats) = match &ctx.tree(tree).kind {
            TreeKind::Case { labels, stats, .. } => (labels.clone(), stats.clone()),
            _ => panic!("expected a case"),
        };
        let labels = self.translate_list(ctx, &labels);
        let stats = self.translate_list(ctx, &stats);
        if let TreeKind::Case {
            labels: l,
            stats: s,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *l = labels;
            *s = stats;
        }
        self.result = Some(tree);
    }


    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector, cases, ..
            } => (*selector, cases.clone()),
            _ => panic!("expected a switch expression"),
        };
        let selector = self.translate(ctx, selector);
        let cases = self.translate_cases(ctx, &cases);
        if let TreeKind::SwitchExpression {
            selector: s,
            cases: c,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = selector;
            *c = cases;
        }
        self.result = Some(tree);
    }


    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lock, body) = match &ctx.tree(tree).kind {
            TreeKind::Synchronized { lock, body } => (*lock, *body),
            _ => panic!("expected a synchronized statement"),
        };
        let lock = self.translate(ctx, lock);
        let body = self.translate(ctx, body);
        if let TreeKind::Synchronized { lock: l, body: b } = &mut ctx.tree_mut(tree).kind {
            *l = lock;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (resources, body, catchers, finalizer) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                resources,
                body,
                catchers,
                finalizer,
                ..
            } => (resources.clone(), *body, catchers.clone(), *finalizer),
            _ => panic!("expected a try statement"),
        };
        let resources = self.translate_list(ctx, &resources);
        let body = self.translate(ctx, body);
        let catchers = self.translate_catchers(ctx, &catchers);
        let finalizer = self.translate(ctx, finalizer);
        if let TreeKind::Try {
            resources: r,
            body: b,
            catchers: c,
            finalizer: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *r = resources;
            *b = body;
            *c = catchers;
            *f = finalizer;
        }
        self.result = Some(tree);
    }


    fn visit_catch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (param, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch { param, body } => (*param, *body),
            _ => panic!("expected a catch clause"),
        };
        let param = self.translate(ctx, param);
        let body = self.translate(ctx, body);
        if let TreeKind::Catch { param: p, body: b } = &mut ctx.tree_mut(tree).kind {
            *p = param;
            *b = body;
        }
        self.result = Some(tree);
    }


    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (*cond, *truepart, *falsepart),
            _ => panic!("expected a conditional"),
        };
        let cond = self.translate(ctx, cond);
        let truepart = self.translate(ctx, truepart);
        let falsepart = self.translate(ctx, falsepart);
        if let TreeKind::Conditional {
            cond: c,
            truepart: t,
            falsepart: f,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = truepart;
            *f = falsepart;
        }
        self.result = Some(tree);
    }


    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond,
                thenpart,
                elsepart,
            } => (*cond, *thenpart, *elsepart),
            _ => panic!("expected an if statement"),
        };
        let cond = self.translate(ctx, cond);
        let thenpart = self.translate(ctx, thenpart);
        let elsepart = self.translate(ctx, elsepart);
        if let TreeKind::If {
            cond: c,
            thenpart: t,
            elsepart: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = cond;
            *t = thenpart;
            *e = elsepart;
        }
        self.result = Some(tree);
    }


    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr } => *expr,
            _ => panic!("expected an expression statement"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::ExpressionStatement { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_break(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let value = match &ctx.tree(tree).kind {
            TreeKind::Yield { value, .. } => *value,
            _ => panic!("expected a yield"),
        };
        let value = self.translate(ctx, value);
        if let TreeKind::Yield { value: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = value;
        }
        self.result = Some(tree);
    }


    fn visit_continue(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_return_body(ctx, tree);
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw { expr } => *expr,
            _ => panic!("expected a throw"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Throw { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_assert(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, detail) = match &ctx.tree(tree).kind {
            TreeKind::Assert { cond, detail } => (*cond, *detail),
            _ => panic!("expected an assert"),
        };
        let cond = self.translate(ctx, cond);
        let detail = self.translate(ctx, detail);
        if let TreeKind::Assert { cond: c, detail: d } = &mut ctx.tree_mut(tree).kind {
            *c = cond;
            *d = detail;
        }
        self.result = Some(tree);
    }


    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation { meth, args, .. } => (*meth, args.clone()),
            _ => panic!("expected a method invocation"),
        };
        let meth = self.translate(ctx, meth);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::MethodInvocation {
            meth: m, args: a, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = meth;
            *a = args;
        }
        self.result = Some(tree);
    }


    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (encl, clazz, args, def) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                encl,
                clazz,
                args,
                def,
                ..
            } => (*encl, *clazz, args.clone(), *def),
            _ => panic!("expected a new class"),
        };
        let encl = self.translate(ctx, encl);
        let clazz = self.translate(ctx, clazz);
        let args = self.translate_list(ctx, &args);
        let def = self.translate(ctx, def);
        if let TreeKind::NewClass {
            encl: e,
            clazz: c,
            args: a,
            def: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = encl;
            *c = clazz;
            *a = args;
            *d = def;
        }
        self.result = Some(tree);
    }


    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, orig_dim_annotations_list, elemtype, dims, elems) =
            match &ctx.tree(tree).kind {
                TreeKind::NewArray {
                    annotations,
                    dim_annotations,
                    elemtype,
                    dims,
                    elems,
                } => (
                    annotations.clone(),
                    dim_annotations.clone(),
                    *elemtype,
                    dims.clone(),
                    elems.clone(),
                ),
                _ => panic!("expected a new array"),
            };
        let annotations = self.translate_annotations(ctx, &annotations);
        let mut dim_annotations = List::nil();
        for orig_dim_annotations in orig_dim_annotations_list.iter() {
            dim_annotations =
                dim_annotations.append(self.translate_annotations(ctx, &orig_dim_annotations));
        }
        let elemtype = self.translate(ctx, elemtype);
        let dims = self.translate_list(ctx, &dims);
        let elems = elems.map(|elems| self.translate_list(ctx, &elems));
        if let TreeKind::NewArray {
            annotations: a,
            dim_annotations: da,
            elemtype: et,
            dims: d,
            elems: e,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *da = dim_annotations;
            *et = elemtype;
            *d = dims;
            *e = elems;
        }
        self.result = Some(tree);
    }


    fn visit_lambda(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.visit_lambda_body(ctx, tree);
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr } => *expr,
            _ => panic!("expected parens"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::Parens { expr: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign { lhs, rhs } => (*lhs, *rhs),
            _ => panic!("expected an assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Assign { lhs: l, rhs: r } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a compound assignment"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::AssignOp { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg, .. } => *arg,
            _ => panic!("expected a unary expression"),
        };
        let arg = self.translate(ctx, arg);
        if let TreeKind::Unary { arg: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = arg;
        }
        self.result = Some(tree);
    }


    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => (*lhs, *rhs),
            _ => panic!("expected a binary expression"),
        };
        let lhs = self.translate(ctx, lhs);
        let rhs = self.translate(ctx, rhs);
        if let TreeKind::Binary { lhs: l, rhs: r, .. } = &mut ctx.tree_mut(tree).kind {
            *l = lhs;
            *r = rhs;
        }
        self.result = Some(tree);
    }


    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast { clazz, expr } => (*clazz, *expr),
            _ => panic!("expected a type cast"),
        };
        let clazz = self.translate(ctx, clazz);
        let expr = self.translate(ctx, expr);
        if let TreeKind::TypeCast { clazz: c, expr: e } = &mut ctx.tree_mut(tree).kind {
            *c = clazz;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf { expr, pattern } => (*expr, *pattern),
            _ => panic!("expected an instanceof"),
        };
        let expr = self.translate(ctx, expr);
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::InstanceOf {
            expr: e,
            pattern: p,
        } = &mut ctx.tree_mut(tree).kind
        {
            *e = expr;
            *p = pattern;
        }
        self.result = Some(tree);
    }


    fn visit_binding_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let var = match &ctx.tree(tree).kind {
            TreeKind::BindingPattern { var } => *var,
            _ => panic!("expected a binding pattern"),
        };
        let var = self.translate(ctx, var);
        if let TreeKind::BindingPattern { var: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = var;
        }
        self.result = Some(tree);
    }


    fn visit_default_case_label(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_parenthesized_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let pattern = match &ctx.tree(tree).kind {
            TreeKind::ParenthesizedPattern { pattern } => *pattern,
            _ => panic!("expected a parenthesized pattern"),
        };
        let pattern = self.translate(ctx, pattern);
        if let TreeKind::ParenthesizedPattern { pattern: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = pattern;
        }
        self.result = Some(tree);
    }


    fn visit_guard_pattern(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (patt, expr) = match &ctx.tree(tree).kind {
            TreeKind::GuardPattern { patt, expr } => (*patt, *expr),
            _ => panic!("expected a guard pattern"),
        };
        let patt = self.translate(ctx, patt);
        let expr = self.translate(ctx, expr);
        if let TreeKind::GuardPattern { patt: p, expr: e } = &mut ctx.tree_mut(tree).kind {
            *p = patt;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess { indexed, index } => (*indexed, *index),
            _ => panic!("expected an array access"),
        };
        let indexed = self.translate(ctx, indexed);
        let index = self.translate(ctx, index);
        if let TreeKind::ArrayAccess {
            indexed: i,
            index: n,
        } = &mut ctx.tree_mut(tree).kind
        {
            *i = indexed;
            *n = index;
        }
        self.result = Some(tree);
    }


    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let selected = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess { selected, .. } => *selected,
            _ => panic!("expected a field access"),
        };
        let selected = self.translate(ctx, selected);
        if let TreeKind::FieldAccess { selected: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = selected;
        }
        self.result = Some(tree);
    }


    fn visit_reference(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::MemberReference { expr, .. } => *expr,
            _ => panic!("expected a member reference"),
        };
        let expr = self.translate(ctx, expr);
        if let TreeKind::MemberReference { expr: slot, .. } = &mut ctx.tree_mut(tree).kind {
            *slot = expr;
        }
        self.result = Some(tree);
    }


    fn visit_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_literal(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_type_ident(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_type_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let elemtype = match &ctx.tree(tree).kind {
            TreeKind::ArrayTypeTree { elemtype } => *elemtype,
            _ => panic!("expected an array type"),
        };
        let elemtype = self.translate(ctx, elemtype);
        if let TreeKind::ArrayTypeTree { elemtype: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = elemtype;
        }
        self.result = Some(tree);
    }


    fn visit_type_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, arguments) = match &ctx.tree(tree).kind {
            TreeKind::TypeApply { clazz, arguments } => (*clazz, arguments.clone()),
            _ => panic!("expected a parameterized type"),
        };
        let clazz = self.translate(ctx, clazz);
        let arguments = self.translate_list(ctx, &arguments);
        if let TreeKind::TypeApply {
            clazz: c,
            arguments: a,
        } = &mut ctx.tree_mut(tree).kind
        {
            *c = clazz;
            *a = arguments;
        }
        self.result = Some(tree);
    }


    fn visit_type_union(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let alternatives = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives } => alternatives.clone(),
            _ => panic!("expected a union type"),
        };
        let alternatives = self.translate_list(ctx, &alternatives);
        if let TreeKind::TypeUnion { alternatives: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = alternatives;
        }
        self.result = Some(tree);
    }


    fn visit_type_intersection(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let bounds = match &ctx.tree(tree).kind {
            TreeKind::TypeIntersection { bounds } => bounds.clone(),
            _ => panic!("expected an intersection type"),
        };
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeIntersection { bounds: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = bounds;
        }
        self.result = Some(tree);
    }


    fn visit_type_parameter(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, bounds) = match &ctx.tree(tree).kind {
            TreeKind::TypeParameter {
                annotations,
                bounds,
                ..
            } => (annotations.clone(), bounds.clone()),
            _ => panic!("expected a type parameter"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let bounds = self.translate_list(ctx, &bounds);
        if let TreeKind::TypeParameter {
            annotations: a,
            bounds: b,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *b = bounds;
        }
        self.result = Some(tree);
    }


    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (kind, inner) = match &ctx.tree(tree).kind {
            TreeKind::Wildcard { kind, inner } => (*kind, *inner),
            _ => panic!("expected a wildcard"),
        };
        let kind = self.translate(ctx, kind);
        let inner = self.translate(ctx, inner);
        if let TreeKind::Wildcard { kind: k, inner: i } = &mut ctx.tree_mut(tree).kind {
            *k = kind;
            *i = inner;
        }
        self.result = Some(tree);
    }


    fn visit_type_bound_kind(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_modifiers(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let annotations = match &ctx.tree(tree).kind {
            TreeKind::Modifiers { annotations, .. } => annotations.clone(),
            _ => panic!("expected modifiers"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        if let TreeKind::Modifiers {
            annotations: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = annotations;
        }
        self.result = Some(tree);
    }


    fn visit_annotation(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotation_type, args) = match &ctx.tree(tree).kind {
            TreeKind::Annotation {
                annotation_type,
                args,
                ..
            } => (*annotation_type, args.clone()),
            _ => panic!("expected an annotation"),
        };
        let annotation_type = self.translate(ctx, annotation_type);
        let args = self.translate_list(ctx, &args);
        if let TreeKind::Annotation {
            annotation_type: a,
            args: ar,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotation_type;
            *ar = args;
        }
        self.result = Some(tree);
    }


    fn visit_annotated_type(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (annotations, underlying_type) = match &ctx.tree(tree).kind {
            TreeKind::AnnotatedType {
                annotations,
                underlying_type,
            } => (annotations.clone(), *underlying_type),
            _ => panic!("expected an annotated type"),
        };
        let annotations = self.translate_annotations(ctx, &annotations);
        let underlying_type = self.translate(ctx, underlying_type);
        if let TreeKind::AnnotatedType {
            annotations: a,
            underlying_type: u,
        } = &mut ctx.tree_mut(tree).kind
        {
            *a = annotations;
            *u = underlying_type;
        }
        self.result = Some(tree);
    }


    fn visit_erroneous(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        self.result = Some(tree);
    }


    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (defs, expr) = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, expr, .. } => (defs.clone(), *expr),
            _ => panic!("expected a let expression"),
        };
        let defs = self.translate_list(ctx, &defs);
        let expr = self.translate(ctx, expr);
        if let TreeKind::LetExpr {
            defs: d, expr: e, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *d = defs;
            *e = expr;
        }
        self.result = Some(tree);
    }


    fn visit_module_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (mods, qual_id, directives) = match &ctx.tree(tree).kind {
            TreeKind::ModuleDecl {
                mods,
                qual_id,
                directives,
                ..
            } => (*mods, *qual_id, directives.clone()),
            _ => panic!("expected a module declaration"),
        };
        let mods = self.translate(ctx, mods);
        let qual_id = self.translate(ctx, qual_id);
        let directives = directives.map(|directives| self.translate_list(ctx, &directives));
        if let TreeKind::ModuleDecl {
            mods: m,
            qual_id: q,
            directives: d,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *m = mods;
            *q = qual_id;
            *d = directives;
        }
        self.result = Some(tree);
    }


    fn visit_exports(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Exports {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an exports directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Exports {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn visit_opens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (qualid, module_names) = match &ctx.tree(tree).kind {
            TreeKind::Opens {
                qualid,
                module_names,
                ..
            } => (*qualid, module_names.clone()),
            _ => panic!("expected an opens directive"),
        };
        let qualid = self.translate(ctx, qualid);
        let module_names = module_names.map(|module_names| self.translate_list(ctx, &module_names));
        if let TreeKind::Opens {
            qualid: q,
            module_names: m,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *q = qualid;
            *m = module_names;
        }
        self.result = Some(tree);
    }


    fn visit_provides(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (service_name, impl_names) = match &ctx.tree(tree).kind {
            TreeKind::Provides {
                service_name,
                impl_names,
                ..
            } => (*service_name, impl_names.clone()),
            _ => panic!("expected a provides directive"),
        };
        let service_name = self.translate(ctx, service_name);
        let impl_names = self.translate_list(ctx, &impl_names);
        if let TreeKind::Provides {
            service_name: s,
            impl_names: i,
            ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *s = service_name;
            *i = impl_names;
        }
        self.result = Some(tree);
    }


    fn visit_requires(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let module_name = match &ctx.tree(tree).kind {
            TreeKind::Requires { module_name, .. } => *module_name,
            _ => panic!("expected a requires directive"),
        };
        let module_name = self.translate(ctx, module_name);
        if let TreeKind::Requires {
            module_name: slot, ..
        } = &mut ctx.tree_mut(tree).kind
        {
            *slot = module_name;
        }
        self.result = Some(tree);
    }


    fn visit_uses(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let qualid = match &ctx.tree(tree).kind {
            TreeKind::Uses { qualid } => *qualid,
            _ => panic!("expected a uses directive"),
        };
        let qualid = self.translate(ctx, qualid);
        if let TreeKind::Uses { qualid: slot } = &mut ctx.tree_mut(tree).kind {
            *slot = qualid;
        }
        self.result = Some(tree);
    }


    fn visit_tree(&mut self, _ctx: &mut CompilerContext, tree: TreeId) {
        panic!("AssertionError: visitTree {:?}", tree);
    }

}

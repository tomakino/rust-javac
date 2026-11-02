//! Bytecode generation.
//!
//! `Gen` lowers attributed and desugared trees to JVM instructions: it walks
//! statements and expressions, emits code through `Code`, and records line
//! numbers, local variable tables, and stack-map frames. Constant-pool
//! management and class-file serialization live in `ClassWriter` and
//! `PoolWriter`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::code::symtab::Symtab;
use crate::code::type_::{Type, TypeKindData};
use crate::code::type_tag::TypeTag;
use crate::code::types::Types;
use crate::comp::annotate::Annotate;
use crate::comp::attr::Attr;
use crate::comp::attr_context::AttrContext;
use crate::comp::check::Check;
use crate::comp::env::Env;
use crate::comp::infer::Infer;
use crate::comp::lower::Lower;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId, TypeId};
use crate::jvm::byte_codes::*;
use crate::jvm::class_file;
use crate::jvm::code::{Chain, Code};
use crate::jvm::cr_table::CRTable;
use crate::jvm::cr_table::CrtKey;
use crate::jvm::crt_flags::*;
use crate::jvm::items::{ImmediateValue, Item, Items};
use crate::jvm::pool_constant::LoadableConstantRef;
use crate::jvm::pool_writer::PoolWriter;
use crate::jvm::stack_map::StackMapFormat;
use crate::jvm::string_concat::StringConcat;
use crate::jvm::target::Target;
use crate::resources::compiler_properties::errors;
use crate::tree::end_pos_table::EndPosTable;
use crate::tree::jc_tree::{Tag, TreeKind, TreeVisitor};
use crate::tree::tree_info::{self, PosKind};
use crate::tree::tree_maker::{TreeFactory, TreeMaker};
use crate::tree::tree_scanner::TreeScannerVisitor;
use crate::util::options::Options;

use crate::util::jc_diagnostic::{DiagnosticPosition, Error};
use crate::util::list::List;
use crate::util::log::Log;
use crate::util::name::Name;
use crate::util::names::Names;
use crate::util::position::NOPOS;

/// An abstract finalizer generator for a code generation context.
pub trait GenFinalizer {
    /// Generate the cleanup code when unwinding.
    fn generate(&mut self, ctx: &mut CompilerContext, codegen: &mut Gen);

    /// Generate the cleanup code at last.
    fn generate_last(&mut self, ctx: &mut CompilerContext, codegen: &mut Gen);

    /// Whether this finalizer has nontrivial cleanup to perform.
    fn has_finalizer(&self) -> bool {
        true
    }

    /// Invoked after the try body has been visited.
    fn after_body(&mut self) {}
}

/// A code generation context.
#[derive(Clone, Default)]
pub struct GenContext {
    /// A chain of unresolved jumps exiting the current environment.
    pub exit: Option<Box<Chain>>,
    /// A chain of unresolved jumps continuing in the current environment.
    pub cont: Option<Box<Chain>>,
    /// The finalizer of the current environment.
    pub finalize: Option<Rc<RefCell<dyn GenFinalizer>>>,
    /// Whether this is a switch statement.
    pub is_switch: bool,
    /// The gaps in the finalizer range.
    pub gaps: Vec<i32>,
}

impl GenContext {
    /// Create an empty context.
    pub fn new() -> GenContext {
        GenContext::default()
    }

    /// Add a chain to the exit chain.
    pub fn add_exit(&mut self, chain: Option<Box<Chain>>) {
        self.exit = Code::merge_chains(chain, self.exit.take());
    }

    /// Add a chain to the continue chain.
    pub fn add_cont(&mut self, chain: Option<Box<Chain>>) {
        self.cont = Code::merge_chains(chain, self.cont.take());
    }
}

/// The code generation pass.
pub struct Gen {
    /// The diagnostic log.
    pub log: Rc<Log>,
    /// The symbol table.
    pub syms: Rc<Symtab>,
    /// The check service.
    pub chk: Rc<Check>,
    /// The resolver.
    pub rs: Rc<crate::comp::resolve::Resolve>,
    /// The tree factory.
    pub make: Rc<TreeMaker>,
    /// The name table.
    pub names: Rc<Names>,
    /// The code generation target.
    pub target: Target,
    /// The name "access$" used for access methods.
    pub access_dollar: Name,
    /// The type relations.
    pub types: Rc<Types>,
    /// The lower pass (for pruned trees).
    pub lower: Option<Rc<RefCell<Lower>>>,
    /// The annotation service.
    pub annotate: Rc<Annotate>,
    /// The attribution service.
    pub attr: Rc<RefCell<Attr>>,
    /// The type inference service.
    pub infer: Rc<Infer>,
    /// The string concatenation lowering.
    pub concat: Option<StringConcat>,
    /// The stack map format to generate.
    pub stack_map: StackMapFormat,
    /// A type standing for the expected type of method expressions.
    pub method_type: TypeId,
    /// The constant pool writer.
    pub pool_writer: Rc<RefCell<PoolWriter>>,
    /// Whether line debug info is generated.
    pub line_debug_info: bool,
    /// Whether variable debug info is generated.
    pub var_debug_info: bool,
    /// Whether the character range table is generated.
    pub gen_crt: bool,
    /// Whether bytecode debug output is generated.
    pub debug_code: bool,
    /// Whether virtualized private invoke is disabled.
    pub disable_virtualized_private_invoke: bool,
    /// The code buffer of the current method.
    pub code: Option<Rc<RefCell<Code>>>,
    /// The item factory of the current method.
    pub items: Option<Rc<Items>>,
    /// The code objects of the generated methods, indexed by symbol.
    pub method_codes: Rc<RefCell<HashMap<SymbolId, Rc<RefCell<Code>>>>>,

    /// The attribution environment of the current class.
    pub attr_env: Option<Rc<Env<AttrContext>>>,
    /// The top level tree.
    pub toplevel: Option<TreeId>,
    /// The number of code generation errors.
    pub nerrs: i32,
    /// The end position table of the current compilation unit.
    pub end_pos_table: Option<Rc<dyn EndPosTable>>,
    /// Whether a conditional switch expression is being generated.
    pub in_cond_switch_expression: bool,
    /// The chain of true jumps of the enclosing switch expression.
    pub switch_expression_true_chain: Option<Box<Chain>>,
    /// The chain of false jumps of the enclosing switch expression.
    pub switch_expression_false_chain: Option<Box<Chain>>,
    /// The stack items saved before a switch expression with try.
    pub stack_before_switch_expression: Vec<Item>,
    /// The temporary result item of the enclosing switch expression.
    pub switch_result: Option<Item>,
    /// The cache of qualifying symbols (Java: Map<Type, Symbol>; array keys
    /// follow Type.ArrayType.equals, Type.java:1360-1364).
    pub qualified_symbol_cache: Vec<(TypeId, SymbolId)>,
    /// The current environment.
    pub env: Option<Rc<Env<GenContext>>>,
    /// The expected type of the expression being visited.
    pub pt: Option<TypeId>,
    /// The item representing the computed value.
    pub result: Option<Item>,
}

impl Gen {
    /// Create the code generation pass.
    ///
    /// # Arguments
    ///   - log: the diagnostic log;
    ///   - syms: the symbol table;
    ///   - chk: the check service;
    ///   - rs: the resolver;
    ///   - make: the tree factory;
    ///   - names: the name table;
    ///   - target: the code generation target;
    ///   - types: the type relations;
    ///   - annotate: the annotation service;
    ///   - attr: the attribution service;
    ///   - infer: the type inference service;
    ///   - pool_writer: the constant pool writer;
    ///   - options: the compiler options.
    ///
    /// # Returns
    ///   - the initialized pass.
    ///
    /// # Panics
    ///   - none.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: &mut CompilerContext,
        log: Rc<Log>,
        syms: Rc<Symtab>,
        chk: Rc<Check>,
        rs: Rc<crate::comp::resolve::Resolve>,
        make: Rc<TreeMaker>,
        names: Rc<Names>,
        target: Target,
        types: Rc<Types>,
        annotate: Rc<Annotate>,
        attr: Rc<RefCell<Attr>>,
        infer: Rc<Infer>,
        pool_writer: Rc<RefCell<PoolWriter>>,
        lower: Rc<RefCell<Lower>>,
        options: &Options,
    ) -> Gen {
        let method_type = {
            let syms_b = &*syms;
            ctx.alloc_type(Type::new(
                TypeTag::METHOD,
                Some(syms_b.method_class),
                TypeKindData::MethodType {
                    argtypes: List::nil(),
                    restype: None,
                    thrown: List::nil(),
                    recvtype: None,
                },
            ))
        };
        let access_dollar = {
            let synthetic = target.synthetic_name_char();
            names.from_string(&crate::util::java_string::JavaString::from(
                format!("access{}", synthetic).as_str(),
            ))
        };
        // Java: Gen.java: lineDebugInfo = options.isUnset(G_CUSTOM) ||
        // options.isSet(G_CUSTOM, "lines"). Options keys are the primary names
        // ("-g:"), not the enum constant name.
        let g_custom_key = crate::main::option::Option::G_CUSTOM.primary_name();
        let line_debug_info = options.is_unset(g_custom_key)
            || options.is_set_choice(crate::main::option::Option::G_CUSTOM, "lines");
        let var_debug_info = if options.is_unset(g_custom_key) {
            options.is_set_option(crate::main::option::Option::G)
        } else {
            options.is_set_choice(crate::main::option::Option::G_CUSTOM, "vars")
        };
        let gen_crt = options.is_set("Xjcov");
        let debug_code = options.is_set("debug.code");
        let disable_virtualized_private_invoke = options.is_set("disableVirtualizedPrivateInvoke");
        // Java: concat = StringConcat.instance(context) (Gen constructor initialization)
        let concat = Some(crate::jvm::string_concat::make_concat(
            Rc::clone(&syms),
            Rc::clone(&names),
            Rc::clone(&make),
            Rc::clone(&types),
            options.get("stringConcat"),
            &target,
        ));
        Gen {
            log,
            syms,
            chk,
            rs,
            make,
            names,
            target,
            access_dollar,
            types,
            lower: Some(lower),
            annotate,
            attr,
            infer,
            concat,
            stack_map: StackMapFormat::JSR202,
            method_type,
            pool_writer,
            line_debug_info,
            var_debug_info,
            gen_crt,
            debug_code,
            disable_virtualized_private_invoke,
            code: None,
            items: None,
            method_codes: Rc::new(RefCell::new(HashMap::new())),
            attr_env: None,
            toplevel: None,
            nerrs: 0,
            end_pos_table: None,
            in_cond_switch_expression: false,
            switch_expression_true_chain: None,
            switch_expression_false_chain: None,
            stack_before_switch_expression: Vec::new(),
            switch_result: None,
            qualified_symbol_cache: Vec::new(),
            env: None,
            pt: None,
            result: None,
        }
    }

    /// Return the current code buffer.
    pub fn get_code(&self) -> Rc<RefCell<Code>> {
        Rc::clone(self.code.as_ref().expect("code buffer"))
    }

    /// Return the current item factory.
    pub fn get_items(&self) -> Rc<Items> {
        Rc::clone(self.items.as_ref().expect("item factory"))
    }

    /// Return the attribution environment of the current class.
    pub fn get_attr_env(&self) -> Rc<Env<AttrContext>> {
        Rc::clone(self.attr_env.as_ref().expect("attribution environment"))
    }
}

impl Gen {
    /// Generate code to load an integer constant.
    fn load_int_const(&mut self, ctx: &mut CompilerContext, n: i32) {
        let items = self.get_items();
        let item = items.make_immediate_item(ctx, self.syms.int_type, ImmediateValue::Int(n));
        items.load(ctx, &item);
    }

    /// Return the opcode that loads a zero constant of the given type code.
    ///
    /// # Arguments
    ///   - tc: the type code.
    ///
    /// # Returns
    ///   - the zero-loading opcode.
    ///
    /// # Panics
    ///   - the type code is not a numeric type code.
    pub fn zero(tc: i32) -> i32 {
        match tc {
            INT_CODE | BYTE_CODE | SHORT_CODE | CHAR_CODE => ICONST_0,
            LONG_CODE => LCONST_0,
            FLOAT_CODE => FCONST_0,
            DOUBLE_CODE => DCONST_0,
            _ => panic!("AssertionError: zero"),
        }
    }

    /// Return the opcode that loads a one constant of the given type code.
    ///
    /// # Arguments
    ///   - tc: the type code.
    ///
    /// # Returns
    ///   - the one-loading opcode.
    ///
    /// # Panics
    ///   - the type code is not a numeric type code.
    pub fn one(tc: i32) -> i32 {
        Gen::zero(tc) + 1
    }

    /// Generate code to load -1 of the given type code.
    fn emit_minus_one(&mut self, ctx: &mut CompilerContext, tc: i32) {
        if tc == LONG_CODE {
            let items = self.get_items();
            let item =
                items.make_immediate_item(ctx, self.syms.long_type, ImmediateValue::Long(-1));
            items.load(ctx, &item);
        } else {
            self.get_code().borrow_mut().emitop0(ctx, ICONST_M1);
        }
    }

    /// Construct a symbol reflecting the qualifying type that should appear in
    /// the bytecode as per JLS 13.1.
    fn binary_qualifier(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
        site: TypeId,
    ) -> SymbolId {
        if ctx.typ(site).has_tag(TypeTag::ARRAY) {
            let syms = &*self.syms;
            let is_length_var = sym == syms.length_var;
            let owner = ctx.sym(sym).owner;
            let is_array_owner = owner == Some(syms.array_class);
            let _ = syms;
            if is_length_var || !is_array_owner {
                return sym;
            }
            let qualifier = if let Some((_, cached)) = self
                .qualified_symbol_cache
                .iter()
                .find(|(cached_site, _)| array_types_equal(ctx, *cached_site, site))
            {
                *cached
            } else {
                let site_tsym = ctx.typ(site).tsym.expect("array type symbol");
                let site_name = ctx.sym(site_tsym).name.clone();
                let no_symbol = self.syms.no_symbol;
                let qualifier = ctx.alloc_symbol(crate::code::symbol::Symbol::new(
                    crate::code::kinds::Kind::TYP,
                    crate::code::flags::PUBLIC as i64,
                    site_name.clone(),
                    Some(site),
                    Some(no_symbol),
                    crate::code::symbol::SymbolKind::ClassSymbol {
                        members_field: None,
                        fullname: site_name.clone(),
                        flatname: site_name,
                        sourcefile: None,
                        classfile: None,
                        trans_local: List::nil(),
                        record_components: List::nil(),
                        permitted: List::nil(),
                        is_permitted_explicit: false,
                        annotation_type_metadata: Default::default(),
                    },
                ));
                self.qualified_symbol_cache.push((site, qualifier));
                qualifier
            };
            return clone_symbol_with_owner(ctx, sym, qualifier);
        }
        let site_tsym = ctx.typ(site).tsym.expect("site type symbol");
        let owner = ctx.sym(sym).owner;
        let object_tsym = ctx
            .typ(self.syms.object_type)
            .tsym
            .expect("object type symbol");
        let flags = ctx.sym(sym).flags_field;
        let is_static_synthetic = (flags
            & (crate::code::flags::STATIC as i64 | crate::code::flags::SYNTHETIC as i64))
            == (crate::code::flags::STATIC as i64 | crate::code::flags::SYNTHETIC as i64);
        if owner == Some(site_tsym) || is_static_synthetic {
            return sym;
        }
        // Leave alone methods inherited from Object (JLS 13.1).
        if owner == Some(object_tsym) {
            return sym;
        }
        clone_symbol_with_owner(ctx, sym, site_tsym)
    }

    /// Insert a reference to the given type in the constant pool and return its
    /// index.
    pub fn make_ref(&mut self, ctx: &mut CompilerContext, pos: i32, ty: TypeId) -> i32 {
        let checked = self.check_dimension(ctx, pos, ty);
        let syms = Rc::clone(&self.syms);
        let mut pool = self.pool_writer.borrow_mut();
        pool.put_class(ctx, &*syms, checked)
    }

    /// Check whether the given type is an array with too many dimensions.
    fn check_dimension(&mut self, ctx: &mut CompilerContext, pos: i32, t: TypeId) -> TypeId {
        self.check_dimension_internal(ctx, pos, t);
        t
    }

    /// Check the given type for invalid array dimensions.
    fn check_dimension_internal(&mut self, ctx: &mut CompilerContext, pos: i32, t: TypeId) {
        match ctx.typ(t).get_tag() {
            TypeTag::METHOD => {
                let ret = ctx.typ(t).get_return_type(ctx);
                if let Some(ret) = ret {
                    self.check_dimension_internal(ctx, pos, ret);
                }
                let params = ctx.typ(t).get_parameter_types(ctx);
                let rest = params;
                for head in &rest {
                    self.check_dimension_internal(ctx, pos, head);
                }
            }
            TypeTag::ARRAY => {
                let dims = self.types.dimensions(ctx, &*self.syms, t);
                if dims > class_file::MAX_DIMENSIONS {
                    crate::util::abstract_log::AbstractLog::error_at(
                        &*self.log,
                        pos,
                        Error::from(errors::limit_dimensions()),
                    );
                    self.nerrs += 1;
                }
            }
            _ => {}
        }
    }

    /// Create a temporary variable of the given type.
    fn make_temp(&mut self, ctx: &mut CompilerContext, ty: TypeId) -> Item {
        let env = self.env.clone().expect("generation environment");
        let encl_method = env.encl_method.expect("enclosing method");
        let owner = match &ctx.tree(encl_method).kind {
            TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("AssertionError: enclosing method symbol"),
        };
        let names = Rc::clone(&self.names);
        let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
            crate::code::flags::SYNTHETIC as i64,
            names.empty.clone(),
            ty,
            owner,
        ));
        let code = self.get_code();
        code.borrow_mut().new_local_symbol(ctx, sym);
        let items = self.get_items();
        items.make_local_item(ctx, sym)
    }

    /// Generate code to call a non-private method or constructor.
    pub fn call_method(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        site: TypeId,
        name: Name,
        argtypes: List<TypeId>,
        is_static: bool,
    ) {
        let env = self.get_attr_env();
        let msym = self.resolve_internal_method(ctx, pos, &env, site, name.clone(), argtypes);
        let items = self.get_items();
        if is_static {
            let item = items.make_static_item(ctx, msym);
            items.invoke(ctx, &item);
        } else {
            let is_init = name == self.names.init;
            let item = items.make_member_item(ctx, msym, is_init);
            items.invoke(ctx, &item);
        }
    }

    /// Resolve a method of a type.
    pub fn resolve_internal_method(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        env: &Rc<Env<AttrContext>>,
        site: TypeId,
        name: Name,
        argtypes: List<TypeId>,
    ) -> SymbolId {
        let diag: Option<Rc<dyn DiagnosticPosition>> = Some(Rc::new(
            crate::util::jc_diagnostic::SimpleDiagnosticPosition::new(pos),
        ));
        let types = Rc::clone(&self.types);
        let infer = Rc::clone(&self.infer);
        let syms = Rc::clone(&self.syms);
        let mut attr = self.attr.borrow_mut();
        let chk = Rc::clone(&self.chk);
        let syms_borrow = &*syms;
        self.rs.resolve_internal_method(
            ctx,
            &syms_borrow,
            &types,
            &mut attr,
            &chk,
            &infer,
            diag,
            env,
            site,
            name,
            argtypes,
            None,
        )
    }

    /// Return whether the given method definition is an access method resulting
    /// from a qualified super.
    fn is_access_super(&self, ctx: &CompilerContext, encl_method: TreeId) -> bool {
        let (mods, name) = match &ctx.tree(encl_method).kind {
            TreeKind::MethodDecl { mods, name, .. } => (*mods, name.clone()),
            _ => panic!("AssertionError: enclosing method expected"),
        };
        let flags = match mods {
            Some(mods) => match &ctx.tree(mods).kind {
                TreeKind::Modifiers { flags, .. } => *flags,
                _ => panic!("AssertionError: modifiers expected"),
            },
            None => 0,
        };
        (flags & crate::code::flags::SYNTHETIC as i64) != 0 && self.is_odd_access_name(ctx, &name)
    }

    /// Return whether the given name starts with "access$" and ends in an odd
    /// digit.
    fn is_odd_access_name(&self, ctx: &CompilerContext, name: &Name) -> bool {
        let _ = ctx;
        let len = name.get_byte_length();
        if len <= 0 {
            return false;
        }
        name.starts_with(&self.access_dollar) && (name.get_byte_at(len - 1) & 1) == 1
    }

    /// Generate code to invoke the finalizer associated with the given
    /// environment.
    fn gen_finalizer(&mut self, ctx: &mut CompilerContext, env: &Rc<Env<GenContext>>) {
        if self.get_code().borrow().is_alive() {
            let finalizer = env.info.borrow().finalize.clone();
            if let Some(finalizer) = finalizer {
                finalizer.borrow_mut().generate(ctx, self);
            }
        }
    }

    /// Generate code to call all finalizers of structures aborted by a non-local
    /// exit and return the target environment of the exit.
    fn unwind(
        &mut self,
        ctx: &mut CompilerContext,
        target: TreeId,
        env: &Rc<Env<GenContext>>,
    ) -> Rc<Env<GenContext>> {
        let mut env1 = Rc::clone(env);
        loop {
            self.gen_finalizer(ctx, &env1);
            if env1.tree == Some(target) {
                break;
            }
            env1 = env1.next.clone().expect("unwind environment");
        }
        env1
    }

    /// Mark the end of a gap in the catch-all range for the finalizer.
    fn end_finalizer_gap(&mut self, ctx: &mut CompilerContext, env: &Rc<Env<GenContext>>) {
        let mut info = env.info.borrow_mut();
        if !info.gaps.is_empty() && info.gaps.len() % 2 == 1 {
            let cp = self.get_code().borrow_mut().cur_cp(ctx);
            info.gaps.push(cp);
        }
    }

    /// Mark the end of all gaps between two environments.
    fn end_finalizer_gaps(
        &mut self,
        ctx: &mut CompilerContext,
        from: &Rc<Env<GenContext>>,
        to: &Rc<Env<GenContext>>,
    ) {
        let mut from = Rc::clone(from);
        loop {
            self.end_finalizer_gap(ctx, &from);
            if Rc::ptr_eq(&from, to) {
                break;
            }
            from = from.next.clone().expect("finalizer gap environment");
        }
    }

    /// Return whether any structure aborted by a non-local exit has a finalizer
    /// requiring an empty stack.
    fn has_finally(
        &self,
        ctx: &CompilerContext,
        target: TreeId,
        env: &Rc<Env<GenContext>>,
    ) -> bool {
        let mut env1 = Rc::clone(env);
        while env1.tree != Some(target) {
            let tree = env1.tree.expect("generation environment tree");
            let is_try = ctx.tree(tree).has_tag(Tag::TRY);
            if is_try {
                let finalizer = env1.info.borrow().finalize.clone();
                if let Some(finalizer) = finalizer
                    && finalizer.borrow().has_finalizer()
                {
                    return true;
                }
            }
            env1 = env1.next.clone().expect("finally environment");
        }
        false
    }
}

/// Java: Type.ArrayType.equals (Type.java:1360-1364): array types compare by
/// their element types recursively; all other Type equals is identity.
fn array_types_equal(ctx: &CompilerContext, a: TypeId, b: TypeId) -> bool {
    if a == b {
        return true;
    }
    let elem = |t: TypeId| match &ctx.typ(t).kind {
        TypeKindData::ArrayType { elemtype, .. } => *elemtype,
        _ => None,
    };
    match (elem(a), elem(b)) {
        (Some(x), Some(y)) => array_types_equal(ctx, x, y),
        _ => false,
    }
}

/// Clone a symbol with the given owner.
fn clone_symbol_with_owner(ctx: &mut CompilerContext, sym: SymbolId, owner: SymbolId) -> SymbolId {
    let mut cloned = ctx.sym(sym).clone();
    cloned.owner = Some(owner);
    // Java: Symbol.clone(Symbol) overrides baseSymbol() (Symbol.java:
    // 1676-1693/1928-1942), which in turn defines poolKey as
    // Pair(newOwner, baseSymbol()).
    // Java: Symbol.clone(Symbol) returns an anonymous subclass whose
    // baseSymbol() returns the cloned symbol itself (Symbol.java:1928-1942);
    // poolKey is then Pair(newOwner, baseSymbol()).
    cloned.base_symbol_override = Some(sym);
    ctx.alloc_symbol(cloned)
}

impl Gen {
    /// Return a duplicated generation environment whose next link is the
    /// current environment.
    fn dup_env(&self, tree: Option<TreeId>, info: GenContext) -> Rc<Env<GenContext>> {
        let base = self.env.clone().expect("generation environment");
        Env::dupto(
            &base,
            Env {
                next: None,
                outer: base.outer.clone(),
                tree,
                toplevel: base.toplevel,
                encl_class: base.encl_class,
                encl_method: base.encl_method,
                info: Rc::new(RefCell::new(info)),
                base_clause: base.base_clause,
            },
        )
    }

    /// Load an item through the item factory.
    fn item_load(&mut self, ctx: &mut CompilerContext, item: &Item) -> Item {
        let items = self.get_items();
        items.load(ctx, item)
    }

    /// Store the top of stack into an item.
    fn item_store(&mut self, ctx: &mut CompilerContext, item: &Item) {
        let items = self.get_items();
        items.store(ctx, item);
    }

    /// Duplicate an item.
    fn item_duplicate(&mut self, ctx: &mut CompilerContext, item: &Item) {
        let items = self.get_items();
        items.duplicate(ctx, item);
    }

    /// Drop an item.
    fn item_drop(&mut self, ctx: &mut CompilerContext, item: &Item) {
        let items = self.get_items();
        items.drop_item(ctx, item);
    }

    /// Invoke the method represented by an item.
    fn item_invoke(&mut self, ctx: &mut CompilerContext, item: &Item) -> Item {
        let items = self.get_items();
        items.invoke(ctx, item)
    }

    /// Coerce an item to a type.
    fn item_coerce_type(&mut self, ctx: &mut CompilerContext, item: &Item, ty: TypeId) -> Item {
        let items = self.get_items();
        items.coerce_type(ctx, item, ty)
    }
}

impl Gen {
    /// Distribute member initializer code into constructors and the static
    /// initializer method.
    fn normalize_defs(
        &mut self,
        ctx: &mut CompilerContext,
        defs: List<TreeId>,
        c: SymbolId,
    ) -> List<TreeId> {
        let mut init_code: Vec<TreeId> = Vec::new();
        let mut init_tas: Vec<crate::ids::AttributeId> = Vec::new();
        let mut clinit_code: Vec<TreeId> = Vec::new();
        let mut clinit_tas: Vec<crate::ids::AttributeId> = Vec::new();
        let mut method_defs: Vec<TreeId> = Vec::new();
        let l = defs;
        for def in &l {
            match &ctx.tree(def).kind {
                TreeKind::Block { flags, .. } => {
                    if (flags & crate::code::flags::STATIC as i64) != 0 {
                        clinit_code.push(def);
                    } else if (flags & crate::code::flags::SYNTHETIC as i64) == 0 {
                        init_code.push(def);
                    }
                }
                TreeKind::MethodDecl { .. } => {
                    method_defs.push(def);
                }
                TreeKind::VariableDecl {
                    sym: Some(sym),
                    init: Some(init),
                    ..
                } => {
                    let sym = *sym;
                    let init = *init;
                    let pos = ctx.tree(def).pos;
                    let sym_ty = ctx.sym(sym).ty.expect("variable type");
                    self.check_dimension(ctx, pos, sym_ty);
                    let flags = ctx.sym(sym).flags_field;
                    let is_static = (flags & crate::code::flags::STATIC as i64) != 0;
                    if !is_static {
                        // Always initialize instance variables.
                        let make = Rc::clone(&self.make);
                        let stmt = make.at(pos).assignment(ctx, sym, init);
                        init_code.push(stmt);
                        self.end_pos_table_replaced(ctx, def, stmt);
                        let tas = self.get_and_remove_non_field_tas(ctx, sym);
                        init_tas.extend(tas);
                    } else {
                        let const_value = ctx.sym(sym).get_const_value();
                        match const_value {
                            None => {
                                let make = Rc::clone(&self.make);
                                // Java: Gen.normalizeDefs positions the
                                // initializer at vdef.pos() (the field
                                // declaration position).
                                let stmt = make.at(pos).assignment(ctx, sym, init);
                                clinit_code.push(stmt);
                                self.end_pos_table_replaced(ctx, def, stmt);
                                let tas = self.get_and_remove_non_field_tas(ctx, sym);
                                clinit_tas.extend(tas);
                            }
                            Some(value) => {
                                let init_pos = ctx.tree(init).pos;
                                self.check_string_constant(ctx, init_pos, Some(&value));
                                // If the init references an external class, add
                                // it to the constant pool.
                                self.class_reference_scan(ctx, init);
                            }
                        }
                    }
                }
                TreeKind::VariableDecl {
                    sym: Some(sym),
                    init: None,
                    ..
                } => {
                    let pos = ctx.tree(def).pos;
                    let sym_ty = ctx.sym(*sym).ty.expect("variable type");
                    self.check_dimension(ctx, pos, sym_ty);
                }
                // Upstream Gen.normalizeDefs (JDK17 Gen.java:421-476) partitions
                // the class defs into BLOCK/METHODDEF/VARDEF only and asserts on any
                // other tag, so a nested JCClassDecl never reaches the genClass defs
                // loop (Gen.java:2419-2421); each nested class is generated by its
                // own genClass invocation. Keep that invariant: ignore nested class
                // declarations here, and keep panicking on any other unexpected tag.
                TreeKind::ClassDecl { .. } => {}
                _ => panic!("AssertionError: unexpected class member"),
            }
        }
        // Insert any instance initializers into all constructors.
        if !init_code.is_empty() {
            let inits: List<TreeId> = List::from_iterable(init_code.clone());
            let cinits = self.init_type_attributes(ctx, c);
            let mut cinits_rest = cinits;
            while cinits_rest.non_empty() {
                init_tas.push(*cinits_rest.head());
                cinits_rest = cinits_rest.tail();
            }
            let init_talist: List<crate::ids::AttributeId> = List::from_iterable(init_tas);
            for t in method_defs.clone() {
                self.normalize_method(ctx, t, inits.clone(), init_talist.clone());
            }
        }
        // If there are class initializers, create a <clinit> method.
        if !clinit_code.is_empty() {
            let syms = Rc::clone(&self.syms);
            let syms_b = &*syms;
            let clinit_ty = ctx.alloc_type(Type::new(
                TypeTag::METHOD,
                Some(syms_b.method_class),
                TypeKindData::MethodType {
                    argtypes: List::nil(),
                    restype: Some(syms_b.void_type),
                    thrown: List::nil(),
                    recvtype: None,
                },
            ));
            let c_flags = ctx.sym(c).flags_field;
            let flags =
                crate::code::flags::STATIC as i64 | (c_flags & crate::code::flags::STRICTFP as i64);
            let names = Rc::clone(&self.names);
            let clinit = ctx.alloc_symbol(crate::code::symbol::Symbol::new_method_symbol(
                flags,
                names.clinit.clone(),
                clinit_ty,
                c,
            ));
            let _ = syms_b;
            if let Some(members) = ctx.sym(c).members() {
                ctx.scope_enter(members, clinit);
            }
            let clinit_stats: List<TreeId> = List::from_iterable(clinit_code.clone());
            let head_pos = ctx.tree(*clinit_stats.head()).pos;
            let make = Rc::clone(&self.make);
            let block = make.at(head_pos).block(ctx, 0, clinit_stats.clone());
            // Java: Gen.java:496-500 sets block.endpos to
            // TreeInfo.endPos(clinitStats.last()) (the closing brace of the
            // user static initializer), not its start position.
            let last_pos = tree_info::end_pos(ctx, clinit_stats.last().expect("clinit last"));
            if let TreeKind::Block { endpos, .. } = &mut ctx.tree_mut(block).kind {
                *endpos = last_pos;
            }
            let method = {
                let make = Rc::clone(&self.make);
                let make = &*make;
                let mods = make.modifiers(ctx, flags);
                let names = Rc::clone(&self.names);
                let method = make.method_def_simple(
                    ctx,
                    mods,
                    names.clinit.clone(),
                    None,
                    List::nil(),
                    List::nil(),
                    List::nil(),
                    Some(block),
                    None,
                );
                if let TreeKind::MethodDecl { sym: slot, .. } = &mut ctx.tree_mut(method).kind {
                    *slot = Some(clinit);
                }
                method
            };
            method_defs.push(method);
            if !clinit_tas.is_empty() {
                let list: List<crate::ids::AttributeId> = List::from_iterable(clinit_tas);
                let mut sym = ctx.sym(clinit).clone();
                sym.append_unique_type_attributes(list);
                *ctx.sym_mut(clinit) = sym;
            }
            let class_tas = self.class_init_type_attributes(ctx, c);
            if !class_tas.is_empty() {
                let mut sym = ctx.sym(clinit).clone();
                sym.append_unique_type_attributes(class_tas);
                *ctx.sym_mut(clinit) = sym;
            }
        }
        List::from_iterable(method_defs)
    }

    /// Record the end position of a replaced tree in the end position table.
    ///
    /// The port stores the end position table behind a read-only handle, so the
    /// replacement is not recorded (registered as an adapter deviation).
    fn end_pos_table_replaced(
        &mut self,
        _ctx: &mut CompilerContext,
        _old_tree: TreeId,
        _new_tree: TreeId,
    ) {
    }

    /// Return the init type attributes of a class symbol.
    fn init_type_attributes(
        &self,
        ctx: &CompilerContext,
        c: SymbolId,
    ) -> List<crate::ids::AttributeId> {
        match &ctx.sym(c).metadata {
            Some(metadata) => metadata.get_init_type_attributes(),
            None => List::nil(),
        }
    }

    /// Return the class init type attributes of a class symbol.
    fn class_init_type_attributes(
        &self,
        ctx: &CompilerContext,
        c: SymbolId,
    ) -> List<crate::ids::AttributeId> {
        match &ctx.sym(c).metadata {
            Some(metadata) => metadata.get_class_init_type_attributes(),
            None => List::nil(),
        }
    }

    /// Remove the non-field type attributes from a variable symbol and return
    /// them.
    fn get_and_remove_non_field_tas(
        &mut self,
        ctx: &mut CompilerContext,
        sym: SymbolId,
    ) -> Vec<crate::ids::AttributeId> {
        let tas = ctx.sym(sym).get_raw_type_attributes();
        let mut field_tas: Vec<crate::ids::AttributeId> = Vec::new();
        let mut nonfield_tas: Vec<crate::ids::AttributeId> = Vec::new();
        let current = tas;
        for ta in &current {
            let pos = ctx
                .attr(ta)
                .get_position(ctx)
                .expect("AssertionError: type attribute position");
            assert_ne!(pos.type_, crate::code::target_type::TargetType::UNKNOWN, "AssertionError: unknown type attribute position");
            if pos.type_ == crate::code::target_type::TargetType::FIELD {
                field_tas.push(ta);
            } else {
                nonfield_tas.push(ta);
            }
        }
        let mut symbol = ctx.sym(sym).clone();
        // The arena port: a clone does not copy the symbol id, which
        // setTypeAttributes needs for the symbol metadata; restore it first.
        symbol.id.set(Some(sym));
        symbol.set_type_attributes(List::from_iterable(field_tas));
        *ctx.sym_mut(sym) = symbol;
        nonfield_tas
    }

    /// Check a constant value and report if it is a string that is too large.
    fn check_string_constant(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        const_value: Option<&crate::util::constants::ConstantValue>,
    ) {
        if self.nerrs != 0 {
            return;
        }
        let Some(value) = const_value else {
            return;
        };
        let crate::util::constants::ConstantValue::String(text) = value else {
            return;
        };
        if text.as_units().len() < 0xFFFF {
            return;
        }
        crate::util::abstract_log::AbstractLog::error_at(
            &*self.log,
            pos,
            Error::from(errors::limit_string()),
        );
        self.nerrs += 1;
        let _ = ctx;
    }

    /// Insert instance initializer code into the initial constructor.
    fn normalize_method(
        &mut self,
        ctx: &mut CompilerContext,
        md: TreeId,
        init_code: List<TreeId>,
        init_tas: List<crate::ids::AttributeId>,
    ) {
        let (name, sym, body) = match &ctx.tree(md).kind {
            TreeKind::MethodDecl {
                name,
                sym: Some(sym),
                body,
                ..
            } => (name.clone(), *sym, *body),
            _ => panic!("AssertionError: method declaration expected"),
        };
        let names = Rc::clone(&self.names);
        if name != names.init || !tree_info::is_initial_constructor(ctx, md) {
            return;
        }
        let body = body.expect("constructor body");
        let stats = match &ctx.tree(body).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("AssertionError: constructor block expected"),
        };
        let mut newstats: Vec<TreeId> = Vec::new();
        let mut stats = stats;
        if stats.non_empty() {
            // Copy initializers of synthetic variables generated in the
            // translation of inner classes.
            while stats.non_empty() && tree_info::is_synthetic_init(ctx, *stats.head()) {
                newstats.push(*stats.head());
                stats = stats.tail();
            }
            // Copy the superclass constructor call.
            newstats.push(*stats.head());
            stats = stats.tail();
            // Copy the remaining synthetic initializers.
            while stats.non_empty() && tree_info::is_synthetic_init(ctx, *stats.head()) {
                newstats.push(*stats.head());
                stats = stats.tail();
            }
            // Insert the initializer code.
            let mut rest = init_code.clone();
            while rest.non_empty() {
                newstats.push(*rest.head());
                rest = rest.tail();
            }
            // Copy the remaining statements.
            while stats.non_empty() {
                newstats.push(*stats.head());
                stats = stats.tail();
            }
        }
        let new_stats_list = List::from_iterable(newstats);
        let endpos = match &ctx.tree(body).kind {
            TreeKind::Block { endpos, .. } => *endpos,
            _ => NOPOS,
        };
        if endpos == NOPOS {
            let last = ctx.tree(new_stats_list.last().expect("last statement")).pos;
            if let TreeKind::Block { endpos, stats, .. } = &mut ctx.tree_mut(body).kind {
                *stats = new_stats_list;
                *endpos = last;
            }
        } else if let TreeKind::Block { stats, .. } = &mut ctx.tree_mut(body).kind {
            *stats = new_stats_list;
        }
        // Java: sym.appendUniqueTypeAttributes(initTAs); — update the symbol in
        // place (Symbol::clone deliberately drops the arena id).
        ctx.sym_mut(sym).append_unique_type_attributes(init_tas);
    }

    /// Scan a tree for class references to be added to the constant pool.
    fn class_reference_scan(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut visitor =
            ClassReferenceVisitor::new(Rc::clone(&self.pool_writer), Rc::clone(&self.syms));
        crate::tree::jc_tree::accept(ctx, tree, &mut visitor);
    }
}

impl Gen {
    /// Generate code for a definition, reporting completion failures.
    fn gen_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let prev_env = self.env.clone();
        crate::tree::jc_tree::accept(ctx, tree, self);
        self.env = prev_env;
    }

    /// Generate code for a statement with character range table flags.
    fn gen_stat_crt(&mut self, ctx: &mut CompilerContext, tree: TreeId, crt_flags: i32) {
        if !self.gen_crt {
            self.gen_stat(ctx, tree);
            return;
        }
        let start_pc = self.get_code().borrow_mut().cur_cp(ctx);
        self.gen_stat(ctx, tree);
        let mut flags = crt_flags;
        if ctx.tree(tree).has_tag(Tag::BLOCK) {
            flags |= CRT_BLOCK;
        }
        let end_pc = self.get_code().borrow_mut().cur_cp(ctx);
        if let Some(crt) = &mut self.get_code().borrow_mut().crt {
            crt.put(CrtKey::Tree(tree), flags, start_pc, end_pc);
        }
    }

    /// Generate code for a statement.
    fn gen_stat(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        if self.get_code().borrow().is_alive() {
            self.get_code().borrow_mut().stat_begin(ctx.tree(tree).pos);
            self.gen_def(ctx, tree);
        } else {
            let env = self.env.clone().expect("generation environment");
            let is_switch = env.info.borrow().is_switch;
            if is_switch && ctx.tree(tree).has_tag(Tag::VARDEF) {
                // Variables declared in a switch can be used even when the
                // declaration is unreachable.
                if let TreeKind::VariableDecl { sym: Some(sym), .. } = &ctx.tree(tree).kind {
                    let sym = *sym;
                    self.get_code().borrow_mut().new_local_symbol(ctx, sym);
                }
            }
        }
    }

    /// Generate code for a statement list with character range table flags.
    fn gen_stats_crt(&mut self, ctx: &mut CompilerContext, trees: List<TreeId>, crt_flags: i32) {
        if !self.gen_crt {
            self.gen_stats(ctx, trees);
            return;
        }
        if trees.length() == 1 {
            self.gen_stat_crt(ctx, *trees.head(), crt_flags | CRT_STATEMENT);
        } else {
            let trees_for_crt = trees.clone();
            let start_pc = self.get_code().borrow_mut().cur_cp(ctx);
            self.gen_stats(ctx, trees);
            let end_pc = self.get_code().borrow_mut().cur_cp(ctx);
            if let Some(crt) = &mut self.get_code().borrow_mut().crt {
                crt.put(
                    CrtKey::List(trees_for_crt.iter().collect()),
                    crt_flags,
                    start_pc,
                    end_pc,
                );
            }
        }
    }

    /// Generate code for a statement list.
    fn gen_stats(&mut self, ctx: &mut CompilerContext, trees: List<TreeId>) {
        let l = trees;
        for head in &l {
            self.gen_stat_crt(ctx, head, CRT_STATEMENT);
        }
    }

    /// Generate code for a boolean expression in a control-flow context with
    /// character range table flags.
    fn gen_cond_crt(&mut self, ctx: &mut CompilerContext, tree: TreeId, crt_flags: i32) -> Item {
        if !self.gen_crt {
            return self.gen_cond(ctx, tree, false);
        }
        let start_pc = self.get_code().borrow_mut().cur_cp(ctx);
        let item = self.gen_cond(ctx, tree, (crt_flags & CRT_FLOW_CONTROLLER) != 0);
        let end_pc = self.get_code().borrow_mut().cur_cp(ctx);
        if let Some(crt) = &mut self.get_code().borrow_mut().crt {
            crt.put(CrtKey::Tree(tree), crt_flags, start_pc, end_pc);
        }
        item
    }

    /// Generate code for a boolean expression in a control-flow context.
    fn gen_cond(&mut self, ctx: &mut CompilerContext, tree: TreeId, mark_branches: bool) -> Item {
        let inner_tree = tree_info::skip_parens(ctx, tree);
        if ctx.tree(inner_tree).has_tag(Tag::CONDEXPR) {
            let (cond, truepart, falsepart) = match &ctx.tree(inner_tree).kind {
                TreeKind::Conditional {
                    cond,
                    truepart,
                    falsepart,
                    ..
                } => (
                    cond.expect("conditional condition"),
                    truepart.expect("conditional true part"),
                    falsepart.expect("conditional false part"),
                ),
                _ => crate::util::assert::error(),
            };
            let cond_item = self.gen_cond_crt(ctx, cond, CRT_FLOW_CONTROLLER);
            let items = self.get_items();
            let cond_is_true = items.cond_is_true(&cond_item);
            let cond_is_false = items.cond_is_false(&cond_item);
            if cond_is_true {
                let jumps = items.cond_true_jumps(&cond_item);
                self.get_code()
                    .borrow_mut()
                    .resolve_pending_chain(ctx, jumps);
                let mut result = self.gen_cond_crt(ctx, truepart, CRT_FLOW_TARGET);
                if mark_branches {
                    items.set_cond_tree(&mut result, truepart);
                }
                return result;
            }
            if cond_is_false {
                let jumps = items.cond_false_jumps(&cond_item);
                self.get_code()
                    .borrow_mut()
                    .resolve_pending_chain(ctx, jumps);
                let mut result = self.gen_cond_crt(ctx, falsepart, CRT_FLOW_TARGET);
                if mark_branches {
                    items.set_cond_tree(&mut result, falsepart);
                }
                return result;
            }
            let second_jumps = items.jump_false(ctx, &cond_item);
            let true_jumps = items.cond_true_jumps(&cond_item);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, true_jumps);
            let mut first = self.gen_cond_crt(ctx, truepart, CRT_FLOW_TARGET);
            if mark_branches {
                items.set_cond_tree(&mut first, truepart);
            }
            let false_jumps = items.jump_false(ctx, &first);
            let first_true = items.cond_true_jumps(&first);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, first_true);
            let true_jumps_chain = self.get_code().borrow_mut().branch(ctx, GOTO);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, second_jumps);
            let second = self.gen_cond_crt(ctx, falsepart, CRT_FLOW_TARGET);
            let second_opcode = items.cond_opcode(&second);
            let second_true = items.cond_true_jumps(&second);
            let second_false = items.cond_false_jumps(&second);
            let merged_true = Code::merge_chains(true_jumps_chain, second_true);
            let merged_false = Code::merge_chains(false_jumps, second_false);
            let mut result = items.make_cond_item(second_opcode, merged_true, merged_false);
            if mark_branches {
                items.set_cond_tree(&mut result, falsepart);
            }
            return result;
        }
        if ctx.tree(inner_tree).has_tag(Tag::SWITCH_EXPRESSION) {
            self.get_code().borrow_mut().resolve_pending(ctx);
            let prev_in_cond = self.in_cond_switch_expression;
            let prev_true = self.switch_expression_true_chain.take();
            let prev_false = self.switch_expression_false_chain.take();
            self.in_cond_switch_expression = true;
            self.switch_expression_true_chain = None;
            self.switch_expression_false_chain = None;
            self.do_handle_switch_expression(ctx, inner_tree);
            let true_chain = self.switch_expression_true_chain.take();
            let false_chain = self.switch_expression_false_chain.take();
            let items = self.get_items();
            let mut result = items.make_cond_item(GOTO, true_chain, false_chain);
            if mark_branches {
                items.set_cond_tree(&mut result, tree);
            }
            self.in_cond_switch_expression = prev_in_cond;
            self.switch_expression_true_chain = prev_true;
            self.switch_expression_false_chain = prev_false;
            return result;
        }
        if ctx.tree(inner_tree).has_tag(Tag::LETEXPR) {
            let needs_cond = match &ctx.tree(inner_tree).kind {
                TreeKind::LetExpr { needs_cond, .. } => *needs_cond,
                _ => false,
            };
            if needs_cond {
                self.get_code().borrow_mut().resolve_pending(ctx);
                let defs = match &ctx.tree(inner_tree).kind {
                    TreeKind::LetExpr { defs, .. } => defs.clone(),
                    _ => crate::util::assert::error(),
                };
                let limit = self.get_code().borrow().nextreg;
                let prev_start = {
                    let stacksize = self.get_code().borrow().state.stacksize;
                    self.get_code()
                        .borrow_mut()
                        .set_let_expr_stack_pos(stacksize)
                };
                self.gen_stats(ctx, defs);
                self.get_code()
                    .borrow_mut()
                    .set_let_expr_stack_pos(prev_start);
                let expr = match &ctx.tree(inner_tree).kind {
                    TreeKind::LetExpr {
                        expr: Some(expr), ..
                    } => *expr,
                    _ => panic!("AssertionError: let expression body"),
                };
                let result = self.gen_cond(ctx, expr, mark_branches);
                self.get_code().borrow_mut().end_scopes(ctx, limit);
                return result;
            }
        }
        let boolean_type = self.syms.boolean_type;
        let mut result = self.gen_expr(ctx, tree, boolean_type);
        result = self.get_items().mk_cond(ctx, &result);
        if mark_branches {
            let items = self.get_items();
            items.set_cond_tree(&mut result, tree);
        }
        result
    }

    /// Generate code for an expression, short-circuiting constant expressions.
    pub fn gen_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId, pt: TypeId) -> Item {
        let prev_pt = self.pt;
        let tree_ty = ctx.tree(tree).ty.expect("expression type");
        let const_value = ctx.typ(tree_ty).const_value().cloned();
        if std::env::var_os("DSH_DEBUG_COND").is_some()
            && const_value.is_some()
            && (ctx.tree(tree).tag == Tag::EQ
                || ctx.tree(tree).tag == Tag::NE)
        {
            eprintln!(
                "[GENCONST] pos={} tag={:?} ty={} const={:?}",
                ctx.tree(tree).pos,
                ctx.tree(tree).tag,
                crate::comp::attr::type_display_of(ctx, tree_ty),
                const_value,
            );
        }
        let mut result = if const_value.is_some() {
            // Short circuit any expression that is a constant.
            let mut visitor =
                ClassReferenceVisitor::new(Rc::clone(&self.pool_writer), Rc::clone(&self.syms));
            crate::tree::jc_tree::accept(ctx, tree, &mut visitor);
            self.check_string_constant(ctx, ctx.tree(tree).pos, const_value.as_ref());
            let sym = tree_info::symbol(ctx, tree);
            match sym {
                Some(sym) if self.is_constant_dynamic(ctx, sym) => {
                    let items = self.get_items();
                    items.make_dynamic_item(ctx, sym)
                }
                _ => {
                    let value = const_value.clone().expect("constant value");
                    let items = self.get_items();
                    items.make_immediate_item(ctx, tree_ty, immediate_value_from_constant(&value))
                }
            }
        } else {
            self.pt = Some(pt);
            crate::tree::jc_tree::accept(ctx, tree, self);
            self.result.take().expect("generated item")
        };
        self.pt = prev_pt;
        result = self.item_coerce_type(ctx, &result, pt);
        result
    }

    /// Return whether a symbol is a constant dynamic.
    fn is_constant_dynamic(&self, ctx: &CompilerContext, sym: SymbolId) -> bool {
        ctx.sym(sym).kind == crate::code::kinds::Kind::VAR
            && matches!(
                ctx.sym(sym).kind_data,
                crate::code::symbol::SymbolKind::DynamicVarSymbol { .. }
            )
    }

    /// Generate code for a list of method arguments.
    fn gen_args(&mut self, ctx: &mut CompilerContext, trees: List<TreeId>, pts: List<TypeId>) {
        let l = trees;
        let mut pts = pts;
        for head in &l {
            let pt = *pts.head();
            pts = pts.tail();
            let item = self.gen_expr(ctx, head, pt);
            self.item_load(ctx, &item);
        }
        assert!(pts.is_empty(), "AssertionError: argument lists must match");
    }
}

/// A visitor for expressions that might be constant expressions, looking for
/// references to class symbols that must be entered in the constant pool.
struct ClassReferenceVisitor {
    /// The constant pool writer.
    pub pool_writer: Rc<RefCell<PoolWriter>>,
    /// The symbol table.
    pub syms: Rc<Symtab>,
}

impl ClassReferenceVisitor {
    /// Create the visitor.
    fn new(pool_writer: Rc<RefCell<PoolWriter>>, syms: Rc<Symtab>) -> ClassReferenceVisitor {
        ClassReferenceVisitor { pool_writer, syms }
    }

    /// Enter a class type into the constant pool.
    fn put_class(&mut self, ctx: &mut CompilerContext, ty: TypeId) {
        let syms = Rc::clone(&self.syms);
        self.pool_writer.borrow_mut().put_class(ctx, &*syms, ty);
    }
}

impl TreeVisitor for ClassReferenceVisitor {
    fn visit_tree(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond,
                truepart,
                falsepart,
                ..
            } => (
                cond.expect("conditional cond"),
                truepart.expect("conditional truepart"),
                falsepart.expect("conditional falsepart"),
            ),
            _ => panic!("AssertionError: conditional expected"),
        };
        crate::tree::jc_tree::accept(ctx, cond, self);
        crate::tree::jc_tree::accept(ctx, truepart, self);
        crate::tree::jc_tree::accept(ctx, falsepart, self);
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr: Some(expr) } => *expr,
            _ => return,
        };
        crate::tree::jc_tree::accept(ctx, expr, self);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let arg = match &ctx.tree(tree).kind {
            TreeKind::Unary { arg: Some(arg), .. } => *arg,
            _ => return,
        };
        crate::tree::jc_tree::accept(ctx, arg, self);
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Binary { lhs, rhs, .. } => {
                (lhs.expect("binary lhs"), rhs.expect("binary rhs"))
            }
            _ => panic!("AssertionError: binary expected"),
        };
        crate::tree::jc_tree::accept(ctx, lhs, self);
        crate::tree::jc_tree::accept(ctx, rhs, self);
    }

    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::TypeCast {
                expr: Some(expr), ..
            } => *expr,
            _ => return,
        };
        crate::tree::jc_tree::accept(ctx, expr, self);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let selected_ty = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess {
                selected: Some(selected),
                ..
            } => ctx.tree(*selected).ty,
            _ => return,
        };
        if let Some(ty) = selected_ty
            && ctx.typ(ty).has_tag(TypeTag::CLASS)
        {
            self.put_class(ctx, ty);
        }
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let sym = match &ctx.tree(tree).kind {
            TreeKind::Ident { sym: Some(sym), .. } => *sym,
            _ => return,
        };
        if let Some(owner) = ctx.sym(sym).owner
            && ctx.sym(owner).kind == crate::code::kinds::Kind::TYP
        {
            let owner_ty = ctx.sym(owner).ty.expect("class symbol type");
            self.put_class(ctx, owner_ty);
        }
    }
}

/// Convert a constant value to an immediate item value.
fn immediate_value_from_constant(value: &crate::util::constants::ConstantValue) -> ImmediateValue {
    use crate::util::constants::ConstantValue;
    match value {
        ConstantValue::Integer(v) => ImmediateValue::Int(*v),
        ConstantValue::Byte(v) => ImmediateValue::Int(*v as i32),
        ConstantValue::Short(v) => ImmediateValue::Int(*v as i32),
        ConstantValue::Character(v) => ImmediateValue::Int(*v as i32),
        ConstantValue::Boolean(v) => ImmediateValue::Int(if *v { 1 } else { 0 }),
        ConstantValue::Long(v) => ImmediateValue::Long(*v),
        ConstantValue::Float(v) => ImmediateValue::Float(*v),
        ConstantValue::Double(v) => ImmediateValue::Double(*v),
        ConstantValue::String(v) => ImmediateValue::String(v.clone()),
    }
}

impl Gen {
    /// Generate code for a method definition.
    pub fn gen_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let local_env = self.dup_env(Some(tree), GenContext::new());
        // Java: localEnv.enclMethod = tree (Gen.java:924); dupEnv keeps the
        // enclosing method of the previous environment, so the fresh method
        // environment must be rebound to the method being generated.
        let local_env = Rc::new(Env {
            next: local_env.next.clone(),
            outer: local_env.outer.clone(),
            tree: Some(tree),
            toplevel: local_env.toplevel,
            encl_class: local_env.encl_class,
            encl_method: Some(tree),
            info: Rc::clone(&local_env.info),
            base_clause: local_env.base_clause,
        });
        let (sym, erasure) = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { sym: Some(sym), .. } => {
                let sym = *sym;
                let ty = ctx.sym(sym).ty.expect("method type");
                let syms = Rc::clone(&self.syms);
                let erased = self.types.erasure(ctx, &*syms, ty);
                (sym, erased)
            }
            _ => panic!("AssertionError: method declaration expected"),
        };
        let return_type = ctx.typ(erasure).get_return_type(ctx).expect("return type");
        self.pt = Some(return_type);
        let pos = ctx.tree(tree).pos;
        self.check_dimension(ctx, pos, erasure);
        self.gen_method(ctx, tree, &local_env, false);
        let _ = sym;
    }

    /// Generate code for a method.
    fn gen_method(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<GenContext>>,
        fatcode: bool,
    ) {
        let (meth, body) = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl {
                sym: Some(sym),
                body,
                ..
            } => (*sym, *body),
            _ => panic!("AssertionError: method declaration expected"),
        };
        let mut extras = 0;
        let is_constructor = ctx.sym(meth).is_constructor(&self.names);
        if is_constructor {
            extras += 1;
            let owner = ctx.sym(meth).owner.expect("method owner");
            let owner_inner = symbol_is_inner(ctx, owner);
            if owner_inner && !ctx.sym(owner).is_static(ctx) {
                extras += 1;
            }
        } else if (ctx.sym(meth).flags_field & crate::code::flags::STATIC as i64) == 0 {
            extras += 1;
        }
        let method_ty = ctx.sym(meth).ty.expect("method type");
        let params_width = {
            let syms = Rc::clone(&self.syms);
            let erased = self.types.erasure(ctx, &*syms, method_ty);
            let params = ctx.typ(erased).get_parameter_types(ctx);
            Code::width_list(ctx, &*syms, &params)
        };
        if params_width + extras > class_file::MAX_PARAMETERS {
            crate::util::abstract_log::AbstractLog::error_at(
                &*self.log,
                ctx.tree(tree).pos,
                Error::from(errors::limit_parameters()),
            );
            self.nerrs += 1;
        } else if let Some(body) = body {
            // Java: Gen.visitMethodDef creates the method environment
            // (localEnv = env.dup(tree); localEnv.enclMethod = tree) and installs
            // it as Gen.env before genMethod runs.  Unwinding a return/break
            // walks that chain until the method tree (Gen.unwind).
            let method_env = Rc::new(Env {
                next: Some(Rc::clone(env)),
                outer: env.outer.clone(),
                tree: Some(tree),
                toplevel: env.toplevel,
                encl_class: env.encl_class,
                encl_method: Some(tree),
                info: Rc::new(RefCell::new(GenContext::new())),
                base_clause: env.base_clause,
            });
            let prev_env = self.env.clone();
            self.env = Some(Rc::clone(&method_env));
            let startpc_crt = self.init_code(ctx, tree, &method_env, fatcode);
            self.gen_stat_crt(ctx, body, CRT_BLOCK);
            if self.get_code().borrow().state.stacksize != 0 {
                let info = errors::stack_sim_error(
                    crate::resources::diagnostic_contract::SymbolArg::placeholder(
                        ctx.sym(meth).name.to_java_string().to_string_lossy(),
                    ),
                );
                crate::util::abstract_log::AbstractLog::error_at(
                    &*self.log,
                    ctx.tree(body).pos,
                    Error::from(info),
                );
                let cls = crate::code::symbol::symbol_encl_class(ctx, meth)
                    .map(|c| {
                        ctx.sym(c)
                            .name
                            .to_java_string()
                            .to_string_lossy()
                            .to_string()
                    })
                    .unwrap_or_default();
                eprintln!(
                    "[STACK] method={} class={} body_pos={} file={}",
                    ctx.sym(meth).name.to_java_string().to_string_lossy(),
                    cls,
                    ctx.tree(body).pos,
                    self.log
                        .current_source_file()
                        .map(|f| f.get_name())
                        .unwrap_or_default()
                );
                panic!("AssertionError: stack size not zero");
            }
            if self.get_code().borrow().is_alive() {
                let body_end = tree_info::end_pos(ctx, body);
                self.get_code().borrow_mut().stat_begin(body_end);
                let return_void = env
                    .encl_method
                    .map(|md| {
                        let md_sym = match &ctx.tree(md).kind {
                            TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
                            _ => meth,
                        };
                        let ty = ctx.sym(md_sym).ty.expect("method type");
                        let syms = Rc::clone(&self.syms);
                        let erased = self.types.erasure(ctx, &*syms, ty);
                        ctx.typ(erased)
                            .get_return_type(ctx)
                            .map(|ret| ctx.typ(ret).has_tag(TypeTag::VOID))
                            .unwrap_or(true)
                    })
                    .unwrap_or(true);
                if return_void {
                    self.get_code().borrow_mut().emitop0(ctx, RETURN);
                } else {
                    // Sometimes dead code seems alive; generate a small loop.
                    let startpc = self.get_code().borrow_mut().entry_point(ctx);
                    let c = self.get_items().make_cond_item_opcode(GOTO);
                    let chain = self.get_items().jump_true(ctx, &c);
                    self.get_code()
                        .borrow_mut()
                        .resolve_chain(ctx, chain, startpc);
                }
            }
            if self.gen_crt {
                let end_pc = self.get_code().borrow_mut().cur_cp(ctx);
                let start = startpc_crt;
                if let Some(crt) = &mut self.get_code().borrow_mut().crt {
                    crt.put(CrtKey::Tree(body), CRT_BLOCK, start, end_pc);
                }
            }
            self.get_code().borrow_mut().end_scopes(ctx, 0);
            let check_pos = ctx.tree(tree).pos;
            let failed = {
                let code = self.get_code();
                let code = code.borrow_mut();
                code.check_limits(check_pos, &self.log)
            };
            if failed {
                self.nerrs += 1;
                self.env = prev_env;
                return;
            }
            let is_fat = self.get_code().borrow().fatcode;
            if !fatcode && is_fat {
                self.gen_method(ctx, tree, env, true);
            }
            if self.stack_map == StackMapFormat::JSR202 {
                let code = self.get_code();
                let mut code = code.borrow_mut();
                code.last_frame = None;
                code.frame_before_last = None;
            }
            self.get_code().borrow_mut().compress_catch_table();
            self.get_code()
                .borrow_mut()
                .fill_exception_parameter_positions(ctx);
            self.env = prev_env;
        }
    }

    /// Create a new code structure for a method and initialize it.
    fn init_code(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<GenContext>>,
        fatcode: bool,
    ) -> i32 {
        let meth = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("AssertionError: method declaration expected"),
        };
        let line_map = if self.line_debug_info {
            let top = self.toplevel.expect("toplevel");
            match &ctx.tree(top).kind {
                TreeKind::CompilationUnit { line_map, .. } => line_map.clone(),
                _ => None,
            }
        } else {
            None
        };
        let crt = if self.gen_crt {
            let end_positions = self.end_pos_table.clone();
            Some(CRTable::new(tree, end_positions))
        } else {
            None
        };
        let code = Rc::new(RefCell::new(Code::new(
            meth,
            fatcode,
            line_map,
            self.var_debug_info,
            self.stack_map,
            self.debug_code,
            crt,
            Rc::clone(&self.syms),
            Rc::clone(&self.types),
            Rc::clone(&self.pool_writer),
        )));
        self.code = Some(Rc::clone(&code));
        self.method_codes
            .borrow_mut()
            .insert(meth, Rc::clone(&code));
        self.items = Some(Rc::new(Items::new(
            Rc::clone(&self.pool_writer),
            Rc::clone(&code),
            Rc::clone(&self.syms),
            Rc::clone(&self.types),
        )));
        if self.debug_code {
            let sym = symbol_display(ctx, meth);
            eprintln!("{} for body {:?}", sym, tree);
        }
        // If the method is not static, create a new local variable address for
        // this.
        let flags = ctx.sym(meth).flags_field;
        if (flags & crate::code::flags::STATIC as i64) == 0 {
            let owner = ctx.sym(meth).owner.expect("method owner");
            let mut self_type = ctx.sym(owner).ty.expect("owner type");
            let object_type = self.syms.object_type;
            if ctx.sym(meth).is_constructor(&self.names) && self_type != object_type {
                self_type = crate::jvm::uninitialized_type::uninitialized_this(ctx, self_type);
            }
            let names = Rc::clone(&self.names);
            let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
                crate::code::flags::FINAL as i64,
                names._this.clone(),
                self_type,
                owner,
            ));
            let code = Rc::clone(&code);
            let reg = code.borrow_mut().new_local_symbol(ctx, sym);
            code.borrow_mut().set_defined_at(ctx, reg);
        }
        // Mark all parameters as defined from the beginning of the method.
        let params = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { params, .. } => params.clone(),
            _ => List::nil(),
        };
        let l = params;
        let mut registered_symbols: Vec<SymbolId> = Vec::new();
        for head in &l {
            let param_sym = match &ctx.tree(head).kind {
                TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                _ => panic!("AssertionError: parameter symbol"),
            };
            let param_ty = ctx.sym(param_sym).ty.expect("parameter type");
            self.check_dimension(ctx, ctx.tree(head).pos, param_ty);
            let reg = code.borrow_mut().new_local_symbol(ctx, param_sym);
            code.borrow_mut().set_defined_at(ctx, reg);
            registered_symbols.push(param_sym);
        }
        // Synthetic lambda methods keep their captured parameters in
        // MethodSymbol.extra_params (they have no VariableDecl tree nodes).
        let (extra_params, captured_locals) = match &ctx.sym(meth).kind_data {
            crate::code::symbol::SymbolKind::MethodSymbol {
                extra_params,
                captured_locals,
                ..
            } => (extra_params.clone(), captured_locals.clone()),
            _ => (List::nil(), List::nil()),
        };
        let extra = extra_params;
        for param_sym in &extra {
            if registered_symbols.contains(&param_sym) {
                continue;
            }
            registered_symbols.push(param_sym);
            let reg = code.borrow_mut().new_local_symbol(ctx, param_sym);
            code.borrow_mut().set_defined_at(ctx, reg);
        }
        let captured = captured_locals;
        for param_sym in &captured {
            if registered_symbols.contains(&param_sym) {
                continue;
            }
            registered_symbols.push(param_sym);
            let reg = code.borrow_mut().new_local_symbol(ctx, param_sym);
            code.borrow_mut().set_defined_at(ctx, reg);
        }
        let startpc_crt = if self.gen_crt {
            code.borrow_mut().cur_cp(ctx)
        } else {
            0
        };
        code.borrow_mut().entry_point(ctx);
        // Suppress the initial stack map.
        code.borrow_mut().pending_stack_map = false;
        let _ = env;
        startpc_crt
    }

    /// Generate code for a variable definition.
    pub fn gen_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (sym, init) = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                sym: Some(sym),
                init,
                ..
            } => (*sym, *init),
            _ => panic!("AssertionError: variable declaration expected"),
        };
        if let Some(init) = init {
            let const_value = ctx.sym(sym).get_const_value();
            self.check_string_constant(ctx, ctx.tree(init).pos, const_value.as_ref());
            if const_value.is_none() || self.var_debug_info {
                assert!(
                    self.get_code().borrow().is_statement_start(),
                    "AssertionError: variable definition not at statement start"
                );
                self.get_code().borrow_mut().new_local_symbol(ctx, sym);
                let sym_ty = ctx.sym(sym).ty.expect("variable type");
                let syms = Rc::clone(&self.syms);
                let erased = self.types.erasure(ctx, &*syms, sym_ty);
                let item = self.gen_expr(ctx, init, erased);
                self.item_load(ctx, &item);
                let items = self.get_items();
                let local = items.make_local_item(ctx, sym);
                items.store(ctx, &local);
                assert!(
                    self.get_code().borrow().is_statement_start(),
                    "AssertionError: variable definition not at statement start"
                );
            }
        } else {
            self.get_code().borrow_mut().new_local_symbol(ctx, sym);
        }
        let var_ty = match &ctx.tree(tree).kind {
            TreeKind::VariableDecl {
                vartype: Some(vartype),
                ..
            } => ctx.tree(*vartype).ty,
            TreeKind::VariableDecl { sym: Some(sym), .. } => ctx.sym(*sym).ty,
            _ => None,
        };
        if let Some(var_ty) = var_ty {
            let pos = ctx.tree(tree).pos;
            self.check_dimension(ctx, pos, var_ty);
        }
    }

    /// Generate code for a skip statement.
    pub fn gen_skip(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    /// Generate code for a block.
    pub fn gen_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let limit = self.get_code().borrow().nextreg;
        let local_env = self.dup_env(Some(tree), GenContext::new());
        let stats = match &ctx.tree(tree).kind {
            TreeKind::Block { stats, .. } => stats.clone(),
            _ => panic!("AssertionError: block expected"),
        };
        // Java: Gen.visitBlock: genStats(tree.stats, localEnv); the ported
        // genStats reads Gen.env, so install the block environment.
        let prev_env = self.env.clone();
        self.env = Some(Rc::clone(&local_env));
        self.gen_stats_crt(ctx, stats, 0);
        self.env = prev_env;
        // End the scope of all block-local variables in the variable info.
        let env = self.env.clone().expect("generation environment");
        let env_tree = env.tree.expect("generation environment tree");
        if !ctx.tree(env_tree).has_tag(Tag::METHODDEF) {
            let endpos = match &ctx.tree(tree).kind {
                TreeKind::Block { endpos, .. } => *endpos,
                _ => NOPOS,
            };
            self.get_code().borrow_mut().stat_begin(endpos);
            self.get_code().borrow_mut().end_scopes(ctx, limit);
            self.get_code().borrow_mut().pending_stat_pos = NOPOS;
        }
    }
}

/// Return whether a class symbol is an inner class.
fn symbol_is_inner(ctx: &CompilerContext, sym: SymbolId) -> bool {
    let owner = match ctx.sym(sym).owner {
        Some(owner) => owner,
        None => return false,
    };
    ctx.sym(owner).kind == crate::code::kinds::Kind::TYP && !ctx.sym(owner).is_interface()
}

/// Return a printable description of a symbol.
fn symbol_display(ctx: &CompilerContext, sym: SymbolId) -> String {
    ctx.sym(sym).name.to_java_string().to_string_lossy()
}

impl Gen {
    /// Generate code for a loop.
    fn gen_loop(
        &mut self,
        ctx: &mut CompilerContext,
        loop_tree: TreeId,
        body: TreeId,
        cond: Option<TreeId>,
        step: List<TreeId>,
        test_first: bool,
    ) {
        let loop_env = self.dup_env(Some(loop_tree), GenContext::new());
        // Java: Gen.visitForLoop/visitWhileLoop: the body/cond/step are
        // generated with loopEnv installed (break/continue unwind to it).
        let prev_env = self.env.clone();
        self.env = Some(Rc::clone(&loop_env));
        let startpc = self.get_code().borrow_mut().entry_point(ctx);
        if test_first {
            // while or for loop
            let c = if let Some(cond) = cond {
                self.get_code().borrow_mut().stat_begin(ctx.tree(cond).pos);
                assert!(
                    self.get_code().borrow().is_statement_start(),
                    "AssertionError: not at statement start"
                );
                let inner = tree_info::skip_parens(ctx, cond);
                self.gen_cond_crt(ctx, inner, CRT_FLOW_CONTROLLER)
            } else {
                self.get_items().make_cond_item_opcode(GOTO)
            };
            let loop_done = self.get_items().jump_false(ctx, &c);
            let true_jumps = self.get_items().cond_true_jumps(&c);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, true_jumps);
            assert!(
                self.get_code().borrow().is_statement_start(),
                "AssertionError: not at statement start"
            );
            self.gen_stat_crt(ctx, body, CRT_STATEMENT | CRT_FLOW_TARGET);
            let cont = loop_env.info.borrow().cont.clone();
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, cont);
            self.gen_stats_crt(ctx, step, 0);
            let branch = self.get_code().borrow_mut().branch(ctx, GOTO);
            self.get_code()
                .borrow_mut()
                .resolve_chain(ctx, branch, startpc);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, loop_done);
        } else {
            self.gen_stat_crt(ctx, body, CRT_STATEMENT | CRT_FLOW_TARGET);
            let cont = loop_env.info.borrow().cont.clone();
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, cont);
            self.gen_stats_crt(ctx, step, 0);
            if self.get_code().borrow().is_alive() {
                let c = if let Some(cond) = cond {
                    self.get_code().borrow_mut().stat_begin(ctx.tree(cond).pos);
                    assert!(
                        self.get_code().borrow().is_statement_start(),
                        "AssertionError: not at statement start"
                    );
                    let inner = tree_info::skip_parens(ctx, cond);
                    self.gen_cond_crt(ctx, inner, CRT_FLOW_CONTROLLER)
                } else {
                    self.get_items().make_cond_item_opcode(GOTO)
                };
                let true_jumps = self.get_items().jump_true(ctx, &c);
                self.get_code()
                    .borrow_mut()
                    .resolve_chain(ctx, true_jumps, startpc);
                assert!(
                    self.get_code().borrow().is_statement_start(),
                    "AssertionError: not at statement start"
                );
                let false_jumps = self.get_items().cond_false_jumps(&c);
                self.get_code()
                    .borrow_mut()
                    .resolve_pending_chain(ctx, false_jumps);
            }
        }
        let exit = loop_env.info.borrow().exit.clone();
        if let Some(exit) = exit {
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, Some(exit));
            let nextreg = self.get_code().borrow().nextreg;
            exclude_from(&mut loop_env.info.borrow_mut().exit, nextreg);
        }
        self.env = prev_env;
    }

    /// Generate code for a do-while loop.
    pub fn gen_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (body, cond) = match &ctx.tree(tree).kind {
            TreeKind::DoWhileLoop {
                body: Some(body),
                cond: Some(cond),
            } => (*body, *cond),
            _ => panic!("AssertionError: do-while loop expected"),
        };
        self.gen_loop(ctx, tree, body, Some(cond), List::nil(), false);
    }

    /// Generate code for a while loop.
    pub fn gen_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (cond, body) = match &ctx.tree(tree).kind {
            TreeKind::WhileLoop {
                cond: Some(cond),
                body: Some(body),
            } => (*cond, *body),
            _ => panic!("AssertionError: while loop expected"),
        };
        self.gen_loop(ctx, tree, body, Some(cond), List::nil(), true);
    }

    /// Generate code for a for loop.
    pub fn gen_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let limit = self.get_code().borrow().nextreg;
        let (init, cond, step, body) = match &ctx.tree(tree).kind {
            TreeKind::ForLoop {
                init,
                cond,
                step,
                body: Some(body),
            } => (init.clone(), *cond, step.clone(), *body),
            _ => panic!("AssertionError: for loop expected"),
        };
        self.gen_stats(ctx, init);
        self.gen_loop(ctx, tree, body, cond, step, true);
        self.get_code().borrow_mut().end_scopes(ctx, limit);
    }

    /// Generate code for an enhanced for loop.
    pub fn gen_foreach_loop(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        panic!("AssertionError: enhanced for loop should have been removed by Lower");
    }

    /// Generate code for a labeled statement.
    pub fn gen_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let local_env = self.dup_env(Some(tree), GenContext::new());
        let body = match &ctx.tree(tree).kind {
            TreeKind::LabeledStatement {
                body: Some(body), ..
            } => *body,
            _ => panic!("AssertionError: labeled statement expected"),
        };
        let prev_env = self.env.clone();
        self.env = Some(Rc::clone(&local_env));
        self.gen_stat_crt(ctx, body, CRT_STATEMENT);
        self.env = prev_env;
        let exit = local_env.info.borrow().exit.clone();
        if let Some(exit) = exit {
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, Some(exit));
            let nextreg = self.get_code().borrow().nextreg;
            exclude_from(&mut local_env.info.borrow_mut().exit, nextreg);
        }
    }

    /// Generate code for a switch statement.
    pub fn gen_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selector, cases, pattern_switch) = match &ctx.tree(tree).kind {
            TreeKind::Switch {
                selector: Some(selector),
                cases,
                pattern_switch,
                ..
            } => (*selector, cases.clone(), *pattern_switch),
            _ => panic!("AssertionError: switch expected"),
        };
        self.handle_switch(ctx, tree, selector, cases, pattern_switch);
    }

    /// Generate code for a switch expression.
    pub fn gen_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.get_code().borrow_mut().resolve_pending(ctx);
        let prev_in_cond = self.in_cond_switch_expression;
        self.in_cond_switch_expression = false;
        self.do_handle_switch_expression(ctx, tree);
        self.in_cond_switch_expression = prev_in_cond;
        let pt = self.pt.expect("expected type");
        let items = self.get_items();
        self.result = Some(items.make_stack_item(ctx, pt));
    }

    /// Generate code for a switch expression in a conditional context.
    fn do_handle_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let prev_stack = std::mem::take(&mut self.stack_before_switch_expression);
        let prev_result = self.switch_result.take();
        let limit = self.get_code().borrow().nextreg;
        let mut stack_before: Vec<Item> = Vec::new();
        let mut switch_result = None;
        if self.has_try(ctx, tree) {
            // If the switch expression contains try-catch, the catch handlers
            // need an empty stack: stash the whole stack to local variables.
            while self.get_code().borrow().state.stacksize > 0 {
                let ty = self.get_code().borrow().state.peek();
                let cp = self.get_code().borrow().cp;
                let stacksize = self.get_code().borrow().state.stacksize;
                let synthetic = self.target.synthetic_name_char();
                let name_text = format!(
                    "{0}stack{0}{1}{0}{2}",
                    synthetic,
                    ctx.tree(tree).pos,
                    stacksize
                );
                let name = self
                    .names
                    .from_string(&crate::util::java_string::JavaString::from(
                        name_text.as_str(),
                    ));
                let env = self.env.clone().expect("generation environment");
                let encl_method = env.encl_method.expect("enclosing method");
                let method_sym = match &ctx.tree(encl_method).kind {
                    TreeKind::MethodDecl { sym: Some(sym), .. } => *sym,
                    _ => panic!("AssertionError: enclosing method symbol"),
                };
                let sym = ctx.alloc_symbol(crate::code::symbol::Symbol::new_var_symbol(
                    crate::code::flags::SYNTHETIC as i64,
                    name,
                    ty,
                    method_sym,
                ));
                let item = {
                    let code = self.get_code();
                    let reg = code.borrow_mut().new_local_symbol(ctx, sym);
                    self.get_items().make_local_item_anonymous(ctx, ty, reg)
                };
                stack_before.insert(0, item.clone());
                self.item_store(ctx, &item);
                let _ = cp;
            }
            switch_result = Some(self.make_temp(ctx, ctx.tree(tree).ty.expect("switch type")));
        }
        self.stack_before_switch_expression = stack_before;
        self.switch_result = switch_result;
        let prev_let = {
            let stacksize = self.get_code().borrow().state.stacksize;
            self.get_code()
                .borrow_mut()
                .set_let_expr_stack_pos(stacksize)
        };
        let (selector, cases, pattern_switch) = match &ctx.tree(tree).kind {
            TreeKind::SwitchExpression {
                selector: Some(selector),
                cases,
                pattern_switch,
                ..
            } => (*selector, cases.clone(), *pattern_switch),
            _ => panic!("AssertionError: switch expression expected"),
        };
        self.handle_switch(ctx, tree, selector, cases, pattern_switch);
        self.get_code()
            .borrow_mut()
            .set_let_expr_stack_pos(prev_let);
        self.stack_before_switch_expression = prev_stack;
        self.switch_result = prev_result;
        self.get_code().borrow_mut().end_scopes(ctx, limit);
    }

    /// Return whether a switch expression contains a try statement.
    fn has_try(&self, ctx: &mut CompilerContext, tree: TreeId) -> bool {
        let mut scanner = TryScanner { has_try: false };
        TreeScannerVisitor::scan(&mut scanner, ctx, Some(tree));
        scanner.has_try
    }

    /// Generate code for a switch statement or expression.
    fn handle_switch(
        &mut self,
        ctx: &mut CompilerContext,
        swtch: TreeId,
        selector: TreeId,
        cases: List<TreeId>,
        pattern_switch: bool,
    ) {
        let limit = self.get_code().borrow().nextreg;
        assert!(
            !ctx.typ(ctx.tree(selector).ty.expect("selector type"))
                .has_tag(TypeTag::CLASS),
            "AssertionError: switch selector must not be a class"
        );
        let switch_start = if pattern_switch {
            self.get_code().borrow_mut().entry_point(ctx)
        } else {
            -1
        };
        let startpc_crt = if self.gen_crt {
            self.get_code().borrow_mut().cur_cp(ctx)
        } else {
            0
        };
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        let int_type = self.syms.int_type;
        let sel = self.gen_expr(ctx, selector, int_type);
        if cases.is_empty() {
            // switch <sel> {}
            let loaded = self.item_load(ctx, &sel);
            self.item_drop(ctx, &loaded);
            if self.gen_crt {
                let inner = tree_info::skip_parens(ctx, selector);
                let end_pc = self.get_code().borrow_mut().cur_cp(ctx);
                if let Some(crt) = &mut self.get_code().borrow_mut().crt {
                    crt.put(
                        CrtKey::Tree(inner),
                        CRT_FLOW_CONTROLLER,
                        startpc_crt,
                        end_pc,
                    );
                }
            }
        } else {
            self.item_load(ctx, &sel);
            if self.gen_crt {
                let inner = tree_info::skip_parens(ctx, selector);
                let end_pc = self.get_code().borrow_mut().cur_cp(ctx);
                if let Some(crt) = &mut self.get_code().borrow_mut().crt {
                    crt.put(
                        CrtKey::Tree(inner),
                        CRT_FLOW_CONTROLLER,
                        startpc_crt,
                        end_pc,
                    );
                }
            }
            let switch_env = self.dup_env(Some(swtch), GenContext::new());
            switch_env.info.borrow_mut().is_switch = true;
            let n_cases = cases.length();
            let mut lo = i32::MAX;
            let mut hi = i32::MIN;
            let mut nlabels = 0;
            let mut labels: Vec<i32> = vec![0; n_cases as usize];
            let mut default_index: i32 = -1;
            let mut l = cases.clone();
            for i in 0..labels.len() {
                let case_tree = *l.head();
                l = l.tail();
                let (case_labels, _) = match &ctx.tree(case_tree).kind {
                    TreeKind::Case { labels, .. } => (labels.clone(), ()),
                    _ => panic!("AssertionError: case expected"),
                };
                if case_label_is_expression(ctx, *case_labels.head()) {
                    assert_eq!(case_labels.length(), 1, "AssertionError: only one case label expected");
                    let label_ty = ctx.tree(*case_labels.head()).ty.expect("label type");
                    let val = match ctx.typ(label_ty).const_value() {
                        Some(crate::util::constants::ConstantValue::Integer(v)) => *v,
                        // Port adapter: the port keeps the Character/Byte/Short
                        // constant wrappers, while the upstream constant value
                        // is already the widened Integer; widen here.
                        Some(crate::util::constants::ConstantValue::Character(c)) => *c as i32,
                        Some(crate::util::constants::ConstantValue::Byte(v)) => *v as i32,
                        Some(crate::util::constants::ConstantValue::Short(v)) => *v as i32,
                        Some(other) => {
                            panic!("AssertionError: integer case label expected: {:?}", other)
                        }
                        None => panic!("AssertionError: constant case label expected"),
                    };
                    labels[i] = val;
                    if val < lo {
                        lo = val;
                    }
                    if hi < val {
                        hi = val;
                    }
                    nlabels += 1;
                } else {
                    assert_eq!(default_index, -1, "AssertionError: multiple default cases");
                    default_index = i as i32;
                }
            }
            let table_space_cost = 4 + (hi as i64 - lo as i64 + 1);
            let table_time_cost = 3;
            let lookup_space_cost = 3 + 2 * nlabels as i64;
            let lookup_time_cost = nlabels as i64;
            let opcode = if nlabels > 0
                && table_space_cost + 3 * table_time_cost
                    <= lookup_space_cost + 3 * lookup_time_cost
            {
                TABLESWITCH
            } else {
                LOOKUPSWITCH
            };
            let startpc = self.get_code().borrow_mut().cur_cp(ctx);
            self.get_code().borrow_mut().emitop0(ctx, opcode);
            self.get_code().borrow_mut().align(ctx, 4);
            let table_base = self.get_code().borrow_mut().cur_cp(ctx);
            let mut offsets: Vec<i32> = vec![0; labels.len()];
            self.get_code().borrow_mut().emit4(-1);
            if opcode == TABLESWITCH {
                self.get_code().borrow_mut().emit4(lo);
                self.get_code().borrow_mut().emit4(hi);
                let mut i = lo as i64;
                while i <= hi as i64 {
                    self.get_code().borrow_mut().emit4(-1);
                    i += 1;
                }
            } else {
                self.get_code().borrow_mut().emit4(nlabels);
                for _ in 0..nlabels {
                    self.get_code().borrow_mut().emit4(-1);
                    self.get_code().borrow_mut().emit4(-1);
                }
            }
            let state_switch = self.get_code().borrow().state.dup();
            self.get_code().borrow_mut().mark_dead();
            let prev_env = self.env.clone();
            self.env = Some(Rc::clone(&switch_env));
            let mut l = cases.clone();
            for i in 0..labels.len() {
                let case_tree = *l.head();
                l = l.tail();
                let pc = {
                    let code = self.get_code();
                    let mut code = code.borrow_mut();
                    code.entry_point_state(ctx, &state_switch)
                };
                if i as i32 != default_index {
                    if opcode == TABLESWITCH {
                        let target = table_base + 4 * (labels[i] - lo + 3);
                        let value = pc - startpc;
                        self.get_code().borrow_mut().put4(target, value);
                    } else {
                        offsets[i] = pc - startpc;
                    }
                } else {
                    let value = pc - startpc;
                    self.get_code().borrow_mut().put4(table_base, value);
                }
                let stats = match &ctx.tree(case_tree).kind {
                    TreeKind::Case { stats, .. } => stats.clone(),
                    _ => panic!("AssertionError: case expected"),
                };
                self.gen_stats_crt(ctx, stats, CRT_FLOW_TARGET);
            }
            self.env = prev_env;
            let cont = switch_env.info.borrow().cont.clone();
            if let Some(cont) = cont {
                assert!(pattern_switch, "AssertionError: pattern switch expected");
                self.get_code()
                    .borrow_mut()
                    .resolve_chain(ctx, Some(cont), switch_start);
            }
            let exit = switch_env.info.borrow().exit.clone();
            if let Some(exit) = exit {
                self.get_code()
                    .borrow_mut()
                    .resolve_pending_chain(ctx, Some(exit));
                let limit_reg = limit;
                exclude_from(&mut switch_env.info.borrow_mut().exit, limit_reg);
            }
            // If the default offset is still unset, set it now.
            let table_value = self.get_code().borrow().get4(table_base);
            if table_value == -1 {
                let pc = {
                    let code = self.get_code();
                    let mut code = code.borrow_mut();
                    code.entry_point_state(ctx, &state_switch)
                };
                let value = pc - startpc;
                self.get_code().borrow_mut().put4(table_base, value);
            }
            if opcode == TABLESWITCH {
                let default_offset = self.get_code().borrow().get4(table_base);
                let mut i = lo as i64;
                while i <= hi as i64 {
                    let t = (table_base as i64 + 4 * (i - lo as i64 + 3)) as i32;
                    if self.get_code().borrow().get4(t) == -1 {
                        self.get_code().borrow_mut().put4(t, default_offset);
                    }
                    i += 1;
                }
            } else {
                if default_index >= 0 {
                    let mut i = default_index as usize;
                    while i < labels.len() - 1 {
                        labels[i] = labels[i + 1];
                        offsets[i] = offsets[i + 1];
                        i += 1;
                    }
                }
                if nlabels > 0 {
                    qsort2(&mut labels, &mut offsets, 0, nlabels - 1);
                }
                for i in 0..nlabels as usize {
                    let caseidx = table_base + 8 * (i as i32 + 1);
                    self.get_code().borrow_mut().put4(caseidx, labels[i]);
                    self.get_code().borrow_mut().put4(caseidx + 4, offsets[i]);
                }
            }
        }
        self.get_code().borrow_mut().end_scopes(ctx, limit);
    }
}

/// Sort int keys and values with the given recursion structure.
fn qsort2(keys: &mut [i32], values: &mut [i32], lo: i32, hi: i32) {
    let mut i = lo;
    let mut j = hi;
    let pivot = keys[((i + j) / 2) as usize];
    loop {
        while keys[i as usize] < pivot {
            i += 1;
        }
        while pivot < keys[j as usize] {
            j -= 1;
        }
        if i <= j {
            keys.swap(i as usize, j as usize);
            values.swap(i as usize, j as usize);
            i += 1;
            j -= 1;
        }
        if i > j {
            break;
        }
    }
    if lo < j {
        qsort2(keys, values, lo, j);
    }
    if i < hi {
        qsort2(keys, values, i, hi);
    }
}

/// Exclude all defined registers above the given limit from a jump chain.
fn exclude_from(chain: &mut Option<Box<Chain>>, limit: i32) {
    let mut current = chain.as_deref_mut();
    while let Some(node) = current {
        node.state.defined.exclude_from(limit);
        current = node.next.as_deref_mut();
    }
}

/// A scanner looking for try statements in a switch expression.
struct TryScanner {
    /// Whether a try statement was found.
    pub has_try: bool,
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(TryScanner);

impl TreeScannerVisitor for TryScanner {
    fn visit_class_def(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}

    fn visit_try(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        self.has_try = true;
    }

    fn visit_lambda(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {}
}

/// The finalizer of a synchronized statement.
struct SyncFinalizer {
    /// The environment of the synchronized statement.
    pub sync_env: Rc<Env<GenContext>>,
    /// The temp variable holding the monitor object.
    pub lock_var: Item,
}

impl GenFinalizer for SyncFinalizer {
    fn generate(&mut self, ctx: &mut CompilerContext, codegen: &mut Gen) {
        self.generate_last(ctx, codegen);
        let gaps_len = self.sync_env.info.borrow().gaps.len();
        assert!(gaps_len.is_multiple_of(2), "AssertionError: finalizer gaps");
        let cp = codegen.get_code().borrow_mut().cur_cp(ctx);
        self.sync_env.info.borrow_mut().gaps.push(cp);
    }

    fn generate_last(&mut self, ctx: &mut CompilerContext, codegen: &mut Gen) {
        if codegen.get_code().borrow().is_alive() {
            let lock_var = self.lock_var.clone();
            codegen.item_load(ctx, &lock_var);
            codegen.get_code().borrow_mut().emitop0(ctx, MONITOREXIT);
            let reg = local_reg(&lock_var);
            codegen.get_code().borrow_mut().state.unlock(reg);
        }
    }
}

/// The finalizer of a try statement.
struct TryFinalizer {
    /// The environment of the try statement.
    pub try_env: Rc<Env<GenContext>>,
    /// The environment enclosing the try statement.
    pub old_env: Rc<Env<GenContext>>,
    /// The try statement tree.
    pub tree: TreeId,
    /// Whether the try statement has a finally block (upstream
    /// tree.finalizer != null).
    pub has_finalizer: bool,
    /// Whether the finally block is body-only (BODY_ONLY_FINALIZE; upstream
    /// TryFinalizer.afterBody).
    pub body_only_finalize: bool,
}

impl GenFinalizer for TryFinalizer {
    fn generate(&mut self, ctx: &mut CompilerContext, codegen: &mut Gen) {
        let gaps_len = self.try_env.info.borrow().gaps.len();
        assert!(gaps_len.is_multiple_of(2), "AssertionError: finalizer gaps");
        let cp = codegen.get_code().borrow_mut().cur_cp(ctx);
        self.try_env.info.borrow_mut().gaps.push(cp);
        self.generate_last(ctx, codegen);
    }

    fn generate_last(&mut self, ctx: &mut CompilerContext, codegen: &mut Gen) {
        let finalizer = match &ctx.tree(self.tree).kind {
            TreeKind::Try { finalizer, .. } => *finalizer,
            _ => None,
        };
        if let Some(finalizer) = finalizer {
            let old_env = Rc::clone(&self.old_env);
            codegen.gen_stat_crt(ctx, finalizer, CRT_BLOCK);
            let _ = old_env;
        }
    }

    fn has_finalizer(&self) -> bool {
        self.has_finalizer
    }

    fn after_body(&mut self) {
        if self.body_only_finalize {
            // For body-only finally, remove the GenFinalizer after the try body
            // so that the finally is not generated to catch bodies.
            self.try_env.info.borrow_mut().finalize = None;
        }
    }
}

/// Return the register of a local item.
fn local_reg(item: &Item) -> i32 {
    match &item.kind {
        crate::jvm::items::ItemKind::Local { reg, .. } => *reg,
        _ => panic!("AssertionError: local item expected"),
    }
}

impl Gen {
    /// Generate code for a synchronized statement.
    pub fn gen_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let limit = self.get_code().borrow().nextreg;
        let lock = match &ctx.tree(tree).kind {
            TreeKind::Synchronized {
                lock: Some(lock), ..
            } => *lock,
            _ => panic!("AssertionError: synchronized statement expected"),
        };
        // Generate code to evaluate the lock and save it in a temp variable.
        let object_type = self.syms.object_type;
        let lock_var = self.make_temp(ctx, object_type);
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        let lock_ty = ctx.tree(lock).ty.expect("lock type");
        let item = self.gen_expr(ctx, lock, lock_ty);
        let loaded = self.item_load(ctx, &item);
        self.item_duplicate(ctx, &loaded);
        self.item_store(ctx, &lock_var);
        // Generate code to enter the monitor.
        self.get_code().borrow_mut().emitop0(ctx, MONITORENTER);
        let reg = local_reg(&lock_var);
        self.get_code().borrow_mut().state.lock(reg);
        // Generate code for a try statement with the given body.
        let sync_env = self.dup_env(Some(tree), GenContext::new());
        let finalizer: Rc<RefCell<dyn GenFinalizer>> = Rc::new(RefCell::new(SyncFinalizer {
            sync_env: Rc::clone(&sync_env),
            lock_var: lock_var.clone(),
        }));
        sync_env.info.borrow_mut().finalize = Some(finalizer);
        sync_env.info.borrow_mut().gaps = Vec::new();
        let body = match &ctx.tree(tree).kind {
            TreeKind::Synchronized {
                body: Some(body), ..
            } => *body,
            _ => panic!("AssertionError: synchronized body expected"),
        };
        // Java: visitSynchronized generates the body inside syncEnv so that a
        // return/break unwinds through the finalizer (monitorexit).
        let prev_env = self.env.clone();
        self.env = Some(Rc::clone(&sync_env));
        self.gen_try(ctx, body, List::nil(), &sync_env);
        self.env = prev_env;
        self.get_code().borrow_mut().end_scopes(ctx, limit);
    }

    /// Generate code for a try statement.
    pub fn gen_try_stmt(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let try_env = self.dup_env(Some(tree), GenContext::new());
        let old_env = self.env.clone().expect("generation environment");
        let (has_finalizer, body_only_finalize) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                finalizer: Some(finalizer),
                ..
            } => {
                let finalizer_flags = match &ctx.tree(*finalizer).kind {
                    TreeKind::Block { flags, .. } => *flags,
                    _ => 0,
                };
                (
                    true,
                    finalizer_flags & crate::code::flags::BODY_ONLY_FINALIZE != 0,
                )
            }
            _ => (false, false),
        };
        let finalizer: Rc<RefCell<dyn GenFinalizer>> = Rc::new(RefCell::new(TryFinalizer {
            try_env: Rc::clone(&try_env),
            old_env: Rc::clone(&old_env),
            tree,
            has_finalizer,
            body_only_finalize,
        }));
        try_env.info.borrow_mut().finalize = Some(finalizer);
        try_env.info.borrow_mut().gaps = Vec::new();
        let (body, catchers) = match &ctx.tree(tree).kind {
            TreeKind::Try {
                body: Some(body),
                catchers,
                ..
            } => (*body, catchers.clone()),
            _ => panic!("AssertionError: try statement expected"),
        };
        // Java: genTry generates the body inside the try environment so that a
        // return/break unwinds through the finally block.
        self.env = Some(Rc::clone(&try_env));
        self.gen_try(ctx, body, catchers, &try_env);
        self.env = Some(old_env);
    }

    /// Generate code for a try or synchronized statement.
    fn gen_try(
        &mut self,
        ctx: &mut CompilerContext,
        body: TreeId,
        catchers: List<TreeId>,
        env: &Rc<Env<GenContext>>,
    ) {
        let limit = self.get_code().borrow().nextreg;
        let startpc = self.get_code().borrow_mut().cur_cp(ctx);
        let state_try = self.get_code().borrow().state.dup();
        self.gen_stat_crt(ctx, body, CRT_BLOCK);
        let endpc = self.get_code().borrow_mut().cur_cp(ctx);
        let gaps: Vec<i32> = env.info.borrow().gaps.clone();
        let body_end = tree_info::end_pos(ctx, body);
        self.get_code().borrow_mut().stat_begin(body_end);
        self.gen_finalizer(ctx, env);
        let env_tree = env.tree.expect("generation environment tree");
        let env_tree_end = tree_info::end_pos(ctx, env_tree);
        self.get_code().borrow_mut().stat_begin(env_tree_end);
        let actual_try = ctx.tree(env_tree).has_tag(Tag::TRY);
        let mut exit_chain = if startpc == endpc && actual_try {
            self.get_code().borrow_mut().branch(ctx, JSR)
        } else {
            self.get_code().borrow_mut().branch(ctx, GOTO)
        };
        self.end_finalizer_gap(ctx, env);
        // Invoke the finalizer afterBody hook through the trait. The finalizer
        // is bound before the call: the hook mutates the same environment's
        // finalize slot (the upstream field write has no borrow scope).
        let after_body_finalizer = { env.info.borrow().finalize.clone() };
        if let Some(finalizer) = after_body_finalizer {
            finalizer.borrow_mut().after_body();
        }
        let has_finalizer = {
            let finalize = env.info.borrow().finalize.clone();
            match finalize {
                Some(finalizer) => finalizer.borrow().has_finalizer(),
                None => false,
            }
        };
        if startpc != endpc {
            let catcher_count = catchers.length() as usize;
            for (index, catcher) in catchers.into_iter().enumerate() {
                let param_ty = catch_param_type(ctx, catcher);
                let state = {
                    let code = self.get_code();
                    let mut code = code.borrow_mut();
                    code.entry_point_state_pushed(ctx, &state_try, param_ty)
                };
                let _ = state;
                self.gen_catch(ctx, catcher, env, startpc, endpc, &gaps);
                self.gen_finalizer(ctx, env);
                if has_finalizer || index + 1 < catcher_count {
                    self.get_code().borrow_mut().stat_begin(env_tree_end);
                    let branch = self.get_code().borrow_mut().branch(ctx, GOTO);
                    exit_chain = Code::merge_chains(exit_chain, branch);
                }
                self.end_finalizer_gap(ctx, env);
            }
        }
        if has_finalizer && (startpc != endpc || !actual_try) {
            // Create a new register segment to avoid allocating the same
            // variables in finalizers and other statements.
            self.get_code().borrow_mut().new_reg_segment();
            // Add a catch-all clause.
            let throwable_type = self.syms.throwable_type;
            let catchall_pc = {
                let code = self.get_code();
                let mut code = code.borrow_mut();
                code.entry_point_state_pushed(ctx, &state_try, throwable_type)
            };
            let mut startseg = startpc;
            while !env.info.borrow().gaps.is_empty() {
                let endseg = env.info.borrow_mut().gaps.remove(0);
                self.register_catch(ctx, ctx.tree(body).pos, startseg, endseg, catchall_pc, 0);
                let next = {
                    let mut info = env.info.borrow_mut();
                    if info.gaps.is_empty() {
                        endseg
                    } else {
                        info.gaps.remove(0)
                    }
                };
                startseg = next;
            }
            let first_stat = tree_info::finalizer_pos(ctx, env_tree, PosKind::FIRST_STAT_POS);
            self.get_code().borrow_mut().stat_begin(first_stat);
            self.get_code().borrow_mut().mark_stat_begin(ctx);
            let throwable_type = self.syms.throwable_type;
            let exc_var = self.make_temp(ctx, throwable_type);
            self.item_store(ctx, &exc_var);
            self.gen_finalizer(ctx, env);
            self.get_code().borrow_mut().resolve_pending(ctx);
            let end_pos = tree_info::finalizer_pos(ctx, env_tree, PosKind::END_POS);
            self.get_code().borrow_mut().stat_begin(end_pos);
            self.get_code().borrow_mut().mark_stat_begin(ctx);
            let exc_loaded = self.item_load(ctx, &exc_var);
            let _ = &exc_loaded;
            let endseg = {
                let mut info = env.info.borrow_mut();
                if info.gaps.is_empty() {
                    startseg
                } else {
                    info.gaps.remove(0)
                }
            };
            self.register_catch(ctx, ctx.tree(body).pos, startseg, endseg, catchall_pc, 0);
            self.get_code().borrow_mut().emitop0(ctx, ATHROW);
            self.get_code().borrow_mut().mark_dead();
            let cont = env.info.borrow().cont.clone();
            if let Some(cont) = cont {
                self.get_code()
                    .borrow_mut()
                    .resolve_pending_chain(ctx, Some(cont));
                let first_stat = tree_info::finalizer_pos(ctx, env_tree, PosKind::FIRST_STAT_POS);
                self.get_code().borrow_mut().stat_begin(first_stat);
                self.get_code().borrow_mut().mark_stat_begin(ctx);
                let throwable_type = self.syms.throwable_type;
                let ret_var = self.make_temp(ctx, throwable_type);
                self.item_store(ctx, &ret_var);
                // Generate the finalizer code.
                if let Some(finalizer) = env.info.borrow().finalize.clone() {
                    finalizer.borrow_mut().generate_last(ctx, self);
                }
                let reg = local_reg(&ret_var);
                self.get_code().borrow_mut().emitop1w(ctx, RET, reg);
                self.get_code().borrow_mut().mark_dead();
            }
        }
        // Resolve all breaks.
        self.get_code()
            .borrow_mut()
            .resolve_pending_chain(ctx, exit_chain);
        self.get_code().borrow_mut().end_scopes(ctx, limit);
    }

    /// Generate code for a catch clause.
    fn gen_catch(
        &mut self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        env: &Rc<Env<GenContext>>,
        startpc: i32,
        endpc: i32,
        gaps: &[i32],
    ) {
        if startpc == endpc {
            return;
        }
        let gaps = gaps.to_vec();
        let catch_type_exprs = self.catch_types_with_annotations(ctx, tree);
        let mut startpc = startpc;
        let mut gi = 0;
        while gi < gaps.len() {
            let end = gaps[gi];
            for (tas, subcatch) in &catch_type_exprs {
                let subcatch_ty = ctx.tree(*subcatch).ty.expect("catch type");
                let catch_type_expr = self.make_ref(ctx, ctx.tree(tree).pos, subcatch_ty);
                let handler_pc = self.get_code().borrow_mut().cur_cp(ctx);
                self.register_catch(
                    ctx,
                    ctx.tree(tree).pos,
                    startpc,
                    end,
                    handler_pc,
                    catch_type_expr,
                );
                for ta in tas.iter() {
                    let mut pos = ctx
                        .attr(ta)
                        .get_position(ctx)
                        .expect("type annotation position");
                    pos.set_catch_info(catch_type_expr, startpc);
                    set_attr_position(ctx, ta, pos);
                }
            }
            gi += 1;
            if gi < gaps.len() {
                startpc = gaps[gi];
                gi += 1;
            }
        }
        if startpc < endpc {
            for (tas, subcatch) in &catch_type_exprs {
                let subcatch_ty = ctx.tree(*subcatch).ty.expect("catch type");
                let catch_type_expr = self.make_ref(ctx, ctx.tree(tree).pos, subcatch_ty);
                let handler_pc = self.get_code().borrow_mut().cur_cp(ctx);
                self.register_catch(
                    ctx,
                    ctx.tree(tree).pos,
                    startpc,
                    endpc,
                    handler_pc,
                    catch_type_expr,
                );
                for ta in tas.iter() {
                    let mut pos = ctx
                        .attr(ta)
                        .get_position(ctx)
                        .expect("type annotation position");
                    pos.set_catch_info(catch_type_expr, startpc);
                    set_attr_position(ctx, ta, pos);
                }
            }
        }
        let (param_sym, body) = match &ctx.tree(tree).kind {
            TreeKind::Catch {
                param: Some(param),
                body: Some(body),
            } => {
                let sym = match &ctx.tree(*param).kind {
                    TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
                    _ => panic!("AssertionError: catch parameter expected"),
                };
                (sym, *body)
            }
            _ => panic!("AssertionError: catch clause expected"),
        };
        self.get_code().borrow_mut().stat_begin(ctx.tree(tree).pos);
        self.get_code().borrow_mut().mark_stat_begin(ctx);
        let limit = self.get_code().borrow().nextreg;
        self.get_code()
            .borrow_mut()
            .new_local_symbol(ctx, param_sym);
        let items = self.get_items();
        let local = items.make_local_item(ctx, param_sym);
        items.store(ctx, &local);
        let first_stat = tree_info::first_stat_pos(ctx, body);
        self.get_code().borrow_mut().stat_begin(first_stat);
        self.gen_stat_crt(ctx, body, CRT_BLOCK);
        self.get_code().borrow_mut().end_scopes(ctx, limit);
        let body_end = tree_info::end_pos(ctx, body);
        self.get_code().borrow_mut().stat_begin(body_end);
        let _ = env;
    }

    /// Return the catch type expressions with their annotations.
    fn catch_types_with_annotations(
        &self,
        ctx: &mut CompilerContext,
        tree: TreeId,
    ) -> Vec<(List<crate::ids::AttributeId>, TreeId)> {
        let (param, vartype) = match &ctx.tree(tree).kind {
            TreeKind::Catch {
                param: Some(param), ..
            } => {
                let vartype = match &ctx.tree(*param).kind {
                    TreeKind::VariableDecl {
                        vartype: Some(vartype),
                        ..
                    } => *vartype,
                    _ => panic!("AssertionError: catch parameter type expected"),
                };
                (*param, vartype)
            }
            _ => panic!("AssertionError: catch clause expected"),
        };
        if tree_info::is_multi_catch(ctx, tree) {
            let first = match &ctx.tree(param).kind {
                TreeKind::VariableDecl { sym: Some(sym), .. } => {
                    ctx.sym(*sym).get_raw_type_attributes()
                }
                _ => List::nil(),
            };
            self.catch_types_with_annotations_from_multicatch(ctx, vartype, first)
        } else {
            let tas = match &ctx.tree(param).kind {
                TreeKind::VariableDecl { sym: Some(sym), .. } => {
                    ctx.sym(*sym).get_raw_type_attributes()
                }
                _ => List::nil(),
            };
            vec![(tas, vartype)]
        }
    }

    /// Collect the catch types of a multicatch with their annotations.
    fn catch_types_with_annotations_from_multicatch(
        &self,
        ctx: &mut CompilerContext,
        tree: TreeId,
        first: List<crate::ids::AttributeId>,
    ) -> Vec<(List<crate::ids::AttributeId>, TreeId)> {
        let alts = match &ctx.tree(tree).kind {
            TreeKind::TypeUnion { alternatives, .. } => alternatives.clone(),
            _ => panic!("AssertionError: union type expected"),
        };
        // Java: Gen.java:1676-1691 builds the list with the head first, then
        // prepends the remaining alternatives and reverses the result, which
        // yields the source order of the union alternatives.
        let mut res: Vec<(List<crate::ids::AttributeId>, TreeId)> = Vec::new();
        let head = *alts.head();
        res.push((first, head));
        let rest = alts.tail();
        for alt in &rest {
            if ctx.tree(alt).has_tag(Tag::ANNOTATED_TYPE) {
                if let TreeKind::AnnotatedType { annotations, .. } = &ctx.tree(alt).kind {
                    let annotations = annotations.clone();
                    let tas = self.annotate.from_annotations(ctx, &annotations);
                    res.push((tas, alt));
                }
            } else {
                res.push((List::nil(), alt));
            }
        }
        res
    }

    /// Register a catch clause in the exception table.
    fn register_catch(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        startpc: i32,
        endpc: i32,
        handler_pc: i32,
        catch_type: i32,
    ) {
        if startpc >= 0
            && startpc <= u16::MAX as i32
            && endpc >= 0
            && endpc <= u16::MAX as i32
            && handler_pc >= 0
            && handler_pc <= u16::MAX as i32
        {
            self.get_code().borrow_mut().add_catch(
                startpc as u16,
                endpc as u16,
                handler_pc as u16,
                catch_type as u16,
            );
        } else {
            crate::util::abstract_log::AbstractLog::error_at(
                &*self.log,
                pos,
                Error::from(errors::limit_code_too_large_for_try_stmt()),
            );
            self.nerrs += 1;
        }
        let _ = ctx;
    }
}

/// Set the position of a type compound attribute.
fn set_attr_position(
    ctx: &mut CompilerContext,
    attr: crate::ids::AttributeId,
    pos: crate::code::type_annotation_position::TypeAnnotationPosition,
) {
    if let crate::code::attribute::AttributeKind::TypeCompound { position, .. } =
        &mut ctx.attr_mut(attr).kind
    {
        *position = Some(pos);
    }
}

/// Return the type of a catch parameter.
fn catch_param_type(ctx: &mut CompilerContext, catch_tree: TreeId) -> TypeId {
    let param = match &ctx.tree(catch_tree).kind {
        TreeKind::Catch {
            param: Some(param), ..
        } => *param,
        _ => panic!("AssertionError: catch clause expected"),
    };
    let sym = match &ctx.tree(param).kind {
        TreeKind::VariableDecl { sym: Some(sym), .. } => *sym,
        _ => panic!("AssertionError: catch parameter symbol"),
    };
    ctx.sym(sym).ty.expect("catch parameter type")
}

impl Gen {
    /// Generate code for an if statement.
    pub fn gen_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let limit = self.get_code().borrow().nextreg;
        let mut then_exit = None;
        let (cond, thenpart, elsepart) = match &ctx.tree(tree).kind {
            TreeKind::If {
                cond: Some(cond),
                thenpart: Some(thenpart),
                elsepart,
            } => (*cond, *thenpart, *elsepart),
            _ => panic!("AssertionError: if statement expected"),
        };
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        let inner = tree_info::skip_parens(ctx, cond);
        let c = self.gen_cond_crt(ctx, inner, CRT_FLOW_CONTROLLER);
        let items = self.get_items();
        let else_chain = items.jump_false(ctx, &c);
        let cond_is_false = items.cond_is_false(&c);
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        if !cond_is_false {
            let true_jumps = items.cond_true_jumps(&c);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, true_jumps);
            self.gen_stat_crt(ctx, thenpart, CRT_STATEMENT | CRT_FLOW_TARGET);
            then_exit = self.get_code().borrow_mut().branch(ctx, GOTO);
        }
        if let Some(else_chain) = else_chain {
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, Some(else_chain));
            if let Some(elsepart) = elsepart {
                self.gen_stat_crt(ctx, elsepart, CRT_STATEMENT | CRT_FLOW_TARGET);
            }
        }
        self.get_code()
            .borrow_mut()
            .resolve_pending_chain(ctx, then_exit);
        self.get_code().borrow_mut().end_scopes(ctx, limit);
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
    }

    /// Generate code for an expression statement.
    pub fn gen_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::ExpressionStatement { expr: Some(expr) } => *expr,
            _ => panic!("AssertionError: expression statement expected"),
        };
        // Optimize x++ to ++x and x-- to --x.
        let tag = ctx.tree(expr).tag;
        if tag == Tag::POSTINC {
            ctx.tree_mut(expr).tag = Tag::PREINC;
        } else if tag == Tag::POSTDEC {
            ctx.tree_mut(expr).tag = Tag::PREDEC;
        }
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        let expr_ty = ctx.tree(expr).ty.expect("expression type");
        let item = self.gen_expr(ctx, expr, expr_ty);
        self.item_drop(ctx, &item);
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
    }

    /// Generate code for a break statement.
    pub fn gen_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        let target = match &ctx.tree(tree).kind {
            TreeKind::Break {
                target: Some(target),
                ..
            } => *target,
            _ => panic!("AssertionError: break target expected"),
        };
        let target_env = self.unwind_break(ctx, target);
        let branch = self.get_code().borrow_mut().branch(ctx, GOTO);
        target_env.info.borrow_mut().add_exit(branch);
        let env = self.env.clone().expect("generation environment");
        self.end_finalizer_gaps(ctx, &env, &target_env);
    }

    /// Generate code for a yield statement.
    pub fn gen_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        let target = match &ctx.tree(tree).kind {
            TreeKind::Yield {
                target: Some(target),
                ..
            } => *target,
            _ => panic!("AssertionError: yield target expected"),
        };
        if self.in_cond_switch_expression {
            let value = match &ctx.tree(tree).kind {
                TreeKind::Yield {
                    value: Some(value), ..
                } => *value,
                _ => panic!("AssertionError: yield value"),
            };
            let value = self.gen_cond_crt(ctx, value, CRT_FLOW_TARGET);
            let items = self.get_items();
            let false_jumps = items.jump_false(ctx, &value);
            let true_jumps = items.cond_true_jumps(&value);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, true_jumps);
            let local_env = self.unwind_break(ctx, target);
            self.reload_stack_before_switch_expr(ctx);
            let true_chain = self.get_code().borrow_mut().branch(ctx, GOTO);
            let env = self.env.clone().expect("generation environment");
            self.end_finalizer_gaps(ctx, &env, &local_env);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, false_jumps);
            let target_env = self.unwind_break(ctx, target);
            self.reload_stack_before_switch_expr(ctx);
            let false_chain = self.get_code().borrow_mut().branch(ctx, GOTO);
            self.switch_expression_true_chain =
                Code::merge_chains(self.switch_expression_true_chain.take(), true_chain);
            self.switch_expression_false_chain =
                Code::merge_chains(self.switch_expression_false_chain.take(), false_chain);
            self.end_finalizer_gaps(ctx, &env, &target_env);
        } else {
            let pt = self.pt.expect("expected type");
            let value = match &ctx.tree(tree).kind {
                TreeKind::Yield {
                    value: Some(value), ..
                } => *value,
                _ => panic!("AssertionError: yield value"),
            };
            let item = self.gen_expr(ctx, value, pt);
            self.item_load(ctx, &item);
            let switch_result = self.switch_result.clone();
            if let Some(switch_result) = &switch_result {
                self.item_store(ctx, switch_result);
            }
            let target_env = self.unwind_break(ctx, target);
            if self.get_code().borrow().is_alive() {
                self.reload_stack_before_switch_expr(ctx);
                if let Some(switch_result) = &self.switch_result.clone() {
                    self.item_load(ctx, switch_result);
                }
                let target_ty = ctx.tree(target).ty.expect("target type");
                let syms = Rc::clone(&self.syms);
                let types = Rc::clone(&self.types);
                self.get_code()
                    .borrow_mut()
                    .state
                    .force_stack_top(ctx, &*syms, &types, target_ty);
                let branch = self.get_code().borrow_mut().branch(ctx, GOTO);
                target_env.info.borrow_mut().add_exit(branch);
                self.get_code().borrow_mut().mark_dead();
            }
            let env = self.env.clone().expect("generation environment");
            self.end_finalizer_gaps(ctx, &env, &target_env);
        }
    }

    /// Unwind to the target environment of a break, preserving the pending
    /// statement position.
    fn unwind_break(&mut self, ctx: &mut CompilerContext, target: TreeId) -> Rc<Env<GenContext>> {
        let tmp_pos = self.get_code().borrow().pending_stat_pos;
        let env = self.env.clone().expect("generation environment");
        let target_env = self.unwind(ctx, target, &env);
        self.get_code().borrow_mut().pending_stat_pos = tmp_pos;
        target_env
    }

    /// Reload the stack items stashed before a switch expression.
    fn reload_stack_before_switch_expr(&mut self, ctx: &mut CompilerContext) {
        for item in self.stack_before_switch_expression.clone() {
            self.item_load(ctx, &item);
        }
    }

    /// Generate code for a continue statement.
    pub fn gen_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let tmp_pos = self.get_code().borrow().pending_stat_pos;
        let env = self.env.clone().expect("generation environment");
        let target = match &ctx.tree(tree).kind {
            TreeKind::Continue {
                target: Some(target),
                ..
            } => *target,
            _ => panic!("AssertionError: continue target expected"),
        };
        let target_env = self.unwind(ctx, target, &env);
        self.get_code().borrow_mut().pending_stat_pos = tmp_pos;
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        let branch = self.get_code().borrow_mut().branch(ctx, GOTO);
        target_env.info.borrow_mut().add_cont(branch);
        self.end_finalizer_gaps(ctx, &env, &target_env);
    }

    /// Generate code for a return statement.
    pub fn gen_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let limit = self.get_code().borrow().nextreg;
        let env = self.env.clone().expect("generation environment");
        let encl_method = env.encl_method.expect("enclosing method");
        let tmp_pos = self.get_code().borrow().pending_stat_pos;
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Return { expr, .. } => *expr,
            _ => panic!("AssertionError: return statement expected"),
        };
        let target_env;
        if let Some(expr) = expr {
            assert!(
                self.get_code().borrow().is_statement_start(),
                "AssertionError: not at statement start"
            );
            let pt = self.pt.expect("expected method return type");
            let item = self.gen_expr(ctx, expr, pt);
            let mut item = self.item_load(ctx, &item);
            let finally = self.has_finally(ctx, encl_method, &env);
            if finally {
                item = self.make_temp(ctx, pt);
                self.item_store(ctx, &item);
            }
            target_env = self.unwind(ctx, encl_method, &env);
            self.get_code().borrow_mut().pending_stat_pos = tmp_pos;
            self.item_load(ctx, &item);
            let tc = Code::typecode(ctx, pt);
            self.get_code()
                .borrow_mut()
                .emitop0(ctx, IRETURN + Code::truncate(tc));
        } else {
            target_env = self.unwind(ctx, encl_method, &env);
            self.get_code().borrow_mut().pending_stat_pos = tmp_pos;
            self.get_code().borrow_mut().emitop0(ctx, RETURN);
        }
        self.end_finalizer_gaps(ctx, &env, &target_env);
        self.get_code().borrow_mut().end_scopes(ctx, limit);
    }

    /// Generate code for a throw statement.
    pub fn gen_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Throw {
                expr: Some(expr), ..
            } => *expr,
            _ => panic!("AssertionError: throw expression expected"),
        };
        let expr_ty = ctx.tree(expr).ty.expect("throw type");
        let item = self.gen_expr(ctx, expr, expr_ty);
        self.item_load(ctx, &item);
        self.get_code().borrow_mut().emitop0(ctx, ATHROW);
        assert!(
            self.get_code().borrow().is_statement_start(),
            "AssertionError: not at statement start"
        );
    }
}

impl Gen {
    /// Generate code for a method invocation.
    pub fn gen_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.set_type_annotation_positions(ctx, ctx.tree(tree).pos);
        let (meth, args) = match &ctx.tree(tree).kind {
            TreeKind::MethodInvocation {
                meth: Some(meth),
                args,
                ..
            } => (*meth, args.clone()),
            _ => panic!("AssertionError: method invocation expected"),
        };
        let method_type = self.method_type;
        let m = self.gen_expr(ctx, meth, method_type);
        let msym = tree_info::symbol(ctx, meth).expect("method symbol");
        let external = {
            let syms = Rc::clone(&self.syms);
            let names = Rc::clone(&self.names);
            crate::code::symbol::external_type(ctx, &self.types, &*syms, &names, msym)
        };
        let params = ctx.typ(external).get_parameter_types(ctx);
        self.gen_args(ctx, args, params);
        if !self.is_invoke_dynamic(ctx, msym) {
            self.get_code().borrow_mut().stat_begin(ctx.tree(tree).pos);
        }
        self.result = Some(self.item_invoke(ctx, &m));
    }

    /// Generate code for a conditional expression.
    pub fn gen_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let mut then_exit = None;
        let (cond, truepart, falsepart) = match &ctx.tree(tree).kind {
            TreeKind::Conditional {
                cond: Some(cond),
                truepart: Some(truepart),
                falsepart: Some(falsepart),
                ..
            } => (*cond, *truepart, *falsepart),
            _ => panic!("AssertionError: conditional expression expected"),
        };
        self.get_code().borrow_mut().stat_begin(ctx.tree(cond).pos);
        let c = self.gen_cond_crt(ctx, cond, CRT_FLOW_CONTROLLER);
        let items = self.get_items();
        let else_chain = items.jump_false(ctx, &c);
        let cond_is_false = items.cond_is_false(&c);
        if std::env::var_os("DSH_DEBUG_COND").is_some() {
            let ct = items.cond_is_true(&c);
            if ct {
                eprintln!(
                    "[GENCOND] file={} pos={} cond_tag={:?} cond_ty={} c={}",
                    self.log
                        .current_source_file()
                        .map(|f| f.get_name().to_string_lossy().to_string())
                        .unwrap_or_default(),
                    ctx.tree(tree).pos,
                    ctx.tree(cond).get_tag(),
                    ctx.tree(cond)
                        .ty
                        .map(|ty| crate::comp::attr::type_display_of(ctx, ty))
                        .unwrap_or_default(),
                    items.describe(ctx, &c),
                );
            }
        }
        if !cond_is_false {
            let true_jumps = items.cond_true_jumps(&c);
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, true_jumps);
            let startpc = if self.gen_crt {
                self.get_code().borrow_mut().cur_cp(ctx)
            } else {
                0
            };
            self.get_code()
                .borrow_mut()
                .stat_begin(ctx.tree(truepart).pos);
            let pt = self.pt.expect("expected type");
            let item = self.gen_expr(ctx, truepart, pt);
            self.item_load(ctx, &item);
            let tree_ty = ctx.tree(tree).ty.expect("conditional type");
            let syms = Rc::clone(&self.syms);
            let types = Rc::clone(&self.types);
            self.get_code()
                .borrow_mut()
                .state
                .force_stack_top(ctx, &*syms, &types, tree_ty);
            if self.gen_crt {
                let end_pc = self.get_code().borrow_mut().cur_cp(ctx);
                if let Some(crt) = &mut self.get_code().borrow_mut().crt {
                    crt.put(CrtKey::Tree(truepart), CRT_FLOW_TARGET, startpc, end_pc);
                }
            }
            then_exit = self.get_code().borrow_mut().branch(ctx, GOTO);
        }
        if let Some(else_chain) = else_chain {
            self.get_code()
                .borrow_mut()
                .resolve_pending_chain(ctx, Some(else_chain));
            let startpc = if self.gen_crt {
                self.get_code().borrow_mut().cur_cp(ctx)
            } else {
                0
            };
            self.get_code()
                .borrow_mut()
                .stat_begin(ctx.tree(falsepart).pos);
            let pt = self.pt.expect("expected type");
            let item = self.gen_expr(ctx, falsepart, pt);
            self.item_load(ctx, &item);
            let tree_ty = ctx.tree(tree).ty.expect("conditional type");
            let syms = Rc::clone(&self.syms);
            let types = Rc::clone(&self.types);
            self.get_code()
                .borrow_mut()
                .state
                .force_stack_top(ctx, &*syms, &types, tree_ty);
            if self.gen_crt {
                let end_pc = self.get_code().borrow_mut().cur_cp(ctx);
                if let Some(crt) = &mut self.get_code().borrow_mut().crt {
                    crt.put(CrtKey::Tree(falsepart), CRT_FLOW_TARGET, startpc, end_pc);
                }
            }
        }
        self.get_code()
            .borrow_mut()
            .resolve_pending_chain(ctx, then_exit);
        let pt = self.pt.expect("expected type");
        let items = self.get_items();
        self.result = Some(items.make_stack_item(ctx, pt));
    }

    /// Set the type annotation positions of the generated code.
    fn set_type_annotation_positions(&mut self, ctx: &mut CompilerContext, tree_pos: i32) {
        let meth = match &self.code {
            Some(code) => code.borrow().meth,
            None => return,
        };
        let cp = self.get_code().borrow().cp;
        let meth_kind = ctx.sym(meth).get_kind(ctx);
        let init_or_clinit = meth_kind == crate::code::symbol::ElementKind::CONSTRUCTOR
            || meth_kind == crate::code::symbol::ElementKind::STATIC_INIT;
        fix_type_annotations(ctx, ctx.sym(meth).get_raw_type_attributes(), tree_pos, cp);
        if !init_or_clinit {
            return;
        }
        if let Some(owner) = ctx.sym(meth).owner {
            fix_type_annotations(ctx, ctx.sym(owner).get_raw_type_attributes(), tree_pos, cp);
        }
        let clazz = match crate::code::symbol::symbol_encl_class(ctx, meth) {
            Some(clazz) => clazz,
            None => return,
        };
        let members = ctx.sym(clazz).members();
        if let Some(members) = members {
            let scope_snapshot = ctx.scope(members).clone();
            let symbols = scope_snapshot.get_symbols(
                members,
                crate::code::scope::LookupKind::NON_RECURSIVE,
                &|_| true,
                ctx,
            );
            for symbol in symbols {
                if !ctx.sym(symbol).get_kind(ctx).is_field() {
                    continue;
                }
                fix_type_annotations(ctx, ctx.sym(symbol).get_raw_type_attributes(), tree_pos, cp);
            }
        }
    }

    /// Generate code for a new class instance.
    pub fn gen_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (args, constructor) = match &ctx.tree(tree).kind {
            TreeKind::NewClass {
                args,
                constructor: Some(constructor),
                ..
            } => (args.clone(), *constructor),
            _ => panic!("AssertionError: new class expected"),
        };
        self.set_type_annotation_positions(ctx, ctx.tree(tree).pos);
        let mut tree_ty = ctx.tree(tree).ty.expect("new class type");
        // A failed diamond attribution leaves the class tree with an <any>
        // result even though the selected constructor is valid (and its owner
        // carries the real class). Emit NEW against that owner.
        let tree_ty_invalid = ctx.typ(tree_ty).has_tag(TypeTag::ERROR)
            || ctx
                .typ(tree_ty)
                .tsym
                .map(|tsym| ctx.sym(tsym).name.to_java_string().to_string_lossy() == "<any>")
                .unwrap_or(false);
        if tree_ty_invalid
            && let Some(owner) = ctx.sym(constructor).owner
            && let Some(owner_ty) = ctx.sym(owner).ty
        {
            tree_ty = owner_ty;
        }
        let pos = ctx.tree(tree).pos;
        let od = self.make_ref(ctx, pos, tree_ty);
        self.get_code()
            .borrow_mut()
            .emitop2_type(ctx, NEW, od, tree_ty);
        self.get_code().borrow_mut().emitop0(ctx, DUP);
        let external = {
            let syms = Rc::clone(&self.syms);
            let names = Rc::clone(&self.names);
            crate::code::symbol::external_type(ctx, &self.types, &*syms, &names, constructor)
        };
        let params = ctx.typ(external).get_parameter_types(ctx);
        self.gen_args(ctx, args, params);
        let items = self.get_items();
        let item = items.make_member_item(ctx, constructor, true);
        items.invoke(ctx, &item);
        self.result = Some(items.make_stack_item(ctx, tree_ty));
    }

    /// Generate code for a new array creation.
    pub fn gen_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.set_type_annotation_positions(ctx, ctx.tree(tree).pos);
        let tree_ty = ctx.tree(tree).ty.expect("new array type");
        let elems = match &ctx.tree(tree).kind {
            TreeKind::NewArray { elems, .. } => elems.clone(),
            _ => panic!("AssertionError: new array expected"),
        };
        match elems {
            Some(elems) => {
                let syms = Rc::clone(&self.syms);
                let elemtype = self.types.elemtype(ctx, &*syms, tree_ty);
                let len = elems.length();
                self.load_int_const(ctx, len);
                let arr = self.make_new_array(ctx, ctx.tree(tree).pos, tree_ty, 1);
                let mut i = 0;
                let l = elems;
                for head in &l {
                    self.item_duplicate(ctx, &arr);
                    self.load_int_const(ctx, i);
                    i += 1;
                    let item = self.gen_expr(ctx, head, elemtype);
                    self.item_load(ctx, &item);
                    let items = self.get_items();
                    let indexed = items.make_indexed_item(ctx, elemtype);
                    items.store(ctx, &indexed);
                }
                self.result = Some(arr);
            }
            None => {
                let dims = match &ctx.tree(tree).kind {
                    TreeKind::NewArray { dims, .. } => dims.clone(),
                    _ => List::nil(),
                };
                let int_type = self.syms.int_type;
                let l = dims.clone();
                for head in &l {
                    let item = self.gen_expr(ctx, head, int_type);
                    self.item_load(ctx, &item);
                }
                let ndims = dims.length();
                self.result = Some(self.make_new_array(ctx, ctx.tree(tree).pos, tree_ty, ndims));
            }
        }
    }

    /// Generate code to create an array with the given element type and number
    /// of dimensions.
    fn make_new_array(
        &mut self,
        ctx: &mut CompilerContext,
        pos: i32,
        ty: TypeId,
        ndims: i32,
    ) -> Item {
        let syms = Rc::clone(&self.syms);
        let elemtype = self.types.elemtype(ctx, &*syms, ty);
        let dimensions = self.types.dimensions(ctx, &*syms, ty);
        if dimensions > class_file::MAX_DIMENSIONS {
            crate::util::abstract_log::AbstractLog::error_at(
                &*self.log,
                pos,
                Error::from(errors::limit_dimensions()),
            );
            self.nerrs += 1;
        }
        let elemcode = Code::arraycode(ctx, elemtype);
        if elemcode == 0 || (elemcode == 1 && ndims == 1) {
            let od = self.make_ref(ctx, pos, elemtype);
            self.get_code().borrow_mut().emit_anewarray(ctx, od, ty);
        } else if elemcode == 1 {
            let od = self.make_ref(ctx, pos, ty);
            self.get_code()
                .borrow_mut()
                .emit_multianewarray(ctx, ndims, od, ty);
        } else {
            self.get_code()
                .borrow_mut()
                .emit_newarray(ctx, elemcode, ty);
        }
        let items = self.get_items();
        items.make_stack_item(ctx, ty)
    }

    /// Generate code for a parenthesized expression.
    pub fn gen_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let expr = match &ctx.tree(tree).kind {
            TreeKind::Parens { expr: Some(expr) } => *expr,
            _ => panic!("AssertionError: parenthesized expression expected"),
        };
        let expr_ty = ctx.tree(expr).ty.expect("expression type");
        let item = self.gen_expr(ctx, expr, expr_ty);
        self.result = Some(item);
    }

    /// Generate code for an assignment.
    pub fn gen_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs) = match &ctx.tree(tree).kind {
            TreeKind::Assign {
                lhs: Some(lhs),
                rhs: Some(rhs),
            } => (*lhs, *rhs),
            _ => panic!("AssertionError: assignment expected"),
        };
        let lhs_ty = ctx.tree(lhs).ty.expect("assignment lhs type");
        let l = self.gen_expr(ctx, lhs, lhs_ty);
        let item = self.gen_expr(ctx, rhs, lhs_ty);
        self.item_load(ctx, &item);
        let rhs_ty = ctx.tree(rhs).ty.expect("assignment rhs type");
        if ctx.typ(rhs_ty).has_tag(TypeTag::BOT) {
            let lhs_ty = ctx.tree(lhs).ty.expect("assignment lhs type");
            let syms = Rc::clone(&self.syms);
            let types = Rc::clone(&self.types);
            self.get_code()
                .borrow_mut()
                .state
                .force_stack_top(ctx, &*syms, &types, lhs_ty);
        }
        let items = self.get_items();
        self.result = Some(items.make_assign_item(l));
    }

    /// Generate code for a compound assignment.
    pub fn gen_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs, operator) = match &ctx.tree(tree).kind {
            TreeKind::AssignOp {
                lhs: Some(lhs),
                rhs: Some(rhs),
                operator: Some(operator),
            } => (*lhs, *rhs, *operator),
            _ => panic!("AssertionError: compound assignment expected"),
        };
        let opcode = operator_opcode(ctx, operator);
        let l;
        if opcode == STRING_ADD {
            // Java keeps one mutable StringConcat on Gen; Rust cannot borrow it
            // and self mutably at once. Clone the lowering instead of taking
            // it, so a nested string concat inside the same expression still
            // finds concat present (Gen.java visitAssignop).
            let mut concat = self
                .concat
                .as_ref()
                .expect("string concat lowering")
                .clone();
            l = concat.make_concat_assignop(ctx, self, tree);
        } else {
            let lhs_ty = ctx.tree(lhs).ty.expect("assignment lhs type");
            l = self.gen_expr(ctx, lhs, lhs_ty);
            let tag = ctx.tree(tree).tag;
            if (tag == Tag::PLUS_ASG || tag == Tag::MINUS_ASG)
                && is_local_item(&l)
                && ctx.typ(lhs_ty).get_tag().is_sub_range_of(TypeTag::INT)
                && ctx
                    .typ(ctx.tree(rhs).ty.expect("rhs type"))
                    .get_tag()
                    .is_sub_range_of(TypeTag::INT)
                && ctx
                    .typ(ctx.tree(rhs).ty.expect("rhs type"))
                    .const_value()
                    .is_some()
            {
                let rhs_ty = ctx.tree(rhs).ty.expect("rhs type");
                let ival = match ctx.typ(rhs_ty).const_value() {
                    Some(crate::util::constants::ConstantValue::Integer(v)) => *v,
                    Some(crate::util::constants::ConstantValue::Byte(v)) => *v as i32,
                    Some(crate::util::constants::ConstantValue::Short(v)) => *v as i32,
                    Some(crate::util::constants::ConstantValue::Character(v)) => *v as i32,
                    Some(other) => panic!("AssertionError: integer constant expected: {:?}", other),
                    None => panic!("AssertionError: constant value expected"),
                };
                let ival = if tag == Tag::MINUS_ASG { -ival } else { ival };
                let items = self.get_items();
                items.incr(ctx, &l, ival);
                self.result = Some(l.clone());
                return;
            }
            self.item_duplicate(ctx, &l);
            let op_ty = ctx.sym(operator).ty.expect("operator type");
            let param1 = *ctx.typ(op_ty).get_parameter_types(ctx).head();
            let coerced = self.item_coerce_type(ctx, &l, param1);
            self.item_load(ctx, &coerced);
            let binop = self.complete_binop(ctx, lhs, rhs, operator);
            let lhs_ty = ctx.tree(lhs).ty.expect("assignment lhs type");
            let coerced = self.item_coerce_type(ctx, &binop, lhs_ty);
            let _ = coerced;
        }
        let items = self.get_items();
        self.result = Some(items.make_assign_item(l));
    }

    /// Generate code for a unary expression.
    pub fn gen_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (arg, operator) = match &ctx.tree(tree).kind {
            TreeKind::Unary {
                arg: Some(arg),
                operator: Some(operator),
            } => (*arg, *operator),
            _ => panic!("AssertionError: unary expression expected"),
        };
        let tag = ctx.tree(tree).tag;
        if tag == Tag::NOT {
            let cond = self.gen_cond(ctx, arg, false);
            let items = self.get_items();
            self.result = Some(items.negate(&cond));
            return;
        }
        let op_ty = ctx.sym(operator).ty.expect("operator type");
        let param1 = *ctx.typ(op_ty).get_parameter_types(ctx).head();
        let od = self.gen_expr(ctx, arg, param1);
        let opcode = operator_opcode(ctx, operator);
        match tag {
            Tag::POS => {
                self.result = Some(self.item_load(ctx, &od));
            }
            Tag::NEG => {
                let loaded = self.item_load(ctx, &od);
                self.get_code().borrow_mut().emitop0(ctx, opcode);
                self.result = Some(loaded);
            }
            Tag::COMPL => {
                let loaded = self.item_load(ctx, &od);
                self.emit_minus_one(ctx, od.typecode);
                self.get_code().borrow_mut().emitop0(ctx, opcode);
                self.result = Some(loaded);
            }
            Tag::PREINC | Tag::PREDEC => {
                self.item_duplicate(ctx, &od);
                if is_local_item(&od) && (opcode == IADD || opcode == ISUB) {
                    let delta = if tag == Tag::PREINC { 1 } else { -1 };
                    let items = self.get_items();
                    items.incr(ctx, &od, delta);
                    self.result = Some(od.clone());
                } else {
                    let _loaded = self.item_load(ctx, &od);
                    let one = Gen::one(od.typecode);
                    self.get_code().borrow_mut().emitop0(ctx, one);
                    self.get_code().borrow_mut().emitop0(ctx, opcode);
                    if od.typecode != INT_CODE && Code::truncate(od.typecode) == INT_CODE {
                        self.get_code()
                            .borrow_mut()
                            .emitop0(ctx, INT2BYTE + od.typecode - BYTE_CODE);
                    }
                    // Java: Gen.visitUnary PREINC/PREDEC wraps the original
                    // item (od), not the loaded stack value, in an AssignItem.
                    let items = self.get_items();
                    self.result = Some(items.make_assign_item(od.clone()));
                }
            }
            Tag::POSTINC | Tag::POSTDEC => {
                self.item_duplicate(ctx, &od);
                if is_local_item(&od) && (opcode == IADD || opcode == ISUB) {
                    let res = self.item_load(ctx, &od);
                    let delta = if tag == Tag::POSTINC { 1 } else { -1 };
                    let items = self.get_items();
                    items.incr(ctx, &od, delta);
                    self.result = Some(res);
                } else {
                    let res = self.item_load(ctx, &od);
                    let items = self.get_items();
                    items.stash(ctx, &od, od.typecode);
                    let one = Gen::one(od.typecode);
                    self.get_code().borrow_mut().emitop0(ctx, one);
                    self.get_code().borrow_mut().emitop0(ctx, opcode);
                    if od.typecode != INT_CODE && Code::truncate(od.typecode) == INT_CODE {
                        self.get_code()
                            .borrow_mut()
                            .emitop0(ctx, INT2BYTE + od.typecode - BYTE_CODE);
                    }
                    let items = self.get_items();
                    items.store(ctx, &od);
                    self.result = Some(res);
                }
            }
            Tag::NULLCHK => {
                let loaded = self.item_load(ctx, &od);
                self.get_code().borrow_mut().emitop0(ctx, DUP);
                self.gen_null_check(ctx, tree);
                self.result = Some(loaded);
            }
            _ => panic!("AssertionError: unexpected unary operator"),
        }
    }

    /// Generate a null check from the object value at the stack top.
    fn gen_null_check(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.get_code().borrow_mut().stat_begin(ctx.tree(tree).pos);
        let objects_type = self.syms.objects_type;
        let object_type = self.syms.object_type;
        let require_non_null = self.names.requireNonNull.clone();
        self.call_method(
            ctx,
            ctx.tree(tree).pos,
            objects_type,
            require_non_null,
            List::of(object_type),
            true,
        );
        self.get_code().borrow_mut().emitop0(ctx, POP);
    }

    /// Generate code for a binary expression.
    pub fn gen_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (lhs, rhs, operator) = match &ctx.tree(tree).kind {
            TreeKind::Binary {
                lhs: Some(lhs),
                rhs: Some(rhs),
                operator: Some(operator),
            } => (*lhs, *rhs, *operator),
            _ => panic!("AssertionError: binary expression expected"),
        };
        let opcode = operator_opcode(ctx, operator);
        if opcode == STRING_ADD {
            let mut concat = self
                .concat
                .as_ref()
                .expect("string concat lowering")
                .clone();
            let item = concat.make_concat_binary(ctx, self, tree);
            self.result = Some(item);
        } else if ctx.tree(tree).tag == Tag::AND {
            let lcond = self.gen_cond_crt(ctx, lhs, CRT_FLOW_CONTROLLER);
            let items = self.get_items();
            if !items.cond_is_false(&lcond) {
                let false_jumps = items.jump_false(ctx, &lcond);
                let true_jumps = items.cond_true_jumps(&lcond);
                self.get_code()
                    .borrow_mut()
                    .resolve_pending_chain(ctx, true_jumps);
                let rcond = self.gen_cond_crt(ctx, rhs, CRT_FLOW_TARGET);
                let rcond_opcode = items.cond_opcode(&rcond);
                let rcond_true = items.cond_true_jumps(&rcond);
                let rcond_false = items.cond_false_jumps(&rcond);
                let merged = Code::merge_chains(false_jumps, rcond_false);
                self.result = Some(items.make_cond_item(rcond_opcode, rcond_true, merged));
            } else {
                self.result = Some(lcond);
            }
        } else if ctx.tree(tree).tag == Tag::OR {
            let lcond = self.gen_cond_crt(ctx, lhs, CRT_FLOW_CONTROLLER);
            let items = self.get_items();
            if !items.cond_is_true(&lcond) {
                let true_jumps = items.jump_true(ctx, &lcond);
                let false_jumps = items.cond_false_jumps(&lcond);
                self.get_code()
                    .borrow_mut()
                    .resolve_pending_chain(ctx, false_jumps);
                let rcond = self.gen_cond_crt(ctx, rhs, CRT_FLOW_TARGET);
                let rcond_opcode = items.cond_opcode(&rcond);
                let rcond_true = items.cond_true_jumps(&rcond);
                let rcond_false = items.cond_false_jumps(&rcond);
                let merged = Code::merge_chains(true_jumps, rcond_true);
                self.result = Some(items.make_cond_item(rcond_opcode, merged, rcond_false));
            } else {
                self.result = Some(lcond);
            }
        } else {
            let op_ty = ctx.sym(operator).ty.expect("operator type");
            let param1 = *ctx.typ(op_ty).get_parameter_types(ctx).head();
            let od = self.gen_expr(ctx, lhs, param1);
            self.item_load(ctx, &od);
            self.result = Some(self.complete_binop(ctx, lhs, rhs, operator));
        }
    }

    /// Complete generating code for an operation whose left operand is already
    /// on the stack.
    fn complete_binop(
        &mut self,
        ctx: &mut CompilerContext,
        lhs: TreeId,
        rhs: TreeId,
        operator: SymbolId,
    ) -> Item {
        let _ = lhs;
        let op_ty = ctx.sym(operator).ty.expect("operator type");
        let syms = Rc::clone(&self.syms);
        let op_erased = self.types.erasure(ctx, &*syms, op_ty);
        let params = ctx.typ(op_erased).get_parameter_types(ctx);
        let mut opcode = operator_opcode(ctx, operator);
        let rhs_ty = ctx.tree(rhs).ty.expect("rhs type");
        if (IF_ICMPEQ..=IF_ICMPLE).contains(&opcode)
            && let Some(crate::util::constants::ConstantValue::Integer(number)) =
                ctx.typ(rhs_ty).const_value()
            && *number == 0
        {
            opcode += IFEQ - IF_ICMPEQ;
        } else if (IF_ACMPEQ..=IF_ACMPNE).contains(&opcode) && tree_info::is_null(ctx, rhs) {
            opcode += IF_ACMP_NULL - IF_ACMPEQ;
        } else {
            // The expected type of the right operand is the second parameter
            // type of the operator, except for shifts with a long count.
            let mut rtype = *params.tail().head();
            if (ISHLL..=LUSHRL).contains(&opcode) {
                opcode += ISHL - ISHLL;
                rtype = self.syms.int_type;
            }
            let item = self.gen_expr(ctx, rhs, rtype);
            self.item_load(ctx, &item);
            if opcode >= (1 << PRE_SHIFT) {
                self.get_code()
                    .borrow_mut()
                    .emitop0(ctx, opcode >> PRE_SHIFT);
                opcode &= 0xFF;
            }
        }
        if (IFEQ..=IF_ACMPNE).contains(&opcode)
            || opcode == IF_ACMP_NULL
            || opcode == IF_ACMP_NONNULL
        {
            let items = self.get_items();
            return items.make_cond_item_opcode(opcode);
        }
        self.get_code().borrow_mut().emitop0(ctx, opcode);
        let restype = ctx
            .typ(op_erased)
            .get_return_type(ctx)
            .expect("operator return type");
        let items = self.get_items();
        items.make_stack_item(ctx, restype)
    }

    /// Generate code for a type cast.
    pub fn gen_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (clazz, expr) = match &ctx.tree(tree).kind {
            TreeKind::TypeCast {
                clazz: Some(clazz),
                expr: Some(expr),
            } => (*clazz, *expr),
            _ => panic!("AssertionError: type cast expected"),
        };
        let clazz_ty = ctx.tree(clazz).ty.expect("cast type");
        let item = self.gen_expr(ctx, expr, clazz_ty);
        self.result = Some(self.item_load(ctx, &item));
        self.set_type_annotation_positions(ctx, ctx.tree(tree).pos);
        let expr_ty = ctx.tree(expr).ty.expect("cast expression type");
        let clazz_tsym = ctx.typ(clazz_ty).tsym.expect("cast type symbol");
        let syms = Rc::clone(&self.syms);
        let same = self.types.is_same_type(ctx, &*syms, expr_ty, clazz_ty);
        let super_ty = self.types.as_super(ctx, &*syms, expr_ty, clazz_tsym);
        if !ctx.typ(clazz_ty).is_primitive() && !same && super_ty.is_none() {
            let pos = ctx.tree(tree).pos;
            let od = self.make_ref(ctx, pos, clazz_ty);
            self.get_code()
                .borrow_mut()
                .emitop2_type(ctx, CHECKCAST, od, clazz_ty);
        }
    }

    /// Generate code for a wildcard (should not be reached).
    pub fn gen_wildcard(&mut self, _ctx: &mut CompilerContext, _tree: TreeId) {
        panic!("AssertionError: unexpected wildcard in code generation");
    }

    /// Generate code for an instanceof expression.
    pub fn gen_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (expr, pattern) = match &ctx.tree(tree).kind {
            TreeKind::InstanceOf {
                expr: Some(expr),
                pattern: Some(pattern),
            } => (*expr, *pattern),
            _ => panic!("AssertionError: instanceof expected"),
        };
        let expr_ty = ctx.tree(expr).ty.expect("instanceof expression type");
        let item = self.gen_expr(ctx, expr, expr_ty);
        self.item_load(ctx, &item);
        self.set_type_annotation_positions(ctx, ctx.tree(tree).pos);
        let pattern_ty = ctx.tree(pattern).ty.expect("pattern type");
        let pos = ctx.tree(tree).pos;
        let od = self.make_ref(ctx, pos, pattern_ty);
        self.get_code()
            .borrow_mut()
            .emitop2_type(ctx, INSTANCEOF, od, pattern_ty);
        let boolean_type = self.syms.boolean_type;
        let items = self.get_items();
        self.result = Some(items.make_stack_item(ctx, boolean_type));
    }

    /// Generate code for an array access.
    pub fn gen_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (indexed, index) = match &ctx.tree(tree).kind {
            TreeKind::ArrayAccess {
                indexed: Some(indexed),
                index: Some(index),
            } => (*indexed, *index),
            _ => panic!("AssertionError: array access expected"),
        };
        let indexed_ty = ctx.tree(indexed).ty.expect("array type");
        let item = self.gen_expr(ctx, indexed, indexed_ty);
        self.item_load(ctx, &item);
        let int_type = self.syms.int_type;
        let index_item = self.gen_expr(ctx, index, int_type);
        self.item_load(ctx, &index_item);
        let tree_ty = ctx.tree(tree).ty.expect("array element type");
        let items = self.get_items();
        self.result = Some(items.make_indexed_item(ctx, tree_ty));
    }

    /// Generate code for an identifier.
    pub fn gen_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (name, sym) = match &ctx.tree(tree).kind {
            TreeKind::Ident {
                name,
                sym: Some(sym),
            } => (name.clone(), *sym),
            _ => panic!("AssertionError: identifier expected"),
        };
        let names = Rc::clone(&self.names);
        if name == names._this || name == names._super {
            let mut res = if name == names._this {
                let items = self.get_items();
                items.make_this_item()
            } else {
                let items = self.get_items();
                items.make_super_item()
            };
            if ctx.sym(sym).kind == crate::code::kinds::Kind::MTH {
                // Generate code to address the constructor.
                self.item_load(ctx, &res);
                let items = self.get_items();
                res = items.make_member_item(ctx, sym, true);
            }
            self.result = Some(res);
            return;
        }
        if self.is_invoke_dynamic(ctx, sym) || self.is_constant_dynamic(ctx, sym) {
            if self.is_constant_dynamic(ctx, sym) {
                self.set_type_annotation_positions(ctx, ctx.tree(tree).pos);
            }
            let items = self.get_items();
            self.result = Some(items.make_dynamic_item(ctx, sym));
            return;
        }
        let owner_kind = ctx.sym(sym).owner.map(|owner| ctx.sym(owner).kind);
        if ctx.sym(sym).kind == crate::code::kinds::Kind::VAR
            && matches!(
                owner_kind,
                Some(crate::code::kinds::Kind::MTH) | Some(crate::code::kinds::Kind::VAR)
            )
        {
            let items = self.get_items();
            self.result = Some(items.make_local_item(ctx, sym));
            return;
        }
        let flags = ctx.sym(sym).flags_field;
        let env = self.env.clone().expect("generation environment");
        let encl_class = env.encl_class.expect("enclosing class");
        let encl_class_ty = match &ctx.tree(encl_class).kind {
            TreeKind::ClassDecl { sym: Some(cs), .. } => ctx.sym(*cs).ty.expect("class type"),
            _ => ctx
                .typ(self.syms.object_type)
                .tsym
                .map(|_| self.syms.object_type)
                .expect("class type"),
        };
        if (flags & crate::code::flags::STATIC as i64) != 0 {
            let encl_method = env.encl_method.expect("enclosing method");
            let sym = if !self.is_access_super(ctx, encl_method) {
                self.binary_qualifier(ctx, sym, encl_class_ty)
            } else {
                sym
            };
            let items = self.get_items();
            self.result = Some(items.make_static_item(ctx, sym));
        } else {
            let this_item = {
                let items = self.get_items();
                items.make_this_item()
            };
            self.item_load(ctx, &this_item);
            let sym = self.binary_qualifier(ctx, sym, encl_class_ty);
            let nonvirtual = self.non_virtual_for_private_access(ctx, sym);
            let items = self.get_items();
            self.result = Some(items.make_member_item(ctx, sym, nonvirtual));
        }
    }

    /// Return whether to use a non-virtual access for a private member.
    fn non_virtual_for_private_access(&self, ctx: &CompilerContext, sym: SymbolId) -> bool {
        let use_virtual =
            self.target.has_virtual_private_invoke() && !self.disable_virtualized_private_invoke;
        !use_virtual && (ctx.sym(sym).flags_field & crate::code::flags::PRIVATE as i64) != 0
    }

    /// Generate code for a field access.
    pub fn gen_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (selected, name, sym) = match &ctx.tree(tree).kind {
            TreeKind::FieldAccess {
                selected: Some(selected),
                name,
                sym: Some(sym),
            } => (*selected, name.clone(), *sym),
            _ => panic!("AssertionError: field access expected"),
        };
        let names = Rc::clone(&self.names);
        if name == names._class {
            let selected_ty = ctx.tree(selected).ty.expect("selected type");
            let pos = ctx.tree(tree).pos;
            let ty = self.check_dimension(ctx, pos, selected_ty);
            self.get_code()
                .borrow_mut()
                .emit_ldc(ctx, LoadableConstantRef::Class(ty));
            let pt = self.pt.expect("expected type");
            let items = self.get_items();
            self.result = Some(items.make_stack_item(ctx, pt));
            return;
        }
        let ssym = tree_info::symbol(ctx, selected);
        let select_super = match ssym {
            Some(ssym) => {
                ctx.sym(ssym).kind == crate::code::kinds::Kind::TYP
                    || ctx.sym(ssym).name == names._super
            }
            None => false,
        };
        let env = self.env.clone().expect("generation environment");
        let access_super = self.is_access_super(ctx, env.encl_method.expect("enclosing method"));
        let mut base = if select_super {
            let items = self.get_items();
            items.make_super_item()
        } else {
            let selected_ty = ctx.tree(selected).ty.expect("selected type");
            self.gen_expr(ctx, selected, selected_ty)
        };
        let is_const_var = ctx.sym(sym).kind == crate::code::kinds::Kind::VAR
            && ctx.sym(sym).get_const_value().is_some();
        if is_const_var {
            let flags = ctx.sym(sym).flags_field;
            if (flags & crate::code::flags::STATIC as i64) != 0 {
                let select_super_base = select_super;
                if !select_super_base
                    && (ssym.is_none()
                        || ssym
                            .map(|ssym| ctx.sym(ssym).kind != crate::code::kinds::Kind::TYP)
                            .unwrap_or(true))
                {
                    base = self.item_load(ctx, &base);
                }
                self.item_drop(ctx, &base);
            } else {
                let _loaded = self.item_load(ctx, &base);
                self.gen_null_check(ctx, selected);
            }
            let sym_ty = ctx.sym(sym).ty.expect("constant type");
            let const_value = ctx.sym(sym).get_const_value().expect("constant value");
            let items = self.get_items();
            self.result = Some(items.make_immediate_item(
                ctx,
                sym_ty,
                immediate_value_from_constant(&const_value),
            ));
            return;
        }
        if self.is_invoke_dynamic(ctx, sym) {
            let items = self.get_items();
            self.result = Some(items.make_dynamic_item(ctx, sym));
            return;
        }
        let selected_ty = ctx.tree(selected).ty.expect("selected type");
        let sym = self.binary_qualifier(ctx, sym, selected_ty);
        let flags = ctx.sym(sym).flags_field;
        if (flags & crate::code::flags::STATIC as i64) != 0 {
            let select_super_base = select_super;
            if !select_super_base
                && (ssym.is_none()
                    || ssym
                        .map(|ssym| ctx.sym(ssym).kind != crate::code::kinds::Kind::TYP)
                        .unwrap_or(true))
            {
                base = self.item_load(ctx, &base);
            }
            self.item_drop(ctx, &base);
            let items = self.get_items();
            self.result = Some(items.make_static_item(ctx, sym));
        } else {
            self.item_load(ctx, &base);
            if sym == self.syms.length_var {
                self.get_code().borrow_mut().emitop0(ctx, ARRAYLENGTH);
                let int_type = self.syms.int_type;
                let items = self.get_items();
                self.result = Some(items.make_stack_item(ctx, int_type));
            } else {
                let nonvirtual =
                    self.non_virtual_for_private_access(ctx, sym) || select_super || access_super;
                let items = self.get_items();
                self.result = Some(items.make_member_item(ctx, sym, nonvirtual));
            }
        }
    }

    /// Return whether a symbol is an invokedynamic method symbol.
    pub fn is_invoke_dynamic(&self, ctx: &CompilerContext, sym: SymbolId) -> bool {
        ctx.sym(sym).kind == crate::code::kinds::Kind::MTH
            && matches!(
                ctx.sym(sym).kind_data,
                crate::code::symbol::SymbolKind::DynamicMethodSymbol { .. }
            )
    }

    /// Generate code for a literal.
    pub fn gen_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        let (typetag, value) = match &ctx.tree(tree).kind {
            TreeKind::Literal { typetag, value } => (*typetag, value.clone()),
            _ => panic!("AssertionError: literal expected"),
        };
        let tree_ty = ctx.tree(tree).ty.expect("literal type");
        if typetag == TypeTag::BOT {
            self.get_code().borrow_mut().emitop0(ctx, ACONST_NULL);
            let items = self.get_items();
            self.result = Some(items.make_stack_item(ctx, tree_ty));
        } else {
            let value = value.expect("literal value");
            let items = self.get_items();
            self.result =
                Some(items.make_immediate_item(ctx, tree_ty, literal_value_to_immediate(&value)));
        }
    }

    /// Generate code for a let expression.
    pub fn gen_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.get_code().borrow_mut().resolve_pending(ctx);
        let limit = self.get_code().borrow().nextreg;
        let prev_start = {
            let stacksize = self.get_code().borrow().state.stacksize;
            self.get_code()
                .borrow_mut()
                .set_let_expr_stack_pos(stacksize)
        };
        let defs = match &ctx.tree(tree).kind {
            TreeKind::LetExpr { defs, .. } => defs.clone(),
            _ => panic!("AssertionError: let expression expected"),
        };
        self.gen_stats(ctx, defs);
        self.get_code()
            .borrow_mut()
            .set_let_expr_stack_pos(prev_start);
        let expr = match &ctx.tree(tree).kind {
            TreeKind::LetExpr {
                expr: Some(expr), ..
            } => *expr,
            _ => panic!("AssertionError: let expression body"),
        };
        let expr_ty = ctx.tree(expr).ty.expect("let expression type");
        let item = self.gen_expr(ctx, expr, expr_ty);
        self.result = Some(self.item_load(ctx, &item));
        self.get_code().borrow_mut().end_scopes(ctx, limit);
    }

    /// Enter the class references of the pruned trees of a class into the
    /// constant pool.
    fn generate_references_to_pruned_tree(
        &mut self,
        ctx: &mut CompilerContext,
        class_symbol: SymbolId,
    ) {
        let pruned = match &self.lower {
            Some(lower) => lower.borrow().pruned_tree.get(&class_symbol).cloned(),
            None => None,
        };
        if let Some(pruned) = pruned {
            for pruned_tree in pruned {
                self.class_reference_scan(ctx, pruned_tree);
            }
        }
    }

    /// Generate code for a class definition.
    ///
    /// # Arguments
    ///   - ctx: the compiler context;
    ///   - env: the attribution environment of the outermost class;
    ///   - cdef: the class definition tree.
    ///
    /// # Returns
    ///   - true when code was generated without errors.
    ///
    /// # Panics
    ///   - a generated item is missing.
    pub fn gen_class(
        &mut self,
        ctx: &mut CompilerContext,
        env: &Rc<Env<AttrContext>>,
        cdef: TreeId,
    ) -> bool {
        self.attr_env = Some(Rc::clone(env));
        let c = match &ctx.tree(cdef).kind {
            TreeKind::ClassDecl { sym: Some(sym), .. } => *sym,
            _ => panic!("AssertionError: class definition expected"),
        };
        let toplevel = env.toplevel.expect("toplevel");
        self.toplevel = Some(toplevel);
        self.end_pos_table = match &ctx.tree(toplevel).kind {
            TreeKind::CompilationUnit { end_positions, .. } => end_positions.clone(),
            _ => None,
        };
        // normalizeDefs can add references to external classes to the pool.
        let defs = match &ctx.tree(cdef).kind {
            TreeKind::ClassDecl { defs, .. } => defs.clone(),
            _ => List::nil(),
        };
        let defs = self.normalize_defs(ctx, defs, c);
        if let TreeKind::ClassDecl { defs: slot, .. } = &mut ctx.tree_mut(cdef).kind {
            *slot = defs.clone();
        }
        self.generate_references_to_pruned_tree(ctx, c);
        let local_env = Rc::new(Env {
            next: None,
            outer: None,
            tree: Some(cdef),
            toplevel: Some(toplevel),
            encl_class: Some(cdef),
            encl_method: None,
            info: Rc::new(RefCell::new(GenContext::new())),
            base_clause: false,
        });
        self.env = Some(Rc::clone(&local_env));
        let l = defs;
        for head in &l {
            self.gen_def(ctx, head);
        }
        let pool_size = self.pool_writer.borrow().size();
        if pool_size > PoolWriter::MAX_ENTRIES {
            crate::util::abstract_log::AbstractLog::error_at(
                &*self.log,
                ctx.tree(cdef).pos,
                Error::from(errors::limit_pool()),
            );
            self.nerrs += 1;
        }
        if self.nerrs != 0 {
            // If errors occurred, discard the generated code.
            // The method symbol code handle is not ported; the generated code
            // is discarded together with the generator state.
        }
        if let TreeKind::ClassDecl { defs: slot, .. } = &mut ctx.tree_mut(cdef).kind {
            *slot = List::nil();
        }
        let result = self.nerrs == 0;
        self.attr_env = None;
        self.env = None;
        self.toplevel = None;
        self.end_pos_table = None;
        self.nerrs = 0;
        self.qualified_symbol_cache.clear();
        result
    }
}

/// Fix the positions of a list of type compound attributes.
fn fix_type_annotations(
    ctx: &mut CompilerContext,
    attrs: List<crate::ids::AttributeId>,
    tree_pos: i32,
    cp: i32,
) {
    let current = attrs;
    for attr in &current {
        let mut attribute = ctx.attr(attr).clone();
        if attribute.has_unknown_position() && attribute.try_fix_position(ctx) {
            *ctx.attr_mut(attr) = attribute;
        }
        if let Some(mut pos) = ctx.attr(attr).get_position(ctx)
            && pos.matches_pos(tree_pos)
        {
            pos.update_pos_offset(cp);
            if let crate::code::attribute::AttributeKind::TypeCompound { position, .. } =
                &mut ctx.attr_mut(attr).kind
            {
                *position = Some(pos);
            }
        }
    }
}

/// Return the opcode of an operator symbol.
fn operator_opcode(ctx: &CompilerContext, operator: SymbolId) -> i32 {
    match &ctx.sym(operator).kind_data {
        crate::code::symbol::SymbolKind::OperatorSymbol { opcode, .. } => *opcode,
        _ => panic!("AssertionError: operator symbol expected"),
    }
}

/// Return whether an item is a local item.
fn is_local_item(item: &Item) -> bool {
    matches!(&item.kind, crate::jvm::items::ItemKind::Local { .. })
}

/// Convert a literal value to an immediate item value.
fn literal_value_to_immediate(value: &crate::tree::jc_tree::LiteralValue) -> ImmediateValue {
    use crate::tree::jc_tree::LiteralValue;
    match value {
        LiteralValue::Integer(v) => ImmediateValue::Int(*v),
        LiteralValue::Byte(v) => ImmediateValue::Int(i32::from(*v)),
        LiteralValue::Short(v) => ImmediateValue::Int(i32::from(*v)),
        LiteralValue::Long(v) => ImmediateValue::Long(*v),
        LiteralValue::Float(v) => ImmediateValue::Float(*v),
        LiteralValue::Double(v) => ImmediateValue::Double(*v),
        LiteralValue::Character(v) => ImmediateValue::Int(*v as i32),
        LiteralValue::Boolean(v) => ImmediateValue::Int(if *v { 1 } else { 0 }),
        LiteralValue::String(v) => ImmediateValue::String(v.clone()),
    }
}

impl TreeVisitor for Gen {
    fn visit_method_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_method_def(ctx, tree);
    }

    fn visit_var_def(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_var_def(ctx, tree);
    }

    fn visit_skip(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_skip(ctx, tree);
    }

    fn visit_block(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_block(ctx, tree);
    }

    fn visit_do_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_do_loop(ctx, tree);
    }

    fn visit_while_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_while_loop(ctx, tree);
    }

    fn visit_for_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_for_loop(ctx, tree);
    }

    fn visit_foreach_loop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_foreach_loop(ctx, tree);
    }

    fn visit_labelled(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_labelled(ctx, tree);
    }

    fn visit_switch(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_switch(ctx, tree);
    }

    fn visit_switch_expression(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_switch_expression(ctx, tree);
    }

    fn visit_synchronized(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_synchronized(ctx, tree);
    }

    fn visit_try(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_try_stmt(ctx, tree);
    }

    fn visit_conditional(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_conditional(ctx, tree);
    }

    fn visit_if(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_if(ctx, tree);
    }

    fn visit_exec(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_exec(ctx, tree);
    }

    fn visit_break(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_break(ctx, tree);
    }

    fn visit_yield(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_yield(ctx, tree);
    }

    fn visit_continue(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_continue(ctx, tree);
    }

    fn visit_return(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_return(ctx, tree);
    }

    fn visit_throw(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_throw(ctx, tree);
    }

    fn visit_apply(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_apply(ctx, tree);
    }

    fn visit_new_class(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_new_class(ctx, tree);
    }

    fn visit_new_array(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_new_array(ctx, tree);
    }

    fn visit_parens(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_parens(ctx, tree);
    }

    fn visit_assign(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_assign(ctx, tree);
    }

    fn visit_assignop(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_assignop(ctx, tree);
    }

    fn visit_unary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_unary(ctx, tree);
    }

    fn visit_binary(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_binary(ctx, tree);
    }

    fn visit_type_cast(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_type_cast(ctx, tree);
    }

    fn visit_type_test(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_type_test(ctx, tree);
    }

    fn visit_indexed(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_indexed(ctx, tree);
    }

    fn visit_select(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_select(ctx, tree);
    }

    fn visit_ident(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_ident(ctx, tree);
    }

    fn visit_literal(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_literal(ctx, tree);
    }

    fn visit_wildcard(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_wildcard(ctx, tree);
    }

    fn visit_let_expr(&mut self, ctx: &mut CompilerContext, tree: TreeId) {
        self.gen_let_expr(ctx, tree);
    }
}

/// Return whether a case label is an expression (as opposed to a pattern).
fn case_label_is_expression(ctx: &CompilerContext, label: TreeId) -> bool {
    !matches!(
        ctx.tree(label).get_tag(),
        Tag::BINDINGPATTERN | Tag::DEFAULTCASELABEL | Tag::GUARDPATTERN | Tag::PARENTHESIZEDPATTERN
    )
}

#[cfg(test)]
mod byte_short_immediate_tests {
    use super::{ImmediateValue, literal_value_to_immediate};
    use crate::tree::jc_tree::LiteralValue;

    #[test]
    fn byte_and_short_convert_to_int_immediates() {
        // Java: Types.constType(byte/short) is an int constant; Code emits an
        // int immediate (the wrapper is widened like Gen.visitLiteral).
        match literal_value_to_immediate(&LiteralValue::Byte(5)) {
            ImmediateValue::Int(value) => assert_eq!(value, 5),
            other => panic!("expected int immediate, got {other:?}"),
        }
        match literal_value_to_immediate(&LiteralValue::Short(-2)) {
            ImmediateValue::Int(value) => assert_eq!(value, -2),
            other => panic!("expected int immediate, got {other:?}"),
        }
    }
}

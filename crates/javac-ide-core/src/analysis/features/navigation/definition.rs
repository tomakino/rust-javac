//! Definition, type definition, and implementations.

use std::path::{Path, PathBuf};

use super::super::super::positions::{
    byte_to_utf16, in_literal_or_comment, line_offset, offset_to_line_col, word_at,
};
use super::super::super::{canonical, FileAnalysis};
use super::is_system_module_class;
use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::compiler_context::CompilerContext;
use crate::ids::{SymbolId, TreeId};
use crate::tree::jc_tree::TreeKind;
use crate::tree::tree_scanner::TreeScannerVisitor;

/// A `super(...)` constructor call: the `super` identifier and the number of
/// arguments written at the call site.
struct SuperConstructorCall {
    method: TreeId,
    argument_count: usize,
}

/// A `new T(...)` call under the cursor: the type name and the constructor the
/// compiler selected for this argument list.
struct NewClassCall {
    class: SymbolId,
    constructor: SymbolId,
    /// `new T() { ... }`: the definition is the type, not the anonymous
    /// constructor generated for the class body.
    anonymous: bool,
}

/// Finds the method declaration whose body contains `target`.
struct EnclosingMethodFinder {
    target: TreeId,
    methods: Vec<SymbolId>,
    found: Option<SymbolId>,
}

impl TreeScannerVisitor for EnclosingMethodFinder {
    fn scan(&mut self, ctx: &mut crate::compiler_context::CompilerContext, tree: Option<TreeId>) {
        let Some(tree) = tree else {
            return;
        };
        if self.found.is_some() {
            return;
        }
        let method = match &ctx.tree(tree).kind {
            TreeKind::MethodDecl { sym: Some(sym), .. } => Some(*sym),
            _ => None,
        };
        if let Some(method) = method {
            self.methods.push(method);
        }
        if tree == self.target {
            self.found = self.methods.last().copied();
        } else {
            crate::tree::jc_tree::accept(ctx, tree, self);
        }
        if method.is_some() {
            self.methods.pop();
        }
    }
}

crate::tree::tree_scanner::tree_scanner_visitor_delegate!(EnclosingMethodFinder);

impl FileAnalysis {
    /// Go to definition.
    pub(crate) fn definition_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Location> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())?;
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        // Strings and comments are not code: no navigation from there.
        if in_literal_or_comment(&text, line_offset(&text, line, column)) {
            return None;
        }
        // `super(...)` calls the superclass constructor. The compiler already
        // attributed the `super` identifier to that constructor, so the
        // definition is the constructor declaration (never the `super` token).
        if let Some(call) = self.super_constructor_call_at(&text, position) {
            return self.super_constructor_location(&call);
        }
        // `new T(...)` names a constructor call: the class name is the type,
        // but the editor definition is the selected constructor overload.
        if let Some(call) = self.new_class_call_at(&text, position) {
            if !call.anonymous
                && let Some(location) = self.constructor_definition(call.constructor)
            {
                return Some(location);
            }
            return self.declaration_location(call.class, None);
        }
        // Compiler-attributed symbol first: parameters/locals must win over a
        // same-named field. Only symbols declared in this arena (an AST name
        // position exists) are answered here; class-path symbols keep the
        // external-source path below.
        if let Some((symbol, _)) = self.symbol_id_at(file, position) {
            // A constructor symbol (for example `T::new`) has no usable name
            // position on its own tree; resolve its declaration directly.
            if self.is_constructor_symbol(symbol)
                && let Some(location) = self.constructor_definition(symbol)
            {
                return Some(location);
            }
            if let Some(pos) =
                crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, symbol)
            {
                let (symbol_line, symbol_column) = offset_to_line_col(&text, pos as usize);
                return Some(javac_analysis_api::Location {
                    file: file.to_path_buf(),
                    range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(
                        symbol_line,
                        symbol_column,
                    )),
                });
            }
            if let Some(location) = self.external_symbol_location(symbol) {
                return Some(location);
            }
            // Type variables have no declaring tree in `symbol_trees`; their
            // symbol position is the `TypeParameter` name token.
            if matches!(
                self.compiler.ctx.sym(symbol).kind_data,
                crate::code::symbol::SymbolKind::TypeVariableSymbol
            ) {
                // Type variables have no declaring tree in `symbol_trees`; find
                // the `TypeParameter` declaration by name (AST-first).
                let name = self
                    .compiler
                    .ctx
                    .sym(symbol)
                    .name
                    .to_java_string()
                    .to_string_lossy();
                for index in 0..self.compiler.ctx.trees.len() {
                    let tree = crate::ids::TreeId::new(index as u32);
                    if let crate::tree::jc_tree::TreeKind::TypeParameter {
                        name: parameter_name,
                        ..
                    } = &self.compiler.ctx.tree(tree).kind
                    {
                        if parameter_name.to_java_string().to_string_lossy() == name {
                            let pos = self.compiler.ctx.tree(tree).pos;
                            if pos >= 0 {
                                let (symbol_line, symbol_column) =
                                    offset_to_line_col(&text, pos as usize);
                                return Some(javac_analysis_api::Location {
                                    file: file.to_path_buf(),
                                    range: javac_analysis_api::Range::point(
                                        javac_analysis_api::Position::new(
                                            symbol_line,
                                            symbol_column,
                                        ),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }
        if let Some(symbol) = FileAnalysis::symbol_at(self, file, line, column) {
            if symbol.pos >= 0 {
                let (symbol_line, symbol_column) =
                    offset_to_line_col(&text, symbol.pos as usize);
                return Some(javac_analysis_api::Location {
                    file: file.to_path_buf(),
                    range: javac_analysis_api::Range::point(
                        javac_analysis_api::Position::new(symbol_line, symbol_column),
                    ),
                });
            }
        }
        // A symbol that resolves through the compiler but is not declared in
        // project sources may live in a classpath jar; use its sources jar.
        if let Some((symbol, _)) = self.symbol_id_at(file, position) {
            if let Some(location) = self.external_symbol_location(symbol) {
                return Some(location);
            }
        }
        let word = word_at(&text, line, column)?;
        if let Some(jdk_src) = super::super::super::jdk_sources::default_jdk_src() {
            return super::super::super::jdk_sources::jdk_source_location(
                &jdk_src,
                &word,
                &self.config.output_dir,
            );
        }
        None
    }

    /// A `super(...)` constructor call under the cursor, when there is one.
    ///
    /// Only the bare form is a constructor call: `super.m()` / `X.super()`
    /// use a field-access method expression and are member selections. The
    /// match is on the attributed compiler tree, not on the raw text.
    fn super_constructor_call_at(
        &self,
        text: &str,
        position: javac_analysis_api::Position,
    ) -> Option<SuperConstructorCall> {
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        if word_at(text, line, column)? != "super" {
            return None;
        }
        // `line_offset`/`word_start_offset` work in bytes; compiler tree
        // positions are Java char (UTF-16) offsets.
        let cursor = line_offset(text, line, column);
        let start =
            byte_to_utf16(text, super::source::word_start_offset(text, cursor)) as i32;
        let ctx = &self.compiler.ctx;
        for tree in ctx.trees.iter() {
            let TreeKind::MethodInvocation {
                meth: Some(method),
                args,
                ..
            } = &tree.kind
            else {
                continue;
            };
            if !matches!(&ctx.tree(*method).kind, TreeKind::Ident { .. }) {
                continue;
            }
            if ctx.tree(*method).pos != start {
                continue;
            }
            let is_super = crate::tree::tree_info::name(ctx, *method)
                .is_some_and(|name| name.to_java_string().to_string_lossy() == "super");
            if !is_super {
                continue;
            }
            return Some(SuperConstructorCall {
                method: *method,
                argument_count: args.size().max(0) as usize,
            });
        }
        None
    }

    /// Declaration of the superclass constructor called by `super(...)`.
    fn super_constructor_location(
        &mut self,
        call: &SuperConstructorCall,
    ) -> Option<javac_analysis_api::Location> {
        // Prefer the constructor Attr resolved for `super(...)`: it is the
        // exact overload, including parents from the JDK/class path whose
        // declaration has no tree in this arena.
        let resolved = crate::tree::tree_info::symbol(&self.compiler.ctx, call.method).filter(
            |symbol| {
                matches!(
                    self.compiler.ctx.sym(*symbol).kind_data,
                    SymbolKind::MethodSymbol { .. }
                )
            },
        );
        let (parent, constructor) = match resolved {
            Some(constructor) => {
                let parent = self.compiler.ctx.sym(constructor).owner?;
                if !matches!(
                    self.compiler.ctx.sym(parent).kind_data,
                    SymbolKind::ClassSymbol { .. }
                ) {
                    return None;
                }
                (parent, Some(constructor))
            }
            // Unattributed (partial/broken source): walk the declaration tree
            // to the enclosing class, take its supertype, and search that
            // class's constructors by the call's argument count.
            None => {
                let current = self.enclosing_class_of(call.method)?;
                let parent = self.direct_superclass(current)?;
                let constructor = self.constructor_with_arity(parent, call.argument_count);
                (parent, constructor)
            }
        };
        if let Some(constructor) = constructor
            && let Some(location) =
                self.declaration_location(constructor, Some(call.argument_count))
        {
            return Some(location);
        }
        // Degrade to the parent class declaration instead of returning None.
        self.declaration_location(parent, None)
    }

    /// A `new T(...)` expression under the cursor, when the cursor is on `T`.
    fn new_class_call_at(
        &self,
        text: &str,
        position: javac_analysis_api::Position,
    ) -> Option<NewClassCall> {
        let line = position.line as usize + 1;
        let column = position.column as usize + 1;
        word_at(text, line, column)?;
        let cursor = line_offset(text, line, column);
        let start =
            byte_to_utf16(text, super::source::word_start_offset(text, cursor)) as i32;
        for tree in self.compiler.ctx.trees.iter() {
            let TreeKind::NewClass {
                clazz: Some(clazz),
                constructor: Some(constructor),
                def,
                ..
            } = &tree.kind
            else {
                continue;
            };
            let Some(class_name) =
                crate::analysis::positions::type_name_position(&self.compiler.ctx, text, *clazz)
            else {
                continue;
            };
            if class_name != start {
                continue;
            }
            let class = crate::tree::tree_info::symbol(&self.compiler.ctx, *clazz)
                .or_else(|| self.compiler.ctx.sym(*constructor).owner)?;
            // An anonymous class body gets a constructor owned by the anonymous
            // class; the `new T(){}` definition is still T.
            let anonymous = def.is_some()
                || self.compiler.ctx.sym(*constructor).owner != Some(class);
            return Some(NewClassCall {
                class,
                constructor: *constructor,
                anonymous,
            });
        }
        None
    }

    /// Whether `symbol` is a constructor (`<init>`).
    fn is_constructor_symbol(&self, symbol: SymbolId) -> bool {
        matches!(
            self.compiler.ctx.sym(symbol).kind_data,
            SymbolKind::MethodSymbol { .. }
        ) && self
            .compiler
            .ctx
            .sym(symbol)
            .name
            .to_java_string()
            .to_string_lossy()
            == "<init>"
    }

    /// Declaration of a constructor symbol; falls back to its class declaration
    /// when the constructor itself cannot be located.
    fn constructor_definition(
        &mut self,
        constructor: SymbolId,
    ) -> Option<javac_analysis_api::Location> {
        let arity = match &self.compiler.ctx.sym(constructor).kind_data {
            SymbolKind::MethodSymbol { params, .. } => params.size().max(0) as usize,
            _ => return None,
        };
        if let Some(location) = self.declaration_location(constructor, Some(arity)) {
            return Some(location);
        }
        let owner = self.compiler.ctx.sym(constructor).owner?;
        self.declaration_location(owner, None)
    }

    /// Location of a declared symbol: the AST name position when its
    /// declaration tree is in this arena, otherwise the external sources.
    fn declaration_location(
        &mut self,
        symbol: SymbolId,
        arity: Option<usize>,
    ) -> Option<javac_analysis_api::Location> {
        if let Some(location) = self.arena_declaration_location(symbol) {
            return Some(location);
        }
        self.external_declaration_location(symbol, arity)
    }

    /// Name position of a declaration tree in the current arena (project
    /// sources, including overlays). `None` for class-path/JDK symbols: their
    /// declaration has no tree here.
    fn arena_declaration_location(
        &self,
        symbol: SymbolId,
    ) -> Option<javac_analysis_api::Location> {
        let position =
            crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, symbol)?;
        let source = self.declaring_class(symbol).and_then(|class| {
            match &self.compiler.ctx.sym(class).kind_data {
                SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile.clone(),
                _ => None,
            }
        })?;
        let path = PathBuf::from(source.get_name().to_string_lossy());
        let key = PathBuf::from(canonical(&path));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(&path).ok())?;
        let (line, column) = offset_to_line_col(&text, position as usize);
        Some(javac_analysis_api::Location {
            file: path,
            range: javac_analysis_api::Range::point(javac_analysis_api::Position::new(
                line, column,
            )),
        })
    }

    /// Location of a symbol declared outside the project sources: JDK classes
    /// resolve through `src.zip`, class-path classes through `-sources.jar`.
    ///
    /// `arity` selects a constructor overload by declared parameter count.
    fn external_declaration_location(
        &mut self,
        symbol: SymbolId,
        arity: Option<usize>,
    ) -> Option<javac_analysis_api::Location> {
        let class = self.declaring_class(symbol)?;
        let is_constructor = self
            .compiler
            .ctx
            .sym(symbol)
            .name
            .to_java_string()
            .to_string_lossy()
            == "<init>";
        let flat = self.compiler.ctx.sym(class).flat_name().to_string();
        let simple = super::super::super::source_files::simple_class_name(&flat);
        let is_jdk = is_system_module_class(&mut self.compiler.ctx, class);
        if is_jdk {
            let jdk_src = super::super::super::jdk_sources::default_jdk_src()?;
            if is_constructor
                && let Some(arity) = arity
                && let Some(location) =
                    super::super::super::jdk_sources::jdk_constructor_location(
                        &jdk_src,
                        &flat,
                        arity,
                        &self.config.output_dir,
                    )
            {
                return Some(location);
            }
            return super::super::super::jdk_sources::jdk_class_location(
                &jdk_src,
                &flat,
                &self.config.output_dir,
            );
        }
        if is_constructor
            && let Some(arity) = arity
            && let Some(location) =
                crate::analysis::external_sources::external_constructor_location(
                    &flat,
                    arity,
                    &self.config.class_path,
                    &self.config.output_dir,
                )
        {
            return Some(location);
        }
        crate::analysis::external_sources::external_source_location(
            &flat,
            simple,
            &self.config.class_path,
            &self.config.output_dir,
        )
    }

    /// The class symbol that declares `symbol`, following the owner chain.
    fn declaring_class(&self, symbol: SymbolId) -> Option<SymbolId> {
        let mut current = symbol;
        for _ in 0..16 {
            let sym = self.compiler.ctx.sym(current);
            if matches!(sym.kind_data, SymbolKind::ClassSymbol { .. }) {
                return Some(current);
            }
            current = sym.owner?;
        }
        None
    }

    /// The class symbol of the method declaration whose body contains
    /// `target` (the `super(...)` call is always inside a constructor).
    fn enclosing_class_of(&mut self, target: TreeId) -> Option<SymbolId> {
        let mut finder = EnclosingMethodFinder {
            target,
            methods: Vec::new(),
            found: None,
        };
        for index in 0..self.compiler.ctx.trees.len() {
            let unit = TreeId::new(index as u32);
            if !matches!(
                self.compiler.ctx.tree(unit).kind,
                TreeKind::CompilationUnit { .. }
            ) {
                continue;
            }
            finder.scan(&mut self.compiler.ctx, Some(unit));
            if finder.found.is_some() {
                break;
            }
        }
        let method = finder.found?;
        let owner = self.compiler.ctx.sym(method).owner?;
        matches!(
            self.compiler.ctx.sym(owner).kind_data,
            SymbolKind::ClassSymbol { .. }
        )
        .then_some(owner)
    }

    /// The direct superclass of `class` (the `super(...)` target site).
    fn direct_superclass(&mut self, class: SymbolId) -> Option<SymbolId> {
        let ty = self.compiler.ctx.sym(class).ty?;
        let types = self.compiler.types.clone();
        let syms = self.compiler.syms.clone();
        let super_ty = types.supertype(&mut self.compiler.ctx, &syms, ty, None);
        self.compiler.ctx.typ(super_ty).tsym
    }

    /// The unique constructor of `class` with `arity` declared parameters.
    fn constructor_with_arity(&mut self, class: SymbolId, arity: usize) -> Option<SymbolId> {
        let scope = self.compiler.ctx.sym(class).members()?;
        let members =
            self.compiler
                .ctx
                .scope_get_symbols(scope, LookupKind::NON_RECURSIVE, &|_| true);
        let mut matching = members.into_iter().filter(|member| {
            let symbol = self.compiler.ctx.sym(*member);
            let SymbolKind::MethodSymbol { params, .. } = &symbol.kind_data else {
                return false;
            };
            symbol.name.to_java_string().to_string_lossy() == "<init>"
                && params.size().max(0) as usize == arity
        });
        let first = matching.next()?;
        // Same-arity overloads cannot be told apart without attribution.
        matching.next().is_none().then_some(first)
    }

    fn external_symbol_location(
        &self,
        symbol: crate::ids::SymbolId,
    ) -> Option<javac_analysis_api::Location> {
        // Only class members resolve through external sources. Parameters and
        // locals belong to their method; matching them by name against a JDK
        // source file would pick a same-named field (Writer.java `lock`).
        let value_kind = &self.compiler.ctx.sym(symbol).kind_data;
        if matches!(value_kind, crate::code::symbol::SymbolKind::VarSymbol { .. }) {
            let owner_is_method = self.compiler.ctx.sym(symbol).owner.is_some_and(|owner| {
                matches!(
                    self.compiler.ctx.sym(owner).kind_data,
                    crate::code::symbol::SymbolKind::MethodSymbol { .. }
                )
            });
            if owner_is_method {
                return None;
            }
        }
        let mut current = symbol;
        for _ in 0..16 {
            let sym = self.compiler.ctx.sym(current);
            if let crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } = &sym.kind_data
            {
                let flat = sym.flat_name().to_string();
                // The member to locate is the symbol under the cursor (a method,
                // field, or the class itself), not the enclosing class name.
                let member = self
                    .compiler
                    .ctx
                    .sym(symbol)
                    .name
                    .to_java_string()
                    .to_string_lossy();
                let in_project = sourcefile.as_ref().is_some_and(|file| {
                    let name = file.get_name().to_string_lossy();
                    self.config
                        .source_path
                        .iter()
                        .any(|root| name.contains(&root.to_string_lossy().to_string()))
                });
                if in_project {
                    return None;
                }
                // JDK classes come from src.zip; other jars from their
                // `-sources.jar` on the class path.
                if let Some(jdk_src) = super::super::super::jdk_sources::default_jdk_src() {
                    if let Some(location) =
                        super::super::super::jdk_sources::jdk_member_location(
                            &jdk_src,
                            &flat,
                            &member,
                            &self.config.output_dir,
                        )
                    {
                        return Some(location);
                    }
                }
                return crate::analysis::external_sources::external_source_location(
                    &flat,
                    &member,
                    &self.config.class_path,
                    &self.config.output_dir,
                );
            }
            current = sym.owner?;
        }
        None
    }

    /// Go to the type of a variable or method.
    pub(crate) fn type_definition_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Option<javac_analysis_api::Location> {
        use crate::code::type_::TypeKindData;
        // The compiler symbol works for locals and parameters too, which are
        // not part of `symbols()` (declared members only).
        let (symbol, _) = self.symbol_id_at(file, position)?;
        let ty = self.compiler.ctx.sym(symbol).ty?;
        let class = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::ClassType { .. } => self.compiler.ctx.typ(ty).tsym,
            _ => None,
        }?;
        let class_pos = self.compiler.ctx.sym(class).pos.get();
        let class_name = self
            .compiler
            .ctx
            .sym(class)
            .name
            .to_java_string()
            .to_string_lossy()
            .to_string();
        let sourcefile = match &self.compiler.ctx.sym(class).kind_data {
            crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                .as_ref()
                .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
            _ => None,
        };
        if let Some(path) = sourcefile {
            if class_pos >= 0 {
                let pos = crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, class)
                    .map(|position| position as i32)
                    .unwrap_or(class_pos);
                return self.location_for(&path, pos);
            }
        }
        if let Some(jdk_src) = super::super::super::jdk_sources::default_jdk_src() {
            let name = self
                .compiler
                .ctx
                .sym(class)
                .name
                .to_java_string()
                .to_string_lossy();
            return super::super::super::jdk_sources::jdk_source_location(
                &jdk_src,
                &name,
                &self.config.output_dir,
            );
        }
        None
    }

    /// Implementations of the interface/class at a position.
    pub(crate) fn implementation_impl(
        &mut self,
        file: &Path,
        position: javac_analysis_api::Position,
    ) -> Vec<javac_analysis_api::Location> {
        let key = PathBuf::from(canonical(file));
        let text = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let Some(word) = word_at(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        ) else {
            return Vec::new();
        };
        let Some(target) = self.class_symbol_named(&word) else {
            return Vec::new();
        };
        let mut locations = Vec::new();
        for tree in self.compiler.ctx.trees.iter() {
            let Ok((sym, extending, implementing)) = (|| {
                match &tree.kind {
                    crate::tree::jc_tree::TreeKind::ClassDecl {
                        sym: Some(sym),
                        extending,
                        implementing,
                        ..
                    } => Ok((*sym, *extending, implementing.clone())),
                    _ => Err(()),
                }
            })() else {
                continue;
            };
            if sym == target {
                continue;
            }
            let supertrees: Vec<crate::ids::TreeId> =
                extending.iter().copied().chain(implementing.iter()).collect();
            let target_flat = self.compiler.ctx.sym(target).flat_name().to_string();
            let mut implements_target = false;
            for supertree in supertrees {
                if let Some(ty) = self.compiler.ctx.tree(supertree).ty {
                    let matches = self
                        .compiler
                        .ctx
                        .typ(ty)
                        .tsym
                        .map(|tsym| self.compiler.ctx.sym(tsym).flat_name().to_string() == target_flat)
                        .unwrap_or(false);
                    if matches {
                        implements_target = true;
                        break;
                    }
                }
            }
            if !implements_target {
                continue;
            }
            let class = self.compiler.ctx.sym(sym);
            let class_name = class
                .name
                .to_java_string()
                .to_string_lossy()
                .to_string();
            let pos = class.pos.get();
            let sourcefile = match &class.kind_data {
                crate::code::symbol::SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile
                    .as_ref()
                    .map(|file| PathBuf::from(file.get_name().to_string_lossy())),
                _ => None,
            };
            if let (Some(path), true) = (sourcefile, pos >= 0) {
                let pos = crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, sym)
                    .map(|position| position as i32)
                    .unwrap_or(pos);
                if let Some(location) = self.location_for(&path, pos) {
                    locations.push(location);
                }
            }
        }
        locations
    }
}




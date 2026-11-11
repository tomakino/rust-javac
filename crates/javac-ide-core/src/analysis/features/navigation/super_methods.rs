//! Go to the overridden (super) methods of a method.
//!
//! The match is semantic: erased parameter types are compared on the parent
//! type *site* (`Types::member_type`), so generic supertypes, varargs, bridge
//! methods, and multiple interfaces are handled by the compiler's types rather
//! than by name/arity heuristics.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use javac_analysis_api::{Location, Position, Range};

use super::super::super::{canonical, FileAnalysis};
use super::is_system_module_class;
use crate::analysis::positions::{
    in_literal_or_comment, line_offset, offset_to_line_col,
};
use crate::analysis::type_names::type_name;
use crate::code::flags;
use crate::code::scope::LookupKind;
use crate::code::symbol::SymbolKind;
use crate::code::type_::TypeKindData;
use crate::ids::{SymbolId, TypeId};

/// A supertype together with the type arguments it was instantiated with.
struct SuperSite {
    type_id: TypeId,
    class: SymbolId,
}

/// Erased signature used for override matching.
struct ErasedSignature {
    name: String,
    params: Vec<String>,
}

impl FileAnalysis {
    /// Declarations of the methods overridden by the method at a position.
    pub(crate) fn super_methods_impl(&mut self, file: &Path, position: Position) -> Vec<Location> {
        self.super_methods_with_names_impl(file, position)
            .into_iter()
            .map(|(location, _)| location)
            .collect()
    }

    /// Same as [`Self::super_methods_impl`], plus a display name per parent.
    pub(crate) fn super_methods_with_names_impl(
        &mut self,
        file: &Path,
        position: Position,
    ) -> Vec<(Location, String)> {
        if self.position_in_literal_or_comment(file, position) {
            return Vec::new();
        }
        let Some((symbol, _)) = self.symbol_id_at(file, position) else {
            return Vec::new();
        };
        // Static/private methods do not override anything.
        let child_flags = self.compiler.ctx.sym(symbol).flags_field;
        if (child_flags & flags::STATIC as i64) != 0 || (child_flags & flags::PRIVATE as i64) != 0
        {
            return Vec::new();
        }
        let Some(signature) = self.erased_signature_of(symbol) else {
            return Vec::new();
        };
        let Some(owner) = self.compiler.ctx.sym(symbol).owner else {
            return Vec::new();
        };
        let sites = self.supertype_closure(owner);
        let mut results = Vec::new();
        for site in &sites {
            if let Some(method) = self.find_matching_method(site, &signature)
                && let Some(location) = self.method_location(method)
            {
                results.push((location, self.override_display_name(method)));
            }
        }
        results.sort_by_key(|(location, _)| {
            (
                location.file.to_string_lossy().to_string(),
                location.range.start.line,
                location.range.start.column,
            )
        });
        results.dedup_by(|(left, _), (right, _)| left.file == right.file && left.range == right.range);
        results
    }
}

/// A method overridden by the hovered symbol, with a navigable declaration.
pub(crate) struct OverrideMethod {
    pub symbol: SymbolId,
    pub display: String,
    pub location: Option<Location>,
}

impl FileAnalysis {
    /// Symbol-based override lookup for hover documentation/inheritance.
    ///
    /// Unlike the position-based helper this does not sort by location: the
    /// supertypes are kept in inheritance order so "first documented parent"
    /// is deterministic.
    pub(crate) fn overridden_method_entries(&mut self, symbol: SymbolId) -> Vec<OverrideMethod> {
        let (is_method, flags_field, owner) = {
            let child = self.compiler.ctx.sym(symbol);
            (
                matches!(child.kind_data, SymbolKind::MethodSymbol { .. }),
                child.flags_field,
                child.owner,
            )
        };
        if !is_method {
            return Vec::new();
        }
        if (flags_field & flags::STATIC as i64) != 0
            || (flags_field & flags::PRIVATE as i64) != 0
        {
            return Vec::new();
        }
        let Some(signature) = self.erased_signature_of(symbol) else {
            return Vec::new();
        };
        let Some(owner) = owner else {
            return Vec::new();
        };
        let sites = self.supertype_closure(owner);
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for site in &sites {
            let Some(method) = self.find_matching_method(site, &signature) else {
                continue;
            };
            if !seen.insert(method) {
                continue;
            }
            let location = self.method_location(method);
            let display = self.override_entry_display(method, &signature);
            result.push(OverrideMethod {
                symbol: method,
                display,
                location,
            });
        }
        result
    }

    fn override_entry_display(&self, method: SymbolId, signature: &ErasedSignature) -> String {
        let sym = self.compiler.ctx.sym(method);
        let name = sym.name.to_java_string().to_string_lossy();
        let owner = sym
            .owner
            .and_then(|owner| match &self.compiler.ctx.sym(owner).kind_data {
                SymbolKind::ClassSymbol { flatname, .. } => flatname
                    .to_string()
                    .rsplit('.')
                    .next()
                    .map(str::to_string),
                _ => None,
            })
            .unwrap_or_default();
        let owner = owner.rsplit('$').next().unwrap_or(&owner);
        format!(
            "{owner}#{name}({})",
            signature.params.join(", ")
        )
    }

/// `Owner.method` for hover.
    fn override_display_name(&self, method: SymbolId) -> String {
        let sym = self.compiler.ctx.sym(method);
        let name = sym.name.to_java_string().to_string_lossy();
        match sym.owner {
            Some(owner) => match &self.compiler.ctx.sym(owner).kind_data {
                SymbolKind::ClassSymbol { flatname, .. } => {
                    let owner_name = flatname
                        .to_string()
                        .rsplit('.')
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    format!("{owner_name}.{name}")
                }
                _ => name,
            },
            None => name,
        }
    }

    fn position_in_literal_or_comment(&self, file: &Path, position: Position) -> bool {
        let key = PathBuf::from(canonical(file));
        let Some(text) = self
            .texts
            .get(&key)
            .cloned()
            .or_else(|| std::fs::read_to_string(file).ok())
        else {
            return false;
        };
        let byte = line_offset(
            &text,
            position.line as usize + 1,
            position.column as usize + 1,
        );
        in_literal_or_comment(&text, byte)
    }

    /// Erased signature of `method`; `None` when it cannot be determined
    /// (unfinished attribution, non-method type, empty type name).
    fn erased_signature_of(&mut self, method: SymbolId) -> Option<ErasedSignature> {
        let name = self
            .compiler
            .ctx
            .sym(method)
            .name
            .to_java_string()
            .to_string_lossy();
        if name == "<init>" {
            return None;
        }
        let ty = self.compiler.ctx.sym(method).ty?;
        let argtypes = match &self.compiler.ctx.typ(ty).kind {
            TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
            _ => return None,
        };
        let syms = Rc::clone(&self.compiler.syms);
        let types = Rc::clone(&self.compiler.types);
        let mut params = Vec::with_capacity(argtypes.size() as usize);
        for arg in argtypes.iter() {
            let erased = types.erasure(&mut self.compiler.ctx, &syms, arg);
            let text = type_name(&self.compiler.ctx, erased);
            if text.is_empty() {
                return None;
            }
            params.push(text);
        }
        Some(ErasedSignature { name, params })
    }

    /// All supertypes (classes and interfaces), keeping their parameterized
    /// type ids so generic methods can be instantiated on the right site.
    fn supertype_closure(&mut self, class: SymbolId) -> Vec<SuperSite> {
        let mut sites = Vec::new();
        let mut stack = vec![class];
        let mut visited = HashSet::new();
        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            let Some(ty) = self.compiler.ctx.sym(current).ty else {
                continue;
            };
            let syms = Rc::clone(&self.compiler.syms);
            let types = Rc::clone(&self.compiler.types);
            let ctx = &mut self.compiler.ctx;
            let super_ty = types.supertype(ctx, &syms, ty, None);
            if let Some(sym) = ctx.typ(super_ty).tsym
                && visited.insert(sym)
            {
                sites.push(SuperSite {
                    type_id: super_ty,
                    class: sym,
                });
                stack.push(sym);
            }
            for interface_ty in types.interfaces(ctx, &syms, ty).iter() {
                if let Some(sym) = ctx.typ(interface_ty).tsym
                    && visited.insert(sym)
                {
                    sites.push(SuperSite {
                        type_id: interface_ty,
                        class: sym,
                    });
                    stack.push(sym);
                }
            }
        }
        sites
    }

    /// Direct method of the site with the same erased parameter types.
    fn find_matching_method(
        &mut self,
        site: &SuperSite,
        child: &ErasedSignature,
    ) -> Option<SymbolId> {
        let scope = self.compiler.ctx.sym(site.class).members()?;
        let members =
            self.compiler
                .ctx
                .scope_get_symbols(scope, LookupKind::RECURSIVE, &|_| true);
        for member in members {
            let sym = self.compiler.ctx.sym(member);
            if !matches!(sym.kind_data, SymbolKind::MethodSymbol { .. }) {
                continue;
            }
            if sym.name.to_java_string().to_string_lossy() != child.name {
                continue;
            }
            let flags_value = sym.flags_field;
            if (flags_value & flags::STATIC as i64) != 0 {
                continue;
            }
            if (flags_value & flags::PRIVATE as i64) != 0 {
                continue;
            }
            if (flags_value & flags::BRIDGE) != 0 {
                continue;
            }
            let syms = Rc::clone(&self.compiler.syms);
            let types = Rc::clone(&self.compiler.types);
            let parent_ty = types.member_type(&mut self.compiler.ctx, &syms, site.type_id, member);
            let parent_args = match &self.compiler.ctx.typ(parent_ty).kind {
                TypeKindData::MethodType { argtypes, .. } => argtypes.clone(),
                _ => continue,
            };
            if parent_args.size() as usize != child.params.len() {
                continue;
            }
            let mut matched = true;
            for (index, parent_arg) in parent_args.iter().enumerate() {
                let erased = types.erasure(&mut self.compiler.ctx, &syms, parent_arg);
                let text = type_name(&self.compiler.ctx, erased);
                if text != child.params[index] {
                    matched = false;
                    break;
                }
            }
            if matched {
                return Some(member);
            }
        }
        None
    }

    fn method_location(&mut self, method: SymbolId) -> Option<Location> {
        let sym = self.compiler.ctx.sym(method);
        let name = sym.name.to_java_string().to_string_lossy();
        let owner = sym.owner?;
        let (sourcefile, flat_name) = match &self.compiler.ctx.sym(owner).kind_data {
            SymbolKind::ClassSymbol {
                sourcefile,
                flatname,
                ..
            } => (sourcefile.clone(), flatname.to_string()),
            _ => return None,
        };
        if let Some(file) = sourcefile {
            let path = PathBuf::from(file.get_name().to_string_lossy());
            let key = PathBuf::from(canonical(&path));
            let in_source_path = self
                .config
                .source_path
                .iter()
                .any(|root| path.starts_with(root));
            let in_analysis = self.texts.contains_key(&key);
            if in_source_path || in_analysis {
                let text = self
                    .texts
                    .get(&key)
                    .cloned()
                    .or_else(|| std::fs::read_to_string(&path).ok())?;
                let pos = crate::analysis::positions::name_start_of_symbol(&self.compiler.ctx, method)
                    .unwrap_or(self.compiler.ctx.sym(method).pos.get().max(0) as u32);
                let (line, column) = offset_to_line_col(&text, pos as usize);
                return Some(Location {
                    file: path,
                    range: Range::point(Position::new(line, column)),
                });
            }
        }
        // JDK classes belong to a system module (java.* / jdk.*), unlike jar
        // or project classes (unnamed module). Do not judge by name prefixes.
        let owner_class = self.compiler.ctx.sym(method).owner;
        let is_jdk = owner_class
            .map(|class| is_system_module_class(&mut self.compiler.ctx, class))
            .unwrap_or(false);
        if is_jdk {
            let jdk_src = super::super::super::jdk_sources::default_jdk_src()?;
            return super::super::super::jdk_sources::jdk_member_location(
                &jdk_src,
                &flat_name,
                &name,
                &self.config.output_dir,
            );
        }
        crate::analysis::external_sources::external_source_location(
            &flat_name,
            &name,
            &self.config.class_path,
            &self.config.output_dir,
        )
    }
}


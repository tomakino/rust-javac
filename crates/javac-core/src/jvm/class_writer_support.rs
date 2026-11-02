//! Helper functions for the class writer: names, module-info symbols and
//! source-file metadata.

use std::rc::Rc;

use crate::code::kinds::Kind;
use crate::code::symbol::{Symbol, SymbolKind};
use crate::code::type_::{Type, TypeKindData};
use crate::code::type_tag::TypeTag;
use crate::compiler_context::CompilerContext;
use crate::ids::SymbolId;
use crate::util::java_string::JavaString;
use crate::util::list::List;
use crate::util::names::Names;

/// Return the flat name of a class symbol.
pub(crate) fn class_flat_name(ctx: &CompilerContext, c: SymbolId) -> String {
    match &ctx.sym(c).kind_data {
        SymbolKind::ClassSymbol { flatname, .. } => flatname.to_java_string().to_string_lossy(),
        _ => ctx.sym(c).name.to_java_string().to_string_lossy(),
    }
}

/// Return the qualified name of a class symbol.
pub(crate) fn qualified_name(ctx: &CompilerContext, c: SymbolId) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current = Some(c);
    while let Some(sym) = current {
        parts.push(ctx.sym(sym).name.to_java_string().to_string_lossy());
        current = ctx.sym(sym).owner;
    }
    parts.reverse();
    parts.join(".")
}

/// Create the synthetic module-info class symbol.
pub(crate) fn create_module_info_class(
    ctx: &mut CompilerContext,
    names: &Rc<Names>,
    unnamed_package: SymbolId,
) -> SymbolId {
    let name = names.module_info.clone();
    let symbol = ctx.alloc_symbol(Symbol::new(
        Kind::TYP,
        0,
        name,
        None,
        Some(unnamed_package),
        SymbolKind::ClassSymbol {
            members_field: None,
            fullname: names.module_info.clone(),
            flatname: names.module_info.clone(),
            sourcefile: None,
            classfile: None,
            trans_local: List::nil(),
            record_components: List::nil(),
            permitted: List::nil(),
            is_permitted_explicit: false,
            annotation_type_metadata: Default::default(),
        },
    ));
    let ty = ctx.alloc_type(Type::new(
        TypeTag::CLASS,
        Some(symbol),
        TypeKindData::ClassType {
            outer_field: None,
            typarams_field: None,
            allparams_field: None,
            supertype_field: None,
            interfaces_field: None,
            all_interfaces_field: None,
        },
    ));
    ctx.sym_mut(symbol).ty = Some(ty);
    symbol
}

/// Return the simple name of a source file object.
pub(crate) fn simple_name(
    file: &Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>,
) -> JavaString {
    let name = file.get_name().to_string_lossy();
    let last = name.rsplit(['/', '\\']).next().unwrap_or(&name);
    JavaString::from(last)
}

/// Return the last modification time of a source file object.
pub(crate) fn last_modified(
    file: &Option<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>>,
) -> i64 {
    let _ = file;
    0
}

/// Return the current wall clock time in milliseconds.
pub(crate) fn current_time_millis() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(_) => 0,
    }
}

/// Return the source file of a class symbol.
pub(crate) fn class_sourcefile(
    ctx: &CompilerContext,
    c: SymbolId,
) -> Option<Rc<dyn javac_tools::tools::java_file_object::JavaFileObject>> {
    match &ctx.sym(c).kind_data {
        SymbolKind::ClassSymbol { sourcefile, .. } => sourcefile.clone(),
        _ => None,
    }
}

//! com.sun.tools.javac.comp

/// Java services are single instances with re-entrant methods (Resolve, Attr,
/// TypeEnter, ...). The port stores mutually recursive services in RefCell;
/// when an outer mutable borrow is suspended and the same service is called
/// re-entrantly, recover the active instance instead of panicking.
pub fn with_reentrant_mut<T, R>(
    cell: &std::cell::RefCell<T>,
    f: impl FnOnce(&mut T) -> R,
) -> R {
    if let Ok(mut guard) = cell.try_borrow_mut() {
        return f(&mut guard);
    }
    let ptr = cell.as_ptr();
    // SAFETY: matches javac's single re-entrant service instance. The outer
    // borrow is not used while the nested call runs.
    f(unsafe { &mut *ptr })
}

pub mod annotate;
pub mod analyzer;
pub mod argument_attr;
pub mod attr;
pub mod attr_contexts;
pub mod attr_recover;
pub mod attr_context;
pub mod check;
pub mod check_context;
pub mod compile_states;
pub mod const_fold;
pub mod deferred_attr;
pub mod enter;
pub mod flow;
pub mod infer;
pub mod env;
pub mod lower;
pub mod match_bindings;
pub mod modules;
pub mod member_enter;
pub mod method_resolution;
pub mod operators;
pub mod post_attr_analyzer;
pub mod resolve;
pub mod result_info;
pub mod type_display;
pub mod todo;
pub mod trans_patterns;
pub mod lambda_to_method;
pub mod tree_differ;
pub mod tree_hasher;
pub mod trans_types;
pub mod type_enter;
pub mod type_envs;

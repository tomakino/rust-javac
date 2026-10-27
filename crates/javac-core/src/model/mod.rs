//! com.sun.tools.javac.model

/// The dynamically generated annotation proxies (AnnotationProxyMaker).
pub mod annotation_proxy_maker;
/// The javax.lang.model annotation mirror views (JavacElements/JavacTypes snapshots).
pub mod annotation_mirrors;
/// The Symbol annotation accessors of AnnoConstruct/Symbol.
pub mod symbol_annotations;

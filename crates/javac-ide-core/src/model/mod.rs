//! com.sun.tools.javac.model

/// The javax.lang.model annotation mirror views (JavacElements/JavacTypes snapshots).
pub mod annotation_mirrors;
/// The dynamically generated annotation proxies (AnnotationProxyMaker).
pub mod annotation_proxy_maker;
/// The Symbol annotation accessors of AnnoConstruct/Symbol.
pub mod symbol_annotations;

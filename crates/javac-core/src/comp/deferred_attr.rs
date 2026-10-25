// Origin: com/sun/tools/javac/comp/DeferredAttr.java @ 982d53c (jdk-17.0.19-ga)

//! Deferred attribution: this file currently ports only the
//! DeferredAttr.AttributionMode enum, which is a field of AttrContext (the
//! Enter/Attr dependency).
//!
//! NOT PORTED IN THIS STEP (registered; dependencies: Attr/Check/Resolve/
//! TreeCopier and the speculative attribution machinery):
//! - the whole DeferredAttr class (attribSpeculative/DeferredType/
//!   DeferredAttrNode/...).

/// Java: enum DeferredAttr.AttributionMode
///
/// PORTING_RULES: the Java enum constant names are kept verbatim (no Rust-style
/// renaming, section 3), hence the allow.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub enum AttributionMode {
    /// Normal, non-speculative, attribution.
    #[default]
    FULL,
    /// Speculative attribution on behalf of an Analyzer.
    ATTRIB_TO_TREE,
    /// Speculative attribution on behalf of an Analyzer.
    ANALYZER,
    /// Speculative attribution.
    SPECULATIVE,
}

impl AttributionMode {
    /// Java: AttributionMode(boolean isSpeculative, boolean recover)
    pub fn is_speculative(self) -> bool {
        !matches!(self, AttributionMode::FULL)
    }

    pub fn recover(self) -> bool {
        matches!(self, AttributionMode::FULL | AttributionMode::ATTRIB_TO_TREE)
    }
}


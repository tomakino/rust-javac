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



// ---------------------------------------------------------------------------
// Java: com/sun/tools/javac/comp/DeferredAttr.java (the Check/Attr direct
// dependency subset: AttrMode / DeferredAttrContext / emptyDeferredAttrContext).
// §19 registration: the remaining DeferredAttr class (DeferredType/DeferredCompleter/
// DeferredAttrNode/DeferredStuckPolicy/attribSpeculative/... and the full
// DeferredAttrContext.complete/buildStuckGraph/pickDeferredNode) is the
// DeferredAttr batch; the fields/methods below are 1:1 with the upstream ones
// that Check/Attr use.
// ---------------------------------------------------------------------------

/// Java: public enum DeferredAttr.AttrMode
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttrMode {
    /// Java: SPECULATIVE
    SPECULATIVE,
    /// Java: CHECK
    CHECK,
}

/// Java: class DeferredAttr.DeferredAttrContext
pub struct DeferredAttrContext {
    /// Java: final AttrMode mode
    pub mode: AttrMode,
    /// Java: final Symbol msym (Java null -> None)
    pub msym: Option<crate::ids::SymbolId>,
    /// Java: final Resolve.MethodResolutionPhase phase
    pub phase: crate::comp::resolve::MethodResolutionPhase,
    /// Java: final InferenceContext inferenceContext
    pub inference_context: std::rc::Rc<crate::comp::infer::InferenceContext>,
    /// Java: final DeferredAttrContext parent (the empty context is the root)
    pub parent: Option<std::rc::Rc<DeferredAttrContext>>,
    /// Java: final Warner warn (Java null -> None)
    pub warn: Option<crate::util::warner::Warner>,
    /// Java: ArrayList<DeferredAttrNode> deferredAttrNodes — the DeferredAttr
    /// batch registers the node type; the port keeps the list empty until then
    /// (the empty context rejects additions upstream).
    pub deferred_attr_nodes: Vec<()>,
    /// True for the singleton emptyDeferredAttrContext (the upstream anonymous
    /// subclass overrides addDeferredAttrNode/complete with Assert.error).
    pub empty: bool,
}

impl DeferredAttrContext {
    /// Java: DeferredAttrContext(AttrMode mode, Symbol msym,
    /// MethodResolutionPhase phase, InferenceContext inferenceContext,
    /// DeferredAttrContext parent, Warner warn)
    pub fn new(
        mode: AttrMode,
        msym: Option<crate::ids::SymbolId>,
        phase: crate::comp::resolve::MethodResolutionPhase,
        inference_context: std::rc::Rc<crate::comp::infer::InferenceContext>,
        parent: Option<std::rc::Rc<DeferredAttrContext>>,
        warn: Option<crate::util::warner::Warner>,
    ) -> DeferredAttrContext {
        DeferredAttrContext {
            mode,
            msym,
            phase,
            inference_context,
            parent,
            warn,
            deferred_attr_nodes: Vec::new(),
            empty: false,
        }
    }

    /// Java: DeferredAttr.emptyDeferredAttrContext = new DeferredAttrContext(
    ///     AttrMode.CHECK, null, MethodResolutionPhase.BOX, infer.emptyContext,
    ///     null, null) { addDeferredAttrNode/complete: Assert.error; }
    pub fn empty() -> DeferredAttrContext {
        DeferredAttrContext {
            mode: AttrMode::CHECK,
            msym: None,
            phase: crate::comp::resolve::MethodResolutionPhase::BOX,
            inference_context: crate::comp::infer::empty_context(),
            parent: None,
            warn: None,
            deferred_attr_nodes: Vec::new(),
            empty: true,
        }
    }

    /// Java: void addDeferredAttrNode(DeferredType dt, ResultInfo resultInfo,
    /// DeferredStuckPolicy deferredStuckPolicy)
    pub fn add_deferred_attr_node(&mut self) {
        if self.empty {
            panic!("AssertionError: Empty deferred context!");
        }
        // TODO(DeferredAttr batch): the DeferredAttrNode list (DeferredType/
        // ResultInfo/DeferredStuckPolicy); registered.
        unimplemented!("TODO(DeferredAttr batch): DeferredAttrContext.addDeferredAttrNode")
    }

    /// Java: void complete()
    pub fn complete(&mut self) {
        if self.empty {
            panic!("AssertionError: Empty deferred context!");
        }
        // TODO(DeferredAttr batch): the deferred node processing loop
        // (insideOverloadPhase/solveAny/notifyChange); registered.
        unimplemented!("TODO(DeferredAttr batch): DeferredAttrContext.complete")
    }

    /// Java: public boolean insideOverloadPhase()
    pub fn inside_overload_phase(&self) -> bool {
        if self.empty {
            return false;
        }
        if self.mode == AttrMode::SPECULATIVE {
            return true;
        }
        match &self.parent {
            Some(parent) => parent.inside_overload_phase(),
            None => false,
        }
    }

    /// Java: DeferredAttrNode pickDeferredNode()
    pub fn pick_deferred_node(&mut self) -> () {
        // TODO(DeferredAttr batch): buildStuckGraph/GraphUtils.tarjan; registered.
        unimplemented!("TODO(DeferredAttr batch): DeferredAttrContext.pickDeferredNode")
    }

    /// Java: List<StuckNode> buildStuckGraph()
    pub fn build_stuck_graph(&mut self) -> Vec<()> {
        // TODO(DeferredAttr batch): the StuckNode graph; registered.
        unimplemented!("TODO(DeferredAttr batch): DeferredAttrContext.buildStuckGraph")
    }
}

impl Default for DeferredAttrContext {
    fn default() -> Self {
        DeferredAttrContext::empty()
    }
}

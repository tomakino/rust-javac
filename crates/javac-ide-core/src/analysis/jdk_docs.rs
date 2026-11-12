//! Javadoc lookup from the JDK src.zip for completion documentation.
//!
//! IDEA shows the element documentation in the completion popup. Class files
//! carry no Javadoc, so the raw `/** ... */` blocks are read lazily from the
//! platform sources and cached per class. `FileAnalysis` parses each block with
//! `DocCommentParser` and renders it with the shared Markdown renderer.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::compiler_context::CompilerContext;
use crate::ids::TreeId;
use crate::parser::parser::Parser;
use crate::parser::lexer::Lexer;
use crate::parser::parser_factory::ParserFactory;
use crate::parser::tokens::CommentStyle;
use crate::tree::jc_tree::TreeKind;
use crate::util::java_string::JavaString;
use std::rc::Rc;

use javac_zip::ZipArchive;

/// Lazily built Javadoc cache over the JDK src.zip.
pub(crate) struct JdkDocCache {
    factory: Rc<ParserFactory>,
    src: Option<PathBuf>,
    archive: Option<ZipArchive>,
    /// Outer class flat name -> parsed docs (None when the source is absent).
    classes: HashMap<String, Option<Rc<JdkClassDocs>>>,
}

/// Javadoc of one source class: the type comment plus member comments keyed by
/// "method:name/arity" or "field:name" (nested types use "class:name").
#[derive(Default)]
struct JdkClassDocs {
    class_doc: Option<String>,
    members: HashMap<String, String>,
}

impl JdkDocCache {
    pub(crate) fn new(factory: Rc<ParserFactory>) -> Self {
        Self {
            factory,
            src: super::jdk_sources::default_jdk_src(),
            archive: None,
            classes: HashMap::new(),
        }
    }

    /// Javadoc of a top-level class (nested flat names are reduced to the
    /// outermost class and are not returned here).
    pub(crate) fn class_doc(&mut self, flat: &str) -> Option<String> {
        let docs = self.docs_for(flat)?;
        docs.class_doc.clone()
    }

    /// Javadoc of a member. Methods are selected by name and parameter count;
    /// fields and nested types by name. Overloads with the same arity share
    /// the first documented comment, which matches the JDK style.
    pub(crate) fn member_doc(
        &mut self,
        class_flat: &str,
        name: &str,
        arity: Option<usize>,
    ) -> Option<String> {
        let docs = self.docs_for(class_flat)?;
        if let Some(arity) = arity
            && let Some(doc) = docs.members.get(&format!("method:{name}/{arity}"))
        {
            return Some(doc.clone());
        }
        docs.members
            .get(&format!("field:{name}"))
            .cloned()
            .or_else(|| docs.members.get(&format!("class:{name}")).cloned())
    }

    fn docs_for(&mut self, flat: &str) -> Option<Rc<JdkClassDocs>> {
        let outer = flat.split_once('$').map_or(flat, |(outer, _)| outer);
        if let Some(cached) = self.classes.get(outer) {
            return cached.clone();
        }
        let docs = self.load(outer).map(Rc::new);
        self.classes.insert(outer.to_string(), docs.clone());
        docs
    }

    fn load(&mut self, outer: &str) -> Option<JdkClassDocs> {
        let archive = self.archive()?;
        let path = format!("{}.java", outer.replace('.', "/"));
        let entry = archive
            .entries()
            .iter()
            .find(|entry| {
                let name = entry.name();
                name == path || name.ends_with(&format!("/{path}"))
            })?
            .name()
            .to_string();
        let bytes = archive.get_entry(&entry)?;
        let text = String::from_utf8_lossy(&bytes);
        Some(parse_docs(&self.factory, &text))
    }

    fn archive(&mut self) -> Option<&ZipArchive> {
        if self.archive.is_none() {
            let src = self.src.as_ref()?;
            self.archive = ZipArchive::open(src).ok();
        }
        self.archive.as_ref()
    }
}

/// Parse the source with `JavacParser` (doc comments kept) and associate each
/// declaration with its `DocCommentTable` comment.
fn parse_docs(factory: &ParserFactory, source: &str) -> JdkClassDocs {
    let mut docs = JdkClassDocs::default();
    let mut ctx = CompilerContext::new();
    let input = JavaString::from(source);
    let mut parser = factory.new_parser(&mut ctx, &input, true, false, false);
    let root = parser.parse_compilation_unit(&mut ctx);
    let table = match &ctx.tree(root).kind {
        TreeKind::CompilationUnit {
            doc_comments: Some(table),
            ..
        } => table.clone(),
        _ => return docs,
    };
    let table = table.borrow();
    let class_pos = (0..ctx.trees.len())
        .filter_map(|index| {
            let tree = TreeId::new(index as u32);
            matches!(ctx.tree(tree).kind, TreeKind::ClassDecl { .. })
                .then_some(ctx.tree(tree).pos)
        })
        .filter(|position| *position >= 0)
        .min();
    for index in 0..ctx.trees.len() {
        let tree = TreeId::new(index as u32);
        let raw = table
            .get_comment_text(tree)
            .map(|text| text.to_string_lossy().to_string())
            .or_else(|| {
                table
                    .get_comment(tree)
                    .and_then(|comment| comment.get_text())
                    .map(|text| text.to_string_lossy().to_string())
            });
        let Some(raw) = raw else { continue };
        match &ctx.tree(tree).kind {
            TreeKind::ClassDecl { name, .. } => {
                let name = name.to_java_string().to_string_lossy().to_string();
                if docs.class_doc.is_none() && Some(ctx.tree(tree).pos) == class_pos {
                    docs.class_doc = Some(raw.clone());
                }
                docs.members.entry(format!("class:{name}")).or_insert(raw);
            }
            TreeKind::MethodDecl { name, params, .. } => {
                let name = name.to_java_string().to_string_lossy().to_string();
                let name = if name == "<init>" {
                    String::new()
                } else {
                    name
                };
                if !name.is_empty() {
                    docs.members
                        .entry(format!("method:{}/{}", name, params.size()))
                        .or_insert(raw);
                }
            }
            TreeKind::VariableDecl { name, .. } => {
                let name = name.to_java_string().to_string_lossy().to_string();
                docs.members
                    .entry(format!("field:{name}"))
                    .or_insert(raw);
            }
            _ => {}
        }
    }
    docs
}


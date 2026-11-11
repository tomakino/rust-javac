//! Javadoc lookup from the JDK src.zip for completion documentation.
//!
//! IDEA shows the element documentation in the completion popup. Class files
//! carry no Javadoc, so the raw `/** ... */` blocks are read lazily from the
//! platform sources and cached per class. `FileAnalysis` parses each block with
//! `DocCommentParser` and renders it with the shared Markdown renderer.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use javac_zip::ZipArchive;

/// Lazily built Javadoc cache over the JDK src.zip.
pub(crate) struct JdkDocCache {
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
    pub(crate) fn new() -> Self {
        Self {
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
        Some(parse_docs(&text))
    }

    fn archive(&mut self) -> Option<&ZipArchive> {
        if self.archive.is_none() {
            let src = self.src.as_ref()?;
            self.archive = ZipArchive::open(src).ok();
        }
        self.archive.as_ref()
    }
}

struct Decl {
    kind: DeclKind,
    name: String,
}

enum DeclKind {
    Class,
    Method(usize),
    Field,
}

/// Parse every Javadoc block in a source file and associate it with the next
/// declaration. This is deliberately a source-level scan, not a full parse:
/// completion needs stable text, not diagnostic fidelity.
fn parse_docs(source: &str) -> JdkClassDocs {
    let mut docs = JdkClassDocs::default();
    let mut cursor = 0usize;
    while let Some(relative) = source[cursor..].find("/**") {
        let start = cursor + relative;
        let Some(end_relative) = source[start..].find("*/") else {
            break;
        };
        let end = start + end_relative + 2;
        if let Some(decl) = declaration_after(&source[end..]) {
            // Raw Javadoc block: `FileAnalysis` parses it with
            // DocCommentParser and renders it with the shared renderer.
            let raw = source[start..end].to_string();
            match decl.kind {
                DeclKind::Class => {
                    if docs.class_doc.is_none() {
                        docs.class_doc = Some(raw.clone());
                    }
                    docs.members
                        .entry(format!("class:{}", decl.name))
                        .or_insert(raw);
                }
                DeclKind::Method(arity) => {
                    docs.members
                        .entry(format!("method:{}/{}", decl.name, arity))
                        .or_insert(raw);
                }
                DeclKind::Field => {
                    docs.members
                        .entry(format!("field:{}", decl.name))
                        .or_insert(raw);
                }
            }
        }
        cursor = end;
    }
    docs
}

fn declaration_after(source: &str) -> Option<Decl> {
    let mut rest = source;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return None;
        }
        if rest.starts_with("@interface") {
            let name = first_ident(&rest["@interface".len()..])?;
            return Some(Decl {
                kind: DeclKind::Class,
                name,
            });
        }
        if rest.starts_with('@') {
            rest = skip_annotation(rest);
            continue;
        }
        let (word, after) = split_ident(rest);
        if word.is_empty() {
            return None;
        }
        if is_modifier(word) || (word == "non" && after.starts_with("-sealed")) {
            rest = after;
            continue;
        }
        break;
    }
    let (word, after) = split_ident(rest);
    if matches!(word, "class" | "interface" | "enum" | "record") {
        let name = first_ident(after.trim_start())?;
        return Some(Decl {
            kind: DeclKind::Class,
            name,
        });
    }
    let paren = rest.find('(');
    let term = first_of(rest, &[';', '{', '=', ',']);
    let method_like = match (paren, term) {
        (Some(paren), Some(term)) => paren < term,
        (Some(_), None) => true,
        _ => false,
    };
    if method_like {
        let paren = paren?;
        let name = last_ident(&rest[..paren])?;
        let arity = count_params(&rest[paren + 1..])?;
        return Some(Decl {
            kind: DeclKind::Method(arity),
            name,
        });
    }
    let term = term.unwrap_or(rest.len());
    let name = last_ident(&rest[..term])?;
    Some(Decl {
        kind: DeclKind::Field,
        name,
    })
}

fn skip_annotation(source: &str) -> &str {
    let bytes = source.as_bytes();
    let mut index = 1usize;
    while index < bytes.len()
        && (bytes[index].is_ascii_alphanumeric()
            || bytes[index] == b'_'
            || bytes[index] == b'.'
            || bytes[index] == b'$')
    {
        index += 1;
    }
    let mut after = index;
    while after < bytes.len() && bytes[after].is_ascii_whitespace() && bytes[after] != b'\n' {
        after += 1;
    }
    if after < bytes.len() && bytes[after] == b'(' {
        let mut depth = 0usize;
        while after < bytes.len() {
            match bytes[after] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[after + 1..];
                    }
                }
                _ => {}
            }
            after += 1;
        }
    }
    &source[index..]
}

fn is_modifier(word: &str) -> bool {
    matches!(
        word,
        "public"
            | "protected"
            | "private"
            | "static"
            | "final"
            | "abstract"
            | "synchronized"
            | "native"
            | "transient"
            | "volatile"
            | "strictfp"
            | "default"
            | "sealed"
    )
}

fn split_ident(source: &str) -> (&str, &str) {
    let source = source.trim_start();
    let end = source
        .char_indices()
        .find(|(_, ch)| !(ch.is_alphanumeric() || *ch == '_' || *ch == '$'))
        .map(|(index, _)| index)
        .unwrap_or(source.len());
    (&source[..end], &source[end..])
}

fn first_ident(source: &str) -> Option<String> {
    let (word, _) = split_ident(source);
    (!word.is_empty()).then(|| word.to_string())
}

fn last_ident(source: &str) -> Option<String> {
    source
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '$'))
        .filter(|word| !word.is_empty())
        .next_back()
        .map(str::to_string)
}

fn first_of(source: &str, chars: &[char]) -> Option<usize> {
    source
        .char_indices()
        .find(|(_, ch)| chars.contains(ch))
        .map(|(index, _)| index)
}

fn count_params(source: &str) -> Option<usize> {
    let mut parens = 0usize;
    let mut angles = 0usize;
    let mut commas = 0usize;
    let mut has_content = false;
    for ch in source.chars() {
        match ch {
            '(' => parens += 1,
            ')' => {
                if parens == 0 {
                    return Some(if has_content { commas + 1 } else { 0 });
                }
                parens -= 1;
            }
            '<' => angles += 1,
            '>' => angles = angles.saturating_sub(1),
            ',' if parens == 0 && angles == 0 => commas += 1,
            _ => {}
        }
        if !ch.is_whitespace() {
            has_content = true;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associates_class_method_and_field_docs() {
        let source = r#"
/**
 * A greeting.
 */
public class Greeter {
    /**
     * Says hello.
     * @param name who to greet
     * @return the greeting
     */
    public String hello(String name) {
        return name;
    }

    /** The default name. */
    public static final String DEFAULT = "world";
}
"#;
        let docs = parse_docs(source);
        let class_doc = docs.class_doc.as_deref().unwrap_or("");
        assert!(class_doc.contains("A greeting."), "{class_doc}");
        let method = docs.members.get("method:hello/1").expect("method doc");
        assert!(method.contains("Says hello."), "{method}");
        assert!(method.contains("@param name"), "{method}");
        assert!(method.contains("@return"), "{method}");
        let field = docs.members.get("field:DEFAULT").expect("field doc");
        assert!(field.contains("The default name."), "{field}");
    }

    #[test]
    fn generic_parameters_do_not_change_arity() {
        let source = r#"
/** A map holder. */
class Holder {
    /** Puts a value. */
    void put(java.util.Map<String, java.util.List<Integer>> map, int value) {}
}
"#;
        let docs = parse_docs(source);
        let class_doc = docs.class_doc.as_deref().unwrap_or("");
        assert!(class_doc.contains("A map holder."), "{class_doc}");
        let method = docs.members.get("method:put/2").expect("method doc");
        assert!(method.contains("Puts a value."), "{method}");
    }

    #[test]
    fn keeps_the_complete_raw_javadoc_block() {
        let source = r#"
/**
 * Returns a {@link java.util.Base64.Decoder} using {@code Base64}.
 * <p>Second paragraph.</p>
 */
public class Thing {
}
"#;
        let docs = parse_docs(source);
        let class_doc = docs.class_doc.expect("doc");
        assert!(class_doc.contains("{@link java.util.Base64.Decoder}"), "{class_doc}");
        assert!(class_doc.contains("{@code Base64}"), "{class_doc}");
        assert!(class_doc.contains("<p>Second paragraph.</p>"), "{class_doc}");
    }
}

//! Javadoc delivery: hover (declaration and reference), declaration kinds,
//! tag rendering, and the JDK src.zip fallback.

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::analysis::AnalysisConfig;
use javac_ide_core::workspace::Workspace;

fn platform_dir() {
    if std::env::var_os("RAVAC_PLATFORM_DIR").is_none() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("debug");
        unsafe {
            std::env::set_var("RAVAC_PLATFORM_DIR", &root);
        }
    }
}

fn session() -> Workspace {
    platform_dir();
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("javadoc-spec");
    Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: output,
        source_level: "17".to_string(),
        source_path: Vec::new(),
    })
}

fn project_session(root: &Path) -> Workspace {
    platform_dir();
    Workspace::new(AnalysisConfig {
        class_path: Vec::new(),
        output_dir: root.join("out"),
        source_level: "17".to_string(),
        source_path: vec![root.to_path_buf()],
    })
}

fn overlay(session: &mut Workspace, name: &str, source: &str) -> PathBuf {
    let path = PathBuf::from(format!("target/tmp/javadoc-spec/{name}.java"));
    std::fs::create_dir_all("target/tmp/javadoc-spec").unwrap();
    std::fs::write(&path, source).unwrap();
    WorkspaceService::diagnostics(session, &path, Some(source));
    path
}

fn position_of(text: &str, marker: &str, offset: usize) -> Position {
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("marker not found: {marker}"))
        + offset;
    let prefix = &text[..start];
    let line = prefix.matches('\n').count() as u32;
    let column: u32 = prefix
        .rsplit('\n')
        .next()
        .unwrap()
        .encode_utf16()
        .count() as u32;
    Position::new(line, column)
}

fn hover_at(session: &mut Workspace, path: &Path, text: &str, marker: &str, offset: usize) -> String {
    let hover = WorkspaceService::hover(session, path, position_of(text, marker, offset))
        .unwrap_or_else(|| panic!("no hover for {marker}"));
    hover
}

const SOURCE: &str = r#"/** Class doc. Second sentence for the class. */
class Doc {
    /** Field doc. */
    int value;

    /** Constructor doc with {@link Doc}. */
    Doc() {
    }

    /**
     * Method doc. More detail.
     *
     * <ul><li>item one</li></ul>
     *
     * @param x the x value
     * @return the result
     * @throws IllegalStateException when bad
     * @deprecated use something else
     */
    int method(int x) {
        return x;
    }

    void plain() {
    }

    void use() {
        method(1);
    }

    /** Enum doc. */
    enum Color {
        /** Constant doc. */
        RED
    }

    /** Annotation doc. */
    @interface Marker {
        /** Element doc. */
        int value();
    }

    /** Record doc. */
    record Point(/** Component doc. */ int x) {
    }
}
"#;

#[test]
fn hover_shows_javadoc_for_every_declaration_kind() {
    let mut workspace = session();
    let path = overlay(&mut workspace, "Doc", SOURCE);

    let cases = [
        ("class", "class Doc {", "Doc", "Class doc. Second sentence for the class."),
        ("field", "int value;", "value", "Field doc."),
        ("constructor", "Doc() {", "Doc", "Constructor doc"),
        ("method", "int method(int x) {", "method", "Method doc. More detail."),
        ("enum", "enum Color {", "Color", "Enum doc."),
        ("enum constant", "RED", "RED", "Constant doc."),
        ("annotation", "@interface Marker {", "Marker", "Annotation doc."),
        ("annotation element", "int value();", "value", "Element doc."),
        ("record", "record Point(", "Point", "Record doc."),
        ("record component", "int x) {", "x", "Component doc."),
    ];
    for (label, marker, word, expected) in cases {
        let hover = hover_at(
            &mut workspace,
            &path,
            SOURCE,
            marker,
            marker.find(word).unwrap_or_else(|| panic!("{label}: word")),
        );
        assert!(hover.contains(expected), "{label} hover: {hover}");
    }
}

#[test]
fn multi_paragraph_javadoc_keeps_every_paragraph_and_block() {
    let source = r#"class Paragraphs {
    /**
     * First paragraph with {@code code} and {@link Paragraphs}. It continues.
     *
     * <p>Second paragraph with {@linkplain Paragraphs#paragraphs(int) plain label}.
     *
     * <p>Third paragraph.
     *
     * <p>Fourth paragraph with {@code List<Foo>}.
     *
     * <pre>
     * new TypeToken&lt;Collection&lt;Foo&gt;&gt;(){}
     * </pre>
     *
     * @param x the x
     * @return the result
     * @throws IllegalStateException when bad
     * @see Paragraphs
     * @since 1.0
     * @deprecated old
     */
    int paragraphs(int x) {
        return x;
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Paragraphs", source);
    let hover = hover_at(&mut workspace, &path, source, "int paragraphs(int x) {", 4);

    // Every paragraph must survive, not just the first sentence.
    assert!(
        hover.contains("First paragraph with `code` and `Paragraphs`. It continues."),
        "{hover}"
    );
    assert!(
        hover.contains("Second paragraph with plain label (`Paragraphs#paragraphs(int)`)."),
        "{hover}"
    );
    assert!(hover.contains("Third paragraph."), "{hover}");
    assert!(
        hover.contains("Fourth paragraph with `List<Foo>`."),
        "{hover}"
    );

    // Paragraphs are separated by a blank line, and the `<pre>` block becomes a
    // fenced code block (raw HTML is not rendered by hover clients).
    assert!(
        hover.contains(
            "It continues.\n\nSecond paragraph with plain label (`Paragraphs#paragraphs(int)`)."
        ),
        "paragraph break lost: {hover}"
    );
    assert!(
        hover.contains("```\nnew TypeToken<Collection<Foo>>(){}\n```"),
        "code block lost: {hover}"
    );
    assert!(!hover.contains("<p>"), "raw paragraph tag leaked: {hover}");
    assert!(!hover.contains("<pre>"), "raw pre tag leaked: {hover}");

    for tag in [
        "- **x** the x",
        "- **Returns** the result",
        "- **Throws** IllegalStateException when bad",
        "- **See** Paragraphs",
        "- **Since** 1.0",
        "- **Deprecated** old",
    ] {
        assert!(hover.contains(tag), "missing tag {tag}: {hover}");
    }
}

#[test]
fn structured_blocks_inline_formatting_and_entity_decoding() {
    let source = r#"class Blocks {
    /**
     * Intro <b>bold</b> <i>italic</i> <s>strike</s> <code>a`b</code>.<br>Hard break.
     *
     * <p>Paragraph with <a href="https://example.com">link text</a>.
     *
     * <ul>
     *   <li>one</li>
     *   <li>two
     *     <ul><li>nested</li></ul>
     *   </li>
     * </ul>
     *
     * <ol><li>first</li><li>second</li></ol>
     *
     * <pre>
     * line1
     *     indented
     * </pre>
     *
     * <blockquote>quoted</blockquote>
     *
     * <table>
     *   <tr><th>Name</th><th>Value</th></tr>
     *   <tr><td>a</td><td>b</td></tr>
     * </table>
     *
     * <h1>Heading</h1>
     * <hr>
     * Entities: &lt; &gt; &amp; &quot; &apos; &nbsp; &#65; &#x42;
     *
     * @param <T> the type
     * @param x the x
     * @return the result
     * @throws IllegalStateException bad
     * @see Blocks
     * @since 1.0
     * @author A
     * @deprecated old
     */
    int blocks(int x) {
        return x;
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Blocks", source);
    let hover = hover_at(&mut workspace, &path, source, "int blocks(int x) {", 4);

    for expected in [
        "**bold**",
        "*italic*",
        "~~strike~~",
        "`` a`b ``",
        "[link text](https://example.com)",
        "- one",
        "- two",
        "  - nested",
        "1. first",
        "2. second",
        "```\nline1\n    indented\n```",
        "> quoted",
        "| Name | Value |",
        "| --- | --- |",
        "| a | b |",
        "# Heading",
        "---",
        "Entities: < > & \" '   A B",
        "  \nHard break.",
        "- **`<T>`** the type",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
    assert!(!hover.contains("<p>"), "raw <p> leaked: {hover}");
    assert!(!hover.contains("<table>"), "raw <table> leaked: {hover}");

    // Tags keep their source order.
    let order = [
        "- **`<T>`** the type",
        "- **x** the x",
        "- **Returns** the result",
        "- **Throws** IllegalStateException bad",
        "- **See** Blocks",
        "- **Since** 1.0",
        "- **Author** A",
        "- **Deprecated** old",
    ];
    let mut last = 0usize;
    for tag in order {
        let position = hover
            .find(tag)
            .unwrap_or_else(|| panic!("missing tag {tag}: {hover}"));
        assert!(position >= last, "tag order broken at {tag}: {hover}");
        last = position;
    }
}

#[test]
fn hover_renders_tags_links_lists_and_skips_empty_sections() {
    let mut workspace = session();
    let path = overlay(&mut workspace, "Doc", SOURCE);

    let method = hover_at(&mut workspace, &path, SOURCE, "int method(int x) {", 4);
    for expected in [
        "Method doc. More detail.",
        "- item one",
        "- **x** the x value",
        "- **Returns** the result",
        "- **Throws** IllegalStateException when bad",
        "- **Deprecated** use something else",
    ] {
        assert!(method.contains(expected), "method hover missing {expected}: {method}");
    }
    assert!(
        !method.contains("<ul>") && !method.contains("<li>"),
        "raw HTML list leaked into hover: {method}"
    );

    let constructor = hover_at(&mut workspace, &path, SOURCE, "Doc() {", 0);
    assert!(constructor.contains("Constructor doc"), "{constructor}");
    assert!(constructor.contains("`Doc`"), "link target lost: {constructor}");

    let plain = hover_at(&mut workspace, &path, SOURCE, "void plain() {", 5);
    assert!(!plain.contains("\n---\n"), "empty doc section: {plain}");
}

#[test]
fn signature_help_and_outline_carry_javadoc() {
    let mut workspace = session();
    let path = overlay(&mut workspace, "Doc", SOURCE);

    let help_position = position_of(SOURCE, "method(1);", "method(".len());
    let help = WorkspaceService::signature_help(&mut workspace, &path, help_position)
        .expect("signature help");
    let documentation = help.signatures[0]
        .documentation
        .as_deref()
        .unwrap_or("");
    assert!(
        documentation.contains("Method doc. More detail."),
        "signature help doc: {documentation}"
    );

    let symbols = WorkspaceService::document_symbols(&mut workspace, &path);
    let class = symbols
        .iter()
        .find(|symbol| symbol.name == "Doc")
        .expect("class symbol");
    assert!(
        class.detail.as_deref().unwrap_or("").contains("Class doc."),
        "class outline detail: {:?}",
        class.detail
    );
    let method = class
        .children
        .iter()
        .find(|symbol| symbol.name == "method")
        .expect("method symbol");
    assert!(
        method
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("Method doc. More detail."),
        "method outline detail: {:?}",
        method.detail
    );
}

#[test]
fn overrides_inherit_parent_javadoc_and_link_the_declaration() {
    let source = r#"class Base {
    /** Base method doc. */
    String value() {
        return "base";
    }
}

class Child extends Base {
    @Override
    String value() {
        return "child";
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Inherited", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "String value() {\n        return \"child\";",
        "String ".len(),
    );
    for expected in [
        "String value()",
        "Base method doc.",
        "Inherited from Base#value()",
        "**Overrides:**",
        "Base#value()](file://",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
}

#[test]
fn inherit_doc_expands_body_and_tags() {
    let source = r#"class Base2 {
    /**
     * Base doc.
     * @param x the x
     */
    int work(int x) {
        return x + 1;
    }
}

class Impl2 extends Base2 {
    /** Impl prefix {@inheritDoc} suffix. */
    @Override
    int work(int x) {
        return x + 2;
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "InheritDoc", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "int work(int x) {\n        return x + 2;",
        "int ".len(),
    );
    for expected in [
        "Impl prefix",
        "Base doc.",
        "- **x** the x",
        "suffix.",
        "Inherited from Base2#work(int)",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
    assert!(
        !hover.contains("{@inheritDoc: unresolved}"),
        "explicit inheritDoc was not expanded: {hover}"
    );
}

#[test]
fn inherit_doc_without_parent_is_marked_unresolved() {
    let source = r#"class Top {
    /** {@inheritDoc} */
    void missingParent() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Unresolved", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "void missingParent() {",
        "void ".len(),
    );
    assert!(
        hover.contains("{@inheritDoc: unresolved}"),
        "unresolved inheritDoc not marked: {hover}"
    );
}

#[test]
fn undocumented_method_without_parent_has_no_empty_doc_section() {
    let source = r#"class PlainNoDoc {
    void plain() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "PlainNoDoc", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "void plain() {",
        "void plain".len(),
    );
    assert!(!hover.contains("\n---\n"), "empty doc/overrides section: {hover}");
    assert!(!hover.contains("Inherited from"), "{hover}");
}

#[test]
fn diamond_override_is_deduplicated() {
    let source = r#"interface RootDoc {
    /** Root doc. */
    default void shared() {
    }
}

interface Left extends RootDoc {
}

interface Right extends RootDoc {
}

class Diamond implements Left, Right {
    @Override
    public void shared() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Diamond", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public void shared() {",
        "public void ".len(),
    );
    assert!(hover.contains("Root doc."), "{hover}");
    // One occurrence in "Inherited from …", one in the Overrides link.
    assert_eq!(
        hover.matches("RootDoc#shared()").count(),
        2,
        "diamond override was not deduplicated: {hover}"
    );
}

#[test]
fn cross_file_override_uses_index_parent_javadoc() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("javadoc-spec")
        .join("cross-file");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("p")).unwrap();
    std::fs::write(
        root.join("p/Base.java"),
        "package p;\npublic class Base {\n    /** Base cross doc. */\n    public String value() { return \"\"; }\n}\n",
    )
    .unwrap();
    let child_source = "package p;\npublic class Child extends Base {\n    @Override\n    public String value() { return \"child\"; }\n}\n";
    let child_path = root.join("p/Child.java");
    std::fs::write(&child_path, child_source).unwrap();
    let mut workspace = project_session(&root);
    workspace.wait_for_index();
    WorkspaceService::diagnostics(&mut workspace, &child_path, Some(child_source));

    let hover = WorkspaceService::hover(
        &mut workspace,
        &child_path,
        position_of(
            child_source,
            "public String value() { return \"child\"; }",
            "public String ".len(),
        ),
    )
    .expect("cross-file hover");
    for expected in [
        "Base cross doc.",
        "Inherited from p.Base#value()",
        "**Overrides:**",
        "p.Base#value()](file://",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
}

#[test]
fn override_inherits_jdk_parent_javadoc() {
    if !jdk_src_available() {
        return;
    }
    let source = r#"class JdkOverride {
    @Override
    public String toString() {
        return "x";
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "JdkOverride", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public String toString() {",
        "public String ".len(),
    );
    for expected in [
        "Returns a string representation of the object.",
        "Inherited from java.lang.Object#toString()",
        "**Overrides:**",
        "java.lang.Object#toString()](file://",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
}

#[test]
fn nested_inherit_doc_expands_the_whole_chain() {
    let source = r#"class BaseDoc {
    /** Base doc. */
    void chain() {
    }
}

class MidDoc extends BaseDoc {
    /** {@inheritDoc} */
    @Override
    void chain() {
    }
}

class LeafDoc extends MidDoc {
    /** {@inheritDoc} */
    @Override
    void chain() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "InheritChain", source);
    let marker = "class LeafDoc extends MidDoc {\n    /** {@inheritDoc} */\n    @Override\n    void chain() {";
    let offset = marker.rfind("void chain").unwrap() + "void ".len();
    let hover = hover_at(&mut workspace, &path, source, marker, offset);
    assert!(
        hover.contains("Base doc."),
        "nested inheritDoc did not resolve: {hover}"
    );
    assert!(
        !hover.contains("{@inheritDoc: unresolved}"),
        "nested inheritDoc unresolved: {hover}"
    );
}

#[test]
fn overrides_are_capped_at_five_with_a_remainder_count() {
    let mut source = String::from("class LimitOverride implements ");
    for index in 0..6 {
        source.push_str(&format!("I{index}, "));
    }
    source.push_str("{\n    @Override\n    public void shared() {\n    }\n}\n\n");
    for index in 0..6 {
        source.push_str(&format!(
            "interface I{index} {{\n    /** Doc {index}. */\n    default void shared() {{\n    }}\n}}\n\n"
        ));
    }
    let source = source.as_str();
    let mut workspace = session();
    let path = overlay(&mut workspace, "OverrideLimit", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public void shared() {",
        "public void ".len(),
    );
    assert!(hover.contains("(+1 more)"), "cap not reported: {hover}");
    let overrides = hover
        .split("**Overrides:**")
        .nth(1)
        .unwrap_or_else(|| panic!("no overrides section: {hover}"));
    assert_eq!(
        overrides.matches("](file://").count()
            + overrides
                .lines()
                .filter(|line| line.starts_with("- I"))
                .count(),
        5,
        "more than five override entries rendered: {overrides}"
    );
}

#[test]
fn api_and_impl_tags_use_the_same_no_at_label_style_as_returns() {
    let source = r#"class Tagged {
    /**
     * Tagged body.
     * @apiNote See {@link java.util.List} and <a href="https://example.com">site</a>.
     * @implSpec Keep {@link java.util.Map}.
     * @implNote Keep note.
     * @return value
     */
    int tagged() {
        return 1;
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Tagged", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "int tagged() {",
        "int ".len(),
    );
    for expected in [
        "- **API Note** See",
        "`java.util.List`",
        "[site](https://example.com)",
        "- **Implementation Requirements** Keep",
        "`java.util.Map`",
        "- **Implementation Note** Keep note.",
        "- **Returns** value",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
    assert!(!hover.contains("**@apiNote**"), "{hover}");
    assert!(!hover.contains("**@implSpec**"), "{hover}");
}

#[test]
fn cross_file_index_inherit_doc_expands_the_parent_chain() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("javadoc-spec")
        .join("cross-chain");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("p")).unwrap();
    std::fs::write(
        root.join("p/Root.java"),
        "package p;\npublic class Root {\n    /** Root chain doc. */\n    public String value() { return \"\"; }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("p/Mid.java"),
        "package p;\npublic class Mid extends Root {\n    /** {@inheritDoc} */\n    @Override\n    public String value() { return \"mid\"; }\n}\n",
    )
    .unwrap();
    let child_source = "package p;\npublic class Child extends Mid {\n    @Override\n    public String value() { return \"child\"; }\n}\n";
    let child_path = root.join("p/Child.java");
    std::fs::write(&child_path, child_source).unwrap();
    let mut workspace = project_session(&root);
    workspace.wait_for_index();
    WorkspaceService::diagnostics(&mut workspace, &child_path, Some(child_source));

    let hover = WorkspaceService::hover(
        &mut workspace,
        &child_path,
        position_of(
            child_source,
            "public String value() { return \"child\"; }",
            "public String ".len(),
        ),
    )
    .expect("cross-file chain hover");
    assert!(hover.contains("Root chain doc."), "{hover}");
    assert!(
        hover.contains("Inherited from p.Mid#value()"),
        "{hover}"
    );
    assert!(!hover.contains("{@inheritDoc: unresolved}"), "{hover}");
}

#[test]
fn cross_file_index_inherit_doc_without_parent_is_unresolved() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("javadoc-spec")
        .join("cross-unresolved");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("p")).unwrap();
    std::fs::write(
        root.join("p/Root.java"),
        "package p;\npublic class Root {\n    public String value() { return \"\"; }\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("p/Mid.java"),
        "package p;\npublic class Mid extends Root {\n    /** {@inheritDoc} */\n    @Override\n    public String value() { return \"mid\"; }\n}\n",
    )
    .unwrap();
    let child_source = "package p;\npublic class Child extends Mid {\n    @Override\n    public String value() { return \"child\"; }\n}\n";
    let child_path = root.join("p/Child.java");
    std::fs::write(&child_path, child_source).unwrap();
    let mut workspace = project_session(&root);
    workspace.wait_for_index();
    WorkspaceService::diagnostics(&mut workspace, &child_path, Some(child_source));

    let hover = WorkspaceService::hover(
        &mut workspace,
        &child_path,
        position_of(
            child_source,
            "public String value() { return \"child\"; }",
            "public String ".len(),
        ),
    )
    .expect("cross-file unresolved hover");
    assert!(
        hover.contains("{@inheritDoc: unresolved}"),
        "index parent inheritDoc was silently dropped: {hover}"
    );
}

#[test]
fn snippet_renders_a_language_fence_and_file_form_is_marked() {
    let source = r#"class SnippetDoc {
    /**
     * Snippet body.
     * @implSpec
     * {@snippet lang=java :
     *     int value = 1;
     *     System.out.println(value);
     * }
     */
    public void inlineSnippet() {
    }

    /**
     * Unsupported file form.
     * @implSpec {@snippet file=MissingSnippet.java}
     */
    public void fileSnippet() {
    }

    /**
     * Language-less snippet.
     * @implSpec
     * {@snippet :
     *     plainSnippet();
     * }
     */
    public void noLanguageSnippet() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "SnippetDoc", source);
    let inline = hover_at(
        &mut workspace,
        &path,
        source,
        "public void inlineSnippet() {",
        "public void ".len(),
    );
    assert!(inline.contains("**Implementation Requirements**"), "{inline}");
    assert!(inline.contains("```java"), "{inline}");
    assert!(inline.contains("int value = 1;"), "{inline}");
    assert!(inline.contains("System.out.println(value);"), "{inline}");
    assert!(!inline.contains("{@snippet"), "{inline}");
    let file = hover_at(
        &mut workspace,
        &path,
        source,
        "public void fileSnippet() {",
        "public void ".len(),
    );
    assert!(
        file.contains("[snippet file unresolved: MissingSnippet.java]"),
        "{file}"
    );
    let no_language = hover_at(
        &mut workspace,
        &path,
        source,
        "public void noLanguageSnippet() {",
        "public void ".len(),
    );
    assert!(no_language.contains("```\n"), "{no_language}");
    assert!(no_language.contains("plainSnippet();"), "{no_language}");
}

#[test]
fn hover_signatures_carry_java_modifiers() {
    let source = r#"abstract class AbstractProbe {
    public abstract void abstractMethod();
}

class ModifierProbe {
    public final int field = 1;

    public ModifierProbe() {
    }

    public void publicMethod() {
    }

    private void privateMethod() {
    }

    protected void protectedMethod() {
    }

    public static void staticMethod() {
    }

    public final void finalMethod() {
    }

    public synchronized void synchronizedMethod() {
    }

    public native void nativeMethod();

    public strictfp void strictMethod() {
    }
}

interface DefaultProbe {
    default int defaultValue() {
        return 1;
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Modifiers", source);
    let cases = [
        ("public void publicMethod()", "public void publicMethod() {"),
        ("private void privateMethod()", "private void privateMethod() {"),
        ("protected void protectedMethod()", "protected void protectedMethod() {"),
        ("public static void staticMethod()", "public static void staticMethod() {"),
        ("public final void finalMethod()", "public final void finalMethod() {"),
        (
            "public synchronized void synchronizedMethod()",
            "public synchronized void synchronizedMethod() {",
        ),
        ("public native void nativeMethod()", "public native void nativeMethod();"),
        ("public strictfp void strictMethod()", "public strictfp void strictMethod() {"),
        (
            "public abstract void abstractMethod()",
            "public abstract void abstractMethod();",
        ),
        ("public ModifierProbe()", "public ModifierProbe() {"),
        ("public final int field", "public final int field"),
        ("default int defaultValue()", "default int defaultValue() {"),
    ];
    for (expected, marker) in cases {
        let hover = hover_at(
            &mut workspace,
            &path,
            source,
            marker,
            marker.find(expected.split_whitespace().last().unwrap_or("")).unwrap_or(0),
        );
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
}

#[test]
fn hover_header_shows_qualified_owner_type() {
    let source = r#"class Outer {
    static class Inner {
    }
}

class HeaderProbe {
    void method() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "HeaderProbe", source);
    let method = hover_at(
        &mut workspace,
        &path,
        source,
        "void method() {",
        "void ".len(),
    );
    assert!(method.contains("HeaderProbe\n"), "{method}");
    assert!(method.contains("```java\nvoid method()"), "{method}");

    let nested = hover_at(
        &mut workspace,
        &path,
        source,
        "class Inner {",
        "class ".len(),
    );
    assert!(nested.contains("Outer.Inner\n"), "{nested}");
}

#[test]
fn hover_source_annotation_distinguishes_project_and_jdk() {
    let source = r#"class SourceProbe {
    Object object;

    void projectMethod() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "SourceProbe", source);
    let project = hover_at(
        &mut workspace,
        &path,
        source,
        "void projectMethod() {",
        "void ".len(),
    );
    assert!(
        project.contains("*Source: ") && project.contains("SourceProbe.java"),
        "{project}"
    );

    if jdk_src_available() {
        let jdk = hover_at(
            &mut workspace,
            &path,
            source,
            "Object object;",
            0,
        );
        assert!(jdk.contains("java.lang.Object"), "{jdk}");
        assert!(jdk.contains("src.zip"), "{jdk}");
    }
}

#[test]
fn completion_detail_stays_lean_without_modifiers() {
    let source_with_cursor = r#"class CompletionLean {
    public void visibleMethod() {
    }

    void caller() {
        vi|
    }
}
"#;
    let cursor = source_with_cursor.find('|').unwrap();
    let source = source_with_cursor.replacen('|', "", 1);
    let before = &source[..cursor];
    let line = before.matches('\n').count() as u32;
    let column = before
        .rsplit('\n')
        .next()
        .unwrap()
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    let mut workspace = session();
    let path = overlay(&mut workspace, "CompletionLean", &source);
    let items = WorkspaceService::completion(&mut workspace, &path, Position::new(line, column));
    let item = items
        .iter()
        .find(|item| item.label == "visibleMethod")
        .unwrap_or_else(|| panic!("visibleMethod missing from {items:?}"));
    let detail = item.detail.clone().unwrap_or_default();
    assert_eq!(detail, "void", "completion detail should stay lean: {detail}");
    assert!(!detail.contains("public "), "completion detail has modifier: {detail}");
}

#[test]
fn tag_block_content_has_no_blockquote_prefix() {
    let source = r#"class TagBlocks {
    /**
     * Body.
     * @implSpec
     * Value:
     * <pre>
     * getClass().getName()
     * </pre>
     *
     * <blockquote>quoted prose</blockquote>
     */
    public void method() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "TagBlocks", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public void method() {",
        "public void ".len(),
    );
    assert!(hover.contains("**Implementation Requirements**"), "{hover}");
    assert!(hover.contains("```\ngetClass().getName()\n```"), "{hover}");
    assert!(!hover.contains("> ```"), "tag code block was quoted: {hover}");
    // An explicit prose blockquote still uses the quote prefix.
    assert!(hover.contains("> quoted prose"), "{hover}");
}

#[test]
fn blocks_after_pre_and_lists_are_not_dropped() {
    let source = r#"class TagTail {
    /**
     * Body before.
     * @implSpec
     * Value:
     * <pre>
     * getClass().getName()
     * </pre>
     * The {@link java.util.Objects#toIdentityString(Object) Objects.toIdentityString}
     * method returns the string for an object.
     *
     * <ul>
     * <li>first item</li>
     * </ul>
     * Tail after list.
     */
    public void method() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "TagTail", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public void method() {",
        "public void ".len(),
    );
    for expected in [
        "**Implementation Requirements**",
        "```\ngetClass().getName()\n```",
        "toIdentityString",
        "method returns the string for an object.",
        "- first item",
        "Tail after list.",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
    let code = hover.find("```\ngetClass().getName()\n```").expect("tag code");
    let tail = hover.find("toIdentityString").expect("tag tail");
    let item = hover.find("- first item").expect("list item");
    let after_list = hover.find("Tail after list.").expect("tail after list");
    assert!(code < tail && tail < item && item < after_list, "{hover}");
}

#[test]
fn body_pre_siblings_are_all_rendered() {
    let source = r#"class BodyBlocks {
    /**
     * <p>Before paragraph.</p>
     * <pre>
     * int bodyCode = 1;
     * </pre>
     * <p>After paragraph.</p>
     */
    public void method() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "BodyBlocks", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public void method() {",
        "public void ".len(),
    );
    for expected in [
        "Before paragraph.",
        "```\nint bodyCode = 1;\n```",
        "After paragraph.",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
    let before = hover.find("Before paragraph.").expect("before");
    let code = hover.find("int bodyCode = 1;").expect("body code");
    let after = hover.find("After paragraph.").expect("after");
    assert!(before < code && code < after, "{hover}");
}

#[test]
fn jdk_style_blockquote_pre_then_tail_paragraph_survives() {
    let source = r#"class JdkStyleImplSpec {
    /**
     * @implSpec
     * Value:
     * <blockquote>
     * <pre>
     * getClass().getName()
     * </pre>
     * </blockquote>
     * The {@link java.util.Objects#toIdentityString(Object) Objects.toIdentityString}
     * method returns the string for an object.
     */
    public void method() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "JdkStyleImplSpec", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public void method() {",
        "public void ".len(),
    );
    for expected in [
        "```\ngetClass().getName()\n```",
        "toIdentityString",
        "method returns the string for an object.",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
    assert!(!hover.contains("> ```"), "{hover}");
    let code = hover.find("```\ngetClass().getName()\n```").expect("code fence");
    let tail = hover.find("toIdentityString").expect("tail paragraph");
    assert!(code < tail, "tail paragraph came before the code block: {hover}");
}

#[test]
fn mixed_blockquote_pre_tail_inside_quote_survives() {
    let source = r#"class MixedQuote {
    /**
     * @implSpec
     * <blockquote>
     * <pre>
     * getClass().getName()
     * </pre>
     * The {@link java.util.Objects#toIdentityString(Object) Objects.toIdentityString}
     * method returns the string for an object.
     * </blockquote>
     */
    public void method() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "MixedQuote", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public void method() {",
        "public void ".len(),
    );
    for expected in [
        "> ```\n> getClass().getName()\n> ```",
        "toIdentityString",
        "method returns the string for an object.",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
}

#[test]
fn snippet_then_tail_paragraph_in_tag_survives() {
    let source = r#"class SnippetTail {
    /**
     * @implSpec
     * The toString method returns a value:
     * {@snippet lang=java :
     * getClass().getName() + '@' + Integer.toHexString(hashCode())
     * }
     * The {@link java.util.Objects#toIdentityString(Object) Objects.toIdentityString}
     * method returns the string for an object equal to the string that would be
     * returned if neither toString nor hashCode were overridden.
     */
    public void method() {
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "SnippetTail", source);
    let hover = hover_at(
        &mut workspace,
        &path,
        source,
        "public void method() {",
        "public void ".len(),
    );
    for expected in [
        "**Implementation Requirements**",
        "```java\ngetClass().getName() + '@' + Integer.toHexString(hashCode())\n```",
        "toIdentityString",
        "method returns the string for an object equal",
        "neither toString nor hashCode were overridden",
    ] {
        assert!(hover.contains(expected), "missing {expected:?}: {hover}");
    }
    let code = hover
        .find("```java\ngetClass().getName() + '@' + Integer.toHexString(hashCode())\n```")
        .expect("snippet fence");
    let tail = hover.find("toIdentityString").expect("tail paragraph");
    assert!(code < tail, "tail paragraph came before the snippet: {hover}");
}

fn jdk_src_available() -> bool {
    if let Some(platform) = std::env::var_os("RAVAC_PLATFORM_DIR") {
        if PathBuf::from(platform).join("src.zip").is_file() {
            return true;
        }
    }
    if let Some(path) = std::env::var_os("RAVAC_JDK_SRC") {
        return PathBuf::from(path).exists();
    }
    std::env::var_os("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("lib").join("src.zip").is_file())
        .unwrap_or(false)
}

#[test]
fn hover_reads_javadoc_of_class_path_symbols_from_the_jdk_sources() {
    if !jdk_src_available() {
        return;
    }
    let source = "class JdkDoc {\n    java.io.Writer writer;\n    java.util.TreeMap map;\n}\n";
    let mut workspace = session();
    let path = overlay(&mut workspace, "JdkDoc", source);
    let hover = hover_at(&mut workspace, &path, source, "java.io.Writer writer", "java.io.".len());
    for expected in [
        "Abstract class for writing to character streams",
        "Most subclasses, however, will override some of the methods",
        "- **See** BufferedWriter",
        "- **Author** Mark Reinhold",
        "- **Since** 1.1",
    ] {
        assert!(hover.contains(expected), "JDK hover missing {expected:?}: {hover}");
    }

    // A multi-paragraph JDK class: external Javadoc must also go through the
    // AST renderer, not a string-level HTML scan.
    let tree_map = hover_at(
        &mut workspace,
        &path,
        source,
        "java.util.TreeMap map",
        "java.util.".len(),
    );
    for expected in [
        "A Red-Black tree based",
        "This implementation provides guaranteed log(n) time cost",
        "Note that this implementation is not synchronized.",
        "SortedMap m = Collections.synchronizedSortedMap(new TreeMap(...));",
    ] {
        assert!(
            tree_map.contains(expected),
            "JDK paragraph missing {expected:?}: {tree_map}"
        );
    }
}

#[test]
fn reference_hover_shows_the_declaration_javadoc() {
    let source = r#"/** Class doc. */
class Caller {
    /** Callee doc. */
    void callee() {
    }

    void use() {
        callee();
    }
}
"#;
    let mut workspace = session();
    let path = overlay(&mut workspace, "Caller", source);
    let hover = hover_at(&mut workspace, &path, source, "        callee();", 8);
    assert!(hover.contains("Callee doc."), "reference hover: {hover}");
}

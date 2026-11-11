//! Structured Markdown rendering of Javadoc trees.
//!
//! One renderer is shared by project sources (`DCDocComment` from the parser)
//! and external sources (JDK src.zip): external Javadoc text is turned into a
//! `DCTree` by `DocCommentParser` first, then rendered here.
//!
//! Block elements are parsed recursively (so nested lists, tables and
//! blockquotes keep their structure); inline elements are rendered with the
//! matching inline state.

use std::rc::Rc;

use javac_source::doctree::doc_tree::Kind;

use crate::tree::dc_tree::{DCKind, DCDocComment, DCTree};
use crate::util::list::List;

/// Render one parsed doc comment as Markdown.
pub(crate) fn format_doc_comment(tree: &DCTree) -> Option<String> {
    let DCKind::DocComment(comment) = &tree.kind else {
        return None;
    };
    // `body` only contains the part after the first sentence
    // (DocTreeMaker::split_body); all visible prose comes from `full_body`.
    let body_nodes = nodes(&comment.full_body);
    let mut out = render_blocks(&body_nodes);
    let tags = render_tags(comment);
    if !tags.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&tags);
    }
    let out = out.trim();
    (!out.is_empty()).then(|| out.to_string())
}

fn nodes(list: &List<Rc<DCTree>>) -> Vec<Rc<DCTree>> {
    list.iter().collect()
}

fn lower(name: &crate::util::name::Name) -> String {
    name.to_java_string().to_string_lossy().to_ascii_lowercase()
}

// ---------------------------------------------------------------------------
// Block rendering
// ---------------------------------------------------------------------------

fn render_blocks(nodes: &[Rc<DCTree>]) -> String {
    let mut blocks: Vec<String> = Vec::new();
    let mut paragraph = String::new();
    let mut index = 0usize;
    while index < nodes.len() {
        match &nodes[index].kind {
            DCKind::Text { text } => paragraph.push_str(&text.to_string_lossy()),
            DCKind::Entity { name } => paragraph.push_str(&decode_entity(
                &name.to_java_string().to_string_lossy(),
            )),
            DCKind::StartElement {
                name,
                attrs,
                self_closing,
                ..
            } => {
                let tag = lower(name);
                if is_block_tag(&tag) {
                    flush_paragraph(&mut blocks, &mut paragraph);
                    if *self_closing || tag == "hr" {
                        if tag == "hr" {
                            blocks.push("---".to_string());
                        }
                        index += 1;
                        continue;
                    }
                    let (inner_end, next) = if matches!(tag.as_str(), "p" | "li" | "tr" | "td" | "th")
                    {
                        optional_end(nodes, index, &tag)
                    } else if let Some(end) = matching_end(nodes, index, &tag) {
                        (end, end + 1)
                    } else {
                        (nodes.len(), nodes.len())
                    };
                    let inner = &nodes[index + 1..inner_end];
                    let block = match tag.as_str() {
                        "p" => inline_trimmed(inner),
                        "ul" => render_list(inner, false, 0),
                        "ol" => render_list(inner, true, 0),
                        "pre" => render_pre(inner),
                        "blockquote" => render_blockquote(inner),
                        "table" => render_table(inner),
                        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                            let level = tag[1..].parse::<usize>().unwrap_or(1);
                            let text = inline_trimmed(inner);
                            if text.is_empty() {
                                String::new()
                            } else {
                                format!("{} {}", "#".repeat(level), text)
                            }
                        }
                        _ => String::new(),
                    };
                    if !block.trim().is_empty() {
                        blocks.push(block);
                    }
                    index = next;
                    continue;
                }
                if tag == "br" {
                    paragraph.push_str("  \n");
                    index += 1;
                    continue;
                }
                let (end, next) = element_range(nodes, index, &tag);
                paragraph.push_str(&render_inline(&nodes[index..end]));
                index = next;
                continue;
            }
            DCKind::EndElement { .. } => {
                // A stray end tag (e.g. after a self-closing block) is ignored.
            }
            _ => paragraph.push_str(&render_inline_node(&nodes[index])),
        }
        index += 1;
    }
    flush_paragraph(&mut blocks, &mut paragraph);
    blocks.join("\n\n")
}

fn flush_paragraph(blocks: &mut Vec<String>, paragraph: &mut String) {
    let text = tidy_paragraph(paragraph);
    if !text.is_empty() {
        blocks.push(text);
    }
    paragraph.clear();
}

/// Collapse Javadoc's soft line wraps to spaces without losing explicit
/// `<br>` hard breaks (two trailing spaces before the newline).
fn tidy_paragraph(text: &str) -> String {
    let mut out = String::new();
    let mut hard_break = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if !out.is_empty() {
            if hard_break {
                out.push_str("  \n");
            } else {
                out.push(' ');
            }
        }
        out.push_str(line);
        hard_break = raw.ends_with("  ");
    }
    out
}

fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "ul" | "ol" | "pre" | "blockquote" | "table" | "hr" | "dl"
            | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

/// End of an HTML element whose closing tag is optional (`<p>`, `<li>`, table
/// cells). Javadoc rarely closes `<p>`; its scope ends at the next block start
/// or the closing tag of an ancestor.
fn optional_end(nodes: &[Rc<DCTree>], start: usize, name: &str) -> (usize, usize) {
    let mut index = start + 1;
    while index < nodes.len() {
        match &nodes[index].kind {
            DCKind::StartElement { name: tag, .. } => {
                let tag = lower(tag);
                if tag == name && matches!(name, "li" | "tr" | "td" | "th") {
                    return (index, index);
                }
                if name == "p" && is_block_tag(&tag) {
                    return (index, index);
                }
                if name == "li" && matches!(tag.as_str(), "ul" | "ol" | "li") {
                    return (index, index);
                }
            }
            DCKind::EndElement { name: tag, .. } => {
                let tag = lower(tag);
                if tag == name {
                    return (index, index + 1);
                }
                if is_block_tag(&tag) || tag == "li" {
                    return (index, index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    (nodes.len(), nodes.len())
}

/// Index of the `EndElement` matching the start at `start`, honouring nesting.
fn element_range(nodes: &[Rc<DCTree>], start: usize, name: &str) -> (usize, usize) {
    match matching_end(nodes, start, name) {
        Some(end) => (end, end + 1),
        None => (nodes.len(), nodes.len()),
    }
}

fn matching_end(nodes: &[Rc<DCTree>], start: usize, name: &str) -> Option<usize> {
    let mut depth = 1usize;
    let mut index = start + 1;
    while index < nodes.len() {
        match &nodes[index].kind {
            DCKind::StartElement { name: tag, .. } if lower(tag) == name => depth += 1,
            DCKind::EndElement { name: tag, .. } if lower(tag) == name => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn render_list(nodes: &[Rc<DCTree>], ordered: bool, depth: usize) -> String {
    let mut items: Vec<String> = Vec::new();
    let mut number = 1usize;
    let mut index = 0usize;
    while index < nodes.len() {
        if let DCKind::StartElement { name, .. } = &nodes[index].kind
            && lower(name) == "li"
        {
            let (end, next) = element_range(nodes, index, "li");
            let inner = &nodes[index + 1..end];
            items.push(render_list_item(inner, ordered, depth, number));
            number += 1;
            index = next;
            continue;
        }
        index += 1;
    }
    items.join("\n")
}

fn render_list_item(nodes: &[Rc<DCTree>], ordered: bool, depth: usize, number: usize) -> String {
    let indent = "  ".repeat(depth);
    let marker = if ordered {
        format!("{number}. ")
    } else {
        "- ".to_string()
    };
    let mut inline = String::new();
    let mut nested: Vec<String> = Vec::new();
    let mut index = 0usize;
    while index < nodes.len() {
        match &nodes[index].kind {
            DCKind::StartElement { name, .. } => {
                let tag = lower(name);
                if is_block_tag(&tag) || tag == "li" {
                    let (end, next) = element_range(nodes, index, &tag);
                    let inner = &nodes[index + 1..end];
                    let block = match tag.as_str() {
                        "ul" => render_list(inner, false, depth + 1),
                        "ol" => render_list(inner, true, depth + 1),
                        "pre" => render_pre(inner),
                        "blockquote" => render_blockquote(inner),
                        "table" => render_table(inner),
                        "p" => tidy_paragraph(&render_inline(inner)),
                        _ => String::new(),
                    };
                    if !block.trim().is_empty() {
                        nested.push(block);
                    }
                    index = next;
                    continue;
                }
                let (end, next) = element_range(nodes, index, &tag);
                inline.push_str(&render_inline(&nodes[index..end]));
                index = next;
                continue;
            }
            _ => inline.push_str(&render_inline_node(&nodes[index])),
        }
        index += 1;
    }
    let mut out = format!("{indent}{marker}{}", inline.trim());
    for block in nested {
        out.push('\n');
        out.push_str(&block);
    }
    out
}

fn render_pre(nodes: &[Rc<DCTree>]) -> String {
    let raw = raw_text(nodes);
    let mut lines: Vec<String> = raw
        .replace("\r\n", "\n")
        .lines()
        .map(str::to_string)
        .collect();
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return String::new();
    }
    // Javadoc's `* ` prefix leaves one leading space on every line; remove the
    // common indentation but keep indentation relative to the block.
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    for line in &mut lines {
        if line.len() >= indent {
            line.drain(..indent);
        }
    }
    let code = lines.join("\n");
    let fence = "`".repeat((longest_backtick_run(&code) + 1).max(3));
    format!("{fence}\n{code}\n{fence}")
}

fn render_blockquote(nodes: &[Rc<DCTree>]) -> String {
    let inner = render_blocks(nodes);
    inner
        .lines()
        .map(|line| {
            if line.is_empty() {
                ">".to_string()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_table(nodes: &[Rc<DCTree>]) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut index = 0usize;
    while index < nodes.len() {
        if let DCKind::StartElement { name, .. } = &nodes[index].kind
            && lower(name) == "tr"
        {
            let (end, next) = element_range(nodes, index, "tr");
            let mut cells: Vec<String> = Vec::new();
            let mut cell = 0usize;
            let row_nodes = &nodes[index + 1..end];
            while cell < row_nodes.len() {
                if let DCKind::StartElement { name, .. } = &row_nodes[cell].kind
                    && matches!(lower(name).as_str(), "th" | "td")
                {
                    let tag = lower(name);
                    let (cell_end, cell_next) = element_range(row_nodes, cell, &tag);
                    cells.push(inline_trimmed(&row_nodes[cell + 1..cell_end]));
                    cell = cell_next;
                    continue;
                }
                cell += 1;
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
            index = next;
            continue;
        }
        index += 1;
    }
    if rows.is_empty() {
        return String::new();
    }
    let columns = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    if columns == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (row_index, row) in rows.iter().enumerate() {
        out.push('|');
        for column in 0..columns {
            let cell = row
                .get(column)
                .map(|cell| cell.replace('|', "\\|").replace('\n', " "))
                .unwrap_or_default();
            out.push(' ');
            out.push_str(&cell);
            out.push_str(" |");
        }
        out.push('\n');
        if row_index == 0 {
            out.push('|');
            for _ in 0..columns {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// Inline rendering
// ---------------------------------------------------------------------------

fn render_inline(nodes: &[Rc<DCTree>]) -> String {
    let mut out = String::new();
    let mut index = 0usize;
    while index < nodes.len() {
        match &nodes[index].kind {
            DCKind::StartElement {
                name,
                attrs,
                self_closing,
                ..
            } => {
                let tag = lower(name);
                if tag == "br" {
                    out.push_str("  \n");
                    index += 1;
                    continue;
                }
                let (end, next) = element_range(nodes, index, &tag);
                let inner = &nodes[index + 1..end];
                match tag.as_str() {
                    "b" | "strong" => {
                        push_wrapped(&mut out, "**", &inline_trimmed(inner), "**")
                    }
                    "i" | "em" => push_wrapped(&mut out, "*", &inline_trimmed(inner), "*"),
                    "s" | "del" => push_wrapped(&mut out, "~~", &inline_trimmed(inner), "~~"),
                    "code" => out.push_str(&code_span(&raw_text(inner))),
                    "a" => {
                        let text = inline_trimmed(inner);
                        match attribute(attrs, "href") {
                            Some(href) if !href.trim().is_empty() => {
                                out.push_str(&format!("[{text}]({href})"))
                            }
                            _ => out.push_str(&text),
                        }
                    }
                    "br" => out.push_str("  \n"),
                    "pre" => push_block(&mut out, render_pre(inner)),
                    "ul" => push_block(&mut out, render_list(inner, false, 0)),
                    "ol" => push_block(&mut out, render_list(inner, true, 0)),
                    "table" => push_block(&mut out, render_table(inner)),
                    "blockquote" => push_block(&mut out, render_blockquote(inner)),
                    "p" => {
                        if !out.is_empty() && !out.ends_with('\n') {
                            out.push('\n');
                        }
                        out.push_str(&inline_trimmed(inner));
                    }
                    _ => out.push_str(&render_inline(inner)),
                }
                index = next;
                continue;
            }
            _ => out.push_str(&render_inline_node(&nodes[index])),
        }
        index += 1;
    }
    out
}

fn inline_trimmed(nodes: &[Rc<DCTree>]) -> String {
    render_inline(nodes).trim().to_string()
}

/// Javadoc wraps prose at arbitrary columns; inside a paragraph those line
/// breaks are soft. Collapse them (and the leading `* ` indentation the parser
/// leaves behind) into single spaces. `<pre>` bypasses this via `raw_text`.
fn normalize_inline_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let leading = text.chars().next().is_some_and(char::is_whitespace);
    let trailing = text.chars().last().is_some_and(char::is_whitespace);
    let core = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if core.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    if leading {
        out.push(' ');
    }
    out.push_str(&core);
    if trailing {
        out.push(' ');
    }
    out
}

fn list_trimmed(list: &List<Rc<DCTree>>) -> String {
    inline_trimmed(&nodes(list))
}

fn render_inline_node(node: &DCTree) -> String {
    match &node.kind {
        DCKind::Text { text } => normalize_inline_text(&text.to_string_lossy()),
        DCKind::Entity { name } => decode_entity(&name.to_java_string().to_string_lossy()),
        DCKind::Literal { kind, body, .. } => {
            let text = raw_text_node(body);
            if *kind == Kind::CODE {
                code_span(&text)
            } else {
                text
            }
        }
        DCKind::Link {
            reference, label, ..
        } => render_link(reference, label),
        DCKind::Reference { signature, .. } => signature.to_string_lossy(),
        DCKind::Identifier { name } => name.to_java_string().to_string_lossy(),
        DCKind::Erroneous { body, .. } => body.to_string_lossy(),
        DCKind::Comment { body } => body.to_string_lossy(),
        DCKind::InheritDoc { .. } => "{@inheritDoc}".to_string(),
        DCKind::Summary { summary, .. } => render_inline(&nodes(summary)),
        DCKind::Index {
            term, description, ..
        } => {
            let term = render_inline_node(term);
            let description = list_trimmed(description);
            if description.is_empty() {
                term
            } else {
                format!("{term} {description}")
            }
        }
        DCKind::Value { reference, .. } => render_inline_node(reference),
        DCKind::SystemProperty { property_name, .. } => format!("${{{}}}", property_name.to_java_string().to_string_lossy()),
        DCKind::UnknownInlineTag { name, content, .. } => {
            format!(
                "@{} {}",
                name.to_java_string().to_string_lossy(),
                list_trimmed(content)
            )
        }
        _ => String::new(),
    }
}

fn render_link(reference: &Rc<DCTree>, label: &List<Rc<DCTree>>) -> String {
    let target = match &reference.kind {
        DCKind::Reference { signature, .. } => signature.to_string_lossy(),
        _ => render_inline_node(reference),
    };
    let label_text = list_trimmed(label);
    if label_text.is_empty() {
        code_span(&target)
    } else {
        // Keep both the label and the Javadoc target visible: there is no URL
        // to attach, but dropping the target loses information the reader
        // needs to find the referenced member.
        format!("{label_text} (`{}`)", target.trim())
    }
}

fn push_block(out: &mut String, block: String) {
    if block.trim().is_empty() {
        return;
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&block);
    out.push('\n');
}

fn push_wrapped(out: &mut String, open: &str, text: &str, close: &str) {
    if text.is_empty() {
        return;
    }
    out.push_str(open);
    out.push_str(text);
    out.push_str(close);
}

fn attribute(attrs: &List<Rc<DCTree>>, name: &str) -> Option<String> {
    for attr in attrs.iter() {
        if let DCKind::Attribute {
            name: attr_name,
            value,
            ..
        } = &attr.kind
        {
            if lower(attr_name) == name {
                return value.as_ref().map(|value| inline_trimmed(&nodes(value)));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

fn render_tags(comment: &DCDocComment) -> String {
    let mut lines: Vec<String> = Vec::new();
    for tag in comment.tags.iter() {
        let line = match &tag.kind {
            DCKind::Param {
                is_type_parameter,
                name,
                description,
                ..
            } => {
                let mut name_text = render_inline_node(name).trim().to_string();
                if *is_type_parameter && !name_text.starts_with('<') {
                    name_text = format!("<{name_text}>");
                }
                let name_text = if name_text.starts_with('<') && name_text.ends_with('>') {
                    code_span(&name_text)
                } else {
                    name_text
                };
                Some(format!(
                    "- **{}** {}",
                    name_text,
                    list_trimmed(description)
                ))
            }
            DCKind::Return { description, .. } => {
                Some(format!("- **Returns** {}", list_trimmed(description)))
            }
            DCKind::Throws {
                name, description, ..
            } => Some(format!(
                "- **Throws** {} {}",
                render_inline_node(name).trim(),
                list_trimmed(description)
            )),
            DCKind::See { reference } => Some(format!(
                "- **See** {}",
                list_trimmed(reference)
            )),
            DCKind::Since { body } => Some(format!("- **Since** {}", list_trimmed(body))),
            DCKind::Author { name } => Some(format!("- **Author** {}", list_trimmed(name))),
            DCKind::Version { body } => Some(format!("- **Version** {}", list_trimmed(body))),
            DCKind::Deprecated { body } => {
                Some(format!("- **Deprecated** {}", list_trimmed(body)))
            }
            DCKind::UnknownBlockTag { name, content } => Some(format!(
                "- **@{}** {}",
                name.to_java_string().to_string_lossy(),
                list_trimmed(content)
            )),
            _ => None,
        };
        if let Some(line) = line {
            lines.push(line);
        }
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

fn raw_text(nodes: &[Rc<DCTree>]) -> String {
    let mut out = String::new();
    let mut index = 0usize;
    while index < nodes.len() {
        match &nodes[index].kind {
            DCKind::StartElement { name, .. } => {
                let tag = lower(name);
                let (end, next) = element_range(nodes, index, &tag);
                out.push_str(&raw_text(&nodes[index + 1..end]));
                index = next;
                continue;
            }
            _ => out.push_str(&raw_text_node(&nodes[index])),
        }
        index += 1;
    }
    out
}

fn raw_text_node(node: &DCTree) -> String {
    match &node.kind {
        DCKind::Text { text } => text.to_string_lossy(),
        DCKind::Entity { name } => decode_entity(&name.to_java_string().to_string_lossy()),
        DCKind::Literal { body, .. } => raw_text_node(body),
        DCKind::EndElement { .. } => String::new(),
        _ => render_inline_node(node),
    }
}

fn code_span(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let fence = "`".repeat((longest_backtick_run(text) + 1).max(1));
    if fence.len() == 1 && !text.starts_with('`') && !text.ends_with('`') {
        return format!("`{text}`");
    }
    // CommonMark: a longer fence needs a space between it and content that
    // begins/ends with a backtick.
    let needs_space = text.starts_with('`') || text.ends_with('`') || fence.len() > 1;
    if needs_space {
        format!("{fence} {text} {fence}")
    } else {
        format!("{fence}{text}{fence}")
    }
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0usize;
    let mut current = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn decode_entity(name: &str) -> String {
    let name = name.trim();
    if let Some(number) = name.strip_prefix('#') {
        let value = if let Some(hex) = number
            .strip_prefix('x')
            .or_else(|| number.strip_prefix('X'))
        {
            u32::from_str_radix(hex, 16).ok()
        } else {
            number.parse::<u32>().ok()
        };
        if let Some(ch) = value.and_then(char::from_u32) {
            return ch.to_string();
        }
        return format!("&{name};");
    }
    match name {
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "amp" => "&".to_string(),
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        "nbsp" => " ".to_string(),
        _ => format!("&{name};"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokens::{Comment, CommentStyle};
    use crate::util::java_string::JavaString;

    struct TestComment;

    impl Comment for TestComment {
        fn get_text(&self) -> Option<JavaString> {
            None
        }

        fn get_source_pos(&self, index: i32) -> i32 {
            index
        }

        fn get_style(&self) -> CommentStyle {
            CommentStyle::JAVADOC
        }

        fn is_deprecated(&self) -> bool {
            false
        }
    }

    fn empty_comment() -> Rc<DCTree> {
        Rc::new(DCTree::new(
            0,
            DCKind::DocComment(DCDocComment {
                comment: Rc::new(TestComment),
                full_body: List::nil(),
                first_sentence: List::nil(),
                body: List::nil(),
                tags: List::nil(),
                preamble: List::nil(),
                postamble: List::nil(),
            }),
        ))
    }

    #[test]
    fn empty_javadoc_renders_none() {
        assert!(format_doc_comment(&empty_comment()).is_none());
    }
}

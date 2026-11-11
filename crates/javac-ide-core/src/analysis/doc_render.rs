//! Markdown rendering of Javadoc trees.

use std::rc::Rc;

use crate::tree::dc_tree::{DCKind, DCDocComment, DCTree};

/// Render one parsed doc comment as Markdown.
pub(crate) fn format_doc_comment(tree: &DCTree) -> Option<String> {
    let DCKind::DocComment(comment) = &tree.kind else {
        return None;
    };
    let mut out = String::new();
    // `body` is the part after the first sentence (DocTreeMaker::split_body);
    // hover shows the whole Javadoc, so render `full_body` and fall back to
    // first sentence + body for tables that only carry the split form.
    let body_text = if !comment.full_body.is_empty() {
        render_nodes(&comment.full_body)
    } else if !comment.first_sentence.is_empty() || !comment.body.is_empty() {
        format!(
            "{}{}",
            render_nodes(&comment.first_sentence),
            render_nodes(&comment.body)
        )
    } else {
        String::new()
    };
    let body = body_text.trim();
    if !body.is_empty() {
        out.push_str(body);
    }
    let tags = render_tags(comment);
    if !tags.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&tags);
    }
    if out.trim().is_empty() {
        None
    } else {
        Some(out.trim().to_string())
    }
}

fn render_tags(comment: &DCDocComment) -> String {
    let mut lines: Vec<String> = Vec::new();
    for tag in comment.tags.iter() {
        let line = match &tag.kind {
            DCKind::Param { name, description, .. } => Some(format!(
                "- **{label}** {text}",
                label = render_one(name),
                text = render_nodes(description).trim()
            )),
            DCKind::Return { description, .. } => {
                Some(format!("- **Returns** {}", render_nodes(description).trim()))
            }
            DCKind::Throws { name, description, .. } => Some(format!(
                "- **Throws** {} {}",
                render_one(name),
                render_nodes(description).trim()
            )),
            DCKind::See { reference } => {
                Some(format!("- **See** {}", render_nodes(reference).trim()))
            }
            DCKind::Since { body } => Some(format!("- **Since** {}", render_nodes(body).trim())),
            DCKind::Deprecated { body } => {
                Some(format!("- **Deprecated** {}", render_nodes(body).trim()))
            }
            DCKind::Author { name } => Some(format!("- **Author** {}", render_nodes(name).trim())),
            DCKind::Version { body } => Some(format!("- **Version** {}", render_nodes(body).trim())),
            DCKind::UnknownBlockTag { name, content } => Some(format!(
                "- **@{}** {}",
                name.to_java_string().to_string_lossy(),
                render_nodes(content).trim()
            )),
            _ => None,
        };
        if let Some(line) = line {
            lines.push(line);
        }
    }
    lines.join("\n")
}

fn render_nodes(nodes: &crate::util::list::List<Rc<DCTree>>) -> String {
    let mut out = String::new();
    for node in nodes.iter() {
        out.push_str(&render_one(node.as_ref()));
    }
    out
}

fn render_one(node: &DCTree) -> String {
    match &node.kind {
        DCKind::Text { text } => text.to_string_lossy(),
        DCKind::Literal { kind, body, .. } => {
            let text = render_one(body);
            if *kind == javac_source::doctree::doc_tree::Kind::CODE {
                format!("`{text}`")
            } else {
                text
            }
        }
        DCKind::Link {
            reference, label, ..
        } => {
            let label_text = render_nodes(label);
            if label_text.trim().is_empty() {
                // Keep the Javadoc target visible: a code span survives
                // Markdown rendering and does not pretend to be a URL.
                format!("`{}`", render_one(reference).trim())
            } else {
                label_text
            }
        }
        DCKind::Reference { signature, .. } => signature.to_string_lossy(),
        DCKind::Identifier { name } => name.to_java_string().to_string_lossy(),
        DCKind::Erroneous { body, .. } => body.to_string_lossy(),
        DCKind::Comment { body } => body.to_string_lossy(),
        DCKind::StartElement { name, .. } => format!("<{}>", name.to_java_string().to_string_lossy()),
        DCKind::EndElement { name, .. } => format!("</{}>", name.to_java_string().to_string_lossy()),
        DCKind::Entity { name } => format!("&{};", name.to_java_string().to_string_lossy()),
        DCKind::InheritDoc { .. } => String::new(),
        DCKind::Summary { summary, .. } => render_nodes(summary),
        DCKind::Index {
            term, description, ..
        } => {
            let term = render_one(term);
            let description = render_nodes(description);
            if description.trim().is_empty() {
                term
            } else {
                format!("{term} {description}")
            }
        }
        DCKind::Value { reference, .. } => render_one(reference),
        DCKind::Param { name, description, .. } => format!(
            "{}{}",
            render_one(name),
            render_nodes(description)
        ),
        _ => String::new(),
    }
}

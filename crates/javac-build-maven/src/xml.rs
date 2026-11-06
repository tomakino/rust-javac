//! XML parsing for Maven POM files (thin wrapper over quick-xml).
//!
//! POM files only need elements, attributes and text; this module builds a
//! small DOM with that surface so the Maven model code stays readable.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// One parsed XML element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// Local name.
    pub name: String,
    /// Attributes in document order.
    pub attributes: Vec<(String, String)>,
    /// Child elements.
    pub children: Vec<Element>,
    /// Concatenated direct text content (trimmed).
    pub text: String,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }

    pub fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name).map(|child| child.text.trim())
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }
}

/// Parse an XML document into its root element.
pub fn parse(text: &str) -> Option<Element> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => stack.push(element_from_start(&start)),
            Ok(Event::Empty(start)) => {
                let element = element_from_start(&start);
                close_element(&mut stack, &mut root, element);
            }
            Ok(Event::End(_)) => {
                if let Some(element) = stack.pop() {
                    close_element(&mut stack, &mut root, element);
                }
            }
            Ok(Event::Text(data)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(data.as_ref());
                }
            }
            Ok(Event::CData(data)) => {
                if let Some(current) = stack.last_mut() {
                    current
                        .text
                        .push_str(data.as_ref());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }
    root
}

fn element_from_start(start: &BytesStart<'_>) -> Element {
    let name = start.name().as_ref().to_string();
    let mut attributes = Vec::new();
    for attribute in start.attributes().flatten() {
        let key = attribute.key.as_ref().to_string();
        let value = attribute.value.as_ref().to_string();
        attributes.push((key, value));
    }
    Element {
        name,
        attributes,
        children: Vec::new(),
        text: String::new(),
    }
}

fn close_element(stack: &mut Vec<Element>, root: &mut Option<Element>, mut element: Element) {
    element.text = element.text.trim().to_string();
    if let Some(parent) = stack.last_mut() {
        parent.children.push(element);
    } else if root.is_none() {
        *root = Some(element);
    }
}
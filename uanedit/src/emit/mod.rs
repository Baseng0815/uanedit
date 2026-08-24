//! The XML this crate writes, as text.
//!
//! The emitter and the encoding of a value sit outside the `xml` feature: a value is shown as XML
//! in a browser that never links the file codec (features.md §2D), and it has to be shown the way
//! a save would write it, so both go through this one implementation.

mod value;

pub use value::value_xml;

use crate::types::xml::{
    XmlElement,
    XmlNode,
};

/// The XML namespace the built-in types are encoded in, which a `Value` declares on its payload
/// (OPC 10000-6 Annex F.1).
pub const TYPES_NAMESPACE: &str = "http://opcfoundation.org/UA/2008/02/Types.xsd";

/// A document being written, holding the line ending and indentation the file uses.
pub(crate) struct Writer {
    pub(crate) out: String,
    newline: String,
    indent: String,
    base: String,
}

impl Writer {
    pub(crate) fn new(
        newline: &str,
        indent: &str,
        base: &str,
    ) -> Self {
        Self {
            out: String::new(),
            newline: newline.to_owned(),
            indent: indent.to_owned(),
            base: base.to_owned(),
        }
    }

    pub(crate) fn finish(self) -> String {
        self.out
    }

    /// A line break, then this element's own indentation plus `depth` levels under it.
    pub(crate) fn line(
        &mut self,
        depth: usize,
    ) {
        self.out.push_str(&self.newline);
        self.out.push_str(&self.base);
        for _ in 0..depth {
            self.out.push_str(&self.indent);
        }
    }

    pub(crate) fn start(
        &mut self,
        name: &str,
    ) {
        self.out.push('<');
        self.out.push_str(name);
    }

    pub(crate) fn opened(&mut self) {
        self.out.push('>');
    }

    pub(crate) fn closed(&mut self) {
        self.out.push_str(" />");
    }

    pub(crate) fn end(
        &mut self,
        name: &str,
    ) {
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push('>');
    }

    pub(crate) fn attribute(
        &mut self,
        name: &str,
        value: &str,
    ) {
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        escape_attribute(&mut self.out, value);
        self.out.push('"');
    }

    pub(crate) fn text(
        &mut self,
        value: &str,
    ) {
        escape_text(&mut self.out, value);
    }

    /// Closes an element over its text content, self-closing when there is none. Every empty
    /// element this crate writes takes that one form.
    pub(crate) fn contents(
        &mut self,
        name: &str,
        text: &str,
    ) {
        if text.is_empty() {
            self.closed();
            return;
        }
        self.opened();
        self.text(text);
        self.end(name);
    }

    /// An element this crate kept rather than modelled, written back as it was read.
    pub(crate) fn element(
        &mut self,
        element: &XmlElement,
    ) {
        self.start(&element.name);
        for (name, value) in &element.attributes {
            self.attribute(name, value);
        }
        if element.children.is_empty() {
            self.closed();
            return;
        }
        self.opened();
        for child in &element.children {
            self.node(child);
        }
        self.end(&element.name);
    }

    fn node(
        &mut self,
        node: &XmlNode,
    ) {
        match node {
            XmlNode::Element(element) => self.element(element),
            XmlNode::Text(text) => self.text(text),
            XmlNode::CData(text) => {
                self.out.push_str("<![CDATA[");
                self.out.push_str(text);
                self.out.push_str("]]>");
            }
            XmlNode::Comment(text) => {
                self.out.push_str("<!--");
                self.out.push_str(text);
                self.out.push_str("-->");
            }
            XmlNode::ProcessingInstruction(text) => {
                self.out.push_str("<?");
                self.out.push_str(text);
                self.out.push_str("?>");
            }
        }
    }
}

fn escape_attribute(
    out: &mut String,
    value: &str,
) {
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#xD;"),
            '\n' => out.push_str("&#xA;"),
            '\t' => out.push_str("&#x9;"),
            other => out.push(other),
        }
    }
}

fn escape_text(
    out: &mut String,
    value: &str,
) {
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#xD;"),
            other => out.push(other),
        }
    }
}

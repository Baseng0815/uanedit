mod node;
mod tables;
mod value;

use core::str::FromStr;

use indexmap::IndexMap;
use quick_xml::events::Event;

use crate::error::{
    DocumentError,
    ParseError,
};
use crate::nodes::node::Node;
use crate::nodeset::aliases::AliasTable;
use crate::nodeset::models::ModelTableEntry;
use crate::nodeset::node_set::NodeSet;
use crate::report::{
    Diagnosis,
    Finding,
    OpenReport,
    Position,
    Preserved,
    PreservedKind,
};
use crate::types::date_time::DateTime;
use crate::types::node_id::NodeId;
use crate::types::node_id_ref::NodeIdRef;
use crate::types::xml::XmlElement;
use crate::xml::cursor::{
    Cursor,
    Tag,
};
use crate::xml::span::{
    Layout,
    NodeRegion,
    Span,
    TableRegion,
};

/// The root element of a NodeSet2 document, and the one the schema's other roots are not.
const ROOT: &str = "UANodeSet";
const CHANGES_ROOT: &str = "UANodeSetChanges";

/// A parse in progress: the event stream, plus the report it is filling in.
pub(crate) struct Reading<'a> {
    pub cursor: Cursor<'a>,
    pub report: OpenReport,
    owner: Option<NodeId>,
}

/// An element's attributes, taken one at a time so that whatever is left over is unknown.
pub(crate) struct Attributes {
    values: IndexMap<String, String>,
    offset: usize,
}

/// Reads a document into its model, its layout, and the report of what reading it turned up.
pub fn document(source: &str) -> Result<(NodeSet, Layout, OpenReport), DocumentError> {
    let mut reading = Reading::new(source);
    let root = reading.root()?;
    let mut layout = Layout {
        root_start: root.span,
        ..Layout::default()
    };
    layout.newline = detect_newline(source).to_owned();

    let mut nodeset = NodeSet::default();
    let mut attributes = reading.attributes(&root);
    nodeset.last_modified = reading.attribute(&mut attributes, "LastModified");

    if !root.empty {
        reading.body(&mut nodeset, &mut layout)?;
    }
    layout.tail_lead = Span::new(layout_end(&layout, root.span), reading.cursor.end().start);
    layout.root_end = reading.cursor.end();
    layout.indent = detect_indent(source, &layout);

    let mut report = reading.report;
    report.bytes = source.len();
    report.namespaces = nodeset.namespaces.uris().len();
    report.models = nodeset.models.len();
    report.aliases = nodeset.aliases.len();
    Ok((nodeset, layout, report))
}

impl<'a> Reading<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            cursor: Cursor::new(source),
            report: OpenReport::default(),
            owner: None,
        }
    }

    /// Finds the root element, refusing the other two roots the schema defines.
    fn root(&mut self) -> Result<Tag<'a>, DocumentError> {
        loop {
            let (event, span) = self.cursor.read()?;
            let tag = match event {
                Event::Start(start) => Tag {
                    start,
                    span,
                    empty: false,
                },
                Event::Empty(start) => Tag {
                    start,
                    span,
                    empty: true,
                },
                Event::Eof => return Err(DocumentError::NoRootElement),
                _ => continue,
            };
            return match tag.local_name() {
                ROOT => Ok(tag),
                CHANGES_ROOT => Err(DocumentError::NodeSetChanges),
                other => Err(DocumentError::UnexpectedRoot(other.to_owned())),
            };
        }
    }

    /// Reads the tables and the nodes, recording where each of them sat.
    fn body(
        &mut self,
        nodeset: &mut NodeSet,
        layout: &mut Layout,
    ) -> Result<(), DocumentError> {
        let mut previous = layout.root_start.end;
        while let Some(tag) = self.cursor.tag()? {
            let lead = Span::new(previous, tag.span.start);
            self.region(nodeset, layout, &tag, lead)?;
            previous = self.cursor.end().end.max(tag.span.end);
        }
        Ok(())
    }

    fn region(
        &mut self,
        nodeset: &mut NodeSet,
        layout: &mut Layout,
        tag: &Tag<'a>,
        lead: Span,
    ) -> Result<(), DocumentError> {
        let name = tag.local_name().to_owned();
        let slot = match name.as_str() {
            "NamespaceUris" => {
                nodeset.namespaces = self.uri_table(tag)?.into();
                Some(&mut layout.namespace_uris)
            }
            "ServerUris" => {
                nodeset.server_uris = self.uri_table(tag)?;
                Some(&mut layout.server_uris)
            }
            "Models" => {
                nodeset.models = self.models(tag)?;
                Some(&mut layout.models)
            }
            "Aliases" => {
                nodeset.aliases = self.aliases(tag)?;
                Some(&mut layout.aliases)
            }
            "Extensions" => {
                nodeset.extensions = self.extensions(tag)?;
                Some(&mut layout.extensions)
            }
            _ => None,
        };
        if let Some(slot) = slot {
            if slot.is_some() {
                self.find(tag.span.start, Diagnosis::DuplicateTable(name.clone()));
            }
            if !layout.nodes.is_empty() {
                self.find(tag.span.start, Diagnosis::TableOutOfOrder(name));
            }
            *slot = Some(TableRegion {
                lead,
                content: Span::new(tag.span.start, self.content_end(tag)),
            });
            return Ok(());
        }
        self.node(nodeset, layout, tag, lead)
    }

    /// Where the element that was just read ended, self-closing or not.
    fn content_end(
        &self,
        tag: &Tag<'a>,
    ) -> usize {
        self.cursor.end().end.max(tag.span.end)
    }

    fn node(
        &mut self,
        nodeset: &mut NodeSet,
        layout: &mut Layout,
        tag: &Tag<'a>,
        lead: Span,
    ) -> Result<(), DocumentError> {
        let Some(node) = self.read_node(tag)? else {
            self.preserve(PreservedKind::UnknownElement, tag.local_name(), tag.span.start);
            self.find(tag.span.start, Diagnosis::UnknownNodeClass(tag.local_name().to_owned()));
            return Ok(());
        };
        let region = NodeRegion {
            lead,
            tag: tag.span,
            body: Span::new(tag.span.end, self.cursor.end().start.max(tag.span.end)),
            end: if tag.empty { Span::default() } else { self.cursor.end() },
        };
        let node_id = node.node_id().clone();
        if layout.nodes.insert(node_id.clone(), region).is_some() {
            self.find(tag.span.start, Diagnosis::DuplicateNodeId(node_id.clone()));
        }
        self.report.count(node.node_class());
        nodeset.nodes.insert(node_id, node);
        Ok(())
    }

    fn attributes(
        &self,
        tag: &Tag<'a>,
    ) -> Attributes {
        Attributes {
            values: tag.attributes(),
            offset: tag.span.start,
        }
    }

    /// Reads an attribute into a value. The padding the schema's token types allow is dropped, as
    /// it is for the numbers and the element text, so a hand-written file parses.
    fn attribute<T: FromStr<Err = ParseError>>(
        &mut self,
        attributes: &mut Attributes,
        name: &str,
    ) -> Option<T> {
        let value = attributes.take(name)?;
        match value.trim().parse() {
            Ok(parsed) => Some(parsed),
            Err(source) => {
                self.find(
                    attributes.offset,
                    Diagnosis::MalformedAttribute {
                        name: name.to_owned(),
                        value,
                        source,
                    },
                );
                None
            }
        }
    }

    fn required<T: FromStr<Err = ParseError>>(
        &mut self,
        attributes: &mut Attributes,
        name: &str,
    ) -> Option<T> {
        if !attributes.has(name) {
            self.find(attributes.offset, Diagnosis::MissingAttribute { name: name.to_owned() });
            return None;
        }
        self.attribute(attributes, name)
    }

    fn reference<T: From<NodeIdRef>>(
        &mut self,
        attributes: &mut Attributes,
        name: &str,
    ) -> Option<T> {
        attributes
            .take(name)
            .map(|value| T::from(node_id_ref(&value)))
    }

    fn boolean(
        &mut self,
        attributes: &mut Attributes,
        name: &str,
    ) -> Option<bool> {
        let value = attributes.take(name)?;
        match boolean(&value) {
            Some(parsed) => Some(parsed),
            None => {
                self.find(
                    attributes.offset,
                    Diagnosis::MalformedAttribute {
                        name: name.to_owned(),
                        value: value.clone(),
                        source: ParseError::Boolean(value),
                    },
                );
                None
            }
        }
    }

    fn number<T: FromStr>(
        &mut self,
        attributes: &mut Attributes,
        name: &str,
    ) -> Option<T> {
        let value = attributes.take(name)?;
        match value.trim().parse() {
            Ok(parsed) => Some(parsed),
            Err(_) => {
                self.find(
                    attributes.offset,
                    Diagnosis::MalformedAttribute {
                        name: name.to_owned(),
                        value: value.clone(),
                        source: ParseError::Integer(value),
                    },
                );
                None
            }
        }
    }

    fn double(
        &mut self,
        attributes: &mut Attributes,
        name: &str,
    ) -> Option<f64> {
        let value = attributes.take(name)?;
        match double(&value) {
            Some(parsed) => Some(parsed),
            None => {
                self.find(
                    attributes.offset,
                    Diagnosis::MalformedAttribute {
                        name: name.to_owned(),
                        value: value.clone(),
                        source: ParseError::Double(value),
                    },
                );
                None
            }
        }
    }

    /// Reads an element's text into a value, reporting a spelling the grammar does not admit.
    fn parsed_text<T: FromStr<Err = ParseError>>(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Option<T>, DocumentError> {
        let offset = tag.span.start;
        let element = tag.local_name().to_owned();
        let text = self.cursor.text(tag)?;
        Ok(match text.trim().parse() {
            Ok(parsed) => Some(parsed),
            Err(source) => {
                self.find(
                    offset,
                    Diagnosis::MalformedElement {
                        element,
                        value: text,
                        source,
                    },
                );
                None
            }
        })
    }

    fn keep_unknown(
        &mut self,
        attributes: Attributes,
    ) -> IndexMap<String, String> {
        for name in attributes.values.keys() {
            self.report.preserved.push(Preserved {
                kind: PreservedKind::UnknownAttribute,
                name: name.clone(),
                owner: self.owner.clone(),
                position: self.position(attributes.offset),
            });
        }
        attributes.values
    }

    fn preserve(
        &mut self,
        kind: PreservedKind,
        name: &str,
        offset: usize,
    ) {
        self.report.preserved.push(Preserved {
            kind,
            name: name.to_owned(),
            owner: self.owner.clone(),
            position: self.position(offset),
        });
    }

    fn find(
        &mut self,
        offset: usize,
        diagnosis: Diagnosis,
    ) {
        self.report.findings.push(Finding {
            position: self.position(offset),
            owner: self.owner.clone(),
            diagnosis,
        });
    }

    fn position(
        &self,
        offset: usize,
    ) -> Position {
        Position::of(self.cursor.source(), offset)
    }
}

impl Attributes {
    fn take(
        &mut self,
        name: &str,
    ) -> Option<String> {
        self.values.shift_remove(name)
    }

    fn has(
        &self,
        name: &str,
    ) -> bool {
        self.values.contains_key(name)
    }
}

/// A NodeId attribute is either spelled out or naming an alias; only the first form parses.
pub(crate) fn node_id_ref(value: &str) -> NodeIdRef {
    match value.parse() {
        Ok(node_id) => NodeIdRef::Id(node_id),
        Err(_) => NodeIdRef::Alias(value.to_owned()),
    }
}

/// `xs:boolean` admits the two words and the two digits.
pub(crate) fn boolean(value: &str) -> Option<bool> {
    match value.trim() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

/// `xs:double` spells the three special values in capitals rather than as Rust does.
pub(crate) fn double(value: &str) -> Option<f64> {
    match value.trim() {
        "INF" => Some(f64::INFINITY),
        "-INF" => Some(f64::NEG_INFINITY),
        "NaN" => Some(f64::NAN),
        number => number.parse().ok(),
    }
}

fn detect_newline(source: &str) -> &str {
    match source.find('\n') {
        Some(index) if source[..index].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

/// One level of indentation, read off whatever the document puts its own first child at.
fn detect_indent(
    source: &str,
    layout: &Layout,
) -> String {
    let lead = layout
        .tables()
        .next()
        .map(|table| table.lead)
        .or_else(|| layout.nodes.values().next().map(|node| node.lead))
        .unwrap_or_default();
    let text = lead.of(source);
    let start = text.rfind('\n').map_or(0, |index| index + 1);
    match &text[start..] {
        "" => "  ".to_owned(),
        indent => indent.to_owned(),
    }
}

/// Where the last region ended, which is where the whitespace before the end tag begins.
fn layout_end(
    layout: &Layout,
    root_start: Span,
) -> usize {
    let tables = layout.tables().map(|table| table.content.end);
    let nodes = layout.nodes.values().map(|node| node.span().end);
    tables.chain(nodes).max().unwrap_or(root_start.end)
}

/// Re-reads one region of a document on its own, so a save can tell what the model changed.
fn fragment<T>(
    source: &str,
    read: impl for<'a> FnOnce(&mut Reading<'a>, &Tag<'a>) -> Result<T, DocumentError>,
) -> Option<T> {
    let mut reading = Reading::new(source);
    let tag = reading.cursor.tag().ok()??;
    read(&mut reading, &tag).ok()
}

pub(crate) fn node_fragment(source: &str) -> Option<Node> {
    fragment(source, |reading, tag| reading.read_node(tag))?
}

pub(crate) fn uri_fragment(source: &str) -> Option<Vec<String>> {
    fragment(source, |reading, tag| reading.uri_table(tag))
}

pub(crate) fn model_fragment(source: &str) -> Option<Vec<ModelTableEntry>> {
    fragment(source, |reading, tag| reading.models(tag))
}

pub(crate) fn alias_fragment(source: &str) -> Option<AliasTable> {
    fragment(source, |reading, tag| reading.aliases(tag))
}

pub(crate) fn extension_fragment(source: &str) -> Option<Vec<XmlElement>> {
    fragment(source, |reading, tag| reading.extensions(tag))
}

/// A start tag's attributes as the document wrote them, in its order and with its spellings.
pub(crate) fn tag_attributes(source: &str) -> IndexMap<String, String> {
    let mut reading = Reading::new(source);
    match reading.cursor.tag() {
        Ok(Some(tag)) => tag.attributes(),
        _ => IndexMap::new(),
    }
}

/// The `LastModified` attribute of a root start tag read on its own.
pub(crate) fn root_last_modified(tag: &str) -> Option<DateTime> {
    let mut reading = Reading::new(tag);
    let tag = reading.cursor.tag().ok()??;
    let mut attributes = reading.attributes(&tag);
    reading.attribute(&mut attributes, "LastModified")
}

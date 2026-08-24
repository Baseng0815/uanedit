use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::error::DocumentError;
use crate::nodes::node::Node;
use crate::nodeset::node_set::NodeSet;
use crate::report::OpenReport;
use crate::xml::span::{
    Layout,
    NodeRegion,
    Span,
    TableRegion,
};
use crate::xml::write::RenderedNode;
use crate::xml::{
    read,
    write,
};

/// A nodeset together with the document it came from.
///
/// The layout says where every part of that document sat, so writing splices the original bytes
/// back for everything the model still agrees with and re-serialises only what changed. A file
/// that was not edited therefore comes back out byte for byte.
///
/// Nothing has to tell the document that an edit happened: writing re-reads each region of the
/// source and compares it with the model, so an edit made through [`Document::nodeset_mut`] or
/// applied to a model that travelled over a wire and came back through
/// [`Document::write_nodeset`] is found either way. Editing code has no bookkeeping to do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    source: String,
    layout: Layout,
    nodeset: NodeSet,
    report: OpenReport,
}

impl Document {
    pub fn parse(source: &str) -> Result<Self, DocumentError> {
        let (nodeset, layout, report) = read::document(source)?;
        Ok(Self {
            source: source.to_owned(),
            layout,
            nodeset,
            report,
        })
    }

    /// Reads a document from the bytes of a file, which the schema requires to be UTF-8.
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, DocumentError> {
        let source = core::str::from_utf8(bytes).map_err(|_| DocumentError::NotUtf8)?;
        Self::parse(source)
    }

    pub fn nodeset(&self) -> &NodeSet {
        &self.nodeset
    }

    /// The model, to edit. No edit invalidates the layout, and none has to be announced.
    pub fn nodeset_mut(&mut self) -> &mut NodeSet {
        &mut self.nodeset
    }

    pub fn into_nodeset(self) -> NodeSet {
        self.nodeset
    }

    pub fn report(&self) -> &OpenReport {
        &self.report
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn write(&self) -> String {
        self.write_nodeset(&self.nodeset)
    }

    /// One node as this crate writes it, at the indentation its source region sits at — the
    /// fragments a save splices in when that node no longer matches the file.
    pub fn render_node(
        &self,
        node: &Node,
    ) -> RenderedNode {
        let base = match self.layout.nodes.get(node.node_id()) {
            Some(region) => region.indentation(&self.source),
            None => &self.layout.indent,
        };
        write::render_node(node, &self.layout, base)
    }

    /// Writes a model that was edited elsewhere against this document's bytes.
    pub fn write_nodeset(
        &self,
        nodeset: &NodeSet,
    ) -> String {
        let mut out = String::with_capacity(self.source.len() + self.source.len() / 8);
        out.push_str(self.text(Span::new(0, self.layout.root_start.start)));
        self.root(&mut out, nodeset);
        self.tables(&mut out, nodeset);
        self.nodes(&mut out, nodeset);
        out.push_str(self.text(self.layout.tail_lead));
        out.push_str(self.text(self.layout.root_end));
        let tail = self.layout.root_end.end.max(self.layout.root_start.end);
        out.push_str(self.text(Span::new(tail, self.source.len())));
        out
    }

    fn text(
        &self,
        span: Span,
    ) -> &str {
        span.of(&self.source)
    }

    fn root(
        &self,
        out: &mut String,
        nodeset: &NodeSet,
    ) {
        let source = self.text(self.layout.root_start);
        match read::root_last_modified(source) == nodeset.last_modified {
            true => out.push_str(source),
            false => out.push_str(&write::root_start(nodeset)),
        }
    }

    fn tables(
        &self,
        out: &mut String,
        nodeset: &NodeSet,
    ) {
        let uris = nodeset.namespaces.uris().to_vec();
        self.table(
            out,
            self.layout.namespace_uris,
            |source| read::uri_fragment(source).is_some_and(|read| read == uris),
            write::namespace_uris(nodeset, &self.layout, &self.layout.indent),
        );
        self.table(
            out,
            self.layout.server_uris,
            |source| read::uri_fragment(source).is_some_and(|read| read == nodeset.server_uris),
            write::server_uris(nodeset, &self.layout, &self.layout.indent),
        );
        self.table(
            out,
            self.layout.models,
            |source| read::model_fragment(source).is_some_and(|read| read == nodeset.models),
            write::models(nodeset, &self.layout, &self.layout.indent),
        );
        self.table(
            out,
            self.layout.aliases,
            |source| read::alias_fragment(source).is_some_and(|read| read == nodeset.aliases),
            write::aliases(nodeset, &self.layout, &self.layout.indent),
        );
        self.table(
            out,
            self.layout.extensions,
            |source| read::extension_fragment(source).is_some_and(|read| read == nodeset.extensions),
            write::extensions(nodeset, &self.layout, &self.layout.indent),
        );
    }

    fn table(
        &self,
        out: &mut String,
        region: Option<TableRegion>,
        unchanged: impl FnOnce(&str) -> bool,
        rendered: Option<String>,
    ) {
        let Some(region) = region else {
            if let Some(rendered) = rendered {
                out.push_str(&self.layout.newline);
                out.push_str(&self.layout.indent);
                out.push_str(&rendered);
            }
            return;
        };
        if unchanged(self.text(region.content)) {
            out.push_str(self.text(region.lead));
            out.push_str(self.text(region.content));
            return;
        }
        let Some(rendered) = rendered else {
            out.push_str(kept_lead(self.text(region.lead)));
            return;
        };
        out.push_str(self.text(region.lead));
        out.push_str(&rendered);
    }

    fn nodes(
        &self,
        out: &mut String,
        nodeset: &NodeSet,
    ) {
        let mut passed = 0;
        for (node_id, node) in &nodeset.nodes {
            match self.layout.nodes.get_full(node_id) {
                Some((index, _, region)) => {
                    self.dropped(out, nodeset, passed..index);
                    passed = passed.max(index + 1);
                    self.node(out, *region, node);
                }
                None => self.new_node(out, node),
            }
        }
        self.dropped(out, nodeset, passed..self.layout.nodes.len());
    }

    /// The comments that lived in front of nodes the model no longer has, which a deletion has no
    /// reason to take with it.
    fn dropped(
        &self,
        out: &mut String,
        nodeset: &NodeSet,
        range: core::ops::Range<usize>,
    ) {
        for index in range {
            let Some((node_id, region)) = self.layout.nodes.get_index(index) else {
                continue;
            };
            if !nodeset.nodes.contains_key(node_id) {
                out.push_str(kept_lead(self.text(region.lead)));
            }
        }
    }

    fn node(
        &self,
        out: &mut String,
        region: NodeRegion,
        node: &Node,
    ) {
        out.push_str(self.text(region.lead));
        let source = self.text(region.span());
        let Some(parsed) = read::node_fragment(source) else {
            out.push_str(source);
            return;
        };
        if parsed == *node {
            out.push_str(source);
            return;
        }
        let base = region.indentation(&self.source);
        let before = write::render_node(&parsed, &self.layout, base);
        let after = write::render_node(node, &self.layout, base);
        match before.tag == after.tag {
            true => out.push_str(self.text(region.tag)),
            false => out.push_str(&self.retag(region, &parsed, node)),
        }
        match before.body == after.body {
            true => out.push_str(self.text(region.body)),
            false => out.push_str(&after.body),
        }
        match before.end == after.end {
            true => out.push_str(self.text(region.end)),
            false => out.push_str(&after.end),
        }
    }

    /// A start tag rewritten in place: every attribute the edit did not touch keeps the order and
    /// the spelling the document gave it, so one edit rewrites one attribute.
    fn retag(
        &self,
        region: NodeRegion,
        parsed: &Node,
        node: &Node,
    ) -> String {
        let written = read::tag_attributes(self.text(region.tag));
        let before = write::node_attributes(parsed);
        let now = write::node_attributes(node);
        let mut merged = IndexMap::new();
        for (name, spelling) in &written {
            let kept = match (before.get(name), now.get(name)) {
                (was, Some(value)) if was == Some(value) => spelling,
                (None, None) => spelling,
                (_, Some(value)) => value,
                (_, None) => continue,
            };
            merged.insert(name.clone(), kept.clone());
        }
        for (name, value) in &now {
            if !merged.contains_key(name) {
                merged.insert(name.clone(), value.clone());
            }
        }
        let base = region.indentation(&self.source);
        write::render_node_as(node, &merged, &self.layout, base).tag
    }

    fn new_node(
        &self,
        out: &mut String,
        node: &Node,
    ) {
        let rendered = write::render_node(node, &self.layout, &self.layout.indent);
        out.push_str(&self.layout.newline);
        out.push_str(&self.layout.indent);
        out.push_str(&rendered.tag);
        out.push_str(&rendered.body);
        out.push_str(&rendered.end);
    }
}

/// What is worth keeping from the whitespace in front of a region that is being dropped: the
/// comments, without the indentation that would have run into the element itself.
fn kept_lead(lead: &str) -> &str {
    let end = lead.trim_end();
    match end.is_empty() {
        true => "",
        false => &lead[..end.len()],
    }
}

use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::types::node_id::NodeId;

/// A byte range of the document a nodeset was read from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(
        start: usize,
        end: usize,
    ) -> Self {
        Self { start, end }
    }

    pub fn is_empty(self) -> bool {
        self.end <= self.start
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn of(
        self,
        source: &str,
    ) -> &str {
        source.get(self.start..self.end).unwrap_or_default()
    }
}

/// Where every part of the source document sits, so a save can splice back what was not edited.
///
/// The spans tile the document: the prologue runs to `root_start`, each region's `lead` begins
/// where the region before it ended, and the epilogue runs from the end of `root_end`. Writing an
/// unedited nodeset is therefore a concatenation of the original bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub root_start: Span,
    pub namespace_uris: Option<TableRegion>,
    pub server_uris: Option<TableRegion>,
    pub models: Option<TableRegion>,
    pub aliases: Option<TableRegion>,
    pub extensions: Option<TableRegion>,
    pub nodes: IndexMap<NodeId, NodeRegion>,
    pub tail_lead: Span,
    pub root_end: Span,
    /// The line ending the document uses, replayed when new content is written.
    pub newline: String,
    /// One level of the document's own indentation, replayed when new content is written.
    pub indent: String,
}

impl Layout {
    /// The tables that the document actually held, in the order the schema puts them.
    pub fn tables(&self) -> impl Iterator<Item = &TableRegion> {
        [
            &self.namespace_uris,
            &self.server_uris,
            &self.models,
            &self.aliases,
            &self.extensions,
        ]
        .into_iter()
        .flatten()
    }
}

/// One of the five tables the schema puts before the nodes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRegion {
    pub lead: Span,
    pub content: Span,
}

/// One node element, split so that an attribute edit does not reflow the children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRegion {
    pub lead: Span,
    pub tag: Span,
    pub body: Span,
    /// Empty when the element was written self-closing.
    pub end: Span,
}

impl NodeRegion {
    pub fn span(self) -> Span {
        Span::new(self.tag.start, self.end.end.max(self.body.end).max(self.tag.end))
    }

    /// The indentation the element was written at, which its children are written one level under.
    pub fn indentation(
        self,
        source: &str,
    ) -> &str {
        let lead = self.lead.of(source);
        let start = lead.rfind('\n').map_or(0, |index| index + 1);
        &lead[start..]
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            root_start: Span::default(),
            namespace_uris: None,
            server_uris: None,
            models: None,
            aliases: None,
            extensions: None,
            nodes: IndexMap::new(),
            tail_lead: Span::default(),
            root_end: Span::default(),
            newline: "\n".to_owned(),
            indent: "  ".to_owned(),
        }
    }
}

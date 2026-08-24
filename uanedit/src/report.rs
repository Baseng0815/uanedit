//! What reading a file produced besides the nodeset.
//!
//! Plain data, and outside the `xml` feature on purpose: the codec fills it in, but a dependent
//! that only holds the model — the browser half of an editor — still has to display it.

use serde::{
    Deserialize,
    Serialize,
};
use thiserror::Error;

use crate::error::ParseError;
use crate::nodes::node_class::NodeClass;
use crate::types::node_id::NodeId;

/// What reading a file produced besides the nodeset: what was loaded, what was kept without being
/// understood, and what was irregular but not fatal (features.md §2E).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenReport {
    pub bytes: usize,
    pub nodes: usize,
    /// The node counts per class, in the order the classes were first seen.
    pub nodes_by_class: Vec<(NodeClass, usize)>,
    pub namespaces: usize,
    pub models: usize,
    pub aliases: usize,
    /// Content this crate does not model and wrote back out untouched.
    pub preserved: Vec<Preserved>,
    /// Departures from the schema that still left a readable nodeset.
    pub findings: Vec<Finding>,
}

/// One piece of the document that survived without being understood.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preserved {
    pub kind: PreservedKind,
    /// The element or attribute name, as written.
    pub name: String,
    pub owner: Option<NodeId>,
    pub position: Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreservedKind {
    /// An `Extension` element, whose payload belongs to whoever wrote it.
    Extension,
    /// A child element the schema this crate knows does not define.
    UnknownElement,
    /// An attribute the schema this crate knows does not define, so likely a newer revision.
    UnknownAttribute,
    /// A `Value` this crate keeps as XML rather than decoding.
    OpaqueValue,
}

/// One irregularity, with where in the document it was found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub position: Position,
    pub owner: Option<NodeId>,
    pub diagnosis: Diagnosis,
}

#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum Diagnosis {
    #[error("`{name}` is required here and is missing")]
    MissingAttribute { name: String },
    #[error("`{name}=\"{value}\"` could not be read: {source}")]
    MalformedAttribute {
        name: String,
        value: String,
        source: ParseError,
    },
    #[error("`{element}` holds `{value}`, which could not be read: {source}")]
    MalformedElement {
        element: String,
        value: String,
        source: ParseError,
    },
    #[error("NodeId `{0}` is defined more than once; only the last definition was kept")]
    DuplicateNodeId(NodeId),
    #[error("alias `{0}` is defined more than once; only the last definition was kept")]
    DuplicateAlias(String),
    #[error("`{0}` appears after the nodes, so saving moves it back in front of them")]
    TableOutOfOrder(String),
    #[error("`{0}` appears more than once; the later one replaced the earlier")]
    DuplicateTable(String),
    #[error("`{0}` is not a node class this schema defines, so the element was kept verbatim")]
    UnknownNodeClass(String),
}

/// A byte offset with the line and column it falls on, both counted from one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn of(
        source: &str,
        offset: usize,
    ) -> Self {
        let head = source.get(..offset).unwrap_or(source);
        let line_start = head.rfind('\n').map_or(0, |index| index + 1);
        Self {
            offset,
            line: head.bytes().filter(|byte| *byte == b'\n').count() + 1,
            column: head[line_start..].chars().count() + 1,
        }
    }
}

impl OpenReport {
    #[cfg(feature = "xml")]
    pub(crate) fn count(
        &mut self,
        node_class: NodeClass,
    ) {
        self.nodes += 1;
        match self
            .nodes_by_class
            .iter_mut()
            .find(|(class, _)| *class == node_class)
        {
            Some((_, count)) => *count += 1,
            None => self.nodes_by_class.push((node_class, 1)),
        }
    }

    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

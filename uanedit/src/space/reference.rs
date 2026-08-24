use serde::{
    Deserialize,
    Serialize,
};

use crate::types::node_id::NodeId;

/// One reference as seen from a node, whichever end of it the file happens to store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceView {
    pub reference_type: NodeId,
    /// True when the node this view was asked for is the source of the reference.
    pub is_forward: bool,
    /// The node at the other end.
    pub other: NodeId,
    pub storage: ReferenceStorage,
}

/// Which node's `References` element a reference is written in.
///
/// One reference may be written on either end and OPC 10000-6 §F.4 only requires one of them, but
/// a file may state both; that is still one reference (OPC 10000-3 §4.4.4), seen from one end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceStorage {
    /// The queried node holds it, at this position in its reference list.
    Own(usize),
    /// The node at the other end holds it; this view is synthesized.
    Synthesized,
    /// Both ends state it; this is the queried node's own position.
    Both(usize),
}

impl ReferenceStorage {
    pub fn is_synthesized(self) -> bool {
        self == Self::Synthesized
    }

    /// True when the node at the other end states the reference in its own `References` element.
    pub fn is_stated_by_other(self) -> bool {
        !matches!(self, Self::Own(_))
    }

    pub fn position(self) -> Option<usize> {
        match self {
            Self::Own(position) | Self::Both(position) => Some(position),
            Self::Synthesized => None,
        }
    }
}

impl ReferenceView {
    /// The two ends of the reference, given the node this view was asked for.
    pub fn ends<'a>(
        &'a self,
        queried: &'a NodeId,
    ) -> (&'a NodeId, &'a NodeId) {
        match self.is_forward {
            true => (queried, &self.other),
            false => (&self.other, queried),
        }
    }
}

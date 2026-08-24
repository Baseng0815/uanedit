use indexmap::IndexSet;

use crate::types::node_id::NodeId;

/// A dense handle for a NodeId, so the graph indexes are arrays rather than maps.
///
/// Handles are valid only for the address space that issued them and are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeKey(u32);

impl NodeKey {
    pub fn from_index(index: usize) -> Self {
        Self(truncate(index))
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Assigns a [`NodeKey`] to every NodeId the space mentions, defined or merely referenced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keys(IndexSet<NodeId>);

impl Keys {
    pub fn intern(
        &mut self,
        node_id: &NodeId,
    ) -> NodeKey {
        if let Some(index) = self.0.get_index_of(node_id) {
            return NodeKey(truncate(index));
        }
        let (index, _) = self.0.insert_full(node_id.clone());
        NodeKey(truncate(index))
    }

    pub fn get(
        &self,
        node_id: &NodeId,
    ) -> Option<NodeKey> {
        self.0
            .get_index_of(node_id)
            .map(|index| NodeKey(truncate(index)))
    }

    pub fn node_id(
        &self,
        key: NodeKey,
    ) -> &NodeId {
        self.0
            .get_index(key.index())
            .expect("a NodeKey is only issued for a NodeId this table interned")
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// A nodeset with more than four billion distinct NodeIds is not a file anyone edits.
fn truncate(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

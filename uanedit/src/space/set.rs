use serde::{
    Deserialize,
    Serialize,
};

use crate::nodeset::NodeSet;
use crate::nodeset::namespaces::NamespaceTable;
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use crate::types::qualified_name::QualifiedName;

/// Which loaded nodeset a node came from; index 0 is always the primary, editable one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SetId(pub usize);

impl SetId {
    /// The one nodeset an address space lets a caller edit.
    pub const PRIMARY: Self = Self(0);

    pub fn is_primary(self) -> bool {
        self == Self::PRIMARY
    }
}

/// A nodeset in an address space, with the mapping from its own namespace indexes to the space's.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedSet {
    node_set: NodeSet,
    to_space: Vec<NamespaceIndex>,
}

impl LoadedSet {
    /// Registers the set's namespace URIs in `space` and records where each of its indexes landed.
    pub fn new(
        node_set: NodeSet,
        space: &mut NamespaceTable,
    ) -> Self {
        let mut set = Self {
            node_set,
            to_space: Vec::new(),
        };
        set.remap(space);
        set
    }

    /// Registers the set's namespaces again, after its own table or the space's table changed.
    pub fn remap(
        &mut self,
        space: &mut NamespaceTable,
    ) {
        self.to_space = vec![0];
        for uri in self.node_set.namespaces.uris() {
            self.to_space.push(space.insert(uri.clone()).unwrap_or(0));
        }
    }

    pub fn node_set(&self) -> &NodeSet {
        &self.node_set
    }

    pub fn node_set_mut(&mut self) -> &mut NodeSet {
        &mut self.node_set
    }

    pub fn into_node_set(self) -> NodeSet {
        self.node_set
    }

    /// The space-wide index for one of this set's own namespace indexes.
    pub fn to_space_index(
        &self,
        local: NamespaceIndex,
    ) -> Option<NamespaceIndex> {
        self.to_space.get(usize::from(local)).copied()
    }

    /// The set's own index for a space-wide one, absent when the set does not declare the namespace.
    pub fn to_local_index(
        &self,
        space: NamespaceIndex,
    ) -> Option<NamespaceIndex> {
        let position = self.to_space.iter().position(|entry| *entry == space)?;
        NamespaceIndex::try_from(position).ok()
    }

    /// A NodeId this set spelled, restated in the space's namespace indexes.
    pub fn to_space_node_id(
        &self,
        local: &NodeId,
    ) -> Option<NodeId> {
        Some(NodeId {
            namespace_index: self.to_space_index(local.namespace_index)?,
            identifier: local.identifier.clone(),
        })
    }

    pub fn to_local_node_id(
        &self,
        space: &NodeId,
    ) -> Option<NodeId> {
        Some(NodeId {
            namespace_index: self.to_local_index(space.namespace_index)?,
            identifier: space.identifier.clone(),
        })
    }

    pub fn to_space_browse_name(
        &self,
        local: &QualifiedName,
    ) -> Option<QualifiedName> {
        Some(QualifiedName {
            namespace_index: self.to_space_index(local.namespace_index)?,
            name: local.name.clone(),
            explicit_index: local.explicit_index,
        })
    }

    /// Whether the set's own namespace table has an entry for this index.
    pub fn declares_index(
        &self,
        index: NamespaceIndex,
    ) -> bool {
        usize::from(index) < self.to_space.len()
    }
}

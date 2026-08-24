use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::nodeset::NodeSet;
use crate::nodeset::namespaces::NamespaceTable;
use crate::space::delta::Delta;
use crate::space::index::SpaceIndex;
use crate::space::keys::NodeKey;
use crate::space::set::{
    LoadedSet,
    SetId,
};
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use crate::types::qualified_name::QualifiedName;

/// One editable nodeset and the dependency nodesets it is written against, seen as one graph.
///
/// Every NodeId and QualifiedName this type takes and returns is stated in the space's own
/// namespace table, which starts with the primary nodeset's table — so for the primary set, and
/// only for it, a space NodeId is the NodeId the file spells.
#[derive(Clone, Debug)]
pub struct AddressSpace {
    namespaces: NamespaceTable,
    sets: Vec<LoadedSet>,
    index: SpaceIndex,
}

impl AddressSpace {
    pub fn new(primary: NodeSet) -> Self {
        let mut space = Self {
            namespaces: NamespaceTable::default(),
            sets: Vec::new(),
            index: SpaceIndex::default(),
        };
        space.push_set(primary);
        space.rebuild();
        space
    }

    /// Loads the primary nodeset together with the dependencies its RequiredModels resolved to.
    pub fn load(
        primary: NodeSet,
        dependencies: impl IntoIterator<Item = NodeSet>,
    ) -> Self {
        let mut space = Self {
            namespaces: NamespaceTable::default(),
            sets: Vec::new(),
            index: SpaceIndex::default(),
        };
        space.push_set(primary);
        for dependency in dependencies {
            space.push_set(dependency);
        }
        space.rebuild();
        space
    }

    /// Adds a read-only nodeset and reindexes; the returned id names it from then on.
    pub fn add_dependency(
        &mut self,
        dependency: NodeSet,
    ) -> SetId {
        let set = self.push_set(dependency);
        self.rebuild();
        set
    }

    fn push_set(
        &mut self,
        node_set: NodeSet,
    ) -> SetId {
        self.sets
            .push(LoadedSet::new(node_set, &mut self.namespaces));
        SetId(self.sets.len() - 1)
    }

    /// The namespace table every index in this type's arguments and results is against.
    pub fn namespaces(&self) -> &NamespaceTable {
        &self.namespaces
    }

    pub fn namespace_uri(
        &self,
        index: NamespaceIndex,
    ) -> Option<&str> {
        self.namespaces.uri(index)
    }

    pub fn namespace_index(
        &self,
        uri: &str,
    ) -> Option<NamespaceIndex> {
        self.namespaces.index_of(uri)
    }

    pub fn primary(&self) -> &NodeSet {
        self.set(SetId::PRIMARY)
            .expect("an address space always holds a primary nodeset")
    }

    /// The primary nodeset for direct mutation; the caller owes the space a [`Self::reindex`].
    pub fn primary_mut(&mut self) -> &mut NodeSet {
        self.sets[SetId::PRIMARY.0].node_set_mut()
    }

    pub fn set(
        &self,
        set: SetId,
    ) -> Option<&NodeSet> {
        self.sets.get(set.0).map(LoadedSet::node_set)
    }

    pub fn set_count(&self) -> usize {
        self.sets.len()
    }

    pub fn sets(&self) -> impl Iterator<Item = (SetId, &NodeSet)> {
        self.sets
            .iter()
            .enumerate()
            .map(|(position, set)| (SetId(position), set.node_set()))
    }

    /// Restates a NodeId one of the loaded files spells in the space's namespace indexes.
    pub fn to_space_node_id(
        &self,
        set: SetId,
        local: &NodeId,
    ) -> Option<NodeId> {
        self.sets.get(set.0)?.to_space_node_id(local)
    }

    /// Restates a space NodeId in the indexes of one loaded file, for writing it back out.
    pub fn to_local_node_id(
        &self,
        set: SetId,
        space: &NodeId,
    ) -> Option<NodeId> {
        self.sets.get(set.0)?.to_local_node_id(space)
    }

    pub fn node(
        &self,
        node_id: &NodeId,
    ) -> Option<&Node> {
        let key = self.index.key(node_id)?;
        self.node_at(key)
    }

    pub(crate) fn node_at(
        &self,
        key: NodeKey,
    ) -> Option<&Node> {
        let definition = self.index.definition(key)?;
        self.sets
            .get(definition.set.0)?
            .node_set()
            .node(&definition.local)
    }

    pub fn contains(
        &self,
        node_id: &NodeId,
    ) -> bool {
        self.node(node_id).is_some()
    }

    /// Which loaded nodeset defines the node, absent when nothing does.
    pub fn set_of(
        &self,
        node_id: &NodeId,
    ) -> Option<SetId> {
        let key = self.index.key(node_id)?;
        Some(self.index.definition(key)?.set)
    }

    /// True unless the node lives in the primary nodeset and outside namespace 0.
    pub fn is_read_only(
        &self,
        node_id: &NodeId,
    ) -> bool {
        node_id.is_in_base_namespace() || self.set_of(node_id) != Some(SetId::PRIMARY)
    }

    pub fn node_class(
        &self,
        node_id: &NodeId,
    ) -> Option<NodeClass> {
        Some(self.node(node_id)?.node_class())
    }

    /// The node's BrowseName restated in the space's namespace indexes.
    pub fn browse_name(
        &self,
        node_id: &NodeId,
    ) -> Option<QualifiedName> {
        let key = self.index.key(node_id)?;
        self.browse_name_at(key)
    }

    pub(crate) fn browse_name_at(
        &self,
        key: NodeKey,
    ) -> Option<QualifiedName> {
        let definition = self.index.definition(key)?;
        let set = self.sets.get(definition.set.0)?;
        let node = set.node_set().node(&definition.local)?;
        set.to_space_browse_name(node.browse_name())
    }

    pub fn is_abstract(
        &self,
        node_id: &NodeId,
    ) -> Option<bool> {
        self.node(node_id)?.is_abstract()
    }

    /// Every node any loaded set defines, primary set first.
    pub fn node_ids(&self) -> impl Iterator<Item = &NodeId> {
        self.index
            .all_keys()
            .filter(|key| self.index.definition(*key).is_some())
            .map(|key| self.index.node_id(key))
    }

    /// Every node of one loaded set, in the order the file lists them.
    pub fn node_ids_of(
        &self,
        set: SetId,
    ) -> impl Iterator<Item = NodeId> {
        self.sets.get(set.0).into_iter().flat_map(move |loaded| {
            loaded
                .node_set()
                .iter()
                .filter_map(|node| loaded.to_space_node_id(node.node_id()))
        })
    }

    pub fn primary_node_ids(&self) -> impl Iterator<Item = NodeId> {
        self.node_ids_of(SetId::PRIMARY)
    }

    pub(crate) fn index(&self) -> &SpaceIndex {
        &self.index
    }

    /// Mutates the primary nodeset and reindexes from the changes the edit reports.
    ///
    /// Reporting every change is the contract that keeps the indexes true; an edit that hides one
    /// leaves the space stale until the next [`Self::rebuild`].
    pub fn edit(
        &mut self,
        edit: impl FnOnce(&mut NodeSet) -> Vec<Delta>,
    ) -> Vec<Delta> {
        let deltas = edit(self.sets[SetId::PRIMARY.0].node_set_mut());
        let deltas: Vec<Delta> = deltas
            .into_iter()
            .map(|delta| self.to_space_delta(delta))
            .collect();
        self.reindex(&deltas);
        deltas
    }

    /// Restates the NodeId a change names in the space's indexes.
    ///
    /// A change names a node the way the file spells it, which is another index whenever the file
    /// lists the OPC UA namespace in its own table; everything downstream of a delta — the indexes,
    /// the rules engine, the findings — speaks the space's.
    fn to_space_delta(
        &self,
        delta: Delta,
    ) -> Delta {
        let restate = |local: NodeId| {
            self.to_space_node_id(SetId::PRIMARY, &local)
                .unwrap_or(local)
        };
        match delta {
            Delta::NodeAdded { node_id } => Delta::NodeAdded {
                node_id: restate(node_id),
            },
            Delta::NodeRemoved { node_id } => Delta::NodeRemoved {
                node_id: restate(node_id),
            },
            Delta::FieldChanged { node_id, field } => Delta::FieldChanged {
                node_id: restate(node_id),
                field,
            },
            Delta::ReferenceAdded { holder, reference } => Delta::ReferenceAdded {
                holder: restate(holder),
                reference,
            },
            Delta::ReferenceRemoved { holder, reference } => Delta::ReferenceRemoved {
                holder: restate(holder),
                reference,
            },
            other => other,
        }
    }

    /// Brings the indexes back in line with the model after the reported changes.
    pub fn reindex(
        &mut self,
        deltas: &[Delta],
    ) {
        if deltas.iter().any(needs_full_rebuild) {
            self.rebuild();
            return;
        }
        let mut touched_graph = false;
        for delta in deltas {
            touched_graph |= delta.touches_graph();
            self.reindex_one(delta);
        }
        if touched_graph {
            self.index.rebuild_type_hierarchy();
        }
    }

    fn reindex_one(
        &mut self,
        delta: &Delta,
    ) {
        let Some(node_id) = delta.node_id().cloned() else {
            return;
        };
        match delta {
            Delta::NodeRemoved { .. } => {
                self.index.retract(&self.sets, &node_id);
                self.index.undeclare(&self.sets, &node_id);
                self.reindex_definition(&node_id);
            }
            _ => {
                let Some(local) = self.to_local_node_id(SetId::PRIMARY, &node_id) else {
                    return;
                };
                let Some(node) = self.primary().node(&local).cloned() else {
                    return;
                };
                self.index.declare(&self.sets, SetId::PRIMARY, &node);
                self.index
                    .index_references(&self.sets, SetId::PRIMARY, &node);
            }
        }
    }

    /// States the references of whichever set still defines the node, after another gave it up.
    fn reindex_definition(
        &mut self,
        space_id: &NodeId,
    ) {
        let Some(set) = self.set_of(space_id) else {
            return;
        };
        let Some(node) = self
            .to_local_node_id(set, space_id)
            .and_then(|local| self.set(set)?.node(&local))
            .cloned()
        else {
            return;
        };
        self.index.index_references(&self.sets, set, &node);
    }

    /// Discards every index and derives them again from the loaded sets.
    pub fn rebuild(&mut self) {
        self.remap_namespaces();
        self.index.rebuild(&self.sets);
    }

    fn remap_namespaces(&mut self) {
        self.namespaces = NamespaceTable::default();
        for set in &mut self.sets {
            set.remap(&mut self.namespaces);
        }
    }
}

/// Changes that move namespace indexes or reference targets under every node at once.
fn needs_full_rebuild(delta: &Delta) -> bool {
    matches!(delta, Delta::NamespacesChanged | Delta::AliasesChanged | Delta::DependencyAdded { .. })
}

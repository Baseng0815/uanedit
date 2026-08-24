use indexmap::{
    IndexMap,
    IndexSet,
};

use crate::ids;
use crate::nodes::node::Node;
use crate::space::keys::{
    Keys,
    NodeKey,
};
use crate::space::set::{
    LoadedSet,
    SetId,
};
use crate::space::standard;
use crate::types::node_id::NodeId;
use crate::types::node_id_ref::NodeIdRef;

/// One reference of the graph, seen from one of its two ends.
///
/// A reference is identified by its source, its type and its target (OPC 10000-3 §4.4.4), so a file
/// that states it on both ends states one reference and the index holds one edge for it — carrying
/// both places the file wrote it down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub other: NodeKey,
    pub reference_type: NodeKey,
    /// The position in this node's own reference list, when this node states the reference.
    pub stored: Option<u32>,
    /// The position in the other node's reference list, when that node states it as well.
    pub other_stored: Option<u32>,
}

impl Edge {
    fn is_stated(self) -> bool {
        self.stored.is_some() || self.other_stored.is_some()
    }
}

/// Where a statement sits across the loaded sets: which set, which node of it, which reference.
///
/// Ordering edges by this is what makes an incremental reindex leave the graph exactly as a rebuild
/// from the same model would.
type Placement = (u32, u32, u32);

const UNPLACED: Placement = (u32::MAX, u32::MAX, u32::MAX);

/// Where a node is defined, and how the file that defines it spells its NodeId.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub set: SetId,
    pub local: NodeId,
}

/// The derived graph an address space answers queries from.
#[derive(Clone, Debug, Default)]
pub struct SpaceIndex {
    keys: Keys,
    defined: Vec<Option<Definition>>,
    outgoing: Vec<Vec<Edge>>,
    incoming: Vec<Vec<Edge>>,
    supertypes: IndexMap<NodeKey, Vec<NodeKey>>,
    subtypes: IndexMap<NodeKey, Vec<NodeKey>>,
    hierarchical: IndexSet<NodeKey>,
    has_child: IndexSet<NodeKey>,
}

impl SpaceIndex {
    pub fn key(
        &self,
        node_id: &NodeId,
    ) -> Option<NodeKey> {
        self.keys.get(node_id)
    }

    pub fn node_id(
        &self,
        key: NodeKey,
    ) -> &NodeId {
        self.keys.node_id(key)
    }

    pub fn definition(
        &self,
        key: NodeKey,
    ) -> Option<&Definition> {
        self.defined.get(key.index())?.as_ref()
    }

    pub fn outgoing(
        &self,
        key: NodeKey,
    ) -> &[Edge] {
        self.outgoing.get(key.index()).map_or(&[], Vec::as_slice)
    }

    pub fn incoming(
        &self,
        key: NodeKey,
    ) -> &[Edge] {
        self.incoming.get(key.index()).map_or(&[], Vec::as_slice)
    }

    /// Every NodeId the space knows, defined or only referenced, in the order it first appeared.
    pub fn all_keys(&self) -> impl Iterator<Item = NodeKey> {
        (0..self.keys.len()).map(NodeKey::from_index)
    }

    pub fn direct_supertypes(
        &self,
        key: NodeKey,
    ) -> &[NodeKey] {
        self.supertypes.get(&key).map_or(&[], Vec::as_slice)
    }

    pub fn direct_subtypes(
        &self,
        key: NodeKey,
    ) -> &[NodeKey] {
        self.subtypes.get(&key).map_or(&[], Vec::as_slice)
    }

    pub fn is_hierarchical(
        &self,
        reference_type: NodeKey,
    ) -> bool {
        self.hierarchical.contains(&reference_type)
    }

    pub fn is_has_child(
        &self,
        reference_type: NodeKey,
    ) -> bool {
        self.has_child.contains(&reference_type)
    }

    /// Rebuilds every index from the loaded sets, from an empty key table.
    pub fn rebuild(
        &mut self,
        sets: &[LoadedSet],
    ) {
        self.keys = Keys::default();
        self.defined.clear();
        self.outgoing.clear();
        self.incoming.clear();
        for (position, set) in sets.iter().enumerate() {
            let set_id = SetId(position);
            for node in set.node_set().iter() {
                self.declare(sets, set_id, node);
            }
            for node in set.node_set().iter() {
                self.index_references(sets, set_id, node);
            }
        }
        self.rebuild_type_hierarchy();
    }

    /// Records that `set` defines `node`, interning its NodeId in the space's namespace indexes.
    ///
    /// A set further down the list never takes a node away from one nearer the primary nodeset,
    /// which is what keeps a NodeId two files define editable.
    pub fn declare(
        &mut self,
        sets: &[LoadedSet],
        set: SetId,
        node: &Node,
    ) -> Option<NodeKey> {
        let space_id = sets.get(set.0)?.to_space_node_id(node.node_id())?;
        let key = self.intern(&space_id);
        let nearer = self
            .defined
            .get(key.index())
            .and_then(Option::as_ref)
            .is_some_and(|existing| existing.set < set);
        if !nearer {
            self.defined[key.index()] = Some(Definition {
                set,
                local: node.node_id().clone(),
            });
        }
        Some(key)
    }

    /// Drops the edges a node stores itself, for a node the primary set no longer defines.
    pub fn retract(
        &mut self,
        sets: &[LoadedSet],
        space_id: &NodeId,
    ) {
        if let Some(key) = self.keys.get(space_id) {
            self.retract_stored(sets, key);
        }
    }

    /// Drops a node's definition, falling back to the next set that still defines the NodeId.
    pub fn undeclare(
        &mut self,
        sets: &[LoadedSet],
        space_id: &NodeId,
    ) {
        if let Some(key) = self.keys.get(space_id) {
            self.defined[key.index()] = defining_set(sets, space_id);
        }
    }

    /// Replaces the edges `node` states, merging them with what the nodes it names state.
    ///
    /// Only the set that defines the node states its references: where two files define one NodeId
    /// — which UA0102 reports — the one nearer the primary nodeset owns both its attributes and
    /// its edges, rather than the two of them interleaving by load order.
    pub fn index_references(
        &mut self,
        sets: &[LoadedSet],
        set: SetId,
        node: &Node,
    ) {
        let Some(loaded) = sets.get(set.0) else {
            return;
        };
        let Some(space_id) = loaded.to_space_node_id(node.node_id()) else {
            return;
        };
        let key = self.intern(&space_id);
        if self
            .definition(key)
            .is_some_and(|defined| defined.set != set)
        {
            return;
        }
        self.retract_stored(sets, key);
        for (position, reference) in node.references().iter().enumerate() {
            let Some(reference_type) = self.resolve(loaded, &reference.reference_type) else {
                continue;
            };
            let Some(other) = self.resolve(loaded, &reference.target) else {
                continue;
            };
            let position = u32::try_from(position).unwrap_or(u32::MAX);
            match reference.is_forward {
                true => self.state(sets, key, other, reference_type, true, position),
                false => self.state(sets, other, key, reference_type, false, position),
            }
        }
    }

    /// Recomputes the type hierarchy and the reference-type classification from the edges.
    pub fn rebuild_type_hierarchy(&mut self) {
        self.supertypes.clear();
        self.subtypes.clear();
        for (standard_subtype, standard_supertype) in standard::REFERENCE_TYPE_SUPERTYPES {
            let subtype = self.intern(standard_subtype);
            let supertype = self.intern(standard_supertype);
            self.relate(subtype, supertype);
        }
        let Some(has_subtype) = self.keys.get(&ids::HAS_SUBTYPE) else {
            return;
        };
        for index in 0..self.outgoing.len() {
            let supertype = NodeKey::from_index(index);
            let subtypes: Vec<NodeKey> = self.outgoing[index]
                .iter()
                .filter(|edge| edge.reference_type == has_subtype)
                .map(|edge| edge.other)
                .collect();
            for subtype in subtypes {
                self.relate(subtype, supertype);
            }
        }
        self.hierarchical = self.closure(&ids::HIERARCHICAL_REFERENCES);
        self.has_child = self.closure(&ids::HAS_CHILD);
    }

    /// Every subtype of `root`, transitively, including `root` itself.
    fn closure(
        &self,
        root: &NodeId,
    ) -> IndexSet<NodeKey> {
        let mut reached = IndexSet::new();
        let Some(root) = self.keys.get(root) else {
            return reached;
        };
        reached.insert(root);
        let mut position = 0;
        while position < reached.len() {
            let key = reached[position];
            position += 1;
            for subtype in self.direct_subtypes(key) {
                reached.insert(*subtype);
            }
        }
        reached
    }

    fn relate(
        &mut self,
        subtype: NodeKey,
        supertype: NodeKey,
    ) {
        if subtype == supertype {
            return;
        }
        let supertypes = self.supertypes.entry(subtype).or_default();
        if !supertypes.contains(&supertype) {
            supertypes.push(supertype);
        }
        let subtypes = self.subtypes.entry(supertype).or_default();
        if !subtypes.contains(&subtype) {
            subtypes.push(subtype);
        }
    }

    fn resolve(
        &mut self,
        loaded: &LoadedSet,
        reference: &NodeIdRef,
    ) -> Option<NodeKey> {
        let local = reference.resolve(&loaded.node_set().aliases)?;
        let space_id = loaded.to_space_node_id(local)?;
        Some(self.intern(&space_id))
    }

    /// Records that one end states the reference, merging it into the edge if it already exists.
    fn state(
        &mut self,
        sets: &[LoadedSet],
        source: NodeKey,
        target: NodeKey,
        reference_type: NodeKey,
        by_source: bool,
        position: u32,
    ) {
        let Some(at) = self.edge_index(source, target, reference_type, true) else {
            let (source_stored, target_stored) = match by_source {
                true => (Some(position), None),
                false => (None, Some(position)),
            };
            self.push_edge(sets, source, target, reference_type, source_stored, target_stored);
            return;
        };
        let before = self.edge_place(sets, source, self.outgoing[source.index()][at]);
        let edge = &mut self.outgoing[source.index()][at];
        match by_source {
            true => edge.stored = Some(position),
            false => edge.other_stored = Some(position),
        }
        if let Some(mirror) = self.edge_index(target, source, reference_type, false) {
            let edge = &mut self.incoming[target.index()][mirror];
            match by_source {
                true => edge.other_stored = Some(position),
                false => edge.stored = Some(position),
            }
        }
        let after = self.edge_place(sets, source, self.outgoing[source.index()][at]);
        if after != before {
            self.reorder(sets, source, target, reference_type);
        }
    }

    fn push_edge(
        &mut self,
        sets: &[LoadedSet],
        source: NodeKey,
        target: NodeKey,
        reference_type: NodeKey,
        source_stored: Option<u32>,
        target_stored: Option<u32>,
    ) {
        let edge = Edge {
            other: target,
            reference_type,
            stored: source_stored,
            other_stored: target_stored,
        };
        let mirror = Edge {
            other: source,
            reference_type,
            stored: target_stored,
            other_stored: source_stored,
        };
        let place = self.edge_place(sets, source, edge);
        let at = self.slot(sets, source, place, true);
        self.outgoing[source.index()].insert(at, edge);
        let at = self.slot(sets, target, place, false);
        self.incoming[target.index()].insert(at, mirror);
    }

    /// Puts an edge whose place has moved back where statement order says it belongs.
    fn reorder(
        &mut self,
        sets: &[LoadedSet],
        source: NodeKey,
        target: NodeKey,
        reference_type: NodeKey,
    ) {
        let Some(at) = self.edge_index(source, target, reference_type, true) else {
            return;
        };
        let edge = self.outgoing[source.index()].remove(at);
        let place = self.edge_place(sets, source, edge);
        let at = self.slot(sets, source, place, true);
        self.outgoing[source.index()].insert(at, edge);
        let Some(at) = self.edge_index(target, source, reference_type, false) else {
            return;
        };
        let mirror = self.incoming[target.index()].remove(at);
        let at = self.slot(sets, target, place, false);
        self.incoming[target.index()].insert(at, mirror);
    }

    /// Forgets the statement one end holds, dropping the edge when the other end holds none.
    fn drop_statement(
        &mut self,
        sets: &[LoadedSet],
        source: NodeKey,
        target: NodeKey,
        reference_type: NodeKey,
        by_source: bool,
    ) {
        let Some(at) = self.edge_index(source, target, reference_type, true) else {
            return;
        };
        let edge = &mut self.outgoing[source.index()][at];
        match by_source {
            true => edge.stored = None,
            false => edge.other_stored = None,
        }
        let survives = edge.is_stated();
        if let Some(mirror) = self.edge_index(target, source, reference_type, false) {
            let edge = &mut self.incoming[target.index()][mirror];
            match by_source {
                true => edge.other_stored = None,
                false => edge.stored = None,
            }
            if !survives {
                self.incoming[target.index()].remove(mirror);
            }
        }
        match survives {
            true => self.reorder(sets, source, target, reference_type),
            false => {
                self.outgoing[source.index()].remove(at);
            }
        }
    }

    /// Removes every statement this node holds, keeping the edges the other end still states.
    fn retract_stored(
        &mut self,
        sets: &[LoadedSet],
        key: NodeKey,
    ) {
        let stated: Vec<Edge> = self.outgoing[key.index()]
            .iter()
            .filter(|edge| edge.stored.is_some())
            .copied()
            .collect();
        for edge in stated {
            self.drop_statement(sets, key, edge.other, edge.reference_type, true);
        }
        let stated: Vec<Edge> = self.incoming[key.index()]
            .iter()
            .filter(|edge| edge.stored.is_some())
            .copied()
            .collect();
        for edge in stated {
            self.drop_statement(sets, edge.other, key, edge.reference_type, false);
        }
    }

    fn edge_index(
        &self,
        owner: NodeKey,
        other: NodeKey,
        reference_type: NodeKey,
        forward: bool,
    ) -> Option<usize> {
        let list = match forward {
            true => self.outgoing.get(owner.index())?,
            false => self.incoming.get(owner.index())?,
        };
        list.iter()
            .position(|edge| edge.other == other && edge.reference_type == reference_type)
    }

    /// Where an edge with this place belongs in a list kept in statement order.
    fn slot(
        &self,
        sets: &[LoadedSet],
        owner: NodeKey,
        place: Placement,
        forward: bool,
    ) -> usize {
        let list = match forward {
            true => &self.outgoing[owner.index()],
            false => &self.incoming[owner.index()],
        };
        match list.last() {
            Some(last) if self.edge_place(sets, owner, *last) <= place => list.len(),
            _ => list.partition_point(|edge| self.edge_place(sets, owner, *edge) <= place),
        }
    }

    /// The edge's place, which is the earlier of the two statements that can make it.
    fn edge_place(
        &self,
        sets: &[LoadedSet],
        owner: NodeKey,
        edge: Edge,
    ) -> Placement {
        let own = edge
            .stored
            .map(|position| self.statement_place(sets, owner, position));
        let other = edge
            .other_stored
            .map(|position| self.statement_place(sets, edge.other, position));
        match (own, other) {
            (Some(own), Some(other)) => own.min(other),
            (Some(place), None) | (None, Some(place)) => place,
            (None, None) => UNPLACED,
        }
    }

    /// Where the statement a node holds at this position sits among all the loaded sets.
    fn statement_place(
        &self,
        sets: &[LoadedSet],
        holder: NodeKey,
        position: u32,
    ) -> Placement {
        let Some(definition) = self.definition(holder) else {
            return UNPLACED;
        };
        let node = sets
            .get(definition.set.0)
            .and_then(|set| set.node_set().nodes.get_index_of(&definition.local))
            .unwrap_or(usize::MAX);
        (index_of(definition.set.0), index_of(node), position)
    }

    fn intern(
        &mut self,
        space_id: &NodeId,
    ) -> NodeKey {
        let key = self.keys.intern(space_id);
        while self.defined.len() <= key.index() {
            self.defined.push(None);
            self.outgoing.push(Vec::new());
            self.incoming.push(Vec::new());
        }
        key
    }
}

/// The set nearest the primary nodeset that still defines this NodeId.
fn defining_set(
    sets: &[LoadedSet],
    space_id: &NodeId,
) -> Option<Definition> {
    sets.iter().enumerate().find_map(|(position, set)| {
        let local = set.to_local_node_id(space_id)?;
        set.node_set().node(&local)?;
        Some(Definition {
            set: SetId(position),
            local,
        })
    })
}

fn index_of(position: usize) -> u32 {
    u32::try_from(position).unwrap_or(u32::MAX)
}

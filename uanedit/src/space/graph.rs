use indexmap::IndexSet;

use crate::ids;
use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::space::address_space::AddressSpace;
use crate::space::index::Edge;
use crate::space::keys::NodeKey;
use crate::space::reference::{
    ReferenceStorage,
    ReferenceView,
};
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;

impl AddressSpace {
    /// Every reference touching the node, in both directions, whichever end the file stores.
    pub fn references(
        &self,
        node_id: &NodeId,
    ) -> Vec<ReferenceView> {
        let mut views = self.forward_references(node_id);
        views.extend(self.inverse_references(node_id));
        views
    }

    pub fn forward_references(
        &self,
        node_id: &NodeId,
    ) -> Vec<ReferenceView> {
        self.views(node_id, true)
    }

    /// The references naming this node as their target, which the file writes on the other end.
    pub fn inverse_references(
        &self,
        node_id: &NodeId,
    ) -> Vec<ReferenceView> {
        self.views(node_id, false)
    }

    fn views(
        &self,
        node_id: &NodeId,
        is_forward: bool,
    ) -> Vec<ReferenceView> {
        let Some(key) = self.index().key(node_id) else {
            return Vec::new();
        };
        let edges = match is_forward {
            true => self.index().outgoing(key),
            false => self.index().incoming(key),
        };
        edges
            .iter()
            .map(|edge| self.view(edge, is_forward))
            .collect()
    }

    fn view(
        &self,
        edge: &Edge,
        is_forward: bool,
    ) -> ReferenceView {
        ReferenceView {
            reference_type: self.index().node_id(edge.reference_type).clone(),
            is_forward,
            other: self.index().node_id(edge.other).clone(),
            storage: match (edge.stored, edge.other_stored) {
                (Some(position), Some(_)) => ReferenceStorage::Both(position as usize),
                (Some(position), None) => ReferenceStorage::Own(position as usize),
                (None, _) => ReferenceStorage::Synthesized,
            },
        }
    }

    /// The targets of the node's references of one type, optionally following the type's subtypes.
    pub fn targets_of_type(
        &self,
        node_id: &NodeId,
        reference_type: &NodeId,
        include_subtypes: bool,
    ) -> Vec<NodeId> {
        self.ends_of_type(node_id, reference_type, include_subtypes, true)
    }

    pub fn sources_of_type(
        &self,
        node_id: &NodeId,
        reference_type: &NodeId,
        include_subtypes: bool,
    ) -> Vec<NodeId> {
        self.ends_of_type(node_id, reference_type, include_subtypes, false)
    }

    fn ends_of_type(
        &self,
        node_id: &NodeId,
        reference_type: &NodeId,
        include_subtypes: bool,
        is_forward: bool,
    ) -> Vec<NodeId> {
        let Some(key) = self.index().key(node_id) else {
            return Vec::new();
        };
        let Some(wanted) = self.index().key(reference_type) else {
            return Vec::new();
        };
        let edges = match is_forward {
            true => self.index().outgoing(key),
            false => self.index().incoming(key),
        };
        edges
            .iter()
            .filter(|edge| self.matches_reference_type(edge.reference_type, wanted, include_subtypes))
            .map(|edge| self.index().node_id(edge.other).clone())
            .collect()
    }

    fn matches_reference_type(
        &self,
        found: NodeKey,
        wanted: NodeKey,
        include_subtypes: bool,
    ) -> bool {
        found == wanted || (include_subtypes && self.key_is_subtype_of(found, wanted))
    }

    /// The nodes this one reaches by a hierarchical reference, which is what a tree shows beneath it.
    pub fn children(
        &self,
        node_id: &NodeId,
    ) -> Vec<NodeId> {
        self.hierarchical_ends(node_id, true)
    }

    pub fn parents(
        &self,
        node_id: &NodeId,
    ) -> Vec<NodeId> {
        self.hierarchical_ends(node_id, false)
    }

    fn hierarchical_ends(
        &self,
        node_id: &NodeId,
        is_forward: bool,
    ) -> Vec<NodeId> {
        let Some(key) = self.index().key(node_id) else {
            return Vec::new();
        };
        let edges = match is_forward {
            true => self.index().outgoing(key),
            false => self.index().incoming(key),
        };
        edges
            .iter()
            .filter(|edge| self.index().is_hierarchical(edge.reference_type))
            .map(|edge| self.index().node_id(edge.other).clone())
            .collect()
    }

    /// True when no hierarchical reference points at the node, which is legal and worth showing.
    pub fn is_unrooted(
        &self,
        node_id: &NodeId,
    ) -> bool {
        let Some(key) = self.index().key(node_id) else {
            return false;
        };
        !self
            .index()
            .incoming(key)
            .iter()
            .any(|edge| self.index().is_hierarchical(edge.reference_type))
    }

    /// Every defined node no hierarchical reference points at.
    pub fn unrooted(&self) -> Vec<NodeId> {
        self.node_ids()
            .filter(|node_id| self.is_unrooted(node_id))
            .cloned()
            .collect()
    }

    pub fn is_hierarchical_reference_type(
        &self,
        reference_type: &NodeId,
    ) -> bool {
        self.index()
            .key(reference_type)
            .is_some_and(|key| self.index().is_hierarchical(key))
    }

    /// True for HasChild and its subtypes, the hierarchy OPC 10000-3 §7.5 forbids loops in.
    pub fn is_has_child_reference_type(
        &self,
        reference_type: &NodeId,
    ) -> bool {
        self.index()
            .key(reference_type)
            .is_some_and(|key| self.index().is_has_child(key))
    }

    /// The type an Object or Variable is an instance of, when it names exactly one.
    pub fn type_definition(
        &self,
        node_id: &NodeId,
    ) -> Option<NodeId> {
        let mut found = self.type_definitions(node_id);
        match found.len() {
            1 => Some(found.remove(0)),
            _ => None,
        }
    }

    /// Every HasTypeDefinition target, since a broken file may state none or several.
    ///
    /// Subtypes count: OPC 10000-3 §4.4.4 makes a subtype of a concrete ReferenceType equal to it
    /// when identifying references, and §3.3.2 extends every permission to the subtypes.
    pub fn type_definitions(
        &self,
        node_id: &NodeId,
    ) -> Vec<NodeId> {
        self.targets_of_type(node_id, &ids::HAS_TYPE_DEFINITION, true)
    }

    pub fn modelling_rule(
        &self,
        node_id: &NodeId,
    ) -> Option<NodeId> {
        self.targets_of_type(node_id, &ids::HAS_MODELLING_RULE, true)
            .into_iter()
            .next()
    }

    /// The DataType a Variable or VariableType names, restated in the space's namespace indexes.
    pub fn data_type(
        &self,
        node_id: &NodeId,
    ) -> Option<NodeId> {
        let set = self.set_of(node_id)?;
        let node = self.node(node_id)?;
        let data_type = match node {
            Node::Variable(variable) => &variable.data_type,
            Node::VariableType(variable_type) => &variable_type.data_type,
            _ => return None,
        };
        let local = self.set(set)?.resolve(data_type)?;
        self.to_space_node_id(set, local)
    }

    /// The node's direct supertypes; more than one means the file broke single inheritance.
    pub fn supertypes(
        &self,
        node_id: &NodeId,
    ) -> Vec<NodeId> {
        let Some(key) = self.index().key(node_id) else {
            return Vec::new();
        };
        self.to_node_ids(self.index().direct_supertypes(key))
    }

    pub fn supertype(
        &self,
        node_id: &NodeId,
    ) -> Option<NodeId> {
        self.supertypes(node_id).into_iter().next()
    }

    pub fn subtypes(
        &self,
        node_id: &NodeId,
    ) -> Vec<NodeId> {
        let Some(key) = self.index().key(node_id) else {
            return Vec::new();
        };
        self.to_node_ids(self.index().direct_subtypes(key))
    }

    fn to_node_ids(
        &self,
        keys: &[NodeKey],
    ) -> Vec<NodeId> {
        keys.iter()
            .map(|key| self.index().node_id(*key).clone())
            .collect()
    }

    /// The chain from the node's direct supertype up to the root of its type hierarchy.
    pub fn supertype_chain(
        &self,
        node_id: &NodeId,
    ) -> Vec<NodeId> {
        let Some(key) = self.index().key(node_id) else {
            return Vec::new();
        };
        self.key_supertype_chain(key)
            .into_iter()
            .skip(1)
            .map(|key| self.index().node_id(key).clone())
            .collect()
    }

    /// The node and its supertypes, nearest first, stopping if the file states a cycle.
    pub(crate) fn key_supertype_chain(
        &self,
        key: NodeKey,
    ) -> IndexSet<NodeKey> {
        let mut chain = IndexSet::new();
        chain.insert(key);
        let mut position = 0;
        while position < chain.len() {
            let current = chain[position];
            position += 1;
            for supertype in self.index().direct_supertypes(current) {
                chain.insert(*supertype);
            }
        }
        chain
    }

    /// Every subtype of the node, transitively, nearest first.
    pub fn all_subtypes(
        &self,
        node_id: &NodeId,
    ) -> Vec<NodeId> {
        let Some(key) = self.index().key(node_id) else {
            return Vec::new();
        };
        self.key_subtree(key)
            .into_iter()
            .skip(1)
            .map(|key| self.index().node_id(key).clone())
            .collect()
    }

    pub(crate) fn key_subtree(
        &self,
        key: NodeKey,
    ) -> IndexSet<NodeKey> {
        let mut reached = IndexSet::new();
        reached.insert(key);
        let mut position = 0;
        while position < reached.len() {
            let current = reached[position];
            position += 1;
            for subtype in self.index().direct_subtypes(current) {
                reached.insert(*subtype);
            }
        }
        reached
    }

    /// True when `subtype` is `supertype` or reaches it by inverse HasSubtype references.
    pub fn is_same_or_subtype_of(
        &self,
        subtype: &NodeId,
        supertype: &NodeId,
    ) -> bool {
        subtype == supertype || self.is_subtype_of(subtype, supertype)
    }

    pub fn is_subtype_of(
        &self,
        subtype: &NodeId,
        supertype: &NodeId,
    ) -> bool {
        let (Some(subtype), Some(supertype)) = (self.index().key(subtype), self.index().key(supertype)) else {
            return false;
        };
        self.key_is_subtype_of(subtype, supertype)
    }

    pub(crate) fn key_is_subtype_of(
        &self,
        subtype: NodeKey,
        supertype: NodeKey,
    ) -> bool {
        subtype != supertype && self.key_supertype_chain(subtype).contains(&supertype)
    }

    /// The hierarchical child of `parent` with this BrowseName, or several if the file repeats one.
    pub fn children_named(
        &self,
        parent: &NodeId,
        browse_name: &QualifiedName,
    ) -> Vec<NodeId> {
        self.children(parent)
            .into_iter()
            .filter(|child| self.browse_name(child).as_ref() == Some(browse_name))
            .collect()
    }

    /// Whether `to` is reachable from `from` by HasChild references, which is what a cycle is.
    pub fn reaches_via_has_child(
        &self,
        from: &NodeId,
        to: &NodeId,
    ) -> bool {
        let (Some(from), Some(to)) = (self.index().key(from), self.index().key(to)) else {
            return false;
        };
        let mut reached = IndexSet::new();
        reached.insert(from);
        let mut position = 0;
        while position < reached.len() {
            let current = reached[position];
            position += 1;
            for edge in self.index().outgoing(current) {
                if !self.index().is_has_child(edge.reference_type) {
                    continue;
                }
                if edge.other == to {
                    return true;
                }
                reached.insert(edge.other);
            }
        }
        false
    }

    pub fn nodes_of_class(
        &self,
        node_class: NodeClass,
    ) -> impl Iterator<Item = &NodeId> {
        self.node_ids()
            .filter(move |node_id| self.node_class(node_id) == Some(node_class))
    }
}

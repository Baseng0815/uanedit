use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::nodeset::aliases::AliasTable;
use crate::nodeset::models::ModelTableEntry;
use crate::nodeset::namespaces::NamespaceTable;
use crate::types::date_time::DateTime;
use crate::types::node_id::NodeId;
use crate::types::node_id_ref::NodeIdRef;
use crate::types::xml::XmlElement;

/// One `.NodeSet2.xml` document: its tables and the nodes it defines.
///
/// Node order is the document's own, because a save that reorders them would rewrite the file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeSet {
    pub last_modified: Option<DateTime>,
    pub namespaces: NamespaceTable,
    pub server_uris: Vec<String>,
    pub models: Vec<ModelTableEntry>,
    pub aliases: AliasTable,
    pub extensions: Vec<XmlElement>,
    pub nodes: IndexMap<NodeId, Node>,
}

impl NodeSet {
    pub fn node(
        &self,
        node_id: &NodeId,
    ) -> Option<&Node> {
        self.nodes.get(node_id)
    }

    pub fn node_mut(
        &mut self,
        node_id: &NodeId,
    ) -> Option<&mut Node> {
        self.nodes.get_mut(node_id)
    }

    /// Adds a node, replacing and returning any node that already had its NodeId.
    pub fn insert(
        &mut self,
        node: impl Into<Node>,
    ) -> Option<Node> {
        let node = node.into();
        self.nodes.insert(node.node_id().clone(), node)
    }

    /// Removes a node, keeping the order of the rest.
    pub fn remove(
        &mut self,
        node_id: &NodeId,
    ) -> Option<Node> {
        self.nodes.shift_remove(node_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.nodes.values_mut()
    }

    pub fn nodes_of_class(
        &self,
        node_class: NodeClass,
    ) -> impl Iterator<Item = &Node> {
        self.iter()
            .filter(move |node| node.node_class() == node_class)
    }

    /// Follows the alias table where a reference named one.
    pub fn resolve<'a>(
        &'a self,
        reference: &'a NodeIdRef,
    ) -> Option<&'a NodeId> {
        reference.resolve(&self.aliases)
    }

    /// Resolves a reference all the way to the node it names.
    pub fn target(
        &self,
        reference: &NodeIdRef,
    ) -> Option<&Node> {
        self.node(self.resolve(reference)?)
    }

    /// The model this file defines, which the schema puts first in the table.
    pub fn model(&self) -> Option<&ModelTableEntry> {
        self.models.first()
    }

    /// The namespace this file defines nodes in, which is index 1 in a single-model nodeset.
    pub fn target_namespace(&self) -> Option<&str> {
        match self.model() {
            Some(model) => Some(&model.model_uri),
            None => self.namespaces.uris().first().map(String::as_str),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

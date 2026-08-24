use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

use crate::space::address_space::AddressSpace;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;

/// A sequence of BrowseNames describing a path between nodes (OPC 10000-3 §6.2.5).
///
/// This is how an instance declaration is identified, since its NodeId differs on every subtype.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BrowsePath(Vec<QualifiedName>);

impl BrowsePath {
    /// The empty path, which names the node a path is resolved from.
    pub fn root() -> Self {
        Self::default()
    }

    pub fn elements(&self) -> &[QualifiedName] {
        &self.0
    }

    /// This path extended by one BrowseName.
    pub fn child(
        &self,
        browse_name: QualifiedName,
    ) -> Self {
        let mut extended = self.0.clone();
        extended.push(browse_name);
        Self(extended)
    }

    /// The path with its last element dropped, absent for the root.
    pub fn parent(&self) -> Option<Self> {
        let (_, head) = self.0.split_last()?;
        Some(Self(head.to_vec()))
    }

    pub fn last(&self) -> Option<&QualifiedName> {
        self.0.last()
    }

    /// True when this path starts with `prefix`, which is what makes it a descendant of it.
    pub fn starts_with(
        &self,
        prefix: &Self,
    ) -> bool {
        self.0.len() >= prefix.0.len() && self.0[..prefix.0.len()] == prefix.0[..]
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<QualifiedName>> for BrowsePath {
    fn from(elements: Vec<QualifiedName>) -> Self {
        Self(elements)
    }
}

impl FromIterator<QualifiedName> for BrowsePath {
    fn from_iter<T: IntoIterator<Item = QualifiedName>>(elements: T) -> Self {
        Self(elements.into_iter().collect())
    }
}

impl fmt::Display for BrowsePath {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        for element in &self.0 {
            write!(f, "/{element}")?;
        }
        Ok(())
    }
}

impl AddressSpace {
    /// Every node the path reaches from `start` by hierarchical references.
    ///
    /// More than one answer is legal: a file may give two children the same BrowseName.
    pub fn resolve_browse_path(
        &self,
        start: &NodeId,
        path: &BrowsePath,
    ) -> Vec<NodeId> {
        let mut reached = vec![start.clone()];
        for element in path.elements() {
            reached = reached
                .iter()
                .flat_map(|node_id| self.children_named(node_id, element))
                .collect();
            if reached.is_empty() {
                break;
            }
        }
        reached
    }
}

use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

use crate::nodeset::aliases::AliasTable;
use crate::types::node_id::NodeId;

/// A NodeId as a nodeset writes it: spelled out, or naming an entry in the alias table.
///
/// Which of the two a document used is kept so a save does not rewrite every reference.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NodeIdRef {
    Id(NodeId),
    Alias(String),
}

impl NodeIdRef {
    pub fn alias(name: impl Into<String>) -> Self {
        Self::Alias(name.into())
    }

    pub fn as_alias(&self) -> Option<&str> {
        match self {
            Self::Alias(name) => Some(name),
            Self::Id(_) => None,
        }
    }

    pub fn as_id(&self) -> Option<&NodeId> {
        match self {
            Self::Id(node_id) => Some(node_id),
            Self::Alias(_) => None,
        }
    }

    pub fn resolve<'a>(
        &'a self,
        aliases: &'a AliasTable,
    ) -> Option<&'a NodeId> {
        match self {
            Self::Id(node_id) => Some(node_id),
            Self::Alias(name) => aliases.get(name),
        }
    }
}

impl Default for NodeIdRef {
    fn default() -> Self {
        Self::Id(NodeId::NULL)
    }
}

impl From<NodeId> for NodeIdRef {
    fn from(node_id: NodeId) -> Self {
        Self::Id(node_id)
    }
}

impl fmt::Display for NodeIdRef {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Id(node_id) => write!(f, "{node_id}"),
            Self::Alias(name) => f.write_str(name),
        }
    }
}

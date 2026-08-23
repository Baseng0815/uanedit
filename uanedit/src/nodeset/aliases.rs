use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::types::node_id::NodeId;

/// The nodeset's alias table, which lets references name a NodeId by a short symbol.
///
/// Insertion order is the order the file lists them in, and is preserved on write.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasTable(IndexMap<String, NodeId>);

impl AliasTable {
    pub fn get(
        &self,
        alias: &str,
    ) -> Option<&NodeId> {
        self.0.get(alias)
    }

    pub fn insert(
        &mut self,
        alias: impl Into<String>,
        node_id: NodeId,
    ) -> Option<NodeId> {
        self.0.insert(alias.into(), node_id)
    }

    pub fn remove(
        &mut self,
        alias: &str,
    ) -> Option<NodeId> {
        self.0.shift_remove(alias)
    }

    /// The first alias that stands for `node_id`, which is what a writer should reuse.
    pub fn alias_for(
        &self,
        node_id: &NodeId,
    ) -> Option<&str> {
        self.0
            .iter()
            .find(|(_, candidate)| *candidate == node_id)
            .map(|(alias, _)| alias.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &NodeId)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(String, NodeId)> for AliasTable {
    fn from_iter<T: IntoIterator<Item = (String, NodeId)>>(entries: T) -> Self {
        Self(entries.into_iter().collect())
    }
}

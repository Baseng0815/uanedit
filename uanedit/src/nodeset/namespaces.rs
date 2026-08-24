use serde::{
    Deserialize,
    Serialize,
};

use crate::ids;
use crate::types::node_id::NamespaceIndex;

/// The namespace URIs a nodeset declares, which are the entries from index 1 up.
///
/// Index 0 is always the OPC UA namespace and is never written to the file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceTable(Vec<String>);

impl From<Vec<String>> for NamespaceTable {
    fn from(uris: Vec<String>) -> Self {
        Self(uris)
    }
}

impl NamespaceTable {
    pub fn new(uris: Vec<String>) -> Self {
        Self(uris)
    }

    /// The URIs as the document lists them, starting at index 1.
    pub fn uris(&self) -> &[String] {
        &self.0
    }

    pub fn uri(
        &self,
        index: NamespaceIndex,
    ) -> Option<&str> {
        match index {
            0 => Some(ids::BASE_NAMESPACE_URI),
            _ => self.0.get(usize::from(index) - 1).map(String::as_str),
        }
    }

    pub fn index_of(
        &self,
        uri: &str,
    ) -> Option<NamespaceIndex> {
        if uri == ids::BASE_NAMESPACE_URI {
            return Some(0);
        }
        let position = self.0.iter().position(|candidate| candidate == uri)?;
        NamespaceIndex::try_from(position + 1).ok()
    }

    /// Appends `uri` unless it is already present, returning the index it now has.
    pub fn insert(
        &mut self,
        uri: impl Into<String>,
    ) -> Option<NamespaceIndex> {
        let uri = uri.into();
        if let Some(index) = self.index_of(&uri) {
            return Some(index);
        }
        self.0.push(uri);
        NamespaceIndex::try_from(self.0.len()).ok()
    }

    /// The number of namespaces including index 0, so one more than the file lists.
    pub fn len(&self) -> usize {
        self.0.len() + 1
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

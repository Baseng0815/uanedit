use core::cmp::Ordering;
use core::fmt;
use core::hash::{
    Hash,
    Hasher,
};
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

use crate::error::ParseError;
use crate::types::node_id::NamespaceIndex;

/// A QualifiedName (OPC 10000-3 §8.3), in the `<index>:<name>` textual form nodesets use.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QualifiedName {
    pub namespace_index: NamespaceIndex,
    pub name: String,
    /// Set when the document spelled out index 0, which the standard nodeset does. It takes no
    /// part in equality.
    pub explicit_index: bool,
}

impl QualifiedName {
    pub fn new(
        namespace_index: NamespaceIndex,
        name: impl Into<String>,
    ) -> Self {
        Self {
            namespace_index,
            name: name.into(),
            explicit_index: false,
        }
    }

    pub fn is_null(&self) -> bool {
        self.namespace_index == 0 && self.name.is_empty()
    }
}

impl PartialEq for QualifiedName {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.namespace_index == other.namespace_index && self.name == other.name
    }
}

impl Eq for QualifiedName {}

impl Hash for QualifiedName {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.namespace_index.hash(state);
        self.name.hash(state);
    }
}

impl Ord for QualifiedName {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.namespace_index
            .cmp(&other.namespace_index)
            .then_with(|| self.name.cmp(&other.name))
    }
}

impl PartialOrd for QualifiedName {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for QualifiedName {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if self.namespace_index != 0 || self.explicit_index || starts_with_index(&self.name) {
            write!(f, "{}:", self.namespace_index)?;
        }
        f.write_str(&self.name)
    }
}

/// A name that would be read back as an index if the index were left off.
fn starts_with_index(name: &str) -> bool {
    name.split_once(':')
        .is_some_and(|(prefix, _)| !prefix.is_empty() && prefix.bytes().all(|digit| digit.is_ascii_digit()))
}

impl FromStr for QualifiedName {
    type Err = ParseError;

    /// A leading `<digits>:` is the namespace index; any other colon — including one behind digits
    /// that no namespace table could ever be that long — belongs to the name.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let Some((prefix, name)) = text.split_once(':') else {
            return Ok(Self::new(0, text));
        };
        if !prefix.bytes().all(|digit| digit.is_ascii_digit()) || prefix.is_empty() {
            return Ok(Self::new(0, text));
        }
        let Ok(namespace_index) = prefix.parse() else {
            return Ok(Self::new(0, text));
        };
        Ok(Self {
            namespace_index,
            name: name.to_owned(),
            explicit_index: namespace_index == 0,
        })
    }
}

use core::fmt;
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

use crate::error::ParseError;
use crate::types::node_id::{
    NodeId,
    parse_identifier,
};

/// An ExpandedNodeId (OPC 10000-6 §5.1.12): a NodeId that may name its namespace and its server
/// by URI rather than by index.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExpandedNodeId {
    pub server_index: u32,
    /// Set when the server was written as a URI, which then takes precedence over the index.
    pub server_uri: Option<String>,
    /// Set when the namespace was written as a URI, which then takes precedence over the index.
    pub namespace_uri: Option<String>,
    pub node_id: NodeId,
}

impl ExpandedNodeId {
    pub fn local(node_id: NodeId) -> Self {
        Self {
            node_id,
            ..Self::default()
        }
    }

    pub fn is_local(&self) -> bool {
        self.server_index == 0 && self.server_uri.is_none()
    }

    pub fn is_null(&self) -> bool {
        self.is_local() && self.namespace_uri.is_none() && self.node_id.is_null()
    }
}

impl From<NodeId> for ExpandedNodeId {
    fn from(node_id: NodeId) -> Self {
        Self::local(node_id)
    }
}

impl fmt::Display for ExpandedNodeId {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match &self.server_uri {
            Some(uri) => write!(f, "svu={};", escape_uri(uri))?,
            None if self.server_index != 0 => write!(f, "svr={};", self.server_index)?,
            None => {}
        }
        match &self.namespace_uri {
            Some(uri) => write!(f, "nsu={};", escape_uri(uri))?,
            None if self.node_id.namespace_index != 0 => write!(f, "ns={};", self.node_id.namespace_index)?,
            None => {}
        }
        write!(f, "{}", self.node_id.identifier)
    }
}

impl FromStr for ExpandedNodeId {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || ParseError::ExpandedNodeId(text.to_owned());
        let mut rest = text;

        let mut server_index = 0;
        let mut server_uri = None;
        if let Some(tail) = rest.strip_prefix("svu=") {
            let (uri, tail) = tail.split_once(';').ok_or_else(invalid)?;
            server_uri = Some(unescape_uri(uri));
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("svr=") {
            let (index, tail) = tail.split_once(';').ok_or_else(invalid)?;
            server_index = index.parse().map_err(|_| invalid())?;
            rest = tail;
        }

        let mut namespace_uri = None;
        let mut namespace_index = 0;
        if let Some(tail) = rest.strip_prefix("nsu=") {
            let (uri, tail) = tail.split_once(';').ok_or_else(invalid)?;
            namespace_uri = Some(unescape_uri(uri));
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("ns=") {
            let (index, tail) = tail.split_once(';').ok_or_else(invalid)?;
            namespace_index = index.parse().map_err(|_| invalid())?;
            rest = tail;
        }

        Ok(Self {
            server_index,
            server_uri,
            namespace_uri,
            node_id: NodeId {
                namespace_index,
                identifier: parse_identifier(rest).ok_or_else(invalid)?,
            },
        })
    }
}

/// Percent-encodes the character that would otherwise end the URI field, and the escape itself.
fn escape_uri(uri: &str) -> String {
    uri.replace('%', "%25").replace(';', "%3B")
}

fn unescape_uri(uri: &str) -> String {
    uri.replace("%3B", ";")
        .replace("%3b", ";")
        .replace("%25", "%")
}

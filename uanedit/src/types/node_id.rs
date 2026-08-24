use core::fmt;
use core::str::FromStr;

use serde::de::{
    self,
    Visitor,
};
use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
};

use crate::error::ParseError;
use crate::types::byte_string::ByteString;
use crate::types::guid::Guid;

/// An index into a nodeset's namespace table; 0 is always the OPC UA namespace.
pub type NamespaceIndex = u16;

/// A NodeId (OPC 10000-3 §8.2), in the `ns=<index>;<type>=<value>` textual form nodesets use.
///
/// Serialises as that textual form rather than as a struct, which is both smaller and a legal map
/// key in a format whose keys are strings.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId {
    pub namespace_index: NamespaceIndex,
    pub identifier: Identifier,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Identifier {
    Numeric(u32),
    String(String),
    Guid(Guid),
    Opaque(ByteString),
}

/// Which of the four identifier forms a NodeId uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IdentifierKind {
    Numeric,
    String,
    Guid,
    Opaque,
}

impl NodeId {
    pub const NULL: Self = Self {
        namespace_index: 0,
        identifier: Identifier::Numeric(0),
    };

    pub fn numeric(
        namespace_index: NamespaceIndex,
        value: u32,
    ) -> Self {
        Self {
            namespace_index,
            identifier: Identifier::Numeric(value),
        }
    }

    pub fn string(
        namespace_index: NamespaceIndex,
        value: impl Into<String>,
    ) -> Self {
        Self {
            namespace_index,
            identifier: Identifier::String(value.into()),
        }
    }

    pub fn guid(
        namespace_index: NamespaceIndex,
        value: Guid,
    ) -> Self {
        Self {
            namespace_index,
            identifier: Identifier::Guid(value),
        }
    }

    /// The textual grammar has no spelling for a null ByteString, so an opaque identifier holds an
    /// empty one instead and `b=` means the same thing on the way back in.
    pub fn opaque(
        namespace_index: NamespaceIndex,
        value: impl Into<ByteString>,
    ) -> Self {
        let value = value.into();
        Self {
            namespace_index,
            identifier: Identifier::Opaque(match value.is_null() {
                true => ByteString::from(Vec::new()),
                false => value,
            }),
        }
    }

    pub fn kind(&self) -> IdentifierKind {
        self.identifier.kind()
    }

    pub fn is_null(&self) -> bool {
        self.namespace_index == 0 && self.identifier == Identifier::Numeric(0)
    }

    /// True when this NodeId is defined by the standard and therefore not editable.
    pub fn is_in_base_namespace(&self) -> bool {
        self.namespace_index == 0
    }
}

impl Identifier {
    pub fn kind(&self) -> IdentifierKind {
        match self {
            Self::Numeric(_) => IdentifierKind::Numeric,
            Self::String(_) => IdentifierKind::String,
            Self::Guid(_) => IdentifierKind::Guid,
            Self::Opaque(_) => IdentifierKind::Opaque,
        }
    }
}

impl Default for Identifier {
    fn default() -> Self {
        Self::Numeric(0)
    }
}

impl IdentifierKind {
    /// The letter that introduces this form in the textual NodeId grammar.
    pub fn prefix(self) -> char {
        match self {
            Self::Numeric => 'i',
            Self::String => 's',
            Self::Guid => 'g',
            Self::Opaque => 'b',
        }
    }
}

impl fmt::Display for Identifier {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Numeric(value) => write!(f, "i={value}"),
            Self::String(value) => write!(f, "s={value}"),
            Self::Guid(value) => write!(f, "g={value}"),
            Self::Opaque(value) => write!(f, "b={value}"),
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if self.namespace_index != 0 {
            write!(f, "ns={};", self.namespace_index)?;
        }
        write!(f, "{}", self.identifier)
    }
}

impl FromStr for NodeId {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || ParseError::NodeId(text.to_owned());
        let (namespace_index, rest) = split_namespace(text).ok_or_else(invalid)?;
        Ok(Self {
            namespace_index,
            identifier: parse_identifier(rest).ok_or_else(invalid)?,
        })
    }
}

/// Splits the optional `ns=<index>;` prefix from the identifier that follows it.
fn split_namespace(text: &str) -> Option<(NamespaceIndex, &str)> {
    let Some(rest) = text.strip_prefix("ns=") else {
        return Some((0, text));
    };
    let (index, rest) = rest.split_once(';')?;
    Some((decimal(index)?, rest))
}

/// Reads an identifier from the part of a NodeId after the namespace; the value runs to the end.
pub(crate) fn parse_identifier(text: &str) -> Option<Identifier> {
    let (kind, value) = text.split_at_checked(2)?;
    match kind {
        "i=" => decimal(value).map(Identifier::Numeric),
        "s=" => Some(Identifier::String(value.to_owned())),
        "g=" => value.parse().ok().map(Identifier::Guid),
        "b=" => ByteString::decode(value).ok().map(Identifier::Opaque),
        _ => None,
    }
}

/// `1*DIGIT`, which is all the grammar admits (OPC 10000-6 §5.1.12): a sign or padding whitespace
/// would give one identifier two spellings, so it is refused rather than quietly normalised.
fn decimal<T: FromStr>(text: &str) -> Option<T> {
    match !text.is_empty() && text.bytes().all(|digit| digit.is_ascii_digit()) {
        true => text.parse().ok(),
        false => None,
    }
}

impl Serialize for NodeId {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TextualNodeId)
    }
}

struct TextualNodeId;

impl Visitor<'_> for TextualNodeId {
    type Value = NodeId;

    fn expecting(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str("a NodeId in textual form, such as `i=84` or `ns=1;s=Machine`")
    }

    fn visit_str<E: de::Error>(
        self,
        text: &str,
    ) -> Result<Self::Value, E> {
        text.parse().map_err(E::custom)
    }
}

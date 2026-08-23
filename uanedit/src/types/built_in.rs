use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

/// The closed set of built-in types every OPC UA value is ultimately made of (OPC 10000-6 §5.1.2).
///
/// The discriminants are the built-in type ids, and the names are the element names the XML
/// encoding uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum BuiltInType {
    #[default]
    Null            = 0,
    Boolean         = 1,
    SByte           = 2,
    Byte            = 3,
    Int16           = 4,
    UInt16          = 5,
    Int32           = 6,
    UInt32          = 7,
    Int64           = 8,
    UInt64          = 9,
    Float           = 10,
    Double          = 11,
    String          = 12,
    DateTime        = 13,
    Guid            = 14,
    ByteString      = 15,
    XmlElement      = 16,
    NodeId          = 17,
    ExpandedNodeId  = 18,
    StatusCode      = 19,
    QualifiedName   = 20,
    LocalizedText   = 21,
    ExtensionObject = 22,
    DataValue       = 23,
    Variant         = 24,
    DiagnosticInfo  = 25,
}

impl BuiltInType {
    pub const ALL: [Self; 26] = [
        Self::Null,
        Self::Boolean,
        Self::SByte,
        Self::Byte,
        Self::Int16,
        Self::UInt16,
        Self::Int32,
        Self::UInt32,
        Self::Int64,
        Self::UInt64,
        Self::Float,
        Self::Double,
        Self::String,
        Self::DateTime,
        Self::Guid,
        Self::ByteString,
        Self::XmlElement,
        Self::NodeId,
        Self::ExpandedNodeId,
        Self::StatusCode,
        Self::QualifiedName,
        Self::LocalizedText,
        Self::ExtensionObject,
        Self::DataValue,
        Self::Variant,
        Self::DiagnosticInfo,
    ];

    pub fn from_id(id: u8) -> Option<Self> {
        Self::ALL.get(usize::from(id)).copied()
    }

    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Boolean => "Boolean",
            Self::SByte => "SByte",
            Self::Byte => "Byte",
            Self::Int16 => "Int16",
            Self::UInt16 => "UInt16",
            Self::Int32 => "Int32",
            Self::UInt32 => "UInt32",
            Self::Int64 => "Int64",
            Self::UInt64 => "UInt64",
            Self::Float => "Float",
            Self::Double => "Double",
            Self::String => "String",
            Self::DateTime => "DateTime",
            Self::Guid => "Guid",
            Self::ByteString => "ByteString",
            Self::XmlElement => "XmlElement",
            Self::NodeId => "NodeId",
            Self::ExpandedNodeId => "ExpandedNodeId",
            Self::StatusCode => "StatusCode",
            Self::QualifiedName => "QualifiedName",
            Self::LocalizedText => "LocalizedText",
            Self::ExtensionObject => "ExtensionObject",
            Self::DataValue => "DataValue",
            Self::Variant => "Variant",
            Self::DiagnosticInfo => "DiagnosticInfo",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.name() == name)
    }

    /// The element name an array of this type takes in the XML encoding.
    pub fn list_name(self) -> String {
        format!("ListOf{}", self.name())
    }
}

impl fmt::Display for BuiltInType {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.name())
    }
}

use core::fmt;
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

use crate::error::ParseError;

/// The eight kinds of node (OPC 10000-3 §8.29); the discriminants are a bit mask, so a set of
/// node classes fits in one value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum NodeClass {
    #[default]
    Unspecified   = 0,
    Object        = 1,
    Variable      = 2,
    Method        = 4,
    ObjectType    = 8,
    VariableType  = 16,
    ReferenceType = 32,
    DataType      = 64,
    View          = 128,
}

impl NodeClass {
    pub const ALL: [Self; 8] = [
        Self::Object,
        Self::Variable,
        Self::Method,
        Self::ObjectType,
        Self::VariableType,
        Self::ReferenceType,
        Self::DataType,
        Self::View,
    ];

    pub fn bits(self) -> u32 {
        self as u32
    }

    pub fn from_bits(bits: u32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.bits() == bits)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unspecified => "Unspecified",
            Self::Object => "Object",
            Self::Variable => "Variable",
            Self::Method => "Method",
            Self::ObjectType => "ObjectType",
            Self::VariableType => "VariableType",
            Self::ReferenceType => "ReferenceType",
            Self::DataType => "DataType",
            Self::View => "View",
        }
    }

    /// The element a nodeset writes this node class as.
    pub fn element_name(self) -> Option<&'static str> {
        match self {
            Self::Unspecified => None,
            Self::Object => Some("UAObject"),
            Self::Variable => Some("UAVariable"),
            Self::Method => Some("UAMethod"),
            Self::ObjectType => Some("UAObjectType"),
            Self::VariableType => Some("UAVariableType"),
            Self::ReferenceType => Some("UAReferenceType"),
            Self::DataType => Some("UADataType"),
            Self::View => Some("UAView"),
        }
    }

    pub fn from_element_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.element_name() == Some(name))
    }

    /// True for the four classes that describe other nodes rather than instances.
    pub fn is_type(self) -> bool {
        matches!(self, Self::ObjectType | Self::VariableType | Self::ReferenceType | Self::DataType)
    }

    pub fn is_instance(self) -> bool {
        matches!(self, Self::Object | Self::Variable | Self::Method | Self::View)
    }
}

impl fmt::Display for NodeClass {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for NodeClass {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.name() == text)
            .ok_or_else(|| ParseError::NodeClass(text.to_owned()))
    }
}

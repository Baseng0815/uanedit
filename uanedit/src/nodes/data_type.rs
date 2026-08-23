use core::fmt;
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

use crate::error::ParseError;
use crate::nodes::common::NodeHeader;
use crate::nodes::definition::DataTypeDefinition;

/// A DataType node (OPC 10000-3 §5.8.3).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataType {
    pub header: NodeHeader,
    pub is_abstract: bool,
    pub definition: Option<DataTypeDefinition>,
    pub purpose: DataTypePurpose,
}

/// What a DataType exists for, which decides whether it belongs in a generated API
/// (UANodeSet `DataTypePurpose`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataTypePurpose {
    #[default]
    Normal,
    ServicesOnly,
    CodeGenerator,
}

impl DataTypePurpose {
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::ServicesOnly => "ServicesOnly",
            Self::CodeGenerator => "CodeGenerator",
        }
    }
}

impl fmt::Display for DataTypePurpose {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for DataTypePurpose {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "Normal" => Ok(Self::Normal),
            "ServicesOnly" => Ok(Self::ServicesOnly),
            "CodeGenerator" => Ok(Self::CodeGenerator),
            _ => Err(ParseError::DataTypePurpose(text.to_owned())),
        }
    }
}

use core::fmt;
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

use crate::error::ParseError;

/// How settled a node's definition is (UANodeSet `ReleaseStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReleaseStatus {
    #[default]
    Released,
    Draft,
    Deprecated,
}

impl ReleaseStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Released => "Released",
            Self::Draft => "Draft",
            Self::Deprecated => "Deprecated",
        }
    }
}

impl fmt::Display for ReleaseStatus {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for ReleaseStatus {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "Released" => Ok(Self::Released),
            "Draft" => Ok(Self::Draft),
            "Deprecated" => Ok(Self::Deprecated),
            _ => Err(ParseError::ReleaseStatus(text.to_owned())),
        }
    }
}

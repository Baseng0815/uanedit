use core::fmt;
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::value_rank::ValueRank;
use crate::error::ParseError;

/// The length of each dimension of an array Value, zero meaning unknown (OPC 10000-3 §5.6.2).
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ArrayDimensions(pub Vec<u32>);

impl ArrayDimensions {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The rank these dimensions describe, which the ValueRank attribute has to agree with.
    pub fn value_rank(&self) -> ValueRank {
        ValueRank::fixed(self.0.len().try_into().unwrap_or(u32::MAX))
    }
}

impl From<Vec<u32>> for ArrayDimensions {
    fn from(dimensions: Vec<u32>) -> Self {
        Self(dimensions)
    }
}

impl fmt::Display for ArrayDimensions {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        for (index, dimension) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            write!(f, "{dimension}")?;
        }
        Ok(())
    }
}

impl FromStr for ArrayDimensions {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Ok(Self::default());
        }
        text.split(',')
            .map(|dimension| {
                dimension
                    .parse()
                    .map_err(|_| ParseError::ArrayDimensions(text.to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }
}

use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

/// How many dimensions a Value has, with negative values naming the special cases
/// (OPC 10000-3 §5.6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ValueRank(pub i32);

impl ValueRank {
    pub const SCALAR_OR_ONE_DIMENSION: Self = Self(-3);
    pub const ANY: Self = Self(-2);
    pub const SCALAR: Self = Self(-1);
    pub const ONE_OR_MORE_DIMENSIONS: Self = Self(0);
    pub const ONE_DIMENSION: Self = Self(1);

    pub fn fixed(dimensions: u32) -> Self {
        Self(dimensions.try_into().unwrap_or(i32::MAX))
    }

    /// The dimension count, when this rank fixes one.
    pub fn dimensions(self) -> Option<u32> {
        self.0.try_into().ok().filter(|dimensions| *dimensions > 0)
    }

    pub fn allows_scalar(self) -> bool {
        matches!(self, Self::SCALAR | Self::ANY | Self::SCALAR_OR_ONE_DIMENSION)
    }

    pub fn allows_array(self) -> bool {
        self != Self::SCALAR
    }

    /// False for the values below -3, which the spec leaves undefined.
    pub fn is_defined(self) -> bool {
        self.0 >= -3
    }
}

impl Default for ValueRank {
    fn default() -> Self {
        Self::SCALAR
    }
}

impl From<i32> for ValueRank {
    fn from(rank: i32) -> Self {
        Self(rank)
    }
}

impl fmt::Display for ValueRank {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match *self {
            Self::SCALAR_OR_ONE_DIMENSION => f.write_str("ScalarOrOneDimension"),
            Self::ANY => f.write_str("Any"),
            Self::SCALAR => f.write_str("Scalar"),
            Self::ONE_OR_MORE_DIMENSIONS => f.write_str("OneOrMoreDimensions"),
            Self::ONE_DIMENSION => f.write_str("OneDimension"),
            Self(rank) => write!(f, "{rank}"),
        }
    }
}

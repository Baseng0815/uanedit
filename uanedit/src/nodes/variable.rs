use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::access_level::AccessLevel;
use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::ids;
use crate::nodes::common::{
    InstanceHeader,
    NodeHeader,
};
use crate::nodes::translation::Translation;
use crate::types::node_id_ref::NodeIdRef;
use crate::types::variant::Variant;

/// A Variable node (OPC 10000-3 §5.6).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Variable {
    pub header: NodeHeader,
    pub instance: InstanceHeader,
    pub value: Option<Variant>,
    pub translations: Vec<Translation>,
    pub data_type: NodeIdRef,
    pub value_rank: ValueRank,
    pub array_dimensions: ArrayDimensions,
    pub access_level: AccessLevel,
    pub user_access_level: AccessLevel,
    #[serde(with = "crate::types::real::double")]
    pub minimum_sampling_interval: f64,
    pub historizing: bool,
}

/// `MinimumSamplingInterval` may be NaN, which does not equal itself; comparing its bits keeps a
/// node that nobody edited comparing equal to the file it came from.
impl PartialEq for Variable {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.minimum_sampling_interval.to_bits() == other.minimum_sampling_interval.to_bits()
            && self.header == other.header
            && self.instance == other.instance
            && self.value == other.value
            && self.translations == other.translations
            && self.data_type == other.data_type
            && self.value_rank == other.value_rank
            && self.array_dimensions == other.array_dimensions
            && self.access_level == other.access_level
            && self.user_access_level == other.user_access_level
            && self.historizing == other.historizing
    }
}

impl Default for Variable {
    /// Matches the UANodeSet schema's attribute defaults, not Rust's zero values.
    fn default() -> Self {
        Self {
            header: NodeHeader::default(),
            instance: InstanceHeader::default(),
            value: None,
            translations: Vec::new(),
            data_type: NodeIdRef::Id(ids::BASE_DATA_TYPE),
            value_rank: ValueRank::SCALAR,
            array_dimensions: ArrayDimensions::default(),
            access_level: AccessLevel::DEFAULT,
            user_access_level: AccessLevel::DEFAULT,
            minimum_sampling_interval: 0.0,
            historizing: false,
        }
    }
}

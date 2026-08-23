use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::ids;
use crate::nodes::common::NodeHeader;
use crate::types::node_id_ref::NodeIdRef;
use crate::types::variant::Variant;

/// A VariableType node (OPC 10000-3 §5.9).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableType {
    pub header: NodeHeader,
    pub is_abstract: bool,
    pub value: Option<Variant>,
    pub data_type: NodeIdRef,
    pub value_rank: ValueRank,
    pub array_dimensions: ArrayDimensions,
}

impl Default for VariableType {
    /// Matches the UANodeSet schema's attribute defaults, not Rust's zero values.
    fn default() -> Self {
        Self {
            header: NodeHeader::default(),
            is_abstract: false,
            value: None,
            data_type: NodeIdRef::Id(ids::BASE_DATA_TYPE),
            value_rank: ValueRank::SCALAR,
            array_dimensions: ArrayDimensions::default(),
        }
    }
}

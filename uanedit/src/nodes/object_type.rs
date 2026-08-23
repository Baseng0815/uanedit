use serde::{
    Deserialize,
    Serialize,
};

use crate::nodes::common::NodeHeader;

/// An ObjectType node (OPC 10000-3 §5.8).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectType {
    pub header: NodeHeader,
    pub is_abstract: bool,
}

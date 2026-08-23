use serde::{
    Deserialize,
    Serialize,
};

use crate::nodes::common::NodeHeader;
use crate::types::localized_text::LocalizedText;

/// A ReferenceType node (OPC 10000-3 §5.3).
///
/// The spec ties the two: a symmetric ReferenceType has no InverseName, and an asymmetric
/// concrete one must have one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceType {
    pub header: NodeHeader,
    pub is_abstract: bool,
    pub symmetric: bool,
    pub inverse_name: Vec<LocalizedText>,
}

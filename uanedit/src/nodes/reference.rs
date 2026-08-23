use serde::{
    Deserialize,
    Serialize,
};

use crate::types::node_id_ref::NodeIdRef;

/// One reference from the node that holds it to `target` (OPC 10000-3 §5.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub reference_type: NodeIdRef,
    /// False when the node holding this reference is the target rather than the source.
    pub is_forward: bool,
    pub target: NodeIdRef,
}

impl Reference {
    pub fn forward(
        reference_type: impl Into<NodeIdRef>,
        target: impl Into<NodeIdRef>,
    ) -> Self {
        Self {
            reference_type: reference_type.into(),
            is_forward: true,
            target: target.into(),
        }
    }

    pub fn inverse(
        reference_type: impl Into<NodeIdRef>,
        target: impl Into<NodeIdRef>,
    ) -> Self {
        Self {
            is_forward: false,
            ..Self::forward(reference_type, target)
        }
    }
}

impl Default for Reference {
    fn default() -> Self {
        Self {
            reference_type: NodeIdRef::default(),
            is_forward: true,
            target: NodeIdRef::default(),
        }
    }
}

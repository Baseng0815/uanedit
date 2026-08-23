use serde::{
    Deserialize,
    Serialize,
};

use crate::nodes::common::{
    InstanceHeader,
    NodeHeader,
};
use crate::types::localized_text::LocalizedText;
use crate::types::node_id_ref::NodeIdRef;

/// A Method node (OPC 10000-3 §5.7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Method {
    pub header: NodeHeader,
    pub instance: InstanceHeader,
    pub argument_descriptions: Vec<MethodArgument>,
    pub executable: bool,
    pub user_executable: bool,
    /// The Method on the type this one instantiates, when the node is an instance declaration.
    pub method_declaration_id: Option<NodeIdRef>,
}

/// Documentation for one argument, held beside the Method rather than in its Arguments Property.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodArgument {
    pub name: Option<String>,
    pub description: Vec<LocalizedText>,
}

impl Default for Method {
    /// Matches the UANodeSet schema's attribute defaults, not Rust's zero values.
    fn default() -> Self {
        Self {
            header: NodeHeader::default(),
            instance: InstanceHeader::default(),
            argument_descriptions: Vec::new(),
            executable: true,
            user_executable: true,
            method_declaration_id: None,
        }
    }
}

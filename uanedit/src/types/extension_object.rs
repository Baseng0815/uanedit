use serde::{
    Deserialize,
    Serialize,
};

use crate::types::node_id::NodeId;
use crate::types::xml::XmlElement;

/// A structured value carried as its DataTypeEncoding NodeId plus a body.
///
/// The body stays as XML because its shape is defined by the model under edit, which this crate
/// cannot decode without the DataType nodes that describe it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionObject {
    pub type_id: NodeId,
    pub body: Option<XmlElement>,
}

impl ExtensionObject {
    pub fn new(
        type_id: NodeId,
        body: XmlElement,
    ) -> Self {
        Self {
            type_id,
            body: Some(body),
        }
    }

    pub fn is_null(&self) -> bool {
        self.type_id.is_null() && self.body.is_none()
    }

    /// True when the body holds a binary-encoded structure rather than an XML-encoded one.
    pub fn is_binary_body(&self) -> bool {
        self.body
            .as_ref()
            .is_some_and(|body| body.local_name() == "ByteString")
    }
}

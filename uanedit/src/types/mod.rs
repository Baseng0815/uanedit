//! The built-in types every attribute and value in a nodeset is denominated in.

pub mod built_in;
pub mod byte_string;
pub mod data_value;
pub mod date_time;
pub mod diagnostic_info;
pub mod expanded_node_id;
pub mod extension_object;
pub mod guid;
pub mod localized_text;
pub mod node_id;
pub mod node_id_ref;
pub mod qualified_name;
pub(crate) mod real;
pub mod status_code;
pub mod variant;
pub mod xml;

pub use built_in::BuiltInType;
pub use byte_string::ByteString;
pub use data_value::DataValue;
pub use date_time::DateTime;
pub use diagnostic_info::DiagnosticInfo;
pub use expanded_node_id::ExpandedNodeId;
pub use extension_object::ExtensionObject;
pub use guid::Guid;
pub use localized_text::LocalizedText;
pub use node_id::{
    Identifier,
    IdentifierKind,
    NamespaceIndex,
    NodeId,
};
pub use node_id_ref::NodeIdRef;
pub use qualified_name::QualifiedName;
pub use status_code::{
    Severity,
    StatusCode,
};
pub use variant::{
    Variant,
    VariantArray,
    VariantMatrix,
};
pub use xml::{
    XmlElement,
    XmlNode,
};

//! The built-in types every attribute and value in a nodeset is denominated in.

pub mod byte_string;
pub mod date_time;
pub mod expanded_node_id;
pub mod guid;
pub mod localized_text;
pub mod node_id;
pub mod qualified_name;
pub mod status_code;
pub mod xml;

pub use byte_string::ByteString;
pub use date_time::DateTime;
pub use expanded_node_id::ExpandedNodeId;
pub use guid::Guid;
pub use localized_text::LocalizedText;
pub use node_id::{
    Identifier,
    IdentifierKind,
    NamespaceIndex,
    NodeId,
};
pub use qualified_name::QualifiedName;
pub use status_code::{
    Severity,
    StatusCode,
};
pub use xml::{
    XmlElement,
    XmlNode,
};

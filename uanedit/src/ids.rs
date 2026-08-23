//! The namespace-0 NodeIds the model itself refers to, and the handful an editor needs constantly.
//!
//! This is deliberately not the whole standard namespace; resolving arbitrary namespace-0 nodes
//! means loading `Opc.Ua.NodeSet2.xml`.

use crate::types::node_id::{
    Identifier,
    NodeId,
};

/// Namespace index 0, which every nodeset shares.
pub const BASE_NAMESPACE_URI: &str = "http://opcfoundation.org/UA/";

const fn base(identifier: u32) -> NodeId {
    NodeId {
        namespace_index: 0,
        identifier: Identifier::Numeric(identifier),
    }
}

pub const BOOLEAN: NodeId = base(1);
pub const STRING: NodeId = base(12);
pub const STRUCTURE: NodeId = base(22);
/// The DataType a Variable falls back to when the attribute is absent.
pub const BASE_DATA_TYPE: NodeId = base(24);
pub const NUMBER: NodeId = base(26);
pub const INTEGER: NodeId = base(27);
pub const UINTEGER: NodeId = base(28);
pub const ENUMERATION: NodeId = base(29);
pub const UNION: NodeId = base(12756);

pub const REFERENCES: NodeId = base(31);
pub const NON_HIERARCHICAL_REFERENCES: NodeId = base(32);
pub const HIERARCHICAL_REFERENCES: NodeId = base(33);
pub const HAS_CHILD: NodeId = base(34);
pub const ORGANIZES: NodeId = base(35);
pub const HAS_EVENT_SOURCE: NodeId = base(36);
pub const HAS_MODELLING_RULE: NodeId = base(37);
pub const HAS_ENCODING: NodeId = base(38);
pub const HAS_DESCRIPTION: NodeId = base(39);
pub const HAS_TYPE_DEFINITION: NodeId = base(40);
pub const GENERATES_EVENT: NodeId = base(41);
pub const AGGREGATES: NodeId = base(44);
pub const HAS_SUBTYPE: NodeId = base(45);
pub const HAS_PROPERTY: NodeId = base(46);
pub const HAS_COMPONENT: NodeId = base(47);
pub const HAS_NOTIFIER: NodeId = base(48);
pub const HAS_ORDERED_COMPONENT: NodeId = base(49);

pub const BASE_OBJECT_TYPE: NodeId = base(58);
pub const FOLDER_TYPE: NodeId = base(61);
pub const BASE_VARIABLE_TYPE: NodeId = base(62);
pub const BASE_DATA_VARIABLE_TYPE: NodeId = base(63);
pub const PROPERTY_TYPE: NodeId = base(68);
pub const BASE_EVENT_TYPE: NodeId = base(2041);

pub const MODELLING_RULE_MANDATORY: NodeId = base(78);
pub const MODELLING_RULE_OPTIONAL: NodeId = base(80);
pub const MODELLING_RULE_EXPOSES_ITS_ARRAY: NodeId = base(83);
pub const MODELLING_RULE_OPTIONAL_PLACEHOLDER: NodeId = base(11508);
pub const MODELLING_RULE_MANDATORY_PLACEHOLDER: NodeId = base(11510);

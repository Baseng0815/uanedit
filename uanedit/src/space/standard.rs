//! The standard ReferenceType hierarchy, built in so the space answers hierarchy questions before
//! `Opc.Ua.NodeSet2.xml` is loaded.
//!
//! The relations are fixed by OPC 10000-3 §7 and OPC 10000-5 §11; a loaded namespace 0 states the
//! same ones and merges with these rather than contradicting them.

use crate::ids;
use crate::types::node_id::NodeId;

/// Each standard ReferenceType with its supertype, from OPC 10000-5 §11.
pub const REFERENCE_TYPE_SUPERTYPES: &[(NodeId, NodeId)] = &[
    (ids::HIERARCHICAL_REFERENCES, ids::REFERENCES),
    (ids::NON_HIERARCHICAL_REFERENCES, ids::REFERENCES),
    (ids::HAS_CHILD, ids::HIERARCHICAL_REFERENCES),
    (ids::ORGANIZES, ids::HIERARCHICAL_REFERENCES),
    (ids::HAS_EVENT_SOURCE, ids::HIERARCHICAL_REFERENCES),
    (ids::AGGREGATES, ids::HAS_CHILD),
    (ids::HAS_SUBTYPE, ids::HAS_CHILD),
    (ids::HAS_PROPERTY, ids::AGGREGATES),
    (ids::HAS_COMPONENT, ids::AGGREGATES),
    (ids::HAS_ORDERED_COMPONENT, ids::HAS_COMPONENT),
    (ids::HAS_NOTIFIER, ids::HAS_EVENT_SOURCE),
    (ids::HAS_MODELLING_RULE, ids::NON_HIERARCHICAL_REFERENCES),
    (ids::HAS_TYPE_DEFINITION, ids::NON_HIERARCHICAL_REFERENCES),
    (ids::HAS_ENCODING, ids::NON_HIERARCHICAL_REFERENCES),
    (ids::GENERATES_EVENT, ids::NON_HIERARCHICAL_REFERENCES),
    (ids::ALWAYS_GENERATES_EVENT, ids::GENERATES_EVENT),
];

/// The standard ReferenceTypes no Reference may name, per OPC 10000-3 §7.2.
pub const ABSTRACT_REFERENCE_TYPES: &[NodeId] = &[
    ids::REFERENCES,
    ids::HIERARCHICAL_REFERENCES,
    ids::NON_HIERARCHICAL_REFERENCES,
    ids::HAS_CHILD,
    ids::AGGREGATES,
];

pub fn is_abstract_reference_type(node_id: &NodeId) -> bool {
    ABSTRACT_REFERENCE_TYPES.contains(node_id)
}

/// Every standard ReferenceType, so a picker has something to offer before namespace 0 is loaded.
pub fn reference_types() -> impl Iterator<Item = NodeId> {
    core::iter::once(ids::REFERENCES).chain(
        REFERENCE_TYPE_SUPERTYPES
            .iter()
            .map(|(subtype, _)| subtype.clone()),
    )
}

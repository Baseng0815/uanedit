//! What the specification allows at each end of a reference.
//!
//! The table holds the standard ReferenceTypes from OPC 10000-3 §7; a user-defined reference type
//! inherits the constraints of its supertypes, which §5.3.3.3 allows it to narrow but not widen.

use crate::ids;
use crate::nodes::node_class::NodeClass;
use crate::rules::node_class_set::NodeClassSet;
use crate::space::{
    AddressSpace,
    standard,
};
use crate::types::node_id::NodeId;

/// A constraint between the two ends that a pair of NodeClass sets cannot state on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    /// HasSubtype: the target is of the same NodeClass as the source (OPC 10000-3 §7.10).
    SameNodeClass,
    /// HasTypeDefinition: an Object takes an ObjectType, a Variable a VariableType (§7.13).
    TypeDefinitionMatchesClass,
    /// HasComponent: only an Object or ObjectType may have an Object or Method component (§7.7).
    ComponentSource,
}

/// The NodeClasses each end of a reference type admits, and any relation between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceConstraint {
    pub source: NodeClassSet,
    pub target: NodeClassSet,
    pub relation: Option<Relation>,
}

impl ReferenceConstraint {
    pub const UNCONSTRAINED: Self = Self {
        source: NodeClassSet::ANY,
        target: NodeClassSet::ANY,
        relation: None,
    };

    fn narrow(
        self,
        other: Self,
    ) -> Self {
        Self {
            source: self.source.intersection(other.source),
            target: self.target.intersection(other.target),
            relation: self.relation.or(other.relation),
        }
    }

    /// Whether the relation between the two ends holds, given their NodeClasses.
    pub fn relation_holds(
        self,
        source: NodeClass,
        target: NodeClass,
    ) -> bool {
        match self.relation {
            None => true,
            Some(Relation::SameNodeClass) => source == target,
            Some(Relation::TypeDefinitionMatchesClass) => match source {
                NodeClass::Object => target == NodeClass::ObjectType,
                NodeClass::Variable => target == NodeClass::VariableType,
                _ => true,
            },
            Some(Relation::ComponentSource) => match target {
                NodeClass::Object | NodeClass::Method => {
                    matches!(source, NodeClass::Object | NodeClass::ObjectType)
                }
                _ => true,
            },
        }
    }
}

const fn constraint(
    source: NodeClassSet,
    target: NodeClassSet,
    relation: Option<Relation>,
) -> ReferenceConstraint {
    ReferenceConstraint {
        source,
        target,
        relation,
    }
}

/// The standard ReferenceTypes that constrain their ends, from OPC 10000-3 §7.
const STANDARD: &[(NodeId, ReferenceConstraint)] = &[
    (
        ids::HAS_TYPE_DEFINITION,
        constraint(
            NodeClassSet::of(&[NodeClass::Object, NodeClass::Variable]),
            NodeClassSet::of(&[NodeClass::ObjectType, NodeClass::VariableType]),
            Some(Relation::TypeDefinitionMatchesClass),
        ),
    ),
    (
        ids::HAS_MODELLING_RULE,
        constraint(
            NodeClassSet::of(&[NodeClass::Object, NodeClass::Variable, NodeClass::Method]),
            NodeClassSet::of(&[NodeClass::Object]),
            None,
        ),
    ),
    (
        ids::HAS_SUBTYPE,
        constraint(NodeClassSet::TYPES, NodeClassSet::TYPES, Some(Relation::SameNodeClass)),
    ),
    (ids::HAS_PROPERTY, constraint(NodeClassSet::ANY, NodeClassSet::of(&[NodeClass::Variable]), None)),
    (
        ids::HAS_COMPONENT,
        constraint(
            NodeClassSet::of(&[
                NodeClass::Object,
                NodeClass::ObjectType,
                NodeClass::Variable,
                NodeClass::VariableType,
            ]),
            NodeClassSet::of(&[NodeClass::Variable, NodeClass::Object, NodeClass::Method]),
            Some(Relation::ComponentSource),
        ),
    ),
    (
        ids::ORGANIZES,
        constraint(
            NodeClassSet::of(&[NodeClass::Object, NodeClass::ObjectType, NodeClass::View]),
            NodeClassSet::ANY,
            None,
        ),
    ),
    (
        ids::HAS_ENCODING,
        constraint(NodeClassSet::of(&[NodeClass::DataType]), NodeClassSet::of(&[NodeClass::Object]), None),
    ),
    (
        ids::HAS_EVENT_SOURCE,
        constraint(
            NodeClassSet::of(&[NodeClass::Object, NodeClass::View, NodeClass::ObjectType]),
            NodeClassSet::ANY,
            None,
        ),
    ),
    // §7.18 constrains only the target of HasNotifier; the source is what §7.17 allows its
    // supertype HasEventSource, which admits an ObjectType referencing an instance declaration.
    (
        ids::HAS_NOTIFIER,
        constraint(
            NodeClassSet::of(&[NodeClass::Object, NodeClass::View, NodeClass::ObjectType]),
            NodeClassSet::of(&[NodeClass::Object]),
            None,
        ),
    ),
    (
        ids::GENERATES_EVENT,
        constraint(
            NodeClassSet::of(&[NodeClass::ObjectType, NodeClass::VariableType, NodeClass::Method]),
            NodeClassSet::of(&[NodeClass::ObjectType]),
            None,
        ),
    ),
    (
        ids::ALWAYS_GENERATES_EVENT,
        constraint(NodeClassSet::of(&[NodeClass::Method]), NodeClassSet::of(&[NodeClass::ObjectType]), None),
    ),
];

/// The constraint a reference type carries, narrowed by every supertype that states one.
pub fn constraint_for(
    space: &AddressSpace,
    reference_type: &NodeId,
) -> ReferenceConstraint {
    let mut found = ReferenceConstraint::UNCONSTRAINED;
    if let Some(own) = standard_constraint(reference_type) {
        found = found.narrow(own);
    }
    for supertype in space.supertype_chain(reference_type) {
        if let Some(inherited) = standard_constraint(&supertype) {
            found = found.narrow(inherited);
        }
    }
    found
}

fn standard_constraint(reference_type: &NodeId) -> Option<ReferenceConstraint> {
    STANDARD
        .iter()
        .find(|(candidate, _)| candidate == reference_type)
        .map(|(_, constraint)| *constraint)
}

/// What a reference type requires of the target's own type, over and above its NodeClass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetType {
    /// The target's type definition shall be this ObjectType or a subtype (OPC 10000-3 §7.14).
    TypeDefinition(NodeId),
    /// The target shall be this type node or a subtype of it (§7.15, §7.16).
    Subtype(NodeId),
}

impl TargetType {
    fn wanted(&self) -> &NodeId {
        match self {
            Self::TypeDefinition(wanted) | Self::Subtype(wanted) => wanted,
        }
    }

    /// False while the nodeset that defines the required type is not loaded, since nothing can be
    /// judged against a type the space does not know.
    pub fn is_checkable(
        &self,
        space: &AddressSpace,
    ) -> bool {
        space.contains(self.wanted())
    }

    pub fn holds(
        &self,
        space: &AddressSpace,
        target: &NodeId,
    ) -> bool {
        match self {
            Self::TypeDefinition(wanted) => space
                .type_definition(target)
                .is_some_and(|found| space.is_same_or_subtype_of(&found, wanted)),
            Self::Subtype(wanted) => space.is_same_or_subtype_of(target, wanted),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::TypeDefinition(wanted) => format!("an Object of {wanted} or a subtype"),
            Self::Subtype(wanted) => format!("{wanted} or a subtype of it"),
        }
    }
}

/// The type constraint a reference type carries, taking it from the supertype that states one.
pub fn target_type_for(
    space: &AddressSpace,
    reference_type: &NodeId,
) -> Option<TargetType> {
    standard_target_type(reference_type).or_else(|| {
        space
            .supertype_chain(reference_type)
            .iter()
            .find_map(standard_target_type)
    })
}

fn standard_target_type(reference_type: &NodeId) -> Option<TargetType> {
    match reference_type {
        candidate if *candidate == ids::HAS_ENCODING => Some(TargetType::TypeDefinition(ids::DATA_TYPE_ENCODING_TYPE)),
        candidate if *candidate == ids::GENERATES_EVENT => Some(TargetType::Subtype(ids::BASE_EVENT_TYPE)),
        _ => None,
    }
}

/// The reference types a node of this NodeClass may be the source of at all.
///
/// OPC 10000-3 §5.3.3.1 for ReferenceType nodes, §5.8.3 for DataType nodes and §5.4 for Views.
/// A reference type no loaded nodeset defines is left alone: where it sits in the hierarchy is
/// unknown, not non-hierarchical.
pub fn source_class_allows(
    space: &AddressSpace,
    source_class: NodeClass,
    reference_type: &NodeId,
) -> bool {
    if !is_known_reference_type(space, reference_type) {
        return true;
    }
    match source_class {
        NodeClass::ReferenceType => is_one_of(space, reference_type, &[ids::HAS_SUBTYPE, ids::HAS_PROPERTY]),
        NodeClass::DataType => {
            is_one_of(space, reference_type, &[ids::HAS_SUBTYPE, ids::HAS_PROPERTY, ids::HAS_ENCODING])
        }
        NodeClass::View => space.is_hierarchical_reference_type(reference_type),
        _ => true,
    }
}

/// True when the space can place the reference type: a loaded node, or one the specification fixes.
fn is_known_reference_type(
    space: &AddressSpace,
    reference_type: &NodeId,
) -> bool {
    space.contains(reference_type) || standard::reference_types().any(|standard| standard == *reference_type)
}

fn is_one_of(
    space: &AddressSpace,
    reference_type: &NodeId,
    allowed: &[NodeId],
) -> bool {
    allowed
        .iter()
        .any(|candidate| space.is_same_or_subtype_of(reference_type, candidate))
}

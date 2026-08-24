//! The queries the pickers and the edit preconditions ask.
//!
//! Both go through the same predicates the rules use, so a choice list the UI offers and a
//! reference the checker would reject can never disagree (guardrails §1.2).

use indexmap::IndexSet;
use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::ids;
use crate::nodes::node_class::NodeClass;
use crate::rules::checks::instances::{
    default_type_definition,
    is_modelling_rule,
    is_property,
};
use crate::rules::checks::references::{
    reference_findings,
    reference_findings_to_class,
};
use crate::rules::code::DiagnosticCode;
use crate::rules::finding::{
    Anchor,
    Finding,
};
use crate::rules::plan;
use crate::space::declarations::ModellingRule;
use crate::space::{
    AddressSpace,
    standard,
};
use crate::types::node_id::NodeId;

/// The answer to an operation-shaped question: the findings the operation would introduce.
///
/// An empty verdict is a yes. A verdict holding only warnings is still a yes, because the
/// specification tolerates them; one holding an error is what the override exists for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub findings: Vec<Finding>,
}

impl Verdict {
    pub fn allowed() -> Self {
        Self::default()
    }

    pub fn is_allowed(&self) -> bool {
        !self.findings.iter().any(Finding::is_error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|finding| finding.is_error())
    }

    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }
}

impl From<Vec<Finding>> for Verdict {
    fn from(findings: Vec<Finding>) -> Self {
        Self { findings }
    }
}

/// Whether node `source` may get a reference of this type to node `target`.
pub fn may_add_reference(
    space: &AddressSpace,
    source: &NodeId,
    reference_type: &NodeId,
    target: &NodeId,
) -> Verdict {
    let mut findings = reference_findings(space, source, reference_type, target);
    let anchor = || Anchor::reference(source.clone(), reference_type.clone(), true, target.clone());
    findings.extend(property_source_finding(space, source, reference_type, anchor()));
    if space.is_has_child_reference_type(reference_type) && space.reaches_via_has_child(target, source) {
        findings.push(Finding::new(
            DiagnosticCode::HasChildCycle,
            anchor(),
            "the reference would close a loop in the HasChild hierarchy",
        ));
    }
    if space
        .forward_references(source)
        .into_iter()
        .any(|view| view.reference_type == *reference_type && view.other == *target)
    {
        findings.push(Finding::new(
            DiagnosticCode::DuplicateReference,
            anchor(),
            "the node already references that node with this reference type",
        ));
    }
    findings.into()
}

/// OPC 10000-3 §5.6.3: a Property is the leaf of every hierarchy.
fn property_source_finding(
    space: &AddressSpace,
    source: &NodeId,
    reference_type: &NodeId,
    anchor: Anchor,
) -> Option<Finding> {
    let offends = is_property(space, source) && space.is_hierarchical_reference_type(reference_type);
    offends.then(|| {
        Finding::new(
            DiagnosticCode::PropertyWithHierarchicalReference,
            anchor,
            "a Property may not be the source of a hierarchical reference",
        )
    })
}

/// The reference types a reference from `source` to `target` may be of.
///
/// The standard types are offered even before namespace 0 is loaded, since the specification fixes
/// them; a loaded namespace 0 only adds their nodes.
pub fn legal_reference_types(
    space: &AddressSpace,
    source: &NodeId,
    target: &NodeId,
) -> Vec<NodeId> {
    reference_types(space)
        .into_iter()
        .filter(|reference_type| may_add_reference(space, source, reference_type, target).is_allowed())
        .collect()
}

/// Every reference type a picker may offer, the standard ones included.
fn reference_types(space: &AddressSpace) -> IndexSet<NodeId> {
    let mut candidates: IndexSet<NodeId> = standard::reference_types().collect();
    candidates.extend(space.nodes_of_class(NodeClass::ReferenceType).cloned());
    candidates
}

/// Whether a new node of this NodeClass may hang under `parent` by a reference of this type.
///
/// What [`may_add_reference`] asks, for a target that does not exist yet: a creation states the
/// reference at the same time as the node, so this is what it can be judged on beforehand.
pub fn may_add_child(
    space: &AddressSpace,
    parent: &NodeId,
    reference_type: &NodeId,
    node_class: NodeClass,
) -> Verdict {
    let mut findings = reference_findings_to_class(space, parent, reference_type, node_class);
    let anchor = Anchor::reference(parent.clone(), reference_type.clone(), true, NodeId::NULL);
    findings.extend(property_source_finding(space, parent, reference_type, anchor));
    findings.into()
}

/// The hierarchical reference types a new node of this class may hang under `parent` by.
pub fn legal_child_reference_types(
    space: &AddressSpace,
    parent: &NodeId,
    node_class: NodeClass,
) -> Vec<NodeId> {
    reference_types(space)
        .into_iter()
        .filter(|reference_type| space.is_hierarchical_reference_type(reference_type))
        .filter(|reference_type| may_add_child(space, parent, reference_type, node_class).is_allowed())
        .collect()
}

/// True for the reference types that make a Variable a Property (OPC 10000-3 §5.6.3).
pub fn is_property_reference(
    space: &AddressSpace,
    reference_type: &NodeId,
) -> bool {
    space.is_same_or_subtype_of(reference_type, &ids::HAS_PROPERTY)
}

/// Whether a new type of this NodeClass may be a subtype of this node.
///
/// OPC 10000-3 §6.3.1 and §7.10: subtyping only relates nodes of the same NodeClass, which is what
/// the HasSubtype reference the creation states is judged by. OPC 10000-5 §7.3 takes PropertyType
/// out: "It is not allowed to subtype this VariableType."
pub fn may_be_supertype(
    space: &AddressSpace,
    node_class: NodeClass,
    supertype: &NodeId,
) -> bool {
    node_class.is_type()
        && *supertype != ids::PROPERTY_TYPE
        && may_add_child(space, supertype, &ids::HAS_SUBTYPE, node_class).is_allowed()
}

/// The nodes a new type of this NodeClass may be created under.
pub fn legal_supertypes(
    space: &AddressSpace,
    node_class: NodeClass,
) -> Vec<NodeId> {
    space
        .node_ids()
        .filter(|node_id| may_be_supertype(space, node_class, node_id))
        .cloned()
        .collect()
}

/// The type itself and every subtype of it that is not abstract, which is what a picker offers.
pub fn concrete_subtypes(
    space: &AddressSpace,
    type_node: &NodeId,
) -> Vec<NodeId> {
    let mut found = Vec::new();
    if space.is_abstract(type_node) == Some(false) {
        found.push(type_node.clone());
    }
    found.extend(
        space
            .all_subtypes(type_node)
            .into_iter()
            .filter(|subtype| space.is_abstract(subtype) != Some(true)),
    );
    found
}

/// Whether an instance of this type may exist at all (OPC 10000-3 §5.5.2, §5.6.5).
pub fn may_instantiate(
    space: &AddressSpace,
    type_node: &NodeId,
) -> Verdict {
    let mut findings = Vec::new();
    if space.is_abstract(type_node) == Some(true) {
        findings.push(Finding::new(
            DiagnosticCode::AbstractTypeDefinition,
            Anchor::node(type_node.clone()),
            format!("{type_node} is abstract, so no instance of it may exist"),
        ));
    }
    findings.into()
}

/// The types an Object or Variable may be given as its type definition.
pub fn legal_type_definitions(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Vec<NodeId> {
    match space.node_class(node_id) {
        Some(NodeClass::Variable) if is_property(space, node_id) => vec![ids::PROPERTY_TYPE],
        Some(node_class) => legal_type_definitions_for(space, node_class, None),
        None => Vec::new(),
    }
}

/// The types a node that does not exist yet may be given as its type definition.
///
/// A Variable reached by a HasProperty reference is a Property, whose type definition is
/// PropertyType and nothing else (OPC 10000-3 §5.6.3).
pub fn legal_type_definitions_for(
    space: &AddressSpace,
    node_class: NodeClass,
    reference_type: Option<&NodeId>,
) -> Vec<NodeId> {
    match node_class {
        NodeClass::Object => concrete_subtypes(space, &ids::BASE_OBJECT_TYPE),
        NodeClass::Variable if is_property_child(space, reference_type) => vec![ids::PROPERTY_TYPE],
        NodeClass::Variable => concrete_subtypes(space, &ids::BASE_VARIABLE_TYPE),
        _ => Vec::new(),
    }
}

/// The type definition a new node of this class gets when the user does not choose one
/// (workflow/type-aware-creation.md §1).
pub fn default_type_definition_for_class(
    space: &AddressSpace,
    node_class: NodeClass,
    reference_type: Option<&NodeId>,
) -> Option<NodeId> {
    match node_class {
        NodeClass::Object => Some(ids::BASE_OBJECT_TYPE),
        NodeClass::Variable => match is_property_child(space, reference_type) {
            true => Some(ids::PROPERTY_TYPE),
            false => Some(ids::BASE_DATA_VARIABLE_TYPE),
        },
        _ => None,
    }
}

fn is_property_child(
    space: &AddressSpace,
    reference_type: Option<&NodeId>,
) -> bool {
    reference_type.is_some_and(|reference_type| is_property_reference(space, reference_type))
}

/// The type definition a bare instance gets when the user does not choose one.
pub fn default_type_definition_for(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Option<NodeId> {
    default_type_definition(space, node_id)
}

/// Whether the node may be given this type definition.
pub fn may_set_type_definition(
    space: &AddressSpace,
    node_id: &NodeId,
    type_definition: &NodeId,
) -> Verdict {
    let mut findings = reference_findings(space, node_id, &ids::HAS_TYPE_DEFINITION, type_definition);
    findings.extend(may_instantiate(space, type_definition).findings);
    if is_property(space, node_id) && *type_definition != ids::PROPERTY_TYPE {
        findings.push(Finding::new(
            DiagnosticCode::PropertyTypeDefinitionNotPropertyType,
            Anchor::node(node_id.clone()),
            "a Property's type definition is PropertyType",
        ));
    }
    findings.into()
}

/// The DataTypes a subtype or an instance may narrow this one to (OPC 10000-3 §6.2.8).
pub fn legal_data_type_narrowings(
    space: &AddressSpace,
    data_type: &NodeId,
) -> Vec<NodeId> {
    let mut found = vec![data_type.clone()];
    found.extend(space.all_subtypes(data_type));
    found
}

/// The Variable or VariableType whose attributes this node's may only restrict
/// (OPC 10000-3 §6.2.8).
pub fn narrowing_source(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Option<NodeId> {
    match space.node_class(node_id)? {
        NodeClass::Variable => space.type_definition(node_id),
        NodeClass::VariableType => space.supertype(node_id),
        _ => None,
    }
}

/// The DataTypes this node's DataType attribute may be set to.
///
/// Everything under the type it narrows from, or under BaseDataType where it narrows nothing.
pub fn legal_data_types_for(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Vec<NodeId> {
    let from = narrowing_source(space, node_id)
        .and_then(|source| space.data_type(&source))
        .unwrap_or(ids::BASE_DATA_TYPE);
    legal_data_type_narrowings(space, &from)
}

/// The ValueRanks this node's ValueRank attribute may be set to, fixed ranks listed up to
/// `max_dimensions`.
pub fn legal_value_ranks_for(
    space: &AddressSpace,
    node_id: &NodeId,
    max_dimensions: u32,
) -> Vec<ValueRank> {
    let from = narrowing_source(space, node_id)
        .and_then(|source| space.node(&source))
        .and_then(plan::value_rank_of)
        .unwrap_or(ValueRank::ANY);
    legal_value_rank_narrowings(from, max_dimensions)
}

pub fn may_set_data_type(
    space: &AddressSpace,
    node_id: &NodeId,
    data_type: &NodeId,
) -> Verdict {
    let mut findings = Vec::new();
    if space
        .node_class(data_type)
        .is_some_and(|class| class != NodeClass::DataType)
    {
        findings.push(Finding::new(
            DiagnosticCode::DataTypeNotADataType,
            Anchor::node(node_id.clone()),
            format!("{data_type} is not a DataType"),
        ));
    }
    if data_type.is_null() {
        findings.push(Finding::new(
            DiagnosticCode::NullDataType,
            Anchor::node(node_id.clone()),
            "the DataType attribute may not be null",
        ));
    }
    findings.into()
}

/// Whether a subtype or an instance may restrict `from` to `to`.
///
/// OPC 10000-3 §6.2.8: Any may become anything, ScalarOrOneDimension may become Scalar or
/// OneDimension, OneOrMoreDimensions may fix a dimension count, and every other rank is frozen.
pub fn may_narrow_value_rank(
    from: ValueRank,
    to: ValueRank,
) -> bool {
    if from == to {
        return true;
    }
    match from {
        ValueRank::ANY => to.is_defined(),
        ValueRank::SCALAR_OR_ONE_DIMENSION => matches!(to, ValueRank::SCALAR | ValueRank::ONE_DIMENSION),
        ValueRank::ONE_OR_MORE_DIMENSIONS => to.dimensions().is_some(),
        _ => false,
    }
}

/// The ranks a picker offers, with fixed ranks listed up to `max_dimensions`.
pub fn legal_value_rank_narrowings(
    from: ValueRank,
    max_dimensions: u32,
) -> Vec<ValueRank> {
    [
        ValueRank::SCALAR_OR_ONE_DIMENSION,
        ValueRank::ANY,
        ValueRank::SCALAR,
        ValueRank::ONE_OR_MORE_DIMENSIONS,
    ]
    .into_iter()
    .chain((1..=max_dimensions).map(ValueRank::fixed))
    .filter(|candidate| may_narrow_value_rank(from, *candidate))
    .collect()
}

/// Whether the new dimensions only replace an unknown length with a fixed one (§6.2.8).
pub fn may_narrow_array_dimensions(
    from: &ArrayDimensions,
    to: &ArrayDimensions,
) -> bool {
    if from.is_empty() {
        return true;
    }
    from.len() == to.len()
        && from
            .0
            .iter()
            .zip(&to.0)
            .all(|(before, after)| *before == *after || *before == 0)
}

/// The modelling rules a subtype may tighten this one to (OPC 10000-3 §6.4.4.2 Table 21).
pub fn legal_modelling_rule_narrowings(rule: Option<ModellingRule>) -> Vec<ModellingRule> {
    match rule {
        Some(rule) => rule.narrowings().to_vec(),
        None => Vec::new(),
    }
}

/// The modelling rules an overriding instance declaration of this class may carry.
///
/// Table 21 aside, a placeholder Method declares nothing but a BrowseName, and the subtype that
/// gives it arguments makes it concrete (OPC 10000-3 §6.4.4.4.4, §6.4.4.4.5).
pub fn legal_override_modelling_rules(
    node_class: NodeClass,
    rule: Option<ModellingRule>,
) -> Vec<ModellingRule> {
    let mut found = legal_modelling_rule_narrowings(rule);
    if node_class != NodeClass::Method {
        return found;
    }
    match rule {
        Some(ModellingRule::OptionalPlaceholder) => {
            found.push(ModellingRule::Optional);
            found.push(ModellingRule::Mandatory);
        }
        Some(ModellingRule::MandatoryPlaceholder) => found.push(ModellingRule::Mandatory),
        _ => {}
    }
    found
}

/// Whether the node may be given this modelling rule (OPC 10000-3 §7.12).
pub fn may_set_modelling_rule(
    space: &AddressSpace,
    node_id: &NodeId,
    modelling_rule: &NodeId,
) -> Verdict {
    let mut findings = Vec::new();
    let carries_rules = space
        .node_class(node_id)
        .is_none_or(|class| matches!(class, NodeClass::Object | NodeClass::Variable | NodeClass::Method));
    if !carries_rules {
        findings.push(Finding::new(
            DiagnosticCode::ModellingRuleOnWrongNodeClass,
            Anchor::node(node_id.clone()),
            "only an Object, a Variable or a Method may carry a ModellingRule",
        ));
    }
    if !is_modelling_rule(space, modelling_rule) {
        findings.push(Finding::new(
            DiagnosticCode::ModellingRuleTargetInvalid,
            Anchor::node(node_id.clone()),
            format!("{modelling_rule} is not an Object of ModellingRuleType"),
        ));
    }
    findings.into()
}

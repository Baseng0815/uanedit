use indexmap::IndexMap;

use crate::ids;
use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::rules::code::{
    DiagnosticCode,
    Severity,
};
use crate::rules::finding::{
    Anchor,
    Finding,
};
use crate::rules::fix::Fix;
use crate::rules::rule::{
    Impact,
    Rule,
    impact_node_and_neighbours,
};
use crate::space::AddressSpace;
use crate::space::browse_path::BrowsePath;
use crate::space::declarations::{
    InstanceDeclaration,
    ModellingRule,
};
use crate::space::delta::Delta;
use crate::types::node_id::NodeId;

/// The type definition a bare instance should get, per workflow/type-aware-creation.md §1.
pub fn default_type_definition(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Option<NodeId> {
    match space.node_class(node_id)? {
        NodeClass::Object => Some(ids::BASE_OBJECT_TYPE),
        NodeClass::Variable => match is_property(space, node_id) {
            true => Some(ids::PROPERTY_TYPE),
            false => Some(ids::BASE_DATA_VARIABLE_TYPE),
        },
        _ => None,
    }
}

/// A Variable is a Property exactly when some node reaches it with a HasProperty reference
/// (OPC 10000-3 §5.6.3).
pub fn is_property(
    space: &AddressSpace,
    node_id: &NodeId,
) -> bool {
    space.node_class(node_id) == Some(NodeClass::Variable)
        && !space
            .sources_of_type(node_id, &ids::HAS_PROPERTY, true)
            .is_empty()
}

/// UA0301, UA0302, UA0303 and UA0306 — what an instance's HasTypeDefinition has to be.
pub struct TypeDefinition {
    code: DiagnosticCode,
}

impl TypeDefinition {
    pub const fn new(code: DiagnosticCode) -> Self {
        Self { code }
    }

    pub const CODES: &'static [DiagnosticCode] = &[
        DiagnosticCode::MissingTypeDefinition,
        DiagnosticCode::MultipleTypeDefinitions,
        DiagnosticCode::AbstractTypeDefinition,
        DiagnosticCode::PropertyTypeDefinitionNotPropertyType,
    ];

    fn findings(
        space: &AddressSpace,
        node_id: &NodeId,
    ) -> Vec<Finding> {
        let mut out = Vec::new();
        let Some(node_class) = space.node_class(node_id) else {
            return out;
        };
        if !matches!(node_class, NodeClass::Object | NodeClass::Variable) {
            return out;
        }
        let found = space.type_definitions(node_id);
        match found.len() {
            0 => out.push(missing(space, node_id, node_class)),
            1 => {}
            count => out.push(
                Finding::new(
                    DiagnosticCode::MultipleTypeDefinitions,
                    Anchor::node(node_id.clone()),
                    format!("the {node_class} has {count} HasTypeDefinition references"),
                )
                .with_fix(Fix::RemoveReference {
                    holder: node_id.clone(),
                    reference_type: ids::HAS_TYPE_DEFINITION,
                    is_forward: true,
                    target: found[1].clone(),
                }),
            ),
        }
        for type_definition in &found {
            out.extend(abstract_finding(space, node_id, type_definition));
            out.extend(property_type_finding(space, node_id, type_definition));
        }
        out
    }
}

fn missing(
    space: &AddressSpace,
    node_id: &NodeId,
    node_class: NodeClass,
) -> Finding {
    let finding = Finding::new(
        DiagnosticCode::MissingTypeDefinition,
        Anchor::node(node_id.clone()),
        format!("the {node_class} has no HasTypeDefinition reference"),
    );
    match default_type_definition(space, node_id) {
        Some(type_definition) => finding.with_fix(Fix::SetTypeDefinition {
            node: node_id.clone(),
            type_definition,
        }),
        None => finding,
    }
}

fn abstract_finding(
    space: &AddressSpace,
    node_id: &NodeId,
    type_definition: &NodeId,
) -> Option<Finding> {
    if space.is_abstract(type_definition) != Some(true) {
        return None;
    }
    // A node carrying a ModellingRule exists to be instantiated rather than to be an instance
    // (OPC 10000-3 §6.2.1, §6.2.2), so its own type may be abstract however the type reaches it.
    if space.modelling_rule(node_id).is_some() {
        return None;
    }
    Some(
        Finding::new(
            DiagnosticCode::AbstractTypeDefinition,
            Anchor::reference(node_id.clone(), ids::HAS_TYPE_DEFINITION, true, type_definition.clone()),
            format!("{type_definition} is abstract, so no instance of it may exist"),
        )
        .with_facts(&type_definition.to_string()),
    )
}

fn property_type_finding(
    space: &AddressSpace,
    node_id: &NodeId,
    type_definition: &NodeId,
) -> Option<Finding> {
    if !is_property(space, node_id) || *type_definition == ids::PROPERTY_TYPE {
        return None;
    }
    Some(
        Finding::new(
            DiagnosticCode::PropertyTypeDefinitionNotPropertyType,
            Anchor::reference(node_id.clone(), ids::HAS_TYPE_DEFINITION, true, type_definition.clone()),
            format!("a Property's type definition is PropertyType, not {type_definition}"),
        )
        .with_facts(&type_definition.to_string())
        .with_fix(Fix::SetTypeDefinition {
            node: node_id.clone(),
            type_definition: ids::PROPERTY_TYPE,
        }),
    )
}

impl Rule for TypeDefinition {
    fn code(&self) -> DiagnosticCode {
        self.code
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        out.extend(
            Self::findings(space, node_id)
                .into_iter()
                .filter(|finding| finding.code == self.code),
        );
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
    }
}

/// UA0305 and UA0311 — the two rules that make a Property a leaf.
pub struct PropertyShape {
    code: DiagnosticCode,
}

impl PropertyShape {
    pub const fn new(code: DiagnosticCode) -> Self {
        Self { code }
    }

    pub const CODES: &'static [DiagnosticCode] = &[
        DiagnosticCode::PropertyWithHierarchicalReference,
        DiagnosticCode::PropertyTargetOfHasComponent,
    ];
}

impl Rule for PropertyShape {
    fn code(&self) -> DiagnosticCode {
        self.code
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        if !is_property(space, node_id) {
            return;
        }
        match self.code {
            DiagnosticCode::PropertyWithHierarchicalReference => {
                for view in space.forward_references(node_id) {
                    if !space.is_hierarchical_reference_type(&view.reference_type) {
                        continue;
                    }
                    out.push(
                        Finding::new(
                            self.code,
                            Anchor::reference(node_id.clone(), view.reference_type.clone(), true, view.other.clone()),
                            format!(
                                "a Property may not be the source of the hierarchical reference {}",
                                view.reference_type
                            ),
                        )
                        .with_fix(Fix::RemoveReference {
                            holder: node_id.clone(),
                            reference_type: view.reference_type,
                            is_forward: true,
                            target: view.other,
                        }),
                    );
                }
            }
            _ => {
                for source in space.sources_of_type(node_id, &ids::HAS_COMPONENT, true) {
                    out.push(
                        Finding::new(
                            self.code,
                            Anchor::reference(node_id.clone(), ids::HAS_COMPONENT, false, source.clone()),
                            format!("{source} references this Property with HasComponent as well"),
                        )
                        .with_facts(&source.to_string())
                        .with_fix(Fix::RemoveReference {
                            holder: source.clone(),
                            reference_type: ids::HAS_COMPONENT,
                            is_forward: true,
                            target: node_id.clone(),
                        }),
                    );
                }
            }
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
    }
}

/// UA0307, UA0308 and UA0310 — what a HasModellingRule reference has to look like.
pub struct ModellingRuleShape {
    code: DiagnosticCode,
}

impl ModellingRuleShape {
    pub const fn new(code: DiagnosticCode) -> Self {
        Self { code }
    }

    pub const CODES: &'static [DiagnosticCode] = &[
        DiagnosticCode::ModellingRuleTargetInvalid,
        DiagnosticCode::ModellingRuleOnWrongNodeClass,
        DiagnosticCode::MultipleModellingRules,
    ];
}

impl Rule for ModellingRuleShape {
    fn code(&self) -> DiagnosticCode {
        self.code
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let found = space.targets_of_type(node_id, &ids::HAS_MODELLING_RULE, true);
        if found.is_empty() {
            return;
        }
        match self.code {
            DiagnosticCode::MultipleModellingRules if found.len() > 1 => out.push(
                Finding::new(
                    self.code,
                    Anchor::node(node_id.clone()),
                    format!("the node has {} HasModellingRule references", found.len()),
                )
                .with_fix(Fix::SetModellingRule {
                    node: node_id.clone(),
                    modelling_rule: Some(found[0].clone()),
                }),
            ),
            DiagnosticCode::ModellingRuleOnWrongNodeClass => {
                let Some(node_class) = space.node_class(node_id) else {
                    return;
                };
                if matches!(node_class, NodeClass::Object | NodeClass::Variable | NodeClass::Method) {
                    return;
                }
                out.push(
                    Finding::new(
                        self.code,
                        Anchor::node(node_id.clone()),
                        format!("a {node_class} may not carry a ModellingRule"),
                    )
                    .with_fix(Fix::SetModellingRule {
                        node: node_id.clone(),
                        modelling_rule: None,
                    }),
                );
            }
            DiagnosticCode::ModellingRuleTargetInvalid => {
                for target in found {
                    if is_modelling_rule(space, &target) {
                        continue;
                    }
                    out.push(
                        Finding::new(
                            self.code,
                            Anchor::reference(node_id.clone(), ids::HAS_MODELLING_RULE, true, target.clone()),
                            format!("{target} is not an Object of ModellingRuleType"),
                        )
                        .with_facts(&target.to_string())
                        .with_fix(Fix::SetModellingRule {
                            node: node_id.clone(),
                            modelling_rule: Some(ids::MODELLING_RULE_OPTIONAL),
                        }),
                    );
                }
            }
            _ => {}
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
    }
}

/// True for one of the standard ModellingRule Objects, or any Object typed by ModellingRuleType.
pub fn is_modelling_rule(
    space: &AddressSpace,
    node_id: &NodeId,
) -> bool {
    if ModellingRule::from_node_id(node_id).is_some() {
        return true;
    }
    if space.node_class(node_id) != Some(NodeClass::Object) {
        return false;
    }
    space
        .type_definition(node_id)
        .is_some_and(|type_definition| space.is_same_or_subtype_of(&type_definition, &ids::MODELLING_RULE_TYPE))
}

/// UA0312 — a type that overrides an inherited declaration without changing anything.
pub struct RedundantDeclarationOverride;

impl Rule for RedundantDeclarationOverride {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::RedundantDeclarationOverride
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        if space
            .node_class(node_id)
            .is_none_or(|class| !matches!(class, NodeClass::ObjectType | NodeClass::VariableType))
        {
            return;
        }
        let Some(supertype) = space.supertype(node_id) else {
            return;
        };
        let inherited = space.instance_declarations(&supertype);
        let own = space.own_instance_declarations(node_id);
        for (path, declaration) in &own {
            let Some(base) = inherited.get(path) else {
                continue;
            };
            if declaration_differs(space, declaration, base) || belongs_to_an_override_chain(&own, path) {
                continue;
            }
            out.push(
                Finding::new(
                    self.code(),
                    Anchor::declaration(node_id.clone(), path.clone()),
                    format!("{path} repeats the declaration {supertype} already makes, unchanged"),
                )
                .with_facts(&declaration.node_id.to_string())
                .with_fix(Fix::RemoveReference {
                    holder: node_id.clone(),
                    reference_type: declaration.reference_type.clone(),
                    is_forward: true,
                    target: declaration.node_id.clone(),
                }),
            );
        }
    }

    fn impact(
        &self,
        _space: &AddressSpace,
        _delta: &Delta,
        impact: &mut Impact,
    ) {
        // The finding sits on the type, however deep in its hierarchy the change was.
        impact.whole_space();
    }
}

/// Whether an overriding declaration says anything its inherited counterpart does not.
fn declaration_differs(
    space: &AddressSpace,
    overriding: &InstanceDeclaration,
    inherited: &InstanceDeclaration,
) -> bool {
    if overriding.node_class != inherited.node_class
        || overriding.type_definition != inherited.type_definition
        || overriding.modelling_rule != inherited.modelling_rule
        || overriding.reference_type != inherited.reference_type
        || space.data_type(&overriding.node_id) != space.data_type(&inherited.node_id)
    {
        return true;
    }
    let (Some(left), Some(right)) = (space.node(&overriding.node_id), space.node(&inherited.node_id)) else {
        return true;
    };
    if left.header().display_name != right.header().display_name
        || left.header().description != right.header().description
    {
        return true;
    }
    match (left, right) {
        (Node::Variable(left), Node::Variable(right)) => {
            left.value != right.value
                || left.value_rank != right.value_rank
                || left.array_dimensions != right.array_dimensions
                || left.access_level != right.access_level
                || left.historizing != right.historizing
        }
        (Node::Method(left), Node::Method(right)) => {
            left.executable != right.executable || left.argument_descriptions != right.argument_descriptions
        }
        (Node::Object(left), Node::Object(right)) => left.event_notifier != right.event_notifier,
        _ => true,
    }
}

/// True when the override is one link of a chain OPC 10000-3 §6.3.3.3 requires.
///
/// §6.3.3.3 only lets a nested declaration be overridden once the declaration above it has been,
/// so an override with one beneath it is the link that one needs. The other way round, an override
/// beneath one hangs under the subtype's new node rather than under the inherited one, so it is
/// not a repetition of anything the supertype still reaches at that path.
fn belongs_to_an_override_chain(
    own: &IndexMap<BrowsePath, InstanceDeclaration>,
    path: &BrowsePath,
) -> bool {
    let anchors = own
        .keys()
        .any(|candidate| candidate != path && candidate.starts_with(path));
    anchors
        || path
            .parent()
            .filter(|parent| !parent.is_empty())
            .is_some_and(|parent| own.contains_key(&parent))
}

/// UA0309 — an instance that does not answer for a Mandatory declaration of its type.
pub struct MissingMandatoryChild;

impl Rule for MissingMandatoryChild {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::MissingMandatoryChild
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        if space
            .node_class(node_id)
            .is_none_or(|class| !class.is_instance())
        {
            return;
        }
        let Some(type_definition) = space.type_definition(node_id) else {
            return;
        };
        for (path, declaration) in space.instance_declarations(&type_definition) {
            if declaration.modelling_rule != Some(ModellingRule::Mandatory) {
                continue;
            }
            if !space.resolve_browse_path(node_id, &path).is_empty() {
                continue;
            }
            // Nothing is owed beneath a declaration the instance does not have: if the optional
            // parent is provided the mandatory child has to be, otherwise not
            // (OPC 10000-3 §6.4.4.4.2).
            if path
                .parent()
                .filter(|parent| !parent.is_empty())
                .is_some_and(|parent| space.resolve_browse_path(node_id, &parent).is_empty())
            {
                continue;
            }
            let is_property = space.is_same_or_subtype_of(&declaration.reference_type, &ids::HAS_PROPERTY);
            let severity = match is_property {
                true => Severity::Error,
                false => Severity::Warning,
            };
            out.push(
                Finding::new(
                    self.code(),
                    Anchor::declaration(node_id.clone(), path.clone()),
                    format!("{type_definition} declares {path} Mandatory and this instance has no such node"),
                )
                .with_severity(severity)
                .with_fix(Fix::MaterializeChild {
                    parent: node_id.clone(),
                    declaration: declaration.node_id.clone(),
                    path,
                    browse_name: declaration.browse_name.clone(),
                    reference_type: declaration.reference_type.clone(),
                }),
            );
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        match delta {
            Delta::FieldChanged { .. } => impact_node_and_neighbours(space, delta, impact),
            // A change to a type moves the obligation of every instance of it, and the delta does
            // not say which node is a type.
            _ => impact.whole_space(),
        }
    }
}

/// UA0313 — an instance answers a MandatoryPlaceholder declaration of its type with nothing.
pub struct MissingMandatoryPlaceholderChild;

impl Rule for MissingMandatoryPlaceholderChild {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::MissingMandatoryPlaceholderChild
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        if space
            .node_class(node_id)
            .is_none_or(|class| !class.is_instance())
        {
            return;
        }
        let Some(type_definition) = space.type_definition(node_id) else {
            return;
        };
        for (path, declaration) in space.instance_declarations(&type_definition) {
            if declaration.modelling_rule != Some(ModellingRule::MandatoryPlaceholder) {
                continue;
            }
            let Some(parent) = path.parent() else {
                continue;
            };
            let holders = match parent.is_empty() {
                true => vec![node_id.clone()],
                false => space.resolve_browse_path(node_id, &parent),
            };
            // Nothing is owed beneath a declaration the instance does not have (§6.4.4.4.2).
            if holders.is_empty()
                || holders
                    .iter()
                    .any(|holder| answers_placeholder(space, holder, &declaration))
            {
                continue;
            }
            out.push(
                Finding::new(
                    self.code(),
                    Anchor::declaration(node_id.clone(), path.clone()),
                    format!(
                        "{type_definition} declares {path} MandatoryPlaceholder and this instance has no {} child of {}",
                        declaration.reference_type,
                        declaration
                            .type_definition
                            .as_ref()
                            .map_or_else(|| "any type".to_owned(), NodeId::to_string),
                    ),
                )
                .with_severity(Severity::Warning)
                .with_facts(&declaration.node_id.to_string()),
            );
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        match delta {
            Delta::FieldChanged { .. } => impact_node_and_neighbours(space, delta, impact),
            _ => impact.whole_space(),
        }
    }
}

/// OPC 10000-3 §6.4.4.4.5: one child of the declared type, by the declared reference type, is
/// enough — the BrowseName is explicitly not constrained.
fn answers_placeholder(
    space: &AddressSpace,
    holder: &NodeId,
    declaration: &InstanceDeclaration,
) -> bool {
    space
        .targets_of_type(holder, &declaration.reference_type, true)
        .into_iter()
        .any(|child| match &declaration.type_definition {
            Some(wanted) => space
                .type_definition(&child)
                .is_some_and(|found| space.is_same_or_subtype_of(&found, wanted)),
            None => true,
        })
}

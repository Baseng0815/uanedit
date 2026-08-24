//! The two trees the creation wizards render (workflow/type-aware-creation.md).
//!
//! Both are queries against the engine rather than anything the UI works out for itself, so the
//! choices a wizard offers and the operation that carries them out cannot disagree
//! (general/guardrails.md §1.2).

use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::rules::query::{
    self,
    Verdict,
};
use crate::space::AddressSpace;
use crate::space::browse_path::BrowsePath;
use crate::space::declarations::{
    InstanceDeclaration,
    ModellingRule,
};
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;

/// How many fixed ranks a picker lists for a declaration whose rank may still be fixed.
const OFFERED_DIMENSIONS: u32 = 4;

/// What creating an instance of one type would materialise, and what the user still decides
/// (workflow/type-aware-creation.md §1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstantiationPlan {
    pub type_node: NodeId,
    /// The NodeClass the new root instance has: an ObjectType makes an Object, a VariableType a
    /// Variable.
    pub node_class: NodeClass,
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    /// Why no instance of the type may exist, empty when one may (OPC 10000-3 §5.5.2, §5.6.5).
    pub verdict: Verdict,
    pub children: Vec<PlannedDeclaration>,
}

impl InstantiationPlan {
    pub fn is_allowed(&self) -> bool {
        self.verdict.is_allowed()
    }

    /// Every declaration in the tree, parents before their children.
    pub fn declarations(&self) -> Vec<&PlannedDeclaration> {
        let mut found = Vec::new();
        for child in &self.children {
            child.collect(&mut found);
        }
        found
    }

    pub fn at(
        &self,
        path: &BrowsePath,
    ) -> Option<&PlannedDeclaration> {
        self.declarations()
            .into_iter()
            .find(|declaration| declaration.path == *path)
    }
}

/// One declaration the wizard offers, with everything it needs to render and narrow it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedDeclaration {
    pub path: BrowsePath,
    /// The declaration node on the type, which the materialised child is copied from.
    pub node_id: NodeId,
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    pub node_class: NodeClass,
    pub reference_type: NodeId,
    /// The type in the supertype chain the declaration came from.
    pub declared_by: NodeId,
    pub modelling_rule: ModellingRule,
    pub type_definition: Option<NodeId>,
    /// The concrete types the declared one may be narrowed to; an instance shall be of a concrete
    /// type even where the declaration's is abstract (OPC 10000-3 §6.2.1).
    pub type_definition_choices: Vec<NodeId>,
    pub data_type: Option<NodeId>,
    pub value_rank: Option<ValueRank>,
    pub array_dimensions: Option<ArrayDimensions>,
    /// Empty for a placeholder, whose own children take no part in instantiation
    /// (OPC 10000-3 §6.4.4.4.4).
    pub children: Vec<PlannedDeclaration>,
}

impl PlannedDeclaration {
    /// True for the declarations the wizard pre-checks and does not let the user clear.
    pub fn is_locked(&self) -> bool {
        self.modelling_rule == ModellingRule::Mandatory
    }

    /// True for the two rules the user answers by naming children instead of ticking a box.
    pub fn is_placeholder(&self) -> bool {
        self.modelling_rule.is_placeholder()
    }

    /// True when the instance owes the declaration at least one child.
    pub fn is_required(&self) -> bool {
        self.modelling_rule.is_required()
    }

    /// The BrowseName to pre-fill a placeholder child with: the declaration's, without the angle
    /// brackets the specification recommends around a placeholder name (OPC 10000-3 §6.4.4.4.4).
    pub fn suggested_browse_name(&self) -> QualifiedName {
        QualifiedName {
            name: strip_brackets(&self.browse_name.name).to_owned(),
            ..self.browse_name.clone()
        }
    }

    fn collect<'a>(
        &'a self,
        found: &mut Vec<&'a Self>,
    ) {
        found.push(self);
        for child in &self.children {
            child.collect(found);
        }
    }
}

/// The name without the angle brackets a placeholder declaration wraps it in.
pub fn strip_brackets(name: &str) -> &str {
    name.strip_prefix('<')
        .and_then(|rest| rest.strip_suffix('>'))
        .unwrap_or(name)
}

/// True for a name that still reads as a placeholder rather than as a child's own.
pub fn is_placeholder_name(name: &str) -> bool {
    name.contains('<') || name.contains('>')
}

/// The tree the create-instance wizard renders for one type.
///
/// ExposesItsArray declarations and declarations carrying no standard ModellingRule are left out,
/// together with everything beneath them: neither is instantiated.
pub fn instantiation_plan(
    space: &AddressSpace,
    type_node: &NodeId,
) -> InstantiationPlan {
    let declarations = space.instance_declarations(type_node);
    InstantiationPlan {
        node_class: instance_class(space, type_node),
        browse_name: space.browse_name(type_node).unwrap_or_default(),
        display_name: display_name(space, type_node),
        verdict: query::may_instantiate(space, type_node),
        children: nest(planned(space, &declarations)),
        type_node: type_node.clone(),
    }
}

/// The NodeClass an instance of this type has.
pub fn instance_class(
    space: &AddressSpace,
    type_node: &NodeId,
) -> NodeClass {
    match space.node_class(type_node) {
        Some(NodeClass::ObjectType) => NodeClass::Object,
        Some(NodeClass::VariableType) => NodeClass::Variable,
        _ => NodeClass::Unspecified,
    }
}

fn planned(
    space: &AddressSpace,
    declarations: &IndexMap<BrowsePath, InstanceDeclaration>,
) -> IndexMap<BrowsePath, PlannedDeclaration> {
    let mut kept: IndexMap<BrowsePath, PlannedDeclaration> = IndexMap::new();
    for (path, declaration) in declarations {
        let Some(modelling_rule) = declaration.modelling_rule else {
            continue;
        };
        if modelling_rule == ModellingRule::ExposesItsArray {
            continue;
        }
        if let Some(parent) = path.parent()
            && !parent.is_empty()
            && !kept.contains_key(&parent)
        {
            continue;
        }
        let node = space.node(&declaration.node_id);
        kept.insert(
            path.clone(),
            PlannedDeclaration {
                path: declaration.path.clone(),
                node_id: declaration.node_id.clone(),
                browse_name: declaration.browse_name.clone(),
                display_name: node
                    .map(|node| node.header().display_name.clone())
                    .unwrap_or_default(),
                description: node
                    .map(|node| node.header().description.clone())
                    .unwrap_or_default(),
                node_class: declaration.node_class,
                reference_type: declaration.reference_type.clone(),
                declared_by: declaration.declared_by.clone(),
                modelling_rule,
                type_definition_choices: declaration
                    .type_definition
                    .as_ref()
                    .map(|type_definition| query::concrete_subtypes(space, type_definition))
                    .unwrap_or_default(),
                type_definition: declaration.type_definition.clone(),
                data_type: space.data_type(&declaration.node_id),
                value_rank: node.and_then(value_rank_of),
                array_dimensions: node.and_then(array_dimensions_of),
                children: Vec::new(),
            },
        );
    }
    kept
}

/// Hangs each declaration under the one whose BrowsePath is its parent.
///
/// The collection lists a parent before its children, so walking it backwards always finds the
/// parent still waiting.
fn nest(mut flat: IndexMap<BrowsePath, PlannedDeclaration>) -> Vec<PlannedDeclaration> {
    let mut roots = Vec::new();
    let paths: Vec<BrowsePath> = flat.keys().cloned().collect();
    for path in paths.into_iter().rev() {
        let Some(declaration) = flat.shift_remove(&path) else {
            continue;
        };
        match path.parent().filter(|parent| !parent.is_empty()) {
            Some(parent) => {
                if let Some(holder) = flat.get_mut(&parent) {
                    holder.children.insert(0, declaration);
                }
            }
            None => roots.push(declaration),
        }
    }
    roots.reverse();
    roots
}

/// What a subtype of this type inherits, and what an override of each declaration may legally
/// become (workflow/type-aware-creation.md §2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtypePlan {
    pub supertype: NodeId,
    pub node_class: NodeClass,
    pub browse_name: QualifiedName,
    pub children: Vec<InheritedDeclaration>,
}

impl SubtypePlan {
    /// Every inherited declaration, parents before their children.
    pub fn declarations(&self) -> Vec<&InheritedDeclaration> {
        let mut found = Vec::new();
        for child in &self.children {
            child.collect(&mut found);
        }
        found
    }

    pub fn at(
        &self,
        path: &BrowsePath,
    ) -> Option<&InheritedDeclaration> {
        self.declarations()
            .into_iter()
            .find(|declaration| declaration.path == *path)
    }
}

/// One declaration a subtype inherits, read-only until the user overrides it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InheritedDeclaration {
    pub path: BrowsePath,
    pub node_id: NodeId,
    /// Never editable on an override (OPC 10000-3 §6.2.7).
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    /// Never editable on an override (OPC 10000-3 §6.2.7).
    pub node_class: NodeClass,
    pub reference_type: NodeId,
    pub declared_by: NodeId,
    pub modelling_rule: Option<ModellingRule>,
    /// The rules an override may tighten this one to (OPC 10000-3 §6.4.4.2 Table 21).
    pub modelling_rule_choices: Vec<ModellingRule>,
    pub type_definition: Option<NodeId>,
    /// The types an override may narrow to; a declaration's type may be abstract (§6.2.1), so
    /// these are not filtered down to the concrete ones.
    pub type_definition_choices: Vec<NodeId>,
    pub data_type: Option<NodeId>,
    pub data_type_choices: Vec<NodeId>,
    pub value_rank: Option<ValueRank>,
    pub value_rank_choices: Vec<ValueRank>,
    pub array_dimensions: Option<ArrayDimensions>,
    pub children: Vec<InheritedDeclaration>,
}

impl InheritedDeclaration {
    fn collect<'a>(
        &'a self,
        found: &mut Vec<&'a Self>,
    ) {
        found.push(self);
        for child in &self.children {
            child.collect(found);
        }
    }
}

pub fn subtype_plan(
    space: &AddressSpace,
    supertype: &NodeId,
) -> SubtypePlan {
    let mut flat: IndexMap<BrowsePath, InheritedDeclaration> = IndexMap::new();
    for (path, declaration) in space.instance_declarations(supertype) {
        let node = space.node(&declaration.node_id);
        let data_type = space.data_type(&declaration.node_id);
        let value_rank = node.and_then(value_rank_of);
        flat.insert(
            path,
            InheritedDeclaration {
                path: declaration.path.clone(),
                node_id: declaration.node_id.clone(),
                browse_name: declaration.browse_name.clone(),
                display_name: node
                    .map(|node| node.header().display_name.clone())
                    .unwrap_or_default(),
                description: node
                    .map(|node| node.header().description.clone())
                    .unwrap_or_default(),
                node_class: declaration.node_class,
                reference_type: declaration.reference_type.clone(),
                declared_by: declaration.declared_by.clone(),
                modelling_rule_choices: query::legal_override_modelling_rules(
                    declaration.node_class,
                    declaration.modelling_rule,
                ),
                modelling_rule: declaration.modelling_rule,
                type_definition_choices: declaration
                    .type_definition
                    .as_ref()
                    .map(|type_definition| same_or_subtypes(space, type_definition))
                    .unwrap_or_default(),
                type_definition: declaration.type_definition.clone(),
                data_type_choices: data_type
                    .as_ref()
                    .map(|data_type| query::legal_data_type_narrowings(space, data_type))
                    .unwrap_or_default(),
                data_type,
                value_rank_choices: value_rank
                    .map(|from| query::legal_value_rank_narrowings(from, OFFERED_DIMENSIONS))
                    .unwrap_or_default(),
                value_rank,
                array_dimensions: node.and_then(array_dimensions_of),
                children: Vec::new(),
            },
        );
    }
    SubtypePlan {
        node_class: space.node_class(supertype).unwrap_or_default(),
        browse_name: space.browse_name(supertype).unwrap_or_default(),
        children: nest_inherited(flat),
        supertype: supertype.clone(),
    }
}

fn nest_inherited(mut flat: IndexMap<BrowsePath, InheritedDeclaration>) -> Vec<InheritedDeclaration> {
    let mut roots = Vec::new();
    let paths: Vec<BrowsePath> = flat.keys().cloned().collect();
    for path in paths.into_iter().rev() {
        let Some(declaration) = flat.shift_remove(&path) else {
            continue;
        };
        match path.parent().filter(|parent| !parent.is_empty()) {
            Some(parent) => {
                if let Some(holder) = flat.get_mut(&parent) {
                    holder.children.insert(0, declaration);
                }
            }
            None => roots.push(declaration),
        }
    }
    roots.reverse();
    roots
}

fn same_or_subtypes(
    space: &AddressSpace,
    type_node: &NodeId,
) -> Vec<NodeId> {
    let mut found = vec![type_node.clone()];
    found.extend(space.all_subtypes(type_node));
    found
}

fn display_name(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Vec<LocalizedText> {
    space
        .node(node_id)
        .map(|node| node.header().display_name.clone())
        .unwrap_or_default()
}

pub(crate) fn value_rank_of(node: &Node) -> Option<ValueRank> {
    match node {
        Node::Variable(variable) => Some(variable.value_rank),
        Node::VariableType(variable_type) => Some(variable_type.value_rank),
        _ => None,
    }
}

pub(crate) fn array_dimensions_of(node: &Node) -> Option<ArrayDimensions> {
    match node {
        Node::Variable(variable) => Some(variable.array_dimensions.clone()),
        Node::VariableType(variable_type) => Some(variable_type.array_dimensions.clone()),
        _ => None,
    }
}

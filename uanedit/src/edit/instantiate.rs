use serde::{
    Deserialize,
    Serialize,
};

use crate::edit::change::Change;
use crate::edit::compile::{
    Compiled,
    Compiler,
};
use crate::edit::create;
use crate::edit::outcome::Refusal;
use crate::edit::reference::{
    self,
    ReferenceKey,
};
use crate::ids;
use crate::nodes::common::{
    InstanceHeader,
    NodeHeader,
};
use crate::nodes::method::Method;
use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::nodes::object::Object;
use crate::nodes::reference::Reference;
use crate::nodes::variable::Variable;
use crate::rules::plan::{
    self,
    InstantiationPlan,
    PlannedDeclaration,
};
use crate::rules::query;
use crate::space::SetId;
use crate::space::browse_path::BrowsePath;
use crate::space::declarations::ModellingRule;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;
use crate::types::variant::Variant;

/// Creating an instance of a type together with the whole hierarchy its declarations ask for
/// (workflow/type-aware-creation.md §1, OPC 10000-3 §6.4.2).
///
/// Everything the wizard collected travels in one operation, so the instance is complete the
/// moment it exists and the undo log holds it as one step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstantiateType {
    pub parent: NodeId,
    /// The hierarchical reference type linking the new instance to its parent.
    pub reference_type: NodeId,
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    pub type_definition: NodeId,
    /// The modelling rule the new root carries, which only a child of a type needs.
    pub modelling_rule: Option<NodeId>,
    pub selections: Selections,
}

/// What the user answered in the wizard, all of it keyed by BrowsePath because that is what
/// identifies an instance declaration across the supertype chain (OPC 10000-3 §6.2.6).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Selections {
    /// The Optional declarations to materialise; choosing one implies its ancestors.
    pub optionals: Vec<BrowsePath>,
    /// The children the user named for a placeholder declaration.
    pub placeholders: Vec<PlaceholderChild>,
    /// The type definitions the user narrowed a declaration to.
    pub narrowings: Vec<TypeNarrowing>,
}

/// One child standing in for a placeholder declaration, which only the user can name
/// (OPC 10000-3 §6.4.4.4.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaceholderChild {
    /// The placeholder declaration this child answers.
    pub path: BrowsePath,
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    /// The type to give the child, absent to use the declaration's.
    pub type_definition: Option<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeNarrowing {
    pub path: BrowsePath,
    pub type_definition: NodeId,
}

impl Selections {
    /// Whether the declaration is to be materialised, which choosing anything beneath it implies.
    pub fn chose(
        &self,
        path: &BrowsePath,
    ) -> bool {
        self.optionals.iter().any(|chosen| chosen.starts_with(path))
            || self
                .placeholders
                .iter()
                .any(|child| child.path.starts_with(path))
    }

    pub fn narrowing(
        &self,
        path: &BrowsePath,
    ) -> Option<&NodeId> {
        self.narrowings
            .iter()
            .find(|narrowing| narrowing.path == *path)
            .map(|narrowing| &narrowing.type_definition)
    }

    pub fn children_of(
        &self,
        path: &BrowsePath,
    ) -> impl Iterator<Item = &PlaceholderChild> {
        self.placeholders
            .iter()
            .filter(move |child| child.path == *path)
    }

    /// Every declaration the selections name, which the operation checks the plan still holds.
    fn paths(&self) -> impl Iterator<Item = &BrowsePath> {
        self.optionals
            .iter()
            .chain(self.placeholders.iter().map(|child| &child.path))
            .chain(self.narrowings.iter().map(|narrowing| &narrowing.path))
    }
}

pub(crate) fn compile(
    compiler: &mut Compiler<'_>,
    create: &InstantiateType,
) -> Result<Compiled, Refusal> {
    compiler.require_known(&create.parent)?;
    compiler.require_known(&create.type_definition)?;
    let plan = plan::instantiation_plan(compiler.space(), &create.type_definition);
    if plan.node_class == NodeClass::Unspecified {
        return Err(Refusal::NotInstantiable {
            node: create.type_definition.clone(),
        });
    }
    compiler.reject(query::may_instantiate(compiler.space(), &create.type_definition))?;
    require_planned(&plan, &create.selections)?;

    let mut compiled = Compiled::default();
    let root = root(compiler, &mut compiled, create, &plan)?;
    materialize(compiler, &mut compiled, create, &plan.children, &root)?;
    Ok(compiled)
}

/// Refuses a selection naming a declaration the type does not have, rather than ignoring it.
fn require_planned(
    plan: &InstantiationPlan,
    selections: &Selections,
) -> Result<(), Refusal> {
    for path in selections.paths() {
        if plan.at(path).is_none() {
            return Err(Refusal::UnknownDeclaration {
                type_node: plan.type_node.clone(),
                path: Box::new(path.clone()),
            });
        }
    }
    Ok(())
}

fn root(
    compiler: &mut Compiler<'_>,
    compiled: &mut Compiled,
    create: &InstantiateType,
    plan: &InstantiationPlan,
) -> Result<NodeId, Refusal> {
    let node_id = compiler.fresh_node_id()?;
    let link = reference::hierarchical_link(compiler, &create.parent, &create.reference_type, plan.node_class)?;
    let mut references = vec![Reference {
        reference_type: compiler.reference_to(&ids::HAS_TYPE_DEFINITION)?,
        is_forward: true,
        target: compiler.reference_to(&create.type_definition)?,
    }];
    compiled.references.push(ReferenceKey::new(
        node_id.clone(),
        ids::HAS_TYPE_DEFINITION,
        create.type_definition.clone(),
    ));
    if let Some(modelling_rule) = &create.modelling_rule {
        compiler.require_known(modelling_rule)?;
        compiler.reject(query::may_set_modelling_rule(compiler.space(), &node_id, modelling_rule))?;
        references.push(Reference {
            reference_type: compiler.reference_to(&ids::HAS_MODELLING_RULE)?,
            is_forward: true,
            target: compiler.reference_to(modelling_rule)?,
        });
        compiled
            .references
            .push(ReferenceKey::new(node_id.clone(), ids::HAS_MODELLING_RULE, modelling_rule.clone()));
    }
    references.push(link);
    compiled
        .references
        .push(ReferenceKey::new(create.parent.clone(), create.reference_type.clone(), node_id.clone()));

    let mut header =
        create::header(compiler, node_id.clone(), &create.browse_name, &create.display_name, &create.description)?;
    header.references = references;
    let instance = InstanceHeader {
        parent_node_id: Some(compiler.reference_to(&create.parent)?),
        design_tool_only: false,
    };
    let node = match plan.node_class {
        NodeClass::Variable => {
            let mut variable = Variable {
                header,
                instance,
                ..Variable::default()
            };
            if let Some(data_type) = compiler.space().data_type(&create.type_definition) {
                variable.data_type = compiler.reference_to(&data_type)?;
            }
            if let Some(type_node) = compiler.space().node(&create.type_definition) {
                variable.value_rank = plan::value_rank_of(type_node).unwrap_or(variable.value_rank);
                variable.array_dimensions = plan::array_dimensions_of(type_node).unwrap_or_default();
            }
            Node::Variable(variable)
        }
        _ => Node::Object(Object {
            header,
            instance,
            ..Object::default()
        }),
    };
    compiled.changes.push(Change::InsertNode {
        position: usize::MAX,
        node: Box::new(node),
    });
    compiled.created.push(node_id.clone());
    Ok(node_id)
}

/// Walks the plan, materialising what the modelling rules and the user's answers ask for.
fn materialize(
    compiler: &mut Compiler<'_>,
    compiled: &mut Compiled,
    create: &InstantiateType,
    declarations: &[PlannedDeclaration],
    parent: &NodeId,
) -> Result<(), Refusal> {
    for declaration in declarations {
        match declaration.modelling_rule {
            ModellingRule::Mandatory => copy(compiler, compiled, create, declaration, parent, None)?,
            ModellingRule::Optional if create.selections.chose(&declaration.path) => {
                copy(compiler, compiled, create, declaration, parent, None)?;
            }
            ModellingRule::OptionalPlaceholder | ModellingRule::MandatoryPlaceholder => {
                let children: Vec<&PlaceholderChild> = create.selections.children_of(&declaration.path).collect();
                if children.is_empty() && declaration.modelling_rule.is_required() {
                    compiler.forbid(Refusal::PlaceholderRequired {
                        path: Box::new(declaration.path.clone()),
                    })?;
                }
                for child in children {
                    if plan::is_placeholder_name(&child.browse_name.name) {
                        compiler.forbid(Refusal::PlaceholderNameNotConcrete {
                            path: Box::new(declaration.path.clone()),
                            browse_name: child.browse_name.clone(),
                        })?;
                    }
                    copy(compiler, compiled, create, declaration, parent, Some(child))?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Copies one declaration onto the instance, then everything the declaration itself declares.
///
/// A placeholder child stops here: what a placeholder declaration declares takes no part in
/// instantiating the type (OPC 10000-3 §6.4.4.4.4).
fn copy(
    compiler: &mut Compiler<'_>,
    compiled: &mut Compiled,
    create: &InstantiateType,
    declaration: &PlannedDeclaration,
    parent: &NodeId,
    named: Option<&PlaceholderChild>,
) -> Result<(), Refusal> {
    let node_id = compiler.fresh_node_id()?;
    let chosen = match named {
        Some(child) => child.type_definition.as_ref(),
        None => create.selections.narrowing(&declaration.path),
    };
    let type_definition = effective_type(compiler, declaration, chosen)?;

    let mut references = Vec::new();
    if let Some(type_definition) = &type_definition {
        references.push(Reference {
            reference_type: compiler.reference_to(&ids::HAS_TYPE_DEFINITION)?,
            is_forward: true,
            target: compiler.reference_to(type_definition)?,
        });
    }
    // The copy keeps the declaration's ModellingRule, which is what makes the instance usable as
    // an instance declaration in turn (OPC 10000-3 §6.4.4.3).
    references.push(Reference {
        reference_type: compiler.reference_to(&ids::HAS_MODELLING_RULE)?,
        is_forward: true,
        target: compiler.reference_to(&declaration.modelling_rule.node_id())?,
    });
    references.push(Reference {
        reference_type: compiler.reference_to(&declaration.reference_type)?,
        is_forward: false,
        target: compiler.reference_to(parent)?,
    });

    let browse_name = match named {
        Some(child) => &child.browse_name,
        None => &declaration.browse_name,
    };
    let display_name = match named {
        Some(child) if !child.display_name.is_empty() => child.display_name.clone(),
        Some(child) => vec![LocalizedText::new(child.browse_name.name.clone())],
        None => declaration.display_name.clone(),
    };
    let description = match named {
        Some(child) => child.description.clone(),
        None => declaration.description.clone(),
    };
    let mut header = create::header(compiler, node_id.clone(), browse_name, &display_name, &description)?;
    header.references = references;
    let instance = InstanceHeader {
        parent_node_id: Some(compiler.reference_to(parent)?),
        design_tool_only: false,
    };
    let node = copied_node(compiler, declaration, header, instance)?;
    compiled.changes.push(Change::InsertNode {
        position: usize::MAX,
        node: Box::new(node),
    });
    compiled.created.push(node_id.clone());

    if named.is_none() {
        materialize(compiler, compiled, create, &declaration.children, &node_id)?;
    }
    Ok(())
}

/// The type the copy points at: the one the user narrowed to, or the declared one.
fn effective_type(
    compiler: &mut Compiler<'_>,
    declaration: &PlannedDeclaration,
    chosen: Option<&NodeId>,
) -> Result<Option<NodeId>, Refusal> {
    let Some(chosen) = chosen else {
        let Some(declared) = declaration.type_definition.clone() else {
            return Ok(None);
        };
        // Only a concrete type may stand as an instance's, even where the declaration's is
        // abstract (OPC 10000-3 §6.2.1).
        compiler.reject(query::may_instantiate(compiler.space(), &declared))?;
        return Ok(Some(declared));
    };
    compiler.require_known(chosen)?;
    if let Some(declared) = &declaration.type_definition
        && !compiler.space().is_same_or_subtype_of(chosen, declared)
    {
        compiler.forbid(Refusal::TypeNotNarrowed {
            path: Box::new(declaration.path.clone()),
            from: declared.clone(),
            to: chosen.clone(),
        })?;
    }
    compiler.reject(query::may_instantiate(compiler.space(), chosen))?;
    Ok(Some(chosen.clone()))
}

/// The node the copy is, with the declaration's attribute values as its initial ones
/// (OPC 10000-3 §6.4.2).
fn copied_node(
    compiler: &mut Compiler<'_>,
    declaration: &PlannedDeclaration,
    mut header: NodeHeader,
    instance: InstanceHeader,
) -> Result<Node, Refusal> {
    let Some(source) = compiler.space().node(&declaration.node_id).cloned() else {
        return Err(Refusal::UnknownNode {
            node: declaration.node_id.clone(),
        });
    };
    header.write_mask = source.header().write_mask;
    header.user_write_mask = source.header().user_write_mask;
    let node = match &source {
        Node::Variable(variable) => {
            let value = copied_value(compiler, &declaration.node_id, variable.value.clone())?;
            let mut copy = Variable {
                header,
                instance,
                value,
                translations: variable.translations.clone(),
                value_rank: variable.value_rank,
                array_dimensions: variable.array_dimensions.clone(),
                access_level: variable.access_level,
                user_access_level: variable.user_access_level,
                minimum_sampling_interval: variable.minimum_sampling_interval,
                historizing: variable.historizing,
                ..Variable::default()
            };
            if let Some(data_type) = compiler.space().data_type(&declaration.node_id) {
                copy.data_type = compiler.reference_to(&data_type)?;
            }
            Node::Variable(copy)
        }
        Node::Method(method) => Node::Method(Method {
            header,
            instance,
            argument_descriptions: method.argument_descriptions.clone(),
            executable: method.executable,
            user_executable: method.user_executable,
            // The Method on the type this one instantiates (OPC 10000-6 Annex F.9).
            method_declaration_id: Some(compiler.reference_to(&declaration.node_id)?),
        }),
        Node::Object(object) => Node::Object(Object {
            header,
            instance,
            event_notifier: object.event_notifier,
        }),
        _ => {
            return Err(Refusal::NotInstantiable {
                node: declaration.node_id.clone(),
            });
        }
    };
    Ok(node)
}

/// The declaration's Value with every NodeId in it restated the way the file the copy goes into
/// spells it; an ExtensionObject body stays as it was, since only its model can decode it.
fn copied_value(
    compiler: &mut Compiler<'_>,
    declaration: &NodeId,
    value: Option<Variant>,
) -> Result<Option<Variant>, Refusal> {
    let (Some(set), Some(value)) = (compiler.space().set_of(declaration), value) else {
        return Ok(None);
    };
    Ok(Some(restated(compiler, set, value)?))
}

fn restated(
    compiler: &mut Compiler<'_>,
    set: SetId,
    value: Variant,
) -> Result<Variant, Refusal> {
    let restated = match value {
        Variant::NodeId(node_id) => Variant::NodeId(restate(compiler, set, &node_id)?),
        Variant::ExpandedNodeId(mut expanded) => {
            if expanded.namespace_uri.is_none() {
                expanded.node_id = restate(compiler, set, &expanded.node_id)?;
            }
            Variant::ExpandedNodeId(expanded)
        }
        Variant::ExtensionObject(mut object) => {
            object.type_id = restate(compiler, set, &object.type_id)?;
            Variant::ExtensionObject(object)
        }
        Variant::Array(mut array) => {
            array.values = restated_all(compiler, set, array.values)?;
            Variant::Array(array)
        }
        Variant::Matrix(mut matrix) => {
            matrix.elements.values = restated_all(compiler, set, matrix.elements.values)?;
            Variant::Matrix(matrix)
        }
        Variant::Variant(inner) => Variant::Variant(Box::new(restated(compiler, set, *inner)?)),
        Variant::DataValue(mut data_value) => {
            data_value.value = match data_value.value {
                Some(inner) => Some(restated(compiler, set, inner)?),
                None => None,
            };
            Variant::DataValue(data_value)
        }
        other => other,
    };
    Ok(restated)
}

fn restated_all(
    compiler: &mut Compiler<'_>,
    set: SetId,
    values: Vec<Variant>,
) -> Result<Vec<Variant>, Refusal> {
    let mut restated_values = Vec::with_capacity(values.len());
    for value in values {
        restated_values.push(restated(compiler, set, value)?);
    }
    Ok(restated_values)
}

fn restate(
    compiler: &mut Compiler<'_>,
    set: SetId,
    node_id: &NodeId,
) -> Result<NodeId, Refusal> {
    match compiler.space().to_space_node_id(set, node_id) {
        Some(space_id) => compiler.local_node_id(&space_id),
        None => Ok(node_id.clone()),
    }
}

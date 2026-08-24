use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::edit::change::Change;
use crate::edit::compile::{
    Compiled,
    Compiler,
};
use crate::edit::create::{
    self,
    CreateType,
    TypeAttributes,
};
use crate::edit::outcome::Refusal;
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
    InheritedDeclaration,
    SubtypePlan,
};
use crate::rules::query;
use crate::space::browse_path::BrowsePath;
use crate::space::declarations::ModellingRule;
use crate::space::delta::NodeField;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;
use crate::types::variant::{
    Variant,
    VariantArray,
};

/// Creating a subtype together with the inherited declarations it overrides
/// (workflow/type-aware-creation.md §2, OPC 10000-3 §6.3.3.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateSubtype {
    pub supertype: NodeId,
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    pub is_abstract: bool,
    pub attributes: TypeAttributes,
    pub overrides: Vec<DeclarationOverride>,
}

/// What a subtype changes about one declaration it inherits.
///
/// The BrowseName and the NodeClass are absent because an override never changes them
/// (OPC 10000-3 §6.2.7); everything else is absent to mean "as inherited".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DeclarationOverride {
    /// The inherited declaration, which only a BrowsePath identifies (OPC 10000-3 §6.2.6).
    pub path: BrowsePath,
    pub type_definition: Option<NodeId>,
    pub modelling_rule: Option<ModellingRule>,
    pub data_type: Option<NodeId>,
    pub value_rank: Option<ValueRank>,
    pub array_dimensions: Option<ArrayDimensions>,
    pub value: Option<Variant>,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    /// The arguments a Method override defines, which may only append to the inherited ones.
    pub arguments: Option<MethodArguments>,
}

/// The InputArguments and OutputArguments a Method override states, as the Property Values the
/// file writes (OPC 10000-3 §5.7.1).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MethodArguments {
    pub input: Option<Variant>,
    pub output: Option<Variant>,
}

pub(crate) fn compile(
    compiler: &mut Compiler<'_>,
    create: &CreateSubtype,
) -> Result<Compiled, Refusal> {
    compiler.require_known(&create.supertype)?;
    let plan = plan::subtype_plan(compiler.space(), &create.supertype);
    let mut compiled = create::type_node(
        compiler,
        &CreateType {
            supertype: create.supertype.clone(),
            browse_name: create.browse_name.clone(),
            display_name: create.display_name.clone(),
            description: create.description.clone(),
            is_abstract: create.is_abstract,
            attributes: create.attributes.clone(),
        },
    )?;
    let subtype = compiled
        .created
        .first()
        .cloned()
        .ok_or_else(|| Refusal::NotInstantiable {
            node: create.supertype.clone(),
        })?;

    // Shallowest first, because a nested declaration can only be overridden once the one that
    // holds it is (OPC 10000-3 §6.3.3.3).
    let mut ordered: Vec<&DeclarationOverride> = create.overrides.iter().collect();
    ordered.sort_by_key(|over| over.path.len());
    let mut anchors: IndexMap<BrowsePath, NodeId> = IndexMap::new();
    for over in ordered {
        let node_id = override_declaration(compiler, &mut compiled, &plan, over, &subtype, &anchors)?;
        anchors.insert(over.path.clone(), node_id);
    }
    Ok(compiled)
}

fn override_declaration(
    compiler: &mut Compiler<'_>,
    compiled: &mut Compiled,
    plan: &SubtypePlan,
    over: &DeclarationOverride,
    subtype: &NodeId,
    anchors: &IndexMap<BrowsePath, NodeId>,
) -> Result<NodeId, Refusal> {
    let inherited = plan
        .at(&over.path)
        .ok_or_else(|| Refusal::UnknownDeclaration {
            type_node: plan.supertype.clone(),
            path: Box::new(over.path.clone()),
        })?;
    let anchor = match over.path.parent().filter(|parent| !parent.is_empty()) {
        Some(parent) => anchors
            .get(&parent)
            .cloned()
            .ok_or_else(|| Refusal::OverrideParentMissing {
                path: Box::new(over.path.clone()),
                parent: Box::new(parent),
            })?,
        None => subtype.clone(),
    };

    let modelling_rule = tightened_rule(compiler, inherited, over)?;
    let type_definition = narrowed_type(compiler, inherited, over)?;
    let node_id = compiler.fresh_node_id()?;

    let mut references = Vec::new();
    // An override restates both, even where it changed neither (OPC 10000-3 §6.3.3.3).
    if let Some(type_definition) = &type_definition {
        references.push(Reference {
            reference_type: compiler.reference_to(&ids::HAS_TYPE_DEFINITION)?,
            is_forward: true,
            target: compiler.reference_to(type_definition)?,
        });
    }
    if let Some(modelling_rule) = modelling_rule {
        references.push(Reference {
            reference_type: compiler.reference_to(&ids::HAS_MODELLING_RULE)?,
            is_forward: true,
            target: compiler.reference_to(&modelling_rule.node_id())?,
        });
    }
    references.push(Reference {
        reference_type: compiler.reference_to(&inherited.reference_type)?,
        is_forward: false,
        target: compiler.reference_to(&anchor)?,
    });

    let display_name = match over.display_name.is_empty() {
        true => inherited.display_name.clone(),
        false => over.display_name.clone(),
    };
    let description = match over.description.is_empty() {
        true => inherited.description.clone(),
        false => over.description.clone(),
    };
    let mut header = create::header(compiler, node_id.clone(), &inherited.browse_name, &display_name, &description)?;
    header.references = references;
    let instance = InstanceHeader {
        parent_node_id: Some(compiler.reference_to(&anchor)?),
        design_tool_only: false,
    };
    let node = overriding_node(compiler, inherited, over, header, instance)?;
    compiled.changes.push(Change::InsertNode {
        position: usize::MAX,
        node: Box::new(node),
    });
    compiled.created.push(node_id.clone());

    if let Some(arguments) = &over.arguments {
        define_arguments(compiler, compiled, inherited, arguments, &node_id)?;
    }
    Ok(node_id)
}

/// The rule the override carries, refused unless it only tightens the inherited one
/// (OPC 10000-3 §6.4.4.2 Table 21).
fn tightened_rule(
    compiler: &Compiler<'_>,
    inherited: &InheritedDeclaration,
    over: &DeclarationOverride,
) -> Result<Option<ModellingRule>, Refusal> {
    let Some(wanted) = over.modelling_rule else {
        return Ok(inherited.modelling_rule);
    };
    let Some(from) = inherited.modelling_rule else {
        return Ok(Some(wanted));
    };
    if !inherited.modelling_rule_choices.contains(&wanted) {
        compiler.forbid(Refusal::RuleNotTightened {
            path: Box::new(over.path.clone()),
            from: from.name().to_owned(),
            to: wanted.name().to_owned(),
        })?;
    }
    Ok(Some(wanted))
}

/// The type definition the override carries, refused unless it is the inherited one or a subtype
/// of it (OPC 10000-3 §6.3.3.3).
fn narrowed_type(
    compiler: &mut Compiler<'_>,
    inherited: &InheritedDeclaration,
    over: &DeclarationOverride,
) -> Result<Option<NodeId>, Refusal> {
    let Some(wanted) = &over.type_definition else {
        return Ok(inherited.type_definition.clone());
    };
    compiler.require_known(wanted)?;
    if let Some(from) = &inherited.type_definition
        && !compiler.space().is_same_or_subtype_of(wanted, from)
    {
        compiler.forbid(Refusal::TypeNotNarrowed {
            path: Box::new(over.path.clone()),
            from: from.clone(),
            to: wanted.clone(),
        })?;
    }
    Ok(Some(wanted.clone()))
}

fn overriding_node(
    compiler: &mut Compiler<'_>,
    inherited: &InheritedDeclaration,
    over: &DeclarationOverride,
    mut header: NodeHeader,
    instance: InstanceHeader,
) -> Result<Node, Refusal> {
    let source = compiler
        .space()
        .node(&inherited.node_id)
        .cloned()
        .ok_or_else(|| Refusal::UnknownNode {
            node: inherited.node_id.clone(),
        })?;
    header.write_mask = source.header().write_mask;
    header.user_write_mask = source.header().user_write_mask;
    let node = match (&source, inherited.node_class) {
        (Node::Variable(variable), _) => {
            let data_type = narrowed_data_type(compiler, inherited, over)?;
            let value_rank = narrowed_value_rank(compiler, inherited, over)?;
            Node::Variable(Variable {
                header,
                instance,
                value: over.value.clone().or_else(|| variable.value.clone()),
                translations: variable.translations.clone(),
                data_type: compiler.reference_to(&data_type)?,
                value_rank,
                array_dimensions: narrowed_array_dimensions(compiler, inherited, over)?,
                access_level: variable.access_level,
                user_access_level: variable.user_access_level,
                minimum_sampling_interval: variable.minimum_sampling_interval,
                historizing: variable.historizing,
            })
        }
        (Node::Method(method), _) => Node::Method(Method {
            header,
            instance,
            argument_descriptions: method.argument_descriptions.clone(),
            executable: method.executable,
            user_executable: method.user_executable,
            method_declaration_id: None,
        }),
        (Node::Object(object), _) => Node::Object(Object {
            header,
            instance,
            event_notifier: object.event_notifier,
        }),
        _ => {
            return Err(Refusal::NotInstantiable {
                node: inherited.node_id.clone(),
            });
        }
    };
    Ok(node)
}

fn narrowed_data_type(
    compiler: &mut Compiler<'_>,
    inherited: &InheritedDeclaration,
    over: &DeclarationOverride,
) -> Result<NodeId, Refusal> {
    let inherited_type = inherited.data_type.clone().unwrap_or(ids::BASE_DATA_TYPE);
    let Some(wanted) = &over.data_type else {
        return Ok(inherited_type);
    };
    compiler.reject(query::may_set_data_type(compiler.space(), &inherited.node_id, wanted))?;
    if !inherited.data_type_choices.contains(wanted) {
        compiler.forbid(Refusal::NotNarrowed {
            node: inherited.node_id.clone(),
            field: NodeField::DATA_TYPE,
            from: inherited_type.to_string(),
            to: wanted.to_string(),
        })?;
    }
    Ok(wanted.clone())
}

fn narrowed_value_rank(
    compiler: &Compiler<'_>,
    inherited: &InheritedDeclaration,
    over: &DeclarationOverride,
) -> Result<ValueRank, Refusal> {
    let from = inherited.value_rank.unwrap_or_default();
    let Some(wanted) = over.value_rank else {
        return Ok(from);
    };
    if !query::may_narrow_value_rank(from, wanted) {
        compiler.forbid(Refusal::NotNarrowed {
            node: inherited.node_id.clone(),
            field: NodeField::VALUE_RANK,
            from: from.to_string(),
            to: wanted.to_string(),
        })?;
    }
    Ok(wanted)
}

fn narrowed_array_dimensions(
    compiler: &Compiler<'_>,
    inherited: &InheritedDeclaration,
    over: &DeclarationOverride,
) -> Result<ArrayDimensions, Refusal> {
    let from = inherited.array_dimensions.clone().unwrap_or_default();
    let Some(wanted) = &over.array_dimensions else {
        return Ok(from);
    };
    if !query::may_narrow_array_dimensions(&from, wanted) {
        compiler.forbid(Refusal::NotNarrowed {
            node: inherited.node_id.clone(),
            field: NodeField::ARRAY_DIMENSIONS,
            from: from.to_string(),
            to: wanted.to_string(),
        })?;
    }
    Ok(wanted.clone())
}

/// Writes the two argument Properties of an overriding Method, which is how a subtype gives a
/// placeholder Method its signature (OPC 10000-3 §6.4.4.4.4).
fn define_arguments(
    compiler: &mut Compiler<'_>,
    compiled: &mut Compiled,
    inherited: &InheritedDeclaration,
    arguments: &MethodArguments,
    method: &NodeId,
) -> Result<(), Refusal> {
    if inherited.node_class != NodeClass::Method {
        return Err(Refusal::FieldNotOnClass {
            node: inherited.node_id.clone(),
            node_class: inherited.node_class,
            field: NodeField::ArgumentDescriptions,
        });
    }
    for (name, value) in [
        ("InputArguments", &arguments.input),
        ("OutputArguments", &arguments.output),
    ] {
        let Some(value) = value else {
            continue;
        };
        let browse_name = QualifiedName::new(0, name);
        let inherited_value = inherited_argument(compiler, &inherited.node_id, &browse_name);
        if !appends(inherited_value.as_ref(), value) {
            compiler.forbid(Refusal::ArgumentsNotAppended {
                path: Box::new(inherited.path.clone()),
            })?;
        }
        argument_property(compiler, compiled, method, &browse_name, value)?;
    }
    Ok(())
}

fn inherited_argument(
    compiler: &Compiler<'_>,
    method: &NodeId,
    browse_name: &QualifiedName,
) -> Option<Variant> {
    let property = compiler
        .space()
        .children_named(method, browse_name)
        .into_iter()
        .next()?;
    match compiler.space().node(&property)? {
        Node::Variable(variable) => variable.value.clone(),
        _ => None,
    }
}

/// Whether the new argument list keeps every inherited argument, in order, and only adds after
/// them (OPC 10000-3 §6.3.3.3).
fn appends(
    inherited: Option<&Variant>,
    wanted: &Variant,
) -> bool {
    let Some(Variant::Array(inherited)) = inherited else {
        return true;
    };
    let Variant::Array(wanted) = wanted else {
        return inherited.values.is_empty();
    };
    wanted.values.len() >= inherited.values.len() && wanted.values[..inherited.values.len()] == inherited.values[..]
}

fn argument_property(
    compiler: &mut Compiler<'_>,
    compiled: &mut Compiled,
    method: &NodeId,
    browse_name: &QualifiedName,
    value: &Variant,
) -> Result<(), Refusal> {
    let node_id = compiler.fresh_node_id()?;
    let references = vec![
        Reference {
            reference_type: compiler.reference_to(&ids::HAS_TYPE_DEFINITION)?,
            is_forward: true,
            target: compiler.reference_to(&ids::PROPERTY_TYPE)?,
        },
        Reference {
            reference_type: compiler.reference_to(&ids::HAS_MODELLING_RULE)?,
            is_forward: true,
            target: compiler.reference_to(&ids::MODELLING_RULE_MANDATORY)?,
        },
        Reference {
            reference_type: compiler.reference_to(&ids::HAS_PROPERTY)?,
            is_forward: false,
            target: compiler.reference_to(method)?,
        },
    ];
    let display_name = vec![LocalizedText::new(browse_name.name.clone())];
    let mut header = create::header(compiler, node_id.clone(), browse_name, &display_name, &[])?;
    header.references = references;
    let count = match value {
        Variant::Array(VariantArray { values, .. }) => u32::try_from(values.len()).unwrap_or(u32::MAX),
        _ => 0,
    };
    compiled.changes.push(Change::InsertNode {
        position: usize::MAX,
        node: Box::new(Node::Variable(Variable {
            header,
            instance: InstanceHeader {
                parent_node_id: Some(compiler.reference_to(method)?),
                design_tool_only: false,
            },
            value: Some(value.clone()),
            data_type: compiler.reference_to(&ids::ARGUMENT)?,
            value_rank: ValueRank::ONE_DIMENSION,
            array_dimensions: ArrayDimensions(vec![count]),
            ..Variable::default()
        })),
    });
    compiled.created.push(node_id);
    Ok(())
}

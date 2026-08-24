use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::event_notifier::EventNotifier;
use crate::attributes::value_rank::ValueRank;
use crate::edit::change::Change;
use crate::edit::compile::{
    Compiled,
    Compiler,
};
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
use crate::nodes::data_type::{
    DataType,
    DataTypePurpose,
};
use crate::nodes::definition::DataTypeDefinition;
use crate::nodes::method::Method;
use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::nodes::object::Object;
use crate::nodes::object_type::ObjectType;
use crate::nodes::reference::Reference;
use crate::nodes::reference_type::ReferenceType;
use crate::nodes::variable::Variable;
use crate::nodes::variable_type::VariableType;
use crate::nodes::view::View;
use crate::rules::query;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;

/// Creating one instance node, with everything the specification requires of it in one call.
///
/// The NodeId is assigned in the nodeset's own namespace; the hierarchical reference, the type
/// definition and the modelling rule are stated on the new node, so the parent's own element is
/// left untouched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateInstance {
    pub parent: NodeId,
    /// The hierarchical reference type linking the new node to its parent.
    pub reference_type: NodeId,
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    /// The modelling rule the new node carries, which only a child of a type needs.
    pub modelling_rule: Option<NodeId>,
    pub attributes: InstanceAttributes,
}

/// The attributes only one instance class carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InstanceAttributes {
    Object {
        /// Left out, the type definition is BaseObjectType.
        type_definition: Option<NodeId>,
        event_notifier: EventNotifier,
    },
    Variable {
        /// Left out, the type definition is PropertyType under a HasProperty reference and
        /// BaseDataVariableType otherwise (workflow/type-aware-creation.md §1).
        type_definition: Option<NodeId>,
        data_type: NodeId,
        value_rank: ValueRank,
        array_dimensions: ArrayDimensions,
    },
    Method {
        executable: bool,
        user_executable: bool,
    },
    View {
        contains_no_loops: bool,
        event_notifier: EventNotifier,
    },
}

impl InstanceAttributes {
    pub fn node_class(&self) -> NodeClass {
        match self {
            Self::Object { .. } => NodeClass::Object,
            Self::Variable { .. } => NodeClass::Variable,
            Self::Method { .. } => NodeClass::Method,
            Self::View { .. } => NodeClass::View,
        }
    }
}

/// Creating one type node, which a HasSubtype reference from its supertype places in the hierarchy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateType {
    pub supertype: NodeId,
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    pub is_abstract: bool,
    pub attributes: TypeAttributes,
}

/// The attributes only one type class carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TypeAttributes {
    ObjectType,
    VariableType {
        data_type: NodeId,
        value_rank: ValueRank,
        array_dimensions: ArrayDimensions,
    },
    DataType {
        definition: Option<DataTypeDefinition>,
        purpose: DataTypePurpose,
    },
    ReferenceType {
        symmetric: bool,
        inverse_name: Vec<LocalizedText>,
    },
}

pub(crate) fn instance(
    compiler: &mut Compiler<'_>,
    create: &CreateInstance,
) -> Result<Compiled, Refusal> {
    compiler.require_known(&create.parent)?;
    let node_id = compiler.fresh_node_id()?;
    let node_class = create.attributes.node_class();
    let link = reference::hierarchical_link(compiler, &create.parent, &create.reference_type, node_class)?;
    let type_definition = instance_type_definition(compiler, create);

    let mut compiled = Compiled::default();
    let mut references = Vec::new();
    if let Some(type_definition) = &type_definition {
        compiler.reject(query::may_instantiate(compiler.space(), type_definition))?;
        references.push(Reference {
            reference_type: compiler.reference_to(&ids::HAS_TYPE_DEFINITION)?,
            is_forward: true,
            target: compiler.reference_to(type_definition)?,
        });
        compiled
            .references
            .push(ReferenceKey::new(node_id.clone(), ids::HAS_TYPE_DEFINITION, type_definition.clone()));
    }
    if let Some(modelling_rule) = &create.modelling_rule {
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

    let mut header = header(compiler, node_id.clone(), &create.browse_name, &create.display_name, &create.description)?;
    header.references = references;
    let instance = InstanceHeader {
        parent_node_id: Some(compiler.reference_to(&create.parent)?),
        design_tool_only: false,
    };
    let node = instance_node(compiler, &node_id, header, instance, &create.attributes)?;
    compiled.changes.push(Change::InsertNode {
        position: usize::MAX,
        node: Box::new(node),
    });
    compiled.created.push(node_id);
    Ok(compiled)
}

fn instance_node(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    header: NodeHeader,
    instance: InstanceHeader,
    attributes: &InstanceAttributes,
) -> Result<Node, Refusal> {
    let node = match attributes {
        InstanceAttributes::Object { event_notifier, .. } => Node::Object(Object {
            header,
            instance,
            event_notifier: *event_notifier,
        }),
        InstanceAttributes::Variable {
            data_type,
            value_rank,
            array_dimensions,
            ..
        } => {
            compiler.reject(query::may_set_data_type(compiler.space(), node_id, data_type))?;
            Node::Variable(Variable {
                header,
                instance,
                data_type: compiler.reference_to(data_type)?,
                value_rank: *value_rank,
                array_dimensions: array_dimensions.clone(),
                ..Variable::default()
            })
        }
        InstanceAttributes::Method {
            executable,
            user_executable,
        } => Node::Method(Method {
            header,
            instance,
            executable: *executable,
            user_executable: *user_executable,
            ..Method::default()
        }),
        InstanceAttributes::View {
            contains_no_loops,
            event_notifier,
        } => Node::View(View {
            header,
            instance,
            contains_no_loops: *contains_no_loops,
            event_notifier: *event_notifier,
        }),
    };
    Ok(node)
}

/// The type definition the new instance gets, defaulted per workflow/type-aware-creation.md §1.
fn instance_type_definition(
    compiler: &Compiler<'_>,
    create: &CreateInstance,
) -> Option<NodeId> {
    let chosen = match &create.attributes {
        InstanceAttributes::Object { type_definition, .. } | InstanceAttributes::Variable { type_definition, .. } => {
            type_definition.clone()
        }
        InstanceAttributes::Method { .. } | InstanceAttributes::View { .. } => None,
    };
    let default = query::default_type_definition_for_class(
        compiler.space(),
        create.attributes.node_class(),
        Some(&create.reference_type),
    );
    chosen.or(default)
}

pub(crate) fn type_node(
    compiler: &mut Compiler<'_>,
    create: &CreateType,
) -> Result<Compiled, Refusal> {
    compiler.require_known(&create.supertype)?;
    let node_id = compiler.fresh_node_id()?;
    let mut header = header(compiler, node_id.clone(), &create.browse_name, &create.display_name, &create.description)?;
    header.references = vec![Reference {
        reference_type: compiler.reference_to(&ids::HAS_SUBTYPE)?,
        is_forward: false,
        target: compiler.reference_to(&create.supertype)?,
    }];
    let node = type_node_of(compiler, &node_id, header, create)?;

    let mut compiled = Compiled::default();
    compiled
        .references
        .push(ReferenceKey::new(create.supertype.clone(), ids::HAS_SUBTYPE, node_id.clone()));
    compiled.changes.push(Change::InsertNode {
        position: usize::MAX,
        node: Box::new(node),
    });
    compiled.created.push(node_id);
    Ok(compiled)
}

fn type_node_of(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    header: NodeHeader,
    create: &CreateType,
) -> Result<Node, Refusal> {
    let node = match &create.attributes {
        TypeAttributes::ObjectType => Node::ObjectType(ObjectType {
            header,
            is_abstract: create.is_abstract,
        }),
        TypeAttributes::VariableType {
            data_type,
            value_rank,
            array_dimensions,
        } => {
            compiler.reject(query::may_set_data_type(compiler.space(), node_id, data_type))?;
            Node::VariableType(VariableType {
                header,
                is_abstract: create.is_abstract,
                data_type: compiler.reference_to(data_type)?,
                value_rank: *value_rank,
                array_dimensions: array_dimensions.clone(),
                ..VariableType::default()
            })
        }
        TypeAttributes::DataType { definition, purpose } => Node::DataType(DataType {
            header,
            is_abstract: create.is_abstract,
            definition: definition.clone(),
            purpose: *purpose,
        }),
        TypeAttributes::ReferenceType {
            symmetric,
            inverse_name,
        } => Node::ReferenceType(ReferenceType {
            header,
            is_abstract: create.is_abstract,
            symmetric: *symmetric,
            inverse_name: inverse_name.clone(),
        }),
    };
    Ok(node)
}

pub(crate) fn header(
    compiler: &mut Compiler<'_>,
    node_id: NodeId,
    browse_name: &QualifiedName,
    display_name: &[LocalizedText],
    description: &[LocalizedText],
) -> Result<NodeHeader, Refusal> {
    let browse_name = compiler.local_browse_name(browse_name)?;
    let display_name = match display_name.is_empty() {
        true => vec![LocalizedText::new(browse_name.name.clone())],
        false => display_name.to_vec(),
    };
    Ok(NodeHeader {
        display_name,
        description: description.to_vec(),
        ..NodeHeader::new(node_id, browse_name)
    })
}

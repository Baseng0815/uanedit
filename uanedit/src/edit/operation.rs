use serde::{
    Deserialize,
    Serialize,
};

use crate::edit::create::{
    CreateInstance,
    CreateType,
    InstanceAttributes,
    TypeAttributes,
};
use crate::edit::delete::DeleteNode;
use crate::edit::field::FieldValue;
use crate::edit::instantiate::InstantiateType;
use crate::edit::reference::{
    AddReference,
    ReferenceKey,
};
use crate::edit::subtype::CreateSubtype;
use crate::nodeset::models::ModelTableEntry;
use crate::space::AddressSpace;
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};

/// One semantic edit, which is the only way anything changes a nodeset (general/guardrails.md §1.1).
///
/// Every operation is a transaction: it asks for everything the specification requires in one call,
/// it is refused whole if the rules engine would report an error over its result, and it undoes as
/// one step however many nodes and references it touched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    CreateInstance(CreateInstance),
    CreateType(CreateType),
    /// Creates an instance of a type with the hierarchy its declarations ask for
    /// (workflow/type-aware-creation.md §1).
    InstantiateType(Box<InstantiateType>),
    /// Creates a subtype together with the declarations it overrides
    /// (workflow/type-aware-creation.md §2).
    CreateSubtype(Box<CreateSubtype>),
    Delete(DeleteNode),
    AddReference(AddReference),
    RemoveReference(ReferenceKey),
    RetargetReference {
        reference: ReferenceKey,
        target: NodeId,
    },
    /// Points an Object or a Variable at its type, replacing the reference it has.
    SetTypeDefinition {
        node: NodeId,
        type_definition: NodeId,
    },
    SetModellingRule {
        node: NodeId,
        modelling_rule: Option<NodeId>,
    },
    SetField {
        node: NodeId,
        value: FieldValue,
    },
    SetDataType {
        node: NodeId,
        data_type: NodeId,
    },
    SetMethodDeclaration {
        node: NodeId,
        declaration: Option<NodeId>,
    },
    SetParentNodeId {
        node: NodeId,
        parent: Option<NodeId>,
    },
    AddNamespace {
        uri: String,
    },
    RenameNamespace {
        index: NamespaceIndex,
        uri: String,
    },
    RemoveNamespace {
        index: NamespaceIndex,
    },
    SetModelEntry {
        index: usize,
        entry: Box<ModelTableEntry>,
    },
    AddModelEntry {
        entry: Box<ModelTableEntry>,
    },
    RemoveModelEntry {
        index: usize,
    },
    AddAlias {
        alias: String,
        node_id: NodeId,
    },
    RemoveAlias {
        alias: String,
    },
}

impl Operation {
    /// What the undo menu calls this operation.
    pub fn label(
        &self,
        space: &AddressSpace,
    ) -> String {
        match self {
            Self::CreateInstance(create) => {
                format!("Create {} {}", instance_class(&create.attributes), create.browse_name)
            }
            Self::CreateType(create) => format!("Create {} {}", type_class(&create.attributes), create.browse_name),
            Self::InstantiateType(create) => {
                format!("Create {} as {}", create.browse_name, name_of(space, &create.type_definition))
            }
            Self::CreateSubtype(create) => {
                format!("Create {} under {}", create.browse_name, name_of(space, &create.supertype))
            }
            Self::Delete(delete) => format!("Delete {}", name_of(space, &delete.node)),
            Self::AddReference(add) => {
                format!("Add {} reference to {}", name_of(space, &add.reference_type), name_of(space, &add.target))
            }
            Self::RemoveReference(key) => {
                format!("Remove {} reference to {}", name_of(space, &key.reference_type), name_of(space, &key.target))
            }
            Self::RetargetReference { reference, target } => format!(
                "Point the {} reference at {}",
                name_of(space, &reference.reference_type),
                name_of(space, target)
            ),
            Self::SetTypeDefinition { node, .. } => format!("Set the type definition of {}", name_of(space, node)),
            Self::SetModellingRule { node, .. } => format!("Set the modelling rule of {}", name_of(space, node)),
            Self::SetField { node, value } => format!("Set {} of {}", value.field(), name_of(space, node)),
            Self::SetDataType { node, .. } => format!("Set DataType of {}", name_of(space, node)),
            Self::SetMethodDeclaration { node, .. } => {
                format!("Set MethodDeclarationId of {}", name_of(space, node))
            }
            Self::SetParentNodeId { node, .. } => format!("Set ParentNodeId of {}", name_of(space, node)),
            Self::AddNamespace { uri } => format!("Add namespace {uri}"),
            Self::RenameNamespace { index, uri } => format!("Rename namespace {index} to {uri}"),
            Self::RemoveNamespace { index } => format!("Remove namespace {index}"),
            Self::SetModelEntry { entry, .. } => format!("Edit the model {}", entry.model_uri),
            Self::AddModelEntry { entry } => format!("Add the model {}", entry.model_uri),
            Self::RemoveModelEntry { index } => format!("Remove model entry {index}"),
            Self::AddAlias { alias, .. } => format!("Add alias {alias}"),
            Self::RemoveAlias { alias } => format!("Remove alias {alias}"),
        }
    }
}

fn instance_class(attributes: &InstanceAttributes) -> &'static str {
    match attributes {
        InstanceAttributes::Object { .. } => "Object",
        InstanceAttributes::Variable { .. } => "Variable",
        InstanceAttributes::Method { .. } => "Method",
        InstanceAttributes::View { .. } => "View",
    }
}

fn type_class(attributes: &TypeAttributes) -> &'static str {
    match attributes {
        TypeAttributes::ObjectType => "ObjectType",
        TypeAttributes::VariableType { .. } => "VariableType",
        TypeAttributes::DataType { .. } => "DataType",
        TypeAttributes::ReferenceType { .. } => "ReferenceType",
    }
}

fn name_of(
    space: &AddressSpace,
    node_id: &NodeId,
) -> String {
    match space.node(node_id) {
        Some(node) => node.header().label(None).to_owned(),
        None => node_id.to_string(),
    }
}

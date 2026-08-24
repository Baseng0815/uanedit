//! The attributes that name another node by NodeId rather than through a reference.
//!
//! A `DataType`, a `ParentNodeId`, a `MethodDeclarationId`, a RolePermissions grant and the
//! DataType of a DataTypeDefinition field all point at a node, and none of them is an edge of the
//! graph the index holds — so nothing that walks references ever sees them.

use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::attribute_id::AttributeId;
use crate::nodes::node::Node;
use crate::space::address_space::AddressSpace;
use crate::space::delta::NodeField;
use crate::space::set::SetId;
use crate::types::node_id::NodeId;
use crate::types::node_id_ref::NodeIdRef;

/// Which attribute of a node names another node, and which entry of it when the attribute is a list.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttributeSlot {
    DataType,
    ParentNodeId,
    MethodDeclarationId,
    /// The DataType of one field of the node's DataTypeDefinition, named by the field.
    DefinitionField {
        field: String,
    },
    /// The Role one RolePermissions grant names, by its place in the list.
    RolePermission {
        position: usize,
    },
}

impl AttributeSlot {
    pub fn field(&self) -> NodeField {
        match self {
            Self::DataType => NodeField::DATA_TYPE,
            Self::ParentNodeId => NodeField::ParentNodeId,
            Self::MethodDeclarationId => NodeField::MethodDeclarationId,
            Self::DefinitionField { .. } => NodeField::Attribute(AttributeId::DataTypeDefinition),
            Self::RolePermission { .. } => NodeField::Attribute(AttributeId::RolePermissions),
        }
    }

    /// True where the model lets the attribute go absent, so a deletion can answer for it by
    /// dropping it rather than by naming another node.
    pub fn is_clearable(&self) -> bool {
        matches!(self, Self::ParentNodeId | Self::MethodDeclarationId | Self::RolePermission { .. })
    }
}

/// A slot as the walk finds it, borrowing the field name rather than owning one, so the scan that
/// only asks "does this node name that one" allocates nothing.
enum SlotKey<'a> {
    DataType,
    ParentNodeId,
    MethodDeclarationId,
    DefinitionField(&'a str),
    RolePermission(usize),
}

impl SlotKey<'_> {
    fn to_slot(&self) -> AttributeSlot {
        match self {
            Self::DataType => AttributeSlot::DataType,
            Self::ParentNodeId => AttributeSlot::ParentNodeId,
            Self::MethodDeclarationId => AttributeSlot::MethodDeclarationId,
            Self::DefinitionField(field) => AttributeSlot::DefinitionField {
                field: (*field).to_owned(),
            },
            Self::RolePermission(position) => AttributeSlot::RolePermission { position: *position },
        }
    }
}

fn visit_attributes<'a>(
    node: &'a Node,
    mut visit: impl FnMut(SlotKey<'a>, &'a NodeIdRef),
) {
    match node {
        Node::Variable(variable) => visit(SlotKey::DataType, &variable.data_type),
        Node::VariableType(variable_type) => visit(SlotKey::DataType, &variable_type.data_type),
        Node::Method(method) => {
            if let Some(declaration) = &method.method_declaration_id {
                visit(SlotKey::MethodDeclarationId, declaration);
            }
        }
        Node::DataType(data_type) => {
            for field in data_type.definition.iter().flat_map(|it| &it.fields) {
                visit(SlotKey::DefinitionField(&field.name), &field.data_type);
            }
        }
        _ => {}
    }
    if let Some(parent) = node
        .instance()
        .and_then(|instance| instance.parent_node_id.as_ref())
    {
        visit(SlotKey::ParentNodeId, parent);
    }
    for (position, grant) in node.header().role_permissions.iter().enumerate() {
        visit(SlotKey::RolePermission(position), &grant.role_id);
    }
}

impl AddressSpace {
    /// Every attribute of this node that names another node, each resolved through the alias table
    /// of the file that states it and restated in the space's namespace indexes.
    pub fn attribute_uses(
        &self,
        node_id: &NodeId,
    ) -> Vec<(AttributeSlot, NodeId)> {
        let (Some(set), Some(node)) = (self.set_of(node_id), self.node(node_id)) else {
            return Vec::new();
        };
        let Some(node_set) = self.set(set) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        visit_attributes(node, |key, reference| {
            if let Some(target) = node_set
                .resolve(reference)
                .and_then(|local| self.to_space_node_id(set, local))
            {
                found.push((key.to_slot(), target));
            }
        });
        found
    }

    /// The editable nodes whose attributes name this one.
    ///
    /// A scan of the primary nodeset rather than an index: only that set can be edited, and an
    /// index would have to be held true through every field change for the sake of a question the
    /// engine asks once per removed node. The comparison stays in the file's own namespace indexes
    /// so no NodeId is rebuilt per node.
    pub fn attribute_referrers(
        &self,
        names: &NodeId,
    ) -> Vec<NodeId> {
        let (Some(node_set), Some(local)) = (self.set(SetId::PRIMARY), self.to_local_node_id(SetId::PRIMARY, names))
        else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for node in node_set.iter() {
            let mut names_it = false;
            visit_attributes(node, |_, reference| {
                names_it |= node_set.resolve(reference) == Some(&local);
            });
            if names_it && let Some(space_id) = self.to_space_node_id(SetId::PRIMARY, node.node_id()) {
                found.push(space_id);
            }
        }
        found
    }
}

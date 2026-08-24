use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::access_level::AccessLevel;
use crate::attributes::access_restrictions::AccessRestrictions;
use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::attribute_id::AttributeId;
use crate::attributes::event_notifier::EventNotifier;
use crate::attributes::permissions::RolePermission;
use crate::attributes::value_rank::ValueRank;
use crate::attributes::write_mask::WriteMask;
use crate::nodes::data_type::DataTypePurpose;
use crate::nodes::definition::DataTypeDefinition;
use crate::nodes::method::MethodArgument;
use crate::nodes::node::Node;
use crate::nodes::release_status::ReleaseStatus;
use crate::nodes::translation::Translation;
use crate::space::delta::NodeField;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id_ref::NodeIdRef;
use crate::types::qualified_name::QualifiedName;
use crate::types::variant::Variant;

/// A value an edit gives one field of a node; the variant names the field.
///
/// The three fields that name another node — DataType, MethodDeclarationId and ParentNodeId — are
/// operations of their own, because they take a NodeId the editor has to state in the file's own
/// namespace indexes and alias table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FieldValue {
    BrowseName(QualifiedName),
    DisplayName(Vec<LocalizedText>),
    Description(Vec<LocalizedText>),
    InverseName(Vec<LocalizedText>),
    Category(Vec<String>),
    Documentation(Option<String>),
    SymbolicName(Option<String>),
    ReleaseStatus(ReleaseStatus),
    WriteMask(WriteMask),
    UserWriteMask(WriteMask),
    DesignToolOnly(bool),
    EventNotifier(EventNotifier),
    ContainsNoLoops(bool),
    IsAbstract(bool),
    Symmetric(bool),
    Value(Option<Variant>),
    ValueRank(ValueRank),
    ArrayDimensions(ArrayDimensions),
    AccessLevel(AccessLevel),
    UserAccessLevel(AccessLevel),
    MinimumSamplingInterval(#[serde(with = "crate::types::real::double")] f64),
    Historizing(bool),
    Executable(bool),
    UserExecutable(bool),
    Translations(Vec<Translation>),
    ArgumentDescriptions(Vec<MethodArgument>),
    DataTypeDefinition(Option<DataTypeDefinition>),
    DataTypePurpose(DataTypePurpose),
    AccessRestrictions(Option<AccessRestrictions>),
    /// Each grant names a Role; a NodeId in it is one of the address space's, restated on the way
    /// into the file like any other reference.
    RolePermissions(Vec<RolePermission>),
}

impl FieldValue {
    pub fn field(&self) -> NodeField {
        match self {
            Self::BrowseName(_) => NodeField::Attribute(AttributeId::BrowseName),
            Self::DisplayName(_) => NodeField::Attribute(AttributeId::DisplayName),
            Self::Description(_) => NodeField::Attribute(AttributeId::Description),
            Self::InverseName(_) => NodeField::Attribute(AttributeId::InverseName),
            Self::Category(_) => NodeField::Category,
            Self::Documentation(_) => NodeField::Documentation,
            Self::SymbolicName(_) => NodeField::SymbolicName,
            Self::ReleaseStatus(_) => NodeField::ReleaseStatus,
            Self::WriteMask(_) => NodeField::Attribute(AttributeId::WriteMask),
            Self::UserWriteMask(_) => NodeField::Attribute(AttributeId::UserWriteMask),
            Self::DesignToolOnly(_) => NodeField::DesignToolOnly,
            Self::EventNotifier(_) => NodeField::Attribute(AttributeId::EventNotifier),
            Self::ContainsNoLoops(_) => NodeField::Attribute(AttributeId::ContainsNoLoops),
            Self::IsAbstract(_) => NodeField::Attribute(AttributeId::IsAbstract),
            Self::Symmetric(_) => NodeField::Attribute(AttributeId::Symmetric),
            Self::Value(_) => NodeField::Attribute(AttributeId::Value),
            Self::ValueRank(_) => NodeField::Attribute(AttributeId::ValueRank),
            Self::ArrayDimensions(_) => NodeField::Attribute(AttributeId::ArrayDimensions),
            Self::AccessLevel(_) => NodeField::Attribute(AttributeId::AccessLevel),
            Self::UserAccessLevel(_) => NodeField::Attribute(AttributeId::UserAccessLevel),
            Self::MinimumSamplingInterval(_) => NodeField::Attribute(AttributeId::MinimumSamplingInterval),
            Self::Historizing(_) => NodeField::Attribute(AttributeId::Historizing),
            Self::Executable(_) => NodeField::Attribute(AttributeId::Executable),
            Self::UserExecutable(_) => NodeField::Attribute(AttributeId::UserExecutable),
            Self::Translations(_) => NodeField::Translations,
            Self::ArgumentDescriptions(_) => NodeField::ArgumentDescriptions,
            Self::DataTypeDefinition(_) => NodeField::Attribute(AttributeId::DataTypeDefinition),
            Self::DataTypePurpose(_) => NodeField::DataTypePurpose,
            Self::AccessRestrictions(_) => NodeField::Attribute(AttributeId::AccessRestrictions),
            Self::RolePermissions(_) => NodeField::Attribute(AttributeId::RolePermissions),
        }
    }
}

/// A field value as the file states it: NodeIds already in the primary nodeset's own indexes, and
/// aliases as they were written.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StoredValue {
    Plain(FieldValue),
    DataType(NodeIdRef),
    MethodDeclarationId(Option<NodeIdRef>),
    ParentNodeId(Option<NodeIdRef>),
}

impl StoredValue {
    pub(crate) fn field(&self) -> NodeField {
        match self {
            Self::Plain(value) => value.field(),
            Self::DataType(_) => NodeField::DATA_TYPE,
            Self::MethodDeclarationId(_) => NodeField::MethodDeclarationId,
            Self::ParentNodeId(_) => NodeField::ParentNodeId,
        }
    }
}

/// Writes the value into the node, returning what it replaced, or nothing when the node's class
/// has no such field.
pub(crate) fn set(
    value: StoredValue,
    node: &mut Node,
) -> Option<StoredValue> {
    match value {
        StoredValue::Plain(value) => set_plain(value, node).map(StoredValue::Plain),
        StoredValue::DataType(next) => {
            let slot = match node {
                Node::Variable(variable) => &mut variable.data_type,
                Node::VariableType(variable_type) => &mut variable_type.data_type,
                _ => return None,
            };
            Some(StoredValue::DataType(core::mem::replace(slot, next)))
        }
        StoredValue::MethodDeclarationId(next) => match node {
            Node::Method(method) => {
                Some(StoredValue::MethodDeclarationId(core::mem::replace(&mut method.method_declaration_id, next)))
            }
            _ => None,
        },
        StoredValue::ParentNodeId(next) => {
            let instance = node.instance_mut()?;
            Some(StoredValue::ParentNodeId(core::mem::replace(&mut instance.parent_node_id, next)))
        }
    }
}

fn set_plain(
    value: FieldValue,
    node: &mut Node,
) -> Option<FieldValue> {
    match value {
        FieldValue::BrowseName(next) => {
            let slot = &mut node.header_mut().browse_name;
            Some(FieldValue::BrowseName(core::mem::replace(slot, next)))
        }
        FieldValue::DisplayName(next) => {
            let slot = &mut node.header_mut().display_name;
            Some(FieldValue::DisplayName(core::mem::replace(slot, next)))
        }
        FieldValue::Description(next) => {
            let slot = &mut node.header_mut().description;
            Some(FieldValue::Description(core::mem::replace(slot, next)))
        }
        FieldValue::Category(next) => {
            let slot = &mut node.header_mut().category;
            Some(FieldValue::Category(core::mem::replace(slot, next)))
        }
        FieldValue::Documentation(next) => {
            let slot = &mut node.header_mut().documentation;
            Some(FieldValue::Documentation(core::mem::replace(slot, next)))
        }
        FieldValue::SymbolicName(next) => {
            let slot = &mut node.header_mut().symbolic_name;
            Some(FieldValue::SymbolicName(core::mem::replace(slot, next)))
        }
        FieldValue::ReleaseStatus(next) => {
            let slot = &mut node.header_mut().release_status;
            Some(FieldValue::ReleaseStatus(core::mem::replace(slot, next)))
        }
        FieldValue::WriteMask(next) => {
            let slot = &mut node.header_mut().write_mask;
            Some(FieldValue::WriteMask(core::mem::replace(slot, next)))
        }
        FieldValue::UserWriteMask(next) => {
            let slot = &mut node.header_mut().user_write_mask;
            Some(FieldValue::UserWriteMask(core::mem::replace(slot, next)))
        }
        FieldValue::DesignToolOnly(next) => {
            let instance = node.instance_mut()?;
            Some(FieldValue::DesignToolOnly(core::mem::replace(&mut instance.design_tool_only, next)))
        }
        FieldValue::InverseName(next) => match node {
            Node::ReferenceType(reference_type) => {
                Some(FieldValue::InverseName(core::mem::replace(&mut reference_type.inverse_name, next)))
            }
            _ => None,
        },
        FieldValue::EventNotifier(next) => {
            let slot = match node {
                Node::Object(object) => &mut object.event_notifier,
                Node::View(view) => &mut view.event_notifier,
                _ => return None,
            };
            Some(FieldValue::EventNotifier(core::mem::replace(slot, next)))
        }
        FieldValue::ContainsNoLoops(next) => match node {
            Node::View(view) => {
                Some(FieldValue::ContainsNoLoops(core::mem::replace(&mut view.contains_no_loops, next)))
            }
            _ => None,
        },
        FieldValue::IsAbstract(next) => {
            let slot = match node {
                Node::ObjectType(object_type) => &mut object_type.is_abstract,
                Node::VariableType(variable_type) => &mut variable_type.is_abstract,
                Node::DataType(data_type) => &mut data_type.is_abstract,
                Node::ReferenceType(reference_type) => &mut reference_type.is_abstract,
                _ => return None,
            };
            Some(FieldValue::IsAbstract(core::mem::replace(slot, next)))
        }
        FieldValue::Symmetric(next) => match node {
            Node::ReferenceType(reference_type) => {
                Some(FieldValue::Symmetric(core::mem::replace(&mut reference_type.symmetric, next)))
            }
            _ => None,
        },
        FieldValue::Value(next) => {
            let slot = match node {
                Node::Variable(variable) => &mut variable.value,
                Node::VariableType(variable_type) => &mut variable_type.value,
                _ => return None,
            };
            Some(FieldValue::Value(core::mem::replace(slot, next)))
        }
        FieldValue::ValueRank(next) => {
            let slot = match node {
                Node::Variable(variable) => &mut variable.value_rank,
                Node::VariableType(variable_type) => &mut variable_type.value_rank,
                _ => return None,
            };
            Some(FieldValue::ValueRank(core::mem::replace(slot, next)))
        }
        FieldValue::ArrayDimensions(next) => {
            let slot = match node {
                Node::Variable(variable) => &mut variable.array_dimensions,
                Node::VariableType(variable_type) => &mut variable_type.array_dimensions,
                _ => return None,
            };
            Some(FieldValue::ArrayDimensions(core::mem::replace(slot, next)))
        }
        FieldValue::AccessLevel(next) => match node {
            Node::Variable(variable) => {
                Some(FieldValue::AccessLevel(core::mem::replace(&mut variable.access_level, next)))
            }
            _ => None,
        },
        FieldValue::UserAccessLevel(next) => match node {
            Node::Variable(variable) => {
                Some(FieldValue::UserAccessLevel(core::mem::replace(&mut variable.user_access_level, next)))
            }
            _ => None,
        },
        FieldValue::MinimumSamplingInterval(next) => match node {
            Node::Variable(variable) => Some(FieldValue::MinimumSamplingInterval(core::mem::replace(
                &mut variable.minimum_sampling_interval,
                next,
            ))),
            _ => None,
        },
        FieldValue::Historizing(next) => match node {
            Node::Variable(variable) => {
                Some(FieldValue::Historizing(core::mem::replace(&mut variable.historizing, next)))
            }
            _ => None,
        },
        FieldValue::Translations(next) => match node {
            Node::Variable(variable) => {
                Some(FieldValue::Translations(core::mem::replace(&mut variable.translations, next)))
            }
            _ => None,
        },
        FieldValue::Executable(next) => match node {
            Node::Method(method) => Some(FieldValue::Executable(core::mem::replace(&mut method.executable, next))),
            _ => None,
        },
        FieldValue::UserExecutable(next) => match node {
            Node::Method(method) => {
                Some(FieldValue::UserExecutable(core::mem::replace(&mut method.user_executable, next)))
            }
            _ => None,
        },
        FieldValue::ArgumentDescriptions(next) => match node {
            Node::Method(method) => {
                Some(FieldValue::ArgumentDescriptions(core::mem::replace(&mut method.argument_descriptions, next)))
            }
            _ => None,
        },
        FieldValue::DataTypeDefinition(next) => match node {
            Node::DataType(data_type) => {
                Some(FieldValue::DataTypeDefinition(core::mem::replace(&mut data_type.definition, next)))
            }
            _ => None,
        },
        FieldValue::DataTypePurpose(next) => match node {
            Node::DataType(data_type) => {
                Some(FieldValue::DataTypePurpose(core::mem::replace(&mut data_type.purpose, next)))
            }
            _ => None,
        },
        FieldValue::AccessRestrictions(next) => {
            let slot = &mut node.header_mut().access_restrictions;
            Some(FieldValue::AccessRestrictions(core::mem::replace(slot, next)))
        }
        FieldValue::RolePermissions(next) => {
            let slot = &mut node.header_mut().role_permissions;
            Some(FieldValue::RolePermissions(core::mem::replace(slot, next)))
        }
    }
}

use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::attribute_id::AttributeId;
use crate::nodes::reference::Reference;
use crate::space::set::SetId;
use crate::types::node_id::NodeId;

/// One change to an address space, reported after the model has been mutated.
///
/// This is the vocabulary the edit operations speak: the space reindexes from a delta and the rules
/// engine re-checks from one, so neither ever walks the whole graph again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Delta {
    NodeAdded {
        node_id: NodeId,
    },
    NodeRemoved {
        node_id: NodeId,
    },
    FieldChanged {
        node_id: NodeId,
        field: NodeField,
    },
    /// A reference was added to the node whose `References` element holds it, which is the source
    /// for a forward reference and the target for an inverse one.
    ReferenceAdded {
        holder: NodeId,
        reference: Reference,
    },
    ReferenceRemoved {
        holder: NodeId,
        reference: Reference,
    },
    NamespacesChanged,
    ModelsChanged,
    AliasesChanged,
    DependencyAdded {
        set: SetId,
    },
}

impl Delta {
    /// The node the change is about, when it is about one.
    pub fn node_id(&self) -> Option<&NodeId> {
        match self {
            Self::NodeAdded { node_id } | Self::NodeRemoved { node_id } => Some(node_id),
            Self::FieldChanged { node_id, .. } => Some(node_id),
            Self::ReferenceAdded { holder, .. } | Self::ReferenceRemoved { holder, .. } => Some(holder),
            Self::NamespacesChanged | Self::ModelsChanged | Self::AliasesChanged | Self::DependencyAdded { .. } => None,
        }
    }

    /// Whether the change can move a node's edges, which is what forces the space to reindex it.
    pub fn touches_graph(&self) -> bool {
        matches!(
            self,
            Self::NodeAdded { .. }
                | Self::NodeRemoved { .. }
                | Self::ReferenceAdded { .. }
                | Self::ReferenceRemoved { .. }
                | Self::AliasesChanged
                | Self::NamespacesChanged
                | Self::DependencyAdded { .. }
        )
    }
}

/// A field of a node an edit can change: an OPC UA attribute, or one of the design-time fields the
/// file format carries beside them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NodeField {
    Attribute(AttributeId),
    SymbolicName,
    Category,
    Documentation,
    ReleaseStatus,
    DesignToolOnly,
    DataTypePurpose,
    ParentNodeId,
    MethodDeclarationId,
    ArgumentDescriptions,
    Translations,
    Extensions,
}

impl NodeField {
    pub const BROWSE_NAME: Self = Self::Attribute(AttributeId::BrowseName);
    pub const DATA_TYPE: Self = Self::Attribute(AttributeId::DataType);
    pub const INVERSE_NAME: Self = Self::Attribute(AttributeId::InverseName);
    pub const IS_ABSTRACT: Self = Self::Attribute(AttributeId::IsAbstract);
    pub const VALUE_RANK: Self = Self::Attribute(AttributeId::ValueRank);
    pub const ARRAY_DIMENSIONS: Self = Self::Attribute(AttributeId::ArrayDimensions);

    pub fn name(self) -> &'static str {
        match self {
            Self::Attribute(attribute) => attribute.name(),
            Self::SymbolicName => "SymbolicName",
            Self::Category => "Category",
            Self::Documentation => "Documentation",
            Self::ReleaseStatus => "ReleaseStatus",
            Self::DesignToolOnly => "DesignToolOnly",
            Self::DataTypePurpose => "DataTypePurpose",
            Self::ParentNodeId => "ParentNodeId",
            Self::MethodDeclarationId => "MethodDeclarationId",
            Self::ArgumentDescriptions => "ArgumentDescriptions",
            Self::Translations => "Translations",
            Self::Extensions => "Extensions",
        }
    }
}

impl From<AttributeId> for NodeField {
    fn from(attribute: AttributeId) -> Self {
        Self::Attribute(attribute)
    }
}

impl fmt::Display for NodeField {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.name())
    }
}

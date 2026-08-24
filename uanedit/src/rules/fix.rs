use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::space::browse_path::BrowsePath;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use crate::types::qualified_name::QualifiedName;

/// What would make a finding go away, stated so an edit operation can apply it without the user
/// having to work out what the rule meant.
///
/// Every NodeId is in the address space's namespace indexes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fix {
    SetTypeDefinition {
        node: NodeId,
        type_definition: NodeId,
    },
    /// Drops the reference from the node whose `References` element holds it.
    RemoveReference {
        holder: NodeId,
        reference_type: NodeId,
        is_forward: bool,
        target: NodeId,
    },
    SetReferenceType {
        holder: NodeId,
        reference_type: NodeId,
        is_forward: bool,
        target: NodeId,
        new_reference_type: NodeId,
    },
    /// Creates the child an instance owes a Mandatory instance declaration.
    MaterializeChild {
        parent: NodeId,
        declaration: NodeId,
        path: BrowsePath,
        browse_name: QualifiedName,
        reference_type: NodeId,
    },
    SetBrowseName {
        node: NodeId,
        browse_name: QualifiedName,
    },
    SetNodeId {
        node: NodeId,
        new_node_id: NodeId,
    },
    SetDataType {
        node: NodeId,
        data_type: NodeId,
    },
    SetValueRank {
        node: NodeId,
        value_rank: ValueRank,
    },
    SetArrayDimensions {
        node: NodeId,
        array_dimensions: ArrayDimensions,
    },
    SetModellingRule {
        node: NodeId,
        modelling_rule: Option<NodeId>,
    },
    SetInverseName {
        node: NodeId,
        inverse_name: Vec<LocalizedText>,
    },
    SetParentNodeId {
        node: NodeId,
        parent: Option<NodeId>,
    },
    /// Adds the namespace URI a used index has no entry for.
    DeclareNamespace {
        index: NamespaceIndex,
    },
    DefineAlias {
        alias: String,
    },
    /// Loads the nodeset that would define an unresolved target.
    LoadDependency {
        namespace_uri: Option<String>,
        target: NodeId,
    },
}

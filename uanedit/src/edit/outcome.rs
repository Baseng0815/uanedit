use serde::{
    Deserialize,
    Serialize,
};
use thiserror::Error;

use crate::edit::delete::DeletionPlan;
use crate::edit::reference::ReferenceKey;
use crate::nodes::node_class::NodeClass;
use crate::rules::engine::Diagnostics;
use crate::rules::finding::Finding;
use crate::rules::query::Verdict;
use crate::space::browse_path::BrowsePath;
use crate::space::delta::{
    Delta,
    NodeField,
};
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use crate::types::qualified_name::QualifiedName;

/// What an operation did: the changes the address space was told about, and the findings as they
/// stand afterwards.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub label: String,
    pub deltas: Vec<Delta>,
    pub diagnostics: Diagnostics,
    /// The nodes the operation created, in the order it created them.
    pub created: Vec<NodeId>,
    /// True when the operation was performed past a refusal (general/guardrails.md §5).
    pub via_override: bool,
    /// The error findings the override let through, which show as introduced and attributed.
    pub overridden: Vec<Finding>,
    /// Every finding the operation left behind that was not there before it, warnings included —
    /// permitted, but never silent (general/guardrails.md §2).
    pub introduced: Vec<Finding>,
    /// True when the operation changed the acknowledgement set, which the sidecar has to be told.
    pub acknowledgements_changed: bool,
}

impl Applied {
    pub fn is_empty(&self) -> bool {
        self.deltas.is_empty()
    }

    /// The findings the operation introduced that the specification tolerates.
    pub fn introduced_warnings(&self) -> impl Iterator<Item = &Finding> {
        self.introduced.iter().filter(|finding| !finding.is_error())
    }
}

/// Why an operation was not performed.
///
/// [`Refusal::Rejected`] and [`Refusal::Introduces`] carry the engine's own findings, so the UI can
/// show them the way the validation panel shows a finding. The rest state what no rule covers;
/// [`Refusal::is_overridable`] says which of those the override still performs.
#[derive(Clone, Debug, PartialEq, Error, Serialize, Deserialize)]
pub enum Refusal {
    #[error("{node} belongs to a nodeset this editor does not change")]
    ReadOnly { node: NodeId },
    #[error("no loaded nodeset defines {node}")]
    UnknownNode { node: NodeId },
    #[error("a {node_class} has no {field}")]
    FieldNotOnClass {
        node: NodeId,
        node_class: NodeClass,
        field: NodeField,
    },
    #[error("{reference_type} is not a hierarchical reference type")]
    NotHierarchical { reference_type: NodeId },
    #[error("the nodeset declares no namespace of its own to create nodes in")]
    NoOwnNamespace,
    #[error("no loaded nodeset declares namespace index {index}")]
    UnknownNamespace { index: NamespaceIndex },
    #[error("namespace index {namespace} has no numeric identifier left for a new node")]
    NoFreeNodeId { namespace: NamespaceIndex },
    #[error("namespace index {index} is fixed by the specification")]
    NamespaceFixed { index: NamespaceIndex },
    #[error("namespace {uri} at index {index} belongs to a loaded dependency, so this nodeset does not rename it")]
    NamespaceOfDependency { index: NamespaceIndex, uri: String },
    #[error("the nodeset already declares {uri}")]
    DuplicateNamespace { uri: String },
    #[error("{} nodes still use namespace {uri} at index {index}", users.len())]
    NamespaceInUse {
        index: NamespaceIndex,
        uri: String,
        users: Vec<NodeId>,
    },
    #[error("removing {uri} would renumber the namespaces above index {index}, and every NodeId with them")]
    NamespaceRenumbers { index: NamespaceIndex, uri: String },
    #[error("the nodeset has no model entry {index}")]
    UnknownModel { index: usize },
    #[error("the nodeset already declares the model {uri}")]
    DuplicateModel { uri: String },
    #[error("the nodeset already defines the alias `{alias}`")]
    DuplicateAlias { alias: String },
    #[error("the nodeset defines no alias `{alias}`")]
    UnknownAlias { alias: String },
    #[error("{} nodes still name the alias `{alias}`", users.len())]
    AliasInUse { alias: String, users: Vec<NodeId> },
    #[error("no node states this reference")]
    ReferenceNotFound { reference: Box<ReferenceKey> },
    #[error("only a nodeset this editor does not change states this reference")]
    ReferenceReadOnly { reference: Box<ReferenceKey> },
    #[error(
        "the deletion leaves {} references, {} attributes and {} children unanswered",
        plan.incoming.len(),
        plan.attributes.len(),
        plan.children.len()
    )]
    UnresolvedDeletion { plan: Box<DeletionPlan> },
    #[error("{field} of {node} may not be left unset, so the deletion needs another node for it")]
    AttributeNotClearable { node: NodeId, field: NodeField },
    #[error("{field} may only be restricted, and {to} does not restrict {from} (OPC 10000-3 §6.2.8)")]
    NotNarrowed {
        node: NodeId,
        field: NodeField,
        from: String,
        to: String,
    },
    #[error("{type_node} declares no instance declaration at {path}")]
    UnknownDeclaration { type_node: NodeId, path: Box<BrowsePath> },
    #[error("{path} can only be overridden once {parent} is (OPC 10000-3 §6.3.3.3)")]
    OverrideParentMissing {
        path: Box<BrowsePath>,
        parent: Box<BrowsePath>,
    },
    #[error(
        "the type definition of {path} may only narrow to a subtype of {from}, and {to} is not one (OPC 10000-3 §6.3.3.3)"
    )]
    TypeNotNarrowed {
        path: Box<BrowsePath>,
        from: NodeId,
        to: NodeId,
    },
    #[error(
        "the ModellingRule of {path} may only be tightened, and {from} does not become {to} (OPC 10000-3 §6.4.4.2)"
    )]
    RuleNotTightened {
        path: Box<BrowsePath>,
        from: String,
        to: String,
    },
    #[error("a Method override may only append arguments after the ones {path} inherits (OPC 10000-3 §6.3.3.3)")]
    ArgumentsNotAppended { path: Box<BrowsePath> },
    #[error("{browse_name} still carries the angle brackets that mark {path} as a placeholder")]
    PlaceholderNameNotConcrete {
        path: Box<BrowsePath>,
        browse_name: QualifiedName,
    },
    #[error("{path} is a MandatoryPlaceholder, so the instance needs at least one such child (OPC 10000-3 §6.4.4.4.5)")]
    PlaceholderRequired { path: Box<BrowsePath> },
    #[error("{node} is not a type that can be instantiated")]
    NotInstantiable { node: NodeId },
    #[error("{}", summary(_0))]
    Rejected(Verdict),
    #[error("{}", summary(_0))]
    Introduces(Verdict),
}

impl Refusal {
    /// The findings that explain the refusal, which is what the validation panel shows.
    pub fn verdict(&self) -> Option<&Verdict> {
        match self {
            Self::Rejected(verdict) | Self::Introduces(verdict) => Some(verdict),
            _ => None,
        }
    }

    /// True for a refusal the override performs anyway (general/guardrails.md §5).
    pub fn is_overridable(&self) -> bool {
        matches!(
            self,
            Self::Rejected(_)
                | Self::Introduces(_)
                | Self::NotNarrowed { .. }
                | Self::TypeNotNarrowed { .. }
                | Self::RuleNotTightened { .. }
                | Self::ArgumentsNotAppended { .. }
                | Self::PlaceholderNameNotConcrete { .. }
                | Self::PlaceholderRequired { .. }
        )
    }
}

fn summary(verdict: &Verdict) -> String {
    let messages: Vec<String> = verdict
        .errors()
        .map(|finding| format!("{}: {}", finding.code, finding.message))
        .collect();
    match messages.is_empty() {
        true => "the operation is not allowed".to_owned(),
        false => messages.join("; "),
    }
}

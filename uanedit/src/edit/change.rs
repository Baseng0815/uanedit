use crate::edit::field::{
    self,
    StoredValue,
};
use crate::nodes::node::Node;
use crate::nodes::reference::Reference;
use crate::nodeset::aliases::AliasTable;
use crate::nodeset::models::ModelTableEntry;
use crate::nodeset::namespaces::NamespaceTable;
use crate::nodeset::node_set::NodeSet;
use crate::space::delta::Delta;
use crate::types::node_id::NodeId;

/// One primitive mutation of the primary nodeset, stated the way the file states it.
///
/// Applying a change reports what it changed and hands back the change that undoes it, which is
/// what the undo log stores; positions are part of that, so an undone removal returns to where it
/// was rather than to the end of the list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Change {
    InsertNode {
        position: usize,
        node: Box<Node>,
    },
    RemoveNode {
        node_id: NodeId,
    },
    InsertReference {
        holder: NodeId,
        position: usize,
        reference: Reference,
    },
    RemoveReference {
        holder: NodeId,
        position: usize,
    },
    ReplaceReference {
        holder: NodeId,
        position: usize,
        reference: Reference,
    },
    SetField {
        node_id: NodeId,
        value: StoredValue,
    },
    SetNamespaces(NamespaceTable),
    SetModels(Vec<ModelTableEntry>),
    SetAliases(AliasTable),
}

impl Change {
    pub(crate) fn apply(
        self,
        node_set: &mut NodeSet,
        deltas: &mut Vec<Delta>,
    ) -> Option<Self> {
        match self {
            Self::InsertNode { position, node } => {
                let node_id = node.node_id().clone();
                let position = position.min(node_set.nodes.len());
                node_set
                    .nodes
                    .shift_insert(position, node_id.clone(), *node);
                deltas.push(Delta::NodeAdded {
                    node_id: node_id.clone(),
                });
                Some(Self::RemoveNode { node_id })
            }
            Self::RemoveNode { node_id } => {
                let position = node_set.nodes.get_index_of(&node_id)?;
                let node = node_set.remove(&node_id)?;
                deltas.push(Delta::NodeRemoved {
                    node_id: node_id.clone(),
                });
                Some(Self::InsertNode {
                    position,
                    node: Box::new(node),
                })
            }
            Self::InsertReference {
                holder,
                position,
                reference,
            } => {
                let references = node_set.node_mut(&holder)?.references_mut();
                let position = position.min(references.len());
                references.insert(position, reference.clone());
                deltas.push(Delta::ReferenceAdded {
                    holder: holder.clone(),
                    reference,
                });
                Some(Self::RemoveReference { holder, position })
            }
            Self::RemoveReference { holder, position } => {
                let references = node_set.node_mut(&holder)?.references_mut();
                if position >= references.len() {
                    return None;
                }
                let reference = references.remove(position);
                deltas.push(Delta::ReferenceRemoved {
                    holder: holder.clone(),
                    reference: reference.clone(),
                });
                Some(Self::InsertReference {
                    holder,
                    position,
                    reference,
                })
            }
            Self::ReplaceReference {
                holder,
                position,
                reference,
            } => {
                let references = node_set.node_mut(&holder)?.references_mut();
                let slot = references.get_mut(position)?;
                let previous = core::mem::replace(slot, reference.clone());
                deltas.push(Delta::ReferenceRemoved {
                    holder: holder.clone(),
                    reference: previous.clone(),
                });
                deltas.push(Delta::ReferenceAdded {
                    holder: holder.clone(),
                    reference,
                });
                Some(Self::ReplaceReference {
                    holder,
                    position,
                    reference: previous,
                })
            }
            Self::SetField { node_id, value } => {
                let field = value.field();
                let previous = field::set(value, node_set.node_mut(&node_id)?)?;
                deltas.push(Delta::FieldChanged {
                    node_id: node_id.clone(),
                    field,
                });
                Some(Self::SetField {
                    node_id,
                    value: previous,
                })
            }
            Self::SetNamespaces(namespaces) => {
                let previous = core::mem::replace(&mut node_set.namespaces, namespaces);
                deltas.push(Delta::NamespacesChanged);
                Some(Self::SetNamespaces(previous))
            }
            Self::SetModels(models) => {
                let previous = core::mem::replace(&mut node_set.models, models);
                deltas.push(Delta::ModelsChanged);
                Some(Self::SetModels(previous))
            }
            Self::SetAliases(aliases) => {
                let previous = core::mem::replace(&mut node_set.aliases, aliases);
                deltas.push(Delta::AliasesChanged);
                Some(Self::SetAliases(previous))
            }
        }
    }
}

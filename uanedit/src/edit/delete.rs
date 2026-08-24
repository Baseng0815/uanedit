use indexmap::IndexSet;
use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::attribute_id::AttributeId;
use crate::attributes::permissions::RolePermission;
use crate::edit::change::Change;
use crate::edit::compile::{
    Compiled,
    Compiler,
};
use crate::edit::field::{
    FieldValue,
    StoredValue,
};
use crate::edit::outcome::Refusal;
use crate::edit::reference::{
    self,
    ReferenceEnd,
    ReferenceKey,
    Statement,
};
use crate::nodes::node::Node;
use crate::nodes::reference::Reference;
use crate::rules::query;
use crate::space::AddressSpace;
use crate::space::attributes::AttributeSlot;
use crate::space::delta::NodeField;
use crate::types::node_id::NodeId;
use crate::types::node_id_ref::NodeIdRef;

/// Deleting a node, with a resolution for everything the deletion would otherwise break
/// (general/guardrails.md §3).
///
/// [`crate::edit::Session::deletion_plan`] says what those are; the operation refuses while any of
/// them is unanswered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeleteNode {
    pub node: NodeId,
    /// What becomes of each reference another node states towards a deleted node.
    pub incoming: Vec<(IncomingReference, IncomingResolution)>,
    /// What becomes of each attribute of another node that names a deleted node.
    pub attributes: Vec<(AttributeUse, AttributeResolution)>,
    /// What becomes of each node hanging under a deleted one, keyed by the child's NodeId.
    pub children: Vec<(NodeId, ChildResolution)>,
}

impl DeleteNode {
    pub fn new(node: NodeId) -> Self {
        Self {
            node,
            incoming: Vec::new(),
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    fn incoming_resolution(
        &self,
        incoming: &IncomingReference,
    ) -> Option<&IncomingResolution> {
        self.incoming
            .iter()
            .find(|(candidate, _)| candidate == incoming)
            .map(|(_, resolution)| resolution)
    }

    fn attribute_resolution(
        &self,
        attribute: &AttributeUse,
    ) -> Option<&AttributeResolution> {
        self.attributes
            .iter()
            .find(|(candidate, _)| candidate == attribute)
            .map(|(_, resolution)| resolution)
    }

    fn child_resolution(
        &self,
        child: &NodeId,
    ) -> Option<&ChildResolution> {
        self.children
            .iter()
            .find(|(candidate, _)| candidate == child)
            .map(|(_, resolution)| resolution)
    }
}

/// One reference another node states towards a node the deletion removes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IncomingReference {
    /// The node whose `References` element states it, which is the node an edit changes.
    pub holder: NodeId,
    pub reference_type: NodeId,
    /// True when the holder is the source of the reference, so the deleted node is its target.
    pub is_forward: bool,
    /// The node being deleted, which the statement names.
    pub names: NodeId,
}

/// What happens to a reference the deletion would leave naming a node that is gone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncomingResolution {
    Remove,
    /// Keeps the reference and points it at another node.
    Retarget {
        node: NodeId,
    },
}

/// One attribute of another node that names a node the deletion removes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributeUse {
    /// The node whose attribute names it, which is the node an edit changes.
    pub holder: NodeId,
    pub slot: AttributeSlot,
    /// The node being deleted, which the attribute names.
    pub names: NodeId,
}

/// What happens to an attribute the deletion would leave naming a node that is gone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeResolution {
    /// Keeps the attribute and points it at another node.
    Retarget { node: NodeId },
    /// Leaves the attribute out, which only the slots the model lets go absent allow.
    Clear,
}

/// One node hanging under a node the deletion removes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedChild {
    pub node: NodeId,
    /// The node being deleted that the child hangs under.
    pub parent: NodeId,
    pub reference_type: NodeId,
    /// The other hierarchical parents of the child, which would keep it rooted.
    pub other_parents: Vec<NodeId>,
    /// True when the child belongs to a nodeset this editor may not change, so it cannot cascade.
    pub read_only: bool,
}

/// What happens to a node that hangs under a node the deletion removes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildResolution {
    /// Delete the child too, which in turn has to resolve the child's own children and references.
    Cascade,
    /// Keep the child and hang it under another node.
    Reparent { parent: NodeId, reference_type: NodeId },
    /// Keep the child where it is, losing only the link to the deleted node.
    Detach,
}

/// Everything a deletion still has to answer for, and what it would remove as it stands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DeletionPlan {
    /// Every node the deletion removes, the node it was asked about first.
    pub deleted: Vec<NodeId>,
    pub incoming: Vec<IncomingReference>,
    /// Attributes of nodes that survive and name a deleted one, which no reference makes visible.
    pub attributes: Vec<AttributeUse>,
    pub children: Vec<OwnedChild>,
    /// References a nodeset this editor may not change states towards a deleted node. Nothing can
    /// resolve them, so the deletion leaves them dangling.
    pub read_only: Vec<IncomingReference>,
}

impl DeletionPlan {
    pub fn is_resolved(&self) -> bool {
        self.incoming.is_empty() && self.attributes.is_empty() && self.children.is_empty()
    }
}

/// What the deletion would still have to resolve, given the resolutions it already carries.
pub fn plan(
    space: &AddressSpace,
    delete: &DeleteNode,
) -> DeletionPlan {
    let mut plan = DeletionPlan::default();
    let mut deleted: IndexSet<NodeId> = IndexSet::new();
    deleted.insert(delete.node.clone());
    let mut position = 0;
    while position < deleted.len() {
        let node_id = deleted[position].clone();
        position += 1;
        for view in space.forward_references(&node_id) {
            if !space.is_hierarchical_reference_type(&view.reference_type) {
                continue;
            }
            if deleted.contains(&view.other) {
                continue;
            }
            match delete.child_resolution(&view.other) {
                Some(ChildResolution::Cascade) => {
                    deleted.insert(view.other.clone());
                }
                Some(_) => {}
                None => push_child(
                    &mut plan.children,
                    OwnedChild {
                        other_parents: space
                            .parents(&view.other)
                            .into_iter()
                            .filter(|parent| *parent != node_id)
                            .collect(),
                        read_only: space.is_read_only(&view.other),
                        node: view.other.clone(),
                        parent: node_id.clone(),
                        reference_type: view.reference_type.clone(),
                    },
                ),
            }
        }
    }
    for node_id in &deleted {
        for incoming in incoming_references(space, node_id) {
            if deleted.contains(&incoming.holder) {
                continue;
            }
            if space.is_read_only(&incoming.holder) {
                push_incoming(&mut plan.read_only, incoming);
                continue;
            }
            let answered = match delete.incoming_resolution(&incoming) {
                Some(IncomingResolution::Retarget { node }) => !deleted.contains(node),
                Some(IncomingResolution::Remove) => true,
                None => false,
            };
            if !answered {
                push_incoming(&mut plan.incoming, incoming);
            }
        }
    }
    plan.attributes = attribute_uses(space, delete, &deleted);
    plan.deleted = deleted.into_iter().collect();
    plan
}

/// The attributes of surviving nodes that name a deleted one (general/guardrails.md §3).
///
/// None of them is a reference, so the graph index says nothing about them and only a pass over
/// the editable nodeset finds them.
fn attribute_uses(
    space: &AddressSpace,
    delete: &DeleteNode,
    deleted: &IndexSet<NodeId>,
) -> Vec<AttributeUse> {
    let mut found = Vec::new();
    for holder in space.primary_node_ids() {
        if deleted.contains(&holder) || space.is_read_only(&holder) {
            continue;
        }
        for (slot, names) in space.attribute_uses(&holder) {
            if !deleted.contains(&names) {
                continue;
            }
            let attribute = AttributeUse {
                holder: holder.clone(),
                slot,
                names,
            };
            // An answer that names another deleted node, or clears an attribute the model requires,
            // leaves the same hole it was given for.
            let answered = match delete.attribute_resolution(&attribute) {
                Some(AttributeResolution::Retarget { node }) => !deleted.contains(node),
                Some(AttributeResolution::Clear) => attribute.slot.is_clearable(),
                None => false,
            };
            if !answered {
                found.push(attribute);
            }
        }
    }
    found
}

/// A reference is one reference however many ends state it (OPC 10000-3 §4.4.4), and one row of the
/// plan answers for it once.
fn push_incoming(
    into: &mut Vec<IncomingReference>,
    incoming: IncomingReference,
) {
    if !into.contains(&incoming) {
        into.push(incoming);
    }
}

fn push_child(
    into: &mut Vec<OwnedChild>,
    child: OwnedChild,
) {
    let stated = into.iter().any(|kept| {
        kept.node == child.node && kept.parent == child.parent && kept.reference_type == child.reference_type
    });
    if !stated {
        into.push(child);
    }
}

/// The references other nodes state towards this one; the ones it states itself go with it.
fn incoming_references(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Vec<IncomingReference> {
    space
        .references(node_id)
        .into_iter()
        .filter(|view| view.storage.is_stated_by_other())
        // A forward hierarchical reference is the link to a child, which a child resolution answers.
        .filter(|view| !(view.is_forward && space.is_hierarchical_reference_type(&view.reference_type)))
        .map(|view| IncomingReference {
            holder: view.other.clone(),
            reference_type: view.reference_type.clone(),
            is_forward: !view.is_forward,
            names: node_id.clone(),
        })
        .collect()
}

pub(crate) fn compile(
    compiler: &mut Compiler<'_>,
    delete: &DeleteNode,
) -> Result<Compiled, Refusal> {
    compiler.require_editable(&delete.node)?;
    let plan = plan(compiler.space(), delete);
    if !plan.is_resolved() {
        return Err(Refusal::UnresolvedDeletion { plan: Box::new(plan) });
    }
    let mut compiled = Compiled::default();
    let mut early = Vec::new();
    let mut removed = Vec::new();

    for (incoming, resolution) in &delete.incoming {
        if !plan.deleted.contains(&incoming.names) {
            continue;
        }
        let found = incoming_statements(compiler, incoming);
        match resolution {
            IncomingResolution::Remove => removed.extend(found),
            IncomingResolution::Retarget { node } => {
                compiler.require_known(node)?;
                for statement in found {
                    compiler.require_editable(&statement.holder)?;
                    let reference = Reference {
                        reference_type: compiler.reference_to(&incoming.reference_type)?,
                        is_forward: incoming.is_forward,
                        target: compiler.reference_to(node)?,
                    };
                    early.push(Change::ReplaceReference {
                        holder: compiler.local_node_id(&statement.holder)?,
                        position: statement.position,
                        reference,
                    });
                }
                let (source, target) = match incoming.is_forward {
                    true => (incoming.holder.clone(), node.clone()),
                    false => (node.clone(), incoming.holder.clone()),
                };
                compiled
                    .references
                    .push(ReferenceKey::new(source, incoming.reference_type.clone(), target));
            }
        }
    }

    early.extend(attribute_changes(compiler, &plan, delete)?);

    for (child, resolution) in &delete.children {
        if *resolution == ChildResolution::Cascade {
            continue;
        }
        for link in child_links(compiler, &plan, child) {
            let statements: Vec<Statement> = reference::statements(compiler, &link)
                .into_iter()
                .filter(|statement| statement.holder != link.source)
                .collect();
            removed.extend(statements);
        }
        let ChildResolution::Reparent { parent, reference_type } = resolution else {
            continue;
        };
        compiler.require_known(parent)?;
        let moved = ReferenceKey::new(parent.clone(), reference_type.clone(), child.clone());
        let stated_by = match compiler.is_editable(child) {
            true => ReferenceEnd::Target,
            false => ReferenceEnd::Source,
        };
        let mut relink = Compiled::default();
        reference::state_reference(compiler, &mut relink, &moved, stated_by)?;
        early.extend(relink.changes);
        compiled.references.extend(relink.references);
    }

    compiled.changes.extend(early);
    compiled
        .changes
        .extend(reference::removals(compiler, &removed)?);
    for node_id in &plan.deleted {
        compiler.require_editable(node_id)?;
        compiled.changes.push(Change::RemoveNode {
            node_id: compiler.local_node_id(node_id)?,
        });
    }
    Ok(compiled)
}

/// The references that link a child to the deleted nodes it hangs under.
fn child_links(
    compiler: &Compiler<'_>,
    plan: &DeletionPlan,
    child: &NodeId,
) -> Vec<ReferenceKey> {
    let mut found = Vec::new();
    for parent in &plan.deleted {
        for view in compiler.space().forward_references(parent) {
            if view.other == *child
                && compiler
                    .space()
                    .is_hierarchical_reference_type(&view.reference_type)
            {
                found.push(ReferenceKey::new(parent.clone(), view.reference_type.clone(), child.clone()));
            }
        }
    }
    found
}

/// One answer per attribute, and one change per node and field — a list-valued attribute is
/// rewritten whole, so two answers about one node do not each overwrite the other's work.
fn attribute_changes(
    compiler: &mut Compiler<'_>,
    plan: &DeletionPlan,
    delete: &DeleteNode,
) -> Result<Vec<Change>, Refusal> {
    let answered: Vec<&(AttributeUse, AttributeResolution)> = delete
        .attributes
        .iter()
        .filter(|(attribute, _)| plan.deleted.contains(&attribute.names))
        // An answer about a node the deletion went on to remove has nothing left to change.
        .filter(|(attribute, _)| !plan.deleted.contains(&attribute.holder))
        .collect();
    let mut changes = Vec::new();
    let mut listed: IndexSet<NodeId> = IndexSet::new();
    for (attribute, resolution) in &answered {
        compiler.require_editable(&attribute.holder)?;
        let stored = match &attribute.slot {
            AttributeSlot::DataType => {
                let AttributeResolution::Retarget { node } = resolution else {
                    return Err(not_clearable(attribute));
                };
                compiler.require_known(node)?;
                compiler.reject(query::may_set_data_type(compiler.space(), &attribute.holder, node))?;
                StoredValue::DataType(compiler.reference_to(node)?)
            }
            AttributeSlot::ParentNodeId => StoredValue::ParentNodeId(optional_target(compiler, resolution)?),
            AttributeSlot::MethodDeclarationId => {
                StoredValue::MethodDeclarationId(optional_target(compiler, resolution)?)
            }
            AttributeSlot::DefinitionField { .. } | AttributeSlot::RolePermission { .. } => {
                listed.insert(attribute.holder.clone());
                continue;
            }
        };
        changes.push(Change::SetField {
            node_id: compiler.local_node_id(&attribute.holder)?,
            value: stored,
        });
    }
    for holder in &listed {
        changes.extend(role_permissions_change(compiler, &answered, holder)?);
        changes.extend(definition_change(compiler, &answered, holder)?);
    }
    Ok(changes)
}

fn optional_target(
    compiler: &mut Compiler<'_>,
    resolution: &AttributeResolution,
) -> Result<Option<NodeIdRef>, Refusal> {
    match resolution {
        AttributeResolution::Clear => Ok(None),
        AttributeResolution::Retarget { node } => {
            compiler.require_known(node)?;
            Ok(Some(compiler.reference_to(node)?))
        }
    }
}

fn role_permissions_change(
    compiler: &mut Compiler<'_>,
    answered: &[&(AttributeUse, AttributeResolution)],
    holder: &NodeId,
) -> Result<Option<Change>, Refusal> {
    let stated = compiler
        .primary_node(holder)
        .map(|node| node.header().role_permissions.clone())
        .unwrap_or_default();
    let mut grants = Vec::with_capacity(stated.len());
    let mut answers = 0;
    for (position, grant) in stated.iter().enumerate() {
        let resolution = answer_for(answered, holder, |slot| match slot {
            AttributeSlot::RolePermission { position: at } => *at == position,
            _ => false,
        });
        match resolution {
            None => grants.push(grant.clone()),
            Some(resolution) => {
                answers += 1;
                if let AttributeResolution::Retarget { node } = resolution {
                    compiler.require_known(node)?;
                    grants.push(RolePermission {
                        role_id: compiler.reference_to(node)?,
                        permissions: grant.permissions,
                    });
                }
            }
        }
    }
    match answers {
        0 => Ok(None),
        _ => Ok(Some(Change::SetField {
            node_id: compiler.local_node_id(holder)?,
            value: StoredValue::Plain(FieldValue::RolePermissions(grants)),
        })),
    }
}

fn definition_change(
    compiler: &mut Compiler<'_>,
    answered: &[&(AttributeUse, AttributeResolution)],
    holder: &NodeId,
) -> Result<Option<Change>, Refusal> {
    let Some(Node::DataType(data_type)) = compiler.primary_node(holder) else {
        return Ok(None);
    };
    let Some(mut definition) = data_type.definition.clone() else {
        return Ok(None);
    };
    let mut answers = 0;
    for field in &mut definition.fields {
        let resolution = answer_for(answered, holder, |slot| match slot {
            AttributeSlot::DefinitionField { field: named } => *named == field.name,
            _ => false,
        });
        let Some(resolution) = resolution else {
            continue;
        };
        let AttributeResolution::Retarget { node } = resolution else {
            return Err(Refusal::AttributeNotClearable {
                node: holder.clone(),
                field: NodeField::Attribute(AttributeId::DataTypeDefinition),
            });
        };
        compiler.require_known(node)?;
        field.data_type = compiler.reference_to(node)?;
        answers += 1;
    }
    match answers {
        0 => Ok(None),
        _ => Ok(Some(Change::SetField {
            node_id: compiler.local_node_id(holder)?,
            value: StoredValue::Plain(FieldValue::DataTypeDefinition(Some(definition))),
        })),
    }
}

fn answer_for<'a>(
    answered: &[&'a (AttributeUse, AttributeResolution)],
    holder: &NodeId,
    matches: impl Fn(&AttributeSlot) -> bool,
) -> Option<&'a AttributeResolution> {
    answered
        .iter()
        .find(|(attribute, _)| attribute.holder == *holder && matches(&attribute.slot))
        .map(|(_, resolution)| resolution)
}

fn not_clearable(attribute: &AttributeUse) -> Refusal {
    Refusal::AttributeNotClearable {
        node: attribute.holder.clone(),
        field: attribute.slot.field(),
    }
}

fn incoming_statements(
    compiler: &Compiler<'_>,
    incoming: &IncomingReference,
) -> Vec<Statement> {
    let key = match incoming.is_forward {
        true => ReferenceKey::new(incoming.holder.clone(), incoming.reference_type.clone(), incoming.names.clone()),
        false => ReferenceKey::new(incoming.names.clone(), incoming.reference_type.clone(), incoming.holder.clone()),
    };
    reference::statements(compiler, &key)
        .into_iter()
        .filter(|statement| statement.holder == incoming.holder)
        .collect()
}

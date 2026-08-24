use indexmap::IndexMap;

use crate::ids;
use crate::nodes::node_class::NodeClass;
use crate::nodes::reference::Reference;
use crate::rules::code::DiagnosticCode;
use crate::rules::finding::{
    Anchor,
    Finding,
};
use crate::rules::fix::Fix;
use crate::rules::rule::{
    Impact,
    Rule,
    impact_node_and_neighbours,
    impact_parents,
};
use crate::space::delta::{
    Delta,
    NodeField,
};
use crate::space::{
    AddressSpace,
    SetId,
};
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use crate::types::node_id_ref::NodeIdRef;
use crate::types::qualified_name::QualifiedName;

/// UA0101 — a reference names a node no loaded set defines.
pub struct DanglingReferenceTarget;

impl Rule for DanglingReferenceTarget {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::DanglingReferenceTarget
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        for view in space.forward_references(node_id) {
            if space.contains(&view.other) {
                continue;
            }
            let anchor = Anchor::reference(node_id.clone(), view.reference_type.clone(), true, view.other.clone());
            let namespace_uri = space
                .namespace_uri(view.other.namespace_index)
                .map(str::to_owned);
            out.push(
                Finding::new(
                    self.code(),
                    anchor,
                    format!("no loaded nodeset defines the reference target {}", view.other),
                )
                .with_fix(Fix::LoadDependency {
                    namespace_uri,
                    target: view.other.clone(),
                }),
            );
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        match delta {
            Delta::NodeAdded { .. } | Delta::NodeRemoved { .. } => impact.whole_space(),
            _ => impact_node_and_neighbours(space, delta, impact),
        }
    }
}

/// UA0102 — the same NodeId is defined by more than one loaded nodeset.
pub struct DuplicateNodeId;

impl Rule for DuplicateNodeId {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::DuplicateNodeId
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let defining: Vec<SetId> = space
            .sets()
            .filter(|(set, node_set)| {
                space
                    .to_local_node_id(*set, node_id)
                    .is_some_and(|local| node_set.node(&local).is_some())
            })
            .map(|(set, _)| set)
            .collect();
        if defining.len() < 2 {
            return;
        }
        let names: Vec<String> = defining
            .iter()
            .filter_map(|set| space.set(*set)?.target_namespace().map(str::to_owned))
            .collect();
        out.push(
            Finding::new(
                self.code(),
                Anchor::node(node_id.clone()),
                format!("{} loaded nodesets define {node_id}", defining.len()),
            )
            .with_facts(&names.join(",")),
        );
    }

    fn impact(
        &self,
        _space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        match delta.node_id() {
            Some(node_id) => impact.node(node_id),
            None => impact.whole_space(),
        }
    }
}

/// UA0103 — two children of a type or an instance declaration share a BrowseName.
pub struct SiblingBrowseNameCollision;

impl Rule for SiblingBrowseNameCollision {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::SiblingBrowseNameCollision
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let is_container = space
            .node_class(node_id)
            .is_some_and(|class| matches!(class, NodeClass::ObjectType | NodeClass::VariableType))
            || space.is_instance_declaration(node_id);
        if !is_container {
            return;
        }
        for (browse_name, children) in group_children_by_browse_name(space, node_id) {
            if children.len() < 2 {
                continue;
            }
            out.push(
                Finding::new(
                    self.code(),
                    Anchor::node(node_id.clone()),
                    format!("{} children share the BrowseName {browse_name}", children.len()),
                )
                .with_facts(&collision_facts(&browse_name, &children))
                .with_fix(Fix::SetBrowseName {
                    node: children[1].clone(),
                    browse_name: browse_name.clone(),
                }),
            );
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
        impact_parents(space, impact);
    }
}

/// UA0110 — two Properties of one node share a BrowseName, which is forbidden anywhere.
pub struct DuplicatePropertyBrowseName;

impl Rule for DuplicatePropertyBrowseName {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::DuplicatePropertyBrowseName
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let mut by_name: IndexMap<QualifiedName, usize> = IndexMap::new();
        for property in space.targets_of_type(node_id, &ids::HAS_PROPERTY, true) {
            if let Some(browse_name) = space.browse_name(&property) {
                *by_name.entry(browse_name).or_default() += 1;
            }
        }
        for (browse_name, count) in by_name {
            if count < 2 {
                continue;
            }
            out.push(
                Finding::new(
                    self.code(),
                    Anchor::node(node_id.clone()),
                    format!("{count} Properties share the BrowseName {browse_name}"),
                )
                .with_facts(&browse_name.to_string()),
            );
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
        impact_parents(space, impact);
    }
}

/// UA0104 — a namespace index the file's own namespace table has no entry for.
pub struct UnknownNamespaceIndex;

impl Rule for UnknownNamespaceIndex {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::UnknownNamespaceIndex
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some((set, node)) = local_node(space, node_id) else {
            return;
        };
        let mut report = |index, which: &str| {
            if undeclared_index(space, set, index) {
                out.push(namespace_finding(node_id, index, which));
            }
        };
        report(node.browse_name().namespace_index, "the BrowseName");
        for reference in node.references() {
            for (part, which) in reference_parts(reference) {
                if let Some(resolved) = space.set(set).and_then(|node_set| node_set.resolve(part)) {
                    report(resolved.namespace_index, which);
                }
            }
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_on_tables(space, delta, impact);
    }
}

/// UA0106 — a reference names an alias the file's alias table does not define.
pub struct UnknownAlias;

impl Rule for UnknownAlias {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::UnknownAlias
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some((set, node)) = local_node(space, node_id) else {
            return;
        };
        let Some(node_set) = space.set(set) else {
            return;
        };
        for reference in node.references() {
            for (part, which) in reference_parts(reference) {
                if node_set.resolve(part).is_some() {
                    continue;
                }
                let alias = part.as_alias().unwrap_or_default().to_owned();
                out.push(
                    Finding::new(
                        self.code(),
                        Anchor::node(node_id.clone()),
                        format!("{which} names the alias `{alias}`, which the nodeset does not define"),
                    )
                    .with_facts(&alias)
                    .with_fix(Fix::DefineAlias { alias }),
                );
            }
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_on_tables(space, delta, impact);
    }
}

fn impact_on_tables(
    space: &AddressSpace,
    delta: &Delta,
    impact: &mut Impact,
) {
    match delta {
        Delta::NamespacesChanged | Delta::AliasesChanged => impact.whole_space(),
        _ => impact_node_and_neighbours(space, delta, impact),
    }
}

fn reference_parts(reference: &Reference) -> [(&NodeIdRef, &'static str); 2] {
    [
        (&reference.reference_type, "the reference type"),
        (&reference.target, "the reference target"),
    ]
}

/// The node as the file that defines it spells it, which is where aliases and indexes still show.
fn local_node<'a>(
    space: &'a AddressSpace,
    node_id: &NodeId,
) -> Option<(SetId, &'a crate::nodes::node::Node)> {
    let set = space.set_of(node_id)?;
    let local = space.to_local_node_id(set, node_id)?;
    Some((set, space.set(set)?.node(&local)?))
}

fn undeclared_index(
    space: &AddressSpace,
    set: SetId,
    index: NamespaceIndex,
) -> bool {
    index != 0
        && space
            .set(set)
            .is_some_and(|node_set| node_set.namespaces.uri(index).is_none())
}

fn namespace_finding(
    node_id: &NodeId,
    index: NamespaceIndex,
    which: &str,
) -> Finding {
    Finding::new(
        DiagnosticCode::UnknownNamespaceIndex,
        Anchor::namespace(node_id.clone(), index),
        format!("{which} uses namespace index {index}, which the nodeset's table does not declare"),
    )
    .with_facts(which)
    .with_fix(Fix::DeclareNamespace { index })
}

/// UA0107 — one node's own `References` element states the same reference twice.
///
/// The graph holds one edge per reference however many ends state it, so this reads the file's own
/// list; a reference written forward on the source and inverse on the target is one reference
/// (OPC 10000-3 §4.4.4) and not a duplicate.
pub struct DuplicateReference;

impl Rule for DuplicateReference {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::DuplicateReference
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some((set, node)) = local_node(space, node_id) else {
            return;
        };
        let Some(node_set) = space.set(set) else {
            return;
        };
        let mut seen: IndexMap<(NodeId, bool, NodeId), usize> = IndexMap::new();
        for reference in node.references() {
            let stated = node_set
                .resolve(&reference.reference_type)
                .and_then(|local| space.to_space_node_id(set, local))
                .zip(
                    node_set
                        .resolve(&reference.target)
                        .and_then(|local| space.to_space_node_id(set, local)),
                );
            if let Some((reference_type, other)) = stated {
                *seen
                    .entry((reference_type, reference.is_forward, other))
                    .or_default() += 1;
            }
        }
        for ((reference_type, is_forward, other), count) in seen {
            if count < 2 {
                continue;
            }
            let direction = match is_forward {
                true => "to",
                false => "from",
            };
            out.push(Finding::new(
                self.code(),
                Anchor::reference(node_id.clone(), reference_type.clone(), is_forward, other.clone()),
                format!("the node states the reference {reference_type} {direction} {other} {count} times"),
            ));
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
    }
}

/// UA0108 — a node with the null NodeId.
pub struct NullNodeId;

impl Rule for NullNodeId {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::NullNodeId
    }

    fn check(
        &self,
        _space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        if node_id.is_null() {
            out.push(Finding::new(self.code(), Anchor::node(node_id.clone()), "the node has the null NodeId"));
        }
    }

    fn impact(
        &self,
        _space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        if let Some(node_id) = delta.node_id() {
            impact.node(node_id);
        }
    }
}

/// UA0109 — the design-time ParentNodeId disagrees with the hierarchical references.
pub struct ParentNodeIdMismatch;

impl Rule for ParentNodeIdMismatch {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::ParentNodeIdMismatch
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some(set) = space.set_of(node_id) else {
            return;
        };
        let Some(node) = space.node(node_id) else {
            return;
        };
        let Some(parent_ref) = node
            .instance()
            .and_then(|instance| instance.parent_node_id.as_ref())
        else {
            return;
        };
        let Some(node_set) = space.set(set) else {
            return;
        };
        let Some(declared) = node_set
            .resolve(parent_ref)
            .and_then(|local| space.to_space_node_id(set, local))
        else {
            return;
        };
        if space.parents(node_id).contains(&declared) {
            return;
        }
        out.push(
            Finding::new(
                self.code(),
                Anchor::field(node_id.clone(), NodeField::ParentNodeId),
                format!("ParentNodeId names {declared}, which no hierarchical reference makes a parent"),
            )
            .with_facts(&declared.to_string())
            .with_fix(Fix::SetParentNodeId {
                node: node_id.clone(),
                parent: space.parents(node_id).into_iter().next(),
            }),
        );
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
    }
}

/// The colliding name together with the nodes that share it, sorted so that reordering the
/// references does not expire the acknowledgement but a further collision does.
fn collision_facts(
    browse_name: &QualifiedName,
    children: &[NodeId],
) -> String {
    let mut colliding: Vec<String> = children.iter().map(NodeId::to_string).collect();
    colliding.sort();
    format!("{browse_name}={}", colliding.join(","))
}

fn group_children_by_browse_name(
    space: &AddressSpace,
    node_id: &NodeId,
) -> IndexMap<QualifiedName, Vec<NodeId>> {
    let mut grouped: IndexMap<QualifiedName, Vec<NodeId>> = IndexMap::new();
    for child in space.children(node_id) {
        if let Some(browse_name) = space.browse_name(&child) {
            grouped.entry(browse_name).or_default().push(child);
        }
    }
    grouped
}

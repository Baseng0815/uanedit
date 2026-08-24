use indexmap::IndexSet;

use crate::rules::code::DiagnosticCode;
use crate::rules::finding::Finding;
use crate::space::delta::Delta;
use crate::space::{
    AddressSpace,
    SetId,
};
use crate::types::node_id::NodeId;

/// The nodes a change can alter the findings of, or a demand to run the rule over everything.
#[derive(Clone, Debug, Default)]
pub struct Impact {
    nodes: IndexSet<NodeId>,
    global: bool,
}

impl Impact {
    pub fn node(
        &mut self,
        node_id: &NodeId,
    ) {
        self.nodes.insert(node_id.clone());
    }

    pub fn extend(
        &mut self,
        node_ids: impl IntoIterator<Item = NodeId>,
    ) {
        self.nodes.extend(node_ids);
    }

    /// Marks the rule as needing a whole pass, which is the honest answer when a change can
    /// remove a finding somewhere the delta does not name.
    pub fn whole_space(&mut self) {
        self.global = true;
    }

    pub fn is_whole_space(&self) -> bool {
        self.global
    }

    pub fn iter(&self) -> impl Iterator<Item = &NodeId> {
        self.nodes.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && !self.global
    }
}

/// One rule of the engine: what it finds about a node, and what changes can move that.
pub trait Rule: Send + Sync {
    fn code(&self) -> DiagnosticCode;

    /// Findings this rule makes about one node.
    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    );

    /// The nodes whose findings under this rule the change can alter.
    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    );

    /// True for a rule that is cheaper as one pass over the graph than as a check per node.
    fn is_whole_space(&self) -> bool {
        false
    }

    /// Report mode. The default checks every node in scope; a whole-space rule overrides it.
    fn report(
        &self,
        space: &AddressSpace,
        subjects: &[NodeId],
        out: &mut Vec<Finding>,
    ) {
        for node_id in subjects {
            self.check(space, node_id, out);
        }
    }
}

/// Marks the node the change is about and every node holding a reference to it, which is what a
/// rule about references has to re-examine.
pub fn impact_node_and_neighbours(
    space: &AddressSpace,
    delta: &Delta,
    impact: &mut Impact,
) {
    let Some(node_id) = delta.node_id() else {
        impact.whole_space();
        return;
    };
    impact.node(node_id);
    match delta {
        Delta::ReferenceAdded { reference, .. } | Delta::ReferenceRemoved { reference, .. } => {
            // A reference names its target the way the file spells it; the impact is stated in the
            // space's indexes, like every other NodeId a rule sees.
            let target = space
                .set(SetId::PRIMARY)
                .and_then(|set| set.resolve(&reference.target))
                .and_then(|local| space.to_space_node_id(SetId::PRIMARY, local));
            if let Some(target) = target {
                impact.node(&target);
            }
        }
        Delta::NodeRemoved { .. } | Delta::NodeAdded { .. } => {
            impact.extend(space.references(node_id).into_iter().map(|view| view.other));
            // A DataType, a ParentNodeId or a RolePermissions grant names a node without the graph
            // holding an edge for it, so nothing in the index would bring those nodes back.
            impact.extend(space.attribute_referrers(node_id));
        }
        _ => {}
    }
}

/// Adds the hierarchical parents of the nodes already marked, for rules that look at siblings.
pub fn impact_parents(
    space: &AddressSpace,
    impact: &mut Impact,
) {
    let parents: Vec<NodeId> = impact
        .iter()
        .flat_map(|node_id| space.parents(node_id))
        .collect();
    impact.extend(parents);
}

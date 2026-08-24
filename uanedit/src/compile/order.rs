use std::collections::HashMap;

use crate::nodes::Node;
use crate::space::ReferenceView;
use crate::types::node_id::NodeId;
use crate::{
    AddressSpace,
    ids,
};

/// The primary nodes in an order every addNode call can rely on: hierarchical parents before
/// children, types before their instances and encodings, ReferenceTypes and DataTypes first.
pub(super) fn sorted(space: &AddressSpace) -> Vec<NodeId> {
    let nodes: Vec<NodeId> = space.primary_node_ids().collect();
    let position: HashMap<&NodeId, usize> = nodes
        .iter()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect();
    let mut indegree = vec![0usize; nodes.len()];
    let mut released: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (index, id) in nodes.iter().enumerate() {
        for view in space.references(id) {
            let Some(source) = ordering_source(space, &view) else {
                continue;
            };
            if source == *id {
                continue;
            }
            if let Some(&from) = position.get(&source) {
                released[from].push(index);
                indegree[index] += 1;
            }
        }
    }

    let mut visited = vec![false; nodes.len()];
    let mut result = Vec::with_capacity(nodes.len());
    let mut drain = |stack: &mut Vec<usize>, indegree: &mut Vec<usize>, visited: &mut Vec<bool>| {
        while let Some(index) = stack.pop() {
            if visited[index] {
                continue;
            }
            visited[index] = true;
            result.push(nodes[index].clone());
            for &next in &released[index] {
                indegree[next] = indegree[next].saturating_sub(1);
                if indegree[next] == 0 && !visited[next] {
                    stack.push(next);
                }
            }
        }
    };

    let mut stack: Vec<usize> = (0..nodes.len())
        .filter(|&index| indegree[index] == 0 && seeds_first(space, &nodes[index]))
        .collect();
    drain(&mut stack, &mut indegree, &mut visited);
    let mut stack: Vec<usize> = (0..nodes.len())
        .filter(|&index| indegree[index] == 0 && !visited[index])
        .collect();
    drain(&mut stack, &mut indegree, &mut visited);

    for (index, id) in nodes.iter().enumerate() {
        if !visited[index] {
            result.push(id.clone());
        }
    }
    result
}

/// The node that must be created before the viewed one, if this reference orders them.
fn ordering_source(
    space: &AddressSpace,
    view: &ReferenceView,
) -> Option<NodeId> {
    let ordered = match view.is_forward {
        false => {
            space.is_hierarchical_reference_type(&view.reference_type)
                || space.is_same_or_subtype_of(&view.reference_type, &ids::HAS_ENCODING)
        }
        true => space.is_same_or_subtype_of(&view.reference_type, &ids::HAS_TYPE_DEFINITION),
    };
    ordered.then(|| view.other.clone())
}

fn seeds_first(
    space: &AddressSpace,
    id: &NodeId,
) -> bool {
    matches!(space.node(id), Some(Node::ReferenceType(_) | Node::DataType(_)))
}

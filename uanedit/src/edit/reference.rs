use serde::{
    Deserialize,
    Serialize,
};

use crate::edit::change::Change;
use crate::edit::compile::{
    Compiled,
    Compiler,
};
use crate::edit::outcome::Refusal;
use crate::nodes::node_class::NodeClass;
use crate::nodes::reference::Reference;
use crate::rules::query;
use crate::types::node_id::NodeId;

/// One reference, named the way the specification identifies it: source, type and target
/// (OPC 10000-3 §4.4.4), whichever end of it the file happens to state.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReferenceKey {
    pub source: NodeId,
    pub reference_type: NodeId,
    pub target: NodeId,
}

impl ReferenceKey {
    pub fn new(
        source: NodeId,
        reference_type: NodeId,
        target: NodeId,
    ) -> Self {
        Self {
            source,
            reference_type,
            target,
        }
    }
}

/// Which end of a new reference states it, since only one end ever does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceEnd {
    /// The source states it forward, which is how a file lists what a node contains.
    Source,
    /// The target states it inversely, which is the only way when the source is not editable.
    Target,
    /// Whichever end the editor may write, the source first.
    #[default]
    Either,
}

/// Adding one reference between two nodes that already exist.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddReference {
    pub source: NodeId,
    pub reference_type: NodeId,
    pub target: NodeId,
    pub stated_by: ReferenceEnd,
}

impl AddReference {
    pub fn new(
        source: NodeId,
        reference_type: NodeId,
        target: NodeId,
    ) -> Self {
        Self {
            source,
            reference_type,
            target,
            stated_by: ReferenceEnd::Either,
        }
    }

    pub fn key(&self) -> ReferenceKey {
        ReferenceKey::new(self.source.clone(), self.reference_type.clone(), self.target.clone())
    }
}

/// Where one statement of a reference lives in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Statement {
    pub holder: NodeId,
    pub position: usize,
    pub is_forward: bool,
}

/// Every statement of the reference the primary nodeset holds, in node order.
pub(crate) fn statements(
    compiler: &Compiler<'_>,
    key: &ReferenceKey,
) -> Vec<Statement> {
    let mut found = Vec::new();
    for (node_id, is_forward) in [(&key.source, true), (&key.target, false)] {
        let Some(node) = compiler.primary_node(node_id) else {
            continue;
        };
        let other = match is_forward {
            true => &key.target,
            false => &key.source,
        };
        for (position, reference) in node.references().iter().enumerate() {
            if reference.is_forward != is_forward {
                continue;
            }
            if compiler.resolves_to(&reference.reference_type, &key.reference_type)
                && compiler.resolves_to(&reference.target, other)
            {
                found.push(Statement {
                    holder: node_id.clone(),
                    position,
                    is_forward,
                });
            }
        }
    }
    found
}

pub(crate) fn add(
    compiler: &mut Compiler<'_>,
    add: &AddReference,
) -> Result<Compiled, Refusal> {
    compiler.require_known(&add.source)?;
    compiler.require_known(&add.target)?;
    compiler.reject(query::may_add_reference(compiler.space(), &add.source, &add.reference_type, &add.target))?;
    let mut compiled = Compiled::default();
    state_reference(compiler, &mut compiled, &add.key(), add.stated_by)?;
    Ok(compiled)
}

/// Writes one statement of the reference on whichever end the caller asked for and may edit.
pub(crate) fn state_reference(
    compiler: &mut Compiler<'_>,
    compiled: &mut Compiled,
    key: &ReferenceKey,
    stated_by: ReferenceEnd,
) -> Result<(), Refusal> {
    let source_editable = compiler.is_editable(&key.source);
    let holder_is_source = match stated_by {
        ReferenceEnd::Source => true,
        ReferenceEnd::Target => false,
        ReferenceEnd::Either => source_editable,
    };
    let holder = match holder_is_source {
        true => &key.source,
        false => &key.target,
    };
    compiler.require_editable(holder)?;
    let other = match holder_is_source {
        true => &key.target,
        false => &key.source,
    };
    let reference = Reference {
        reference_type: compiler.reference_to(&key.reference_type)?,
        is_forward: holder_is_source,
        target: compiler.reference_to(other)?,
    };
    let position = compiler.reference_count(holder);
    compiled.changes.push(Change::InsertReference {
        holder: compiler.local_node_id(holder)?,
        position,
        reference,
    });
    compiled.references.push(key.clone());
    Ok(())
}

pub(crate) fn remove(
    compiler: &mut Compiler<'_>,
    key: &ReferenceKey,
) -> Result<Compiled, Refusal> {
    let found = statements(compiler, key);
    if found.is_empty() {
        return Err(missing(compiler, key));
    }
    let mut compiled = Compiled::default();
    compiled.changes.extend(removals(compiler, &found)?);
    Ok(compiled)
}

pub(crate) fn retarget(
    compiler: &mut Compiler<'_>,
    key: &ReferenceKey,
    target: &NodeId,
) -> Result<Compiled, Refusal> {
    compiler.require_known(target)?;
    let found = statements(compiler, key);
    if found.is_empty() {
        return Err(missing(compiler, key));
    }
    compiler.reject(query::may_add_reference(compiler.space(), &key.source, &key.reference_type, target))?;
    let mut compiled = Compiled::default();
    let moved = ReferenceKey::new(key.source.clone(), key.reference_type.clone(), target.clone());
    let stated_on_source: Vec<Statement> = found
        .iter()
        .filter(|statement| statement.is_forward)
        .cloned()
        .collect();
    for statement in &stated_on_source {
        compiler.require_editable(&statement.holder)?;
        let reference = Reference {
            reference_type: compiler.reference_to(&key.reference_type)?,
            is_forward: true,
            target: compiler.reference_to(target)?,
        };
        compiled.changes.push(Change::ReplaceReference {
            holder: compiler.local_node_id(&statement.holder)?,
            position: statement.position,
            reference,
        });
    }
    // A statement the old target holds names a node the reference no longer touches, so it goes.
    // Restating it is only needed where the source held none, since one statement is the whole
    // reference (OPC 10000-6 §F.4) and a second one would be a duplicate of it.
    let stated_on_target: Vec<Statement> = found
        .iter()
        .filter(|statement| !statement.is_forward)
        .cloned()
        .collect();
    if !stated_on_target.is_empty() {
        compiled
            .changes
            .extend(removals(compiler, &stated_on_target)?);
        if stated_on_source.is_empty() {
            state_reference(compiler, &mut compiled, &moved, ReferenceEnd::Either)?;
        }
    }
    compiled.references.push(moved);
    Ok(compiled)
}

/// Drops the reference of this type the node states, if any, and states the new one in its place.
pub(crate) fn replace_single(
    compiler: &mut Compiler<'_>,
    compiled: &mut Compiled,
    node_id: &NodeId,
    reference_type: &NodeId,
    target: Option<&NodeId>,
) -> Result<(), Refusal> {
    compiler.require_editable(node_id)?;
    let existing: Vec<NodeId> = compiler
        .space()
        .targets_of_type(node_id, reference_type, false);
    let mut found = Vec::new();
    for existing_target in &existing {
        found.extend(statements(
            compiler,
            &ReferenceKey::new(node_id.clone(), reference_type.clone(), existing_target.clone()),
        ));
    }
    let in_place = found
        .iter()
        .find(|statement| statement.is_forward && statement.holder == *node_id)
        .cloned();
    let mut rest: Vec<Statement> = found
        .into_iter()
        .filter(|statement| Some(statement) != in_place.as_ref())
        .collect();
    let Some(target) = target else {
        rest.extend(in_place);
        compiled.changes.extend(removals(compiler, &rest)?);
        return Ok(());
    };
    compiler.require_known(target)?;
    let reference = Reference {
        reference_type: compiler.reference_to(reference_type)?,
        is_forward: true,
        target: compiler.reference_to(target)?,
    };
    if rest.is_empty()
        && in_place
            .as_ref()
            .is_some_and(|statement| compiler.states(statement, &reference))
    {
        return Ok(());
    }
    // Stated before the removals, so no removal has moved the position it names.
    match in_place {
        Some(statement) => compiled.changes.push(Change::ReplaceReference {
            holder: compiler.local_node_id(&statement.holder)?,
            position: statement.position,
            reference,
        }),
        None => compiled.changes.push(Change::InsertReference {
            holder: compiler.local_node_id(node_id)?,
            position: compiler.reference_count(node_id),
            reference,
        }),
    }
    compiled.changes.extend(removals(compiler, &rest)?);
    compiled
        .references
        .push(ReferenceKey::new(node_id.clone(), reference_type.clone(), target.clone()));
    Ok(())
}

/// Removals ordered so that no earlier one moves a later one's position.
pub(crate) fn removals(
    compiler: &mut Compiler<'_>,
    statements: &[Statement],
) -> Result<Vec<Change>, Refusal> {
    let mut ordered: Vec<&Statement> = statements.iter().collect();
    ordered.sort_by(|left, right| {
        left.holder
            .cmp(&right.holder)
            .then(right.position.cmp(&left.position))
    });
    let mut changes = Vec::new();
    for statement in ordered {
        compiler.require_editable(&statement.holder)?;
        changes.push(Change::RemoveReference {
            holder: compiler.local_node_id(&statement.holder)?,
            position: statement.position,
        });
    }
    Ok(changes)
}

fn missing(
    compiler: &Compiler<'_>,
    key: &ReferenceKey,
) -> Refusal {
    let stated_elsewhere = compiler
        .space()
        .forward_references(&key.source)
        .into_iter()
        .any(|view| view.reference_type == key.reference_type && view.other == key.target);
    match stated_elsewhere {
        true => Refusal::ReferenceReadOnly {
            reference: Box::new(key.clone()),
        },
        false => Refusal::ReferenceNotFound {
            reference: Box::new(key.clone()),
        },
    }
}

/// The hierarchical reference a new node hangs under, stated on the new node itself.
///
/// The class the new node will have is what the rules judge the link by, since there is no node to
/// look at yet; [`query::legal_child_reference_types`] offers exactly what this accepts.
pub(crate) fn hierarchical_link(
    compiler: &mut Compiler<'_>,
    parent: &NodeId,
    reference_type: &NodeId,
    node_class: NodeClass,
) -> Result<Reference, Refusal> {
    if !compiler
        .space()
        .is_hierarchical_reference_type(reference_type)
    {
        return Err(Refusal::NotHierarchical {
            reference_type: reference_type.clone(),
        });
    }
    compiler.reject(query::may_add_child(compiler.space(), parent, reference_type, node_class))?;
    Ok(Reference {
        reference_type: compiler.reference_to(reference_type)?,
        is_forward: false,
        target: compiler.reference_to(parent)?,
    })
}

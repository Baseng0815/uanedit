use indexmap::IndexSet;

use crate::edit::change::Change;
use crate::edit::compile::compile;
use crate::edit::delete::{
    self,
    DeleteNode,
    DeletionPlan,
};
use crate::edit::operation::Operation;
use crate::edit::outcome::{
    Applied,
    Refusal,
};
use crate::edit::reference::ReferenceKey;
use crate::edit::table;
use crate::nodes::reference::Reference;
use crate::nodeset::node_set::NodeSet;
use crate::rules::acknowledge::{
    Acknowledgement,
    Acknowledgements,
};
use crate::rules::checks::references::reference_findings;
use crate::rules::engine::{
    Diagnostics,
    RuleEngine,
    Scope,
};
use crate::rules::finding::{
    Finding,
    Origin,
};
use crate::rules::fingerprint::Fingerprint;
use crate::space::delta::Delta;
use crate::space::{
    AddressSpace,
    SetId,
};
use crate::types::node_id::NodeId;

/// One open document: the address space, the rules engine over it, and the undo log.
///
/// This is the object an editing front end holds per open nodeset. Nothing else changes the model:
/// every edit is an [`Operation`] handed to [`Session::apply`], which refuses it whole rather than
/// leave an error finding behind (general/guardrails.md §3), and records how to take it back.
pub struct Session {
    space: AddressSpace,
    engine: RuleEngine,
    diagnostics: Diagnostics,
    /// The error findings that are this editor's doing, as of the last completed edit.
    introduced: IndexSet<Fingerprint>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    serial: u64,
    saved: u64,
}

/// What one set of changes did: what to report, how to take it back, and what to look at again.
struct Changed {
    deltas: Vec<Delta>,
    inverses: Vec<Change>,
    neighbours: Vec<NodeId>,
}

impl Changed {
    fn names(
        &self,
        node_id: &NodeId,
    ) -> bool {
        self.deltas
            .iter()
            .any(|delta| delta.node_id() == Some(node_id))
    }
}

/// One entry of the log: what it was called, and the changes that take the document back.
struct Entry {
    label: String,
    changes: Vec<Change>,
    via_override: bool,
    reason: Option<String>,
    /// Identifies the state the document is in with this entry on the undo stack.
    serial: u64,
}

impl Session {
    /// Opens a document, taking the findings the file already has as its baseline.
    pub fn open(space: AddressSpace) -> Self {
        Self::with_acknowledgements(space, Acknowledgements::default())
    }

    pub fn with_acknowledgements(
        space: AddressSpace,
        acknowledgements: Acknowledgements,
    ) -> Self {
        let mut engine = RuleEngine::with_scope(Scope::Primary, acknowledgements);
        let diagnostics = engine.open(&space);
        let introduced = introduced_errors(&diagnostics);
        Self {
            space,
            engine,
            diagnostics,
            introduced,
            undo: Vec::new(),
            redo: Vec::new(),
            serial: 0,
            saved: 0,
        }
    }

    pub fn space(&self) -> &AddressSpace {
        &self.space
    }

    /// The nodeset to write back out.
    pub fn primary(&self) -> &NodeSet {
        self.space.primary()
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    pub fn engine(&self) -> &RuleEngine {
        &self.engine
    }

    pub fn acknowledgements(&self) -> &Acknowledgements {
        self.engine.acknowledgements()
    }

    pub fn into_space(self) -> AddressSpace {
        self.space
    }

    /// Performs the operation, or refuses it and leaves the document exactly as it was.
    pub fn apply(
        &mut self,
        operation: Operation,
    ) -> Result<Applied, Refusal> {
        self.perform(&operation, false, None)
    }

    /// Performs an operation the engine refuses, attributing the findings it leaves to the override
    /// (general/guardrails.md §5).
    pub fn apply_with_override(
        &mut self,
        operation: Operation,
        reason: Option<String>,
    ) -> Result<Applied, Refusal> {
        self.perform(&operation, true, reason)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// What the operation on top of the undo stack was called.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.label.as_str())
    }

    /// Every operation on the undo stack, oldest first.
    pub fn history(&self) -> impl Iterator<Item = &str> {
        self.undo.iter().map(|entry| entry.label.as_str())
    }

    pub fn undo(&mut self) -> Option<Applied> {
        let entry = self.undo.pop()?;
        let (applied, reversed) = self.replay(entry);
        self.redo.push(reversed);
        Some(applied)
    }

    pub fn redo(&mut self) -> Option<Applied> {
        let entry = self.redo.pop()?;
        let (applied, reversed) = self.replay(entry);
        self.undo.push(reversed);
        Some(applied)
    }

    /// What a deletion still has to answer for, given the resolutions it carries so far.
    pub fn deletion_plan(
        &self,
        delete: &DeleteNode,
    ) -> DeletionPlan {
        delete::plan(&self.space, delete)
    }

    pub fn deletion_plan_for(
        &self,
        node: &NodeId,
    ) -> DeletionPlan {
        self.deletion_plan(&DeleteNode::new(node.clone()))
    }

    /// The nodes that would stop resolving if the alias were removed.
    pub fn alias_users(
        &self,
        alias: &str,
    ) -> Vec<NodeId> {
        table::alias_users(self.primary(), alias)
    }

    pub fn acknowledge(
        &mut self,
        acknowledgement: Acknowledgement,
    ) -> &Diagnostics {
        self.engine.acknowledge(acknowledgement);
        self.diagnostics = self.engine.diagnostics();
        &self.diagnostics
    }

    pub fn unacknowledge(
        &mut self,
        fingerprint: &Fingerprint,
    ) -> &Diagnostics {
        self.engine.unacknowledge(fingerprint);
        self.diagnostics = self.engine.diagnostics();
        &self.diagnostics
    }

    /// Drops the acknowledgements no finding matches any more, which is what the sidecar keeps.
    pub fn prune_acknowledgements(&mut self) {
        self.engine.prune_acknowledgements();
    }

    /// True when the document has changed since it was last written out.
    pub fn is_modified(&self) -> bool {
        self.state() != self.saved
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.state();
    }

    fn state(&self) -> u64 {
        self.undo.last().map_or(0, |entry| entry.serial)
    }

    fn perform(
        &mut self,
        operation: &Operation,
        forced: bool,
        reason: Option<String>,
    ) -> Result<Applied, Refusal> {
        let label = operation.label(&self.space);
        let compiled = compile(&self.space, operation, forced)?;
        if compiled.changes.is_empty() {
            return Ok(self.unchanged(label));
        }
        let before = fingerprints(&self.diagnostics);
        let changed = self.mutate(compiled.changes);
        self.refresh(&changed);
        let mut findings = self.new_errors(&compiled.references);
        merge(&mut findings, compiled.rejections);
        if !findings.is_empty() && !forced {
            let undone = self.mutate(changed.inverses);
            self.refresh(&undone);
            self.note_introduced();
            return Err(Refusal::Introduces(findings.into()));
        }
        let acknowledgements_changed = self.attribute(&findings, reason.clone());
        self.note_introduced();
        self.redo.clear();
        self.serial += 1;
        self.undo.push(Entry {
            label: label.clone(),
            changes: changed.inverses,
            via_override: forced,
            reason,
            serial: self.serial,
        });
        Ok(Applied {
            label,
            deltas: changed.deltas,
            diagnostics: self.diagnostics.clone(),
            created: compiled.created,
            via_override: forced,
            overridden: findings,
            introduced: self.since(&before),
            acknowledgements_changed,
        })
    }

    /// Applies a log entry and turns it into the entry that takes it back again.
    fn replay(
        &mut self,
        entry: Entry,
    ) -> (Applied, Entry) {
        let before = fingerprints(&self.diagnostics);
        let changed = self.mutate(entry.changes);
        self.refresh(&changed);
        let mut acknowledgements_changed = false;
        let overridden = match entry.via_override {
            true => {
                let findings = self.new_errors(&[]);
                acknowledgements_changed = self.attribute(&findings, entry.reason.clone());
                findings
            }
            false => Vec::new(),
        };
        self.note_introduced();
        let created = changed
            .deltas
            .iter()
            .filter_map(|delta| match delta {
                Delta::NodeAdded { node_id } => Some(node_id.clone()),
                _ => None,
            })
            .collect();
        let applied = Applied {
            label: entry.label.clone(),
            deltas: changed.deltas,
            diagnostics: self.diagnostics.clone(),
            created,
            via_override: entry.via_override,
            overridden,
            introduced: self.since(&before),
            acknowledgements_changed,
        };
        let reversed = Entry {
            changes: changed.inverses,
            ..entry
        };
        (applied, reversed)
    }

    /// Applies the changes and reports them, together with the nodes around them.
    ///
    /// The neighbours are gathered before the change, because the reference that made a node a
    /// neighbour is gone afterwards and the rules would not know to look at it again.
    fn mutate(
        &mut self,
        changes: Vec<Change>,
    ) -> Changed {
        let neighbours = self.neighbours(&changes);
        let mut inverses = Vec::new();
        let deltas = self.space.edit(|node_set| {
            let mut deltas = Vec::new();
            for change in changes {
                if let Some(inverse) = change.apply(node_set, &mut deltas) {
                    inverses.push(inverse);
                }
            }
            deltas
        });
        inverses.reverse();
        Changed {
            deltas,
            inverses,
            neighbours,
        }
    }

    /// Re-checks from the changes, and from the neighbours whose findings they can have moved.
    fn refresh(
        &mut self,
        changed: &Changed,
    ) {
        let mut diagnostics = self.engine.update(&self.space, &changed.deltas);
        let neighbours: Vec<NodeId> = changed
            .neighbours
            .iter()
            .filter(|node_id| !changed.names(node_id))
            .cloned()
            .collect();
        if !neighbours.is_empty() {
            diagnostics = self.engine.recheck(&self.space, &neighbours);
        }
        self.diagnostics = diagnostics;
    }

    /// The editable nodes a change names, or that a reference it removes or adds names.
    ///
    /// A change names a node the way the file spells it; the rules engine works in the space's
    /// indexes, so every one of them is restated on the way in.
    fn neighbours(
        &self,
        changes: &[Change],
    ) -> Vec<NodeId> {
        let mut found: IndexSet<NodeId> = IndexSet::new();
        for change in changes {
            match change {
                Change::InsertNode { node, .. } => {
                    self.name_node(&mut found, node.node_id());
                    for reference in node.references() {
                        self.name_reference(&mut found, reference);
                    }
                }
                Change::RemoveNode { node_id } => {
                    if let Some(space_id) = self.space.to_space_node_id(SetId::PRIMARY, node_id) {
                        found.extend(
                            self.space
                                .references(&space_id)
                                .into_iter()
                                .map(|view| view.other),
                        );
                        found.insert(space_id);
                    }
                }
                Change::InsertReference { holder, reference, .. }
                | Change::ReplaceReference { holder, reference, .. } => {
                    self.name_node(&mut found, holder);
                    self.name_reference(&mut found, reference);
                    if let Some(reference) = self.stated_reference(holder, change) {
                        self.name_reference(&mut found, reference);
                    }
                }
                Change::RemoveReference { holder, .. } => {
                    self.name_node(&mut found, holder);
                    if let Some(reference) = self.stated_reference(holder, change) {
                        self.name_reference(&mut found, reference);
                    }
                }
                Change::SetField { node_id, .. } => {
                    self.name_node(&mut found, node_id);
                }
                _ => {}
            }
        }
        found
            .into_iter()
            .filter(|node_id| self.space.set_of(node_id) == Some(SetId::PRIMARY))
            .collect()
    }

    fn name_node(
        &self,
        found: &mut IndexSet<NodeId>,
        local: &NodeId,
    ) {
        if let Some(space_id) = self.space.to_space_node_id(SetId::PRIMARY, local) {
            found.insert(space_id);
        }
    }

    fn stated_reference(
        &self,
        holder: &NodeId,
        change: &Change,
    ) -> Option<&Reference> {
        let position = match change {
            Change::RemoveReference { position, .. } | Change::ReplaceReference { position, .. } => *position,
            _ => return None,
        };
        self.space
            .primary()
            .node(holder)?
            .references()
            .get(position)
    }

    fn name_reference(
        &self,
        found: &mut IndexSet<NodeId>,
        reference: &Reference,
    ) {
        for part in [&reference.reference_type, &reference.target] {
            if let Some(node_id) = self.space.primary().resolve(part) {
                self.name_node(found, node_id);
            }
        }
    }

    fn note_introduced(&mut self) {
        self.introduced = introduced_errors(&self.diagnostics);
    }

    /// The error findings the last change left behind that were not there before it.
    ///
    /// The engine only checks a reference from a node of the editable set, so the references the
    /// operation stated are judged here as well, whichever nodeset their source belongs to.
    fn new_errors(
        &self,
        references: &[ReferenceKey],
    ) -> Vec<Finding> {
        let mut found: Vec<Finding> = self
            .diagnostics
            .findings
            .iter()
            .filter(|finding| finding.is_error() && finding.origin == Origin::Introduced)
            .filter(|finding| !self.introduced.contains(&finding.fingerprint))
            .cloned()
            .collect();
        for key in references {
            let stated = reference_findings(&self.space, &key.source, &key.reference_type, &key.target);
            merge(&mut found, stated.into_iter().filter(Finding::is_error).collect());
        }
        found
    }

    /// Attributes the findings an override let through to it, and says whether that changed the
    /// acknowledgement set the sidecar keeps.
    fn attribute(
        &mut self,
        findings: &[Finding],
        reason: Option<String>,
    ) -> bool {
        if findings.is_empty() {
            return false;
        }
        let fingerprints: Vec<Fingerprint> = findings
            .iter()
            .map(|finding| finding.fingerprint.clone())
            .collect();
        let changed = self.engine.attribute_to_override(&fingerprints, reason);
        self.diagnostics = self.engine.diagnostics();
        changed
    }

    /// The findings that are there now and were not before, whatever their severity.
    fn since(
        &self,
        before: &IndexSet<Fingerprint>,
    ) -> Vec<Finding> {
        self.diagnostics
            .findings
            .iter()
            .filter(|finding| !before.contains(&finding.fingerprint))
            .cloned()
            .collect()
    }

    fn unchanged(
        &self,
        label: String,
    ) -> Applied {
        Applied {
            label,
            deltas: Vec::new(),
            diagnostics: self.diagnostics.clone(),
            created: Vec::new(),
            via_override: false,
            overridden: Vec::new(),
            introduced: Vec::new(),
            acknowledgements_changed: false,
        }
    }
}

fn fingerprints(diagnostics: &Diagnostics) -> IndexSet<Fingerprint> {
    diagnostics
        .findings
        .iter()
        .map(|finding| finding.fingerprint.clone())
        .collect()
}

fn introduced_errors(diagnostics: &Diagnostics) -> IndexSet<Fingerprint> {
    diagnostics
        .findings
        .iter()
        .filter(|finding| finding.is_error() && finding.origin == Origin::Introduced)
        .map(|finding| finding.fingerprint.clone())
        .collect()
}

fn merge(
    into: &mut Vec<Finding>,
    findings: Vec<Finding>,
) {
    for finding in findings {
        if !into
            .iter()
            .any(|kept| kept.fingerprint == finding.fingerprint)
        {
            into.push(finding);
        }
    }
}

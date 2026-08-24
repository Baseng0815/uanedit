use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::rules::acknowledge::{
    Acknowledgement,
    Acknowledgements,
    Baseline,
};
use crate::rules::checks;
use crate::rules::code::{
    DiagnosticCode,
    Severity,
};
use crate::rules::finding::{
    Finding,
    Origin,
};
use crate::rules::fingerprint::Fingerprint;
use crate::rules::rule::{
    Impact,
    Rule,
};
use crate::space::delta::Delta;
use crate::space::{
    AddressSpace,
    SetId,
};
use crate::types::node_id::NodeId;

/// Which nodesets the engine reports on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    /// The editable nodeset only, which is the only one a finding is actionable in.
    #[default]
    Primary,
    /// Every loaded nodeset, for auditing a dependency.
    AllSets,
}

/// How many findings there are, split the way the validation panel shows them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingCounts {
    pub errors: usize,
    pub warnings: usize,
    pub acknowledged: usize,
    pub introduced: usize,
}

/// A snapshot of the findings, which is what crosses to the browser.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostics {
    pub findings: Vec<Finding>,
    pub counts: FindingCounts,
}

impl Diagnostics {
    pub fn errors(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|finding| finding.is_error())
    }

    pub fn for_node<'a>(
        &'a self,
        node_id: &'a NodeId,
    ) -> impl Iterator<Item = &'a Finding> {
        self.findings
            .iter()
            .filter(move |finding| finding.anchor.node == *node_id)
    }
}

/// The one encoding of the OPC 10000-3 and 10000-5 constraints: report mode over a loaded graph,
/// incremental re-checks from an edit delta, and the predicates the pickers and operations query.
pub struct RuleEngine {
    rules: Vec<Box<dyn Rule>>,
    scope: Scope,
    findings: IndexMap<(DiagnosticCode, NodeId), Vec<Finding>>,
    baseline: Baseline,
    baselined: bool,
    acknowledgements: Acknowledgements,
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleEngine {
    pub fn new() -> Self {
        Self {
            rules: checks::all(),
            scope: Scope::default(),
            findings: IndexMap::new(),
            baseline: Baseline::default(),
            baselined: false,
            acknowledgements: Acknowledgements::default(),
        }
    }

    pub fn with_scope(
        scope: Scope,
        acknowledgements: Acknowledgements,
    ) -> Self {
        Self {
            scope,
            acknowledgements,
            ..Self::new()
        }
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn baseline(&self) -> &Baseline {
        &self.baseline
    }

    pub fn acknowledgements(&self) -> &Acknowledgements {
        &self.acknowledgements
    }

    /// Runs every rule over the loaded graph and takes the result as the file's baseline, so
    /// anything that appears later is this editor's doing.
    pub fn open(
        &mut self,
        space: &AddressSpace,
    ) -> Diagnostics {
        self.report(space);
        let baseline: Baseline = self.fingerprints().cloned().collect();
        self.baseline = baseline;
        self.baselined = true;
        self.stamp();
        self.diagnostics()
    }

    /// Runs every rule over the loaded graph without touching the baseline.
    pub fn report(
        &mut self,
        space: &AddressSpace,
    ) -> Diagnostics {
        let subjects = self.subjects(space);
        self.findings.clear();
        let mut produced = Vec::new();
        for rule in &self.rules {
            produced.clear();
            rule.report(space, &subjects, &mut produced);
            store(&mut self.findings, produced.drain(..));
        }
        self.stamp();
        self.diagnostics()
    }

    /// Re-checks only what the reported changes can have moved.
    pub fn update(
        &mut self,
        space: &AddressSpace,
        deltas: &[Delta],
    ) -> Diagnostics {
        for delta in deltas {
            if let Delta::NodeRemoved { node_id } = delta {
                self.findings.retain(|(_, anchored), _| anchored != node_id);
            }
        }
        let subjects = self.subjects(space);
        let mut produced = Vec::new();
        for rule in &self.rules {
            let mut impact = Impact::default();
            for delta in deltas {
                rule.impact(space, delta, &mut impact);
            }
            if impact.is_empty() {
                continue;
            }
            produced.clear();
            match impact.is_whole_space() {
                true => {
                    self.findings.retain(|(code, _), _| *code != rule.code());
                    rule.report(space, &subjects, &mut produced);
                }
                false => {
                    for node_id in impact.iter() {
                        self.findings.shift_remove(&(rule.code(), node_id.clone()));
                        if in_scope(self.scope, space, node_id) {
                            rule.check(space, node_id, &mut produced);
                        }
                    }
                }
            }
            store(&mut self.findings, produced.drain(..));
        }
        self.stamp();
        self.diagnostics()
    }

    /// Runs every rule over these nodes again and replaces what they found about them.
    ///
    /// An edit can move a node's findings without any delta naming it — the reference that made it
    /// a neighbour is the one the edit removed — and there is no delta that means "look again".
    pub fn recheck(
        &mut self,
        space: &AddressSpace,
        nodes: &[NodeId],
    ) -> Diagnostics {
        let mut produced = Vec::new();
        for rule in &self.rules {
            for node_id in nodes {
                self.findings.shift_remove(&(rule.code(), node_id.clone()));
                if in_scope(self.scope, space, node_id) {
                    rule.check(space, node_id, &mut produced);
                }
            }
            store(&mut self.findings, produced.drain(..));
        }
        self.stamp();
        self.diagnostics()
    }

    /// Mutes a finding the user has reviewed; it stays counted and returns if the facts change.
    pub fn acknowledge(
        &mut self,
        acknowledgement: Acknowledgement,
    ) {
        self.acknowledgements.insert(acknowledgement);
        self.stamp();
    }

    pub fn unacknowledge(
        &mut self,
        fingerprint: &Fingerprint,
    ) {
        self.acknowledgements.remove(fingerprint);
        self.stamp();
    }

    pub fn set_acknowledgements(
        &mut self,
        acknowledgements: Acknowledgements,
    ) {
        self.acknowledgements = acknowledgements;
        self.stamp();
    }

    /// Drops the acknowledgements no finding matches any more, which is what the sidecar keeps.
    pub fn prune_acknowledgements(&mut self) {
        let live: Vec<Fingerprint> = self.fingerprints().cloned().collect();
        self.acknowledgements.retain_live(live.iter());
    }

    /// Records that an override produced these findings: they show as introduced, acknowledged and
    /// attributed, which is what guardrails §5 asks of the escape hatch.
    ///
    /// Returns whether that moved the acknowledgement set, which is what the sidecar persists.
    pub fn attribute_to_override(
        &mut self,
        fingerprints: &[Fingerprint],
        reason: Option<String>,
    ) -> bool {
        let mut changed = false;
        for fingerprint in fingerprints {
            let mut acknowledgement = Acknowledgement::new(fingerprint.clone());
            acknowledgement.reason = reason.clone();
            let previous = self.acknowledgements.insert(acknowledgement.clone());
            changed |= previous.as_ref() != Some(&acknowledgement);
        }
        self.stamp();
        for findings in self.findings.values_mut() {
            for finding in findings
                .iter_mut()
                .filter(|f| fingerprints.contains(&f.fingerprint))
            {
                finding.via_override = true;
            }
        }
        changed
    }

    pub fn diagnostics(&self) -> Diagnostics {
        let mut findings: Vec<Finding> = self.findings.values().flatten().cloned().collect();
        findings.sort_by(|left, right| {
            left.anchor
                .node
                .cmp(&right.anchor.node)
                .then_with(|| left.code.id().cmp(right.code.id()))
                .then_with(|| left.fingerprint.cmp(&right.fingerprint))
        });
        let counts = FindingCounts {
            errors: findings
                .iter()
                .filter(|f| f.severity == Severity::Error)
                .count(),
            warnings: findings
                .iter()
                .filter(|f| f.severity == Severity::Warning)
                .count(),
            acknowledged: findings.iter().filter(|f| f.acknowledged).count(),
            introduced: findings
                .iter()
                .filter(|f| f.origin == Origin::Introduced)
                .count(),
        };
        Diagnostics { findings, counts }
    }

    fn subjects(
        &self,
        space: &AddressSpace,
    ) -> Vec<NodeId> {
        match self.scope {
            Scope::Primary => space.node_ids_of(SetId::PRIMARY).collect(),
            Scope::AllSets => space.node_ids().cloned().collect(),
        }
    }

    fn fingerprints(&self) -> impl Iterator<Item = &Fingerprint> {
        self.findings
            .values()
            .flatten()
            .map(|finding| &finding.fingerprint)
    }

    /// Marks each finding as inherited or introduced, and as acknowledged or not.
    fn stamp(&mut self) {
        for findings in self.findings.values_mut() {
            for finding in findings {
                finding.origin = match self.baselined && self.baseline.contains(&finding.fingerprint) {
                    true => Origin::Inherited,
                    false => Origin::Introduced,
                };
                finding.acknowledged = self.acknowledgements.contains(&finding.fingerprint);
            }
        }
    }
}

/// Whether a node is one this engine reports on, which an incremental re-check has to respect as
/// much as a whole pass does.
fn in_scope(
    scope: Scope,
    space: &AddressSpace,
    node_id: &NodeId,
) -> bool {
    match scope {
        Scope::Primary => space.set_of(node_id) == Some(SetId::PRIMARY),
        Scope::AllSets => space.contains(node_id),
    }
}

fn store(
    findings: &mut IndexMap<(DiagnosticCode, NodeId), Vec<Finding>>,
    produced: impl Iterator<Item = Finding>,
) {
    for finding in produced {
        findings
            .entry((finding.code, finding.anchor.node.clone()))
            .or_default()
            .push(finding);
    }
}

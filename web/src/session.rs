//! The open document the browser owns: one [`Session`], and the signals the editor renders from.
//!
//! The session sits outside the reactive graph — it is neither `Clone` nor cheap — so the views
//! subscribe to the counters beside it instead. Every edit bumps `revision`, which is what the
//! inspector reads; only an edit that moved the graph or changed a name a row shows bumps
//! `structure`, which is what the tree reads. That split is what keeps a ValueRank edit from
//! re-flattening six thousand rows.

use dioxus::prelude::*;
use uanedit::attributes::attribute_id::AttributeId;
use uanedit::edit::Refusal;
use uanedit::space::{
    AddressSpace,
    Delta,
    NodeField,
};
use uanedit::types::node_id::NodeId;
use uanedit::{
    Operation,
    Session,
};

use crate::api::{
    DiffPreview,
    OpenedFile,
    VersionNudge,
    diff_preview,
    render_file,
    save_file,
};

/// The handle every part of the editor shares, provided by the app shell so the top app bar can
/// reach the same document the routed view fills.
#[derive(Clone, Copy)]
pub struct EditorHandle {
    session: CopyValue<Option<Session>>,
    pub file: Signal<Option<String>>,
    pub revision: Signal<u64>,
    pub structure: Signal<u64>,
    pub dirty: Signal<bool>,
    pub history: Signal<History>,
    pub selection: Signal<Option<NodeId>>,
    pub status: Signal<Option<Status>>,
    pub busy: Signal<bool>,
    /// The last diff preview, which is what opens the diff dialog.
    pub diff: Signal<Option<DiffPreview>>,
    /// What the last save wants asked about the model's version (features.md §2C).
    pub nudge: Signal<Option<VersionNudge>>,
}

/// What the undo and redo buttons say, as of the last edit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
    pub undo: Option<String>,
    pub redo: Option<String>,
}

/// A short-lived message in the app bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    pub kind: StatusKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Error,
}

/// What one edit did, in the terms a field shows beside itself.
pub enum EditResult {
    /// The model changed, adding this many warning findings.
    Applied {
        warnings: usize,
    },
    /// The value was already what the edit asked for, so nothing moved.
    Unchanged,
    Refused(Refusal),
    /// No document is open, which only happens between routes.
    Closed,
}

/// The state the signals track, read from the session in one pass.
struct Settled {
    dirty: bool,
    history: History,
}

impl Default for EditorHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorHandle {
    pub fn new() -> Self {
        Self {
            session: CopyValue::new(None),
            file: Signal::new(None),
            revision: Signal::new(0),
            structure: Signal::new(0),
            dirty: Signal::new(false),
            history: Signal::new(History::default()),
            selection: Signal::new(None),
            status: Signal::new(None),
            busy: Signal::new(false),
            diff: Signal::new(None),
            nudge: Signal::new(None),
        }
    }

    /// Builds the address space the payload describes and takes it under edit.
    pub fn open(
        mut self,
        opened: &OpenedFile,
    ) {
        let dependencies = opened
            .dependencies
            .iter()
            .map(|dependency| dependency.nodeset.clone());
        let space = AddressSpace::load(opened.primary.clone(), dependencies);
        self.session
            .set(Some(Session::with_acknowledgements(space, opened.acknowledgements.clone())));
        self.file.set(Some(opened.name.clone()));
        self.reset();
    }

    pub fn close(mut self) {
        self.session.set(None);
        self.file.set(None);
        self.reset();
    }

    fn reset(mut self) {
        self.selection.set(None);
        self.status.set(None);
        self.diff.set(None);
        self.nudge.set(None);
        self.dirty.set(false);
        self.history.set(History::default());
        self.bump(true);
    }

    /// Reads the loaded address space. Nothing is subscribed to here, so a caller that has to
    /// re-run reads `revision` or `structure` as well.
    pub fn with_space<R>(
        self,
        read: impl FnOnce(&AddressSpace) -> R,
    ) -> Option<R> {
        Some(read(self.session.read().as_ref()?.space()))
    }

    pub fn apply(
        mut self,
        operation: Operation,
    ) -> EditResult {
        let (empty, structural, warnings, settled) = {
            let mut slot = self.session.write();
            let Some(session) = slot.as_mut() else {
                return EditResult::Closed;
            };
            match session.apply(operation) {
                Err(refusal) => return EditResult::Refused(refusal),
                Ok(applied) => (
                    applied.is_empty(),
                    applied.deltas.iter().any(redraws_tree),
                    applied.introduced_warnings().count(),
                    Settled::read(session),
                ),
            }
        };
        if empty {
            return EditResult::Unchanged;
        }
        self.settle(settled, structural);
        EditResult::Applied { warnings }
    }

    pub fn undo(self) {
        self.step(true);
    }

    pub fn redo(self) {
        self.step(false);
    }

    fn step(
        mut self,
        backwards: bool,
    ) {
        let stepped = {
            let mut slot = self.session.write();
            let Some(session) = slot.as_mut() else {
                return;
            };
            let applied = match backwards {
                true => session.undo(),
                false => session.redo(),
            };
            applied.map(|applied| {
                let structural = applied.deltas.iter().any(redraws_tree);
                (applied.label, structural, Settled::read(session))
            })
        };
        let Some((label, structural, settled)) = stepped else {
            return;
        };
        self.settle(settled, structural);
        self.announce(Status::info(match backwards {
            true => format!("Undid {label}"),
            false => format!("Redid {label}"),
        }));
    }

    /// Writes the edited model back through the server, which splices it into the loaded bytes.
    pub fn save(self) {
        if *self.busy.peek() {
            return;
        }
        let Some(name) = self.file.peek().clone() else {
            return;
        };
        let Some(nodeset) = self
            .session
            .read()
            .as_ref()
            .map(|session| session.primary().clone())
        else {
            return;
        };
        spawn(async move {
            let (mut busy, mut nudge) = (self.busy, self.nudge);
            busy.set(true);
            let outcome = save_file(name, nodeset).await;
            busy.set(false);
            match outcome {
                Ok(outcome) => {
                    self.mark_saved();
                    nudge.set(outcome.version_nudge.clone());
                    self.announce(Status::success(match outcome.changed {
                        true => format!("Saved {} · {} bytes", outcome.name, outcome.bytes),
                        false => format!("Saved {} · {} bytes, byte-identical", outcome.name, outcome.bytes),
                    }));
                }
                Err(error) => self.announce(Status::error(format!("Save failed: {error}"))),
            }
        });
    }

    /// Hands the browser the text a save would write, as a file download.
    pub fn download(self) {
        if *self.busy.peek() {
            return;
        }
        let Some(name) = self.file.peek().clone() else {
            return;
        };
        let Some(nodeset) = self
            .session
            .read()
            .as_ref()
            .map(|session| session.primary().clone())
        else {
            return;
        };
        spawn(async move {
            let mut busy = self.busy;
            busy.set(true);
            let rendered = render_file(name.clone(), nodeset).await;
            busy.set(false);
            let sent = match rendered {
                Ok(text) => document::eval(DOWNLOAD_JS)
                    .send((name.as_str(), text.as_str()))
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            match sent {
                Ok(()) => self.announce(Status::success(format!("Downloaded {name}"))),
                Err(error) => self.announce(Status::error(format!("Download failed: {error}"))),
            }
        });
    }

    /// The minimal diff the save would make against the bytes on disk (features.md §2E).
    pub fn preview(self) {
        if *self.busy.peek() {
            return;
        }
        let Some(name) = self.file.peek().clone() else {
            return;
        };
        let Some(nodeset) = self
            .session
            .read()
            .as_ref()
            .map(|session| session.primary().clone())
        else {
            return;
        };
        spawn(async move {
            let (mut busy, mut diff) = (self.busy, self.diff);
            busy.set(true);
            let preview = diff_preview(name, nodeset).await;
            busy.set(false);
            match preview {
                Ok(preview) => diff.set(Some(preview)),
                Err(error) => self.announce(Status::error(format!("Diff failed: {error}"))),
            }
        });
    }

    fn mark_saved(mut self) {
        let settled = {
            let mut slot = self.session.write();
            let Some(session) = slot.as_mut() else {
                return;
            };
            session.mark_saved();
            Settled::read(session)
        };
        self.dirty.set(settled.dirty);
        self.history.set(settled.history);
    }

    fn settle(
        mut self,
        settled: Settled,
        structural: bool,
    ) {
        self.dirty.set(settled.dirty);
        self.history.set(settled.history);
        self.bump(structural);
    }

    fn bump(
        mut self,
        structural: bool,
    ) {
        self.revision
            .with_mut(|value| *value = value.wrapping_add(1));
        if structural {
            self.structure
                .with_mut(|value| *value = value.wrapping_add(1));
        }
    }

    /// Shows a message and retires it, unless something newer took its place first.
    fn announce(
        mut self,
        status: Status,
    ) {
        self.status.set(Some(status.clone()));
        spawn(async move {
            sleep(STATUS_MILLIS).await;
            if self.status.peek().as_ref() == Some(&status) {
                self.status.set(None);
            }
        });
    }
}

/// Whether the tree has to be built again: the graph moved, or a row's own label changed.
fn redraws_tree(delta: &Delta) -> bool {
    delta.touches_graph()
        || matches!(
            delta,
            Delta::FieldChanged { field, .. }
                if *field == NodeField::BROWSE_NAME || *field == NodeField::Attribute(AttributeId::DisplayName)
        )
}

impl Settled {
    fn read(session: &Session) -> Self {
        Self {
            dirty: session.is_modified(),
            history: History {
                undo: session.undo_label().map(ToOwned::to_owned),
                redo: session.redo_label().map(ToOwned::to_owned),
            },
        }
    }
}

impl Status {
    pub fn info(text: impl Into<String>) -> Self {
        Self::new(text, StatusKind::Info)
    }

    pub fn success(text: impl Into<String>) -> Self {
        Self::new(text, StatusKind::Success)
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self::new(text, StatusKind::Error)
    }

    fn new(
        text: impl Into<String>,
        kind: StatusKind,
    ) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }

    pub fn class(&self) -> &'static str {
        match self.kind {
            StatusKind::Info => "status",
            StatusKind::Success => "status success",
            StatusKind::Error => "status error",
        }
    }
}

const STATUS_MILLIS: u32 = 5000;

/// Receives `(name, text)` and clicks a blob URL, which is how a browser is asked to save a file.
const DOWNLOAD_JS: &str = r#"
    const [name, text] = await dioxus.recv();
    const url = URL.createObjectURL(new Blob([text], { type: "application/xml" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 60000);
"#;

use uanedit::edit::delete::{
    DeleteNode,
    DeletionPlan,
};
use uanedit::rules::acknowledge::{
    Acknowledgement,
    Acknowledgements,
};
use uanedit::rules::finding::Finding;
use uanedit::rules::fingerprint::Fingerprint;

use crate::api::save_acknowledgements;

/// Spawning on the root scope, which the dioxus prelude does not re-export.
use dioxus::core::spawn_forever;

/// What one operation did, for the flows that need more of the answer than a field does: what was
/// created, and what an override let through.
pub enum Outcome {
    Applied(Box<Done>),
    Unchanged,
    Refused(Refusal),
    Closed,
}

pub struct Done {
    pub label: String,
    pub created: Vec<NodeId>,
    /// The error findings an override introduced, empty for an ordinary apply (guardrails.md §5).
    pub overridden: Vec<Finding>,
    /// The findings the operation left behind that were not there before it, warnings included.
    pub introduced: Vec<Finding>,
    pub warnings: usize,
}

impl Done {
    /// What the app bar says about it, warnings included, since a warning is permitted but never
    /// silent (guardrails.md §2).
    pub fn status(&self) -> Status {
        if !self.overridden.is_empty() {
            return Status::error(format!("{} · overridden, see Validation", self.label));
        }
        match self.warnings {
            0 => Status::success(self.label.clone()),
            1 => Status::info(format!("{} · one new warning", self.label)),
            count => Status::info(format!("{} · {count} new warnings", self.label)),
        }
    }
}

impl EditorHandle {
    pub fn perform(
        self,
        operation: Operation,
    ) -> Outcome {
        self.run(operation, None)
    }

    /// Performs an operation the engine refuses, attributing what it leaves to the override.
    pub fn perform_with_override(
        self,
        operation: Operation,
        reason: Option<String>,
    ) -> Outcome {
        self.run(operation, Some(reason))
    }

    fn run(
        mut self,
        operation: Operation,
        override_reason: Option<Option<String>>,
    ) -> Outcome {
        let (done, structural, settled, attributed) = {
            let mut slot = self.session.write();
            let Some(session) = slot.as_mut() else {
                return Outcome::Closed;
            };
            let applied = match override_reason {
                Some(reason) => session.apply_with_override(operation, reason),
                None => session.apply(operation),
            };
            match applied {
                Err(refusal) => return Outcome::Refused(refusal),
                Ok(applied) if applied.is_empty() => return Outcome::Unchanged,
                Ok(applied) => {
                    let structural = applied.deltas.iter().any(redraws_tree);
                    let warnings = applied.introduced_warnings().count();
                    // The override acknowledges what it let through; the sidecar keeps the reason.
                    let attributed = applied
                        .acknowledgements_changed
                        .then(|| session.acknowledgements().clone());
                    let done = Done {
                        label: applied.label,
                        created: applied.created,
                        overridden: applied.overridden,
                        introduced: applied.introduced,
                        warnings,
                    };
                    (done, structural, Settled::read(session), attributed)
                }
            }
        };
        self.settle(settled, structural);
        if let Some(acknowledgements) = attributed {
            self.persist(acknowledgements);
        }
        Outcome::Applied(Box::new(done))
    }

    /// Reads the open document. Nothing is subscribed to here, so a caller that has to re-run reads
    /// `revision` as well.
    pub fn with_session<R>(
        self,
        read: impl FnOnce(&Session) -> R,
    ) -> Option<R> {
        Some(read(self.session.read().as_ref()?))
    }

    /// What the deletion still has to answer for, given the resolutions it carries so far.
    pub fn deletion_plan(
        self,
        delete: &DeleteNode,
    ) -> Option<DeletionPlan> {
        self.with_session(|session| session.deletion_plan(delete))
    }

    pub fn acknowledge(
        mut self,
        fingerprint: Fingerprint,
        reason: Option<String>,
    ) {
        let acknowledgements = {
            let mut slot = self.session.write();
            let Some(session) = slot.as_mut() else {
                return;
            };
            let mut acknowledgement = Acknowledgement::new(fingerprint);
            acknowledgement.reason = reason;
            session.acknowledge(acknowledgement);
            session.acknowledgements().clone()
        };
        self.bump(false);
        self.persist(acknowledgements);
    }

    pub fn unacknowledge(
        mut self,
        fingerprint: &Fingerprint,
    ) {
        let acknowledgements = {
            let mut slot = self.session.write();
            let Some(session) = slot.as_mut() else {
                return;
            };
            session.unacknowledge(fingerprint);
            session.acknowledgements().clone()
        };
        self.bump(false);
        self.persist(acknowledgements);
    }

    /// Writes the acknowledgements to the sidecar beside the nodeset (guardrails.md §4).
    ///
    /// On the root scope rather than the caller's: an override acknowledges from a dialog that
    /// closes in the same update, and a task belonging to a scope that is going away is cancelled.
    fn persist(
        self,
        acknowledgements: Acknowledgements,
    ) {
        let Some(name) = self.file.peek().clone() else {
            return;
        };
        spawn_forever(async move {
            if let Err(error) = save_acknowledgements(name, acknowledgements).await {
                self.announce(Status::error(format!("Acknowledgements not saved: {error}")));
            }
        });
    }

    pub fn say(
        self,
        status: Status,
    ) {
        self.announce(status);
    }
}

/// The one timer the editor needs. Native builds never reach a debounce or a status timeout, since
/// only the browser half runs event handlers.
pub async fn sleep(millis: u32) {
    #[cfg(target_arch = "wasm32")]
    gloo_timers::future::TimeoutFuture::new(millis).await;
    #[cfg(not(target_arch = "wasm32"))]
    let _ = millis;
}

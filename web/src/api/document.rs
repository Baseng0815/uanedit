use dioxus::prelude::*;
use serde::{
    Deserialize,
    Serialize,
};
use uanedit::NodeSet;
use uanedit::report::OpenReport;
use uanedit::rules::Acknowledgements;

use crate::api::WorkspaceFile;

/// Everything the browser needs to build its own address space for one file.
///
/// Editing is client-authoritative: the server ships the parsed nodesets and keeps only the
/// document's bytes, so a save is the edited model coming back to be spliced into them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenedFile {
    pub name: String,
    pub primary: NodeSet,
    /// Every model the file requires, transitively, that the workspace could supply.
    pub dependencies: Vec<LoadedDependency>,
    pub report: OpenReport,
    pub acknowledgements: Acknowledgements,
    /// The standard nodeset is not in the workspace, so references into namespace 0 dangle.
    pub missing_ns0: bool,
    pub unresolved_models: Vec<UnresolvedModel>,
    /// Requirements the workspace satisfied with a different version than the file asked for.
    pub version_checks: Vec<VersionCheck>,
    pub timing: OpenTiming,
}

/// One dependency file, parsed. Keyed by file rather than by model, because one file may define
/// several models and shipping it twice would double a multi-megabyte payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadedDependency {
    pub model_uri: String,
    pub file_name: String,
    pub nodeset: NodeSet,
}

/// A required model no file in the workspace defines.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedModel {
    pub model_uri: String,
    /// The model that named it, or `None` when it is namespace 0, which every file needs.
    pub required_by: Option<String>,
    pub required_version: Option<String>,
    pub required_publication_date: Option<String>,
}

/// A resolved requirement whose version or publication date differs from what was asked for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCheck {
    pub model_uri: String,
    pub file_name: String,
    pub required_version: Option<String>,
    pub found_version: Option<String>,
    pub required_publication_date: Option<String>,
    pub found_publication_date: Option<String>,
    /// False when the found publication date predates the required one.
    pub satisfied: bool,
}

/// Where the time went, so the cost of opening a large nodeset is visible rather than guessed at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenTiming {
    pub parse_millis: u64,
    pub index_millis: u64,
    pub dependencies_millis: u64,
    pub total_millis: u64,
    /// The registry answered from what it already held, so nothing was read and every count is zero.
    pub already_open: bool,
}

/// What a save did, and what the model table should probably say about it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveOutcome {
    pub name: String,
    pub bytes: usize,
    /// False when the written text matched the bytes already on disk, which is the round trip.
    pub changed: bool,
    pub version_nudge: Option<VersionNudge>,
    /// The report from re-reading what was written, so the client tracks the new on-disk state.
    pub report: OpenReport,
}

/// The model's content changed and its version did not (features.md §2C).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionNudge {
    pub model_uri: String,
    pub model_version: Option<String>,
    pub publication_date: Option<String>,
}

/// The would-be save as a diff against the bytes on disk (features.md §2E).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffPreview {
    pub name: String,
    pub changed: bool,
    pub added: usize,
    pub removed: usize,
    pub hunks: Vec<DiffHunk>,
    /// The diff was too large to ship whole and the hunks stop early.
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[server]
pub async fn open_file(name: String) -> ServerFnResult<OpenedFile> {
    use dioxus::prelude::ServerFnError;

    use crate::server::documents;

    documents::open(&name).map_err(ServerFnError::new)
}

/// Axum caps a request body at two megabytes by default; an edited nodeset on the way back is
/// larger than that as soon as the model is — the standard nodeset alone is four megabytes of XML
/// and nine as JSON. Only the middleware, which is server-only, reads this.
#[cfg(feature = "server")]
const BODY_LIMIT: usize = 512 * 1024 * 1024;

#[server]
#[middleware(dioxus::server::axum::extract::DefaultBodyLimit::max(BODY_LIMIT))]
pub async fn save_file(
    name: String,
    nodeset: NodeSet,
) -> ServerFnResult<SaveOutcome> {
    use dioxus::prelude::ServerFnError;

    use crate::server::documents;

    documents::save(&name, &nodeset).map_err(ServerFnError::new)
}

/// The text a save would write, for the download button — deployed on the web, the workspace
/// directory is the server's, so this is how a file leaves it.
#[server]
#[middleware(dioxus::server::axum::extract::DefaultBodyLimit::max(BODY_LIMIT))]
pub async fn render_file(
    name: String,
    nodeset: NodeSet,
) -> ServerFnResult<String> {
    use dioxus::prelude::ServerFnError;

    use crate::server::documents;

    documents::render(&name, &nodeset).map_err(ServerFnError::new)
}

#[server]
#[middleware(dioxus::server::axum::extract::DefaultBodyLimit::max(BODY_LIMIT))]
pub async fn diff_preview(
    name: String,
    nodeset: NodeSet,
) -> ServerFnResult<DiffPreview> {
    use dioxus::prelude::ServerFnError;

    use crate::server::documents;

    documents::diff(&name, &nodeset).map_err(ServerFnError::new)
}

#[server]
pub async fn create_file(
    name: String,
    model_uri: String,
    version: String,
) -> ServerFnResult<WorkspaceFile> {
    use dioxus::prelude::ServerFnError;

    use crate::server::documents;

    documents::create(&name, &model_uri, &version).map_err(ServerFnError::new)
}

#[server]
pub async fn save_acknowledgements(
    name: String,
    acknowledgements: Acknowledgements,
) -> ServerFnResult<()> {
    use dioxus::prelude::ServerFnError;

    use crate::server::documents;

    documents::set_acknowledgements(&name, acknowledgements).map_err(ServerFnError::new)
}

#[server]
pub async fn close_file(name: String) -> ServerFnResult<bool> {
    use dioxus::prelude::ServerFnError;

    use crate::server::documents;

    documents::close(&name).map_err(ServerFnError::new)
}

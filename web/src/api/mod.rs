//! Server functions — the wire. Signatures compile into both halves; only the bodies are
//! server-only, so server-only imports go inside them.
//!
//! The re-exports are allowed to be unused: this module is the whole wire, and which half of it a
//! given build reaches for depends on the feature and on which views have landed.
#![allow(unused_imports, reason = "the wire is complete before the views that consume it are")]

mod clock;
mod document;
mod workspace;

pub use clock::current_time;
pub use document::{
    DiffHunk,
    DiffLine,
    DiffLineKind,
    DiffPreview,
    LoadedDependency,
    OpenTiming,
    OpenedFile,
    SaveOutcome,
    UnresolvedModel,
    VersionCheck,
    VersionNudge,
    close_file,
    create_file,
    diff_preview,
    open_file,
    save_acknowledgements,
    save_file,
};
pub use workspace::{
    Workspace,
    WorkspaceFile,
    list_workspace,
};

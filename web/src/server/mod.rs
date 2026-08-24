//! Server-only code: the filesystem side of the app, never compiled into the browser bundle.

pub mod acknowledgements;
pub mod diff;
pub mod documents;
pub mod models;
pub mod workspace;

use thiserror::Error;
use uanedit::error::DocumentError;

/// Everything the filesystem side can refuse, in the words the user sees.
#[derive(Debug, Error)]
pub enum ServerError {
    #[error("`{0}` is not a file name")]
    BadName(String),
    #[error("`{0}` is not in the workspace")]
    NotFound(String),
    #[error("`{0}` is not open")]
    NotOpen(String),
    #[error("`{0}` already exists")]
    Exists(String),
    #[error("`{path}` could not be read: {source}")]
    Read { path: String, source: std::io::Error },
    #[error("`{path}` could not be written: {source}")]
    Write { path: String, source: std::io::Error },
    #[error("`{path}` is not a nodeset: {source}")]
    Document { path: String, source: DocumentError },
    #[error("the acknowledgements beside `{path}` are not readable: {source}")]
    Sidecar { path: String, source: serde_json::Error },
    #[error("the open-document registry was left locked by a panicking request; restart the server")]
    Poisoned,
}

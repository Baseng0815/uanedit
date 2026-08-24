//! The acknowledgements sidecar: `<file name>.acks.json` beside the nodeset.
//!
//! Beside rather than inside, because writing them into the nodeset would pollute the diff the
//! round trip exists to keep clean, and dependency files are read-only anyway (guardrails.md §4).
//! JSON next to the file it belongs to, so it is committed with the model it annotates.

use std::path::{
    Path,
    PathBuf,
};

use uanedit::rules::Acknowledgements;

use crate::server::{
    ServerError,
    workspace,
};

const SUFFIX: &str = ".acks.json";

pub fn sidecar(nodeset: &Path) -> PathBuf {
    let mut name = nodeset.as_os_str().to_owned();
    name.push(SUFFIX);
    PathBuf::from(name)
}

/// An absent sidecar is an empty set; an unreadable one is an error, because silently discarding
/// acknowledgements the user wrote would be worse than refusing to open the file.
pub fn load(nodeset: &Path) -> Result<Acknowledgements, ServerError> {
    let path = sidecar(nodeset);
    if !path.is_file() {
        return Ok(Acknowledgements::default());
    }
    let source = workspace::read(&path)?;
    serde_json::from_str(&source).map_err(|source| ServerError::Sidecar {
        path: path.display().to_string(),
        source,
    })
}

/// An empty set removes the sidecar rather than leaving `{}` in the tree.
pub fn store(
    nodeset: &Path,
    acknowledgements: &Acknowledgements,
) -> Result<(), ServerError> {
    let path = sidecar(nodeset);
    if acknowledgements.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    let text = serde_json::to_string_pretty(acknowledgements).map_err(|source| ServerError::Sidecar {
        path: path.display().to_string(),
        source,
    })?;
    workspace::write(&path, &format!("{text}\n"))
}

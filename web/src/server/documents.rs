//! The open-document registry: what the server keeps while the browser does the editing.
//!
//! Editing is client-authoritative. The browser gets the parsed nodesets and builds its own
//! address space; what stays here is the [`Document`] — the file's bytes and where every element
//! sat in them — because that is what makes a save splice rather than reflow (architecture.md §3).
//! A save is the edited model coming back to be written against those bytes.
//!
//! One process, one user (architecture.md §7.3), so one mutex over a map is the whole story. The
//! lock is held across a parse, which makes the second of two concurrent opens of the same file
//! wait for the first and then find it already open rather than parse it again.

use std::collections::HashMap;
use std::sync::{
    LazyLock,
    Mutex,
    MutexGuard,
};
use std::time::Instant;

use uanedit::nodeset::{
    AliasTable,
    ModelTableEntry,
};
use uanedit::rules::Acknowledgements;
use uanedit::xml::Document;
use uanedit::{
    NodeSet,
    xml,
};

use crate::api::{
    DiffPreview,
    LoadedDependency,
    OpenTiming,
    OpenedFile,
    SaveOutcome,
    UnresolvedModel,
    VersionCheck,
    VersionNudge,
    WorkspaceFile,
};
use crate::server::{
    ServerError,
    acknowledgements,
    diff,
    models,
    workspace,
};

/// One open file: the bytes, what the workspace resolved for it, and its acknowledgements.
struct OpenDocument {
    document: Document,
    dependencies: Vec<LoadedDependency>,
    acknowledgements: Acknowledgements,
    missing_ns0: bool,
    unresolved: Vec<UnresolvedModel>,
    version_checks: Vec<VersionCheck>,
    timing: OpenTiming,
}

static REGISTRY: LazyLock<Mutex<HashMap<String, OpenDocument>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn registry() -> Result<MutexGuard<'static, HashMap<String, OpenDocument>>, ServerError> {
    REGISTRY.lock().map_err(|_| ServerError::Poisoned)
}

/// Opening a file that is already open returns the held state rather than parsing it again.
///
/// The sidecar is re-read even then: it is small, it lives beside the file, and an acknowledgement
/// deleted on disk between two opens has to be gone here too (guardrails.md §4).
pub fn open(name: &str) -> Result<OpenedFile, ServerError> {
    let name = workspace::file_name(name)?;
    let path = workspace::path(&name)?;
    let mut registry = registry()?;

    if let Some(open) = registry.get_mut(&name) {
        open.acknowledgements = acknowledgements::load(&path)?;
        let timing = OpenTiming {
            already_open: true,
            ..OpenTiming::default()
        };
        return Ok(payload(&name, open, timing));
    }

    let started = Instant::now();
    let open = load(&name)?;
    let timing = OpenTiming {
        total_millis: millis(started),
        ..open.timing
    };
    let opened = payload(&name, &open, timing);
    registry.insert(name, open);
    Ok(opened)
}

pub fn close(name: &str) -> Result<bool, ServerError> {
    let name = workspace::file_name(name)?;
    Ok(registry()?.remove(&name).is_some())
}

/// Writes the edited model against the bytes the open document holds, then re-reads what was
/// written so the held document tracks the new on-disk state.
pub fn save(
    name: &str,
    nodeset: &NodeSet,
) -> Result<SaveOutcome, ServerError> {
    let name = workspace::file_name(name)?;
    let path = workspace::path(&name)?;
    let mut registry = registry()?;
    let open = registry
        .get_mut(&name)
        .ok_or_else(|| ServerError::NotOpen(name.clone()))?;

    let written = open.document.write_nodeset(nodeset);
    let changed = written != open.document.source();
    let version_nudge = version_nudge(open.document.nodeset(), nodeset);

    workspace::write(&path, &written)?;
    let document = xml::parse(&written).map_err(|source| ServerError::Document {
        path: path.display().to_string(),
        source,
    })?;
    let report = document.report().clone();
    open.document = document;

    Ok(SaveOutcome {
        name,
        bytes: written.len(),
        changed,
        version_nudge,
        report,
    })
}

/// The text a save would write, without writing it.
pub fn render(
    name: &str,
    nodeset: &NodeSet,
) -> Result<String, ServerError> {
    let name = workspace::file_name(name)?;
    let registry = registry()?;
    let open = registry
        .get(&name)
        .ok_or_else(|| ServerError::NotOpen(name.clone()))?;
    Ok(open.document.write_nodeset(nodeset))
}

pub fn diff(
    name: &str,
    nodeset: &NodeSet,
) -> Result<DiffPreview, ServerError> {
    let name = workspace::file_name(name)?;
    let registry = registry()?;
    let open = registry
        .get(&name)
        .ok_or_else(|| ServerError::NotOpen(name.clone()))?;

    let written = open.document.write_nodeset(nodeset);
    Ok(diff::preview(&name, open.document.source(), &written))
}

pub fn set_acknowledgements(
    name: &str,
    acknowledgements: Acknowledgements,
) -> Result<(), ServerError> {
    let name = workspace::file_name(name)?;
    let path = workspace::path(&name)?;
    acknowledgements::store(&path, &acknowledgements)?;
    if let Some(open) = registry()?.get_mut(&name) {
        open.acknowledgements = acknowledgements;
    }
    Ok(())
}

/// A new nodeset with nothing in it but the tables that make it a valid document.
pub fn create(
    name: &str,
    model_uri: &str,
    version: &str,
) -> Result<WorkspaceFile, ServerError> {
    let name = workspace::file_name(&with_xml_extension(name))?;
    let path = workspace::path(&name)?;
    if path.exists() {
        return Err(ServerError::Exists(name));
    }

    workspace::write(&path, &xml::write(&blank(model_uri, version)))?;
    Ok(workspace::describe(&path))
}

fn blank(
    model_uri: &str,
    version: &str,
) -> NodeSet {
    let model = ModelTableEntry {
        version: Some(version.to_owned()),
        publication_date: Some(workspace::now()),
        // Mandatory since spec 1.05.03, and there is no second version to give it yet.
        model_version: Some(version.to_owned()),
        ..ModelTableEntry::new(model_uri)
    };
    NodeSet::from_parts(
        None,
        vec![model_uri.to_owned()].into(),
        Vec::new(),
        vec![model],
        AliasTable::default(),
        Vec::new(),
        [],
    )
}

fn with_xml_extension(name: &str) -> String {
    match name.to_lowercase().ends_with(".xml") {
        true => name.to_owned(),
        false => format!("{name}.NodeSet2.xml"),
    }
}

fn load(name: &str) -> Result<OpenDocument, ServerError> {
    let path = workspace::path(name)?;
    if !path.is_file() {
        return Err(ServerError::NotFound(name.to_owned()));
    }

    let source = workspace::read(&path)?;
    let parsing = Instant::now();
    let document = xml::parse(&source).map_err(|source| ServerError::Document {
        path: path.display().to_string(),
        source,
    })?;
    let parse_millis = millis(parsing);

    let resolution = models::resolve(document.nodeset(), name);
    Ok(OpenDocument {
        document,
        dependencies: resolution.dependencies,
        acknowledgements: acknowledgements::load(&path)?,
        missing_ns0: resolution.missing_ns0,
        unresolved: resolution.unresolved,
        version_checks: resolution.version_checks,
        timing: OpenTiming {
            parse_millis,
            index_millis: resolution.index_millis,
            dependencies_millis: resolution.dependencies_millis,
            total_millis: 0,
            already_open: false,
        },
    })
}

fn payload(
    name: &str,
    open: &OpenDocument,
    timing: OpenTiming,
) -> OpenedFile {
    OpenedFile {
        name: name.to_owned(),
        primary: open.document.nodeset().clone(),
        dependencies: open.dependencies.clone(),
        report: open.document.report().clone(),
        acknowledgements: open.acknowledgements.clone(),
        missing_ns0: open.missing_ns0,
        unresolved_models: open.unresolved.clone(),
        version_checks: open.version_checks.clone(),
        timing,
    }
}

/// The content changed and the version did not, which is the one thing a save should ask about.
fn version_nudge(
    before: &NodeSet,
    after: &NodeSet,
) -> Option<VersionNudge> {
    let content_changed = before.nodes != after.nodes
        || before.aliases != after.aliases
        || before.namespaces != after.namespaces
        || before.server_uris != after.server_uris
        || before.extensions != after.extensions;
    if !content_changed {
        return None;
    }

    let (was, now) = (before.model()?, after.model()?);
    if was.model_version != now.model_version || was.publication_date != now.publication_date {
        return None;
    }

    Some(VersionNudge {
        model_uri: now.model_uri.clone(),
        model_version: now.model_version.clone(),
        publication_date: now.publication_date.as_ref().map(ToString::to_string),
    })
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

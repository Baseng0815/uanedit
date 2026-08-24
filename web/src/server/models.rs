//! Resolving a file's `RequiredModel` entries against the workspace directory.
//!
//! The workspace is the universe (features.md §2C): a dependency is a sibling file that defines
//! the model, found by reading every sibling's tables. Nothing is fetched, and nothing missing is
//! an error — an unresolved model is a fact the payload carries so the UI can say so.

use std::collections::{
    HashSet,
    VecDeque,
};
use std::time::Instant;

use uanedit::nodeset::ModelTableEntry;
use uanedit::{
    NodeSet,
    ids,
    xml,
};

use crate::api::{
    LoadedDependency,
    UnresolvedModel,
    VersionCheck,
};
use crate::server::workspace;

pub struct Resolution {
    pub dependencies: Vec<LoadedDependency>,
    pub missing_ns0: bool,
    pub unresolved: Vec<UnresolvedModel>,
    pub version_checks: Vec<VersionCheck>,
    pub index_millis: u64,
    pub dependencies_millis: u64,
}

/// One workspace file's tables, read without its nodes.
struct IndexedFile {
    name: String,
    models: Vec<ModelTableEntry>,
}

impl IndexedFile {
    fn defines(
        &self,
        model_uri: &str,
    ) -> bool {
        self.models.iter().any(|model| model.model_uri == model_uri)
    }

    fn entry(
        &self,
        model_uri: &str,
    ) -> Option<&ModelTableEntry> {
        self.models
            .iter()
            .find(|model| model.model_uri == model_uri)
    }
}

/// A requirement waiting to be resolved, with the model that asked for it.
struct Requirement {
    model_uri: String,
    required_by: Option<String>,
    version: Option<String>,
    publication_date: Option<String>,
}

impl Requirement {
    fn from_entry(
        entry: &ModelTableEntry,
        required_by: &str,
    ) -> Self {
        Self {
            model_uri: entry.model_uri.clone(),
            required_by: Some(required_by.to_owned()),
            version: entry.version.clone(),
            publication_date: entry.publication_date.as_ref().map(ToString::to_string),
        }
    }

    /// Namespace 0 is required by every file whether or not it says so, because the standard
    /// nodes are what a model's references point at (architecture.md §5).
    fn base_namespace() -> Self {
        Self {
            model_uri: ids::BASE_NAMESPACE_URI.to_owned(),
            required_by: None,
            version: None,
            publication_date: None,
        }
    }
}

pub fn resolve(
    nodeset: &NodeSet,
    own_file: &str,
) -> Resolution {
    let indexed = Instant::now();
    let index = build_index(own_file);
    let index_millis = millis(indexed);

    let loading = Instant::now();
    let mut resolution = Resolution {
        dependencies: Vec::new(),
        missing_ns0: false,
        unresolved: Vec::new(),
        version_checks: Vec::new(),
        index_millis,
        dependencies_millis: 0,
    };

    let own: HashSet<&str> = nodeset
        .models
        .iter()
        .map(|model| model.model_uri.as_str())
        .chain(nodeset.namespaces.uris().iter().map(String::as_str))
        .collect();

    let mut queue: VecDeque<Requirement> = nodeset
        .models
        .iter()
        .flat_map(|model| {
            model
                .required_models
                .iter()
                .map(|required| Requirement::from_entry(required, &model.model_uri))
        })
        .collect();
    queue.push_back(Requirement::base_namespace());

    let mut seen: HashSet<String> = HashSet::new();
    let mut loaded: HashSet<String> = HashSet::new();

    while let Some(requirement) = queue.pop_front() {
        if own.contains(requirement.model_uri.as_str()) || !seen.insert(requirement.model_uri.clone()) {
            continue;
        }
        let Some(file) = find(&index, &requirement.model_uri) else {
            if requirement.model_uri == ids::BASE_NAMESPACE_URI {
                resolution.missing_ns0 = true;
            }
            resolution.unresolved.push(UnresolvedModel {
                model_uri: requirement.model_uri,
                required_by: requirement.required_by,
                required_version: requirement.version,
                required_publication_date: requirement.publication_date,
            });
            continue;
        };

        if let Some(check) = check_version(&requirement, file) {
            resolution.version_checks.push(check);
        }
        for model in &file.models {
            for required in &model.required_models {
                queue.push_back(Requirement::from_entry(required, &model.model_uri));
            }
        }
        if !loaded.insert(file.name.clone()) {
            continue;
        }
        if let Some(nodeset) = load(&file.name) {
            resolution.dependencies.push(LoadedDependency {
                model_uri: requirement.model_uri,
                file_name: file.name.clone(),
                nodeset,
            });
        }
    }

    resolution.dependencies_millis = millis(loading);
    resolution
}

/// Prefers the file whose own model is the wanted one over a file that merely requires it.
fn find<'a>(
    index: &'a [IndexedFile],
    model_uri: &str,
) -> Option<&'a IndexedFile> {
    index
        .iter()
        .find(|file| {
            file.models
                .first()
                .is_some_and(|model| model.model_uri == model_uri)
        })
        .or_else(|| index.iter().find(|file| file.defines(model_uri)))
}

fn check_version(
    requirement: &Requirement,
    file: &IndexedFile,
) -> Option<VersionCheck> {
    // A requirement that pinned nothing — namespace 0, which is required implicitly — cannot
    // disagree with what was found, so there is nothing to report.
    if requirement.version.is_none() && requirement.publication_date.is_none() {
        return None;
    }
    let found = file.entry(&requirement.model_uri)?;
    let found_publication_date = found.publication_date.as_ref().map(ToString::to_string);
    if requirement.version == found.version && requirement.publication_date == found_publication_date {
        return None;
    }

    let satisfied = publication_date_entry(requirement).is_satisfied_by(found);

    Some(VersionCheck {
        model_uri: requirement.model_uri.clone(),
        file_name: file.name.clone(),
        required_version: requirement.version.clone(),
        found_version: found.version.clone(),
        required_publication_date: requirement.publication_date.clone(),
        found_publication_date,
        satisfied,
    })
}

/// Rebuilds the requirement as a model entry so the domain crate's own comparison decides.
fn publication_date_entry(requirement: &Requirement) -> ModelTableEntry {
    let mut entry = ModelTableEntry::new(&requirement.model_uri);
    entry.publication_date = requirement
        .publication_date
        .as_deref()
        .and_then(|text| text.parse().ok());
    entry
}

fn build_index(own_file: &str) -> Vec<IndexedFile> {
    workspace::list_files()
        .into_iter()
        .filter(|file| file.name != own_file)
        .filter_map(|file| {
            let path = workspace::path(&file.name).ok()?;
            let source = workspace::read(&path).ok()?;
            let models = xml::parse(&header(&source)).ok()?.into_nodeset().models;
            Some(IndexedFile {
                name: file.name,
                models,
            })
        })
        .collect()
}

fn load(name: &str) -> Option<NodeSet> {
    let path = workspace::path(name).ok()?;
    let source = workspace::read(&path).ok()?;
    Some(xml::parse(&source).ok()?.into_nodeset())
}

/// The document up to the end of its model table, so indexing a four-megabyte nodeset reads two
/// kilobytes of it. The schema puts every table before the nodes, so the cut is always legal XML.
fn header(source: &str) -> String {
    const MODELS: &str = "</Models>";
    const NAMESPACES: &str = "</NamespaceUris>";
    /// Far past where the tables end in any real file, and short enough that the scan is free.
    const HORIZON: usize = 256 * 1024;

    let head = source.get(..HORIZON.min(source.len())).unwrap_or(source);
    let end = head
        .find(MODELS)
        .map(|start| start + MODELS.len())
        .or_else(|| head.find(NAMESPACES).map(|start| start + NAMESPACES.len()));
    match end {
        Some(end) => format!("{}</UANodeSet>", &source[..end]),
        None => source.to_owned(),
    }
}

fn millis(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

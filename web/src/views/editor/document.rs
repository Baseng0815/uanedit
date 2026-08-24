//! The document-level surface: the namespace, model and alias tables the file carries, and what
//! the workspace resolved for it (features.md §2C).
//!
//! Every control writes one semantic operation through [`Tables`] and puts the refusal back beside
//! the row it came from, which is how a namespace still in use names the nodes that hold it.

use core::str::FromStr;

use dioxus::prelude::*;
use uanedit::edit::{
    Refusal,
    alias_users,
    namespace_users,
};
use uanedit::nodeset::ModelTableEntry;
use uanedit::report::OpenReport;
use uanedit::space::AddressSpace;
use uanedit::types::date_time::DateTime;
use uanedit::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use uanedit::{
    Operation,
    ids,
};

use crate::api::{
    OpenTiming,
    OpenedFile,
    UnresolvedModel,
    VersionCheck,
};
use crate::components::{
    Dialog,
    Icon,
};
use crate::session::{
    EditResult,
    EditorHandle,
};
use crate::views::editor::fields::TextField;

/// The nodes a refusal names before the list turns into a wall.
const MAX_USERS: usize = 16;

/// What the open resolved for this file. Small enough to be a prop: the payload's nodesets are
/// megabytes, and the address space already holds them.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentInfo {
    pub name: String,
    pub namespace: String,
    pub nodes: usize,
    pub dependencies: Vec<Dependency>,
    pub missing_ns0: bool,
    pub unresolved: Vec<UnresolvedModel>,
    pub version_checks: Vec<VersionCheck>,
    pub timing: OpenTiming,
    pub report: OpenReport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub file_name: String,
    pub model_uri: String,
}

impl DocumentInfo {
    pub fn of(opened: &OpenedFile) -> Self {
        Self {
            name: opened.name.clone(),
            namespace: opened
                .primary
                .target_namespace()
                .unwrap_or("no target namespace")
                .to_owned(),
            nodes: opened.primary.len(),
            dependencies: opened
                .dependencies
                .iter()
                .map(|dependency| Dependency {
                    file_name: dependency.file_name.clone(),
                    model_uri: dependency.model_uri.clone(),
                })
                .collect(),
            missing_ns0: opened.missing_ns0,
            // Namespace 0 has a warning of its own, so it is not also an unresolved model.
            unresolved: opened
                .unresolved_models
                .iter()
                .filter(|model| model.model_uri != ids::BASE_NAMESPACE_URI)
                .cloned()
                .collect(),
            version_checks: opened.version_checks.clone(),
            timing: opened.timing,
            report: opened.report.clone(),
        }
    }

    /// Everything the open could not answer, which is what the status row keeps visible.
    pub fn warnings(&self) -> usize {
        usize::from(self.missing_ns0)
            + self.unresolved.len()
            + self
                .version_checks
                .iter()
                .filter(|check| !check.satisfied)
                .count()
    }
}

/// The channel one table row writes through, and where a refusal lands.
#[derive(Clone, Copy)]
struct Tables {
    handle: EditorHandle,
    nonce: Signal<u64>,
    failure: Signal<Option<Failure>>,
}

/// A refused operation, at the row that asked for it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Failure {
    key: String,
    message: String,
    /// The nodes a `NamespaceInUse` or `AliasInUse` refusal names.
    users: Vec<String>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Usage {
    namespaces: Vec<usize>,
    aliases: Vec<usize>,
}

#[component]
pub fn DocumentDialog(
    info: DocumentInfo,
    onclose: EventHandler<()>,
) -> Element {
    let handle: EditorHandle = use_context();
    let nonce = use_signal(|| 0_u64);
    let failure = use_signal(|| None::<Failure>);
    let tables = Tables { handle, nonce, failure };

    // One pass over the nodes per edit rather than one per render: counting who holds a namespace
    // or an alias walks every node's references.
    let usage = use_memo(move || {
        let _ = handle.revision.read();
        handle.with_space(usage_of).unwrap_or_default()
    });

    let _ = *handle.revision.read();
    let counts = usage();
    let body = handle.with_space(|space| {
        rsx! {
            {namespace_section(tables, space, &counts.namespaces)}
            {model_section(tables, space)}
            {dependency_section(&info)}
            {alias_section(tables, space, &counts.aliases)}
        }
    });

    rsx! {
        Dialog {
            title: "Document",
            icon: "description",
            subtitle: info.name.clone(),
            wide: true,
            onclose,
            actions: rsx! {
                span { class: "dialog__actions-note", "Every change here is one edit, and undoes as one." }
                button { class: "button tonal", onclick: move |_| onclose.call(()), "Done" }
            },
            match body {
                Some(body) => body,
                None => rsx! {
                    div { class: "doc-empty", "No document is open." }
                },
            }
        }
    }
}

fn usage_of(space: &AddressSpace) -> Usage {
    let primary = space.primary();
    Usage {
        namespaces: (0..primary.namespaces.len())
            .map(|index| match NamespaceIndex::try_from(index) {
                Ok(index) => namespace_users(primary, index).len(),
                Err(_) => 0,
            })
            .collect(),
        aliases: primary
            .aliases
            .iter()
            .map(|(alias, _)| alias_users(primary, alias).len())
            .collect(),
    }
}

impl Tables {
    fn nonce(self) -> u64 {
        *self.nonce.read()
    }

    fn current(self) -> Option<Failure> {
        let failure = self.failure.read();
        failure.as_ref().cloned()
    }

    fn failed(
        self,
        key: &str,
    ) -> Option<Failure> {
        let failure = self.failure.read();
        failure
            .as_ref()
            .filter(|failure| failure.key == key)
            .cloned()
    }

    fn perform(
        self,
        key: &str,
        operation: Operation,
    ) -> bool {
        let (mut failure, mut nonce) = (self.failure, self.nonce);
        match self.handle.apply(operation) {
            EditResult::Refused(refusal) => {
                failure.set(Some(Failure::of(key, &refusal)));
                nonce.with_mut(|value| *value = value.wrapping_add(1));
                false
            }
            EditResult::Closed => false,
            EditResult::Applied { .. } | EditResult::Unchanged => {
                failure.set(None);
                true
            }
        }
    }

    /// Puts a message the domain never saw — a value that did not parse — at the row.
    fn reject(
        self,
        key: &str,
        message: impl Into<String>,
    ) {
        let (mut failure, mut nonce) = (self.failure, self.nonce);
        failure.set(Some(Failure {
            key: key.to_owned(),
            message: message.into(),
            users: Vec::new(),
        }));
        nonce.with_mut(|value| *value = value.wrapping_add(1));
    }

    /// Rewrites one model entry from the entry as it stands now, which is the whole-entry semantics
    /// [`Operation::SetModelEntry`] asks for.
    fn set_model(
        self,
        key: &str,
        index: usize,
        change: impl FnOnce(&mut ModelTableEntry),
    ) -> bool {
        let Some(mut entry) = self
            .handle
            .with_space(|space| space.primary().models.get(index).cloned())
            .flatten()
        else {
            return false;
        };
        change(&mut entry);
        self.perform(
            key,
            Operation::SetModelEntry {
                index,
                entry: Box::new(entry),
            },
        )
    }

    fn set_required(
        self,
        index: usize,
        position: usize,
        change: impl FnOnce(&mut ModelTableEntry),
    ) -> bool {
        self.set_model(&required_key(index, position), index, |entry| {
            if let Some(required) = entry.required_models.get_mut(position) {
                change(required);
            }
        })
    }
}

impl Failure {
    fn of(
        key: &str,
        refusal: &Refusal,
    ) -> Self {
        let users = match refusal {
            Refusal::NamespaceInUse { users, .. } | Refusal::AliasInUse { users, .. } => {
                users.iter().map(ToString::to_string).collect()
            }
            _ => Vec::new(),
        };
        Self {
            key: key.to_owned(),
            message: refusal.to_string(),
            users,
        }
    }
}

/* ---------------------------------------------------------------- Namespaces */

fn namespace_section(
    tables: Tables,
    space: &AddressSpace,
    counts: &[usize],
) -> Element {
    let uris = space.primary().namespaces.uris().to_vec();
    let nonce = tables.nonce();

    rsx! {
        section { class: "doc-section",
            header { class: "doc-section__head",
                span { class: "doc-section__title", "Namespaces" }
                span { class: "chip mono", "{uris.len() + 1}" }
            }
            span { class: "doc-section__hint",
                "Index 0 is fixed by the specification, and a namespace a dependency declares is renamed in that file."
            }
            div { class: "doc-rows",
                NamespaceRow {
                    key: "base",
                    index: 0,
                    uri: ids::BASE_NAMESPACE_URI.to_owned(),
                    users: counts.first().copied().unwrap_or_default(),
                    locked: "Namespace 0 is fixed by the specification".to_owned(),
                    nonce,
                    failure: None,
                    onrename: move |_: String| {},
                    onremove: move |_: ()| {},
                }
                for (position , uri) in uris.iter().enumerate() {
                    NamespaceRow {
                        key: "{position}",
                        index: namespace_index(position),
                        uri: uri.clone(),
                        users: counts.get(position + 1).copied().unwrap_or_default(),
                        locked: locked_reason(space, uri),
                        nonce,
                        failure: tables.failed(&namespace_key(position)),
                        onrename: move |uri: String| {
                            tables
                                .perform(
                                    &namespace_key(position),
                                    Operation::RenameNamespace {
                                        index: namespace_index(position),
                                        uri: uri.trim().to_owned(),
                                    },
                                );
                        },
                        onremove: move |_: ()| {
                            tables
                                .perform(
                                    &namespace_key(position),
                                    Operation::RemoveNamespace {
                                        index: namespace_index(position),
                                    },
                                );
                        },
                    }
                }
            }
            AddOne {
                label: "New namespace URI",
                placeholder: "http://example.org/Other/",
                action: "Add",
                mono: true,
                onadd: move |uri: String| tables.perform("ns:add", Operation::AddNamespace { uri }),
            }
            {note(tables.failed("ns:add"))}
        }
    }
}

fn namespace_index(position: usize) -> NamespaceIndex {
    NamespaceIndex::try_from(position + 1).unwrap_or(NamespaceIndex::MAX)
}

fn namespace_key(position: usize) -> String {
    format!("ns:{}", position + 1)
}

/// Renaming a namespace a dependency also declares would renumber the space under every NodeId the
/// editor holds, which is why the domain refuses it — the row says so before it is tried.
fn locked_reason(
    space: &AddressSpace,
    uri: &str,
) -> Option<String> {
    let shared = space.sets().skip(1).any(|(_, node_set)| {
        node_set
            .namespaces
            .index_of(uri)
            .is_some_and(|index| index != 0)
    });
    shared.then(|| "Declared by a loaded dependency".to_owned())
}

#[component]
fn NamespaceRow(
    index: NamespaceIndex,
    uri: String,
    users: usize,
    locked: Option<String>,
    nonce: u64,
    failure: Option<Failure>,
    onrename: EventHandler<String>,
    onremove: EventHandler<()>,
) -> Element {
    let fixed = locked.is_some();

    rsx! {
        div { class: "doc-entry",
            div { class: "doc-row namespace",
                span { class: "doc-row__index mono", "{index}" }
                InlineText {
                    value: uri,
                    nonce,
                    mono: true,
                    disabled: fixed,
                    invalid: failure.is_some(),
                    placeholder: "namespace URI",
                    oncommit: move |uri: String| onrename.call(uri),
                }
                span { class: "chip mono doc-row__count", title: "Nodes that name this namespace", "{users}" }
                if let Some(reason) = locked {
                    span { class: "chip locked", title: reason,
                        Icon { name: "lock", class: "small" }
                    }
                } else {
                    button {
                        class: "icon-button tiny",
                        title: "Remove this namespace",
                        onclick: move |_| onremove.call(()),
                        Icon { name: "close", class: "small" }
                    }
                }
            }
            {note(failure)}
        }
    }
}

/* -------------------------------------------------------------------- Models */

fn model_section(
    tables: Tables,
    space: &AddressSpace,
) -> Element {
    let models = space.primary().models.clone();
    let nonce = tables.nonce();
    let failure = tables.current();

    rsx! {
        section { class: "doc-section",
            header { class: "doc-section__head",
                span { class: "doc-section__title", "Model entries" }
                span { class: "chip mono", "{models.len()}" }
            }
            if models.is_empty() {
                div { class: "doc-empty",
                    "This file declares no model. The first entry is the model the file itself defines."
                }
            }
            for (index , entry) in models.iter().enumerate() {
                ModelCard {
                    key: "{index}",
                    index,
                    entry: entry.clone(),
                    nonce,
                    failure: failure.clone(),
                    onfield: move |(field, value): (&'static str, String)| {
                        set_model_field(tables, index, field, value);
                    },
                    onrequired: move |(position, field, value): (usize, &'static str, String)| {
                        set_required_field(tables, index, position, field, value);
                    },
                    onremoverequired: move |position: usize| {
                        tables
                            .set_model(
                                &required_key(index, position),
                                index,
                                |entry| {
                                    if position < entry.required_models.len() {
                                        entry.required_models.remove(position);
                                    }
                                },
                            );
                    },
                    onaddrequired: move |(uri, version): (String, String)| {
                        tables
                            .set_model(
                                &format!("model:{index}:required:add"),
                                index,
                                |entry| {
                                    entry
                                        .required_models
                                        .push(ModelTableEntry {
                                            version: optional(version),
                                            ..ModelTableEntry::new(uri.trim())
                                        });
                                },
                            )
                    },
                    onremove: move |_: ()| {
                        tables.perform(&format!("model:{index}"), Operation::RemoveModelEntry { index });
                    },
                }
            }
            AddOne {
                label: "New model URI",
                placeholder: "http://example.org/Other/",
                action: "Add",
                mono: true,
                onadd: move |uri: String| {
                    tables
                        .perform(
                            "model:add",
                            Operation::AddModelEntry {
                                entry: Box::new(ModelTableEntry::new(uri.trim())),
                            },
                        )
                },
            }
            {note(tables.failed("model:add"))}
        }
    }
}

fn set_model_field(
    tables: Tables,
    index: usize,
    field: &'static str,
    value: String,
) {
    let key = format!("model:{index}:{field}");
    if field == "published" {
        match parse_date(&value) {
            Ok(date) => {
                tables.set_model(&key, index, |entry| entry.publication_date = date);
            }
            Err(message) => tables.reject(&key, message),
        }
        return;
    }
    tables.set_model(&key, index, |entry| match field {
        "uri" => entry.model_uri = value.trim().to_owned(),
        "version" => entry.version = optional(value),
        _ => entry.model_version = optional(value),
    });
}

fn set_required_field(
    tables: Tables,
    index: usize,
    position: usize,
    field: &'static str,
    value: String,
) {
    if field == "published" {
        match parse_date(&value) {
            Ok(date) => {
                tables.set_required(index, position, |required| required.publication_date = date);
            }
            Err(message) => tables.reject(&required_key(index, position), message),
        }
        return;
    }
    tables.set_required(index, position, |required| match field {
        "uri" => required.model_uri = value.trim().to_owned(),
        _ => required.version = optional(value),
    });
}

fn required_key(
    index: usize,
    position: usize,
) -> String {
    format!("model:{index}:required:{position}")
}

#[component]
fn ModelCard(
    index: usize,
    entry: ModelTableEntry,
    nonce: u64,
    failure: Option<Failure>,
    onfield: EventHandler<(&'static str, String)>,
    onrequired: EventHandler<(usize, &'static str, String)>,
    onremoverequired: EventHandler<usize>,
    onaddrequired: Callback<(String, String), bool>,
    onremove: EventHandler<()>,
) -> Element {
    let missing_version = entry.model_version.is_none();

    rsx! {
        div { class: "doc-model",
            header { class: "doc-model__head",
                span { class: "doc-model__index mono", "#{index}" }
                span { class: "doc-model__uri mono", "{entry.model_uri}" }
                button {
                    class: "icon-button tiny",
                    title: "Remove this model entry",
                    onclick: move |_| onremove.call(()),
                    Icon { name: "close", class: "small" }
                }
            }
            {note(at(&failure, &format!("model:{index}")))}
            div { class: "doc-fields",
                TextField {
                    label: "ModelUri",
                    value: entry.model_uri.clone(),
                    nonce,
                    mono: true,
                    oncommit: move |value: String| onfield.call(("uri", value)),
                    error: message(&failure, &format!("model:{index}:uri")),
                }
                TextField {
                    label: "Version",
                    value: entry.version.clone().unwrap_or_default(),
                    nonce,
                    mono: true,
                    placeholder: "1.0.0",
                    oncommit: move |value: String| onfield.call(("version", value)),
                    error: message(&failure, &format!("model:{index}:version")),
                }
                TextField {
                    label: "PublicationDate",
                    value: entry.publication_date.as_ref().map(ToString::to_string).unwrap_or_default(),
                    nonce,
                    mono: true,
                    placeholder: "2026-01-01T00:00:00Z",
                    oncommit: move |value: String| onfield.call(("published", value)),
                    error: message(&failure, &format!("model:{index}:published")),
                }
                TextField {
                    label: "ModelVersion",
                    value: entry.model_version.clone().unwrap_or_default(),
                    nonce,
                    mono: true,
                    placeholder: "1.0.0",
                    oncommit: move |value: String| onfield.call(("model-version", value)),
                    error: message(&failure, &format!("model:{index}:model-version")),
                    hint: missing_version.then(|| "Mandatory since schema 1.05.03".to_owned()),
                }
            }
            div { class: "doc-subhead",
                span { "Required models" }
                span { class: "chip mono", "{entry.required_models.len()}" }
            }
            if entry.required_models.is_empty() {
                div { class: "doc-empty", "Nothing pinned. A model that uses another's types states it here." }
            }
            div { class: "doc-rows",
                for (position , required) in entry.required_models.iter().enumerate() {
                    RequiredRow {
                        key: "{position}",
                        uri: required.model_uri.clone(),
                        version: required.version.clone().unwrap_or_default(),
                        published: required.publication_date.as_ref().map(ToString::to_string).unwrap_or_default(),
                        nonce,
                        failure: at(&failure, &required_key(index, position)),
                        onfield: move |(field, value): (&'static str, String)| {
                            onrequired.call((position, field, value))
                        },
                        onremove: move |_: ()| onremoverequired.call(position),
                    }
                }
            }
            AddPair {
                first_label: "Required ModelUri",
                first_placeholder: "http://opcfoundation.org/UA/DI/",
                second_label: "Version",
                second_placeholder: "1.0.0",
                action: "Pin",
                onadd: onaddrequired,
            }
            {note(at(&failure, &format!("model:{index}:required:add")))}
        }
    }
}

#[component]
fn RequiredRow(
    uri: String,
    version: String,
    published: String,
    nonce: u64,
    failure: Option<Failure>,
    onfield: EventHandler<(&'static str, String)>,
    onremove: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "doc-entry",
            div { class: "doc-row required",
                InlineText {
                    value: uri,
                    nonce,
                    mono: true,
                    invalid: failure.is_some(),
                    placeholder: "model URI",
                    oncommit: move |value: String| onfield.call(("uri", value)),
                }
                InlineText {
                    value: version,
                    nonce,
                    mono: true,
                    placeholder: "version",
                    oncommit: move |value: String| onfield.call(("version", value)),
                }
                InlineText {
                    value: published,
                    nonce,
                    mono: true,
                    placeholder: "publication date",
                    oncommit: move |value: String| onfield.call(("published", value)),
                }
                button {
                    class: "icon-button tiny",
                    title: "Unpin this model",
                    onclick: move |_| onremove.call(()),
                    Icon { name: "close", class: "small" }
                }
            }
            {note(failure)}
        }
    }
}

/* -------------------------------------------------------------- Dependencies */

fn dependency_section(info: &DocumentInfo) -> Element {
    let resolved = info.dependencies.is_empty() && info.unresolved.is_empty() && !info.missing_ns0;

    rsx! {
        section { class: "doc-section",
            header { class: "doc-section__head",
                span { class: "doc-section__title", "Dependencies" }
                span { class: "chip mono", "{info.dependencies.len()}" }
                div { class: "doc-section__spacer" }
                if info.warnings() > 0 {
                    span { class: "chip warn", "{info.warnings()} unresolved" }
                }
            }
            span { class: "doc-section__hint",
                "Required models resolve against the other files in the workspace directory."
            }
            div { class: "doc-rows",
                if info.missing_ns0 {
                    DependencyRow {
                        severity: "bad",
                        icon: "link_off",
                        title: "http://opcfoundation.org/UA/".to_owned(),
                        detail: "The standard nodeset is not in this workspace, so references into namespace 0 do not resolve. Place Opc.Ua.NodeSet2.xml beside this file."
                            .to_owned(),
                    }
                }
                for dependency in info.dependencies.iter() {
                    DependencyRow {
                        key: "{dependency.file_name}",
                        severity: "good",
                        icon: "link",
                        title: dependency.model_uri.clone(),
                        detail: format!("Loaded from {}", dependency.file_name),
                    }
                }
                for unresolved in info.unresolved.iter() {
                    DependencyRow {
                        key: "{unresolved.model_uri}",
                        severity: "bad",
                        icon: "link_off",
                        title: unresolved.model_uri.clone(),
                        detail: unresolved_detail(unresolved),
                    }
                }
                for check in info.version_checks.iter() {
                    DependencyRow {
                        key: "{check.model_uri}",
                        severity: match check.satisfied {
                            true => "good",
                            false => "warn",
                        },
                        icon: match check.satisfied {
                            true => "check_circle",
                            false => "warning",
                        },
                        title: check.model_uri.clone(),
                        detail: version_detail(check),
                    }
                }
                if resolved {
                    div { class: "doc-empty", "Nothing to resolve: this file requires no other model." }
                }
            }
        }
    }
}

#[component]
fn DependencyRow(
    severity: &'static str,
    icon: &'static str,
    title: String,
    detail: String,
) -> Element {
    rsx! {
        div { class: "doc-dep {severity}",
            span { class: "doc-dep__icon", Icon { name: icon, class: "small" } }
            div { class: "doc-dep__text",
                span { class: "doc-dep__title mono", {title} }
                span { class: "doc-dep__detail", {detail} }
            }
        }
    }
}

fn unresolved_detail(unresolved: &UnresolvedModel) -> String {
    let asked = match (&unresolved.required_version, &unresolved.required_publication_date) {
        (Some(version), Some(date)) => format!(" at {version}, {date}"),
        (Some(version), None) => format!(" at {version}"),
        (None, Some(date)) => format!(" at {date}"),
        (None, None) => String::new(),
    };
    match &unresolved.required_by {
        Some(by) => format!("Required by {by}{asked} · no file in this workspace defines it"),
        None => format!("Required{asked} · no file in this workspace defines it"),
    }
}

fn version_detail(check: &VersionCheck) -> String {
    let versions = format!(
        "required {}, found {}",
        check.required_version.as_deref().unwrap_or("any"),
        check.found_version.as_deref().unwrap_or("none")
    );
    let found = check.found_publication_date.as_deref().unwrap_or("none");
    match check.satisfied {
        true => format!("{} · {versions} · published {found}", check.file_name),
        false => format!(
            "{} · {versions} · needs {}, has {found}",
            check.file_name,
            check.required_publication_date.as_deref().unwrap_or("any")
        ),
    }
}

/* ------------------------------------------------------------------ Aliases */

fn alias_section(
    tables: Tables,
    space: &AddressSpace,
    counts: &[usize],
) -> Element {
    let aliases: Vec<(String, String)> = space
        .primary()
        .aliases
        .iter()
        .map(|(alias, node_id)| (alias.clone(), node_id.to_string()))
        .collect();

    rsx! {
        section { class: "doc-section",
            header { class: "doc-section__head",
                span { class: "doc-section__title", "Aliases" }
                span { class: "chip mono", "{aliases.len()}" }
            }
            span { class: "doc-section__hint",
                "Kept in the order the file lists them; a reference written through an alias keeps it on save."
            }
            if aliases.is_empty() {
                div { class: "doc-empty", "This file defines no aliases." }
            }
            div { class: "doc-rows",
                for (position , entry) in aliases.iter().enumerate() {
                    AliasRow {
                        key: "{entry.0}",
                        alias: entry.0.clone(),
                        node_id: entry.1.clone(),
                        users: counts.get(position).copied().unwrap_or_default(),
                        failure: tables.failed(&alias_key(&entry.0)),
                        onremove: move |alias: String| {
                            tables.perform(&alias_key(&alias), Operation::RemoveAlias { alias });
                        },
                    }
                }
            }
            AddPair {
                first_label: "Alias",
                first_placeholder: "HasComponent",
                second_label: "NodeId",
                second_placeholder: "i=47",
                action: "Add",
                onadd: move |(alias, text): (String, String)| {
                    match NodeId::from_str(text.trim()) {
                        Ok(node_id) => {
                            tables
                                .perform(
                                    "alias:add",
                                    Operation::AddAlias {
                                        alias: alias.trim().to_owned(),
                                        node_id,
                                    },
                                )
                        }
                        Err(error) => {
                            tables.reject("alias:add", error.to_string());
                            false
                        }
                    }
                },
            }
            {note(tables.failed("alias:add"))}
        }
    }
}

fn alias_key(alias: &str) -> String {
    format!("alias:{alias}")
}

#[component]
fn AliasRow(
    alias: String,
    node_id: String,
    users: usize,
    failure: Option<Failure>,
    onremove: EventHandler<String>,
) -> Element {
    let named = alias.clone();

    rsx! {
        div { class: "doc-entry",
            div { class: "doc-row alias",
                span { class: "doc-row__static mono", {alias} }
                span { class: "doc-row__static mono", {node_id} }
                span { class: "chip mono doc-row__count", title: "Nodes that name this alias", "{users}" }
                button {
                    class: "icon-button tiny",
                    title: "Remove this alias",
                    onclick: move |_| onremove.call(named.clone()),
                    Icon { name: "close", class: "small" }
                }
            }
            {note(failure)}
        }
    }
}

/* --------------------------------------------------------------- Primitives */

/// A one-line editor that commits on change and re-seeds from the model when `nonce` moves.
#[component]
fn InlineText(
    value: String,
    nonce: u64,
    oncommit: EventHandler<String>,
    #[props(default)] placeholder: String,
    #[props(default)] mono: bool,
    #[props(default)] disabled: bool,
    #[props(default)] invalid: bool,
) -> Element {
    let mut draft = use_signal(|| value.clone());
    use_effect(use_reactive!(|(value, nonce)| {
        let _ = nonce;
        draft.set(value);
    }));

    rsx! {
        input {
            class: match (mono, invalid) {
                (true, true) => "field__input mono invalid",
                (true, false) => "field__input mono",
                (false, true) => "field__input invalid",
                (false, false) => "field__input",
            },
            value: "{draft}",
            placeholder,
            disabled,
            oninput: move |event| draft.set(event.value()),
            onchange: move |event| oncommit.call(event.value()),
        }
    }
}

#[component]
fn AddOne(
    label: &'static str,
    placeholder: &'static str,
    action: &'static str,
    #[props(default)] mono: bool,
    onadd: Callback<String, bool>,
) -> Element {
    let draft = use_signal(String::new);

    rsx! {
        div { class: "doc-add",
            div { class: "field",
                span { class: "field__label", {label} }
                input {
                    class: if mono { "field__input mono" } else { "field__input" },
                    value: "{draft}",
                    placeholder,
                    oninput: move |event| {
                        let mut draft = draft;
                        draft.set(event.value());
                    },
                    onkeydown: move |event| {
                        if event.key() == Key::Enter {
                            commit_one(draft, onadd);
                        }
                    },
                }
            }
            button {
                class: "button tonal doc-add__button",
                onclick: move |_| commit_one(draft, onadd),
                Icon { name: "add", class: "small" }
                {action}
            }
        }
    }
}

fn commit_one(
    mut draft: Signal<String>,
    onadd: Callback<String, bool>,
) {
    let text = draft.peek().trim().to_owned();
    if !text.is_empty() && onadd.call(text) {
        draft.set(String::new());
    }
}

#[component]
fn AddPair(
    first_label: &'static str,
    first_placeholder: &'static str,
    second_label: &'static str,
    second_placeholder: &'static str,
    action: &'static str,
    onadd: Callback<(String, String), bool>,
) -> Element {
    let first = use_signal(String::new);
    let second = use_signal(String::new);

    rsx! {
        div { class: "doc-add",
            div { class: "field",
                span { class: "field__label", {first_label} }
                input {
                    class: "field__input mono",
                    value: "{first}",
                    placeholder: first_placeholder,
                    oninput: move |event| {
                        let mut first = first;
                        first.set(event.value());
                    },
                    onkeydown: move |event| {
                        if event.key() == Key::Enter {
                            commit_pair(first, second, onadd);
                        }
                    },
                }
            }
            div { class: "field",
                span { class: "field__label", {second_label} }
                input {
                    class: "field__input mono",
                    value: "{second}",
                    placeholder: second_placeholder,
                    oninput: move |event| {
                        let mut second = second;
                        second.set(event.value());
                    },
                    onkeydown: move |event| {
                        if event.key() == Key::Enter {
                            commit_pair(first, second, onadd);
                        }
                    },
                }
            }
            button {
                class: "button tonal doc-add__button",
                onclick: move |_| commit_pair(first, second, onadd),
                Icon { name: "add", class: "small" }
                {action}
            }
        }
    }
}

fn commit_pair(
    mut first: Signal<String>,
    mut second: Signal<String>,
    onadd: Callback<(String, String), bool>,
) {
    let pair = (first.peek().trim().to_owned(), second.peek().trim().to_owned());
    if !pair.0.is_empty() && onadd.call(pair) {
        first.set(String::new());
        second.set(String::new());
    }
}

fn note(failure: Option<Failure>) -> Element {
    let Some(failure) = failure else {
        return rsx! {};
    };
    let extra = failure.users.len().saturating_sub(MAX_USERS);

    rsx! {
        div { class: "doc-note",
            Icon { name: "error", class: "small" }
            span { "{failure.message}" }
        }
        if !failure.users.is_empty() {
            div { class: "doc-users",
                for user in failure.users.iter().take(MAX_USERS) {
                    span { key: "{user}", class: "chip mono", {user.as_str()} }
                }
                if extra > 0 {
                    span { class: "chip", "+{extra} more" }
                }
            }
        }
    }
}

fn at(
    failure: &Option<Failure>,
    key: &str,
) -> Option<Failure> {
    failure
        .as_ref()
        .filter(|failure| failure.key == key)
        .cloned()
}

fn message(
    failure: &Option<Failure>,
    key: &str,
) -> Option<String> {
    at(failure, key).map(|failure| failure.message)
}

fn optional(text: String) -> Option<String> {
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn parse_date(text: &str) -> Result<Option<DateTime>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match DateTime::from_str(text) {
        Ok(date) => Ok(Some(date)),
        Err(error) => Err(error.to_string()),
    }
}

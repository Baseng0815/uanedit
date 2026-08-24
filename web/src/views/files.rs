use dioxus::prelude::*;

use crate::api::{
    Workspace,
    WorkspaceFile,
    list_workspace,
};
use crate::components::{
    CreateFileDialog,
    DocumentStyles,
    Icon,
};
use crate::route::Route;

#[component]
pub fn Files() -> Element {
    let workspace = use_server_future(list_workspace)?;
    let mut creating = use_signal(|| false);

    rsx! {
        DocumentStyles {}
        div { class: "files",
            div { class: "files__header",
                h1 { class: "type-title-large", "Workspace" }
                div { class: "files__header-spacer" }
                button {
                    class: "button",
                    title: "Create a nodeset in this workspace",
                    onclick: move |_| creating.set(true),
                    Icon { name: "add", class: "small" }
                    "New nodeset"
                }
            }
            if creating() {
                CreateFileDialog { onclose: move |_: ()| creating.set(false) }
            }
            match &*workspace.read() {
                Some(Ok(workspace)) => rsx! {
                    Listing { workspace: workspace.clone() }
                },
                Some(Err(error)) => rsx! {
                    div { class: "notice",
                        Icon { name: "error" }
                        span { "The workspace could not be read: {error}" }
                    }
                },
                None => rsx! {
                    div { class: "loading type-body", "Loading…" }
                },
            }
        }
    }
}

#[component]
fn Listing(workspace: Workspace) -> Element {
    rsx! {
        span { class: "files__root type-body-small mono", "{workspace.root}" }
        if workspace.files.is_empty() {
            EmptyWorkspace {}
        } else {
            div { class: "list",
                for file in workspace.files.iter() {
                    FileRow { key: "{file.name}", file: file.clone() }
                }
            }
        }
    }
}

#[component]
fn FileRow(file: WorkspaceFile) -> Element {
    rsx! {
        Link { class: "list-item", to: Route::Editor { file: file.name.clone() },
            span { class: "list-item__leading", Icon { name: "description", class: "small" } }
            span { class: "list-item__text",
                span { class: "list-item__headline", "{file.name}" }
                span { class: "list-item__supporting mono",
                    "{format_size(file.size)}"
                    if let Some(modified) = format_modified(file.modified.as_deref()) {
                        " · {modified}"
                    }
                }
            }
            span { class: "list-item__trailing", Icon { name: "chevron_right" } }
        }
    }
}

#[component]
fn EmptyWorkspace() -> Element {
    rsx! {
        div { class: "empty-state",
            Icon { name: "folder_off" }
            span { class: "type-title", "No nodesets here yet" }
            span { class: "empty-state__hint type-body",
                "Drop a "
                span { class: "mono", ".NodeSet2.xml" }
                " file into the directory above, or point "
                span { class: "mono", "UANEDIT_WORKSPACE" }
                " at the one you keep your models in and restart the server."
            }
        }
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;

    match bytes {
        0..KB => format!("{bytes} B"),
        KB..MB => format!("{}.{} kB", bytes / KB, (bytes % KB) * 10 / KB),
        _ => format!("{}.{} MB", bytes / MB, (bytes % MB) * 10 / MB),
    }
}

fn format_modified(modified: Option<&str>) -> Option<String> {
    let stamp = modified?;
    Some(format!("{} UTC", stamp.trim_end_matches('Z').replace('T', " ")))
}

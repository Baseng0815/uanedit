//! The editor route: open the file, build the address space, and lay out the three panes.

mod arguments;
mod definition;
mod diagnostics;
mod diff;
mod document;
mod fields;
mod forms;
mod icons;
mod inspector;
mod instantiate;
mod lifecycle;
mod nudge;
mod picker;
mod references;
mod report;
mod right_pane;
mod shell;
mod status;
mod subtype;
mod tree;
mod validation;
mod value_edit;

use dioxus::prelude::*;

use crate::api::open_file;
use crate::components::Icon;
use crate::session::EditorHandle;
use crate::views::editor::document::DocumentInfo;
use crate::views::editor::inspector::InspectorPane;
use crate::views::editor::right_pane::RightPane;
use crate::views::editor::shell::{
    DialogHost,
    Dialogs,
    Navigate,
};
use crate::views::editor::status::DocumentBar;
use crate::views::editor::tree::TreePane;

#[component]
pub fn Editor(file: String) -> Element {
    let name = file.clone();
    let opened = use_server_future(move || open_file(name.clone()))?;
    let handle: EditorHandle = use_context();
    // The panes ask the tree to reveal a node, and any of them may open a dialog, so both channels
    // are provided above them rather than owned by the tree.
    use_context_provider(Navigate::new);
    use_context_provider(Dialogs::new);

    // The browser builds its own address space (architecture.md §3), after hydration, so the server
    // never indexes a nodeset it will not edit.
    use_effect(move || {
        if let Some(Ok(payload)) = opened.read().as_ref() {
            handle.open(payload);
        }
    });
    use_drop(move || handle.close());

    rsx! {
        div { class: "editor",
            match &*opened.read() {
                Some(Ok(opened)) => rsx! {
                    DocumentBar { info: DocumentInfo::of(opened) }
                },
                Some(Err(error)) => rsx! {
                    div { class: "notice",
                        Icon { name: "error" }
                        span { "{file} could not be opened: {error}" }
                    }
                },
                None => rsx! {
                    div { class: "loading type-body", "Opening {file}…" }
                },
            }
            div { class: "workspace",
                TreePane {}
                InspectorPane {}
                RightPane {}
            }
            DialogHost {}
        }
    }
}

/// A count and its noun, pluralised.
pub fn counted(
    count: usize,
    noun: &str,
) -> String {
    match count {
        1 => format!("1 {noun}"),
        _ => format!("{count} {noun}s"),
    }
}

/// The last meaningful path segment of a namespace URI, which is all a chip has room for.
pub fn short_namespace(uri: &str) -> String {
    let trimmed = uri.trim_end_matches('/');
    match trimmed.rsplit('/').find(|part| !part.is_empty()) {
        Some(part) if !part.contains(':') => part.to_owned(),
        _ => trimmed.to_owned(),
    }
}

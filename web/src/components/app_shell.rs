use dioxus::prelude::*;

use crate::components::{
    Icon,
    ThemeToggle,
};
use crate::route::Route;
use crate::session::EditorHandle;

/// The router layout: top app bar, navigation rail, and the routed body between them.
///
/// The open document is provided here rather than in the editor route, because the app bar sits
/// above the `Outlet` and context only flows downwards.
#[component]
pub fn AppShell() -> Element {
    let handle = use_context_provider(EditorHandle::new);

    rsx! {
        div {
            class: "app",
            // Focusable, and focused on mount, so a shortcut reaches the shell before anything
            // inside it has been clicked.
            tabindex: "-1",
            onmounted: move |event| {
                let element = event.data();
                spawn(async move {
                    let _ = element.set_focus(true).await;
                });
            },
            onkeydown: move |event| shortcut(&event, handle),
            TopAppBar {}
            NavigationRail {}
            main { class: "app__body",
                SuspenseBoundary {
                    fallback: |_| rsx! {
                        div { class: "loading type-body", "Loading…" }
                    },
                    Outlet::<Route> {}
                }
            }
        }
    }
}

fn shortcut(
    event: &KeyboardEvent,
    handle: EditorHandle,
) {
    let modifiers = event.modifiers();
    if !modifiers.contains(Modifiers::CONTROL) && !modifiers.contains(Modifiers::META) {
        return;
    }
    let Key::Character(character) = event.key() else {
        return;
    };
    match character.to_ascii_lowercase().as_str() {
        "z" => {
            event.prevent_default();
            match modifiers.contains(Modifiers::SHIFT) {
                true => handle.redo(),
                false => handle.undo(),
            }
        }
        "y" => {
            event.prevent_default();
            handle.redo();
        }
        "s" => {
            event.prevent_default();
            handle.save();
        }
        _ => {}
    }
}

#[component]
fn TopAppBar() -> Element {
    let handle: EditorHandle = use_context();
    let open_file = use_open_file();
    let loaded = handle.file.read().is_some();
    let dirty = *handle.dirty.read();
    let busy = *handle.busy.read();
    let history = handle.history.read().clone();
    let status = handle.status.read().clone();

    let file_class = if open_file.is_some() {
        "top-app-bar__file"
    } else {
        "top-app-bar__file empty"
    };
    let file_name = open_file.unwrap_or_else(|| "No file open".to_string());
    let file_title = file_name.clone();

    rsx! {
        header { class: "top-app-bar",
            Link { class: "top-app-bar__brand", to: Route::Files {},
                span { class: "top-app-bar__brand-name", "uanedit" }
                span { class: "top-app-bar__brand-tag type-label-small", "NodeSet2 editor" }
            }
            div { class: "top-app-bar__document",
                span { class: file_class, title: file_title, {file_name} }
                if dirty {
                    span { class: "top-app-bar__dirty", title: "Unsaved changes" }
                }
            }
            div { class: "top-app-bar__spacer" }
            if let Some(status) = status {
                span { class: status.class(), "{status.text}" }
            }
            div { class: "top-app-bar__actions",
                button {
                    class: "icon-button",
                    disabled: !loaded || history.undo.is_none(),
                    title: label("Undo", history.undo.as_deref(), "Ctrl+Z"),
                    onclick: move |_| handle.undo(),
                    Icon { name: "undo" }
                }
                button {
                    class: "icon-button",
                    disabled: !loaded || history.redo.is_none(),
                    title: label("Redo", history.redo.as_deref(), "Ctrl+Shift+Z"),
                    onclick: move |_| handle.redo(),
                    Icon { name: "redo" }
                }
                button {
                    class: "icon-button",
                    disabled: !loaded || busy,
                    title: "Preview the diff this save would make",
                    onclick: move |_| handle.preview(),
                    Icon { name: "difference" }
                }
                button {
                    class: "icon-button",
                    disabled: !loaded || busy,
                    title: "Download the XML a save would write",
                    onclick: move |_| handle.download(),
                    Icon { name: "download" }
                }
                button {
                    class: "button tonal",
                    disabled: !loaded || busy,
                    title: "Save (Ctrl+S)",
                    onclick: move |_| handle.save(),
                    Icon { name: "save", class: "small" }
                    "Save"
                }
                ThemeToggle {}
            }
        }
    }
}

fn label(
    action: &str,
    entry: Option<&str>,
    shortcut: &str,
) -> String {
    match entry {
        Some(entry) => format!("{action}: {entry} ({shortcut})"),
        None => format!("Nothing to {} ({shortcut})", action.to_lowercase()),
    }
}

#[component]
fn NavigationRail() -> Element {
    let on_files = matches!(use_route::<Route>(), Route::Files {});

    rsx! {
        nav { class: "nav-rail",
            Link {
                class: if on_files { "nav-rail__destination active" } else { "nav-rail__destination" },
                to: Route::Files {},
                span { class: "nav-rail__indicator", Icon { name: "folder_open" } }
                span { class: "type-label-small", "Files" }
            }
            PendingDestination {
                icon: "rule",
                label: "Validation",
                note: "Validation — in the open file's right-hand pane; no workspace-wide view yet",
            }
            PendingDestination {
                icon: "settings",
                label: "Settings",
                note: "Settings — not implemented yet",
            }
        }
    }
}

#[component]
fn PendingDestination(
    icon: &'static str,
    label: &'static str,
    note: &'static str,
) -> Element {
    rsx! {
        div {
            class: "nav-rail__destination disabled",
            title: note,
            span { class: "nav-rail__indicator", Icon { name: icon } }
            span { class: "type-label-small", {label} }
        }
    }
}

fn use_open_file() -> Option<String> {
    match use_route::<Route>() {
        Route::Editor { file } => Some(file),
        Route::Files {} => None,
    }
}

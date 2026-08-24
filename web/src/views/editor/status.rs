//! The open-file status row above the three panes, and the dialogs it opens.
//!
//! What the file turned out to be in one line; the detail — the report, the tables, the diff, the
//! version nudge — sits behind it (features.md §2C, §2E).

use dioxus::prelude::*;

use crate::components::{
    DocumentStyles,
    Icon,
};
use crate::session::EditorHandle;
use crate::views::editor::counted;
use crate::views::editor::diff::DiffDialog;
use crate::views::editor::document::{
    DocumentDialog,
    DocumentInfo,
};
use crate::views::editor::nudge::VersionNudgeDialog;
use crate::views::editor::report::ReportDialog;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    Document,
    Report,
}

#[component]
pub fn DocumentBar(info: DocumentInfo) -> Element {
    let handle: EditorHandle = use_context();
    let mut surface = use_signal(|| None::<Surface>);
    let open = surface();
    let preview = handle.diff.read().clone();
    let nudge = handle.nudge.read().clone();
    let warnings = info.warnings();
    let report = info.report.clone();

    rsx! {
        DocumentStyles {}
        section { class: "doc-bar",
            span { class: "doc-bar__name type-title-small", "{info.name}" }
            span { class: "doc-bar__namespace type-body-small mono", title: "{info.namespace}", "{info.namespace}" }
            span { class: "chip mono", title: "Nodes this file defines", {counted(info.nodes, "node")} }
            for dependency in info.dependencies.iter() {
                span {
                    key: "{dependency.file_name}",
                    class: "chip mono",
                    title: "{dependency.model_uri}",
                    Icon { name: "link", class: "small" }
                    "{dependency.file_name}"
                }
            }
            if warnings > 0 {
                button {
                    class: "chip warn action",
                    title: "Models the workspace could not resolve",
                    onclick: move |_| surface.set(Some(Surface::Document)),
                    Icon { name: "warning", class: "small" }
                    "{warnings} unresolved"
                }
            }
            if report.is_clean() {
                span {
                    class: "chip",
                    title: "Elements kept although this editor does not model them",
                    "{report.preserved.len()} preserved"
                }
            } else {
                button {
                    class: "chip warn action",
                    title: "Irregularities the file already carried",
                    onclick: move |_| surface.set(Some(Surface::Report)),
                    "{report.findings.len()} findings"
                }
            }
            div { class: "doc-bar__spacer" }
            if !info.timing.already_open {
                span { class: "chip mono", title: "Server-side open", "{info.timing.total_millis} ms" }
            }
            div { class: "doc-bar__actions",
                button {
                    class: "button text tiny",
                    title: "Namespaces, model entries, dependencies and aliases",
                    onclick: move |_| surface.set(Some(Surface::Document)),
                    Icon { name: "description", class: "small" }
                    "Document"
                }
                button {
                    class: "button text tiny",
                    title: "What was loaded, kept, and read past",
                    onclick: move |_| surface.set(Some(Surface::Report)),
                    Icon { name: "fact_check", class: "small" }
                    "Report"
                }
            }
        }
        if open == Some(Surface::Document) {
            DocumentDialog { info: info.clone(), onclose: move |_: ()| surface.set(None) }
        }
        if open == Some(Surface::Report) {
            ReportDialog { info: info.clone(), onclose: move |_: ()| surface.set(None) }
        }
        if let Some(preview) = preview {
            DiffDialog {
                preview,
                onclose: move |_: ()| {
                    let mut diff = handle.diff;
                    diff.set(None);
                },
            }
        }
        if let Some(nudge) = nudge {
            VersionNudgeDialog {
                nudge,
                onclose: move |_: ()| {
                    let mut nudge = handle.nudge;
                    nudge.set(None);
                },
            }
        }
    }
}

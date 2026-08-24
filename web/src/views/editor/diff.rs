//! The would-be save as a unified diff against the bytes on disk — the invariant as a feature
//! (features.md §2E).

use dioxus::prelude::*;

use crate::api::{
    DiffHunk,
    DiffLineKind,
    DiffPreview,
};
use crate::components::{
    Dialog,
    Icon,
};

#[component]
pub fn DiffDialog(
    preview: DiffPreview,
    onclose: EventHandler<()>,
) -> Element {
    rsx! {
        Dialog {
            title: "Diff preview",
            icon: "difference",
            subtitle: preview.name.clone(),
            wide: true,
            onclose,
            actions: rsx! {
                span { class: "dialog__actions-note", "Against the bytes on disk, as this editor would write them." }
                button { class: "button tonal", onclick: move |_| onclose.call(()), "Close" }
            },
            if preview.changed {
                div { class: "diff",
                    div { class: "diff__summary",
                        span { class: "chip mono diff__added", "+{preview.added}" }
                        span { class: "chip mono diff__removed", "−{preview.removed}" }
                        span { class: "chip mono", "{preview.hunks.len()} hunks" }
                    }
                    if preview.truncated {
                        div { class: "diff__truncated",
                            Icon { name: "more_horiz", class: "small" }
                            span { "The diff is longer than this preview carries; the hunks below stop early." }
                        }
                    }
                    for (index , hunk) in preview.hunks.iter().enumerate() {
                        Hunk { key: "{index}", hunk: hunk.clone() }
                    }
                }
            } else {
                div { class: "diff__identical",
                    Icon { name: "task_alt" }
                    span { class: "type-title", "No changes" }
                    span { class: "type-body-small", "The output is byte-identical to the file on disk." }
                }
            }
        }
    }
}

#[component]
fn Hunk(hunk: DiffHunk) -> Element {
    let old_lines = hunk
        .lines
        .iter()
        .filter(|line| line.old_line.is_some())
        .count();
    let new_lines = hunk
        .lines
        .iter()
        .filter(|line| line.new_line.is_some())
        .count();

    rsx! {
        div { class: "diff__hunk",
            div { class: "diff__hunk-head",
                "@@ -{hunk.old_start},{old_lines} +{hunk.new_start},{new_lines} @@"
            }
            for (index , line) in hunk.lines.iter().enumerate() {
                div { key: "{index}", class: "diff__line {kind_class(line.kind)}",
                    span { class: "diff__gutter",
                        span { class: "diff__num", {number(line.old_line)} }
                        span { class: "diff__num", {number(line.new_line)} }
                    }
                    span { class: "diff__sign", {sign(line.kind)} }
                    span { class: "diff__text", "{line.text}" }
                }
            }
        }
    }
}

fn kind_class(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Added => "added",
        DiffLineKind::Removed => "removed",
        DiffLineKind::Context => "context",
    }
}

fn sign(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "−",
        DiffLineKind::Context => " ",
    }
}

fn number(line: Option<usize>) -> String {
    match line {
        Some(line) => line.to_string(),
        None => String::new(),
    }
}

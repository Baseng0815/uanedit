//! The open-file report: what was loaded, what was kept without being understood, and what was
//! irregular but not fatal (features.md §2E).

use dioxus::prelude::*;
use uanedit::report::{
    Finding,
    OpenReport,
    Position,
    Preserved,
    PreservedKind,
};

use crate::components::{
    Dialog,
    Icon,
};
use crate::views::editor::document::DocumentInfo;

/// Enough of either list to read; past this the dialog would be the file.
const MAX_ITEMS: usize = 200;

#[component]
pub fn ReportDialog(
    info: DocumentInfo,
    onclose: EventHandler<()>,
) -> Element {
    let report = info.report.clone();

    rsx! {
        Dialog {
            title: "Open report",
            icon: "fact_check",
            subtitle: info.name.clone(),
            wide: true,
            onclose,
            actions: rsx! {
                button { class: "button tonal", onclick: move |_| onclose.call(()), "Close" }
            },
            {verdict(&report)}
            {counts(&report)}
            {timing(&info)}
            {preserved(&report)}
            {findings(&report)}
        }
    }
}

fn verdict(report: &OpenReport) -> Element {
    rsx! {
        if report.is_clean() {
            div { class: "report-verdict",
                Icon { name: "task_alt", class: "small" }
                span { "The file read cleanly. Everything it carries came back into the model or was kept verbatim." }
            }
        } else {
            div { class: "report-verdict warn",
                Icon { name: "warning", class: "small" }
                span {
                    "{report.findings.len()} irregularities were read past. The file still loaded; saving writes what this editor understands."
                }
            }
        }
    }
}

fn counts(report: &OpenReport) -> Element {
    rsx! {
        section { class: "doc-section",
            header { class: "doc-section__head",
                span { class: "doc-section__title", "Loaded" }
            }
            div { class: "report-counts",
                Count { value: bytes(report.bytes), label: "bytes" }
                Count { value: report.nodes.to_string(), label: "nodes" }
                Count { value: report.namespaces.to_string(), label: "namespaces" }
                Count { value: report.models.to_string(), label: "models" }
                Count { value: report.aliases.to_string(), label: "aliases" }
                Count { value: report.preserved.len().to_string(), label: "preserved" }
            }
            if !report.nodes_by_class.is_empty() {
                div { class: "doc-subhead", span { "By node class" } }
                div { class: "doc-users",
                    for (class , count) in report.nodes_by_class.iter() {
                        span { key: "{class}", class: "chip", "{class} · {count}" }
                    }
                }
            }
        }
    }
}

#[component]
fn Count(
    value: String,
    label: &'static str,
) -> Element {
    rsx! {
        div { class: "report-count",
            span { class: "report-count__value", {value} }
            span { class: "report-count__label", {label} }
        }
    }
}

fn timing(info: &DocumentInfo) -> Element {
    let timing = info.timing;

    rsx! {
        section { class: "doc-section",
            header { class: "doc-section__head",
                span { class: "doc-section__title", "Time to open" }
            }
            if timing.already_open {
                div { class: "doc-empty", "The server answered from the document it already held, so nothing was read." }
            } else {
                div { class: "doc-users",
                    span { class: "chip mono", "parse {timing.parse_millis} ms" }
                    span { class: "chip mono", "index {timing.index_millis} ms" }
                    span { class: "chip mono", "dependencies {timing.dependencies_millis} ms" }
                    span { class: "chip mono", "total {timing.total_millis} ms" }
                }
            }
        }
    }
}

fn preserved(report: &OpenReport) -> Element {
    let extra = report.preserved.len().saturating_sub(MAX_ITEMS);

    rsx! {
        section { class: "doc-section",
            header { class: "doc-section__head",
                span { class: "doc-section__title", "Kept, not understood" }
                span { class: "chip mono", "{report.preserved.len()}" }
            }
            span { class: "doc-section__hint",
                "Foreign extensions and attributes from a newer schema revision. They are written back exactly as they were read."
            }
            if report.preserved.is_empty() {
                div { class: "doc-empty", "Nothing: every element in the file is one this editor models." }
            }
            for (index , item) in report.preserved.iter().take(MAX_ITEMS).enumerate() {
                PreservedItem { key: "{index}", item: item.clone() }
            }
            if extra > 0 {
                div { class: "report-more", "+{extra} more" }
            }
        }
    }
}

#[component]
fn PreservedItem(item: Preserved) -> Element {
    rsx! {
        div { class: "report-item",
            span { class: "report-item__what",
                span { class: "report-item__kind", {kind(item.kind)} }
                span { class: "mono", "{item.name}" }
                if let Some(owner) = &item.owner {
                    span { class: "mono", " · {owner}" }
                }
            }
            span { class: "report-item__where", {place(item.position)} }
        }
    }
}

fn findings(report: &OpenReport) -> Element {
    let extra = report.findings.len().saturating_sub(MAX_ITEMS);

    rsx! {
        section { class: "doc-section",
            header { class: "doc-section__head",
                span { class: "doc-section__title", "Read past" }
                span { class: "chip mono", "{report.findings.len()}" }
            }
            if report.findings.is_empty() {
                div { class: "doc-empty", "Nothing: the document matched the schema this editor reads." }
            }
            for (index , finding) in report.findings.iter().take(MAX_ITEMS).enumerate() {
                FindingItem { key: "{index}", finding: finding.clone() }
            }
            if extra > 0 {
                div { class: "report-more", "+{extra} more" }
            }
        }
    }
}

#[component]
fn FindingItem(finding: Finding) -> Element {
    rsx! {
        div { class: "report-item",
            span { class: "report-item__what",
                span { "{finding.diagnosis}" }
                if let Some(owner) = &finding.owner {
                    span { class: "mono", " · {owner}" }
                }
            }
            span { class: "report-item__where", {place(finding.position)} }
        }
    }
}

fn kind(kind: PreservedKind) -> &'static str {
    match kind {
        PreservedKind::Extension => "extension",
        PreservedKind::UnknownElement => "element",
        PreservedKind::UnknownAttribute => "attribute",
        PreservedKind::OpaqueValue => "value",
    }
}

fn place(position: Position) -> String {
    format!("line {}:{}", position.line, position.column)
}

fn bytes(count: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = 1024 * KB;

    match count {
        0..KB => count.to_string(),
        KB..MB => format!("{}.{} k", count / KB, (count % KB) * 10 / KB),
        _ => format!("{}.{} M", count / MB, (count % MB) * 10 / MB),
    }
}

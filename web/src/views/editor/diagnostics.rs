//! How a finding looks wherever it is shown: the validation panel, a refusal, an override result.
//!
//! One rendering, because guardrails.md §2 asks for one diagnostic vocabulary — a stable code, a
//! one-line message, and the spec clause behind it in the manner of `rustc --explain`.

use dioxus::prelude::*;
use uanedit::edit::Refusal;
use uanedit::rules::code::{
    DiagnosticCode,
    Severity,
};
use uanedit::rules::finding::{
    Finding,
    Origin,
};

use crate::components::Icon;

pub fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "sev sev--error",
        Severity::Warning => "sev sev--warning",
    }
}

pub fn severity_icon(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// The badge that says whose doing a finding is (guardrails.md §4, §5).
pub fn origin_badge(finding: &Finding) -> Element {
    if finding.via_override {
        return rsx! {
            span { class: "badge badge--override", title: "Introduced by an operation forced past the engine",
                Icon { name: "bolt", class: "small" }
                "override"
            }
        };
    }
    match finding.origin {
        Origin::Inherited => rsx! {
            span { class: "badge", title: "Present in the file as it was opened", "inherited" }
        },
        Origin::Introduced => rsx! {
            span { class: "badge badge--introduced", title: "Created by an edit made here", "introduced" }
        },
    }
}

/// The `rustc --explain` text for one rule, collapsed until asked for.
#[component]
pub fn Explain(code: DiagnosticCode) -> Element {
    rsx! {
        details { class: "explain",
            summary { class: "explain__summary type-label-small",
                Icon { name: "menu_book", class: "small" }
                "Explain {code.id()}"
            }
            div { class: "explain__body type-body-small",
                p { class: "explain__summary-line", "{code.summary()}" }
                p { "{code.explanation()}" }
            }
        }
    }
}

/// A refusal, shown as the findings that explain it, with the override as a distinct last resort.
///
/// The override is deliberately not the dialog's default action: it is a second, alarming button
/// with a reason beside it (guardrails.md §5).
#[component]
pub fn RefusalNotice(
    refusal: Refusal,
    onoverride: EventHandler<Option<String>>,
) -> Element {
    let mut reason = use_signal(String::new);
    let findings: Vec<Finding> = refusal
        .verdict()
        .map(|verdict| verdict.findings.clone())
        .unwrap_or_default();
    let overridable = refusal.is_overridable();

    rsx! {
        div {
            class: "refusal",
            // A long wizard scrolls the refusal below the fold, where the action that raised it
            // reads as having done nothing at all.
            onmounted: move |event| {
                let element = event.data();
                spawn(async move {
                    let _ = element.scroll_to(ScrollBehavior::Smooth).await;
                });
            },
            header { class: "refusal__head",
                Icon { name: "block", class: "small" }
                span { class: "type-label", "Refused — this would introduce a specification error" }
            }
            if findings.is_empty() {
                p { class: "refusal__line type-body-small", "{refusal}" }
            } else {
                for (position , finding) in findings.iter().enumerate() {
                    div { key: "{position}", class: "refusal__finding",
                        div { class: "refusal__finding-head",
                            span { class: severity_class(finding.severity),
                                Icon { name: severity_icon(finding.severity), class: "small" }
                                "{finding.severity}"
                            }
                            span { class: "mono refusal__code", "{finding.code.id()}" }
                            span { class: "refusal__message", "{finding.message}" }
                        }
                        Explain { code: finding.code }
                    }
                }
            }
            if overridable {
                div { class: "override",
                    div { class: "override__head",
                        Icon { name: "bolt", class: "small" }
                        span { class: "type-label", "Override" }
                    }
                    p { class: "override__body type-body-small",
                        "Performing it anyway leaves the finding above in the file. It is recorded as introduced and attributed to this override, and it will not be hidden."
                    }
                    input {
                        class: "override__reason",
                        value: "{reason}",
                        placeholder: "Why is this necessary? (recorded with the finding)",
                        oninput: move |event| reason.set(event.value()),
                    }
                    button {
                        class: "button override__action",
                        onclick: move |_| {
                            let text = reason.peek().trim().to_owned();
                            onoverride.call((!text.is_empty()).then_some(text));
                        },
                        Icon { name: "bolt", class: "small" }
                        "Apply anyway (override)"
                    }
                }
            }
        }
    }
}

/// What an override let through, said at the moment it happened rather than only in the panel.
#[component]
pub fn OverrideResult(findings: Vec<Finding>) -> Element {
    if findings.is_empty() {
        return rsx! {};
    }

    rsx! {
        div { class: "override-result",
            div { class: "override-result__head",
                Icon { name: "bolt", class: "small" }
                span { class: "type-label", {overridden_title(findings.len())} }
            }
            for (position , finding) in findings.iter().enumerate() {
                div { key: "{position}", class: "override-result__line",
                    span { class: "mono refusal__code", "{finding.code.id()}" }
                    span { "{finding.message}" }
                }
            }
            span { class: "type-label-small override-result__foot",
                "Attributed to the override in the Validation tab."
            }
        }
    }
}

fn overridden_title(count: usize) -> String {
    match count {
        1 => "Applied past the engine · one error introduced".to_owned(),
        count => format!("Applied past the engine · {count} errors introduced"),
    }
}

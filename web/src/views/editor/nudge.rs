//! The version-bump nudge: the file is already saved, and its model says it did not change
//! (features.md §2C).
//!
//! Nothing here gates the save. Accepting the nudge is one more edit — a whole model entry through
//! `SetModelEntry` — followed by a second save.

use dioxus::prelude::*;
use uanedit::Operation;
use uanedit::types::DateTime;

use crate::api::{
    VersionNudge,
    current_time,
};
use crate::components::{
    Dialog,
    Icon,
};
use crate::session::{
    EditResult,
    EditorHandle,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bump {
    Patch,
    Minor,
    Major,
}

#[component]
pub fn VersionNudgeDialog(
    nudge: VersionNudge,
    onclose: EventHandler<()>,
) -> Element {
    let handle: EditorHandle = use_context();
    let current = nudge.model_version.clone().unwrap_or_default();
    let uri = use_signal(|| nudge.model_uri.clone());
    let mut typed = use_signal(|| current.clone());
    let mut stamp = use_signal(|| true);
    let mut busy = use_signal(|| false);
    let mut failure = use_signal(|| None::<String>);

    let mut apply = move |version: String| {
        if version.trim().is_empty() || *busy.peek() {
            return;
        }
        let model_uri = uri.peek().clone();
        let wants_stamp = *stamp.peek();
        busy.set(true);
        failure.set(None);
        spawn(async move {
            let (mut busy, mut failure) = (busy, failure);
            let now = match wants_stamp {
                true => match current_time().await {
                    Ok(now) => Some(now),
                    Err(error) => {
                        busy.set(false);
                        failure.set(Some(format!("The server clock could not be read: {error}")));
                        return;
                    }
                },
                false => None,
            };
            match bump_model(handle, &model_uri, version, now) {
                Ok(()) => {
                    busy.set(false);
                    // Out of this dialog's scope on purpose: closing it must not cancel the save
                    // the bump is for.
                    dioxus::core::spawn_forever(async move { handle.save() });
                    onclose.call(());
                }
                Err(message) => {
                    busy.set(false);
                    failure.set(Some(message));
                }
            }
        });
    };

    let working = busy();
    let dotted = bumped(&current, Bump::Patch).is_some();

    rsx! {
        Dialog {
            title: "The model changed",
            icon: "history",
            subtitle: nudge.model_uri.clone(),
            onclose,
            actions: rsx! {
                span { class: "dialog__actions-note", "The file is already saved." }
                button {
                    class: "button text",
                    disabled: working,
                    onclick: move |_| onclose.call(()),
                    "Keep as is"
                }
            },
            div { class: "doc-form",
                span { class: "type-body",
                    "This model's content changed and its ModelVersion and PublicationDate did not. A consumer of the file has no other way to tell that it moved."
                }
                div { class: "doc-users",
                    span { class: "chip mono", "ModelVersion {stated(nudge.model_version.as_deref())}" }
                    span { class: "chip mono", "PublicationDate {stated(nudge.publication_date.as_deref())}" }
                }
                label { class: "field__toggle",
                    input {
                        r#type: "checkbox",
                        checked: stamp(),
                        disabled: working,
                        onchange: move |event| stamp.set(event.checked()),
                    }
                    span { class: "field__toggle-label", "Also set PublicationDate to now" }
                }
                if let Some(failed) = failure() {
                    div { class: "notice",
                        Icon { name: "error" }
                        span { {failed} }
                    }
                }
                if dotted {
                    div { class: "field",
                        span { class: "field__label", "Bump ModelVersion and save again" }
                        div { class: "doc-users",
                            BumpButton {
                                label: "Patch",
                                next: bumped(&current, Bump::Patch).unwrap_or_default(),
                                disabled: working,
                                onpick: apply,
                            }
                            BumpButton {
                                label: "Minor",
                                next: bumped(&current, Bump::Minor).unwrap_or_default(),
                                disabled: working,
                                onpick: apply,
                            }
                            BumpButton {
                                label: "Major",
                                next: bumped(&current, Bump::Major).unwrap_or_default(),
                                disabled: working,
                                onpick: apply,
                            }
                        }
                    }
                } else {
                    div { class: "doc-add",
                        div { class: "field",
                            span { class: "field__label", "ModelVersion" }
                            input {
                                class: "field__input mono",
                                value: "{typed}",
                                placeholder: "1.0.1",
                                disabled: working,
                                oninput: move |event| typed.set(event.value()),
                            }
                            span { class: "field__hint",
                                "The value on file is not a dotted number, so there is no bump to offer."
                            }
                        }
                        button {
                            class: "button doc-add__button",
                            disabled: working,
                            onclick: move |_| {
                                let version = typed.peek().clone();
                                apply(version);
                            },
                            "Set and save"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn BumpButton(
    label: &'static str,
    next: String,
    disabled: bool,
    onpick: EventHandler<String>,
) -> Element {
    let picked = next.clone();

    rsx! {
        button {
            class: "button tonal",
            disabled,
            title: "Set ModelVersion to {next} and save again",
            onclick: move |_| onpick.call(picked.clone()),
            "{label} · {next}"
        }
    }
}

/// Rewrites the model entry the nudge names, from the entry as it stands now.
fn bump_model(
    handle: EditorHandle,
    uri: &str,
    version: String,
    now: Option<DateTime>,
) -> Result<(), String> {
    let found = handle
        .with_space(|space| {
            let models = &space.primary().models;
            models
                .iter()
                .position(|model| model.model_uri == uri)
                .and_then(|index| models.get(index).cloned().map(|entry| (index, entry)))
        })
        .flatten();
    let Some((index, mut entry)) = found else {
        return Err(format!("The document no longer declares the model {uri}."));
    };

    entry.model_version = Some(version.trim().to_owned());
    if let Some(now) = now {
        entry.publication_date = Some(now);
    }
    match handle.apply(Operation::SetModelEntry {
        index,
        entry: Box::new(entry),
    }) {
        EditResult::Refused(refusal) => Err(refusal.to_string()),
        EditResult::Closed => Err("No document is open any more.".to_owned()),
        EditResult::Applied { .. } | EditResult::Unchanged => Ok(()),
    }
}

fn stated(value: Option<&str>) -> String {
    match value {
        Some(value) if !value.is_empty() => value.to_owned(),
        _ => "not set".to_owned(),
    }
}

/// The next dotted version, keeping the width of every segment so `1.05.03` becomes `1.05.04`.
fn bumped(
    version: &str,
    bump: Bump,
) -> Option<String> {
    let parts: Vec<&str> = version.split('.').collect();
    if !(1..=3).contains(&parts.len()) {
        return None;
    }
    let mut segments: Vec<(u64, usize)> = Vec::new();
    for part in parts {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        segments.push((part.parse().ok()?, part.len()));
    }
    while segments.len() < 3 {
        segments.push((0, 1));
    }

    let position = match bump {
        Bump::Major => 0,
        Bump::Minor => 1,
        Bump::Patch => 2,
    };
    if let Some(segment) = segments.get_mut(position) {
        segment.0 = segment.0.saturating_add(1);
    }
    for segment in segments.iter_mut().skip(position + 1) {
        segment.0 = 0;
    }

    Some(
        segments
            .iter()
            .map(|(value, width)| format!("{value:0width$}"))
            .collect::<Vec<String>>()
            .join("."),
    )
}

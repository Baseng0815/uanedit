//! The inspector's input primitives.
//!
//! Every one of them holds a draft and commits on change rather than per keystroke, and re-seeds
//! that draft from the model whenever the model value or `nonce` moves — `nonce` being how a
//! refused edit puts the control back to what the document actually says.

use dioxus::prelude::*;
use uanedit::types::localized_text::LocalizedText;

use crate::components::Icon;

/// One choice in a picker. `detail` is the NodeId, shown beside the name in mono.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub value: String,
    pub label: String,
    pub detail: Option<String>,
}

#[component]
pub fn TextField(
    label: String,
    value: String,
    nonce: u64,
    oncommit: EventHandler<String>,
    #[props(default)] placeholder: String,
    #[props(default)] mono: bool,
    #[props(default)] disabled: bool,
    error: Option<String>,
    hint: Option<String>,
) -> Element {
    let mut draft = use_signal(|| value.clone());
    use_effect(use_reactive!(|(value, nonce)| {
        let _ = nonce;
        draft.set(value);
    }));

    rsx! {
        div { class: field_class(error.is_some()),
            span { class: "field__label", {label} }
            input {
                class: if mono { "field__input mono" } else { "field__input" },
                value: "{draft}",
                placeholder,
                disabled,
                oninput: move |event| draft.set(event.value()),
                onchange: move |event| oncommit.call(event.value()),
            }
            FieldNote { error: error.clone(), hint }
        }
    }
}

#[component]
pub fn BoolField(
    label: String,
    value: bool,
    nonce: u64,
    oncommit: EventHandler<bool>,
    #[props(default)] disabled: bool,
    error: Option<String>,
    hint: Option<String>,
) -> Element {
    let mut draft = use_signal(|| value);
    use_effect(use_reactive!(|(value, nonce)| {
        let _ = nonce;
        draft.set(value);
    }));

    rsx! {
        div { class: field_class(error.is_some()),
            label { class: "field__toggle",
                input {
                    r#type: "checkbox",
                    checked: draft(),
                    disabled,
                    onchange: move |event| {
                        draft.set(event.checked());
                        oncommit.call(event.checked());
                    },
                }
                span { class: "field__toggle-label", {label} }
            }
            FieldNote { error: error.clone(), hint }
        }
    }
}

#[component]
pub fn SelectField(
    label: String,
    value: String,
    choices: Vec<Choice>,
    nonce: u64,
    oncommit: EventHandler<String>,
    #[props(default)] disabled: bool,
    error: Option<String>,
    hint: Option<String>,
) -> Element {
    let mut draft = use_signal(|| value.clone());
    use_effect(use_reactive!(|(value, nonce)| {
        let _ = nonce;
        draft.set(value);
    }));

    let chosen = draft();
    let detail = choices
        .iter()
        .find(|choice| choice.value == chosen)
        .and_then(|choice| choice.detail.clone());

    rsx! {
        div { class: field_class(error.is_some()),
            span { class: "field__label", {label} }
            div { class: "field__row",
                select {
                    class: "field__select",
                    value: chosen.clone(),
                    disabled,
                    onchange: move |event| {
                        draft.set(event.value());
                        oncommit.call(event.value());
                    },
                    // `value` on a select is not applied when the element mounts in a later diff,
                    // so the chosen option marks itself.
                    for choice in choices.iter() {
                        option {
                            key: "{choice.value}",
                            value: "{choice.value}",
                            selected: choice.value == chosen,
                            "{choice.label}"
                        }
                    }
                }
                if let Some(detail) = detail {
                    span { class: "field__detail mono", {detail} }
                }
            }
            FieldNote { error: error.clone(), hint }
        }
    }
}

/// A QualifiedName: the namespace it is stated in, and the name itself.
#[component]
pub fn QualifiedNameField(
    label: String,
    namespace: u16,
    name: String,
    namespaces: Vec<Choice>,
    nonce: u64,
    oncommit: EventHandler<(u16, String)>,
    #[props(default)] disabled: bool,
    error: Option<String>,
) -> Element {
    let mut draft = use_signal(|| (namespace, name.clone()));
    use_effect(use_reactive!(|(namespace, name, nonce)| {
        let _ = nonce;
        draft.set((namespace, name));
    }));

    let (index, text) = draft();
    let commit = move || oncommit.call(draft.peek().clone());

    rsx! {
        div { class: field_class(error.is_some()),
            span { class: "field__label", {label} }
            div { class: "field__row",
                select {
                    class: "field__select narrow mono",
                    value: "{index}",
                    disabled,
                    onchange: move |event| {
                        let index = event.value().parse().unwrap_or(0);
                        draft.with_mut(|draft| draft.0 = index);
                        commit();
                    },
                    for choice in namespaces.iter() {
                        option {
                            key: "{choice.value}",
                            value: "{choice.value}",
                            selected: choice.value == index.to_string(),
                            "{choice.label}"
                        }
                    }
                }
                input {
                    class: "field__input",
                    value: text.clone(),
                    disabled,
                    oninput: move |event| {
                        let name = event.value();
                        draft.with_mut(|draft| draft.1 = name);
                    },
                    onchange: move |_| commit(),
                }
            }
            FieldNote { error: error.clone(), hint: None }
        }
    }
}

/// One row per locale, which is what the file carries (features.md §2A).
#[component]
pub fn LocalizedTextField(
    label: String,
    value: Vec<LocalizedText>,
    nonce: u64,
    oncommit: EventHandler<Vec<LocalizedText>>,
    #[props(default)] disabled: bool,
    error: Option<String>,
    hint: Option<String>,
) -> Element {
    let mut draft = use_signal(|| value.clone());
    use_effect(use_reactive!(|(value, nonce)| {
        let _ = nonce;
        draft.set(value);
    }));

    let commit = move || oncommit.call(draft.peek().clone());
    let entries = draft();

    rsx! {
        div { class: field_class(error.is_some()),
            span { class: "field__label", {label} }
            div { class: "locales",
                for (position , entry) in entries.iter().enumerate() {
                    div { key: "{position}", class: "locales__row",
                        input {
                            class: "locales__locale mono",
                            value: "{entry.locale.clone().unwrap_or_default()}",
                            placeholder: "locale",
                            disabled,
                            oninput: move |event| {
                                let locale = event.value();
                                draft.with_mut(|texts| {
                                    if let Some(text) = texts.get_mut(position) {
                                        text.locale = (!locale.is_empty()).then_some(locale);
                                    }
                                });
                            },
                            onchange: move |_| commit(),
                        }
                        input {
                            class: "locales__text",
                            value: "{entry.text}",
                            placeholder: "text",
                            disabled,
                            oninput: move |event| {
                                let value = event.value();
                                draft.with_mut(|texts| {
                                    if let Some(text) = texts.get_mut(position) {
                                        text.text = value;
                                    }
                                });
                            },
                            onchange: move |_| commit(),
                        }
                        button {
                            class: "icon-button tiny",
                            disabled,
                            title: "Remove this locale",
                            onclick: move |_| {
                                draft.with_mut(|texts| {
                                    if position < texts.len() {
                                        texts.remove(position);
                                    }
                                });
                                commit();
                            },
                            Icon { name: "close", class: "small" }
                        }
                    }
                }
                if !disabled {
                    button {
                        class: "button text tiny",
                        onclick: move |_| draft.with_mut(|texts| texts.push(LocalizedText::default())),
                        Icon { name: "add", class: "small" }
                        "Add locale"
                    }
                }
            }
            FieldNote { error: error.clone(), hint }
        }
    }
}

/// One checkbox per flag the crate names; bits it does not name are left alone.
#[component]
pub fn MaskField(
    label: String,
    flags: Vec<(String, u32, bool)>,
    unknown: u32,
    oncommit: EventHandler<(u32, bool)>,
    #[props(default)] disabled: bool,
    error: Option<String>,
) -> Element {
    rsx! {
        div { class: field_class(error.is_some()),
            span { class: "field__label", {label} }
            div { class: "flags",
                for (name , bits , set) in flags.iter().cloned() {
                    label { key: "{bits}", class: "flags__flag",
                        input {
                            r#type: "checkbox",
                            checked: set,
                            disabled,
                            onchange: move |event| oncommit.call((bits, event.checked())),
                        }
                        span { {name} }
                    }
                }
                if unknown != 0 {
                    span { class: "chip mono", title: "Bits this editor does not name, kept as they were",
                        "+{unknown:#x}"
                    }
                }
            }
            FieldNote { error: error.clone(), hint: None }
        }
    }
}

/// A value the editor shows but does not yet edit (features.md §2D).
#[component]
pub fn ReadOnlyBlock(
    label: String,
    text: String,
    note: Option<String>,
) -> Element {
    rsx! {
        div { class: "field",
            span { class: "field__label", {label} }
            pre { class: "field__block mono", {text} }
            if let Some(note) = note {
                span { class: "field__hint", {note} }
            }
        }
    }
}

#[component]
fn FieldNote(
    error: Option<String>,
    hint: Option<String>,
) -> Element {
    rsx! {
        if let Some(error) = error {
            span { class: "field__error",
                Icon { name: "error", class: "small" }
                {error}
            }
        } else if let Some(hint) = hint {
            span { class: "field__hint", {hint} }
        }
    }
}

fn field_class(failed: bool) -> &'static str {
    match failed {
        true => "field invalid",
        false => "field",
    }
}

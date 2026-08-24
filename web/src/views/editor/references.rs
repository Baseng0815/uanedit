//! The references panel, and the two dialogs that write a reference.
//!
//! A reference may be stated on either end of the file — or, in the OPC house style, on both — so
//! the panel shows the forward references of the selected node and then the inverse ones the graph
//! answers for, and it says on which node each statement actually lives — that is what an edit
//! changes; removing a reference removes every statement of it.

use dioxus::prelude::*;
use uanedit::edit::{
    AddReference,
    ReferenceKey,
    Refusal,
};
use uanedit::nodes::NodeClass;
use uanedit::rules::code::DiagnosticCode;
use uanedit::rules::query;
use uanedit::space::AddressSpace;
use uanedit::types::node_id::NodeId;
use uanedit::{
    Operation,
    ids,
};

use crate::components::Icon;
use crate::session::{
    EditorHandle,
    Outcome,
};
use crate::views::editor::diagnostics::RefusalNotice;
use crate::views::editor::icons::class_icon;
use crate::views::editor::picker::{
    NodePicker,
    candidate,
    label_of,
};
use crate::views::editor::shell::{
    Dialog,
    Dialogs,
    Navigate,
    Request,
};

/// What the panel can reach without a hook, so the row and section helpers stay plain functions.
#[derive(Clone, Copy)]
struct Panel {
    handle: EditorHandle,
    navigate: Navigate,
    dialogs: Dialogs,
    refused: Signal<Option<(Refusal, Operation)>>,
}

/// How the reference reads from the selected node, and where its one statement lives.
#[derive(Clone, PartialEq)]
struct Row {
    reference: ReferenceKey,
    other: NodeId,
    label: String,
    identifier: String,
    node_class: NodeClass,
    /// The node whose `References` element states it, which is the node an edit changes.
    holder: NodeId,
    holder_label: String,
    stated_here: bool,
    editable: bool,
    unresolved: bool,
}

#[derive(Clone, PartialEq)]
struct Group {
    name: String,
    identifier: String,
    rows: Vec<Row>,
}

#[component]
pub fn ReferencesPanel() -> Element {
    let handle: EditorHandle = use_context();
    let navigate: Navigate = use_context();
    let dialogs: Dialogs = use_context();
    let mut refused = use_signal(|| None::<(Refusal, Operation)>);

    let sections = use_memo(move || {
        let _ = *handle.revision.read();
        let Some(node_id) = handle.selection.read().clone() else {
            return (Vec::new(), Vec::new());
        };
        handle
            .with_space(|space| (groups(space, &node_id, true), groups(space, &node_id, false)))
            .unwrap_or_default()
    });

    let selected = handle.selection.read().clone();
    use_effect(use_reactive!(|selected| {
        let _ = selected;
        refused.set(None);
    }));

    let Some(node_id) = handle.selection.read().clone() else {
        return rsx! {
            div { class: "pane__placeholder",
                Icon { name: "link" }
                span { class: "type-body-small", "Select a node to see what it references and what references it." }
            }
        };
    };

    let panel = Panel {
        handle,
        navigate,
        dialogs,
        refused,
    };
    let read_only = handle
        .with_space(|space| space.is_read_only(&node_id))
        .unwrap_or(true);
    let (forward, inverse) = sections.read().clone();
    let empty = forward.is_empty() && inverse.is_empty();
    let refusal_now = refused.read().clone();
    let source = node_id.clone();

    rsx! {
        div { class: "refs",
            div { class: "refs__actions",
                button {
                    class: "button tonal tiny",
                    title: match read_only {
                        true => "This node belongs to a nodeset this editor does not change",
                        false => "Add a reference between this node and another",
                    },
                    disabled: read_only,
                    onclick: move |_| dialogs.open(Request::AddReference { node: source.clone() }),
                    Icon { name: "add_link", class: "small" }
                    "Add reference"
                }
            }
            if let Some((refusal, operation)) = refusal_now {
                RefusalNotice {
                    refusal,
                    onoverride: move |reason: Option<String>| apply(panel, operation.clone(), Some(reason)),
                }
            }
            if empty {
                div { class: "pane__placeholder",
                    Icon { name: "link_off" }
                    span { class: "type-body-small", "No loaded nodeset states a reference touching this node." }
                }
            }
            {section(panel, "Forward", "arrow_forward", "The references this node states, or that name it as their source", &forward)}
            {section(panel, "Inverse", "arrow_back", "The references that name this node as their target", &inverse)}
        }
    }
}

fn section(
    panel: Panel,
    title: &'static str,
    icon: &'static str,
    hint: &'static str,
    groups: &[Group],
) -> Element {
    if groups.is_empty() {
        return rsx! {};
    }
    let total: usize = groups.iter().map(|group| group.rows.len()).sum();

    rsx! {
        section { class: "refs__section",
            header { class: "refs__section-head", title: hint,
                Icon { name: icon, class: "small" }
                span { class: "type-label refs__section-title", {title} }
                div { class: "refs__spacer" }
                span { class: "chip mono", "{total}" }
            }
            for group in groups.iter() {
                div { key: "{group.identifier}", class: "refs__group",
                    div { class: "refs__group-head",
                        span { class: "refs__type type-label-small", "{group.name}" }
                        span { class: "refs__type-id mono", "{group.identifier}" }
                    }
                    for row in group.rows.iter() {
                        {reference_row(panel, row)}
                    }
                }
            }
        }
    }
}

fn reference_row(
    panel: Panel,
    row: &Row,
) -> Element {
    let navigate = panel.navigate;
    let dialogs = panel.dialogs;
    let target = row.other.clone();
    let remove = Operation::RemoveReference(row.reference.clone());
    let reference = row.reference.clone();
    let holder = row.holder.clone();
    let mut class = String::from("refs__row");
    if !row.editable {
        class.push_str(" read-only");
    }
    let held = match row.stated_here {
        true => "Stated on this node".to_owned(),
        false => format!("Stated on {}", row.holder_label),
    };
    let glyph = match row.stated_here {
        true => "edit_note",
        false => "subdirectory_arrow_left",
    };

    rsx! {
        div { key: "{row.identifier}", class: class,
            span {
                class: "refs__target",
                title: "{row.label} · {row.identifier}",
                onclick: move |_| navigate.to(target.clone()),
                Icon { name: class_icon(row.node_class), class: "tree__class small" }
                span { class: "refs__label", "{row.label}" }
                span { class: "refs__id mono", "{row.identifier}" }
            }
            if row.unresolved {
                span { class: "chip", title: "No loaded nodeset defines this NodeId", "unresolved" }
            }
            span { class: "refs__held", title: held,
                Icon { name: glyph, class: "small" }
            }
            span { class: "refs__row-actions",
                if row.editable {
                    button {
                        class: "icon-button tiny",
                        title: "Point this reference at another node",
                        onclick: move |_| {
                            dialogs
                                .open(Request::Retarget {
                                    reference: reference.clone(),
                                    held_by: holder.clone(),
                                })
                        },
                        Icon { name: "swap_horiz", class: "small" }
                    }
                    button {
                        class: "icon-button tiny",
                        title: "Remove this reference",
                        onclick: move |_| apply(panel, remove.clone(), None),
                        Icon { name: "link_off", class: "small" }
                    }
                } else {
                    Icon { name: "lock", class: "small refs__locked" }
                }
            }
        }
    }
}

/// Performs the operation and puts whatever came back where the panel shows it.
fn apply(
    panel: Panel,
    operation: Operation,
    override_reason: Option<Option<String>>,
) {
    let handle = panel.handle;
    let mut refused = panel.refused;
    let outcome = match override_reason {
        Some(reason) => handle.perform_with_override(operation.clone(), reason),
        None => handle.perform(operation.clone()),
    };
    match outcome {
        Outcome::Refused(refusal) => refused.set(Some((refusal, operation))),
        Outcome::Applied(done) => {
            refused.set(None);
            handle.say(done.status());
            if !done.overridden.is_empty() {
                panel.dialogs.report(done.overridden);
            }
        }
        Outcome::Unchanged | Outcome::Closed => refused.set(None),
    }
}

/// The node's references of one direction, grouped by reference type in the order they are stated.
fn groups(
    space: &AddressSpace,
    node_id: &NodeId,
    forward: bool,
) -> Vec<Group> {
    let views = match forward {
        true => space.forward_references(node_id),
        false => space.inverse_references(node_id),
    };
    let mut groups: Vec<Group> = Vec::new();
    for view in views {
        let (source, target) = view.ends(node_id);
        let holder = match view.storage.is_synthesized() {
            true => view.other.clone(),
            false => node_id.clone(),
        };
        let other = candidate(space, &view.other);
        let row = Row {
            reference: ReferenceKey::new(source.clone(), view.reference_type.clone(), target.clone()),
            unresolved: !space.contains(&view.other),
            label: other.label,
            identifier: other.identifier,
            node_class: other.node_class,
            holder_label: label_of(space, &holder),
            stated_here: !view.storage.is_synthesized(),
            editable: !space.is_read_only(&holder),
            holder,
            other: view.other.clone(),
        };
        let identifier = view.reference_type.to_string();
        match groups
            .iter_mut()
            .find(|group| group.identifier == identifier)
        {
            Some(group) => group.rows.push(row),
            None => groups.push(Group {
                name: label_of(space, &view.reference_type),
                identifier,
                rows: vec![row],
            }),
        }
    }
    groups
}

/// Adding a reference: the other end first, then the types the engine says are legal between them.
#[component]
pub fn AddReferenceDialog(node: NodeId) -> Element {
    let handle: EditorHandle = use_context();
    let dialogs: Dialogs = use_context();
    let mut forward = use_signal(|| true);
    let mut other = use_signal(|| None::<NodeId>);
    let mut reference_type = use_signal(|| None::<NodeId>);
    let mut picking = use_signal(|| false);
    let refused = use_signal(|| None::<Refusal>);

    let subject = node.clone();
    let ends = use_memo(move || {
        let picked = other.read().clone()?;
        Some(match *forward.read() {
            true => (subject.clone(), picked),
            false => (picked, subject.clone()),
        })
    });

    let legal = use_memo(move || {
        let _ = *handle.revision.read();
        let Some((source, target)) = ends.read().clone() else {
            return Vec::new();
        };
        handle
            .with_space(|space| {
                query::legal_reference_types(space, &source, &target)
                    .into_iter()
                    .map(|reference_type| {
                        let named = candidate(space, &reference_type);
                        (named.label, named.identifier)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });

    let reasons = use_memo(move || {
        if !legal.read().is_empty() {
            return Vec::new();
        }
        let Some((source, target)) = ends.read().clone() else {
            return Vec::new();
        };
        handle
            .with_space(|space| why_nothing_is_legal(space, &source, &target))
            .unwrap_or_default()
    });

    let named = other
        .read()
        .clone()
        .and_then(|node_id| handle.with_space(|space| candidate(space, &node_id)));
    let choices = legal.read().clone();
    let pair = ends.read().clone();
    let picked = reference_type.read().clone();
    let chosen_type = picked.as_ref().map(ToString::to_string).unwrap_or_default();
    let operation = pair
        .clone()
        .zip(picked.clone())
        .map(|((source, target), reference_type)| {
            Operation::AddReference(AddReference::new(source, reference_type, target))
        });
    let ready = operation.is_some();
    let confirm = operation.clone();
    let overridden = operation.clone();
    let refusal_now = refused.read().clone();
    let forward_now = *forward.read();
    let chosen_label = named
        .as_ref()
        .map_or_else(|| "Choose a node…".to_owned(), |named| named.label.clone());

    rsx! {
        Dialog {
            title: "Add reference".to_owned(),
            icon: "add_link",
            subtitle: "Only one end of a reference is ever written to the file; the editor states it on the end it may change."
                .to_owned(),
            onclose: move |_| dialogs.close(),
            div { class: "dialog__body",
                div { class: "field",
                    span { class: "field__label", "Direction" }
                    div { class: "segmented",
                        button {
                            class: if forward_now { "segmented__option on" } else { "segmented__option" },
                            onclick: move |_| {
                                forward.set(true);
                                reference_type.set(None);
                            },
                            Icon { name: "arrow_forward", class: "small" }
                            "This node is the source"
                        }
                        button {
                            class: if forward_now { "segmented__option" } else { "segmented__option on" },
                            onclick: move |_| {
                                forward.set(false);
                                reference_type.set(None);
                            },
                            Icon { name: "arrow_back", class: "small" }
                            "This node is the target"
                        }
                    }
                }
                div { class: "field",
                    span { class: "field__label", "Other node" }
                    div { class: "field__row",
                        button {
                            class: "button tonal tiny",
                            onclick: move |_| picking.set(true),
                            Icon { name: "search", class: "small" }
                            {chosen_label}
                        }
                        if let Some(named) = named.clone() {
                            span { class: "field__detail mono", {named.identifier} }
                        }
                    }
                }
                div { class: "field",
                    span { class: "field__label", "Reference type" }
                    if pair.is_none() {
                        span { class: "field__hint", "Choose the other node first — which types are legal depends on both ends." }
                    } else if choices.is_empty() {
                        div { class: "refs__nothing",
                            span { class: "type-body-small",
                                "No reference type is legal between these two nodes. Every candidate is rejected for these reasons:"
                            }
                            for (code , message) in reasons.read().iter() {
                                div { key: "{code}", class: "refs__reason",
                                    span { class: "mono refusal__code", "{code}" }
                                    span { class: "type-body-small", {message.clone()} }
                                }
                            }
                        }
                    } else {
                        select {
                            class: "field__select",
                            value: picked.as_ref().map(ToString::to_string).unwrap_or_default(),
                            onchange: move |event| reference_type.set(event.value().parse::<NodeId>().ok()),
                            option { value: "", selected: chosen_type.is_empty(), "Choose a reference type…" }
                            for (label , identifier) in choices.iter() {
                                option {
                                    key: "{identifier}",
                                    value: identifier.clone(),
                                    selected: *identifier == chosen_type,
                                    "{label} · {identifier}"
                                }
                            }
                        }
                    }
                }
                if let Some(refusal) = refusal_now {
                    RefusalNotice {
                        refusal,
                        onoverride: move |reason: Option<String>| {
                            if let Some(operation) = overridden.clone() {
                                finish(handle, dialogs, operation, Some(reason), refused);
                            }
                        },
                    }
                }
            }
            footer { class: "dialog__actions",
                button { class: "button text", onclick: move |_| dialogs.close(), "Cancel" }
                button {
                    class: "button",
                    disabled: !ready,
                    onclick: move |_| {
                        if let Some(operation) = confirm.clone() {
                            finish(handle, dialogs, operation, None, refused);
                        }
                    },
                    "Add reference"
                }
            }
        }
        if *picking.read() {
            NodePicker {
                title: "Choose the other node".to_owned(),
                hint: "Any loaded node. The engine decides which reference types are legal between the two."
                    .to_owned(),
                onpick: move |node_id: NodeId| {
                    other.set(Some(node_id));
                    reference_type.set(None);
                    picking.set(false);
                },
                oncancel: move |_| picking.set(false),
            }
        }
    }
}

/// Pointing an existing reference at another node, keeping its type.
#[component]
pub fn RetargetDialog(
    reference: ReferenceKey,
    held_by: NodeId,
) -> Element {
    let handle: EditorHandle = use_context();
    let dialogs: Dialogs = use_context();
    let mut target = use_signal(|| None::<NodeId>);
    let mut picking = use_signal(|| true);
    let refused = use_signal(|| None::<Refusal>);

    let described = handle
        .with_space(|space| {
            (
                label_of(space, &reference.source),
                label_of(space, &reference.reference_type),
                label_of(space, &reference.target),
                label_of(space, &held_by),
            )
        })
        .unwrap_or_default();
    let named = target
        .read()
        .clone()
        .and_then(|node_id| handle.with_space(|space| candidate(space, &node_id)));
    let operation = target
        .read()
        .clone()
        .map(|target| Operation::RetargetReference {
            reference: reference.clone(),
            target,
        });
    let confirm = operation.clone();
    let overridden = operation.clone();
    let ready = operation.is_some();
    let refusal_now = refused.read().clone();
    let chosen_label = named
        .as_ref()
        .map_or_else(|| "Choose a node…".to_owned(), |named| named.label.clone());

    rsx! {
        Dialog {
            title: "Retarget reference".to_owned(),
            icon: "swap_horiz",
            subtitle: format!("Stated on {}", described.3),
            onclose: move |_| dialogs.close(),
            div { class: "dialog__body",
                div { class: "field",
                    span { class: "field__label", "Reference" }
                    span { class: "field__static", "{described.0} — {described.1} → {described.2}" }
                }
                div { class: "field",
                    span { class: "field__label", "New target" }
                    div { class: "field__row",
                        button {
                            class: "button tonal tiny",
                            onclick: move |_| picking.set(true),
                            Icon { name: "search", class: "small" }
                            {chosen_label}
                        }
                        if let Some(named) = named.clone() {
                            span { class: "field__detail mono", {named.identifier} }
                        }
                    }
                }
                if let Some(refusal) = refusal_now {
                    RefusalNotice {
                        refusal,
                        onoverride: move |reason: Option<String>| {
                            if let Some(operation) = overridden.clone() {
                                finish(handle, dialogs, operation, Some(reason), refused);
                            }
                        },
                    }
                }
            }
            footer { class: "dialog__actions",
                button { class: "button text", onclick: move |_| dialogs.close(), "Cancel" }
                button {
                    class: "button",
                    disabled: !ready,
                    onclick: move |_| {
                        if let Some(operation) = confirm.clone() {
                            finish(handle, dialogs, operation, None, refused);
                        }
                    },
                    "Retarget"
                }
            }
        }
        if *picking.read() {
            NodePicker {
                title: "Choose the new target".to_owned(),
                hint: "The reference keeps its type, so the engine judges the new pair.".to_owned(),
                onpick: move |node_id: NodeId| {
                    target.set(Some(node_id));
                    picking.set(false);
                },
                oncancel: move |_| picking.set(false),
            }
        }
    }
}

/// Applies a dialog's operation, closing on success and showing the refusal on the spot.
pub fn finish(
    handle: EditorHandle,
    dialogs: Dialogs,
    operation: Operation,
    override_reason: Option<Option<String>>,
    mut refused: Signal<Option<Refusal>>,
) {
    let outcome = match override_reason {
        Some(reason) => handle.perform_with_override(operation, reason),
        None => handle.perform(operation),
    };
    match outcome {
        Outcome::Refused(refusal) => refused.set(Some(refusal)),
        Outcome::Applied(done) => {
            handle.say(done.status());
            match done.overridden.is_empty() {
                true => dialogs.close(),
                false => dialogs.report(done.overridden),
            }
        }
        Outcome::Unchanged | Outcome::Closed => dialogs.close(),
    }
}

/// Why the legal-type list came back empty: the distinct rules that reject every candidate.
fn why_nothing_is_legal(
    space: &AddressSpace,
    source: &NodeId,
    target: &NodeId,
) -> Vec<(DiagnosticCode, String)> {
    let mut reasons: Vec<(DiagnosticCode, String)> = Vec::new();
    for reference_type in query::concrete_subtypes(space, &ids::REFERENCES) {
        for finding in query::may_add_reference(space, source, &reference_type, target).errors() {
            if !reasons.iter().any(|(code, _)| *code == finding.code) {
                reasons.push((finding.code, finding.message.clone()));
            }
        }
    }
    reasons
}

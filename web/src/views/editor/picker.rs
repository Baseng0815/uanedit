//! The node picker: one search over the whole address space, used wherever an operation names
//! another node — a reference target, a deletion's retarget, a new node's parent or supertype.
//!
//! The matcher is the tree's, so a node found in the tree search is found here under the same text.

use std::rc::Rc;

use dioxus::prelude::*;
use uanedit::nodes::NodeClass;
use uanedit::space::AddressSpace;
use uanedit::types::node_id::NodeId;

use crate::components::Icon;
use crate::session::{
    EditorHandle,
    sleep,
};
use crate::views::editor::icons::class_icon;

const LIMIT: usize = 150;
const DEBOUNCE_MILLIS: u32 = 160;
const MIN_NEEDLE: usize = 2;

type Allows = dyn Fn(&AddressSpace, &NodeId) -> bool;

/// Which nodes a picker offers. The engine still judges the pick; this only scopes the list.
#[derive(Clone)]
pub struct PickFilter(Rc<Allows>);

impl PickFilter {
    pub fn new(allows: impl Fn(&AddressSpace, &NodeId) -> bool + 'static) -> Self {
        Self(Rc::new(allows))
    }

    fn allows(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
    ) -> bool {
        (self.0)(space, node_id)
    }
}

/// Two filters are the same filter when they are the same closure; a fresh one re-renders the list.
impl PartialEq for PickFilter {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, PartialEq)]
pub struct Candidate {
    pub node: NodeId,
    pub label: String,
    pub browse_name: String,
    pub identifier: String,
    pub node_class: NodeClass,
    pub read_only: bool,
}

/// Whether the node answers to this needle, which is already lowercase and trimmed.
pub fn matches(
    space: &AddressSpace,
    node_id: &NodeId,
    needle: &str,
) -> bool {
    let Some(node) = space.node(node_id) else {
        return false;
    };
    let header = node.header();
    contains(&header.browse_name.name, needle)
        || header
            .display_name
            .iter()
            .any(|text| contains(&text.text, needle))
        || header
            .symbolic_name
            .as_deref()
            .is_some_and(|name| contains(name, needle))
        || contains(&node_id.to_string(), needle)
}

fn contains(
    haystack: &str,
    needle: &str,
) -> bool {
    haystack.to_lowercase().contains(needle)
}

/// What to call a node in one line: its DisplayName, or its NodeId when nothing defines it.
pub fn label_of(
    space: &AddressSpace,
    node_id: &NodeId,
) -> String {
    match space.node(node_id) {
        Some(node) => node.header().label(None).to_owned(),
        None => node_id.to_string(),
    }
}

pub fn candidate(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Candidate {
    Candidate {
        label: label_of(space, node_id),
        browse_name: space
            .browse_name(node_id)
            .map(|name| name.to_string())
            .unwrap_or_default(),
        identifier: node_id.to_string(),
        node_class: space.node_class(node_id).unwrap_or_default(),
        read_only: space.is_read_only(node_id),
        node: node_id.clone(),
    }
}

/// A modal search over every loaded node; `onpick` closes it by clearing whatever opened it.
#[component]
pub fn NodePicker(
    title: String,
    #[props(default)] hint: String,
    filter: Option<PickFilter>,
    onpick: EventHandler<NodeId>,
    oncancel: EventHandler<()>,
) -> Element {
    let handle: EditorHandle = use_context();
    let mut query = use_signal(String::new);
    let mut needle = use_signal(String::new);

    use_effect(move || {
        let typed = query.read().clone();
        spawn(async move {
            sleep(DEBOUNCE_MILLIS).await;
            if *query.peek() == typed {
                needle.set(typed);
            }
        });
    });

    let hits = use_memo(use_reactive!(|filter| {
        let _ = *handle.structure.read();
        let needle = needle.read().trim().to_lowercase();
        if needle.len() < MIN_NEEDLE {
            return Vec::new();
        }
        handle
            .with_space(|space| search(space, &needle, filter.as_ref()))
            .unwrap_or_default()
    }));

    let searching = needle.read().trim().len() >= MIN_NEEDLE;
    let found = hits.read().clone();

    rsx! {
        div { class: "scrim", onclick: move |_| oncancel.call(()),
            div {
                class: "dialog picker",
                onclick: move |event| event.stop_propagation(),
                header { class: "dialog__head",
                    Icon { name: "search", class: "small" }
                    span { class: "dialog__title type-title-small", {title} }
                    div { class: "dialog__head-spacer" }
                    button {
                        class: "icon-button tiny",
                        title: "Cancel",
                        onclick: move |_| oncancel.call(()),
                        Icon { name: "close", class: "small" }
                    }
                }
                div { class: "picker__search",
                    Icon { name: "search", class: "small" }
                    input {
                        class: "tree__search-input",
                        autofocus: true,
                        value: "{query}",
                        placeholder: "BrowseName, DisplayName, NodeId, SymbolicName",
                        oninput: move |event| query.set(event.value()),
                    }
                }
                if !hint.is_empty() {
                    span { class: "picker__hint type-label-small", {hint} }
                }
                div { class: "picker__results",
                    if !searching {
                        div { class: "picker__empty type-body-small", "Type at least two characters." }
                    } else if found.is_empty() {
                        div { class: "picker__empty type-body-small", "No node matches, or every match is out of this picker's scope." }
                    } else {
                        for hit in found.iter() {
                            {result_row(hit, onpick)}
                        }
                        if found.len() == LIMIT {
                            div { class: "picker__empty type-label-small", "First {LIMIT} matches — narrow the search." }
                        }
                    }
                }
            }
        }
    }
}

fn result_row(
    hit: &Candidate,
    onpick: EventHandler<NodeId>,
) -> Element {
    let target = hit.node.clone();
    let class = match hit.read_only {
        true => "picker__row read-only",
        false => "picker__row",
    };

    rsx! {
        div {
            key: "{hit.identifier}",
            class: class,
            onclick: move |_| onpick.call(target.clone()),
            Icon { name: class_icon(hit.node_class), class: "tree__class small" }
            span { class: "picker__label", "{hit.label}" }
            span { class: "picker__browse mono", "{hit.browse_name}" }
            span { class: "picker__id mono", "{hit.identifier}" }
        }
    }
}

fn search(
    space: &AddressSpace,
    needle: &str,
    filter: Option<&PickFilter>,
) -> Vec<Candidate> {
    let mut found = Vec::new();
    for node_id in space.node_ids() {
        if !matches(space, node_id, needle) {
            continue;
        }
        if filter.is_some_and(|filter| !filter.allows(space, node_id)) {
            continue;
        }
        found.push(candidate(space, node_id));
        if found.len() >= LIMIT {
            break;
        }
    }
    found
}

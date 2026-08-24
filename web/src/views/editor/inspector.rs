//! The node inspector: the middle pane.

use dioxus::prelude::*;
use uanedit::nodes::Node;
use uanedit::space::AddressSpace;
use uanedit::types::node_id::NodeId;

use crate::components::Icon;
use crate::session::EditorHandle;
use crate::views::editor::forms::{
    Editing,
    attribute_form,
};
use crate::views::editor::icons::class_icon;
use crate::views::editor::short_namespace;

#[component]
pub fn InspectorPane() -> Element {
    let handle: EditorHandle = use_context();
    let nonce = use_signal(|| 0_u64);
    let mut failure = use_signal(|| None::<(String, String)>);
    let mut note = use_signal(|| None::<(String, String)>);

    // Reading the revision here is what re-reads the model after an edit; the tree does not.
    let _ = *handle.revision.read();
    let selected = handle.selection.read().clone();

    use_effect(use_reactive!(|selected| {
        let _ = selected;
        failure.set(None);
        note.set(None);
    }));

    let body = selected.and_then(|node_id| {
        handle.with_space(|space| {
            let Some(node) = space.node(&node_id) else {
                return missing(&node_id);
            };
            let editing = Editing {
                handle,
                node: node_id.clone(),
                read_only: space.is_read_only(&node_id),
                nonce,
                failure,
                note,
            };
            rsx! {
                {node_header(space, &node_id, node)}
                {attribute_form(&editing, space, node)}
            }
        })
    });

    rsx! {
        section { class: "pane",
            header { class: "pane__header",
                Icon { name: "tune", class: "small" }
                span { class: "pane__title", "Inspector" }
            }
            div { class: "pane__body inspector",
                match body {
                    Some(body) => body,
                    None => rsx! {
                        div { class: "pane__placeholder",
                            Icon { name: "tune" }
                            span { class: "type-body-small", "Select a node in the tree to edit its attributes." }
                        }
                    },
                }
            }
        }
    }
}

fn node_header(
    space: &AddressSpace,
    node_id: &NodeId,
    node: &Node,
) -> Element {
    let node_class = node.node_class();
    let namespace = space
        .namespace_uri(node_id.namespace_index)
        .unwrap_or_default();
    let read_only = space.is_read_only(node_id);
    let reason = match node_id.is_in_base_namespace() {
        true => "Namespace 0 is fixed by the specification",
        false => "This node belongs to a dependency, not to the file under edit",
    };

    rsx! {
        header { class: "inspector__head",
            span { class: "inspector__icon", Icon { name: class_icon(node_class) } }
            div { class: "inspector__titles",
                span { class: "inspector__name type-title", "{node.header().label(None)}" }
                div { class: "inspector__meta",
                    span { class: "chip", "{node_class}" }
                    span { class: "chip mono", "{node_id}" }
                    span { class: "chip mono", title: namespace.to_owned(), {short_namespace(namespace)} }
                    if read_only {
                        span { class: "chip locked", title: reason,
                            Icon { name: "lock", class: "small" }
                            "read-only"
                        }
                    }
                }
            }
        }
    }
}

fn missing(node_id: &NodeId) -> Element {
    rsx! {
        div { class: "notice",
            Icon { name: "error" }
            span { "No loaded nodeset defines {node_id} any more." }
        }
    }
}

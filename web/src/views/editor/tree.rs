//! The address-space tree: the left pane.
//!
//! Only the rows in the viewport are rendered. The list is a flat `Vec<Row>` over a fixed row
//! height, so the scroll position alone says which slice to draw and two spacer divs stand in for
//! the rest — a nodeset with namespace 0 loaded is tens of thousands of nodes, and a fully expanded
//! subtree repeats every node that has more than one hierarchical parent.

use std::collections::HashSet;
use std::rc::Rc;

use dioxus::html::geometry::PixelsVector2D;
use dioxus::prelude::*;
use uanedit::nodes::NodeClass;
use uanedit::space::AddressSpace;
use uanedit::types::node_id::{
    Identifier,
    NamespaceIndex,
    NodeId,
};

use crate::components::Icon;
use crate::session::{
    EditorHandle,
    sleep,
};
use crate::views::editor::icons::class_icon;
use crate::views::editor::picker::matches;
use crate::views::editor::shell::{
    Dialogs,
    Navigate,
    Request,
};
use crate::views::editor::short_namespace;

/// Kept in step with `--row-height-dense`; the windowing arithmetic needs it as a number.
const ROW_HEIGHT: f64 = 26.0;
const OVERSCAN: usize = 10;
const SEARCH_LIMIT: usize = 200;
const SEARCH_MILLIS: u32 = 180;
const MIN_SEARCH: usize = 2;
const INDENT: f64 = 14.0;

/// The standard Root folder, which is where the hierarchy starts once namespace 0 is loaded.
const ROOT_FOLDER: NodeId = NodeId {
    namespace_index: 0,
    identifier: Identifier::Numeric(84),
};

#[derive(Clone, PartialEq)]
enum Row {
    Section(String),
    Node(NodeRow),
}

#[derive(Clone, PartialEq)]
struct NodeRow {
    node: NodeId,
    /// The path from the root, which is what expansion is keyed by: one node under two parents is
    /// two rows that open independently.
    key: String,
    depth: usize,
    label: String,
    identifier: String,
    node_class: NodeClass,
    read_only: bool,
    expandable: bool,
    expanded: bool,
}

/// Where the tree starts: the parentless nodes that lead somewhere, and the ones that do not.
#[derive(Clone, Default, PartialEq)]
struct Roots {
    rooted: Vec<NodeId>,
    unrooted: Vec<NodeId>,
}

#[derive(Clone, PartialEq)]
struct Hit {
    node: NodeId,
    label: String,
    identifier: String,
    browse_name: String,
    node_class: NodeClass,
    read_only: bool,
}

#[component]
pub fn TreePane() -> Element {
    let handle: EditorHandle = use_context();
    let navigate: Navigate = use_context();
    let dialogs: Dialogs = use_context();
    let mut expanded = use_signal(HashSet::<String>::new);
    let mut hidden = use_signal(HashSet::<NamespaceIndex>::new);
    let mut query = use_signal(String::new);
    let mut needle = use_signal(String::new);
    let mut scroll_top = use_signal(|| 0.0_f64);
    let mut viewport = use_signal(|| 600.0_f64);
    let mut container = use_signal(|| None::<Rc<MountedData>>);
    let mut pending = use_signal(|| None::<String>);
    let mut selection = handle.selection;

    let namespaces = use_memo(move || {
        let _ = *handle.structure.read();
        handle
            .with_space(|space| {
                (0..space.namespaces().len())
                    .filter_map(|index| NamespaceIndex::try_from(index).ok())
                    .map(|index| (index, space.namespace_uri(index).unwrap_or_default().to_owned()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });

    let roots = use_memo(move || {
        let _ = *handle.structure.read();
        let hidden = hidden.read().clone();
        handle
            .with_space(|space| find_roots(space, &hidden))
            .unwrap_or_default()
    });

    let rows = use_memo(move || {
        let _ = *handle.structure.read();
        let (hidden, expanded, roots) = (hidden.read().clone(), expanded.read().clone(), roots.read().clone());
        handle
            .with_space(|space| flatten(space, &hidden, &expanded, &roots))
            .unwrap_or_default()
    });

    let hits = use_memo(move || {
        let _ = *handle.structure.read();
        let needle = needle.read().trim().to_lowercase();
        if needle.len() < MIN_SEARCH {
            return Vec::new();
        }
        let hidden = hidden.read().clone();
        handle
            .with_space(|space| search(space, &needle, &hidden))
            .unwrap_or_default()
    });

    // The search box runs over every node in the space, so it waits for the typing to stop.
    use_effect(move || {
        let typed = query.read().clone();
        spawn(async move {
            sleep(SEARCH_MILLIS).await;
            if *query.peek() == typed {
                needle.set(typed);
            }
        });
    });

    // Revealing waits for the rows the expansion produced before it can know where to scroll.
    use_effect(move || {
        let rows = rows.read();
        let Some(key) = pending.read().clone() else {
            return;
        };
        let Some(index) = rows
            .iter()
            .position(|row| matches!(row, Row::Node(row) if row.key == key))
        else {
            return;
        };
        let offset = (pixels(index) - *viewport.peek() / 3.0).max(0.0);
        if let Some(element) = container.peek().clone() {
            spawn(async move {
                let _ = element
                    .scroll(PixelsVector2D::new(0.0, offset), ScrollBehavior::Instant)
                    .await;
            });
        }
        scroll_top.set(offset);
        pending.set(None);
    });

    let reveal = move |target: NodeId| {
        let filter = hidden.peek().clone();
        let chain = handle
            .with_space(|space| ancestry(space, &filter, &target))
            .unwrap_or_default();
        let Some(first) = chain.first() else {
            return;
        };
        let prefix = match roots.peek().rooted.contains(first) {
            true => "r",
            false => "u",
        };
        let mut key = format!("{prefix}/{first}");
        let mut keys = vec![key.clone()];
        for node_id in chain.iter().skip(1) {
            key = format!("{key}/{node_id}");
            keys.push(key.clone());
        }
        expanded.with_mut(|open| {
            for key in keys.iter().take(keys.len().saturating_sub(1)) {
                open.insert(key.clone());
            }
        });
        selection.set(Some(target));
        pending.set(Some(key));
        query.set(String::new());
        needle.set(String::new());
    };

    // Any pane may ask for a node; the tree is the only thing that knows how to get to it.
    use_effect(move || {
        let mut target = navigate.target;
        let Some(node_id) = target.read().clone() else {
            return;
        };
        let mut reveal = reveal;
        reveal(node_id);
        target.set(None);
    });

    let searching = needle.read().trim().len() >= MIN_SEARCH;
    let selected = selection.read().clone();
    let create_anchor = selected.clone();
    let subtype_target = selected.clone();
    let delete_target = selected.clone();
    let deletable = selected
        .as_ref()
        .is_some_and(|node_id| handle.with_space(|space| !space.is_read_only(node_id)) == Some(true));
    // A subtype may be created under any type node, a dependency's included: the new type is ours.
    let subtypeable = selected.as_ref().is_some_and(|node_id| {
        handle.with_space(|space| space.node_class(node_id).is_some_and(NodeClass::is_type)) == Some(true)
    });
    let rows = rows.read();
    let total = rows.len();
    let first = rows_in(*scroll_top.read()).saturating_sub(OVERSCAN);
    let last = (first + rows_in(*viewport.read()) + 2 * OVERSCAN + 2).min(total);
    let window = rows.get(first..last).unwrap_or_default();
    let hidden_now = hidden.read().clone();

    rsx! {
        section { class: "pane",
            header { class: "pane__header",
                Icon { name: "account_tree", class: "small" }
                span { class: "pane__title", "Address space" }
                div { class: "pane__header-spacer" }
                button {
                    class: "icon-button tiny",
                    title: "Create a node under the selected one",
                    onclick: move |_| {
                        dialogs
                            .open(Request::Create {
                                anchor: create_anchor.clone(),
                            })
                    },
                    Icon { name: "add", class: "small" }
                }
                button {
                    class: "icon-button tiny",
                    disabled: !subtypeable,
                    title: match subtypeable {
                        true => "Create a subtype of the selected type, overriding what it inherits",
                        false => "Select a type node",
                    },
                    onclick: move |_| {
                        dialogs
                            .open(Request::Subtype {
                                supertype: subtype_target.clone(),
                            })
                    },
                    Icon { name: "lan", class: "small" }
                }
                button {
                    class: "icon-button tiny",
                    disabled: !deletable,
                    title: match deletable {
                        true => "Delete the selected node, answering for what it leaves behind",
                        false => "Select a node this editor may change",
                    },
                    onclick: move |_| {
                        if let Some(node) = delete_target.clone() {
                            dialogs.open(Request::Delete { node });
                        }
                    },
                    Icon { name: "delete", class: "small" }
                }
                span { class: "chip mono", "{total}" }
            }
            div { class: "tree__controls",
                div { class: "tree__search",
                    Icon { name: "search", class: "small" }
                    input {
                        class: "tree__search-input",
                        value: "{query}",
                        placeholder: "BrowseName, DisplayName, NodeId, SymbolicName",
                        oninput: move |event| query.set(event.value()),
                    }
                    if !query.read().is_empty() {
                        button {
                            class: "icon-button tiny",
                            title: "Clear",
                            onclick: move |_| {
                                query.set(String::new());
                                needle.set(String::new());
                            },
                            Icon { name: "close", class: "small" }
                        }
                    }
                }
                div { class: "tree__namespaces",
                    for (index , uri) in namespaces.read().iter() {
                        button {
                            key: "{index}",
                            class: if hidden_now.contains(index) { "tree__namespace" } else { "tree__namespace on" },
                            title: uri.clone(),
                            onclick: {
                                let index = *index;
                                move |_| {
                                    hidden
                                        .with_mut(|filter| {
                                            if !filter.remove(&index) {
                                                filter.insert(index);
                                            }
                                        });
                                }
                            },
                            span { class: "mono", "{index}" }
                            "{short_namespace(uri)}"
                        }
                    }
                }
            }
            if searching {
                div { class: "tree__results",
                    div { class: "tree__results-head type-label-small",
                        {results_summary(hits.read().len())}
                    }
                    for hit in hits.read().iter() {
                        {hit_row(hit, selected.as_ref(), reveal)}
                    }
                }
            } else {
                div {
                    class: "tree__scroll",
                    onmounted: move |event| {
                        let element = event.data();
                        container.set(Some(element.clone()));
                        spawn(async move {
                            if let Ok(rect) = element.get_client_rect().await {
                                viewport.set(rect.size.height);
                            }
                        });
                    },
                    onscroll: move |event| {
                        scroll_top.set(event.data().scroll_top());
                        viewport.set(f64::from(event.data().client_height()));
                    },
                    div { class: "tree__spacer", style: "height: {pixels(first)}px" }
                    for row in window.iter() {
                        {row_element(row, selected.as_ref(), expanded, selection)}
                    }
                    div { class: "tree__spacer", style: "height: {pixels(total - last)}px" }
                    if total == 0 {
                        div { class: "pane__placeholder",
                            Icon { name: "account_tree" }
                            span { class: "type-body-small", "Nothing to show — every namespace is filtered out, or the file is still opening." }
                        }
                    }
                }
            }
        }
    }
}

fn row_element(
    row: &Row,
    selected: Option<&NodeId>,
    mut expanded: Signal<HashSet<String>>,
    mut selection: Signal<Option<NodeId>>,
) -> Element {
    let row = match row {
        Row::Section(label) => {
            return rsx! {
                div { key: "section:{label}", class: "tree__section type-label-small", {label.clone()} }
            };
        }
        Row::Node(row) => row,
    };

    let key = row.key.clone();
    let target = row.node.clone();
    let indent = f64::from(u32::try_from(row.depth).unwrap_or(u32::MAX)) * INDENT;
    let twisty = match row.expanded {
        true => "expand_more",
        false => "chevron_right",
    };
    let mut class = String::from("tree__row");
    if selected == Some(&row.node) {
        class.push_str(" selected");
    }
    if row.read_only {
        class.push_str(" read-only");
    }

    rsx! {
        div {
            key: "{row.key}",
            class: class,
            style: "padding-left: {indent}px",
            onclick: move |_| selection.set(Some(target.clone())),
            span {
                class: "tree__twisty",
                onclick: move |event| {
                    event.stop_propagation();
                    expanded
                        .with_mut(|open| {
                            if !open.remove(&key) {
                                open.insert(key.clone());
                            }
                        });
                },
                if row.expandable {
                    Icon { name: twisty, class: "small" }
                }
            }
            Icon { name: class_icon(row.node_class), class: "tree__class small" }
            span { class: "tree__label", "{row.label}" }
            span { class: "tree__id mono", "{row.identifier}" }
        }
    }
}

fn hit_row(
    hit: &Hit,
    selected: Option<&NodeId>,
    mut reveal: impl FnMut(NodeId) + 'static,
) -> Element {
    let target = hit.node.clone();
    let mut class = String::from("tree__hit");
    if selected == Some(&hit.node) {
        class.push_str(" selected");
    }
    if hit.read_only {
        class.push_str(" read-only");
    }

    rsx! {
        div {
            key: "{hit.identifier}",
            class: class,
            onclick: move |_| reveal(target.clone()),
            Icon { name: class_icon(hit.node_class), class: "tree__class small" }
            span { class: "tree__label", "{hit.label}" }
            span { class: "tree__hit-browse mono", "{hit.browse_name}" }
            span { class: "tree__id mono", "{hit.identifier}" }
        }
    }
}

fn results_summary(count: usize) -> String {
    match count {
        0 => "No node matches".to_owned(),
        1 => "1 match".to_owned(),
        SEARCH_LIMIT => format!("first {SEARCH_LIMIT} matches"),
        count => format!("{count} matches"),
    }
}

/// The nodes no hierarchical reference reaches, split by whether anything hangs off them.
///
/// Both halves are shown: a parentless node with children is a tree of its own, and one without is
/// legal too and belongs in a labelled bucket rather than nowhere (features.md §2A).
fn find_roots(
    space: &AddressSpace,
    hidden: &HashSet<NamespaceIndex>,
) -> Roots {
    let mut found = Roots::default();
    for node_id in space.node_ids() {
        if hidden.contains(&node_id.namespace_index) {
            continue;
        }
        if space
            .parents(node_id)
            .iter()
            .any(|parent| !hidden.contains(&parent.namespace_index))
        {
            continue;
        }
        let leads_somewhere = space
            .children(node_id)
            .iter()
            .any(|child| !hidden.contains(&child.namespace_index));
        match leads_somewhere {
            true => found.rooted.push(node_id.clone()),
            false => found.unrooted.push(node_id.clone()),
        }
    }
    if let Some(position) = found
        .rooted
        .iter()
        .position(|node_id| *node_id == ROOT_FOLDER)
    {
        let root = found.rooted.remove(position);
        found.rooted.insert(0, root);
    }
    found
}

fn visible_children(
    space: &AddressSpace,
    node_id: &NodeId,
    hidden: &HashSet<NamespaceIndex>,
) -> Vec<NodeId> {
    let mut found: Vec<NodeId> = Vec::new();
    for child in space.children(node_id) {
        if hidden.contains(&child.namespace_index) || found.contains(&child) {
            continue;
        }
        found.push(child);
    }
    found
}

fn flatten(
    space: &AddressSpace,
    hidden: &HashSet<NamespaceIndex>,
    expanded: &HashSet<String>,
    roots: &Roots,
) -> Vec<Row> {
    let mut walk = Walk {
        space,
        hidden,
        expanded,
        rows: Vec::new(),
    };
    let mut ancestors = Vec::new();
    for node_id in &roots.rooted {
        walk.push(node_id, 0, format!("r/{node_id}"), &mut ancestors);
    }
    if !roots.unrooted.is_empty() {
        walk.rows
            .push(Row::Section(format!("Unrooted · {}", roots.unrooted.len())));
        for node_id in &roots.unrooted {
            walk.push(node_id, 0, format!("u/{node_id}"), &mut ancestors);
        }
    }
    walk.rows
}

struct Walk<'a> {
    space: &'a AddressSpace,
    hidden: &'a HashSet<NamespaceIndex>,
    expanded: &'a HashSet<String>,
    rows: Vec<Row>,
}

impl Walk<'_> {
    fn push(
        &mut self,
        node_id: &NodeId,
        depth: usize,
        key: String,
        ancestors: &mut Vec<NodeId>,
    ) {
        let children = visible_children(self.space, node_id, self.hidden);
        let expandable = !children.is_empty();
        let expanded = expandable && self.expanded.contains(&key);
        let row = self.row(node_id, depth, key.clone(), expandable, expanded);
        self.rows.push(Row::Node(row));
        if !expanded {
            return;
        }
        ancestors.push(node_id.clone());
        for child in children {
            // A hierarchy may close a loop; the row that would repeat an ancestor stops here.
            if ancestors.contains(&child) {
                continue;
            }
            self.push(&child, depth + 1, format!("{key}/{child}"), ancestors);
        }
        ancestors.pop();
    }

    fn row(
        &self,
        node_id: &NodeId,
        depth: usize,
        key: String,
        expandable: bool,
        expanded: bool,
    ) -> NodeRow {
        NodeRow {
            label: label_of(self.space, node_id),
            identifier: node_id.to_string(),
            node_class: self.space.node_class(node_id).unwrap_or_default(),
            read_only: self.space.is_read_only(node_id),
            node: node_id.clone(),
            key,
            depth,
            expandable,
            expanded,
        }
    }
}

fn search(
    space: &AddressSpace,
    needle: &str,
    hidden: &HashSet<NamespaceIndex>,
) -> Vec<Hit> {
    let mut found = Vec::new();
    for node_id in space.node_ids() {
        if hidden.contains(&node_id.namespace_index) {
            continue;
        }
        if !matches(space, node_id, needle) {
            continue;
        }
        found.push(Hit {
            label: label_of(space, node_id),
            identifier: node_id.to_string(),
            browse_name: space
                .browse_name(node_id)
                .map(|name| name.to_string())
                .unwrap_or_default(),
            node_class: space.node_class(node_id).unwrap_or_default(),
            read_only: space.is_read_only(node_id),
            node: node_id.clone(),
        });
        if found.len() >= SEARCH_LIMIT {
            break;
        }
    }
    found
}

/// The chain from a root down to the node, which is what has to be open for it to be visible.
fn ancestry(
    space: &AddressSpace,
    hidden: &HashSet<NamespaceIndex>,
    target: &NodeId,
) -> Vec<NodeId> {
    let mut chain = vec![target.clone()];
    let mut seen: HashSet<NodeId> = HashSet::new();
    seen.insert(target.clone());
    while let Some(current) = chain.last().cloned() {
        let parent = space
            .parents(&current)
            .into_iter()
            .find(|parent| !hidden.contains(&parent.namespace_index) && !seen.contains(parent));
        let Some(parent) = parent else {
            break;
        };
        seen.insert(parent.clone());
        chain.push(parent);
    }
    chain.reverse();
    chain
}

fn label_of(
    space: &AddressSpace,
    node_id: &NodeId,
) -> String {
    match space.node(node_id) {
        Some(node) => node.header().label(None).to_owned(),
        None => node_id.to_string(),
    }
}

fn pixels(rows: usize) -> f64 {
    f64::from(u32::try_from(rows).unwrap_or(u32::MAX)) * ROW_HEIGHT
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a scroll offset clamped to a positive, bounded row count"
)]
fn rows_in(pixels: f64) -> usize {
    let rows = (pixels / ROW_HEIGHT).ceil();
    if rows <= 0.0 {
        return 0;
    }
    rows.min(f64::from(u32::MAX)) as usize
}

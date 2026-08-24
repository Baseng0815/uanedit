//! The validation panel: the rules engine in report mode over the open document.
//!
//! The DI companion spec alone carries some nine hundred inherited errors, so findings are grouped
//! by rule and only an opened group materialises its rows. Nothing is filtered away — an
//! acknowledged finding is muted and still counted, which is what guardrails.md §4 asks for.

use std::collections::{
    HashMap,
    HashSet,
};

use dioxus::prelude::*;
use uanedit::edit::{
    CreateInstance,
    InstanceAttributes,
    ReferenceKey,
    Refusal,
};
use uanedit::nodes::Node;
use uanedit::rules::code::{
    DiagnosticCode,
    Severity,
};
use uanedit::rules::engine::FindingCounts;
use uanedit::rules::finding::{
    Finding,
    Origin,
};
use uanedit::rules::fix::Fix;
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
use crate::views::editor::diagnostics::{
    Explain,
    origin_badge,
    severity_class,
    severity_icon,
};
use crate::views::editor::picker::label_of;
use crate::views::editor::shell::Navigate;

/// How many rows an opened group shows before the user asks for more.
const PAGE: usize = 25;

#[derive(Clone, PartialEq)]
struct Group {
    code: DiagnosticCode,
    id: String,
    severity: Severity,
    total: usize,
    introduced: usize,
    acknowledged: usize,
    fixable: usize,
}

/// The rows an opened group renders, and how many the filters leave it — which is what the paging
/// footer counts against, since a hidden row is not one more page away.
#[derive(Clone, PartialEq)]
struct Shown {
    visible: usize,
    rows: Vec<Row>,
}

#[derive(Clone, PartialEq)]
struct Row {
    finding: Finding,
    anchor_label: String,
    anchor_detail: String,
    fix: Option<Operation>,
    fix_text: String,
}

/// What a row needs that is not in the row itself.
#[derive(Clone, Copy)]
struct Panel {
    handle: EditorHandle,
    navigate: Navigate,
    refused: Signal<Option<Refusal>>,
}

#[component]
pub fn ValidationPanel() -> Element {
    let handle: EditorHandle = use_context();
    let navigate: Navigate = use_context();
    let opened = use_signal(HashSet::<String>::new);
    let limits = use_signal(HashMap::<String, usize>::new);
    let mut hide_acknowledged = use_signal(|| false);
    let mut introduced_only = use_signal(|| false);
    let refused = use_signal(|| None::<Refusal>);

    let summary = use_memo(move || {
        let _ = *handle.revision.read();
        handle
            .with_session(|session| {
                let diagnostics = session.diagnostics();
                (diagnostics.counts, groups(&diagnostics.findings))
            })
            .unwrap_or_default()
    });

    let rows = use_memo(move || {
        let _ = *handle.revision.read();
        let opened = opened.read().clone();
        let limits = limits.read().clone();
        let hidden = *hide_acknowledged.read();
        let only_introduced = *introduced_only.read();
        handle
            .with_session(|session| {
                let space = session.space();
                let mut by_code: HashMap<String, Shown> = HashMap::new();
                for code in &opened {
                    let limit = limits.get(code).copied().unwrap_or(PAGE);
                    let mut found: Vec<&Finding> = session
                        .diagnostics()
                        .findings
                        .iter()
                        .filter(|finding| finding.code.id() == code)
                        .filter(|finding| !(hidden && finding.acknowledged))
                        .filter(|finding| !(only_introduced && finding.origin != Origin::Introduced))
                        .collect();
                    // What this editor did comes first; nine hundred inherited rows must not bury it.
                    found.sort_by_key(|finding| finding.origin != Origin::Introduced);
                    let visible = found.len();
                    let rows = found
                        .into_iter()
                        .take(limit)
                        .map(|finding| row(space, finding))
                        .collect();
                    by_code.insert(code.clone(), Shown { visible, rows });
                }
                by_code
            })
            .unwrap_or_default()
    });

    let panel = Panel {
        handle,
        navigate,
        refused,
    };
    let (counts, groups) = summary.read().clone();
    let materialised = rows.read().clone();
    let opened_now = opened.read().clone();
    let refusal_now = refused.read().clone();
    let only_introduced = *introduced_only.read();
    let inherited = counts.errors + counts.warnings - counts.introduced;
    let shown: Vec<&Group> = groups
        .iter()
        .filter(|group| !(only_introduced && group.introduced == 0))
        .collect();

    rsx! {
        div { class: "validation",
            header { class: "validation__counts",
                span { class: severity_class(Severity::Error), title: "Specification violations",
                    Icon { name: "error", class: "small" }
                    "{counts.errors}"
                }
                span { class: severity_class(Severity::Warning), title: "Deviations the specification tolerates",
                    Icon { name: "warning", class: "small" }
                    "{counts.warnings}"
                }
                div { class: "refs__spacer" }
                span { class: "badge", title: "Present in the file as it was opened", "{inherited} inherited" }
                span {
                    class: "badge badge--introduced",
                    title: "Created by an edit made here",
                    "{counts.introduced} introduced"
                }
                span { class: "badge", title: "Reviewed and muted, still counted", "{counts.acknowledged} acknowledged" }
            }
            div { class: "validation__controls",
                label { class: "field__toggle",
                    input {
                        r#type: "checkbox",
                        checked: *hide_acknowledged.read(),
                        onchange: move |event| hide_acknowledged.set(event.checked()),
                    }
                    span { class: "field__toggle-label type-label-small", "Hide acknowledged rows" }
                }
                label { class: "field__toggle",
                    input {
                        r#type: "checkbox",
                        checked: only_introduced,
                        onchange: move |event| introduced_only.set(event.checked()),
                    }
                    span { class: "field__toggle-label type-label-small", "Introduced here only" }
                }
            }
            if let Some(refusal) = refusal_now {
                RefusedFix { refusal }
            }
            if shown.is_empty() {
                div { class: "pane__placeholder",
                    Icon { name: "verified" }
                    span { class: "type-body-small",
                        if only_introduced {
                            "No edit made here has left a finding behind."
                        } else {
                            "No rule has anything to say about this nodeset."
                        }
                    }
                }
            }
            for group in shown.iter() {
                {group_block(panel, group, &opened_now, materialised.get(&group.id), opened, limits, only_introduced)}
            }
        }
    }
}

fn group_block(
    panel: Panel,
    group: &Group,
    opened_now: &HashSet<String>,
    rows: Option<&Shown>,
    mut opened: Signal<HashSet<String>>,
    mut limits: Signal<HashMap<String, usize>>,
    only_introduced: bool,
) -> Element {
    let is_open = opened_now.contains(&group.id);
    let key = group.id.clone();
    let more = group.id.clone();
    let shown = rows.map_or(0, |shown| shown.rows.len());
    let visible = rows.map_or(0, |shown| shown.visible);
    let total = match only_introduced {
        true => group.introduced,
        false => group.total,
    };
    let twisty = match is_open {
        true => "expand_more",
        false => "chevron_right",
    };

    rsx! {
        section { key: "{group.id}", class: "finding-group",
            div {
                class: "finding-group__head",
                onclick: move |_| {
                    opened
                        .with_mut(|open| {
                            if !open.remove(&key) {
                                open.insert(key.clone());
                            }
                        })
                },
                Icon { name: twisty, class: "small" }
                span { class: severity_class(group.severity),
                    Icon { name: severity_icon(group.severity), class: "small" }
                    "{group.severity}"
                }
                span { class: "mono finding-group__code", "{group.id}" }
                span { class: "finding-group__summary", "{group.code.summary()}" }
                div { class: "refs__spacer" }
                if group.introduced > 0 {
                    span { class: "badge badge--introduced", title: "Introduced here", "{group.introduced}" }
                }
                if group.acknowledged > 0 {
                    span { class: "badge", title: "Acknowledged", "{group.acknowledged}" }
                }
                if group.fixable > 0 {
                    span { class: "badge", title: "Carry a machine-applicable fix",
                        Icon { name: "build", class: "small" }
                        "{group.fixable}"
                    }
                }
                span { class: "chip mono", "{total}" }
            }
            if is_open {
                div { class: "finding-group__body",
                    Explain { code: group.code }
                    for row in rows.into_iter().flat_map(|shown| &shown.rows) {
                        {finding_row(panel, row)}
                    }
                    if shown < visible {
                        button {
                            class: "button text tiny",
                            onclick: move |_| {
                                limits
                                    .with_mut(|limits| {
                                        let entry = limits.entry(more.clone()).or_insert(PAGE);
                                        *entry += 4 * PAGE;
                                    })
                            },
                            "Show more · {shown} of {visible}"
                        }
                    }
                }
            }
        }
    }
}

fn finding_row(
    panel: Panel,
    row: &Row,
) -> Element {
    let handle = panel.handle;
    let navigate = panel.navigate;
    let mut refused = panel.refused;
    let finding = &row.finding;
    let anchor = finding.anchor.node.clone();
    let fingerprint = finding.fingerprint.clone();
    let toggle = finding.fingerprint.clone();
    let acknowledged = finding.acknowledged;
    let fix = row.fix.clone();
    let class = match acknowledged {
        true => "finding acknowledged",
        false => "finding",
    };
    let bell = match acknowledged {
        true => "notifications_active",
        false => "notifications_off",
    };
    let toggle_title = match acknowledged {
        true => "Un-mute this finding",
        false => "Mark as reviewed; it stays counted and returns if the facts change",
    };

    rsx! {
        div { key: "{fingerprint}", class: class,
            div { class: "finding__line",
                span { class: "finding__message", "{finding.message}" }
                {origin_badge(finding)}
            }
            div { class: "finding__meta",
                span {
                    class: "finding__anchor",
                    title: "Select and reveal this node",
                    onclick: move |_| navigate.to(anchor.clone()),
                    Icon { name: "my_location", class: "small" }
                    span { "{row.anchor_label}" }
                    span { class: "mono finding__anchor-id", "{finding.anchor.node}" }
                }
                if !row.anchor_detail.is_empty() {
                    span { class: "mono finding__detail", "{row.anchor_detail}" }
                }
                div { class: "refs__spacer" }
                if let Some(fix) = fix {
                    button {
                        class: "button tonal tiny",
                        title: "{row.fix_text}",
                        onclick: move |_| {
                            match handle.perform(fix.clone()) {
                                Outcome::Refused(refusal) => refused.set(Some(refusal)),
                                Outcome::Applied(done) => {
                                    refused.set(None);
                                    handle.say(done.status());
                                }
                                Outcome::Unchanged | Outcome::Closed => refused.set(None),
                            }
                        },
                        Icon { name: "build", class: "small" }
                        "Apply fix"
                    }
                } else if !row.fix_text.is_empty() {
                    span { class: "finding__suggestion type-label-small", title: "{row.fix_text}",
                        Icon { name: "lightbulb", class: "small" }
                        "Manual fix"
                    }
                }
                button {
                    class: "button text tiny",
                    title: toggle_title,
                    onclick: move |_| {
                        match acknowledged {
                            true => handle.unacknowledge(&toggle),
                            false => handle.acknowledge(toggle.clone(), None),
                        }
                    },
                    Icon { name: bell, class: "small" }
                    if acknowledged {
                        "Unacknowledge"
                    } else {
                        "Acknowledge"
                    }
                }
            }
            if !row.fix_text.is_empty() && row.fix.is_none() {
                span { class: "finding__manual type-body-small", "{row.fix_text}" }
            }
        }
    }
}

#[component]
fn RefusedFix(refusal: Refusal) -> Element {
    rsx! {
        div { class: "notice",
            Icon { name: "error" }
            span { "The fix was refused: {refusal}" }
        }
    }
}

/// One entry per rule that has anything to say, errors first and the loudest rule at the top.
fn groups(findings: &[Finding]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for finding in findings {
        let position = groups
            .iter()
            .position(|group| group.code == finding.code)
            .unwrap_or_else(|| {
                groups.push(Group {
                    code: finding.code,
                    id: finding.code.id().to_owned(),
                    severity: finding.severity,
                    total: 0,
                    introduced: 0,
                    acknowledged: 0,
                    fixable: 0,
                });
                groups.len() - 1
            });
        let group = &mut groups[position];
        group.total += 1;
        if finding.severity == Severity::Error {
            group.severity = Severity::Error;
        }
        if finding.origin == Origin::Introduced {
            group.introduced += 1;
        }
        if finding.acknowledged {
            group.acknowledged += 1;
        }
        if finding.fix.is_some() {
            group.fixable += 1;
        }
    }
    groups.sort_by(|left, right| {
        left.severity
            .cmp(&right.severity)
            .then(right.total.cmp(&left.total))
            .then(left.id.cmp(&right.id))
    });
    groups
}

fn row(
    space: &AddressSpace,
    finding: &Finding,
) -> Row {
    let fix_text = finding
        .fix
        .as_ref()
        .map(|fix| describe_fix(space, fix))
        .unwrap_or_default();
    Row {
        anchor_label: label_of(space, &finding.anchor.node),
        anchor_detail: finding
            .anchor
            .detail
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        fix: finding
            .fix
            .as_ref()
            .and_then(|fix| fix_operation(space, fix)),
        fix_text,
        finding: finding.clone(),
    }
}

/// The fix descriptors that are one operation. The rest are described in words instead.
fn fix_operation(
    space: &AddressSpace,
    fix: &Fix,
) -> Option<Operation> {
    match fix {
        Fix::SetTypeDefinition { node, type_definition } => Some(Operation::SetTypeDefinition {
            node: node.clone(),
            type_definition: type_definition.clone(),
        }),
        Fix::RemoveReference {
            holder,
            reference_type,
            is_forward,
            target,
        } => {
            let (source, target) = match is_forward {
                true => (holder.clone(), target.clone()),
                false => (target.clone(), holder.clone()),
            };
            Some(Operation::RemoveReference(ReferenceKey::new(source, reference_type.clone(), target)))
        }
        Fix::SetBrowseName { node, browse_name } => Some(Operation::SetField {
            node: node.clone(),
            value: uanedit::edit::FieldValue::BrowseName(browse_name.clone()),
        }),
        Fix::SetDataType { node, data_type } => Some(Operation::SetDataType {
            node: node.clone(),
            data_type: data_type.clone(),
        }),
        Fix::SetValueRank { node, value_rank } => Some(Operation::SetField {
            node: node.clone(),
            value: uanedit::edit::FieldValue::ValueRank(*value_rank),
        }),
        Fix::SetArrayDimensions { node, array_dimensions } => Some(Operation::SetField {
            node: node.clone(),
            value: uanedit::edit::FieldValue::ArrayDimensions(array_dimensions.clone()),
        }),
        Fix::SetModellingRule { node, modelling_rule } => Some(Operation::SetModellingRule {
            node: node.clone(),
            modelling_rule: modelling_rule.clone(),
        }),
        Fix::SetInverseName { node, inverse_name } => Some(Operation::SetField {
            node: node.clone(),
            value: uanedit::edit::FieldValue::InverseName(inverse_name.clone()),
        }),
        Fix::SetParentNodeId { node, parent } => Some(Operation::SetParentNodeId {
            node: node.clone(),
            parent: parent.clone(),
        }),
        Fix::MaterializeChild {
            parent,
            declaration,
            browse_name,
            reference_type,
            ..
        } => materialize(space, parent, declaration, browse_name, reference_type),
        Fix::SetReferenceType { .. }
        | Fix::SetNodeId { .. }
        | Fix::DeclareNamespace { .. }
        | Fix::DefineAlias { .. }
        | Fix::LoadDependency { .. } => None,
    }
}

/// The child a Mandatory declaration owes, copied from the declaration the type states.
fn materialize(
    space: &AddressSpace,
    parent: &NodeId,
    declaration: &NodeId,
    browse_name: &uanedit::types::qualified_name::QualifiedName,
    reference_type: &NodeId,
) -> Option<Operation> {
    let node = space.node(declaration)?;
    let attributes = match node {
        Node::Object(_) => InstanceAttributes::Object {
            type_definition: space.type_definition(declaration),
            event_notifier: uanedit::attributes::event_notifier::EventNotifier::default(),
        },
        Node::Variable(variable) => InstanceAttributes::Variable {
            type_definition: space.type_definition(declaration),
            data_type: space.data_type(declaration).unwrap_or(ids::BASE_DATA_TYPE),
            value_rank: variable.value_rank,
            array_dimensions: variable.array_dimensions.clone(),
        },
        Node::Method(method) => InstanceAttributes::Method {
            executable: method.executable,
            user_executable: method.user_executable,
        },
        _ => return None,
    };
    Some(Operation::CreateInstance(CreateInstance {
        parent: parent.clone(),
        reference_type: reference_type.clone(),
        browse_name: browse_name.clone(),
        display_name: node.header().display_name.clone(),
        description: Vec::new(),
        modelling_rule: None,
        attributes,
    }))
}

fn describe_fix(
    space: &AddressSpace,
    fix: &Fix,
) -> String {
    match fix {
        Fix::SetTypeDefinition { type_definition, .. } => {
            format!("Point the node at {}", label_of(space, type_definition))
        }
        Fix::RemoveReference {
            holder,
            reference_type,
            target,
            ..
        } => format!(
            "Drop the {} reference {} states towards {}",
            label_of(space, reference_type),
            label_of(space, holder),
            label_of(space, target)
        ),
        Fix::SetBrowseName { browse_name, .. } => format!("Rename the node to {browse_name}"),
        Fix::SetDataType { data_type, .. } => format!("Set the DataType to {}", label_of(space, data_type)),
        Fix::SetValueRank { value_rank, .. } => format!("Set the ValueRank to {value_rank}"),
        Fix::SetArrayDimensions { array_dimensions, .. } => {
            format!("Set the ArrayDimensions to [{array_dimensions}]")
        }
        Fix::SetModellingRule { modelling_rule, .. } => match modelling_rule {
            Some(rule) => format!("Set the ModellingRule to {}", label_of(space, rule)),
            None => "Remove the HasModellingRule reference".to_owned(),
        },
        Fix::SetInverseName { inverse_name, .. } => match inverse_name.is_empty() {
            true => "Drop the InverseName, which a symmetric ReferenceType has no use for".to_owned(),
            false => "Set the InverseName".to_owned(),
        },
        Fix::SetParentNodeId { parent, .. } => match parent {
            Some(parent) => format!("Set ParentNodeId to {}", label_of(space, parent)),
            None => "Clear ParentNodeId".to_owned(),
        },
        Fix::MaterializeChild { path, .. } => format!("Create the child the type declares at {path}"),
        Fix::SetReferenceType { new_reference_type, .. } => {
            format!("Restate the reference as {} — remove it and add the new one", label_of(space, new_reference_type))
        }
        Fix::SetNodeId { new_node_id, .. } => {
            format!("Give the node the NodeId {new_node_id}; this editor has no NodeId edit yet")
        }
        Fix::DeclareNamespace { index } => {
            format!("Add a URI for namespace index {index} to the nodeset's namespace table")
        }
        Fix::DefineAlias { alias } => format!("Define the alias `{alias}` in the nodeset's alias table"),
        Fix::LoadDependency { namespace_uri, target } => match namespace_uri {
            Some(uri) => format!("Put the nodeset that defines {uri} in the workspace, beside this file"),
            None => format!("Load the nodeset that defines {target}"),
        },
    }
}

/// The counts the tab badge shows, read without materialising a finding.
pub fn counts(handle: EditorHandle) -> FindingCounts {
    handle
        .with_session(|session| session.diagnostics().counts)
        .unwrap_or_default()
}

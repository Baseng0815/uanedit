//! The create-instance wizard (workflow/type-aware-creation.md §1).
//!
//! Everything it offers is `rules::plan::instantiation_plan` rendered: Mandatory declarations are
//! pre-checked and locked, Optional ones are checkboxes, a placeholder is answered by naming
//! children, and a declaration's type may be narrowed to one of the concrete types the engine
//! lists. What the user answered travels in one operation, so the instance is complete the moment
//! it exists and undoes as one step.

use std::collections::HashSet;
use std::rc::Rc;

use dioxus::prelude::*;
use uanedit::edit::{
    InstantiateType,
    PlaceholderChild,
    Refusal,
    Selections,
    TypeNarrowing,
};
use uanedit::nodes::NodeClass;
use uanedit::rules::plan::{
    self,
    PlannedDeclaration,
};
use uanedit::rules::query;
use uanedit::space::AddressSpace;
use uanedit::space::browse_path::BrowsePath;
use uanedit::space::declarations::ModellingRule;
use uanedit::types::localized_text::LocalizedText;
use uanedit::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use uanedit::types::qualified_name::QualifiedName;
use uanedit::{
    Operation,
    ids,
};

use crate::components::Icon;
use crate::session::{
    EditorHandle,
    Outcome,
    Status,
};
use crate::views::editor::counted;
use crate::views::editor::diagnostics::RefusalNotice;
use crate::views::editor::fields::Choice;
use crate::views::editor::forms::{
    namespace_choices,
    node_choices,
};
use crate::views::editor::icons::class_icon;
use crate::views::editor::picker::{
    NodePicker,
    PickFilter,
    candidate,
    label_of,
};
use crate::views::editor::shell::{
    Dialog,
    Dialogs,
    Navigate,
};

/// One declaration of the plan, with the labels and choice lists the row needs to render.
#[derive(Clone, PartialEq)]
struct PlanRow {
    path: BrowsePath,
    browse_name: QualifiedName,
    suggested: QualifiedName,
    label: String,
    node_class: NodeClass,
    modelling_rule: ModellingRule,
    type_definition: Option<String>,
    type_choices: Vec<Choice>,
    detail: Option<String>,
    children: Vec<PlanRow>,
}

impl PlanRow {
    fn is_locked(&self) -> bool {
        self.modelling_rule == ModellingRule::Mandatory
    }

    fn is_placeholder(&self) -> bool {
        self.modelling_rule.is_placeholder()
    }
}

/// What the wizard was opened with, either from the tree or from the plain create dialog.
#[derive(Clone, Default, PartialEq)]
pub struct Seed {
    pub anchor: Option<NodeId>,
    pub type_definition: Option<NodeId>,
    pub reference_type: Option<NodeId>,
    pub browse_name: Option<QualifiedName>,
    pub display_name: String,
}

#[component]
pub fn InstantiateDialog(seed: Seed) -> Element {
    let handle: EditorHandle = use_context();
    let dialogs: Dialogs = use_context();
    let navigate: Navigate = use_context();

    let mut instance_class = use_signal(|| {
        seed.type_definition
            .as_ref()
            .and_then(|type_node| handle.with_space(|space| plan::instance_class(space, type_node)))
            .filter(|class| *class != NodeClass::Unspecified)
            .unwrap_or(NodeClass::Object)
    });
    let mut type_node = use_signal(|| seed.type_definition.clone());
    let mut parent = use_signal(|| seed.anchor.clone());
    let mut reference_type = use_signal(|| seed.reference_type.clone().or(Some(ids::HAS_COMPONENT)));
    let mut namespace = use_signal(|| {
        seed.browse_name
            .as_ref()
            .map_or_else(|| own_namespace(handle), |name| name.namespace_index)
    });
    let mut name = use_signal(|| {
        seed.browse_name
            .as_ref()
            .map(|browse_name| browse_name.name.clone())
            .unwrap_or_default()
    });
    let mut display_name = use_signal(|| seed.display_name.clone());
    let mut selections = use_signal(Selections::default);
    let mut picking_type = use_signal(|| false);
    let mut picking_parent = use_signal(|| false);
    let refused = use_signal(|| None::<Refusal>);

    let namespaces = use_memo(move || {
        let _ = *handle.revision.read();
        handle.with_space(namespace_choices).unwrap_or_default()
    });

    let concrete = use_memo(move || {
        let _ = *handle.revision.read();
        let base = match *instance_class.read() {
            NodeClass::Variable => ids::BASE_VARIABLE_TYPE,
            _ => ids::BASE_OBJECT_TYPE,
        };
        handle
            .with_space(|space| query::concrete_subtypes(space, &base))
            .unwrap_or_default()
    });

    let plan = use_memo(move || {
        let _ = *handle.revision.read();
        let chosen = type_node.read().clone()?;
        handle.with_space(|space| plan::instantiation_plan(space, &chosen))
    });

    let rows = use_memo(move || {
        let _ = *handle.revision.read();
        let plan = plan.read().clone();
        let Some(plan) = plan else {
            return Vec::new();
        };
        handle
            .with_space(|space| decorate(space, &plan.children))
            .unwrap_or_default()
    });

    // A type from another class, or from before the class changed, is not on the engine's list.
    use_effect(move || {
        let offered = concrete.read().clone();
        let chosen = type_node.peek().clone();
        if chosen.is_none_or(|chosen| offered.contains(&chosen)) {
            return;
        }
        type_node.set(None);
        selections.set(Selections::default());
    });

    let hierarchical = use_memo(move || {
        let _ = *handle.revision.read();
        let class = *instance_class.read();
        let Some(parent) = parent.read().clone() else {
            return Vec::new();
        };
        handle
            .with_space(|space| node_choices(space, &query::legal_child_reference_types(space, &parent, class), None))
            .unwrap_or_default()
    });

    use_effect(move || {
        let offered = hierarchical.read().clone();
        let chosen = reference_type.peek().as_ref().map(ToString::to_string);
        if offered.is_empty()
            || offered
                .iter()
                .any(|choice| Some(&choice.value) == chosen.as_ref())
        {
            return;
        }
        reference_type.set(offered[0].value.parse::<NodeId>().ok());
    });

    let class_now = *instance_class.read();
    let plan_now = plan.read().clone();
    let rows_now = rows.read().clone();
    let selections_now = selections.read().clone();
    let namespaces_now = namespaces.read().clone();
    let reference_now = reference_type.read().as_ref().map(ToString::to_string);
    let namespace_now = namespace.read().to_string();
    let type_label = type_node
        .read()
        .clone()
        .and_then(|node_id| handle.with_space(|space| candidate(space, &node_id)));
    let parent_label = parent
        .read()
        .clone()
        .and_then(|node_id| handle.with_space(|space| candidate(space, &node_id)));
    let verdict = plan_now
        .as_ref()
        .map(|plan| plan.verdict.clone())
        .unwrap_or_default();
    let owed = owed_placeholders(&rows_now, &selections_now);
    let total = 1 + planned_count(&rows_now, &selections_now);
    let operation = build(
        parent.read().clone(),
        reference_type.read().clone(),
        type_node.read().clone(),
        QualifiedName::new(*namespace.read(), name.read().trim()),
        display_name.read().trim().to_owned(),
        selections_now.clone(),
    );
    let ready = operation.is_some() && owed.is_empty() && verdict.is_allowed();
    let confirm = operation.clone();
    let overridden = operation.clone();
    let refusal_now = refused.read().clone();
    let filter = {
        let offered: Rc<HashSet<NodeId>> = Rc::new(concrete.read().iter().cloned().collect());
        PickFilter::new(move |_, node_id| offered.contains(node_id))
    };

    rsx! {
        Dialog {
            title: "Create an instance of a type".to_owned(),
            icon: "widgets",
            subtitle: "Every child the type's modelling rules ask for is created in the same operation, and undoes as one step."
                .to_owned(),
            wide: true,
            onclose: move |_| dialogs.close(),
            div { class: "dialog__body",
                div { class: "field",
                    span { class: "field__label", "Instance class" }
                    div { class: "class-grid",
                        for choice in [NodeClass::Object, NodeClass::Variable] {
                            button {
                                key: "{choice}",
                                class: if choice == class_now { "class-grid__option on" } else { "class-grid__option" },
                                onclick: move |_| instance_class.set(choice),
                                Icon { name: class_icon(choice), class: "small" }
                                "{choice}"
                            }
                        }
                    }
                    span { class: "field__hint",
                        "The picker offers concrete types only — an abstract type has no instances (OPC 10000-3 §5.5.2)."
                    }
                }
                div { class: "field",
                    span { class: "field__label", "Type" }
                    div { class: "field__row",
                        button {
                            class: "button tonal tiny",
                            onclick: move |_| picking_type.set(true),
                            Icon { name: "search", class: "small" }
                            {
                                type_label
                                    .as_ref()
                                    .map_or_else(|| "Choose a type…".to_owned(), |named| named.label.clone())
                            }
                        }
                        if let Some(named) = type_label.clone() {
                            span { class: "field__detail mono", {named.identifier} }
                        }
                        span { class: "chip mono", "{concrete.read().len()} concrete" }
                    }
                }
                div { class: "field",
                    span { class: "field__label", "Parent" }
                    div { class: "field__row",
                        button {
                            class: "button tonal tiny",
                            onclick: move |_| picking_parent.set(true),
                            Icon { name: "search", class: "small" }
                            {
                                parent_label
                                    .as_ref()
                                    .map_or_else(|| "Choose a node…".to_owned(), |named| named.label.clone())
                            }
                        }
                        if let Some(named) = parent_label.clone() {
                            span { class: "field__detail mono", {named.identifier} }
                        }
                    }
                }
                div { class: "field",
                    span { class: "field__label", "Hierarchical reference" }
                    select {
                        class: "field__select",
                        value: reference_now.clone().unwrap_or_default(),
                        onchange: move |event| reference_type.set(event.value().parse::<NodeId>().ok()),
                        for choice in hierarchical.read().iter() {
                            option {
                                key: "{choice.value}",
                                value: "{choice.value}",
                                selected: Some(&choice.value) == reference_now.as_ref(),
                                "{choice.label}"
                            }
                        }
                    }
                }
                div { class: "field",
                    span { class: "field__label", "BrowseName" }
                    div { class: "field__row",
                        select {
                            class: "field__select narrow mono",
                            value: "{namespace}",
                            onchange: move |event| namespace.set(event.value().parse().unwrap_or(0)),
                            for choice in namespaces_now.iter() {
                                option {
                                    key: "{choice.value}",
                                    value: "{choice.value}",
                                    selected: choice.value == namespace_now,
                                    "{choice.label}"
                                }
                            }
                        }
                        input {
                            class: "field__input",
                            value: "{name}",
                            placeholder: "the name the new instance browses under",
                            oninput: move |event| name.set(event.value()),
                        }
                    }
                }
                div { class: "field",
                    span { class: "field__label", "DisplayName" }
                    input {
                        class: "field__input",
                        value: "{display_name}",
                        placeholder: "defaults to the BrowseName",
                        oninput: move |event| display_name.set(event.value()),
                    }
                }
                if plan_now.is_some() {
                    section { class: "plan__section",
                        header { class: "plan__section-head",
                            Icon { name: "account_tree", class: "small" }
                            span { class: "type-label", "What the type declares" }
                            div { class: "plan__spacer" }
                            span { class: "chip mono", {counted(total, "node")} }
                        }
                        if rows_now.is_empty() {
                            span { class: "field__hint", "The type declares no children." }
                        }
                        {plan_tree(&rows_now, 0, selections, &namespaces_now)}
                    }
                }
                if !verdict.is_allowed() {
                    div { class: "plan__blockers",
                        div { class: "plan__blockers-head",
                            Icon { name: "block", class: "small" }
                            span { class: "type-label", "No instance of this type may exist" }
                        }
                        for (position , finding) in verdict.findings.iter().enumerate() {
                            div { key: "{position}", class: "plan__row read-only",
                                span { class: "mono refusal__code", "{finding.code.id()}" }
                                span { class: "plan__row-name", "{finding.message}" }
                            }
                        }
                    }
                }
                for (position , owed) in owed.iter().enumerate() {
                    div { key: "o{position}", class: "plan__row read-only",
                        Icon { name: "priority_high", class: "small" }
                        span { class: "plan__row-name", {owed.clone()} }
                    }
                }
                if let Some(refusal) = refusal_now {
                    RefusalNotice {
                        refusal,
                        onoverride: move |reason: Option<String>| {
                            if let Some(operation) = overridden.clone() {
                                created(handle, dialogs, navigate, operation, Some(reason), refused);
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
                    title: match (operation.is_some(), owed.is_empty()) {
                        (false, _) => "A type, a parent and a BrowseName are needed first",
                        (_, false) => "Every MandatoryPlaceholder needs at least one named child",
                        _ => "Create the instance and everything its type asks for, as one undo entry",
                    },
                    onclick: move |_| {
                        if let Some(operation) = confirm.clone() {
                            created(handle, dialogs, navigate, operation, None, refused);
                        }
                    },
                    {format!("Create {}", counted(total, "node"))}
                }
            }
        }
        if *picking_type.read() {
            NodePicker {
                title: "Choose the type to instantiate".to_owned(),
                hint: "Concrete types only; the wizard then offers what the type declares.".to_owned(),
                filter: Some(filter),
                onpick: move |node_id: NodeId| {
                    type_node.set(Some(node_id));
                    selections.set(Selections::default());
                    picking_type.set(false);
                },
                oncancel: move |_| picking_type.set(false),
            }
        }
        if *picking_parent.read() {
            NodePicker {
                title: "Choose the parent".to_owned(),
                hint: "The new instance hangs under this one.".to_owned(),
                onpick: move |node_id: NodeId| {
                    parent.set(Some(node_id));
                    picking_parent.set(false);
                },
                oncancel: move |_| picking_parent.set(false),
            }
        }
    }
}

fn decorate(
    space: &AddressSpace,
    declarations: &[PlannedDeclaration],
) -> Vec<PlanRow> {
    declarations
        .iter()
        .map(|declaration| PlanRow {
            path: declaration.path.clone(),
            browse_name: declaration.browse_name.clone(),
            suggested: declaration.suggested_browse_name(),
            label: declaration
                .display_name
                .first()
                .map(|text| text.text.clone())
                .unwrap_or_else(|| declaration.browse_name.name.clone()),
            node_class: declaration.node_class,
            modelling_rule: declaration.modelling_rule,
            type_definition: declaration
                .type_definition
                .as_ref()
                .map(ToString::to_string),
            type_choices: node_choices(
                space,
                &declaration.type_definition_choices,
                declaration.type_definition.as_ref(),
            ),
            detail: detail_of(space, declaration),
            children: decorate(space, &declaration.children),
        })
        .collect()
}

fn detail_of(
    space: &AddressSpace,
    declaration: &PlannedDeclaration,
) -> Option<String> {
    let data_type = declaration.data_type.as_ref()?;
    let rank = declaration.value_rank.unwrap_or_default();
    Some(format!("{} · {rank}", label_of(space, data_type)))
}

/// The declarations, with the ones beneath a chosen declaration nested under it.
fn plan_tree(
    rows: &[PlanRow],
    depth: usize,
    selections: Signal<Selections>,
    namespaces: &[Choice],
) -> Element {
    let chosen = selections.read().clone();

    rsx! {
        for row in rows.iter() {
            div { key: "{row.path}", class: "decl",
                {declaration_row(row, depth, selections, namespaces)}
                if row.is_placeholder() {
                    {placeholder_children(row, depth, selections, namespaces)}
                } else if row.is_locked() || chosen.chose(&row.path) {
                    {plan_tree(&row.children, depth + 1, selections, namespaces)}
                }
            }
        }
    }
}

fn declaration_row(
    row: &PlanRow,
    depth: usize,
    mut selections: Signal<Selections>,
    _namespaces: &[Choice],
) -> Element {
    let chosen = selections.read().clone();
    let path = row.path.clone();
    let materialized = row.is_locked() || chosen.chose(&path);
    let narrowed = chosen
        .narrowing(&path)
        .map(ToString::to_string)
        .or_else(|| row.type_definition.clone())
        .unwrap_or_default();
    let toggle_path = path.clone();
    let narrow_path = path.clone();

    rsx! {
        div {
            class: match materialized {
                true => "decl__row on",
                false => "decl__row",
            },
            style: "padding-left: calc({depth} * var(--gap-lg))",
            if row.is_placeholder() {
                Icon { name: "more_horiz", class: "small" }
            } else {
                input {
                    r#type: "checkbox",
                    checked: materialized,
                    disabled: row.is_locked(),
                    title: match row.is_locked() {
                        true => "Mandatory — every instance of the type has it (OPC 10000-3 §6.4.4.4.1)",
                        false => "Optional — create it or leave it out",
                    },
                    onchange: move |event| {
                        let on = event.checked();
                        selections
                            .with_mut(|selections| {
                                selections.optionals.retain(|chosen| !chosen.starts_with(&toggle_path));
                                if on {
                                    selections.optionals.push(toggle_path.clone());
                                    return;
                                }
                                selections.placeholders.retain(|child| !child.path.starts_with(&toggle_path));
                                selections.narrowings.retain(|narrowing| !narrowing.path.starts_with(&toggle_path));
                            });
                    },
                }
            }
            Icon { name: class_icon(row.node_class), class: "tree__class small" }
            span { class: "decl__name", "{row.browse_name}" }
            span { class: "chip", "{row.modelling_rule.name()}" }
            if let Some(detail) = row.detail.clone() {
                span { class: "decl__detail mono", {detail} }
            }
            div { class: "plan__spacer" }
            if row.type_choices.len() > 1 && !row.is_placeholder() {
                select {
                    class: "field__select narrow",
                    value: narrowed.clone(),
                    disabled: !materialized,
                    title: "The type the child is an instance of; a subtype narrows it",
                    onchange: move |event| {
                        let Ok(picked) = event.value().parse::<NodeId>() else {
                            return;
                        };
                        selections
                            .with_mut(|selections| {
                                selections.narrowings.retain(|narrowing| narrowing.path != narrow_path);
                                selections
                                    .narrowings
                                    .push(TypeNarrowing {
                                        path: narrow_path.clone(),
                                        type_definition: picked,
                                    });
                            });
                    },
                    for choice in row.type_choices.iter() {
                        option {
                            key: "{choice.value}",
                            value: "{choice.value}",
                            selected: choice.value == narrowed,
                            "{choice.label}"
                        }
                    }
                }
            }
        }
    }
}

/// A placeholder is answered by naming children, which is the only thing the type leaves open
/// (OPC 10000-3 §6.4.4.4.4).
fn placeholder_children(
    row: &PlanRow,
    depth: usize,
    mut selections: Signal<Selections>,
    namespaces: &[Choice],
) -> Element {
    let chosen = selections.read().clone();
    let named: Vec<(usize, PlaceholderChild)> = chosen
        .placeholders
        .iter()
        .enumerate()
        .filter(|(_, child)| child.path == row.path)
        .map(|(position, child)| (position, child.clone()))
        .collect();
    let path = row.path.clone();
    let suggested = row.suggested.clone();
    let type_choices = row.type_choices.clone();
    let declared = row.type_definition.clone().unwrap_or_default();
    let owed = named.is_empty() && row.modelling_rule.is_required();

    rsx! {
        div {
            class: "decl__children",
            style: "padding-left: calc({depth + 1} * var(--gap-lg))",
            for (position , child) in named.iter().cloned() {
                div { key: "p{position}", class: "decl__child",
                    select {
                        class: "field__select narrow mono",
                        value: "{child.browse_name.namespace_index}",
                        onchange: move |event| {
                            let index: NamespaceIndex = event.value().parse().unwrap_or(0);
                            selections
                                .with_mut(|selections| {
                                    if let Some(child) = selections.placeholders.get_mut(position) {
                                        child.browse_name.namespace_index = index;
                                    }
                                });
                        },
                        for choice in namespaces.iter() {
                            option {
                                key: "{choice.value}",
                                value: "{choice.value}",
                                selected: choice.value == child.browse_name.namespace_index.to_string(),
                                "{choice.label}"
                            }
                        }
                    }
                    input {
                        class: "field__input",
                        value: "{child.browse_name.name}",
                        placeholder: "the name this child browses under",
                        oninput: move |event| {
                            let name = event.value();
                            selections
                                .with_mut(|selections| {
                                    if let Some(child) = selections.placeholders.get_mut(position) {
                                        child.browse_name.name = name;
                                    }
                                });
                        },
                    }
                    if type_choices.len() > 1 {
                        select {
                            class: "field__select narrow",
                            value: child.type_definition.as_ref().map(ToString::to_string).unwrap_or_else(|| declared.clone()),
                            onchange: move |event| {
                                let picked = event.value().parse::<NodeId>().ok();
                                selections
                                    .with_mut(|selections| {
                                        if let Some(child) = selections.placeholders.get_mut(position) {
                                            child.type_definition = picked.clone();
                                        }
                                    });
                            },
                            for choice in type_choices.iter() {
                                option {
                                    key: "{choice.value}",
                                    value: "{choice.value}",
                                    selected: choice.value
                                        == child
                                            .type_definition
                                            .as_ref()
                                            .map(ToString::to_string)
                                            .unwrap_or_else(|| declared.clone()),
                                    "{choice.label}"
                                }
                            }
                        }
                    }
                    button {
                        class: "icon-button tiny",
                        title: "Drop this child",
                        onclick: move |_| {
                            selections
                                .with_mut(|selections| {
                                    if position < selections.placeholders.len() {
                                        selections.placeholders.remove(position);
                                    }
                                });
                        },
                        Icon { name: "close", class: "small" }
                    }
                }
            }
            button {
                class: if owed { "button tonal tiny" } else { "button text tiny" },
                onclick: move |_| {
                    selections
                        .with_mut(|selections| {
                            selections
                                .placeholders
                                .push(PlaceholderChild {
                                    path: path.clone(),
                                    browse_name: suggested.clone(),
                                    display_name: Vec::new(),
                                    description: Vec::new(),
                                    type_definition: None,
                                });
                        });
                },
                Icon { name: "add", class: "small" }
                "Add a named child"
            }
        }
    }
}

/// The MandatoryPlaceholder declarations the instance still owes a child (§6.4.4.4.5).
fn owed_placeholders(
    rows: &[PlanRow],
    selections: &Selections,
) -> Vec<String> {
    let mut owed = Vec::new();
    for row in rows {
        if row.is_placeholder() {
            if row.modelling_rule.is_required() && selections.children_of(&row.path).next().is_none() {
                owed.push(format!("{} is a MandatoryPlaceholder and has no child yet", row.browse_name));
            }
            continue;
        }
        if row.is_locked() || selections.chose(&row.path) {
            owed.extend(owed_placeholders(&row.children, selections));
        }
    }
    owed
}

/// How many nodes the operation would create beneath the root.
fn planned_count(
    rows: &[PlanRow],
    selections: &Selections,
) -> usize {
    let mut count = 0;
    for row in rows {
        if row.is_placeholder() {
            count += selections.children_of(&row.path).count();
            continue;
        }
        if !row.is_locked() && !selections.chose(&row.path) {
            continue;
        }
        count += 1 + planned_count(&row.children, selections);
    }
    count
}

fn build(
    parent: Option<NodeId>,
    reference_type: Option<NodeId>,
    type_definition: Option<NodeId>,
    browse_name: QualifiedName,
    display_name: String,
    selections: Selections,
) -> Option<Operation> {
    if browse_name.name.is_empty() {
        return None;
    }
    Some(Operation::InstantiateType(Box::new(InstantiateType {
        parent: parent?,
        reference_type: reference_type?,
        browse_name,
        display_name: match display_name.is_empty() {
            true => Vec::new(),
            false => vec![LocalizedText::new(display_name)],
        },
        description: Vec::new(),
        type_definition: type_definition?,
        modelling_rule: None,
        selections,
    })))
}

fn created(
    handle: EditorHandle,
    dialogs: Dialogs,
    navigate: Navigate,
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
            if let Some(node_id) = done.created.first() {
                navigate.to(node_id.clone());
            }
            handle.say(instantiated(&done));
            match done.overridden.is_empty() {
                true => dialogs.close(),
                false => dialogs.report(done.overridden),
            }
        }
        Outcome::Unchanged | Outcome::Closed => dialogs.close(),
    }
}

/// What the app bar says: the label the one undo entry carries, and how much it created.
fn instantiated(done: &crate::session::Done) -> Status {
    let text = format!("{} · {} · one undo entry", done.label, counted(done.created.len(), "node"));
    if !done.overridden.is_empty() {
        return Status::error(format!("{text} · overridden, see Validation"));
    }
    match done.warnings {
        0 => Status::success(text),
        1 => Status::info(format!("{text} · one new warning")),
        count => Status::info(format!("{text} · {count} new warnings")),
    }
}

/// The namespace index the nodeset creates its own nodes in.
fn own_namespace(handle: EditorHandle) -> NamespaceIndex {
    handle
        .with_session(|session| {
            session
                .primary()
                .target_namespace()
                .and_then(|uri| session.space().namespace_index(uri))
        })
        .flatten()
        .unwrap_or(1)
}

/// True when the type declares children an instance owes it, which is what the wizard exists for.
pub fn declares_required_children(
    space: &AddressSpace,
    type_definition: &NodeId,
) -> bool {
    plan::instantiation_plan(space, type_definition)
        .declarations()
        .iter()
        .any(|declaration| declaration.is_required())
}

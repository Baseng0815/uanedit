//! Node lifecycle: the create dialog and the deletion-resolution dialog.
//!
//! Both are one operation. Creating asks for everything OPC 10000-3 requires of the class in one
//! call, and deleting answers for every reference and every child before it runs, so the graph is
//! never left dangling and the whole thing is one undo entry (guardrails.md §3).

use dioxus::prelude::*;
use uanedit::attributes::array_dimensions::ArrayDimensions;
use uanedit::attributes::event_notifier::EventNotifier;
use uanedit::attributes::value_rank::ValueRank;
use uanedit::edit::delete::{
    AttributeResolution,
    AttributeUse,
    ChildResolution,
    DeleteNode,
    IncomingReference,
    IncomingResolution,
};
use uanedit::edit::{
    CreateInstance,
    CreateType,
    InstanceAttributes,
    Refusal,
    TypeAttributes,
};
use uanedit::nodes::NodeClass;
use uanedit::nodes::data_type::DataTypePurpose;
use uanedit::rules::query;
use uanedit::space::{
    AddressSpace,
    AttributeSlot,
};
use uanedit::types::localized_text::LocalizedText;
use uanedit::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use uanedit::types::qualified_name::QualifiedName;
use uanedit::{
    Operation,
    Session,
    ids,
};

use crate::components::Icon;
use crate::session::{
    EditorHandle,
    Outcome,
};
use crate::views::editor::diagnostics::RefusalNotice;
use crate::views::editor::icons::class_icon;
use crate::views::editor::instantiate::{
    Seed,
    declares_required_children,
};
use crate::views::editor::picker::{
    NodePicker,
    PickFilter,
    candidate,
    label_of,
};
use crate::views::editor::references::finish;
use crate::views::editor::shell::{
    Dialog,
    Dialogs,
    Navigate,
    Request,
};
use crate::views::editor::short_namespace;

/// The classes a bare create offers: the eight, less Unspecified.
const CREATABLE: [NodeClass; 8] = [
    NodeClass::Object,
    NodeClass::Variable,
    NodeClass::Method,
    NodeClass::View,
    NodeClass::ObjectType,
    NodeClass::VariableType,
    NodeClass::DataType,
    NodeClass::ReferenceType,
];

const MAX_DIMENSIONS: u32 = 4;

/// One choice of a `<select>` over nodes: the NodeId as the value, the DisplayName beside it.
#[derive(Clone, PartialEq)]
struct Choice {
    value: String,
    label: String,
}

#[component]
pub fn CreateDialog(anchor: Option<NodeId>) -> Element {
    let handle: EditorHandle = use_context();
    let dialogs: Dialogs = use_context();
    let navigate: Navigate = use_context();

    let mut node_class = use_signal(|| NodeClass::Object);
    let mut parent = use_signal(|| anchor.clone());
    let mut reference_type = use_signal(|| Some(ids::HAS_COMPONENT));
    let mut namespace = use_signal(move || own_namespace(handle));
    let mut name = use_signal(String::new);
    let mut display_name = use_signal(String::new);
    let mut type_definition = use_signal(|| None::<NodeId>);
    let mut data_type = use_signal(|| ids::BASE_DATA_TYPE);
    let mut value_rank = use_signal(|| ValueRank::SCALAR);
    let mut is_abstract = use_signal(|| false);
    let mut symmetric = use_signal(|| false);
    let mut purpose = use_signal(|| DataTypePurpose::Normal);
    let mut executable = use_signal(|| true);
    let mut contains_no_loops = use_signal(|| false);
    let mut picking = use_signal(|| false);
    let refused = use_signal(|| None::<Refusal>);

    let namespaces = use_memo(move || {
        let _ = *handle.revision.read();
        handle.with_space(namespace_choices).unwrap_or_default()
    });

    let hierarchical = use_memo(move || {
        let _ = *handle.revision.read();
        let class = *node_class.read();
        let Some(parent) = parent.read().clone() else {
            return Vec::new();
        };
        handle
            .with_space(|space| choices(space, query::legal_child_reference_types(space, &parent, class)))
            .unwrap_or_default()
    });

    // The engine's list is what the parent takes; a type left over from another parent is not on it.
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
        type_definition.set(None);
    });

    let type_definitions = use_memo(move || {
        let _ = *handle.revision.read();
        let class = *node_class.read();
        let reference_type = reference_type.read().clone();
        handle
            .with_space(|space| {
                choices(space, query::legal_type_definitions_for(space, class, reference_type.as_ref()))
            })
            .unwrap_or_default()
    });

    let data_types = use_memo(move || {
        let _ = *handle.revision.read();
        handle
            .with_space(|space| choices(space, query::legal_data_type_narrowings(space, &ids::BASE_DATA_TYPE)))
            .unwrap_or_default()
    });

    // The type the new node would be an instance of, which is what the wizard would work from.
    let effective_type = use_memo(move || {
        let class = *node_class.read();
        if !matches!(class, NodeClass::Object | NodeClass::Variable) {
            return None;
        }
        let reference_type = reference_type.read().clone();
        let chosen = type_definition.read().clone();
        handle
            .with_space(|space| {
                chosen.or_else(|| query::default_type_definition_for_class(space, class, reference_type.as_ref()))
            })
            .flatten()
    });

    // 18 of the first 40 ObjectTypes declare Mandatory children; a bare create leaves them out.
    let declares_children = use_memo(move || {
        let _ = *handle.revision.read();
        let Some(type_node) = effective_type.read().clone() else {
            return false;
        };
        handle
            .with_space(|space| declares_required_children(space, &type_node))
            .unwrap_or(false)
    });

    let class_now = *node_class.read();
    let instance = class_now.is_instance();
    let reference_now = reference_type.read().as_ref().map(ToString::to_string);
    let namespace_now = namespace.read().to_string();
    let type_definition_now = type_definition.read().as_ref().map(ToString::to_string);
    let data_type_now = data_type.read().to_string();
    let rank_now = *value_rank.read();
    let purpose_now = *purpose.read();
    let ranks = query::legal_value_rank_narrowings(ValueRank::ANY, MAX_DIMENSIONS);
    let anchored = parent.read().clone();
    let anchored_name = anchored
        .as_ref()
        .and_then(|node_id| handle.with_space(|space| candidate(space, node_id)));
    let anchor_label = anchored_name
        .as_ref()
        .map_or_else(|| "Choose a node…".to_owned(), |named| named.label.clone());
    let operation = handle
        .with_space(|space| {
            build(
                space,
                class_now,
                anchored.clone(),
                reference_type.read().clone(),
                QualifiedName::new(*namespace.read(), name.read().trim()),
                display_name.read().trim().to_owned(),
                Attributes {
                    type_definition: type_definition.read().clone(),
                    data_type: data_type.read().clone(),
                    value_rank: *value_rank.read(),
                    is_abstract: *is_abstract.read(),
                    symmetric: *symmetric.read(),
                    purpose: *purpose.read(),
                    executable: *executable.read(),
                    contains_no_loops: *contains_no_loops.read(),
                },
            )
        })
        .flatten();
    let confirm = operation.clone();
    let overridden = operation.clone();
    let ready = operation.is_some();
    let refusal_now = refused.read().clone();
    let picker_filter = match instance {
        true => None,
        false => Some(PickFilter::new(move |space, node_id| query::may_be_supertype(space, class_now, node_id))),
    };
    let open_wizard = move |_: MouseEvent| {
        dialogs.open(Request::Instantiate {
            seed: Seed {
                anchor: parent.peek().clone(),
                type_definition: effective_type.peek().clone(),
                reference_type: reference_type.peek().clone(),
                browse_name: Some(QualifiedName::new(*namespace.peek(), name.peek().trim())),
                display_name: display_name.peek().trim().to_owned(),
            },
        });
    };

    rsx! {
        Dialog {
            title: "Create node".to_owned(),
            icon: "add_circle",
            subtitle: "The NodeId is assigned in this nodeset's own namespace.".to_owned(),
            wide: true,
            onclose: move |_| dialogs.close(),
            div { class: "dialog__body",
                div { class: "field",
                    span { class: "field__label", "NodeClass" }
                    div { class: "class-grid",
                        for choice in CREATABLE {
                            button {
                                key: "{choice}",
                                class: if choice == class_now { "class-grid__option on" } else { "class-grid__option" },
                                onclick: move |_| {
                                    node_class.set(choice);
                                    type_definition.set(None);
                                    is_abstract.set(false);
                                },
                                Icon { name: class_icon(choice), class: "small" }
                                "{choice}"
                            }
                        }
                    }
                }
                div { class: "field",
                    span { class: "field__label",
                        if instance {
                            "Parent"
                        } else {
                            "Supertype"
                        }
                    }
                    div { class: "field__row",
                        button {
                            class: "button tonal tiny",
                            onclick: move |_| picking.set(true),
                            Icon { name: "search", class: "small" }
                            {anchor_label}
                        }
                        if let Some(named) = anchored_name.clone() {
                            span { class: "field__detail mono", {named.identifier} }
                        }
                    }
                    if !instance {
                        span { class: "field__hint",
                            "A HasSubtype reference from the supertype places the new type in the hierarchy."
                        }
                    }
                }
                if instance {
                    div { class: "field",
                        span { class: "field__label", "Hierarchical reference" }
                        select {
                            class: "field__select",
                            value: reference_type.read().as_ref().map(ToString::to_string).unwrap_or_default(),
                            onchange: move |event| {
                                reference_type.set(event.value().parse::<NodeId>().ok());
                                type_definition.set(None);
                            },
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
                }
                div { class: "field",
                    span { class: "field__label", "BrowseName" }
                    div { class: "field__row",
                        select {
                            class: "field__select narrow mono",
                            value: "{namespace}",
                            onchange: move |event| namespace.set(event.value().parse().unwrap_or(0)),
                            for choice in namespaces.read().iter() {
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
                            placeholder: "the name this node browses under",
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
                if matches!(class_now, NodeClass::Object | NodeClass::Variable) {
                    div { class: "field",
                        span { class: "field__label", "TypeDefinition" }
                        select {
                            class: "field__select",
                            value: type_definition.read().as_ref().map(ToString::to_string).unwrap_or_default(),
                            onchange: move |event| type_definition.set(event.value().parse::<NodeId>().ok()),
                            option { value: "", selected: type_definition_now.is_none(), "Default for this class and reference" }
                            for choice in type_definitions.read().iter() {
                                option {
                                    key: "{choice.value}",
                                    value: "{choice.value}",
                                    selected: Some(&choice.value) == type_definition_now.as_ref(),
                                    "{choice.label}"
                                }
                            }
                        }
                        if *declares_children.read() {
                            span { class: "field__hint",
                                "This type declares children every instance of it owes. A bare create makes the node alone; the wizard makes the hierarchy."
                            }
                        }
                    }
                }
                if matches!(class_now, NodeClass::Variable | NodeClass::VariableType) {
                    div { class: "field",
                        span { class: "field__label", "DataType" }
                        select {
                            class: "field__select",
                            value: "{data_type}",
                            onchange: move |event| {
                                if let Ok(picked) = event.value().parse::<NodeId>() {
                                    data_type.set(picked);
                                }
                            },
                            for choice in data_types.read().iter() {
                                option {
                                    key: "{choice.value}",
                                    value: "{choice.value}",
                                    selected: choice.value == data_type_now,
                                    "{choice.label}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        span { class: "field__label", "ValueRank" }
                        select {
                            class: "field__select",
                            value: "{value_rank.read().0}",
                            onchange: move |event| {
                                if let Ok(rank) = event.value().parse::<i32>() {
                                    value_rank.set(ValueRank(rank));
                                }
                            },
                            for rank in ranks.iter().copied() {
                                option { key: "{rank.0}", value: "{rank.0}", selected: rank == rank_now, "{rank}" }
                            }
                        }
                    }
                }
                if class_now.is_type() {
                    label { class: "field__toggle",
                        input {
                            r#type: "checkbox",
                            checked: *is_abstract.read(),
                            onchange: move |event| is_abstract.set(event.checked()),
                        }
                        span { class: "field__toggle-label", "IsAbstract" }
                    }
                }
                if class_now == NodeClass::ReferenceType {
                    label { class: "field__toggle",
                        input {
                            r#type: "checkbox",
                            checked: *symmetric.read(),
                            onchange: move |event| symmetric.set(event.checked()),
                        }
                        span { class: "field__toggle-label", "Symmetric" }
                    }
                }
                if class_now == NodeClass::DataType {
                    div { class: "field",
                        span { class: "field__label", "DataTypePurpose" }
                        select {
                            class: "field__select",
                            value: "{purpose.read()}",
                            onchange: move |event| {
                                if let Ok(picked) = event.value().parse::<DataTypePurpose>() {
                                    purpose.set(picked);
                                }
                            },
                            for choice in [DataTypePurpose::Normal, DataTypePurpose::ServicesOnly, DataTypePurpose::CodeGenerator] {
                                option { key: "{choice}", value: "{choice}", selected: choice == purpose_now, "{choice}" }
                            }
                        }
                    }
                }
                if class_now == NodeClass::Method {
                    label { class: "field__toggle",
                        input {
                            r#type: "checkbox",
                            checked: *executable.read(),
                            onchange: move |event| executable.set(event.checked()),
                        }
                        span { class: "field__toggle-label", "Executable" }
                    }
                }
                if class_now == NodeClass::View {
                    label { class: "field__toggle",
                        input {
                            r#type: "checkbox",
                            checked: *contains_no_loops.read(),
                            onchange: move |event| contains_no_loops.set(event.checked()),
                        }
                        span { class: "field__toggle-label", "ContainsNoLoops" }
                    }
                }
                if let Some(refusal) = refusal_now {
                    if *declares_children.read() {
                        div { class: "plan__ready",
                            Icon { name: "auto_fix_high", class: "small" }
                            span { class: "type-body-small",
                                "The type declares children the instance owes. The wizard creates them with it, in one operation."
                            }
                            div { class: "plan__spacer" }
                            button { class: "button tonal tiny", onclick: open_wizard,
                                Icon { name: "widgets", class: "small" }
                                "Open the instantiation wizard"
                            }
                        }
                    }
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
                if instance {
                    button {
                        class: "button text",
                        title: "Create an instance of a type, with everything its modelling rules ask for",
                        onclick: open_wizard,
                        Icon { name: "widgets", class: "small" }
                        "From type…"
                    }
                }
                div { class: "plan__spacer" }
                button { class: "button text", onclick: move |_| dialogs.close(), "Cancel" }
                button {
                    class: "button",
                    disabled: !ready,
                    onclick: move |_| {
                        if let Some(operation) = confirm.clone() {
                            created(handle, dialogs, navigate, operation, None, refused);
                        }
                    },
                    "Create"
                }
            }
        }
        if *picking.read() {
            NodePicker {
                title: match instance {
                    true => "Choose the parent".to_owned(),
                    false => "Choose the supertype".to_owned(),
                },
                hint: match instance {
                    true => "The new node hangs under this one.".to_owned(),
                    false => "The new type is a subtype of this one; the picker offers the same NodeClass."
                        .to_owned(),
                },
                filter: picker_filter,
                onpick: move |node_id: NodeId| {
                    parent.set(Some(node_id));
                    picking.set(false);
                },
                oncancel: move |_| picking.set(false),
            }
        }
    }
}

/// The per-class attributes a create carries, gathered so `build` takes one argument for them.
struct Attributes {
    type_definition: Option<NodeId>,
    data_type: NodeId,
    value_rank: ValueRank,
    is_abstract: bool,
    symmetric: bool,
    purpose: DataTypePurpose,
    executable: bool,
    contains_no_loops: bool,
}

fn build(
    space: &AddressSpace,
    node_class: NodeClass,
    anchor: Option<NodeId>,
    reference_type: Option<NodeId>,
    browse_name: QualifiedName,
    display_name: String,
    attributes: Attributes,
) -> Option<Operation> {
    let anchor = anchor.filter(|node_id| space.contains(node_id))?;
    if browse_name.name.is_empty() {
        return None;
    }
    let display_name = match display_name.is_empty() {
        true => Vec::new(),
        false => vec![LocalizedText::new(display_name)],
    };

    if node_class.is_instance() {
        let reference_type = reference_type?;
        let instance = match node_class {
            NodeClass::Object => InstanceAttributes::Object {
                type_definition: attributes.type_definition,
                event_notifier: EventNotifier::default(),
            },
            NodeClass::Variable => InstanceAttributes::Variable {
                type_definition: attributes.type_definition,
                data_type: attributes.data_type,
                value_rank: attributes.value_rank,
                array_dimensions: ArrayDimensions::default(),
            },
            NodeClass::Method => InstanceAttributes::Method {
                executable: attributes.executable,
                user_executable: attributes.executable,
            },
            _ => InstanceAttributes::View {
                contains_no_loops: attributes.contains_no_loops,
                event_notifier: EventNotifier::default(),
            },
        };
        return Some(Operation::CreateInstance(CreateInstance {
            parent: anchor,
            reference_type,
            browse_name,
            display_name,
            description: Vec::new(),
            modelling_rule: None,
            attributes: instance,
        }));
    }

    let type_attributes = match node_class {
        NodeClass::ObjectType => TypeAttributes::ObjectType,
        NodeClass::VariableType => TypeAttributes::VariableType {
            data_type: attributes.data_type,
            value_rank: attributes.value_rank,
            array_dimensions: ArrayDimensions::default(),
        },
        NodeClass::DataType => TypeAttributes::DataType {
            definition: None,
            purpose: attributes.purpose,
        },
        _ => TypeAttributes::ReferenceType {
            symmetric: attributes.symmetric,
            inverse_name: Vec::new(),
        },
    };
    Some(Operation::CreateType(CreateType {
        supertype: anchor,
        browse_name,
        display_name,
        description: Vec::new(),
        is_abstract: attributes.is_abstract,
        attributes: type_attributes,
    }))
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
            handle.say(done.status());
            match done.overridden.is_empty() {
                true => dialogs.close(),
                false => dialogs.report(done.overridden),
            }
        }
        Outcome::Unchanged | Outcome::Closed => dialogs.close(),
    }
}

/// Deleting a node: every incoming reference and every child answered for before it runs.
#[component]
pub fn DeleteDialog(node: NodeId) -> Element {
    let handle: EditorHandle = use_context();
    let dialogs: Dialogs = use_context();
    let mut delete = use_signal(|| DeleteNode::new(node.clone()));
    let mut retargeting = use_signal(|| None::<IncomingReference>);
    let renaming = use_signal(|| None::<AttributeUse>);
    let mut reparenting = use_signal(|| None::<NodeId>);
    let mut dangling_accepted = use_signal(|| false);
    let refused = use_signal(|| None::<Refusal>);

    let plan = use_memo(move || {
        let _ = *handle.revision.read();
        let asked = delete.read().clone();
        handle.deletion_plan(&asked).unwrap_or_default()
    });

    let current = plan.read().clone();
    let answered = delete.read().clone();
    // One list per answered reparent, since what a link may be of depends on the parent it names.
    let reparent_choices: Vec<Vec<Choice>> = answered
        .children
        .iter()
        .map(|(child, resolution)| match resolution {
            ChildResolution::Reparent { parent, .. } => handle
                .with_space(|space| {
                    let class = space.node_class(child).unwrap_or_default();
                    choices(space, query::legal_child_reference_types(space, parent, class))
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        })
        .collect();
    // One list per unanswered DataType, since what a node may narrow to depends on the node.
    let data_type_choices: Vec<Vec<Choice>> = current
        .attributes
        .iter()
        .map(|attribute| match &attribute.slot {
            AttributeSlot::DataType => handle
                .with_space(|space| {
                    let legal = query::legal_data_types_for(space, &attribute.holder)
                        .into_iter()
                        .filter(|node_id| !current.deleted.contains(node_id))
                        .collect();
                    choices(space, legal)
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        })
        .collect();
    let named = handle
        .with_session(|session| describe(session, &current, &answered))
        .unwrap_or_default();
    let blocked = !current.read_only.is_empty() && !*dangling_accepted.read();
    let ready = current.is_resolved() && !blocked;
    let operation = Operation::Delete(answered.clone());
    let confirm = operation.clone();
    let overridden = operation.clone();
    let refusal_now = refused.read().clone();
    let subject = handle
        .with_space(|space| label_of(space, &node))
        .unwrap_or_else(|| node.to_string());

    rsx! {
        Dialog {
            title: format!("Delete {subject}"),
            icon: "delete",
            subtitle: "Everything below is resolved in the same transaction, and undoes as one step."
                .to_owned(),
            wide: true,
            onclose: move |_| dialogs.close(),
            div { class: "dialog__body",
                div { class: "plan__summary",
                    Icon { name: "delete_sweep", class: "small" }
                    span { class: "type-label", {removes_text(current.deleted.len())} }
                    div { class: "plan__spacer" }
                    for label in named.deleted.iter().take(6) {
                        span { key: "{label}", class: "chip", {label.clone()} }
                    }
                    if named.deleted.len() > 6 {
                        span { class: "chip", "+{named.deleted.len() - 6}" }
                    }
                }
                if !current.read_only.is_empty() {
                    div { class: "plan__blockers",
                        div { class: "plan__blockers-head",
                            Icon { name: "lock", class: "small" }
                            span { class: "type-label", {blockers_text(current.read_only.len())} }
                        }
                        p { class: "type-body-small",
                            "They are stated in nodesets this editor does not change, so nothing here can remove or retarget them. Deleting anyway leaves them naming a NodeId no loaded nodeset defines."
                        }
                        for (position , blocker) in named.read_only.iter().enumerate() {
                            div { key: "{position}", class: "plan__row read-only",
                                span { class: "plan__row-name", {blocker.clone()} }
                            }
                        }
                        label { class: "field__toggle",
                            input {
                                r#type: "checkbox",
                                checked: *dangling_accepted.read(),
                                onchange: move |event| dangling_accepted.set(event.checked()),
                            }
                            span { class: "field__toggle-label",
                                "Delete anyway, leaving those references dangling"
                            }
                        }
                    }
                }
                {incoming_section(&current, &named, delete, retargeting)}
                {attributes_section(&current, &named, &data_type_choices, delete, renaming)}
                {children_section(&current, &named, delete, reparenting, &reparent_choices)}
                if current.is_resolved() {
                    div { class: "plan__ready",
                        Icon { name: "check_circle", class: "small" }
                        span { class: "type-body-small", "Nothing is left unanswered." }
                    }
                }
                if let Some(refusal) = refusal_now {
                    RefusalNotice {
                        refusal,
                        onoverride: move |reason: Option<String>| {
                            finish(handle, dialogs, overridden.clone(), Some(reason), refused);
                        },
                    }
                }
            }
            footer { class: "dialog__actions",
                button { class: "button text", onclick: move |_| dialogs.close(), "Cancel" }
                button {
                    class: "button danger",
                    disabled: !ready,
                    title: match (current.is_resolved(), blocked) {
                        (false, _) => "Every reference, attribute and child has to be answered for first",
                        (_, true) => "Accept the dangling references first",
                        _ => "Delete, as one undo entry",
                    },
                    onclick: move |_| finish(handle, dialogs, confirm.clone(), None, refused),
                    Icon { name: "delete", class: "small" }
                    "Delete"
                }
            }
        }
        if let Some(incoming) = retargeting.read().clone() {
            NodePicker {
                title: "Point the reference at another node".to_owned(),
                hint: "The reference stays where it is and names this node instead.".to_owned(),
                onpick: move |node_id: NodeId| {
                    delete
                        .with_mut(|delete| {
                            delete
                                .incoming
                                .push((incoming.clone(), IncomingResolution::Retarget { node: node_id }));
                        });
                    retargeting.set(None);
                },
                oncancel: move |_| retargeting.set(None),
            }
        }
        if let Some(attribute) = renaming.read().clone() {
            {attribute_picker(attribute, delete, renaming)}
        }
        if let Some(child) = reparenting.read().clone() {
            NodePicker {
                title: "Choose the new parent".to_owned(),
                hint: "The child keeps its own subtree and hangs under this node instead.".to_owned(),
                onpick: move |node_id: NodeId| {
                    let original = original_reference_type(&plan.read(), &child);
                    let reference_type = relink_type(handle, &node_id, &child, original);
                    delete
                        .with_mut(|delete| {
                            delete
                                .children
                                .push((
                                    child.clone(),
                                    ChildResolution::Reparent {
                                        parent: node_id,
                                        reference_type,
                                    },
                                ));
                        });
                    reparenting.set(None);
                },
                oncancel: move |_| reparenting.set(None),
            }
        }
    }
}

fn incoming_section(
    plan: &uanedit::edit::DeletionPlan,
    named: &Named,
    mut delete: Signal<DeleteNode>,
    mut retargeting: Signal<Option<IncomingReference>>,
) -> Element {
    let answered = delete.read().clone();
    if plan.incoming.is_empty() && answered.incoming.is_empty() {
        return rsx! {};
    }

    rsx! {
        section { class: "plan__section",
            header { class: "plan__section-head",
                Icon { name: "call_received", class: "small" }
                span { class: "type-label", "Incoming references" }
                div { class: "plan__spacer" }
                span { class: "chip mono", "{plan.incoming.len()} unanswered" }
            }
            for (position , incoming) in plan.incoming.iter().enumerate() {
                div { key: "u{position}", class: "plan__row",
                    span { class: "plan__row-name",
                        {named.incoming.get(position).cloned().unwrap_or_default()}
                    }
                    span { class: "plan__row-actions",
                        button {
                            class: "button tonal tiny",
                            onclick: {
                                let incoming = incoming.clone();
                                move |_| {
                                    delete
                                        .with_mut(|delete| {
                                            delete.incoming.push((incoming.clone(), IncomingResolution::Remove));
                                        })
                                }
                            },
                            "Remove"
                        }
                        button {
                            class: "button text tiny",
                            onclick: {
                                let incoming = incoming.clone();
                                move |_| retargeting.set(Some(incoming.clone()))
                            },
                            "Retarget…"
                        }
                    }
                }
            }
            for (position , resolved) in named.answered_incoming.iter().enumerate() {
                div { key: "a{position}", class: "plan__row answered",
                    Icon { name: "check", class: "small" }
                    span { class: "plan__row-name", {resolved.clone()} }
                    button {
                        class: "icon-button tiny",
                        title: "Answer this one again",
                        onclick: move |_| {
                            delete
                                .with_mut(|delete| {
                                    if position < delete.incoming.len() {
                                        delete.incoming.remove(position);
                                    }
                                })
                        },
                        Icon { name: "undo", class: "small" }
                    }
                }
            }
        }
    }
}

/// The attributes of surviving nodes that name a deleted one, which no reference makes visible.
fn attributes_section(
    plan: &uanedit::edit::DeletionPlan,
    named: &Named,
    data_type_choices: &[Vec<Choice>],
    mut delete: Signal<DeleteNode>,
    mut renaming: Signal<Option<AttributeUse>>,
) -> Element {
    let answered = delete.read().clone();
    if plan.attributes.is_empty() && answered.attributes.is_empty() {
        return rsx! {};
    }
    let data_type_choices = data_type_choices.to_vec();

    rsx! {
        section { class: "plan__section",
            header { class: "plan__section-head",
                Icon { name: "label", class: "small" }
                span { class: "type-label", "Attribute uses" }
                div { class: "plan__spacer" }
                span { class: "chip mono", "{plan.attributes.len()} unanswered" }
            }
            for (position , attribute) in plan.attributes.iter().enumerate() {
                div { key: "u{position}", class: "plan__row",
                    span { class: "plan__row-name",
                        {named.attributes.get(position).cloned().unwrap_or_default()}
                    }
                    span { class: "plan__row-actions",
                        if data_type_choices.get(position).is_some_and(|list| !list.is_empty()) {
                            select {
                                class: "field__select narrow",
                                value: "",
                                onchange: {
                                    let attribute = attribute.clone();
                                    move |event: FormEvent| {
                                        let Ok(node) = event.value().parse::<NodeId>() else {
                                            return;
                                        };
                                        let attribute = attribute.clone();
                                        delete
                                            .with_mut(|delete| {
                                                delete
                                                    .attributes
                                                    .push((attribute.clone(), AttributeResolution::Retarget { node }));
                                            });
                                    }
                                },
                                option { value: "", selected: true, "Choose a DataType…" }
                                for choice in data_type_choices.get(position).into_iter().flatten() {
                                    option { key: "{choice.value}", value: "{choice.value}", "{choice.label}" }
                                }
                            }
                        }
                        button {
                            class: "button text tiny",
                            onclick: {
                                let attribute = attribute.clone();
                                move |_| renaming.set(Some(attribute.clone()))
                            },
                            "Retarget…"
                        }
                        if attribute.slot.is_clearable() {
                            button {
                                class: "button tonal tiny",
                                title: "Leave the attribute out, which the model allows for this one",
                                onclick: {
                                    let attribute = attribute.clone();
                                    move |_| {
                                        delete
                                            .with_mut(|delete| {
                                                delete.attributes.push((attribute.clone(), AttributeResolution::Clear));
                                            })
                                    }
                                },
                                "Clear"
                            }
                        }
                    }
                }
            }
            for (position , resolved) in named.answered_attributes.iter().enumerate() {
                div { key: "a{position}", class: "plan__row answered",
                    Icon { name: "check", class: "small" }
                    span { class: "plan__row-name", {resolved.clone()} }
                    button {
                        class: "icon-button tiny",
                        title: "Answer this one again",
                        onclick: move |_| {
                            delete
                                .with_mut(|delete| {
                                    if position < delete.attributes.len() {
                                        delete.attributes.remove(position);
                                    }
                                })
                        },
                        Icon { name: "undo", class: "small" }
                    }
                }
            }
        }
    }
}

/// The picker an attribute retarget opens, scoped to DataTypes where the attribute names one.
fn attribute_picker(
    attribute: AttributeUse,
    mut delete: Signal<DeleteNode>,
    mut renaming: Signal<Option<AttributeUse>>,
) -> Element {
    let filter = match &attribute.slot {
        AttributeSlot::DataType | AttributeSlot::DefinitionField { .. } => {
            Some(PickFilter::new(|space, node_id| space.node_class(node_id) == Some(NodeClass::DataType)))
        }
        _ => None,
    };
    rsx! {
        NodePicker {
            title: "Point the attribute at another node".to_owned(),
            hint: "The attribute keeps its place and names this node instead.".to_owned(),
            filter,
            onpick: move |node_id: NodeId| {
                delete
                    .with_mut(|delete| {
                        delete
                            .attributes
                            .push((attribute.clone(), AttributeResolution::Retarget { node: node_id }));
                    });
                renaming.set(None);
            },
            oncancel: move |_| renaming.set(None),
        }
    }
}

fn children_section(
    plan: &uanedit::edit::DeletionPlan,
    named: &Named,
    mut delete: Signal<DeleteNode>,
    mut reparenting: Signal<Option<NodeId>>,
    reparent_choices: &[Vec<Choice>],
) -> Element {
    let answered = delete.read().clone();
    if plan.children.is_empty() && answered.children.is_empty() {
        return rsx! {};
    }
    let reparent_choices = reparent_choices.to_vec();

    rsx! {
        section { class: "plan__section",
            header { class: "plan__section-head",
                Icon { name: "account_tree", class: "small" }
                span { class: "type-label", "Owned children" }
                div { class: "plan__spacer" }
                span { class: "chip mono", "{plan.children.len()} unanswered" }
            }
            for (position , child) in plan.children.iter().enumerate() {
                div { key: "u{position}", class: "plan__row",
                    span { class: "plan__row-name",
                        {named.children.get(position).cloned().unwrap_or_default()}
                    }
                    span { class: "plan__row-actions",
                        button {
                            class: "button tonal tiny",
                            disabled: child.read_only,
                            title: match child.read_only {
                                true => "This child belongs to a nodeset this editor does not change",
                                false => "Delete the child too, resolving its own references in turn",
                            },
                            onclick: {
                                let node = child.node.clone();
                                move |_| {
                                    delete
                                        .with_mut(|delete| {
                                            delete.children.push((node.clone(), ChildResolution::Cascade));
                                        })
                                }
                            },
                            "Cascade"
                        }
                        button {
                            class: "button text tiny",
                            onclick: {
                                let node = child.node.clone();
                                move |_| reparenting.set(Some(node.clone()))
                            },
                            "Reparent…"
                        }
                        button {
                            class: "button text tiny",
                            title: match child.other_parents.is_empty() {
                                true => "Keep the child; it loses its only hierarchical parent",
                                false => "Keep the child; its other parents still root it",
                            },
                            onclick: {
                                let node = child.node.clone();
                                move |_| {
                                    delete
                                        .with_mut(|delete| {
                                            delete.children.push((node.clone(), ChildResolution::Detach));
                                        })
                                }
                            },
                            "Detach"
                        }
                    }
                }
            }
            for (position , resolved) in named.answered_children.iter().enumerate() {
                div { key: "a{position}", class: "plan__row answered",
                    Icon { name: "check", class: "small" }
                    span { class: "plan__row-name", {resolved.clone()} }
                    if let Some((_, ChildResolution::Reparent { parent, .. })) = answered.children.get(position).cloned() {
                        select {
                            class: "field__select narrow",
                            value: reparent_type(&answered, position),
                            onchange: {
                                let parent = parent.clone();
                                move |event: FormEvent| {
                                    let Ok(reference_type) = event.value().parse::<NodeId>() else {
                                        return;
                                    };
                                    let parent = parent.clone();
                                    delete
                                        .with_mut(|delete| {
                                            if let Some((_, resolution)) = delete.children.get_mut(position) {
                                                *resolution = ChildResolution::Reparent {
                                                    parent: parent.clone(),
                                                    reference_type,
                                                };
                                            }
                                        });
                                }
                            },
                            for choice in reparent_choices.get(position).into_iter().flatten() {
                                option {
                                    key: "{choice.value}",
                                    value: "{choice.value}",
                                    selected: choice.value == reparent_type(&answered, position),
                                    "{choice.label}"
                                }
                            }
                        }
                    }
                    button {
                        class: "icon-button tiny",
                        title: "Answer this one again",
                        onclick: move |_| {
                            delete
                                .with_mut(|delete| {
                                    if position < delete.children.len() {
                                        delete.children.remove(position);
                                    }
                                })
                        },
                        Icon { name: "undo", class: "small" }
                    }
                }
            }
        }
    }
}

/// Everything the plan names, said in DisplayNames rather than NodeIds.
#[derive(Clone, Default, PartialEq)]
struct Named {
    deleted: Vec<String>,
    incoming: Vec<String>,
    attributes: Vec<String>,
    children: Vec<String>,
    read_only: Vec<String>,
    answered_incoming: Vec<String>,
    answered_attributes: Vec<String>,
    answered_children: Vec<String>,
}

fn describe(
    session: &Session,
    plan: &uanedit::edit::DeletionPlan,
    delete: &DeleteNode,
) -> Named {
    let space = session.space();
    Named {
        deleted: plan
            .deleted
            .iter()
            .map(|node_id| label_of(space, node_id))
            .collect(),
        incoming: plan
            .incoming
            .iter()
            .map(|incoming| incoming_text(space, incoming))
            .collect(),
        attributes: plan
            .attributes
            .iter()
            .map(|attribute| attribute_text(space, attribute))
            .collect(),
        read_only: plan
            .read_only
            .iter()
            .map(|incoming| incoming_text(space, incoming))
            .collect(),
        children: plan
            .children
            .iter()
            .map(|child| {
                format!(
                    "{} · {} · {}",
                    label_of(space, &child.node),
                    label_of(space, &child.reference_type),
                    match child.other_parents.len() {
                        0 => "no other parent".to_owned(),
                        count => format!("{count} other parents"),
                    }
                )
            })
            .collect(),
        answered_incoming: delete
            .incoming
            .iter()
            .map(|(incoming, resolution)| {
                let what = match resolution {
                    IncomingResolution::Remove => "removed".to_owned(),
                    IncomingResolution::Retarget { node } => format!("retargeted to {}", label_of(space, node)),
                };
                format!("{} — {what}", incoming_text(space, incoming))
            })
            .collect(),
        answered_attributes: delete
            .attributes
            .iter()
            .map(|(attribute, resolution)| {
                let what = match resolution {
                    AttributeResolution::Clear => "left unset".to_owned(),
                    AttributeResolution::Retarget { node } => format!("set to {}", label_of(space, node)),
                };
                format!("{} — {what}", attribute_text(space, attribute))
            })
            .collect(),
        answered_children: delete
            .children
            .iter()
            .map(|(child, resolution)| {
                let what = match resolution {
                    ChildResolution::Cascade => "deleted too".to_owned(),
                    ChildResolution::Detach => "kept, detached".to_owned(),
                    ChildResolution::Reparent { parent, .. } => {
                        format!("moved under {}", label_of(space, parent))
                    }
                };
                format!("{} — {what}", label_of(space, child))
            })
            .collect(),
    }
}

fn incoming_text(
    space: &AddressSpace,
    incoming: &IncomingReference,
) -> String {
    let arrow = match incoming.is_forward {
        true => "→",
        false => "←",
    };
    format!(
        "{} {arrow} {} ({})",
        label_of(space, &incoming.holder),
        label_of(space, &incoming.names),
        label_of(space, &incoming.reference_type)
    )
}

fn attribute_text(
    space: &AddressSpace,
    attribute: &AttributeUse,
) -> String {
    format!(
        "{} · {} → {}",
        label_of(space, &attribute.holder),
        slot_text(&attribute.slot),
        label_of(space, &attribute.names)
    )
}

fn slot_text(slot: &AttributeSlot) -> String {
    match slot {
        AttributeSlot::DefinitionField { field } => format!("DataTypeDefinition field {field}"),
        AttributeSlot::RolePermission { position } => format!("RolePermissions grant {}", position + 1),
        other => other.field().to_string(),
    }
}

/// What the reparented child hangs under: the reference type it had, where the new parent takes it.
fn relink_type(
    handle: EditorHandle,
    parent: &NodeId,
    child: &NodeId,
    original: Option<NodeId>,
) -> NodeId {
    handle
        .with_space(|space| {
            let node_class = space.node_class(child).unwrap_or_default();
            let legal = query::legal_child_reference_types(space, parent, node_class);
            original
                .filter(|reference_type| legal.contains(reference_type))
                .or_else(|| legal.first().cloned())
        })
        .flatten()
        .unwrap_or(ids::HAS_COMPONENT)
}

fn original_reference_type(
    plan: &uanedit::edit::DeletionPlan,
    child: &NodeId,
) -> Option<NodeId> {
    plan.children
        .iter()
        .find(|owned| owned.node == *child)
        .map(|owned| owned.reference_type.clone())
}

fn reparent_type(
    delete: &DeleteNode,
    position: usize,
) -> String {
    match delete.children.get(position) {
        Some((_, ChildResolution::Reparent { reference_type, .. })) => reference_type.to_string(),
        _ => String::new(),
    }
}

fn removes_text(count: usize) -> String {
    match count {
        1 => "Removes one node".to_owned(),
        count => format!("Removes {count} nodes"),
    }
}

fn blockers_text(count: usize) -> String {
    match count {
        1 => "One reference cannot be resolved".to_owned(),
        count => format!("{count} references cannot be resolved"),
    }
}

/// The namespace index the nodeset creates its own nodes in, which is where a BrowseName defaults.
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

fn namespace_choices(space: &AddressSpace) -> Vec<Choice> {
    (0..space.namespaces().len())
        .filter_map(|index| NamespaceIndex::try_from(index).ok())
        .map(|index| {
            let uri = space.namespace_uri(index).unwrap_or_default();
            Choice {
                value: index.to_string(),
                label: format!("{index} · {}", short_namespace(uri)),
            }
        })
        .collect()
}

fn choices(
    space: &AddressSpace,
    nodes: Vec<NodeId>,
) -> Vec<Choice> {
    let mut choices: Vec<Choice> = nodes
        .into_iter()
        .map(|node_id| Choice {
            label: format!("{} · {node_id}", label_of(space, &node_id)),
            value: node_id.to_string(),
        })
        .collect();
    choices.sort_by(|left, right| left.label.cmp(&right.label));
    choices
}

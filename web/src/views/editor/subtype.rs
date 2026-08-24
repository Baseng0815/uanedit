//! The create-subtype wizard (workflow/type-aware-creation.md §2).
//!
//! An inherited declaration is read-only until the user overrides it, and an override may only do
//! what OPC 10000-3 §6.3.3.3 and §6.4.4.2 permit: narrow the type definition, tighten the
//! modelling rule, restrict a Variable's DataType, ValueRank and ArrayDimensions, restate the
//! names. Every choice list is the engine's own, so the wizard and the operation cannot disagree.

use dioxus::prelude::*;
use uanedit::attributes::array_dimensions::ArrayDimensions;
use uanedit::attributes::value_rank::ValueRank;
use uanedit::edit::{
    CreateSubtype,
    DeclarationOverride,
    MethodArguments,
    Refusal,
    TypeAttributes,
};
use uanedit::nodes::data_type::DataTypePurpose;
use uanedit::nodes::definition::DataTypeDefinition;
use uanedit::nodes::{
    Node,
    NodeClass,
};
use uanedit::rules::finding::Finding;
use uanedit::rules::plan::{
    self,
    InheritedDeclaration,
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
use uanedit::types::variant::Variant;
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
use crate::views::editor::arguments::{
    ArgumentRow,
    ArgumentsEditor,
    INPUT_ARGUMENTS,
    OUTPUT_ARGUMENTS,
    argument_data_types,
    argument_rows,
    arguments_value,
    inherited_arguments,
    rank_choices,
};
use crate::views::editor::counted;
use crate::views::editor::definition::{
    DefinitionEditor,
    field_data_types,
};
use crate::views::editor::diagnostics::{
    Explain,
    RefusalNotice,
};
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

const MAX_DIMENSIONS: u32 = 4;

const RULES: [ModellingRule; 5] = [
    ModellingRule::Mandatory,
    ModellingRule::Optional,
    ModellingRule::ExposesItsArray,
    ModellingRule::OptionalPlaceholder,
    ModellingRule::MandatoryPlaceholder,
];

fn rule_named(name: &str) -> Option<ModellingRule> {
    RULES.into_iter().find(|rule| rule.name() == name)
}

/// Changes one override in place, which every control in the override editor goes through.
fn edit_override(
    mut overrides: Signal<Vec<DeclarationOverride>>,
    position: usize,
    change: &dyn Fn(&mut DeclarationOverride),
) {
    overrides.with_mut(|overrides| {
        if let Some(over) = overrides.get_mut(position) {
            change(over);
        }
    });
}

/// One inherited declaration, with the choice lists an override of it may pick from.
#[derive(Clone, PartialEq)]
struct InheritedRow {
    path: BrowsePath,
    /// The declaration node on the supertype, which is where its inherited arguments are.
    declaration_node: NodeId,
    browse_name: QualifiedName,
    node_class: NodeClass,
    modelling_rule: Option<ModellingRule>,
    rule_choices: Vec<Choice>,
    declared_by: String,
    type_definition: Option<String>,
    type_choices: Vec<Choice>,
    data_type: Option<String>,
    data_type_choices: Vec<Choice>,
    value_rank: Option<ValueRank>,
    rank_choices: Vec<Choice>,
    array_dimensions: String,
    display_name: String,
    description: String,
    children: Vec<InheritedRow>,
}

/// The per-class attributes the new type carries, held apart so one signal set covers all four.
#[derive(Clone, PartialEq)]
struct Attributes {
    data_type: NodeId,
    value_rank: ValueRank,
    array_dimensions: ArrayDimensions,
    purpose: DataTypePurpose,
    definition: Option<DataTypeDefinition>,
    symmetric: bool,
    inverse_name: String,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            data_type: ids::BASE_DATA_TYPE,
            value_rank: ValueRank::SCALAR,
            array_dimensions: ArrayDimensions::default(),
            purpose: DataTypePurpose::Normal,
            definition: None,
            symmetric: false,
            inverse_name: String::new(),
        }
    }
}

#[component]
pub fn SubtypeDialog(supertype: Option<NodeId>) -> Element {
    let handle: EditorHandle = use_context();
    let dialogs: Dialogs = use_context();
    let navigate: Navigate = use_context();

    let mut supertype = use_signal(|| supertype.clone());
    let mut namespace = use_signal(move || own_namespace(handle));
    let mut name = use_signal(String::new);
    let mut display_name = use_signal(String::new);
    let mut is_abstract = use_signal(|| false);
    let mut attributes = use_signal(Attributes::default);
    let mut overrides = use_signal(Vec::<DeclarationOverride>::new);
    let mut picking = use_signal(|| false);
    let mut applied = use_signal(|| None::<Vec<Finding>>);
    let refused = use_signal(|| None::<Refusal>);

    let namespaces = use_memo(move || {
        let _ = *handle.revision.read();
        handle.with_space(namespace_choices).unwrap_or_default()
    });

    let node_class = use_memo(move || {
        let _ = *handle.revision.read();
        let Some(supertype) = supertype.read().clone() else {
            return NodeClass::ObjectType;
        };
        handle
            .with_space(|space| space.node_class(&supertype))
            .flatten()
            .unwrap_or(NodeClass::ObjectType)
    });

    let plan = use_memo(move || {
        let _ = *handle.revision.read();
        let supertype = supertype.read().clone()?;
        handle.with_space(|space| plan::subtype_plan(space, &supertype))
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

    let inherited_attributes = use_memo(move || {
        let _ = *handle.revision.read();
        let supertype = supertype.read().clone();
        handle
            .with_space(|space| inherited_shape(space, supertype.as_ref()))
            .unwrap_or_default()
    });

    // The supertype fixes what a VariableType's own attributes may narrow from.
    use_effect(move || {
        let shape = inherited_attributes.read().clone();
        attributes.with_mut(|attributes| {
            attributes.data_type = shape.data_type.clone();
            attributes.value_rank = shape.value_rank;
        });
    });

    let data_types = use_memo(move || {
        let _ = *handle.revision.read();
        let from = inherited_attributes.read().data_type.clone();
        handle
            .with_space(|space| node_choices(space, &query::legal_data_type_narrowings(space, &from), None))
            .unwrap_or_default()
    });

    let ranks = use_memo(move || {
        let from = inherited_attributes.read().value_rank;
        query::legal_value_rank_narrowings(from, MAX_DIMENSIONS)
            .into_iter()
            .map(|rank| Choice {
                value: rank.0.to_string(),
                label: format!("{rank}"),
                detail: None,
            })
            .collect::<Vec<_>>()
    });

    let definition_types = use_memo(move || {
        let _ = *handle.revision.read();
        handle.with_space(field_data_types).unwrap_or_default()
    });

    let argument_types = use_memo(move || {
        let _ = *handle.revision.read();
        handle.with_space(argument_data_types).unwrap_or_default()
    });

    let class_now = *node_class.read();
    let rows_now = rows.read().clone();
    let namespaces_now = namespaces.read().clone();
    let overrides_now = overrides.read().clone();
    let attributes_now = attributes.read().clone();
    let namespace_now = namespace.read().to_string();
    let supertype_label = supertype
        .read()
        .clone()
        .and_then(|node_id| handle.with_space(|space| candidate(space, &node_id)));
    let operation = build(
        supertype.read().clone(),
        class_now,
        QualifiedName::new(*namespace.read(), name.read().trim()),
        display_name.read().trim().to_owned(),
        *is_abstract.read(),
        &attributes_now,
        without_unchanged_arguments(handle, &rows_now, overrides_now.clone()),
    );
    let ready = operation.is_some();
    let confirm = operation.clone();
    let overridden = operation.clone();
    let refusal_now = refused.read().clone();
    let filter = PickFilter::new(move |space, node_id| query::may_be_supertype(space, class_now, node_id));

    if let Some(findings) = applied.read().clone() {
        return rsx! {
            AppliedReport {
                findings,
                onclose: move |_| {
                    applied.set(None);
                    dialogs.close();
                },
            }
        };
    }

    rsx! {
        Dialog {
            title: "Create a subtype".to_owned(),
            icon: "lan",
            subtitle: "Inherited declarations stay as they are until an override changes something the specification lets it change."
                .to_owned(),
            wide: true,
            onclose: move |_| dialogs.close(),
            div { class: "dialog__body",
                div { class: "field",
                    span { class: "field__label", "Supertype" }
                    div { class: "field__row",
                        button {
                            class: "button tonal tiny",
                            onclick: move |_| picking.set(true),
                            Icon { name: "search", class: "small" }
                            {
                                supertype_label
                                    .as_ref()
                                    .map_or_else(|| "Choose a type…".to_owned(), |named| named.label.clone())
                            }
                        }
                        span { class: "chip", "{class_now}" }
                        if let Some(named) = supertype_label.clone() {
                            span { class: "field__detail mono", {named.identifier} }
                        }
                    }
                    span { class: "field__hint",
                        "A HasSubtype reference from the supertype places the new type in the hierarchy; the NodeClass is the supertype's."
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
                            placeholder: "the name the new type browses under",
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
                label { class: "field__toggle",
                    input {
                        r#type: "checkbox",
                        checked: *is_abstract.read(),
                        onchange: move |event| is_abstract.set(event.checked()),
                    }
                    span { class: "field__toggle-label", "IsAbstract" }
                }
                {type_attributes(class_now, attributes, &data_types.read(), &ranks.read(), &definition_types.read(), &namespaces_now, *namespace.read(), name.read().trim())}
                if !rows_now.is_empty() {
                    section { class: "plan__section",
                        header { class: "plan__section-head",
                            Icon { name: "account_tree", class: "small" }
                            span { class: "type-label", "Inherited declarations" }
                            div { class: "plan__spacer" }
                            span { class: "chip mono", "{overrides_now.len()} overridden" }
                        }
                        {inherited_tree(&rows_now, 0, overrides, handle, &argument_types.read())}
                    }
                }
                if let Some(refusal) = refusal_now {
                    RefusalNotice {
                        refusal,
                        onoverride: move |reason: Option<String>| {
                            if let Some(operation) = overridden.clone() {
                                created(handle, dialogs, navigate, operation, Some(reason), refused, applied);
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
                    title: "Create the subtype and its overrides, as one undo entry",
                    onclick: move |_| {
                        if let Some(operation) = confirm.clone() {
                            created(handle, dialogs, navigate, operation, None, refused, applied);
                        }
                    },
                    "Create the subtype"
                }
            }
        }
        if *picking.read() {
            NodePicker {
                title: "Choose the supertype".to_owned(),
                hint: "The new type is a subtype of this one; subtyping relates types of the same NodeClass."
                    .to_owned(),
                filter: Some(filter),
                onpick: move |node_id: NodeId| {
                    supertype.set(Some(node_id));
                    overrides.set(Vec::new());
                    picking.set(false);
                },
                oncancel: move |_| picking.set(false),
            }
        }
    }
}

/// What the supertype fixes for a VariableType's own DataType and ValueRank.
#[derive(Clone, PartialEq)]
struct InheritedShape {
    data_type: NodeId,
    value_rank: ValueRank,
}

impl Default for InheritedShape {
    fn default() -> Self {
        Self {
            data_type: ids::BASE_DATA_TYPE,
            value_rank: ValueRank::ANY,
        }
    }
}

fn inherited_shape(
    space: &AddressSpace,
    supertype: Option<&NodeId>,
) -> InheritedShape {
    let Some(supertype) = supertype else {
        return InheritedShape::default();
    };
    InheritedShape {
        data_type: space.data_type(supertype).unwrap_or(ids::BASE_DATA_TYPE),
        value_rank: match space.node(supertype) {
            Some(Node::VariableType(variable_type)) => variable_type.value_rank,
            _ => ValueRank::ANY,
        },
    }
}

#[expect(clippy::too_many_arguments, reason = "the per-class attributes need every list at once")]
fn type_attributes(
    node_class: NodeClass,
    mut attributes: Signal<Attributes>,
    data_types: &[Choice],
    ranks: &[Choice],
    definition_types: &[Choice],
    namespaces: &[Choice],
    namespace: NamespaceIndex,
    name: &str,
) -> Element {
    let current = attributes.read().clone();
    let data_type = current.data_type.to_string();
    let rank = current.value_rank.0.to_string();
    let suggested = QualifiedName::new(namespace, name);

    rsx! {
        if node_class == NodeClass::VariableType {
            div { class: "field",
                span { class: "field__label", "DataType" }
                select {
                    class: "field__select",
                    value: data_type.clone(),
                    onchange: move |event| {
                        let Ok(picked) = event.value().parse::<NodeId>() else {
                            return;
                        };
                        attributes.with_mut(|attributes| attributes.data_type = picked);
                    },
                    for choice in data_types.iter() {
                        option {
                            key: "{choice.value}",
                            value: "{choice.value}",
                            selected: choice.value == data_type,
                            "{choice.label}"
                        }
                    }
                }
            }
            div { class: "field",
                span { class: "field__label", "ValueRank" }
                select {
                    class: "field__select",
                    value: rank.clone(),
                    onchange: move |event| {
                        let Ok(picked) = event.value().parse::<i32>() else {
                            return;
                        };
                        attributes.with_mut(|attributes| attributes.value_rank = ValueRank(picked));
                    },
                    for choice in ranks.iter() {
                        option {
                            key: "{choice.value}",
                            value: "{choice.value}",
                            selected: choice.value == rank,
                            "{choice.label}"
                        }
                    }
                }
            }
            div { class: "field",
                span { class: "field__label", "ArrayDimensions" }
                input {
                    class: "field__input mono",
                    value: "{current.array_dimensions}",
                    placeholder: "e.g. 0 or 3,4",
                    onchange: move |event| {
                        let Ok(parsed) = event.value().trim().parse::<ArrayDimensions>() else {
                            return;
                        };
                        attributes.with_mut(|attributes| attributes.array_dimensions = parsed.clone());
                    },
                }
            }
        }
        if node_class == NodeClass::DataType {
            div { class: "field",
                span { class: "field__label", "DataTypePurpose" }
                select {
                    class: "field__select",
                    value: "{current.purpose}",
                    onchange: move |event| {
                        let Ok(picked) = event.value().parse::<DataTypePurpose>() else {
                            return;
                        };
                        attributes.with_mut(|attributes| attributes.purpose = picked);
                    },
                    for choice in [DataTypePurpose::Normal, DataTypePurpose::ServicesOnly, DataTypePurpose::CodeGenerator] {
                        option { key: "{choice}", value: "{choice}", selected: choice == current.purpose, "{choice}" }
                    }
                }
            }
            DefinitionEditor {
                definition: current.definition.clone(),
                suggested_name: suggested,
                namespaces: namespaces.to_vec(),
                data_types: definition_types.to_vec(),
                ranks: ranks.to_vec(),
                nonce: 0,
                onchange: move |definition: Option<DataTypeDefinition>| {
                    attributes.with_mut(|attributes| attributes.definition = definition.clone());
                },
            }
        }
        if node_class == NodeClass::ReferenceType {
            label { class: "field__toggle",
                input {
                    r#type: "checkbox",
                    checked: current.symmetric,
                    onchange: move |event| {
                        let set = event.checked();
                        attributes.with_mut(|attributes| attributes.symmetric = set);
                    },
                }
                span { class: "field__toggle-label", "Symmetric" }
            }
            div { class: "field",
                span { class: "field__label", "InverseName" }
                input {
                    class: "field__input",
                    value: "{current.inverse_name}",
                    placeholder: "what the reference reads as backwards",
                    onchange: move |event| {
                        let text = event.value();
                        attributes.with_mut(|attributes| attributes.inverse_name = text.clone());
                    },
                }
            }
        }
    }
}

fn decorate(
    space: &AddressSpace,
    declarations: &[InheritedDeclaration],
) -> Vec<InheritedRow> {
    declarations
        .iter()
        .map(|declaration| InheritedRow {
            path: declaration.path.clone(),
            declaration_node: declaration.node_id.clone(),
            browse_name: declaration.browse_name.clone(),
            node_class: declaration.node_class,
            modelling_rule: declaration.modelling_rule,
            rule_choices: declaration
                .modelling_rule_choices
                .iter()
                .map(|rule| Choice {
                    value: rule.name().to_owned(),
                    label: rule.name().to_owned(),
                    detail: None,
                })
                .collect(),
            declared_by: label_of(space, &declaration.declared_by),
            type_definition: declaration
                .type_definition
                .as_ref()
                .map(ToString::to_string),
            type_choices: node_choices(
                space,
                &declaration.type_definition_choices,
                declaration.type_definition.as_ref(),
            ),
            data_type: declaration.data_type.as_ref().map(ToString::to_string),
            data_type_choices: node_choices(space, &declaration.data_type_choices, declaration.data_type.as_ref()),
            value_rank: declaration.value_rank,
            rank_choices: declaration
                .value_rank_choices
                .iter()
                .map(|rank| Choice {
                    value: rank.0.to_string(),
                    label: format!("{rank}"),
                    detail: None,
                })
                .collect(),
            array_dimensions: declaration
                .array_dimensions
                .clone()
                .unwrap_or_default()
                .to_string(),
            display_name: declaration
                .display_name
                .first()
                .map(|text| text.text.clone())
                .unwrap_or_default(),
            description: declaration
                .description
                .first()
                .map(|text| text.text.clone())
                .unwrap_or_default(),
            children: decorate(space, &declaration.children),
        })
        .collect()
}

fn inherited_tree(
    rows: &[InheritedRow],
    depth: usize,
    overrides: Signal<Vec<DeclarationOverride>>,
    handle: EditorHandle,
    argument_types: &[Choice],
) -> Element {
    rsx! {
        for row in rows.iter() {
            div { key: "{row.path}", class: "decl",
                {inherited_row(row, depth, overrides, handle, argument_types)}
                {inherited_tree(&row.children, depth + 1, overrides, handle, argument_types)}
            }
        }
    }
}

fn inherited_row(
    row: &InheritedRow,
    depth: usize,
    mut overrides: Signal<Vec<DeclarationOverride>>,
    handle: EditorHandle,
    argument_types: &[Choice],
) -> Element {
    let position = overrides
        .read()
        .iter()
        .position(|over| over.path == row.path);
    let add_path = row.path.clone();
    let rule = row
        .modelling_rule
        .map_or_else(|| "no rule".to_owned(), |rule| rule.name().to_owned());
    let overridable = !row.rule_choices.is_empty()
        || row.type_choices.len() > 1
        || row.data_type_choices.len() > 1
        || row.rank_choices.len() > 1
        || row.node_class == NodeClass::Method;

    rsx! {
        div {
            class: match position.is_some() {
                true => "decl__row on",
                false => "decl__row",
            },
            style: "padding-left: calc({depth} * var(--gap-lg))",
            Icon { name: class_icon(row.node_class), class: "tree__class small" }
            span { class: "decl__name", "{row.browse_name}" }
            span { class: "chip", {rule} }
            span { class: "decl__detail", "from {row.declared_by}" }
            div { class: "plan__spacer" }
            match position {
                Some(position) => rsx! {
                    button {
                        class: "button text tiny",
                        title: "Drop the override and inherit the declaration as it is",
                        onclick: move |_| {
                            overrides
                                .with_mut(|overrides| {
                                    if position < overrides.len() {
                                        overrides.remove(position);
                                    }
                                })
                        },
                        Icon { name: "undo", class: "small" }
                        "Inherit"
                    }
                },
                None => rsx! {
                    button {
                        class: "button tonal tiny",
                        disabled: !overridable,
                        title: match overridable {
                            true => "Restate the declaration on the new type and change what the specification lets an override change",
                            false => "Nothing about this declaration may be narrowed",
                        },
                        onclick: move |_| {
                            overrides
                                .with_mut(|overrides| {
                                    overrides
                                        .push(DeclarationOverride {
                                            path: add_path.clone(),
                                            ..DeclarationOverride::default()
                                        });
                                })
                        },
                        Icon { name: "edit", class: "small" }
                        "Override…"
                    }
                },
            }
        }
        if let Some(position) = position {
            {override_editor(row, position, depth, overrides, handle, argument_types)}
        }
    }
}

fn override_editor(
    row: &InheritedRow,
    position: usize,
    depth: usize,
    overrides: Signal<Vec<DeclarationOverride>>,
    handle: EditorHandle,
    argument_types: &[Choice],
) -> Element {
    let current = overrides.read().get(position).cloned().unwrap_or_default();
    let rule = current
        .modelling_rule
        .map(|rule| rule.name().to_owned())
        .or_else(|| row.modelling_rule.map(|rule| rule.name().to_owned()))
        .unwrap_or_default();
    let type_definition = current
        .type_definition
        .as_ref()
        .map(ToString::to_string)
        .or_else(|| row.type_definition.clone())
        .unwrap_or_default();
    let data_type = current
        .data_type
        .as_ref()
        .map(ToString::to_string)
        .or_else(|| row.data_type.clone())
        .unwrap_or_default();
    let value_rank = current
        .value_rank
        .or(row.value_rank)
        .map(|rank| rank.0.to_string())
        .unwrap_or_default();
    let dimensions = current
        .array_dimensions
        .clone()
        .map(|dimensions| dimensions.to_string())
        .unwrap_or_else(|| row.array_dimensions.clone());
    let display_name = current
        .display_name
        .first()
        .map(|text| text.text.clone())
        .unwrap_or_else(|| row.display_name.clone());
    let description = current
        .description
        .first()
        .map(|text| text.text.clone())
        .unwrap_or_else(|| row.description.clone());
    let is_variable = row.node_class == NodeClass::Variable;
    let is_method = row.node_class == NodeClass::Method;

    rsx! {
        div {
            class: "decl__override",
            style: "padding-left: calc({depth + 1} * var(--gap-lg))",
            if !row.rule_choices.is_empty() {
                div { class: "decl__control",
                    span { class: "type-label-small", "ModellingRule" }
                    select {
                        class: "field__select narrow",
                        value: rule.clone(),
                        onchange: move |event| {
                            let picked = rule_named(&event.value());
                            edit_override(overrides, position, &|over| over.modelling_rule = picked);
                        },
                        for choice in row.rule_choices.iter() {
                            option {
                                key: "{choice.value}",
                                value: "{choice.value}",
                                selected: choice.value == rule,
                                "{choice.label}"
                            }
                        }
                    }
                }
            }
            if row.type_choices.len() > 1 {
                div { class: "decl__control",
                    span { class: "type-label-small", "TypeDefinition" }
                    select {
                        class: "field__select",
                        value: type_definition.clone(),
                        onchange: move |event| {
                            let picked = event.value().parse::<NodeId>().ok();
                            edit_override(overrides, position, &|over| over.type_definition = picked.clone());
                        },
                        for choice in row.type_choices.iter() {
                            option {
                                key: "{choice.value}",
                                value: "{choice.value}",
                                selected: choice.value == type_definition,
                                "{choice.label}"
                            }
                        }
                    }
                }
            }
            if is_variable && row.data_type_choices.len() > 1 {
                div { class: "decl__control",
                    span { class: "type-label-small", "DataType" }
                    select {
                        class: "field__select",
                        value: data_type.clone(),
                        onchange: move |event| {
                            let picked = event.value().parse::<NodeId>().ok();
                            edit_override(overrides, position, &|over| over.data_type = picked.clone());
                        },
                        for choice in row.data_type_choices.iter() {
                            option {
                                key: "{choice.value}",
                                value: "{choice.value}",
                                selected: choice.value == data_type,
                                "{choice.label}"
                            }
                        }
                    }
                }
            }
            if is_variable && row.rank_choices.len() > 1 {
                div { class: "decl__control",
                    span { class: "type-label-small", "ValueRank" }
                    select {
                        class: "field__select narrow",
                        value: value_rank.clone(),
                        onchange: move |event| {
                            let picked = event.value().parse::<i32>().ok().map(ValueRank);
                            edit_override(overrides, position, &|over| over.value_rank = picked);
                        },
                        for choice in row.rank_choices.iter() {
                            option {
                                key: "{choice.value}",
                                value: "{choice.value}",
                                selected: choice.value == value_rank,
                                "{choice.label}"
                            }
                        }
                    }
                }
            }
            if is_variable {
                div { class: "decl__control",
                    span { class: "type-label-small", "ArrayDimensions" }
                    input {
                        class: "argument__dimensions mono",
                        value: dimensions.clone(),
                        placeholder: "dims",
                        onchange: move |event| {
                            let parsed = event.value().trim().parse::<ArrayDimensions>().ok();
                            edit_override(overrides, position, &|over| over.array_dimensions = parsed.clone());
                        },
                    }
                }
            }
            div { class: "decl__control",
                span { class: "type-label-small", "DisplayName" }
                input {
                    class: "field__input",
                    value: display_name.clone(),
                    onchange: move |event| {
                        let text = event.value();
                        edit_override(
                            overrides,
                            position,
                            &|over| {
                                over
                                    .display_name = match text.trim().is_empty() {
                                    true => Vec::new(),
                                    false => vec![LocalizedText::new(text.trim())],
                                };
                            },
                        );
                    },
                }
            }
            div { class: "decl__control",
                span { class: "type-label-small", "Description" }
                input {
                    class: "field__input",
                    value: description.clone(),
                    onchange: move |event| {
                        let text = event.value();
                        edit_override(
                            overrides,
                            position,
                            &|over| {
                                over
                                    .description = match text.trim().is_empty() {
                                    true => Vec::new(),
                                    false => vec![LocalizedText::new(text.trim())],
                                };
                            },
                        );
                    },
                }
            }
            if is_method {
                {method_signature(row, position, &current, overrides, handle, argument_types)}
            }
        }
    }
}

/// A Method override defines or extends its signature; §6.3.3.3 lets it only append arguments.
fn method_signature(
    row: &InheritedRow,
    position: usize,
    current: &DeclarationOverride,
    mut overrides: Signal<Vec<DeclarationOverride>>,
    handle: EditorHandle,
    argument_types: &[Choice],
) -> Element {
    let Some(arguments) = current.arguments.clone() else {
        let node_id = row.declaration_node.clone();
        return rsx! {
            div { class: "decl__control",
                span { class: "type-label-small", "Arguments" }
                button {
                    class: "button tonal tiny",
                    onclick: move |_| {
                        let input = inherited_arguments(handle, &node_id, INPUT_ARGUMENTS);
                        let output = inherited_arguments(handle, &node_id, OUTPUT_ARGUMENTS);
                        overrides
                            .with_mut(|overrides| {
                                if let Some(over) = overrides.get_mut(position) {
                                    over.arguments = Some(MethodArguments {
                                        input: Some(arguments_value(&input)),
                                        output: Some(arguments_value(&output)),
                                    });
                                }
                            });
                    },
                    Icon { name: "function", class: "small" }
                    "Define the signature"
                }
            }
        };
    };
    let inherited_input = inherited_arguments(handle, &row.declaration_node, INPUT_ARGUMENTS).len();
    let inherited_output = inherited_arguments(handle, &row.declaration_node, OUTPUT_ARGUMENTS).len();
    let input = argument_rows(arguments.input.as_ref()).unwrap_or_default();
    let output = argument_rows(arguments.output.as_ref()).unwrap_or_default();

    rsx! {
        ArgumentsEditor {
            title: INPUT_ARGUMENTS.to_owned(),
            rows: input,
            data_types: argument_types.to_vec(),
            ranks: rank_choices(),
            nonce: 0,
            locked: inherited_input,
            onchange: move |rows: Vec<ArgumentRow>| {
                let value = arguments_value(&rows);
                overrides
                    .with_mut(|overrides| {
                        if let Some(arguments) = overrides.get_mut(position).and_then(|over| over.arguments.as_mut()) {
                            arguments.input = Some(value.clone());
                        }
                    });
            },
        }
        ArgumentsEditor {
            title: OUTPUT_ARGUMENTS.to_owned(),
            rows: output,
            data_types: argument_types.to_vec(),
            ranks: rank_choices(),
            nonce: 0,
            locked: inherited_output,
            onchange: move |rows: Vec<ArgumentRow>| {
                let value = arguments_value(&rows);
                overrides
                    .with_mut(|overrides| {
                        if let Some(arguments) = overrides.get_mut(position).and_then(|over| over.arguments.as_mut()) {
                            arguments.output = Some(value.clone());
                        }
                    });
            },
        }
    }
}

/// Drops the argument list an override leaves exactly as it inherits it.
///
/// A subtype should not override a node unless it changes something (OPC 10000-3 §6.3.3.3), and
/// writing the untouched half of a signature is what makes the engine say UA0312.
fn without_unchanged_arguments(
    handle: EditorHandle,
    rows: &[InheritedRow],
    overrides: Vec<DeclarationOverride>,
) -> Vec<DeclarationOverride> {
    overrides
        .into_iter()
        .map(|mut over| {
            let (Some(arguments), Some(declaration)) = (over.arguments.clone(), declaration_node(rows, &over.path))
            else {
                return over;
            };
            let unchanged = |value: &Option<Variant>, browse_name| {
                value.as_ref() == Some(&arguments_value(&inherited_arguments(handle, &declaration, browse_name)))
            };
            let kept = MethodArguments {
                input: arguments
                    .input
                    .filter(|input| !unchanged(&Some(input.clone()), INPUT_ARGUMENTS)),
                output: arguments
                    .output
                    .filter(|output| !unchanged(&Some(output.clone()), OUTPUT_ARGUMENTS)),
            };
            over.arguments = (kept != MethodArguments::default()).then_some(kept);
            over
        })
        .collect()
}

fn declaration_node(
    rows: &[InheritedRow],
    path: &BrowsePath,
) -> Option<NodeId> {
    for row in rows {
        if row.path == *path {
            return Some(row.declaration_node.clone());
        }
        if let Some(found) = declaration_node(&row.children, path) {
            return Some(found);
        }
    }
    None
}

fn build(
    supertype: Option<NodeId>,
    node_class: NodeClass,
    browse_name: QualifiedName,
    display_name: String,
    is_abstract: bool,
    attributes: &Attributes,
    overrides: Vec<DeclarationOverride>,
) -> Option<Operation> {
    if browse_name.name.is_empty() {
        return None;
    }
    let type_attributes = match node_class {
        NodeClass::ObjectType => TypeAttributes::ObjectType,
        NodeClass::VariableType => TypeAttributes::VariableType {
            data_type: attributes.data_type.clone(),
            value_rank: attributes.value_rank,
            array_dimensions: attributes.array_dimensions.clone(),
        },
        NodeClass::DataType => TypeAttributes::DataType {
            definition: attributes.definition.clone(),
            purpose: attributes.purpose,
        },
        NodeClass::ReferenceType => TypeAttributes::ReferenceType {
            symmetric: attributes.symmetric,
            inverse_name: match attributes.inverse_name.trim().is_empty() {
                true => Vec::new(),
                false => vec![LocalizedText::new(attributes.inverse_name.trim())],
            },
        },
        _ => return None,
    };
    Some(Operation::CreateSubtype(Box::new(CreateSubtype {
        supertype: supertype?,
        browse_name,
        display_name: match display_name.is_empty() {
            true => Vec::new(),
            false => vec![LocalizedText::new(display_name)],
        },
        description: Vec::new(),
        is_abstract,
        attributes: type_attributes,
        overrides,
    })))
}

fn created(
    handle: EditorHandle,
    dialogs: Dialogs,
    navigate: Navigate,
    operation: Operation,
    override_reason: Option<Option<String>>,
    mut refused: Signal<Option<Refusal>>,
    mut applied: Signal<Option<Vec<Finding>>>,
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
            handle.say(Status::success(format!(
                "{} · {} · one undo entry",
                done.label,
                counted(done.created.len(), "node")
            )));
            if !done.overridden.is_empty() {
                dialogs.report(done.overridden);
                return;
            }
            match done.introduced.is_empty() {
                true => dialogs.close(),
                false => applied.set(Some(done.introduced)),
            }
        }
        Outcome::Unchanged | Outcome::Closed => dialogs.close(),
    }
}

/// What the subtype left behind: the warnings the specification tolerates but never silently
/// (guardrails.md §2), which is where a no-op override shows up as UA0312.
#[component]
fn AppliedReport(
    findings: Vec<Finding>,
    onclose: EventHandler<()>,
) -> Element {
    rsx! {
        Dialog {
            title: "The subtype was created".to_owned(),
            icon: "warning",
            subtitle: "It left findings the specification tolerates. They are in the Validation tab as introduced."
                .to_owned(),
            onclose: move |_| onclose.call(()),
            div { class: "dialog__body",
                for (position , finding) in findings.iter().enumerate() {
                    div { key: "{position}", class: "refusal__finding",
                        div { class: "refusal__finding-head",
                            span { class: "mono refusal__code", "{finding.code.id()}" }
                            span { class: "refusal__message", "{finding.message}" }
                        }
                        Explain { code: finding.code }
                    }
                }
            }
            footer { class: "dialog__actions",
                button { class: "button", onclick: move |_| onclose.call(()), "Close" }
            }
        }
    }
}

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

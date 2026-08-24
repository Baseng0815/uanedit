//! The DataTypeDefinition editor (features.md §2B).
//!
//! A nodeset writes structures, unions, enumerations and option sets through one `Definition`
//! element, telling them apart by the flags it carries and by whether its fields state a DataType
//! or a Value (OPC 10000-3 §8.47, §8.49). The editor picks the kind and then offers only the
//! attributes that kind gives a field, so a field carrying both a Value and a DataType is not
//! constructible here.

use dioxus::prelude::*;
use uanedit::attributes::array_dimensions::ArrayDimensions;
use uanedit::attributes::value_rank::ValueRank;
use uanedit::ids;
use uanedit::nodes::definition::{
    DataTypeDefinition,
    DataTypeField,
};
use uanedit::rules::query;
use uanedit::space::AddressSpace;
use uanedit::types::node_id::NodeId;
use uanedit::types::node_id_ref::NodeIdRef;
use uanedit::types::qualified_name::QualifiedName;

use crate::components::Icon;
use crate::views::editor::fields::{
    Choice,
    QualifiedNameField,
};
use crate::views::editor::forms::node_choices;

const MAX_DIMENSIONS: u32 = 4;

/// The four shapes a `Definition` element describes, which its flags and fields imply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefinitionKind {
    Structure,
    Union,
    Enumeration,
    OptionSet,
}

impl DefinitionKind {
    const ALL: [Self; 4] = [Self::Structure, Self::Union, Self::Enumeration, Self::OptionSet];

    pub fn name(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Union => "Union",
            Self::Enumeration => "Enumeration",
            Self::OptionSet => "OptionSet",
        }
    }

    /// True for the kinds whose fields name a member's type rather than a value.
    fn is_structured(self) -> bool {
        matches!(self, Self::Structure | Self::Union)
    }
}

/// Which of the four a definition already is.
pub fn kind_of(definition: &DataTypeDefinition) -> DefinitionKind {
    if definition.is_option_set {
        return DefinitionKind::OptionSet;
    }
    if definition.is_union {
        return DefinitionKind::Union;
    }
    match definition.is_enumeration() {
        true => DefinitionKind::Enumeration,
        false => DefinitionKind::Structure,
    }
}

/// The definition restated as the kind, with every field brought to what that kind gives it.
fn as_kind(
    definition: &DataTypeDefinition,
    kind: DefinitionKind,
) -> DataTypeDefinition {
    let mut next = definition.clone();
    next.is_union = kind == DefinitionKind::Union;
    next.is_option_set = kind == DefinitionKind::OptionSet;
    let mut assigned = 0;
    for field in &mut next.fields {
        if kind.is_structured() {
            field.value = -1;
            if kind == DefinitionKind::Union {
                field.is_optional = false;
            }
            continue;
        }
        field.data_type = NodeIdRef::Id(ids::BASE_DATA_TYPE);
        field.value_rank = ValueRank::SCALAR;
        field.array_dimensions = ArrayDimensions::default();
        field.max_string_length = 0;
        field.is_optional = false;
        field.allow_sub_types = false;
        if field.value < 0 {
            field.value = assigned;
        }
        assigned = field.value.saturating_add(1);
    }
    // The kind of a definition with no fields is not expressible: `is_enumeration` reads the
    // fields, so an empty one always reads back as a structure.
    if !kind.is_structured() && next.fields.is_empty() {
        next.fields.push(value_field(0));
    }
    next
}

/// Changes one field of the draft in place, which every control in a field's card goes through.
fn edit_field(
    mut draft: Signal<Option<DataTypeDefinition>>,
    position: usize,
    change: &dyn Fn(&mut DataTypeField),
) {
    draft.with_mut(|slot| {
        if let Some(field) = slot
            .as_mut()
            .and_then(|definition| definition.fields.get_mut(position))
        {
            change(field);
        }
    });
}

fn value_field(value: i32) -> DataTypeField {
    DataTypeField {
        value,
        ..DataTypeField::default()
    }
}

/// The value a new field of a value-carrying kind takes: one past the highest already used.
fn next_value(definition: &DataTypeDefinition) -> i32 {
    definition
        .fields
        .iter()
        .map(|field| field.value)
        .max()
        .unwrap_or(-1)
        .saturating_add(1)
}

/// The DataTypes a structure field may be of, which is every DataType the space holds.
pub fn field_data_types(space: &AddressSpace) -> Vec<Choice> {
    node_choices(space, &query::legal_data_type_narrowings(space, &ids::BASE_DATA_TYPE), None)
}

pub fn rank_choices() -> Vec<Choice> {
    query::legal_value_rank_narrowings(ValueRank::ANY, MAX_DIMENSIONS)
        .into_iter()
        .map(|rank| Choice {
            value: rank.0.to_string(),
            label: format!("{rank}"),
            detail: None,
        })
        .collect()
}

/// The `Definition` of a DataType, edited as the kind it is.
#[component]
pub fn DefinitionEditor(
    definition: Option<DataTypeDefinition>,
    suggested_name: QualifiedName,
    namespaces: Vec<Choice>,
    data_types: Vec<Choice>,
    ranks: Vec<Choice>,
    nonce: u64,
    onchange: EventHandler<Option<DataTypeDefinition>>,
    #[props(default)] disabled: bool,
    error: Option<String>,
    hint: Option<String>,
) -> Element {
    let mut draft = use_signal(|| definition.clone());
    let mut kind = use_signal(|| {
        definition
            .as_ref()
            .map_or(DefinitionKind::Structure, kind_of)
    });
    use_effect(use_reactive!(|(definition, nonce)| {
        let _ = nonce;
        if let Some(definition) = &definition {
            kind.set(kind_of(definition));
        }
        draft.set(definition);
    }));

    let commit = move || onchange.call(draft.peek().clone());
    let Some(current) = draft() else {
        return rsx! {
            div { class: "field",
                span { class: "field__label", "DataTypeDefinition" }
                div { class: "field__row",
                    span { class: "field__static", "none" }
                    if !disabled {
                        button {
                            class: "button tonal tiny",
                            onclick: move |_| {
                                draft
                                    .set(
                                        Some(DataTypeDefinition {
                                            name: suggested_name.clone(),
                                            ..DataTypeDefinition::default()
                                        }),
                                    );
                                kind.set(DefinitionKind::Structure);
                                commit();
                            },
                            Icon { name: "add", class: "small" }
                            "Define this DataType"
                        }
                    }
                }
                if let Some(error) = error {
                    span { class: "field__error",
                        Icon { name: "error", class: "small" }
                        {error}
                    }
                }
            }
        };
    };

    let now = *kind.read();
    let structure_type = current.structure_type();
    let name_commit = {
        EventHandler::new(move |(namespace_index, name): (u16, String)| {
            draft.with_mut(|slot| {
                if let Some(definition) = slot {
                    definition.name = QualifiedName::new(namespace_index, name);
                }
            });
            commit();
        })
    };

    rsx! {
        div { class: "field",
            span { class: "field__label", "DataTypeDefinition" }
            div { class: "definition",
                div { class: "definition__head",
                    // StructureType describes a structured DataType only (OPC 10000-3 §8.49).
                    if now.is_structured() {
                        span { class: "chip", title: "The layout the fields imply", "{structure_type}" }
                    }
                    span { class: "chip mono", "{current.fields.len()} fields" }
                    if let Some(base_type) = current.base_type.clone() {
                        span { class: "chip mono", title: "BaseType is obsolete in the schema; it is kept as written", "{base_type}" }
                    }
                    div { class: "definition__spacer" }
                    select {
                        class: "field__select narrow",
                        value: now.name(),
                        disabled,
                        onchange: move |event| {
                            let Some(picked) = DefinitionKind::ALL
                                .into_iter()
                                .find(|candidate| candidate.name() == event.value())
                            else {
                                return;
                            };
                            kind.set(picked);
                            draft
                                .with_mut(|slot| {
                                    if let Some(definition) = slot {
                                        *definition = as_kind(definition, picked);
                                    }
                                });
                            commit();
                        },
                        for candidate in DefinitionKind::ALL {
                            option {
                                key: "{candidate.name()}",
                                value: candidate.name(),
                                selected: candidate == now,
                                "{candidate.name()}"
                            }
                        }
                    }
                    if !disabled {
                        button {
                            class: "icon-button tiny",
                            title: "Remove the definition",
                            onclick: move |_| {
                                draft.set(None);
                                commit();
                            },
                            Icon { name: "delete", class: "small" }
                        }
                    }
                }
                QualifiedNameField {
                    label: "Name",
                    namespace: current.name.namespace_index,
                    name: current.name.name.clone(),
                    namespaces,
                    nonce,
                    disabled,
                    oncommit: name_commit,
                }
                div { class: "definition__fields-list",
                    for (position , field) in current.fields.iter().enumerate() {
                        {field_card(position, field, now, &data_types, &ranks, disabled, draft, onchange)}
                    }
                }
                if !disabled {
                    button {
                        class: "button tonal tiny",
                        onclick: move |_| {
                            draft
                                .with_mut(|slot| {
                                    let Some(definition) = slot else {
                                        return;
                                    };
                                    let field = match now.is_structured() {
                                        true => DataTypeField::default(),
                                        false => value_field(next_value(definition)),
                                    };
                                    definition.fields.push(field);
                                });
                            commit();
                        },
                        Icon { name: "add", class: "small" }
                        {match now.is_structured() {
                            true => "Add a field",
                            false => "Add a value",
                        }}
                    }
                }
            }
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
}

#[expect(clippy::too_many_arguments, reason = "one field of the definition needs the whole row's state")]
fn field_card(
    position: usize,
    field: &DataTypeField,
    kind: DefinitionKind,
    data_types: &[Choice],
    ranks: &[Choice],
    disabled: bool,
    mut draft: Signal<Option<DataTypeDefinition>>,
    onchange: EventHandler<Option<DataTypeDefinition>>,
) -> Element {
    let commit = move || onchange.call(draft.peek().clone());
    let data_type = field.data_type.to_string();
    let rank = field.value_rank.0.to_string();
    let description = field.description.first().cloned().unwrap_or_default();
    let last = draft
        .peek()
        .as_ref()
        .map_or(0, |definition| definition.fields.len().saturating_sub(1));

    rsx! {
        div { key: "{position}", class: "deffield",
            div { class: "deffield__bar",
                span { class: "argument__index mono", "{position}" }
                input {
                    class: "argument__name",
                    value: "{field.name}",
                    placeholder: "field name",
                    disabled,
                    oninput: move |event| {
                        let name = event.value();
                        edit_field(draft, position, &|field| field.name = name.clone());
                    },
                    onchange: move |_| commit(),
                }
                if kind.is_structured() {
                    select {
                        class: "field__select",
                        value: data_type.clone(),
                        disabled,
                        onchange: move |event| {
                            let Ok(picked) = event.value().parse::<NodeId>() else {
                                return;
                            };
                            edit_field(draft, position, &|field| field.data_type = NodeIdRef::Id(picked.clone()));
                            commit();
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
                    select {
                        class: "field__select narrow",
                        value: rank.clone(),
                        disabled,
                        onchange: move |event| {
                            let Ok(picked) = event.value().parse::<i32>() else {
                                return;
                            };
                            edit_field(draft, position, &|field| field.value_rank = ValueRank(picked));
                            commit();
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
                    input {
                        class: "argument__dimensions mono",
                        value: "{field.array_dimensions}",
                        placeholder: "dims",
                        title: "ArrayDimensions, comma-separated",
                        disabled,
                        onchange: move |event| {
                            let Ok(parsed) = event.value().trim().parse::<ArrayDimensions>() else {
                                return;
                            };
                            edit_field(draft, position, &|field| field.array_dimensions = parsed.clone());
                            commit();
                        },
                    }
                } else {
                    input {
                        class: "argument__dimensions mono",
                        value: "{field.value}",
                        r#type: "number",
                        title: match kind {
                            DefinitionKind::OptionSet => "The bit this field occupies",
                            _ => "The enumeration value",
                        },
                        disabled,
                        onchange: move |event| {
                            let Ok(parsed) = event.value().trim().parse::<i32>() else {
                                return;
                            };
                            edit_field(draft, position, &|field| field.value = parsed);
                            commit();
                        },
                    }
                    span { class: "type-label-small",
                        {match kind {
                            DefinitionKind::OptionSet => "bit",
                            _ => "value",
                        }}
                    }
                }
                if !disabled {
                    button {
                        class: "icon-button tiny",
                        title: "Move up",
                        disabled: position == 0,
                        onclick: move |_| {
                            draft
                                .with_mut(|slot| {
                                    if let Some(definition) = slot {
                                        definition.fields.swap(position, position.saturating_sub(1));
                                    }
                                });
                            commit();
                        },
                        Icon { name: "arrow_upward", class: "small" }
                    }
                    button {
                        class: "icon-button tiny",
                        title: "Move down",
                        disabled: position >= last,
                        onclick: move |_| {
                            draft
                                .with_mut(|slot| {
                                    if let Some(definition) = slot
                                        && position + 1 < definition.fields.len()
                                    {
                                        definition.fields.swap(position, position + 1);
                                    }
                                });
                            commit();
                        },
                        Icon { name: "arrow_downward", class: "small" }
                    }
                    button {
                        class: "icon-button tiny",
                        title: "Remove this field",
                        onclick: move |_| {
                            draft
                                .with_mut(|slot| {
                                    if let Some(definition) = slot
                                        && position < definition.fields.len()
                                    {
                                        definition.fields.remove(position);
                                    }
                                });
                            commit();
                        },
                        Icon { name: "close", class: "small" }
                    }
                }
            }
            div { class: "deffield__bar",
                if kind == DefinitionKind::Structure {
                    label { class: "flags__flag",
                        input {
                            r#type: "checkbox",
                            checked: field.is_optional,
                            disabled,
                            onchange: move |event| {
                                let set = event.checked();
                                edit_field(draft, position, &|field| field.is_optional = set);
                                commit();
                            },
                        }
                        span { "IsOptional" }
                    }
                }
                if kind.is_structured() {
                    label { class: "flags__flag",
                        input {
                            r#type: "checkbox",
                            checked: field.allow_sub_types,
                            disabled,
                            onchange: move |event| {
                                let set = event.checked();
                                edit_field(draft, position, &|field| field.allow_sub_types = set);
                                commit();
                            },
                        }
                        span { "AllowSubTypes" }
                    }
                    span { class: "type-label-small", "MaxStringLength" }
                    input {
                        class: "argument__dimensions mono",
                        value: "{field.max_string_length}",
                        r#type: "number",
                        title: "0 leaves the length unconstrained",
                        disabled,
                        onchange: move |event| {
                            let Ok(parsed) = event.value().trim().parse::<u32>() else {
                                return;
                            };
                            edit_field(draft, position, &|field| field.max_string_length = parsed);
                            commit();
                        },
                    }
                }
                span { class: "argument__label type-label-small", "Description" }
                input {
                    class: "argument__description",
                    value: "{description.text}",
                    placeholder: "what the field means",
                    disabled,
                    oninput: move |event| {
                        let text = event.value();
                        edit_field(
            draft,
            position,
                            &|field| {
                                let mut entry = field.description.first().cloned().unwrap_or_default();
                                entry.text = text.clone();
                                field.description = match text.is_empty() {
                                    true => Vec::new(),
                                    false => vec![entry],
                                };
                            },
                        );
                    },
                    onchange: move |_| commit(),
                }
            }
        }
    }
}

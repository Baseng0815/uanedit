//! InputArguments and OutputArguments as rows rather than as XML.
//!
//! An Argument is an ExtensionObject whose body is the `<Argument>` element the file holds
//! (OPC 10000-3 §8.6), so an edit patches that element in place: everything this editor does not
//! model — a newer schema's field, an attribute, the whitespace — survives it.

use dioxus::prelude::*;
use uanedit::attributes::array_dimensions::ArrayDimensions;
use uanedit::attributes::value_rank::ValueRank;
use uanedit::edit::{
    CreateInstance,
    FieldValue,
    InstanceAttributes,
};
use uanedit::emit::value_xml;
use uanedit::nodes::Node;
use uanedit::rules::query;
use uanedit::space::AddressSpace;
use uanedit::types::built_in::BuiltInType;
use uanedit::types::extension_object::ExtensionObject;
use uanedit::types::localized_text::LocalizedText;
use uanedit::types::node_id::NodeId;
use uanedit::types::qualified_name::QualifiedName;
use uanedit::types::variant::{
    Variant,
    VariantArray,
};
use uanedit::types::xml::{
    XmlElement,
    XmlNode,
};
use uanedit::{
    Operation,
    ids,
};

use crate::components::Icon;
use crate::session::EditorHandle;
use crate::views::editor::fields::{
    Choice,
    ReadOnlyBlock,
};
use crate::views::editor::forms::{
    Editing,
    node_choices,
    section,
};

/// The BrowseNames of the two Properties a Method's signature lives in (OPC 10000-3 §5.7.1).
pub const INPUT_ARGUMENTS: &str = "InputArguments";
pub const OUTPUT_ARGUMENTS: &str = "OutputArguments";

/// Argument's Default XML encoding, which is what a `<TypeId>` names (OPC 10000-5 §12.15).
const ARGUMENT_ENCODING: u32 = 297;

const MAX_DIMENSIONS: u32 = 4;

/// The order OPC 10000-3 §8.6 lists an Argument's fields in, which a new element follows.
const ARGUMENT_ORDER: [&str; 5] = ["Name", "DataType", "ValueRank", "ArrayDimensions", "Description"];
const TEXT_ORDER: [&str; 2] = ["Locale", "Text"];
const IDENTIFIER_ORDER: [&str; 1] = ["Identifier"];

/// Where the value writer puts a fresh `<Argument>`, so its fields line up with a file's own.
const ARGUMENT_INDENT: &str = "\n            ";
const FIELD_INDENT: &str = "\n              ";

fn fresh_argument() -> XmlElement {
    let mut element = XmlElement::new("Argument");
    element.push(XmlNode::Text(ARGUMENT_INDENT.to_owned()));
    element
}

/// One Argument, with the element it came from so an edit only touches what it changes.
#[derive(Clone, Debug, PartialEq)]
pub struct ArgumentRow {
    pub name: String,
    /// As the file spells it; the picker works in the address space's NodeIds and converts.
    pub data_type: NodeId,
    pub value_rank: ValueRank,
    pub array_dimensions: ArrayDimensions,
    pub description: Option<LocalizedText>,
    source: Option<(NodeId, XmlElement)>,
}

impl ArgumentRow {
    fn fresh() -> Self {
        Self {
            name: String::new(),
            data_type: ids::BASE_DATA_TYPE,
            value_rank: ValueRank::SCALAR,
            array_dimensions: ArrayDimensions::default(),
            description: None,
            source: None,
        }
    }
}

/// The Arguments a Value holds, or nothing when it is not an Argument array this editor may write.
pub fn argument_rows(value: Option<&Variant>) -> Option<Vec<ArgumentRow>> {
    match value {
        None | Some(Variant::Null) => Some(Vec::new()),
        Some(Variant::Array(array)) if array.element_type == BuiltInType::ExtensionObject => {
            array.values.iter().map(argument_row).collect()
        }
        Some(_) => None,
    }
}

fn argument_row(value: &Variant) -> Option<ArgumentRow> {
    let Variant::ExtensionObject(object) = value else {
        return None;
    };
    let body = object.body.as_ref()?;
    if body.local_name() != "Argument" {
        return None;
    }
    Some(ArgumentRow {
        name: text_of(body, "Name"),
        data_type: body
            .child("DataType")
            .map(|child| text_of(child, "Identifier"))
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or_default(),
        value_rank: body
            .child("ValueRank")
            .and_then(|child| child.text().trim().parse::<i32>().ok())
            .map_or(ValueRank::SCALAR, ValueRank),
        array_dimensions: ArrayDimensions(
            body.child("ArrayDimensions")
                .map(read_dimensions)
                .unwrap_or_default(),
        ),
        description: body.child("Description").map(|child| LocalizedText {
            locale: child.child("Locale").map(XmlElement::text),
            text: text_of(child, "Text"),
        }),
        source: Some((object.type_id.clone(), body.clone())),
    })
}

/// The rows as the `Value` of an arguments Property.
pub fn arguments_value(rows: &[ArgumentRow]) -> Variant {
    Variant::Array(VariantArray::new(BuiltInType::ExtensionObject, rows.iter().map(argument_variant).collect()))
}

fn argument_variant(row: &ArgumentRow) -> Variant {
    let (type_id, mut body) = row
        .source
        .clone()
        .unwrap_or_else(|| (NodeId::numeric(0, ARGUMENT_ENCODING), fresh_argument()));
    set_text(&mut body, "Name", &row.name, &ARGUMENT_ORDER);
    let slot = child_slot(&mut body, "DataType", &ARGUMENT_ORDER, true);
    if let XmlNode::Element(child) = &mut body.children[slot] {
        set_text(child, "Identifier", &row.data_type.to_string(), &IDENTIFIER_ORDER);
    }
    set_text(&mut body, "ValueRank", &row.value_rank.0.to_string(), &ARGUMENT_ORDER);
    set_dimensions(&mut body, &row.array_dimensions);
    set_description(&mut body, row.description.as_ref());
    Variant::ExtensionObject(ExtensionObject::new(type_id, body))
}

fn text_of(
    element: &XmlElement,
    local_name: &str,
) -> String {
    element
        .child(local_name)
        .map(XmlElement::text)
        .unwrap_or_default()
}

fn read_dimensions(element: &XmlElement) -> Vec<u32> {
    element
        .elements()
        .filter_map(|entry| entry.text().trim().parse().ok())
        .collect()
}

fn child_index(
    element: &XmlElement,
    local_name: &str,
) -> Option<usize> {
    element
        .children
        .iter()
        .position(|child| matches!(child, XmlNode::Element(found) if found.local_name() == local_name))
}

/// The index of the named child, inserting it in the order the specification lists first.
///
/// A container is seeded with the indentation it sits at, so its own children land a level in and
/// its closing tag on a line of its own.
fn child_slot(
    element: &mut XmlElement,
    local_name: &str,
    order: &[&str],
    container: bool,
) -> usize {
    if let Some(index) = child_index(element, local_name) {
        return index;
    }
    let rank = rank_of(order, local_name);
    let before = element.children.iter().position(|child| match child {
        XmlNode::Element(found) => rank_of(order, found.local_name()) > rank,
        _ => false,
    });
    let pad = child_indent(element);
    let mut created = XmlElement::new(local_name);
    if container {
        created.push(XmlNode::Text(pad.clone()));
    }
    let fresh = XmlNode::Element(created);
    match before {
        Some(index) => {
            element.children.insert(index, XmlNode::Text(pad));
            element.children.insert(index, fresh);
            index
        }
        None => {
            let at = tail_index(element);
            element.children.insert(at, fresh);
            element.children.insert(at, XmlNode::Text(pad));
            at + 1
        }
    }
}

fn rank_of(
    order: &[&str],
    local_name: &str,
) -> usize {
    order
        .iter()
        .position(|name| *name == local_name)
        .unwrap_or(order.len())
}

/// The whitespace the element puts before a child, so an inserted one lines up with the rest.
///
/// With no child to copy, the element's own indentation plus one level, which is what a container
/// seeded with its closing indentation offers.
fn child_indent(element: &XmlElement) -> String {
    let first = element
        .children
        .iter()
        .position(|child| matches!(child, XmlNode::Element(_)));
    if let Some(index) = first {
        if let Some(XmlNode::Text(text)) = index
            .checked_sub(1)
            .and_then(|before| element.children.get(before))
            && is_indent(text)
        {
            return text.clone();
        }
        return FIELD_INDENT.to_owned();
    }
    match element.children.last() {
        Some(XmlNode::Text(text)) if is_indent(text) => format!("{text}  "),
        _ => FIELD_INDENT.to_owned(),
    }
}

fn is_indent(text: &str) -> bool {
    text.trim().is_empty() && text.contains('\n')
}

/// Where a child goes when nothing follows it: before the whitespace that closes the element.
fn tail_index(element: &XmlElement) -> usize {
    match element.children.last() {
        Some(XmlNode::Text(text)) if text.trim().is_empty() => element.children.len() - 1,
        _ => element.children.len(),
    }
}

fn set_text(
    element: &mut XmlElement,
    local_name: &str,
    text: &str,
    order: &[&str],
) {
    let index = child_slot(element, local_name, order, false);
    let XmlNode::Element(child) = &mut element.children[index] else {
        return;
    };
    if child.text() == text {
        return;
    }
    child.children.clear();
    if !text.is_empty() {
        child.push(XmlNode::Text(text.to_owned()));
    }
}

fn set_dimensions(
    element: &mut XmlElement,
    dimensions: &ArrayDimensions,
) {
    let pad = child_indent(element);
    let inner = format!("{pad}  ");
    let index = child_slot(element, "ArrayDimensions", &ARGUMENT_ORDER, false);
    let XmlNode::Element(child) = &mut element.children[index] else {
        return;
    };
    if read_dimensions(child) == dimensions.0 && !(dimensions.0.is_empty() && !child.children.is_empty()) {
        return;
    }
    child.children.clear();
    for length in &dimensions.0 {
        child.push(XmlNode::Text(inner.clone()));
        let mut entry = XmlElement::new("UInt32");
        entry.push(XmlNode::Text(length.to_string()));
        child.push(entry);
    }
    if !dimensions.0.is_empty() {
        child.push(XmlNode::Text(pad));
    }
}

fn set_description(
    element: &mut XmlElement,
    description: Option<&LocalizedText>,
) {
    let Some(description) = description else {
        remove_child(element, "Description");
        return;
    };
    let index = child_slot(element, "Description", &ARGUMENT_ORDER, true);
    let XmlNode::Element(child) = &mut element.children[index] else {
        return;
    };
    match &description.locale {
        Some(locale) => set_text(child, "Locale", locale, &TEXT_ORDER),
        None => remove_child(child, "Locale"),
    }
    set_text(child, "Text", &description.text, &TEXT_ORDER);
}

fn remove_child(
    element: &mut XmlElement,
    local_name: &str,
) {
    let Some(index) = element
        .children
        .iter()
        .position(|child| matches!(child, XmlNode::Element(found) if found.local_name() == local_name))
    else {
        return;
    };
    element.children.remove(index);
    if index > 0 && matches!(element.children.get(index - 1), Some(XmlNode::Text(text)) if text.trim().is_empty()) {
        element.children.remove(index - 1);
    }
}

/// The DataTypes an argument may be of, which is every DataType the space holds.
pub fn argument_data_types(space: &AddressSpace) -> Vec<Choice> {
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

/// The arguments section of a Method's inspector form.
pub fn method_arguments(
    editing: &Editing,
    space: &AddressSpace,
    node: &Node,
) -> Element {
    if node.node_class() != uanedit::nodes::NodeClass::Method {
        return rsx! {};
    }
    let data_types = argument_data_types(space);
    let ranks = rank_choices();
    let body = rsx! {
        {arguments_property(editing, space, INPUT_ARGUMENTS, &data_types, &ranks)}
        {arguments_property(editing, space, OUTPUT_ARGUMENTS, &data_types, &ranks)}
    };
    section("Arguments", "function", body)
}

fn arguments_property(
    editing: &Editing,
    space: &AddressSpace,
    browse_name: &'static str,
    data_types: &[Choice],
    ranks: &[Choice],
) -> Element {
    let property = space
        .children_named(&editing.node, &QualifiedName::new(0, browse_name))
        .into_iter()
        .next();
    let Some(property) = property else {
        return missing_property(editing, browse_name);
    };
    let value = match space.node(&property) {
        Some(Node::Variable(variable)) => variable.value.clone(),
        _ => None,
    };
    let Some(rows) = argument_rows(value.as_ref()) else {
        return rsx! {
            ReadOnlyBlock {
                label: browse_name.to_owned(),
                text: value.as_ref().map(value_xml).unwrap_or_default(),
                note: "Not an Argument array this editor knows how to write, so it is shown as it stands.",
            }
        };
    };
    let read_only = editing.read_only || space.is_read_only(&property);
    let editing = editing.clone();
    let shown = value.as_ref().map(value_xml);
    let commit = {
        let editing = editing.clone();
        let property = property.clone();
        EventHandler::new(move |rows: Vec<ArgumentRow>| {
            write_arguments(&editing, browse_name, &property, &rows);
        })
    };

    rsx! {
        div { class: "field",
            ArgumentsEditor {
                title: browse_name.to_owned(),
                rows,
                data_types: data_types.to_vec(),
                ranks: ranks.to_vec(),
                disabled: read_only,
                nonce: editing.nonce(),
                error: editing.error_for(browse_name),
                onchange: commit,
            }
            if let Some(shown) = shown {
                details { class: "arguments__xml",
                    summary { class: "explain__summary type-label-small",
                        Icon { name: "code", class: "small" }
                        "The Value as this editor writes it"
                    }
                    pre { class: "field__block mono", {shown} }
                }
            }
        }
    }
}

/// Writes the rows, keeping ArrayDimensions on the Property at the count the file states.
fn write_arguments(
    editing: &Editing,
    field: &'static str,
    property: &NodeId,
    rows: &[ArgumentRow],
) {
    editing.set_on(field, property.clone(), FieldValue::Value(Some(arguments_value(rows))));
    let count = u32::try_from(rows.len()).unwrap_or(u32::MAX);
    let stated = editing
        .handle
        .with_space(|space| match space.node(property) {
            Some(Node::Variable(variable)) => Some(variable.array_dimensions.clone()),
            _ => None,
        })
        .flatten();
    if stated.is_some_and(|dimensions| dimensions.0 == vec![count]) {
        return;
    }
    editing.set_on(field, property.clone(), FieldValue::ArrayDimensions(ArrayDimensions(vec![count])));
}

/// A Method whose signature the file does not state yet: the Property is what carries it.
fn missing_property(
    editing: &Editing,
    browse_name: &'static str,
) -> Element {
    if editing.read_only {
        return rsx! {
            div { class: "field",
                span { class: "field__label", {browse_name.to_owned()} }
                span { class: "field__static", "none" }
            }
        };
    }
    let editing = editing.clone();

    rsx! {
        div { class: "field",
            span { class: "field__label", {browse_name.to_owned()} }
            div { class: "field__row",
                span { class: "field__static", "The Method states no such Property." }
                button {
                    class: "button tonal tiny",
                    onclick: move |_| add_arguments_property(&editing, browse_name),
                    Icon { name: "add", class: "small" }
                    "Add it"
                }
            }
        }
    }
}

fn add_arguments_property(
    editing: &Editing,
    browse_name: &'static str,
) {
    editing.perform_on(
        browse_name,
        Operation::CreateInstance(CreateInstance {
            parent: editing.node.clone(),
            reference_type: ids::HAS_PROPERTY,
            browse_name: QualifiedName::new(0, browse_name),
            display_name: vec![LocalizedText::new(browse_name)],
            description: Vec::new(),
            modelling_rule: None,
            attributes: InstanceAttributes::Variable {
                type_definition: Some(ids::PROPERTY_TYPE),
                data_type: ids::ARGUMENT,
                value_rank: ValueRank::ONE_DIMENSION,
                array_dimensions: ArrayDimensions(vec![0]),
            },
        }),
    );
}

/// The rows of one arguments Property, edited as a list rather than as XML.
///
/// `locked` is how many leading rows an override inherits: OPC 10000-3 §6.3.3.3 lets a subtype
/// append arguments to a Method it overrides, never change or drop the ones it inherits.
#[component]
pub fn ArgumentsEditor(
    title: String,
    rows: Vec<ArgumentRow>,
    data_types: Vec<Choice>,
    ranks: Vec<Choice>,
    nonce: u64,
    onchange: EventHandler<Vec<ArgumentRow>>,
    #[props(default)] locked: usize,
    #[props(default)] disabled: bool,
    error: Option<String>,
) -> Element {
    let mut draft = use_signal(|| rows.clone());
    use_effect(use_reactive!(|(rows, nonce)| {
        let _ = nonce;
        draft.set(rows);
    }));

    let commit = move || onchange.call(draft.peek().clone());
    let entries = draft();
    let count = entries.len();

    rsx! {
        div { class: "arguments",
            header { class: "arguments__head",
                span { class: "type-label", {title} }
                span { class: "chip mono", "{count}" }
                div { class: "arguments__spacer" }
                if !disabled {
                    button {
                        class: "button tonal tiny",
                        onclick: move |_| {
                            draft.with_mut(|rows| rows.push(ArgumentRow::fresh()));
                            commit();
                        },
                        Icon { name: "add", class: "small" }
                        "Add argument"
                    }
                }
            }
            if entries.is_empty() {
                span { class: "field__hint", "No arguments." }
            }
            for (position , entry) in entries.iter().enumerate() {
                {
                    argument_editor_row(
                        position,
                        entry,
                        &data_types,
                        &ranks,
                        disabled || position < locked,
                        position < locked,
                        draft,
                        onchange,
                    )
                }
            }
            if let Some(error) = error {
                span { class: "field__error",
                    Icon { name: "error", class: "small" }
                    {error}
                }
            }
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "one row of a table needs the whole row's state")]
fn argument_editor_row(
    position: usize,
    entry: &ArgumentRow,
    data_types: &[Choice],
    ranks: &[Choice],
    disabled: bool,
    inherited: bool,
    mut draft: Signal<Vec<ArgumentRow>>,
    onchange: EventHandler<Vec<ArgumentRow>>,
) -> Element {
    let commit = move || onchange.call(draft.peek().clone());
    let data_type = entry.data_type.to_string();
    let rank = entry.value_rank.0.to_string();
    let dimensions = entry.array_dimensions.to_string();
    let description = entry.description.clone().unwrap_or_default();
    let last = draft.peek().len().saturating_sub(1);

    rsx! {
        div {
            key: "{position}",
            class: if inherited { "argument inherited" } else { "argument" },
            div { class: "argument__bar",
                span { class: "argument__index mono", "{position}" }
                input {
                    class: "argument__name",
                    value: "{entry.name}",
                    placeholder: "name",
                    disabled,
                    oninput: move |event| {
                        let name = event.value();
                        draft.with_mut(|rows| {
                            if let Some(row) = rows.get_mut(position) {
                                row.name = name;
                            }
                        });
                    },
                    onchange: move |_| commit(),
                }
                select {
                    class: "field__select",
                    value: data_type.clone(),
                    disabled,
                    onchange: move |event| {
                        let Ok(picked) = event.value().parse::<NodeId>() else {
                            return;
                        };
                        draft.with_mut(|rows| {
                            if let Some(row) = rows.get_mut(position) {
                                row.data_type = picked;
                            }
                        });
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
                        draft.with_mut(|rows| {
                            if let Some(row) = rows.get_mut(position) {
                                row.value_rank = ValueRank(picked);
                            }
                        });
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
                    value: dimensions.clone(),
                    placeholder: "dims",
                    title: "ArrayDimensions, comma-separated",
                    disabled,
                    onchange: move |event| {
                        let Ok(parsed) = event.value().trim().parse::<ArrayDimensions>() else {
                            return;
                        };
                        draft.with_mut(|rows| {
                            if let Some(row) = rows.get_mut(position) {
                                row.array_dimensions = parsed;
                            }
                        });
                        commit();
                    },
                }
                if inherited {
                    span { class: "chip", title: "Inherited — an override may only append after it", "inherited" }
                }
                if !disabled {
                    button {
                        class: "icon-button tiny",
                        title: "Move up",
                        disabled: position == 0,
                        onclick: move |_| {
                            draft.with_mut(|rows| rows.swap(position, position.saturating_sub(1)));
                            commit();
                        },
                        Icon { name: "arrow_upward", class: "small" }
                    }
                    button {
                        class: "icon-button tiny",
                        title: "Move down",
                        disabled: position >= last,
                        onclick: move |_| {
                            draft.with_mut(|rows| {
                                if position + 1 < rows.len() {
                                    rows.swap(position, position + 1);
                                }
                            });
                            commit();
                        },
                        Icon { name: "arrow_downward", class: "small" }
                    }
                    button {
                        class: "icon-button tiny",
                        title: "Remove this argument",
                        onclick: move |_| {
                            draft.with_mut(|rows| {
                                if position < rows.len() {
                                    rows.remove(position);
                                }
                            });
                            commit();
                        },
                        Icon { name: "close", class: "small" }
                    }
                }
            }
            div { class: "argument__bar",
                span { class: "argument__label type-label-small", "Description" }
                input {
                    class: "locales__locale mono",
                    value: "{description.locale.clone().unwrap_or_default()}",
                    placeholder: "locale",
                    disabled,
                    oninput: move |event| {
                        let locale = event.value();
                        draft.with_mut(|rows| {
                            if let Some(row) = rows.get_mut(position) {
                                let mut text = row.description.clone().unwrap_or_default();
                                text.locale = (!locale.is_empty()).then_some(locale);
                                row.description = Some(text);
                            }
                        });
                    },
                    onchange: move |_| commit(),
                }
                input {
                    class: "argument__description",
                    value: "{description.text}",
                    placeholder: "what the argument means",
                    disabled,
                    oninput: move |event| {
                        let value = event.value();
                        draft.with_mut(|rows| {
                            if let Some(row) = rows.get_mut(position) {
                                let mut text = row.description.clone().unwrap_or_default();
                                text.text = value;
                                row.description = Some(text);
                            }
                        });
                    },
                    onchange: move |_| commit(),
                }
                if !disabled {
                    button {
                        class: "icon-button tiny",
                        title: "Drop the Description",
                        onclick: move |_| {
                            draft.with_mut(|rows| {
                                if let Some(row) = rows.get_mut(position) {
                                    row.description = None;
                                }
                            });
                            commit();
                        },
                        Icon { name: "backspace", class: "small" }
                    }
                }
            }
        }
    }
}

/// The Arguments a Method declaration states, which an override may only append to.
pub fn inherited_arguments(
    handle: EditorHandle,
    method: &NodeId,
    browse_name: &'static str,
) -> Vec<ArgumentRow> {
    handle
        .with_space(|space| {
            let property = space
                .children_named(method, &QualifiedName::new(0, browse_name))
                .into_iter()
                .next()?;
            let Node::Variable(variable) = space.node(&property)? else {
                return None;
            };
            argument_rows(variable.value.as_ref())
        })
        .flatten()
        .unwrap_or_default()
}

//! The Value section of a Variable or a VariableType.
//!
//! Three states, chosen from the value itself rather than from anything this module decides: a
//! typed editor for a scalar or a one-dimensional array of a built-in type, the read-only XML
//! rendering for everything else, and an empty state that offers to start one (features.md §2D).
//!
//! Nothing here builds a value out of text the model has not seen: every commit hands the session a
//! whole `Variant` built through the domain's own types, and the elements the user did not touch
//! are carried across unchanged, lexical forms and all.

use dioxus::prelude::*;
use uanedit::attributes::value_rank::ValueRank;
use uanedit::edit::FieldValue;
use uanedit::emit::value_xml;
use uanedit::nodes::Node;
use uanedit::space::AddressSpace;
use uanedit::types::built_in::BuiltInType;
use uanedit::types::byte_string::ByteString;
use uanedit::types::date_time::DateTime;
use uanedit::types::expanded_node_id::ExpandedNodeId;
use uanedit::types::guid::Guid;
use uanedit::types::localized_text::LocalizedText;
use uanedit::types::node_id::{
    Identifier,
    NodeId,
};
use uanedit::types::qualified_name::QualifiedName;
use uanedit::types::status_code::StatusCode;
use uanedit::types::variant::{
    Variant,
    VariantArray,
};

use crate::components::Icon;
use crate::views::editor::fields::ReadOnlyBlock;
use crate::views::editor::forms::{
    Editing,
    section,
};

/// The name a refusal and a warning land under, which is the section's own.
const FIELD: &str = "Value";

/// Which typed editor a value gets.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Scalar(BuiltInType),
    Array(BuiltInType),
}

pub fn value_section(
    editing: &Editing,
    space: &AddressSpace,
    node: &Node,
) -> Element {
    let (value, rank) = match node {
        Node::Variable(variable) => (variable.value.as_ref(), variable.value_rank),
        Node::VariableType(variable_type) => (variable_type.value.as_ref(), variable_type.value_rank),
        _ => return rsx! {},
    };

    let body = match value {
        None => empty_state(editing, space, rank, false),
        Some(Variant::Null) => empty_state(editing, space, rank, true),
        Some(current) => match shape_of(current) {
            Some(Shape::Scalar(built_in)) => scalar_state(editing, built_in, current),
            Some(Shape::Array(built_in)) => array_state(editing, built_in, current),
            None => opaque_state(current),
        },
    };
    section(
        "Value",
        "data_object",
        rsx! {
            div { class: "value", {body} }
        },
    )
}

/// The editor a value maps to, decided by the arm it already carries.
///
/// An array qualifies only when every element is the element type it declares, so a list this
/// crate read as something else — a `Raw` element, a mixed list — keeps its XML rendering.
fn shape_of(value: &Variant) -> Option<Shape> {
    match value {
        Variant::Array(array) => {
            let element = array.element_type;
            let uniform = array
                .values
                .iter()
                .all(|value| value.built_in_type() == element);
            (editable(element) && uniform).then_some(Shape::Array(element))
        }
        Variant::Null | Variant::Raw(_) | Variant::Matrix(_) => None,
        scalar => editable(scalar.built_in_type()).then_some(Shape::Scalar(scalar.built_in_type())),
    }
}

/// The built-in types v1 edits: the scalars whose whole value is one lexical form, plus
/// LocalizedText. XmlElement, ExtensionObject, DataValue, Variant and DiagnosticInfo carry nested
/// documents this editor has no control for, so they stay read-only.
fn editable(built_in: BuiltInType) -> bool {
    matches!(
        built_in,
        BuiltInType::Boolean
            | BuiltInType::SByte
            | BuiltInType::Byte
            | BuiltInType::Int16
            | BuiltInType::UInt16
            | BuiltInType::Int32
            | BuiltInType::UInt32
            | BuiltInType::Int64
            | BuiltInType::UInt64
            | BuiltInType::Float
            | BuiltInType::Double
            | BuiltInType::String
            | BuiltInType::DateTime
            | BuiltInType::Guid
            | BuiltInType::ByteString
            | BuiltInType::NodeId
            | BuiltInType::ExpandedNodeId
            | BuiltInType::StatusCode
            | BuiltInType::QualifiedName
            | BuiltInType::LocalizedText
    )
}

/// The built-in a DataType attribute names, and only when it names one exactly.
///
/// A subtype of a built-in — an enumeration over Int32, `Duration` over Double — is deliberately
/// not offered a typed editor in v1: the file may want an encoding this editor does not know it
/// owes. Such a node still gets one once its Value carries a supported arm.
fn built_in_of(data_type: &NodeId) -> Option<BuiltInType> {
    if data_type.namespace_index != 0 {
        return None;
    }
    let Identifier::Numeric(identifier) = data_type.identifier else {
        return None;
    };
    let built_in = u8::try_from(identifier)
        .ok()
        .and_then(BuiltInType::from_id)?;
    editable(built_in).then_some(built_in)
}

fn offers_array(rank: ValueRank) -> bool {
    rank == ValueRank::ONE_DIMENSION
        || rank == ValueRank::ONE_OR_MORE_DIMENSIONS
        || rank == ValueRank::ANY
        || rank == ValueRank::SCALAR_OR_ONE_DIMENSION
}

fn empty_state(
    editing: &Editing,
    space: &AddressSpace,
    rank: ValueRank,
    stated: bool,
) -> Element {
    let built_in = space
        .data_type(&editing.node)
        .as_ref()
        .and_then(built_in_of);
    let scalar = built_in.is_some() && rank.allows_scalar();
    let array = built_in.is_some() && offers_array(rank);
    let offered = !editing.read_only && (scalar || array);

    let scalar_value = {
        let editing = editing.clone();
        move |_| {
            if let Some(built_in) = built_in {
                editing.set(FIELD, FieldValue::Value(Some(default_variant(built_in))));
            }
        }
    };
    let array_value = {
        let editing = editing.clone();
        move |_| {
            if let Some(built_in) = built_in {
                let array = Variant::Array(VariantArray::new(built_in, Vec::new()));
                editing.set(FIELD, FieldValue::Value(Some(array)));
            }
        }
    };

    rsx! {
        div { class: "value__empty type-body-small",
            if stated {
                "This node states an empty Value element."
            } else {
                "This node states no Value."
            }
        }
        if offered {
            div { class: "value__actions",
                if scalar {
                    button { class: "button tonal tiny", onclick: scalar_value,
                        Icon { name: "add", class: "small" }
                        "Add value"
                    }
                }
                if array {
                    button { class: "button tonal tiny", onclick: array_value,
                        Icon { name: "add", class: "small" }
                        "Add array"
                    }
                }
            }
        }
        if !offered {
            span { class: "field__hint", {unavailable(space, editing, built_in, rank)} }
        }
        if stated && !editing.read_only {
            {clear_button(editing, "Remove the empty Value element")}
        }
        {value_note(editing)}
    }
}

/// Why the empty state has nothing to offer, in the terms of the node's own attributes.
fn unavailable(
    space: &AddressSpace,
    editing: &Editing,
    built_in: Option<BuiltInType>,
    rank: ValueRank,
) -> String {
    if editing.read_only {
        return "This node belongs to a nodeset this editor does not change.".to_owned();
    }
    if built_in.is_none() {
        let named = space
            .data_type(&editing.node)
            .map(|data_type| match space.node(&data_type) {
                Some(node) => node.header().label(None).to_owned(),
                None => data_type.to_string(),
            })
            .unwrap_or_else(|| "this node's DataType".to_owned());
        return format!("{named} is not a built-in type — a value of it is edited as XML in a later version.");
    }
    format!("ValueRank {rank} asks for a value this editor does not build yet.")
}

fn scalar_state(
    editing: &Editing,
    built_in: BuiltInType,
    current: &Variant,
) -> Element {
    rsx! {
        {value_head(editing, built_in.name().to_owned(), "scalar".to_owned())}
        VariantInput {
            built_in,
            value: current.clone(),
            nonce: editing.nonce(),
            disabled: editing.read_only,
            oncommit: commit_value(editing),
        }
        {value_note(editing)}
    }
}

fn array_state(
    editing: &Editing,
    built_in: BuiltInType,
    current: &Variant,
) -> Element {
    let Variant::Array(array) = current else {
        return rsx! {};
    };
    let editing = editing.clone();
    let commit = {
        let editing = editing.clone();
        EventHandler::new(move |values: Vec<Variant>| {
            let array = Variant::Array(VariantArray::new(built_in, values));
            editing.set(FIELD, FieldValue::Value(Some(array)));
        })
    };

    rsx! {
        {value_head(&editing, built_in.list_name(), counted(array.len()))}
        ValueArray {
            built_in,
            values: array.values.clone(),
            nonce: editing.nonce(),
            disabled: editing.read_only,
            oncommit: commit,
        }
        {value_note(&editing)}
    }
}

fn opaque_state(value: &Variant) -> Element {
    rsx! {
        div { class: "value__head",
            span { class: "chip mono", {opaque_name(value)} }
            span { class: "chip", "read-only" }
        }
        ReadOnlyBlock {
            label: "As this editor reads it",
            text: value_xml(value),
            note: "This value's type is edited as XML in a later version. The file's own encoding is what a save writes back.",
        }
    }
}

fn opaque_name(value: &Variant) -> String {
    match value {
        Variant::Matrix(matrix) => format!("Matrix · {} dimensions", matrix.dimensions.len()),
        Variant::Raw(element) => format!("{} · unmodelled", element.local_name()),
        Variant::Array(array) => array.element_type.list_name(),
        other => other.built_in_type().name().to_owned(),
    }
}

/// The chips that say what the value is, and the action that takes it away.
fn value_head(
    editing: &Editing,
    kind: String,
    detail: String,
) -> Element {
    rsx! {
        div { class: "value__head",
            span { class: "chip mono", {kind} }
            span { class: "chip", {detail} }
            if !editing.read_only {
                {clear_button(editing, "Remove the Value element from this node")}
            }
        }
    }
}

fn clear_button(
    editing: &Editing,
    title: &'static str,
) -> Element {
    let editing = editing.clone();
    rsx! {
        button {
            class: "button text tiny value__clear",
            title,
            onclick: move |_| editing.set(FIELD, FieldValue::Value(None)),
            Icon { name: "backspace", class: "small" }
            "Clear value"
        }
    }
}

fn commit_value(editing: &Editing) -> EventHandler<Variant> {
    let editing = editing.clone();
    EventHandler::new(move |value: Variant| editing.set(FIELD, FieldValue::Value(Some(value))))
}

/// The refusal or the warning the last commit left, at the section it came from.
fn value_note(editing: &Editing) -> Element {
    let error = editing.error_for(FIELD);
    let hint = editing.note_for(FIELD);
    rsx! {
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

fn counted(len: usize) -> String {
    match len {
        1 => "1 element".to_owned(),
        count => format!("{count} elements"),
    }
}

/// The rows of a one-dimensional array; every change hands back the whole list, so the elements
/// that were not touched keep the values — and the spellings — they were read with.
#[component]
fn ValueArray(
    built_in: BuiltInType,
    values: Vec<Variant>,
    nonce: u64,
    disabled: bool,
    oncommit: EventHandler<Vec<Variant>>,
) -> Element {
    let appended = values.clone();
    let last = values.len().saturating_sub(1);

    rsx! {
        div { class: "value-array",
            for (position , value) in values.iter().cloned().enumerate() {
                div { key: "{position}", class: "value-array__row",
                    span { class: "value-array__index mono", "{position}" }
                    div { class: "value-array__cell",
                        VariantInput {
                            built_in,
                            value,
                            nonce,
                            disabled,
                            oncommit: {
                                let values = values.clone();
                                EventHandler::new(move |next: Variant| {
                                    let mut values = values.clone();
                                    if let Some(slot) = values.get_mut(position) {
                                        *slot = next;
                                    }
                                    oncommit.call(values);
                                })
                            },
                        }
                    }
                    button {
                        class: "icon-button tiny",
                        disabled: disabled || position == 0,
                        title: "Move up",
                        onclick: {
                            let values = values.clone();
                            move |_| oncommit.call(swapped(&values, position, position.wrapping_sub(1)))
                        },
                        Icon { name: "arrow_upward", class: "small" }
                    }
                    button {
                        class: "icon-button tiny",
                        disabled: disabled || position == last,
                        title: "Move down",
                        onclick: {
                            let values = values.clone();
                            move |_| oncommit.call(swapped(&values, position, position + 1))
                        },
                        Icon { name: "arrow_downward", class: "small" }
                    }
                    button {
                        class: "icon-button tiny",
                        disabled,
                        title: "Remove this element",
                        onclick: {
                            let values = values.clone();
                            move |_| oncommit.call(without(&values, position))
                        },
                        Icon { name: "close", class: "small" }
                    }
                }
            }
            if !disabled {
                button {
                    class: "button text tiny",
                    onclick: move |_| {
                        let mut values = appended.clone();
                        values.push(default_variant(built_in));
                        oncommit.call(values);
                    },
                    Icon { name: "add", class: "small" }
                    "Add element"
                }
            }
        }
    }
}

fn swapped(
    values: &[Variant],
    from: usize,
    to: usize,
) -> Vec<Variant> {
    let mut values = values.to_vec();
    if from < values.len() && to < values.len() {
        values.swap(from, to);
    }
    values
}

fn without(
    values: &[Variant],
    position: usize,
) -> Vec<Variant> {
    let mut values = values.to_vec();
    if position < values.len() {
        values.remove(position);
    }
    values
}

/// The control one built-in value gets: a checkbox, a locale pair, or text this module parses.
#[component]
fn VariantInput(
    built_in: BuiltInType,
    value: Variant,
    nonce: u64,
    disabled: bool,
    oncommit: EventHandler<Variant>,
) -> Element {
    match built_in {
        BuiltInType::Boolean => rsx! {
            ValueBool {
                value: matches!(value, Variant::Boolean(true)),
                nonce,
                disabled,
                oncommit,
            }
        },
        BuiltInType::LocalizedText => rsx! {
            ValueLocalized {
                value: match value {
                    Variant::LocalizedText(text) => text,
                    _ => LocalizedText::default(),
                },
                nonce,
                disabled,
                oncommit,
            }
        },
        _ => rsx! {
            ValueText {
                built_in,
                text: variant_text(&value),
                nonce,
                disabled,
                oncommit,
            }
        },
    }
}

#[component]
fn ValueBool(
    value: bool,
    nonce: u64,
    disabled: bool,
    oncommit: EventHandler<Variant>,
) -> Element {
    let mut draft = use_signal(|| value);
    use_effect(use_reactive!(|(value, nonce)| {
        let _ = nonce;
        draft.set(value);
    }));

    rsx! {
        label { class: "field__toggle",
            input {
                r#type: "checkbox",
                checked: draft(),
                disabled,
                onchange: move |event| {
                    draft.set(event.checked());
                    oncommit.call(Variant::Boolean(event.checked()));
                },
            }
            span { class: "field__toggle-label mono", "{draft}" }
        }
    }
}

/// A LocalizedText, whose absent locale the file distinguishes from an empty one.
#[component]
fn ValueLocalized(
    value: LocalizedText,
    nonce: u64,
    disabled: bool,
    oncommit: EventHandler<Variant>,
) -> Element {
    let mut draft = use_signal(|| value.clone());
    use_effect(use_reactive!(|(value, nonce)| {
        let _ = nonce;
        draft.set(value);
    }));

    let commit = move || {
        let text = draft.peek().clone();
        oncommit.call(Variant::LocalizedText(text));
    };
    let entry = draft();

    rsx! {
        div { class: "locales__row",
            input {
                class: "locales__locale mono",
                value: "{entry.locale.clone().unwrap_or_default()}",
                placeholder: "locale",
                disabled,
                oninput: move |event| {
                    let locale = event.value();
                    draft.with_mut(|text| text.locale = (!locale.is_empty()).then_some(locale));
                },
                onchange: move |_| commit(),
            }
            input {
                class: "locales__text",
                value: "{entry.text}",
                placeholder: "text",
                disabled,
                oninput: move |event| {
                    let text = event.value();
                    draft.with_mut(|value| value.text = text);
                },
                onchange: move |_| commit(),
            }
        }
    }
}

/// A value whose whole content is one lexical form. A draft that will not parse stays in the
/// control with the reason beside it, and never reaches the session.
#[component]
fn ValueText(
    built_in: BuiltInType,
    text: String,
    nonce: u64,
    disabled: bool,
    oncommit: EventHandler<Variant>,
) -> Element {
    let mut draft = use_signal(|| text.clone());
    let mut failed = use_signal(|| None::<String>);
    use_effect(use_reactive!(|(text, nonce)| {
        let _ = nonce;
        draft.set(text);
        failed.set(None);
    }));

    // A commit the model reads as no change leaves the props alone, so the draft is put back to
    // the value's own spelling here rather than in the effect.
    let commit = move |entered: String| {
        let (mut draft, mut failed) = (draft, failed);
        match parse_variant(built_in, &entered) {
            Ok(value) => {
                failed.set(None);
                draft.set(variant_text(&value));
                oncommit.call(value);
            }
            Err(message) => failed.set(Some(message)),
        }
    };
    let invalid = failed.read().is_some();

    rsx! {
        div { class: if invalid { "value-cell invalid" } else { "value-cell" },
            if built_in == BuiltInType::ByteString {
                textarea {
                    class: "field__input value-cell__area mono",
                    value: "{draft}",
                    placeholder: placeholder(built_in),
                    disabled,
                    oninput: move |event| draft.set(event.value()),
                    onchange: move |event| commit(event.value()),
                }
            } else {
                input {
                    class: "field__input mono",
                    value: "{draft}",
                    placeholder: placeholder(built_in),
                    disabled,
                    oninput: move |event| draft.set(event.value()),
                    onchange: move |event| commit(event.value()),
                }
            }
            if let Some(message) = failed() {
                span { class: "field__error",
                    Icon { name: "error", class: "small" }
                    {message}
                }
            }
        }
    }
}

/// The text a control seeds from, which is the text a save writes for the same value.
fn variant_text(value: &Variant) -> String {
    match value {
        Variant::Boolean(flag) => flag.to_string(),
        Variant::SByte(number) => number.to_string(),
        Variant::Byte(number) => number.to_string(),
        Variant::Int16(number) => number.to_string(),
        Variant::UInt16(number) => number.to_string(),
        Variant::Int32(number) => number.to_string(),
        Variant::UInt32(number) => number.to_string(),
        Variant::Int64(number) => number.to_string(),
        Variant::UInt64(number) => number.to_string(),
        Variant::Float(number) => real_text(f64::from(*number)),
        Variant::Double(number) => real_text(*number),
        Variant::String(text) => text.clone(),
        Variant::DateTime(instant) => instant.to_string(),
        Variant::Guid(guid) => guid.to_string(),
        Variant::ByteString(bytes) => bytes.encode().unwrap_or_default(),
        Variant::NodeId(node_id) => node_id.to_string(),
        Variant::ExpandedNodeId(node_id) => node_id.to_string(),
        Variant::StatusCode(status) => status.0.to_string(),
        Variant::QualifiedName(name) => name.to_string(),
        _ => String::new(),
    }
}

/// `xs:double` and `xs:float` spell their three special values in capitals, as the writer does.
fn real_text(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "INF" } else { "-INF" }.to_owned();
    }
    value.to_string()
}

/// Reads one built-in value from the control's text, through the domain's own parsers.
fn parse_variant(
    built_in: BuiltInType,
    text: &str,
) -> Result<Variant, String> {
    let trimmed = text.trim();
    match built_in {
        BuiltInType::Boolean => match trimmed {
            "true" | "1" => Ok(Variant::Boolean(true)),
            "false" | "0" => Ok(Variant::Boolean(false)),
            _ => Err(expected(built_in, "true or false")),
        },
        BuiltInType::SByte => whole(trimmed)
            .map(Variant::SByte)
            .ok_or_else(|| ranged(built_in)),
        BuiltInType::Byte => whole(trimmed)
            .map(Variant::Byte)
            .ok_or_else(|| ranged(built_in)),
        BuiltInType::Int16 => whole(trimmed)
            .map(Variant::Int16)
            .ok_or_else(|| ranged(built_in)),
        BuiltInType::UInt16 => whole(trimmed)
            .map(Variant::UInt16)
            .ok_or_else(|| ranged(built_in)),
        BuiltInType::Int32 => whole(trimmed)
            .map(Variant::Int32)
            .ok_or_else(|| ranged(built_in)),
        BuiltInType::UInt32 => whole(trimmed)
            .map(Variant::UInt32)
            .ok_or_else(|| ranged(built_in)),
        BuiltInType::Int64 => whole(trimmed)
            .map(Variant::Int64)
            .ok_or_else(|| ranged(built_in)),
        BuiltInType::UInt64 => whole(trimmed)
            .map(Variant::UInt64)
            .ok_or_else(|| ranged(built_in)),
        BuiltInType::Float => whole(trimmed)
            .map(Variant::Float)
            .ok_or_else(|| expected(built_in, "a number, or NaN, INF or -INF")),
        BuiltInType::Double => whole(trimmed)
            .map(Variant::Double)
            .ok_or_else(|| expected(built_in, "a number, or NaN, INF or -INF")),
        BuiltInType::String => Ok(Variant::String(text.to_owned())),
        BuiltInType::DateTime => whole::<DateTime>(trimmed)
            .map(Variant::DateTime)
            .ok_or_else(|| expected(built_in, "an instant such as 2024-01-31T09:00:00Z")),
        BuiltInType::Guid => whole::<Guid>(trimmed)
            .map(Variant::Guid)
            .ok_or_else(|| expected(built_in, "a GUID such as 09087e75-8e5e-499b-954f-f2a9603db28a")),
        BuiltInType::ByteString => ByteString::decode(text)
            .map(Variant::ByteString)
            .map_err(|_| expected(built_in, "base64")),
        BuiltInType::NodeId => whole::<NodeId>(trimmed)
            .map(Variant::NodeId)
            .ok_or_else(|| expected(built_in, "a NodeId such as i=84 or ns=1;s=Machine")),
        BuiltInType::ExpandedNodeId => whole::<ExpandedNodeId>(trimmed)
            .map(Variant::ExpandedNodeId)
            .ok_or_else(|| expected(built_in, "a NodeId, optionally with nsu= and svr=")),
        BuiltInType::StatusCode => status_code(trimmed)
            .map(|code| Variant::StatusCode(StatusCode(code)))
            .ok_or_else(|| expected(built_in, "a code as a number, decimal or 0x-prefixed")),
        BuiltInType::QualifiedName => whole::<QualifiedName>(trimmed)
            .map(Variant::QualifiedName)
            .ok_or_else(|| expected(built_in, "a name, optionally as <index>:<name>")),
        _ => Err(format!("{built_in} is not edited here")),
    }
}

fn whole<T: core::str::FromStr>(text: &str) -> Option<T> {
    text.parse().ok()
}

/// A StatusCode is written as a number; hex is what everyone reads one in.
fn status_code(text: &str) -> Option<u32> {
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(digits) => u32::from_str_radix(digits, 16).ok(),
        None => text.parse().ok(),
    }
}

fn expected(
    built_in: BuiltInType,
    what: &str,
) -> String {
    format!("{built_in} takes {what}")
}

fn ranged(built_in: BuiltInType) -> String {
    expected(built_in, "a whole number inside its range")
}

fn placeholder(built_in: BuiltInType) -> &'static str {
    match built_in {
        BuiltInType::DateTime => "2024-01-31T09:00:00Z",
        BuiltInType::Guid => "09087e75-8e5e-499b-954f-f2a9603db28a",
        BuiltInType::ByteString => "base64",
        BuiltInType::NodeId => "i=84 or ns=1;s=Machine",
        BuiltInType::ExpandedNodeId => "nsu=http://example.org/;i=84",
        BuiltInType::StatusCode => "0 or 0x80340000",
        BuiltInType::QualifiedName => "1:Name",
        _ => "",
    }
}

/// What a newly added value or array element starts as: the null value of its type.
fn default_variant(built_in: BuiltInType) -> Variant {
    match built_in {
        BuiltInType::Boolean => Variant::Boolean(false),
        BuiltInType::SByte => Variant::SByte(0),
        BuiltInType::Byte => Variant::Byte(0),
        BuiltInType::Int16 => Variant::Int16(0),
        BuiltInType::UInt16 => Variant::UInt16(0),
        BuiltInType::Int32 => Variant::Int32(0),
        BuiltInType::UInt32 => Variant::UInt32(0),
        BuiltInType::Int64 => Variant::Int64(0),
        BuiltInType::UInt64 => Variant::UInt64(0),
        BuiltInType::Float => Variant::Float(0.0),
        BuiltInType::Double => Variant::Double(0.0),
        BuiltInType::String => Variant::String(String::new()),
        BuiltInType::DateTime => Variant::DateTime(DateTime::EPOCH),
        BuiltInType::Guid => Variant::Guid(Guid::NULL),
        BuiltInType::ByteString => Variant::ByteString(ByteString::NULL),
        BuiltInType::NodeId => Variant::NodeId(NodeId::NULL),
        BuiltInType::ExpandedNodeId => Variant::ExpandedNodeId(ExpandedNodeId::default()),
        BuiltInType::StatusCode => Variant::StatusCode(StatusCode::GOOD),
        BuiltInType::QualifiedName => Variant::QualifiedName(QualifiedName::default()),
        BuiltInType::LocalizedText => Variant::LocalizedText(LocalizedText::default()),
        _ => Variant::Null,
    }
}

//! The per-class attribute forms.
//!
//! Every control writes through [`Editing`], which turns the widget's value into one semantic
//! operation and puts a refusal back at the field it came from. Every choice list comes from
//! `uanedit::rules::query`, never from a filter written here (guardrails.md §1.2).

use core::str::FromStr;

use dioxus::prelude::*;
use uanedit::Operation;
use uanedit::attributes::access_level::AccessLevel;
use uanedit::attributes::array_dimensions::ArrayDimensions;
use uanedit::attributes::event_notifier::EventNotifier;
use uanedit::attributes::value_rank::ValueRank;
use uanedit::attributes::write_mask::WriteMask;
use uanedit::edit::FieldValue;
use uanedit::nodes::common::NodeHeader;
use uanedit::nodes::definition::DataTypeDefinition;
use uanedit::nodes::{
    Node,
    NodeClass,
    ReleaseStatus,
};
use uanedit::rules::query;
use uanedit::space::AddressSpace;
use uanedit::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use uanedit::types::qualified_name::QualifiedName;

use crate::components::Icon;
use crate::session::{
    EditResult,
    EditorHandle,
};
use crate::views::editor::arguments::method_arguments;
use crate::views::editor::definition::{
    DefinitionEditor,
    field_data_types,
    rank_choices as definition_ranks,
};
use crate::views::editor::fields::{
    BoolField,
    Choice,
    LocalizedTextField,
    MaskField,
    QualifiedNameField,
    SelectField,
    TextField,
};
use crate::views::editor::icons::class_icon;
use crate::views::editor::short_namespace;
use crate::views::editor::value_edit::value_section;

/// The highest fixed rank a picker offers; past four dimensions nobody is choosing from a list.
const MAX_DIMENSIONS: u32 = 4;

/// The channel one inspector form writes through, and where a refusal lands.
#[derive(Clone)]
pub struct Editing {
    pub handle: EditorHandle,
    pub node: NodeId,
    pub read_only: bool,
    pub nonce: Signal<u64>,
    pub failure: Signal<Option<(String, String)>>,
    pub note: Signal<Option<(String, String)>>,
}

impl Editing {
    pub fn nonce(&self) -> u64 {
        *self.nonce.read()
    }

    pub fn error_for(
        &self,
        field: &str,
    ) -> Option<String> {
        message_for(&self.failure, field)
    }

    pub fn note_for(
        &self,
        field: &str,
    ) -> Option<String> {
        message_for(&self.note, field)
    }

    pub(super) fn set(
        &self,
        field: &'static str,
        value: FieldValue,
    ) {
        self.perform(
            field,
            Operation::SetField {
                node: self.node.clone(),
                value,
            },
        );
    }

    /// A field edit on another node: the Property that holds a Method's arguments is the node the
    /// value actually lives on (OPC 10000-3 §5.7.1).
    pub(super) fn set_on(
        &self,
        field: &'static str,
        node: NodeId,
        value: FieldValue,
    ) {
        self.perform(field, Operation::SetField { node, value });
    }

    pub(super) fn perform_on(
        &self,
        field: &'static str,
        operation: Operation,
    ) {
        self.perform(field, operation);
    }

    fn perform(
        &self,
        field: &'static str,
        operation: Operation,
    ) {
        let (mut failure, mut note) = (self.failure, self.note);
        match self.handle.apply(operation) {
            EditResult::Refused(refusal) => {
                note.set(None);
                self.reject(field, refusal.to_string());
            }
            EditResult::Applied { warnings } => {
                failure.set(None);
                note.set((warnings > 0).then(|| (field.to_owned(), warning_text(warnings))));
            }
            EditResult::Unchanged | EditResult::Closed => {
                failure.set(None);
                note.set(None);
            }
        }
    }

    /// Puts a message at the field and re-seeds its control from the model.
    fn reject(
        &self,
        field: &'static str,
        message: impl Into<String>,
    ) {
        let (mut failure, mut nonce) = (self.failure, self.nonce);
        failure.set(Some((field.to_owned(), message.into())));
        nonce.with_mut(|value| *value = value.wrapping_add(1));
    }

    fn on<T: 'static>(
        &self,
        field: &'static str,
        into: impl Fn(T) -> FieldValue + 'static,
    ) -> EventHandler<T> {
        let editing = self.clone();
        EventHandler::new(move |value: T| editing.set(field, into(value)))
    }

    /// A commit that parses the control's text, and says so at the field when it cannot.
    fn parsed<T: FromStr + 'static>(
        &self,
        field: &'static str,
        expected: &'static str,
        into: impl Fn(T) -> FieldValue + 'static,
    ) -> EventHandler<String> {
        let editing = self.clone();
        EventHandler::new(move |text: String| match text.trim().parse::<T>() {
            Ok(value) => editing.set(field, into(value)),
            Err(_) => editing.reject(field, format!("{field} takes {expected}")),
        })
    }

    /// A commit that names another node, which is an operation of its own rather than a field.
    fn targets(
        &self,
        field: &'static str,
        into: impl Fn(NodeId, NodeId) -> Operation + 'static,
    ) -> EventHandler<String> {
        let editing = self.clone();
        EventHandler::new(move |text: String| match text.parse::<NodeId>() {
            Ok(target) => editing.perform(field, into(editing.node.clone(), target)),
            Err(_) => editing.reject(field, format!("{text} is not a NodeId")),
        })
    }

    fn flags(
        &self,
        field: &'static str,
        current: AccessLevel,
        into: impl Fn(AccessLevel) -> FieldValue + 'static,
    ) -> EventHandler<(u32, bool)> {
        let editing = self.clone();
        EventHandler::new(move |(bits, enabled): (u32, bool)| {
            let mut next = current;
            next.set(AccessLevel(bits), enabled);
            editing.set(field, into(next));
        })
    }

    fn notifier(
        &self,
        field: &'static str,
        current: EventNotifier,
    ) -> EventHandler<(u32, bool)> {
        let editing = self.clone();
        EventHandler::new(move |(bits, enabled): (u32, bool)| {
            let mut next = current;
            next.set(EventNotifier(u8::try_from(bits).unwrap_or(0)), enabled);
            editing.set(field, FieldValue::EventNotifier(next));
        })
    }
}

fn message_for(
    slot: &Signal<Option<(String, String)>>,
    field: &str,
) -> Option<String> {
    slot.read()
        .as_ref()
        .filter(|(name, _)| name == field)
        .map(|(_, message)| message.clone())
}

fn warning_text(warnings: usize) -> String {
    match warnings {
        1 => "Applied · one new warning in the validation panel".to_owned(),
        count => format!("Applied · {count} new warnings in the validation panel"),
    }
}

pub fn attribute_form(
    editing: &Editing,
    space: &AddressSpace,
    node: &Node,
) -> Element {
    let namespaces = namespace_choices(space);
    let header = node.header();

    rsx! {
        div { class: "form",
            {identity_section(editing, &namespaces, header)}
            {names_section(editing, header)}
            {class_section(editing, space, node)}
            {value_section(editing, space, node)}
            {method_arguments(editing, space, node)}
            {type_definition_section(editing, space, node)}
            {metadata_section(editing, node)}
        }
    }
}

fn identity_section(
    editing: &Editing,
    namespaces: &[Choice],
    header: &NodeHeader,
) -> Element {
    let explicit = header.browse_name.explicit_index;
    let commit = {
        let editing = editing.clone();
        EventHandler::new(move |(namespace_index, name): (NamespaceIndex, String)| {
            editing.set(
                "BrowseName",
                FieldValue::BrowseName(QualifiedName {
                    namespace_index,
                    name,
                    explicit_index: explicit && namespace_index == 0,
                }),
            );
        })
    };

    let body = rsx! {
        QualifiedNameField {
            label: "BrowseName",
            namespace: header.browse_name.namespace_index,
            name: header.browse_name.name.clone(),
            namespaces: namespaces.to_vec(),
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("BrowseName"),
            oncommit: commit,
        }
        div { class: "field",
            span { class: "field__label", "NodeId" }
            span { class: "field__static mono", "{editing.node}" }
        }
        TextField {
            label: "WriteMask",
            value: header.write_mask.bits().to_string(),
            mono: true,
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("WriteMask"),
            oncommit: editing.parsed("WriteMask", "a bit mask as a number", |bits: u32| {
                FieldValue::WriteMask(WriteMask(bits))
            }),
        }
        TextField {
            label: "UserWriteMask",
            value: header.user_write_mask.bits().to_string(),
            mono: true,
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("UserWriteMask"),
            oncommit: editing.parsed("UserWriteMask", "a bit mask as a number", |bits: u32| {
                FieldValue::UserWriteMask(WriteMask(bits))
            }),
        }
    };
    section("Identity", "badge", body)
}

fn names_section(
    editing: &Editing,
    header: &NodeHeader,
) -> Element {
    let body = rsx! {
        LocalizedTextField {
            label: "DisplayName",
            value: header.display_name.clone(),
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("DisplayName"),
            hint: editing.note_for("DisplayName"),
            oncommit: editing.on("DisplayName", FieldValue::DisplayName),
        }
        LocalizedTextField {
            label: "Description",
            value: header.description.clone(),
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("Description"),
            hint: editing.note_for("Description"),
            oncommit: editing.on("Description", FieldValue::Description),
        }
    };
    section("Names", "translate", body)
}

fn class_section(
    editing: &Editing,
    space: &AddressSpace,
    node: &Node,
) -> Element {
    let title = node.node_class().name();
    let icon = class_icon(node.node_class());
    let body = match node {
        Node::Object(object) => rsx! {
            {notifier_field(editing, "EventNotifier", object.event_notifier)}
        },
        Node::Variable(variable) => rsx! {
            {data_type_field(editing, space)}
            {value_rank_field(editing, space, variable.value_rank)}
            {array_dimensions_field(editing, &variable.array_dimensions)}
            {access_field(editing, "AccessLevel", variable.access_level, FieldValue::AccessLevel)}
            {access_field(editing, "UserAccessLevel", variable.user_access_level, FieldValue::UserAccessLevel)}
            TextField {
                label: "MinimumSamplingInterval",
                value: variable.minimum_sampling_interval.to_string(),
                mono: true,
                nonce: editing.nonce(),
                disabled: editing.read_only,
                error: editing.error_for("MinimumSamplingInterval"),
                oncommit: editing.parsed("MinimumSamplingInterval", "milliseconds as a number", FieldValue::MinimumSamplingInterval),
            }
            {bool_field(editing, "Historizing", variable.historizing, FieldValue::Historizing)}
        },
        Node::Method(method) => rsx! {
            {bool_field(editing, "Executable", method.executable, FieldValue::Executable)}
            {bool_field(editing, "UserExecutable", method.user_executable, FieldValue::UserExecutable)}
        },
        Node::View(view) => rsx! {
            {bool_field(editing, "ContainsNoLoops", view.contains_no_loops, FieldValue::ContainsNoLoops)}
            {notifier_field(editing, "EventNotifier", view.event_notifier)}
        },
        Node::ObjectType(object_type) => rsx! {
            {bool_field(editing, "IsAbstract", object_type.is_abstract, FieldValue::IsAbstract)}
        },
        Node::VariableType(variable_type) => rsx! {
            {bool_field(editing, "IsAbstract", variable_type.is_abstract, FieldValue::IsAbstract)}
            {data_type_field(editing, space)}
            {value_rank_field(editing, space, variable_type.value_rank)}
            {array_dimensions_field(editing, &variable_type.array_dimensions)}
        },
        Node::DataType(data_type) => rsx! {
            {bool_field(editing, "IsAbstract", data_type.is_abstract, FieldValue::IsAbstract)}
            div { class: "field",
                span { class: "field__label", "DataTypePurpose" }
                span { class: "field__static", "{data_type.purpose}" }
            }
            {definition_field(editing, space, data_type.definition.as_ref())}
        },
        Node::ReferenceType(reference_type) => rsx! {
            {bool_field(editing, "IsAbstract", reference_type.is_abstract, FieldValue::IsAbstract)}
            {bool_field(editing, "Symmetric", reference_type.symmetric, FieldValue::Symmetric)}
            LocalizedTextField {
                label: "InverseName",
                value: reference_type.inverse_name.clone(),
                nonce: editing.nonce(),
                disabled: editing.read_only,
                error: editing.error_for("InverseName"),
                hint: editing.note_for("InverseName"),
                oncommit: editing.on("InverseName", FieldValue::InverseName),
            }
        },
    };
    section(title, icon, body)
}

/// The type an Object or a Variable is an instance of, offered from the rules engine's own list.
fn type_definition_section(
    editing: &Editing,
    space: &AddressSpace,
    node: &Node,
) -> Element {
    if !matches!(node.node_class(), NodeClass::Object | NodeClass::Variable) {
        return rsx! {};
    }
    let current = space.type_definition(&editing.node);
    let choices = node_choices(space, &query::legal_type_definitions(space, &editing.node), current.as_ref());
    let body = rsx! {
        SelectField {
            label: "TypeDefinition",
            value: current.as_ref().map(ToString::to_string).unwrap_or_default(),
            choices,
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("TypeDefinition"),
            hint: editing.note_for("TypeDefinition"),
            oncommit: editing.targets("TypeDefinition", |node, type_definition| Operation::SetTypeDefinition {
                node,
                type_definition,
            }),
        }
    };
    section("Type", "category", body)
}

/// The fields no runtime ever shows, which makes this editor their only user interface
/// (features.md §2A).
fn metadata_section(
    editing: &Editing,
    node: &Node,
) -> Element {
    let header = node.header();
    let statuses = [ReleaseStatus::Released, ReleaseStatus::Draft, ReleaseStatus::Deprecated];
    let body = rsx! {
        TextField {
            label: "SymbolicName",
            value: header.symbolic_name.clone().unwrap_or_default(),
            mono: true,
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("SymbolicName"),
            oncommit: editing.on("SymbolicName", |text: String| FieldValue::SymbolicName(non_empty(text))),
        }
        TextField {
            label: "Category",
            value: header.category.join(", "),
            placeholder: "comma-separated",
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("Category"),
            oncommit: editing.on("Category", |text: String| FieldValue::Category(split_categories(&text))),
        }
        TextField {
            label: "Documentation",
            value: header.documentation.clone().unwrap_or_default(),
            placeholder: "a URL or a reference",
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("Documentation"),
            oncommit: editing.on("Documentation", |text: String| FieldValue::Documentation(non_empty(text))),
        }
        SelectField {
            label: "ReleaseStatus",
            value: header.release_status.name().to_owned(),
            choices: statuses.iter().map(|status| Choice {
                value: status.name().to_owned(),
                label: status.name().to_owned(),
                detail: None,
            }).collect::<Vec<_>>(),
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("ReleaseStatus"),
            oncommit: editing.parsed("ReleaseStatus", "one of the listed values", FieldValue::ReleaseStatus),
        }
        if let Some(instance) = node.instance() {
            {bool_field(editing, "DesignToolOnly", instance.design_tool_only, FieldValue::DesignToolOnly)}
        }
    };
    section("Design metadata", "draft", body)
}

fn data_type_field(
    editing: &Editing,
    space: &AddressSpace,
) -> Element {
    let current = space.data_type(&editing.node);
    let choices = node_choices(space, &query::legal_data_types_for(space, &editing.node), current.as_ref());

    rsx! {
        SelectField {
            label: "DataType",
            value: current.as_ref().map(ToString::to_string).unwrap_or_default(),
            choices,
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("DataType"),
            hint: editing.note_for("DataType"),
            oncommit: editing.targets("DataType", |node, data_type| Operation::SetDataType { node, data_type }),
        }
    }
}

fn value_rank_field(
    editing: &Editing,
    space: &AddressSpace,
    current: ValueRank,
) -> Element {
    let mut ranks = query::legal_value_ranks_for(space, &editing.node, MAX_DIMENSIONS);
    if !ranks.contains(&current) {
        ranks.insert(0, current);
    }

    rsx! {
        SelectField {
            label: "ValueRank",
            value: current.0.to_string(),
            choices: ranks.iter().map(|rank| Choice {
                value: rank.0.to_string(),
                label: format!("{rank}"),
                detail: Some(rank.0.to_string()),
            }).collect::<Vec<_>>(),
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("ValueRank"),
            hint: editing.note_for("ValueRank"),
            oncommit: editing.parsed("ValueRank", "a rank from the list", |rank: i32| FieldValue::ValueRank(ValueRank(rank))),
        }
    }
}

fn array_dimensions_field(
    editing: &Editing,
    current: &ArrayDimensions,
) -> Element {
    rsx! {
        TextField {
            label: "ArrayDimensions",
            value: current.to_string(),
            placeholder: "e.g. 0 or 3,4",
            mono: true,
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("ArrayDimensions"),
            hint: editing.note_for("ArrayDimensions"),
            oncommit: editing.parsed("ArrayDimensions", "comma-separated lengths", FieldValue::ArrayDimensions),
        }
    }
}

fn access_field(
    editing: &Editing,
    field: &'static str,
    current: AccessLevel,
    into: impl Fn(AccessLevel) -> FieldValue + 'static,
) -> Element {
    rsx! {
        MaskField {
            label: field,
            flags: AccessLevel::FLAGS.iter().map(|(name, flag)| (pretty(name), flag.0, current.contains(*flag))).collect::<Vec<_>>(),
            unknown: current.unknown_bits(),
            disabled: editing.read_only,
            error: editing.error_for(field),
            oncommit: editing.flags(field, current, into),
        }
    }
}

fn notifier_field(
    editing: &Editing,
    field: &'static str,
    current: EventNotifier,
) -> Element {
    rsx! {
        MaskField {
            label: field,
            flags: EventNotifier::FLAGS.iter().map(|(name, flag)| (pretty(name), u32::from(flag.0), current.contains(*flag))).collect::<Vec<_>>(),
            unknown: u32::from(current.unknown_bits()),
            disabled: editing.read_only,
            error: editing.error_for(field),
            oncommit: editing.notifier(field, current),
        }
    }
}

fn bool_field(
    editing: &Editing,
    field: &'static str,
    current: bool,
    into: impl Fn(bool) -> FieldValue + 'static,
) -> Element {
    rsx! {
        BoolField {
            label: field,
            value: current,
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for(field),
            hint: editing.note_for(field),
            oncommit: editing.on(field, into),
        }
    }
}

/// The DataType's own `Definition`, edited as the kind of definition it is (features.md §2B).
fn definition_field(
    editing: &Editing,
    space: &AddressSpace,
    definition: Option<&DataTypeDefinition>,
) -> Element {
    let suggested = space.browse_name(&editing.node).unwrap_or_default();
    let commit = {
        let editing = editing.clone();
        EventHandler::new(move |definition: Option<DataTypeDefinition>| {
            editing.set("DataTypeDefinition", FieldValue::DataTypeDefinition(definition));
        })
    };

    rsx! {
        DefinitionEditor {
            definition: definition.cloned(),
            suggested_name: suggested,
            namespaces: namespace_choices(space),
            data_types: field_data_types(space),
            ranks: definition_ranks(),
            nonce: editing.nonce(),
            disabled: editing.read_only,
            error: editing.error_for("DataTypeDefinition"),
            hint: editing.note_for("DataTypeDefinition"),
            onchange: commit,
        }
    }
}

pub(super) fn node_choices(
    space: &AddressSpace,
    nodes: &[NodeId],
    current: Option<&NodeId>,
) -> Vec<Choice> {
    let mut choices: Vec<Choice> = nodes.iter().map(|node| node_choice(space, node)).collect();
    choices.sort_by(|left, right| left.label.cmp(&right.label));
    if let Some(current) = current
        && !nodes.contains(current)
    {
        choices.insert(0, node_choice(space, current));
    }
    choices
}

fn node_choice(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Choice {
    Choice {
        value: node_id.to_string(),
        label: match space.node(node_id) {
            Some(node) => node.header().label(None).to_owned(),
            None => node_id.to_string(),
        },
        detail: Some(node_id.to_string()),
    }
}

pub(super) fn namespace_choices(space: &AddressSpace) -> Vec<Choice> {
    (0..space.namespaces().len())
        .filter_map(|index| NamespaceIndex::try_from(index).ok())
        .map(|index| {
            let uri = space.namespace_uri(index).unwrap_or_default();
            Choice {
                value: index.to_string(),
                label: format!("{index} · {}", short_namespace(uri)),
                detail: Some(uri.to_owned()),
            }
        })
        .collect()
}

pub(super) fn section(
    title: &str,
    icon: &'static str,
    body: Element,
) -> Element {
    rsx! {
        section { class: "form__section",
            header { class: "form__section-head",
                Icon { name: icon, class: "small" }
                span { class: "form__section-title type-label", {title.to_owned()} }
            }
            div { class: "form__fields", {body} }
        }
    }
}

fn non_empty(text: String) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn split_categories(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// `CURRENT_READ` reads as a constant, not as a label.
fn pretty(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + &characters.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect()
}

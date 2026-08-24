use indexmap::IndexMap;

use crate::attributes::value_rank::ValueRank;
use crate::ids;
use crate::nodes::common::UnknownChild;
use crate::nodes::definition::DataTypeDefinition;
use crate::nodes::method::MethodArgument;
use crate::nodes::node::Node;
use crate::nodes::reference::Reference;
use crate::nodes::translation::Translation;
use crate::types::node_id_ref::NodeIdRef;
use crate::xml::span::Layout;
use crate::xml::write::{
    RenderedNode,
    Writer,
};

/// One node element, split into the start tag, the children, and the end tag.
pub(crate) fn render(
    node: &Node,
    layout: &Layout,
    base: &str,
) -> RenderedNode {
    render_as(node, &attributes(node), layout, base)
}

/// The same, with the start tag's attributes given rather than derived, so a save can put back the
/// order and the spellings the document used for everything an edit did not touch.
pub(crate) fn render_as(
    node: &Node,
    attributes: &IndexMap<String, String>,
    layout: &Layout,
    base: &str,
) -> RenderedNode {
    let name = node
        .node_class()
        .element_name()
        .unwrap_or("UAObject")
        .to_owned();

    let mut tag = Writer::with_layout(layout, base);
    tag.start(&name);
    for (attribute, value) in attributes {
        tag.attribute(attribute, value);
    }

    let mut body = Writer::with_layout(layout, base);
    children(&mut body, node);
    let body = body.finish();

    if body.is_empty() {
        tag.closed();
        return RenderedNode {
            tag: tag.finish(),
            body,
            end: String::new(),
        };
    }
    tag.opened();
    let mut end = Writer::with_layout(layout, base);
    end.end(&name);
    RenderedNode {
        tag: tag.finish(),
        body,
        end: end.finish(),
    }
}

/// The start tag's attributes as this crate writes them, in its own order.
pub(crate) fn attributes(node: &Node) -> IndexMap<String, String> {
    let header = node.header();
    let mut tag = Tag::default();
    tag.set("NodeId", header.node_id.to_string());
    tag.set("BrowseName", header.browse_name.to_string());
    tag.unless("WriteMask", header.write_mask.bits().to_string(), "0");
    tag.unless("UserWriteMask", header.user_write_mask.bits().to_string(), "0");
    if let Some(restrictions) = header.access_restrictions {
        tag.set("AccessRestrictions", restrictions.bits().to_string());
    }
    tag.flag("HasNoPermissions", header.has_no_permissions, false);
    if let Some(symbolic_name) = &header.symbolic_name {
        tag.set("SymbolicName", symbolic_name.clone());
    }
    tag.unless("ReleaseStatus", header.release_status.name().to_owned(), "Released");
    if let Some(instance) = node.instance() {
        if let Some(parent) = &instance.parent_node_id {
            tag.set("ParentNodeId", parent.to_string());
        }
        tag.flag("DesignToolOnly", instance.design_tool_only, false);
    }
    class_attributes(&mut tag, node);
    for (name, value) in &header.unknown_attributes {
        tag.set(name, value.clone());
    }
    tag.0
}

/// A start tag being built, which is a map so a save can compare it attribute by attribute.
#[derive(Default)]
struct Tag(IndexMap<String, String>);

impl Tag {
    fn set(
        &mut self,
        name: &str,
        value: String,
    ) {
        self.0.insert(name.to_owned(), value);
    }

    /// Writes an attribute only when the value is not the one the schema assumes.
    fn unless(
        &mut self,
        name: &str,
        value: String,
        default: &str,
    ) {
        if value != default {
            self.set(name, value);
        }
    }

    fn flag(
        &mut self,
        name: &str,
        value: bool,
        default: bool,
    ) {
        if value != default {
            self.set(name, if value { "true" } else { "false" }.to_owned());
        }
    }
}

fn class_attributes(
    tag: &mut Tag,
    node: &Node,
) {
    match node {
        Node::Object(object) => {
            tag.unless("EventNotifier", object.event_notifier.bits().to_string(), "0");
        }
        Node::Variable(variable) => {
            data_type(tag, &variable.data_type);
            value_rank(tag, variable.value_rank);
            tag.unless("ArrayDimensions", variable.array_dimensions.to_string(), "");
            tag.unless("AccessLevel", variable.access_level.bits().to_string(), "1");
            tag.unless("UserAccessLevel", variable.user_access_level.bits().to_string(), "1");
            tag.unless("MinimumSamplingInterval", double(variable.minimum_sampling_interval), "0");
            tag.flag("Historizing", variable.historizing, false);
        }
        Node::Method(method) => {
            tag.flag("Executable", method.executable, true);
            tag.flag("UserExecutable", method.user_executable, true);
            if let Some(declaration) = &method.method_declaration_id {
                tag.set("MethodDeclarationId", declaration.to_string());
            }
        }
        Node::View(view) => {
            tag.flag("ContainsNoLoops", view.contains_no_loops, false);
            tag.unless("EventNotifier", view.event_notifier.bits().to_string(), "0");
        }
        Node::ObjectType(object_type) => tag.flag("IsAbstract", object_type.is_abstract, false),
        Node::VariableType(variable_type) => {
            tag.flag("IsAbstract", variable_type.is_abstract, false);
            data_type(tag, &variable_type.data_type);
            value_rank(tag, variable_type.value_rank);
            tag.unless("ArrayDimensions", variable_type.array_dimensions.to_string(), "");
        }
        Node::DataType(data) => {
            tag.flag("IsAbstract", data.is_abstract, false);
            tag.unless("Purpose", data.purpose.name().to_owned(), "Normal");
        }
        Node::ReferenceType(reference_type) => {
            tag.flag("IsAbstract", reference_type.is_abstract, false);
            tag.flag("Symmetric", reference_type.symmetric, false);
        }
    }
}

fn data_type(
    tag: &mut Tag,
    data_type: &NodeIdRef,
) {
    if *data_type != NodeIdRef::Id(ids::BASE_DATA_TYPE) {
        tag.set("DataType", data_type.to_string());
    }
}

fn value_rank(
    tag: &mut Tag,
    rank: ValueRank,
) {
    tag.unless("ValueRank", rank.0.to_string(), "-1");
}

fn children(
    writer: &mut Writer,
    node: &Node,
) {
    let header = node.header();
    let empty = header.empty_children;
    let mut order = Order::new(&header.unknown_elements);
    for text in &header.display_name {
        order.slot(writer);
        writer.localized("DisplayName", text, 1);
    }
    for text in &header.description {
        order.slot(writer);
        writer.localized("Description", text, 1);
    }
    for category in &header.category {
        order.slot(writer);
        writer.line(1);
        writer.start("Category");
        writer.contents("Category", category);
    }
    if let Some(documentation) = &header.documentation {
        order.slot(writer);
        writer.line(1);
        writer.start("Documentation");
        writer.contents("Documentation", documentation);
    }
    if !header.references.is_empty() || empty.references {
        order.slot(writer);
        references(writer, &header.references);
    }
    if !header.role_permissions.is_empty() || empty.role_permissions {
        order.slot(writer);
        writer.role_permissions(&header.role_permissions, 1);
    }
    if !header.extensions.is_empty() || empty.extensions {
        order.slot(writer);
        writer.line(1);
        writer.extension_list(&header.extensions, 1);
    }
    class_children(writer, node, &mut order);
    order.rest(writer);
    if !writer.is_empty() {
        writer.line(0);
    }
}

/// Keeps the children this crate does not model where the document had them, by counting the ones
/// it does as they go out.
struct Order<'a> {
    unknown: &'a [UnknownChild],
    next: usize,
    written: usize,
}

impl<'a> Order<'a> {
    fn new(unknown: &'a [UnknownChild]) -> Self {
        Self {
            unknown,
            next: 0,
            written: 0,
        }
    }

    /// Room for one modelled child: anything unknown that sat before it goes out first.
    fn slot(
        &mut self,
        writer: &mut Writer,
    ) {
        while let Some(child) = self
            .unknown
            .get(self.next)
            .filter(|child| child.after <= self.written)
        {
            writer.line(1);
            writer.element(&child.element);
            self.next += 1;
        }
        self.written += 1;
    }

    fn rest(
        &mut self,
        writer: &mut Writer,
    ) {
        for child in &self.unknown[self.next..] {
            writer.line(1);
            writer.element(&child.element);
        }
        self.next = self.unknown.len();
    }
}

fn class_children(
    writer: &mut Writer,
    node: &Node,
    order: &mut Order,
) {
    match node {
        Node::Variable(variable) => {
            if let Some(value) = &variable.value {
                order.slot(writer);
                writer.value(value, 1);
            }
            for translation in &variable.translations {
                order.slot(writer);
                translation_element(writer, translation);
            }
        }
        Node::VariableType(variable_type) => {
            if let Some(value) = &variable_type.value {
                order.slot(writer);
                writer.value(value, 1);
            }
        }
        Node::Method(method) => {
            for argument in &method.argument_descriptions {
                order.slot(writer);
                argument_element(writer, argument);
            }
        }
        Node::DataType(data_type) => {
            if let Some(definition) = &data_type.definition {
                order.slot(writer);
                definition_element(writer, definition);
            }
        }
        Node::ReferenceType(reference_type) => {
            for text in &reference_type.inverse_name {
                order.slot(writer);
                writer.localized("InverseName", text, 1);
            }
        }
        Node::Object(_) | Node::View(_) | Node::ObjectType(_) => {}
    }
}

fn references(
    writer: &mut Writer,
    references: &[Reference],
) {
    writer.line(1);
    writer.start("References");
    if references.is_empty() {
        writer.closed();
        return;
    }
    writer.opened();
    for reference in references {
        writer.line(2);
        writer.start("Reference");
        writer.attribute("ReferenceType", &reference.reference_type.to_string());
        writer.flag("IsForward", reference.is_forward, true);
        writer.contents("Reference", &reference.target.to_string());
    }
    writer.line(1);
    writer.end("References");
}

fn definition_element(
    writer: &mut Writer,
    definition: &DataTypeDefinition,
) {
    writer.line(1);
    writer.start("Definition");
    writer.attribute("Name", &definition.name.to_string());
    if let Some(symbolic_name) = &definition.symbolic_name {
        writer.attribute("SymbolicName", symbolic_name);
    }
    writer.flag("IsUnion", definition.is_union, false);
    writer.flag("IsOptionSet", definition.is_option_set, false);
    if let Some(base_type) = &definition.base_type {
        writer.attribute("BaseType", &base_type.to_string());
    }
    if definition.fields.is_empty() {
        writer.closed();
        return;
    }
    writer.opened();
    for field in &definition.fields {
        writer.line(2);
        writer.start("Field");
        writer.attribute("Name", &field.name);
        if let Some(symbolic_name) = &field.symbolic_name {
            writer.attribute("SymbolicName", symbolic_name);
        }
        if field.data_type != NodeIdRef::Id(ids::BASE_DATA_TYPE) {
            writer.attribute("DataType", &field.data_type.to_string());
        }
        writer.unless("ValueRank", &field.value_rank.0.to_string(), "-1");
        writer.unless("ArrayDimensions", &field.array_dimensions.to_string(), "");
        writer.unless("MaxStringLength", &field.max_string_length.to_string(), "0");
        writer.unless("Value", &field.value.to_string(), "-1");
        writer.flag("IsOptional", field.is_optional, false);
        writer.flag("AllowSubTypes", field.allow_sub_types, false);
        if field.display_name.is_empty() && field.description.is_empty() {
            writer.closed();
            continue;
        }
        writer.opened();
        for text in &field.display_name {
            writer.localized("DisplayName", text, 3);
        }
        for text in &field.description {
            writer.localized("Description", text, 3);
        }
        writer.line(2);
        writer.end("Field");
    }
    writer.line(1);
    writer.end("Definition");
}

fn argument_element(
    writer: &mut Writer,
    argument: &MethodArgument,
) {
    writer.line(1);
    writer.start("ArgumentDescription");
    if argument.name.is_none() && argument.description.is_empty() {
        writer.closed();
        return;
    }
    writer.opened();
    if let Some(name) = &argument.name {
        writer.line(2);
        writer.start("Name");
        writer.contents("Name", name);
    }
    for text in &argument.description {
        writer.localized("Description", text, 2);
    }
    writer.line(1);
    writer.end("ArgumentDescription");
}

fn translation_element(
    writer: &mut Writer,
    translation: &Translation,
) {
    writer.line(1);
    writer.start("Translation");
    match translation {
        Translation::Text(texts) if texts.is_empty() => {
            writer.closed();
        }
        Translation::Fields(fields) if fields.is_empty() => {
            writer.closed();
        }
        Translation::Text(texts) => {
            writer.opened();
            for text in texts {
                writer.localized("Text", text, 2);
            }
            writer.line(1);
            writer.end("Translation");
        }
        Translation::Fields(fields) => {
            writer.opened();
            for field in fields {
                writer.line(2);
                writer.start("Field");
                writer.attribute("Name", &field.name);
                if field.text.is_empty() {
                    writer.closed();
                    continue;
                }
                writer.opened();
                for text in &field.text {
                    writer.localized("Text", text, 3);
                }
                writer.line(2);
                writer.end("Field");
            }
            writer.line(1);
            writer.end("Translation");
        }
    }
}

/// `xs:double` writes a whole number without a fractional part, and the three specials in capitals.
fn double(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "INF" } else { "-INF" }.to_owned();
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{value:.0}");
    }
    format!("{value}")
}

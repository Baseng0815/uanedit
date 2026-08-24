use crate::attributes::access_level::AccessLevel;
use crate::attributes::event_notifier::EventNotifier;
use crate::attributes::value_rank::ValueRank;
use crate::attributes::write_mask::WriteMask;
use crate::error::DocumentError;
use crate::ids;
use crate::nodes::common::{
    InstanceHeader,
    NodeHeader,
    UnknownChild,
};
use crate::nodes::data_type::DataType;
use crate::nodes::definition::{
    DataTypeDefinition,
    DataTypeField,
};
use crate::nodes::method::{
    Method,
    MethodArgument,
};
use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::nodes::object::Object;
use crate::nodes::object_type::ObjectType;
use crate::nodes::reference::Reference;
use crate::nodes::reference_type::ReferenceType;
use crate::nodes::translation::{
    StructureTranslation,
    Translation,
};
use crate::nodes::variable::Variable;
use crate::nodes::variable_type::VariableType;
use crate::nodes::view::View;
use crate::report::{
    Diagnosis,
    PreservedKind,
};
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::NodeId;
use crate::types::node_id_ref::NodeIdRef;
use crate::xml::cursor::Tag;
use crate::xml::read::{
    Attributes,
    Reading,
};

impl<'a> Reading<'a> {
    /// One node element, or nothing when the element is not a node class the schema defines.
    pub(super) fn read_node(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Option<Node>, DocumentError> {
        let Some(class) = NodeClass::from_element_name(tag.local_name()) else {
            self.cursor.skip(tag)?;
            return Ok(None);
        };
        let mut attributes = self.attributes(tag);
        let mut node = self.class(class, &mut attributes);
        node.header_mut().unknown_attributes = self.keep_unknown(attributes);
        if !tag.empty {
            while let Some(child) = self.cursor.tag()? {
                self.node_child(&mut node, &child)?;
            }
        }
        self.owner = None;
        Ok(Some(node))
    }

    fn class(
        &mut self,
        class: NodeClass,
        attributes: &mut Attributes,
    ) -> Node {
        let header = self.header(attributes);
        match class {
            NodeClass::Object => Object {
                header,
                instance: self.instance(attributes),
                event_notifier: self.event_notifier(attributes),
            }
            .into(),
            NodeClass::Variable => {
                let instance = self.instance(attributes);
                Variable {
                    header,
                    instance,
                    value: None,
                    translations: Vec::new(),
                    data_type: self.data_type(attributes),
                    value_rank: self.value_rank(attributes),
                    array_dimensions: self
                        .attribute(attributes, "ArrayDimensions")
                        .unwrap_or_default(),
                    access_level: self
                        .number::<u32>(attributes, "AccessLevel")
                        .map_or(AccessLevel::DEFAULT, Into::into),
                    user_access_level: self
                        .number::<u32>(attributes, "UserAccessLevel")
                        .map_or(AccessLevel::DEFAULT, Into::into),
                    minimum_sampling_interval: self
                        .double(attributes, "MinimumSamplingInterval")
                        .unwrap_or_default(),
                    historizing: self.boolean(attributes, "Historizing").unwrap_or_default(),
                }
                .into()
            }
            NodeClass::Method => {
                let instance = self.instance(attributes);
                Method {
                    header,
                    instance,
                    argument_descriptions: Vec::new(),
                    executable: self.boolean(attributes, "Executable").unwrap_or(true),
                    user_executable: self.boolean(attributes, "UserExecutable").unwrap_or(true),
                    method_declaration_id: self.reference(attributes, "MethodDeclarationId"),
                }
                .into()
            }
            NodeClass::View => {
                let instance = self.instance(attributes);
                View {
                    header,
                    instance,
                    contains_no_loops: self
                        .boolean(attributes, "ContainsNoLoops")
                        .unwrap_or_default(),
                    event_notifier: self.event_notifier(attributes),
                }
                .into()
            }
            NodeClass::ObjectType => ObjectType {
                header,
                is_abstract: self.is_abstract(attributes),
            }
            .into(),
            NodeClass::VariableType => VariableType {
                header,
                is_abstract: self.is_abstract(attributes),
                value: None,
                data_type: self.data_type(attributes),
                value_rank: self.value_rank(attributes),
                array_dimensions: self
                    .attribute(attributes, "ArrayDimensions")
                    .unwrap_or_default(),
            }
            .into(),
            NodeClass::DataType => DataType {
                header,
                is_abstract: self.is_abstract(attributes),
                definition: None,
                purpose: self.attribute(attributes, "Purpose").unwrap_or_default(),
            }
            .into(),
            NodeClass::ReferenceType | NodeClass::Unspecified => ReferenceType {
                header,
                is_abstract: self.is_abstract(attributes),
                symmetric: self.boolean(attributes, "Symmetric").unwrap_or_default(),
                inverse_name: Vec::new(),
            }
            .into(),
        }
    }

    fn header(
        &mut self,
        attributes: &mut Attributes,
    ) -> NodeHeader {
        let node_id: NodeId = self.required(attributes, "NodeId").unwrap_or_default();
        self.owner = Some(node_id.clone());
        NodeHeader {
            node_id,
            browse_name: self.required(attributes, "BrowseName").unwrap_or_default(),
            write_mask: self.write_mask(attributes, "WriteMask"),
            user_write_mask: self.write_mask(attributes, "UserWriteMask"),
            access_restrictions: self
                .number::<u16>(attributes, "AccessRestrictions")
                .map(Into::into),
            has_no_permissions: self
                .boolean(attributes, "HasNoPermissions")
                .unwrap_or_default(),
            symbolic_name: attributes.take("SymbolicName"),
            release_status: self
                .attribute(attributes, "ReleaseStatus")
                .unwrap_or_default(),
            ..NodeHeader::default()
        }
    }

    fn instance(
        &mut self,
        attributes: &mut Attributes,
    ) -> InstanceHeader {
        InstanceHeader {
            parent_node_id: self.reference(attributes, "ParentNodeId"),
            design_tool_only: self
                .boolean(attributes, "DesignToolOnly")
                .unwrap_or_default(),
        }
    }

    fn is_abstract(
        &mut self,
        attributes: &mut Attributes,
    ) -> bool {
        self.boolean(attributes, "IsAbstract").unwrap_or_default()
    }

    fn data_type(
        &mut self,
        attributes: &mut Attributes,
    ) -> NodeIdRef {
        self.reference(attributes, "DataType")
            .unwrap_or(NodeIdRef::Id(ids::BASE_DATA_TYPE))
    }

    fn value_rank(
        &mut self,
        attributes: &mut Attributes,
    ) -> ValueRank {
        self.number::<i32>(attributes, "ValueRank")
            .map_or(ValueRank::SCALAR, ValueRank)
    }

    fn event_notifier(
        &mut self,
        attributes: &mut Attributes,
    ) -> EventNotifier {
        EventNotifier::from(
            self.number::<u8>(attributes, "EventNotifier")
                .unwrap_or_default(),
        )
    }

    fn write_mask(
        &mut self,
        attributes: &mut Attributes,
        name: &str,
    ) -> WriteMask {
        WriteMask::from(self.number::<u32>(attributes, name).unwrap_or_default())
    }

    fn node_child(
        &mut self,
        node: &mut Node,
        child: &Tag<'a>,
    ) -> Result<(), DocumentError> {
        let name = child.local_name().to_owned();
        match (&mut *node, name.as_str()) {
            (Node::Variable(variable), "Value") => {
                variable.value = Some(self.value(child)?);
                return Ok(());
            }
            (Node::Variable(variable), "Translation") => {
                let translation = self.translation(child)?;
                variable.translations.push(translation);
                return Ok(());
            }
            (Node::VariableType(variable_type), "Value") => {
                variable_type.value = Some(self.value(child)?);
                return Ok(());
            }
            (Node::Method(method), "ArgumentDescription") => {
                let argument = self.argument(child)?;
                method.argument_descriptions.push(argument);
                return Ok(());
            }
            (Node::DataType(data_type), "Definition") => {
                data_type.definition = Some(self.definition(child)?);
                return Ok(());
            }
            (Node::ReferenceType(reference_type), "InverseName") => {
                let text = self.localized_text(child)?;
                reference_type.inverse_name.push(text);
                return Ok(());
            }
            _ => {}
        }
        if self.common_child(node.header_mut(), child, &name)? {
            return Ok(());
        }
        self.preserve(PreservedKind::UnknownElement, &name, child.span.start);
        let element = self.cursor.element(child)?;
        let after = modelled_children(node);
        node.header_mut()
            .unknown_elements
            .push(UnknownChild { element, after });
        Ok(())
    }

    /// One of the children every node class shares, or `false` when the schema does not name it.
    fn common_child(
        &mut self,
        header: &mut NodeHeader,
        child: &Tag<'a>,
        name: &str,
    ) -> Result<bool, DocumentError> {
        match name {
            "DisplayName" => header.display_name.push(self.localized_text(child)?),
            "Description" => header.description.push(self.localized_text(child)?),
            "Category" => header.category.push(self.cursor.text(child)?),
            "Documentation" => header.documentation = Some(self.cursor.text(child)?),
            "References" => {
                header.references = self.references(child)?;
                header.empty_children.references = header.references.is_empty();
            }
            "RolePermissions" => {
                header.role_permissions = self.role_permissions(child)?;
                header.empty_children.role_permissions = header.role_permissions.is_empty();
            }
            "Extensions" => {
                header.extensions = self.extensions(child)?;
                header.empty_children.extensions = header.extensions.is_empty();
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn localized_text(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<LocalizedText, DocumentError> {
        let mut attributes = self.attributes(tag);
        let locale = attributes.take("Locale");
        Ok(LocalizedText {
            locale,
            text: self.cursor.text(tag)?,
        })
    }

    fn references(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Vec<Reference>, DocumentError> {
        let mut references = Vec::new();
        if tag.empty {
            return Ok(references);
        }
        while let Some(child) = self.cursor.tag()? {
            if child.local_name() != "Reference" {
                self.preserve(PreservedKind::UnknownElement, child.local_name(), child.span.start);
                self.cursor.skip(&child)?;
                continue;
            }
            let mut attributes = self.attributes(&child);
            let reference_type = match self.reference(&mut attributes, "ReferenceType") {
                Some(reference_type) => reference_type,
                None => {
                    self.find(
                        child.span.start,
                        Diagnosis::MissingAttribute {
                            name: "ReferenceType".to_owned(),
                        },
                    );
                    NodeIdRef::default()
                }
            };
            let is_forward = self.boolean(&mut attributes, "IsForward").unwrap_or(true);
            let target = self.cursor.text(&child)?;
            references.push(Reference {
                reference_type,
                is_forward,
                target: super::node_id_ref(target.trim()),
            });
        }
        Ok(references)
    }

    fn definition(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<DataTypeDefinition, DocumentError> {
        let mut attributes = self.attributes(tag);
        let mut definition = DataTypeDefinition {
            name: self.required(&mut attributes, "Name").unwrap_or_default(),
            symbolic_name: attributes.take("SymbolicName"),
            is_union: self.boolean(&mut attributes, "IsUnion").unwrap_or_default(),
            is_option_set: self
                .boolean(&mut attributes, "IsOptionSet")
                .unwrap_or_default(),
            base_type: self.attribute(&mut attributes, "BaseType"),
            fields: Vec::new(),
        };
        if tag.empty {
            return Ok(definition);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Field" => definition.fields.push(self.field(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(definition)
    }

    fn field(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<DataTypeField, DocumentError> {
        let mut attributes = self.attributes(tag);
        let mut field = DataTypeField {
            name: attributes.take("Name").unwrap_or_default(),
            symbolic_name: attributes.take("SymbolicName"),
            display_name: Vec::new(),
            description: Vec::new(),
            data_type: self.data_type(&mut attributes),
            value_rank: self.value_rank(&mut attributes),
            array_dimensions: self
                .attribute(&mut attributes, "ArrayDimensions")
                .unwrap_or_default(),
            max_string_length: self
                .number(&mut attributes, "MaxStringLength")
                .unwrap_or_default(),
            value: self.number(&mut attributes, "Value").unwrap_or(-1),
            is_optional: self
                .boolean(&mut attributes, "IsOptional")
                .unwrap_or_default(),
            allow_sub_types: self
                .boolean(&mut attributes, "AllowSubTypes")
                .unwrap_or_default(),
        };
        if tag.empty {
            return Ok(field);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "DisplayName" => field.display_name.push(self.localized_text(&child)?),
                "Description" => field.description.push(self.localized_text(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(field)
    }

    fn argument(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<MethodArgument, DocumentError> {
        let mut argument = MethodArgument::default();
        if tag.empty {
            return Ok(argument);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Name" => argument.name = Some(self.cursor.text(&child)?),
                "Description" => argument.description.push(self.localized_text(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(argument)
    }

    fn translation(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Translation, DocumentError> {
        let mut texts = Vec::new();
        let mut fields = Vec::new();
        if tag.empty {
            return Ok(Translation::Text(texts));
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Text" => texts.push(self.localized_text(&child)?),
                "Field" => fields.push(self.structure_translation(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(match fields.is_empty() {
            true => Translation::Text(texts),
            false => Translation::Fields(fields),
        })
    }

    fn structure_translation(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<StructureTranslation, DocumentError> {
        let mut attributes = self.attributes(tag);
        let mut translation = StructureTranslation {
            name: attributes.take("Name").unwrap_or_default(),
            text: Vec::new(),
        };
        if tag.empty {
            return Ok(translation);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Text" => translation.text.push(self.localized_text(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(translation)
    }
}

/// How many children this crate models the node has so far, counted in the order the writer emits
/// them, so an unknown element can be put back where it was.
fn modelled_children(node: &Node) -> usize {
    let header = node.header();
    let empty = header.empty_children;
    header.display_name.len()
        + header.description.len()
        + header.category.len()
        + usize::from(header.documentation.is_some())
        + usize::from(!header.references.is_empty() || empty.references)
        + usize::from(!header.role_permissions.is_empty() || empty.role_permissions)
        + usize::from(!header.extensions.is_empty() || empty.extensions)
        + match node {
            Node::Variable(variable) => usize::from(variable.value.is_some()) + variable.translations.len(),
            Node::VariableType(variable_type) => usize::from(variable_type.value.is_some()),
            Node::Method(method) => method.argument_descriptions.len(),
            Node::DataType(data_type) => usize::from(data_type.definition.is_some()),
            Node::ReferenceType(reference_type) => reference_type.inverse_name.len(),
            Node::Object(_) | Node::View(_) | Node::ObjectType(_) => 0,
        }
}

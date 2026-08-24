use core::str::FromStr;

use crate::error::{
    DocumentError,
    ParseError,
};
use crate::report::{
    Diagnosis,
    PreservedKind,
};
use crate::types::built_in::BuiltInType;
use crate::types::byte_string::ByteString;
use crate::types::data_value::DataValue;
use crate::types::diagnostic_info::DiagnosticInfo;
use crate::types::extension_object::ExtensionObject;
use crate::types::guid::Guid;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;
use crate::types::status_code::StatusCode;
use crate::types::variant::{
    Variant,
    VariantArray,
    VariantMatrix,
};
use crate::types::xml::XmlElement;
use crate::xml::cursor::Tag;
use crate::xml::read::{
    Reading,
    boolean,
    double,
};

impl<'a> Reading<'a> {
    /// The `Value` element of a Variable or VariableType, which holds at most one value.
    pub(super) fn value(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Variant, DocumentError> {
        if tag.empty {
            return Ok(Variant::Null);
        }
        let mut value = Variant::Null;
        while let Some(child) = self.cursor.tag()? {
            value = self.variant(&child)?;
        }
        Ok(value)
    }

    pub(crate) fn variant(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Variant, DocumentError> {
        let name = tag.local_name().to_owned();
        if carries_attributes(tag) {
            return Ok(Variant::Raw(self.cursor.element(tag)?));
        }
        if let Some(element_type) = name.strip_prefix("ListOf").and_then(BuiltInType::from_name) {
            return Ok(Variant::Array(self.array(tag, element_type)?));
        }
        if name == "Matrix" {
            return self.matrix(tag);
        }
        match BuiltInType::from_name(&name) {
            Some(built_in) => self.scalar(tag, built_in),
            None => {
                self.preserve(PreservedKind::OpaqueValue, &name, tag.span.start);
                Ok(Variant::Raw(self.cursor.element(tag)?))
            }
        }
    }

    fn scalar(
        &mut self,
        tag: &Tag<'a>,
        built_in: BuiltInType,
    ) -> Result<Variant, DocumentError> {
        Ok(match built_in {
            BuiltInType::Null => {
                self.cursor.skip(tag)?;
                Variant::Null
            }
            BuiltInType::Boolean => Variant::Boolean(self.flag(tag)?),
            BuiltInType::SByte => Variant::SByte(self.integer(tag)?),
            BuiltInType::Byte => Variant::Byte(self.integer(tag)?),
            BuiltInType::Int16 => Variant::Int16(self.integer(tag)?),
            BuiltInType::UInt16 => Variant::UInt16(self.integer(tag)?),
            BuiltInType::Int32 => Variant::Int32(self.integer(tag)?),
            BuiltInType::UInt32 => Variant::UInt32(self.integer(tag)?),
            BuiltInType::Int64 => Variant::Int64(self.integer(tag)?),
            BuiltInType::UInt64 => Variant::UInt64(self.integer(tag)?),
            #[expect(clippy::cast_possible_truncation, reason = "a Float is written as a Double is")]
            BuiltInType::Float => Variant::Float(self.real(tag)? as f32),
            BuiltInType::Double => Variant::Double(self.real(tag)?),
            BuiltInType::String => Variant::String(self.cursor.text(tag)?),
            BuiltInType::DateTime => Variant::DateTime(self.spelled(tag)?),
            BuiltInType::Guid => Variant::Guid(self.guid(tag)?),
            BuiltInType::ByteString => Variant::ByteString(self.byte_string(tag)?),
            BuiltInType::XmlElement => self.xml_element(tag)?,
            BuiltInType::NodeId => Variant::NodeId(self.identifier(tag)?),
            BuiltInType::ExpandedNodeId => Variant::ExpandedNodeId(self.identifier(tag)?),
            BuiltInType::StatusCode => Variant::StatusCode(self.status_code(tag)?),
            BuiltInType::QualifiedName => Variant::QualifiedName(self.qualified_name(tag)?),
            BuiltInType::LocalizedText => Variant::LocalizedText(self.text_value(tag)?),
            BuiltInType::ExtensionObject => Variant::ExtensionObject(self.extension_object(tag)?),
            BuiltInType::DataValue => Variant::DataValue(Box::new(self.data_value(tag)?)),
            BuiltInType::Variant => Variant::Variant(Box::new(self.value(tag)?)),
            BuiltInType::DiagnosticInfo => Variant::DiagnosticInfo(Box::new(self.diagnostic_info(tag)?)),
        })
    }

    fn array(
        &mut self,
        tag: &Tag<'a>,
        element_type: BuiltInType,
    ) -> Result<VariantArray, DocumentError> {
        let mut values = Vec::new();
        if !tag.empty {
            while let Some(child) = self.cursor.tag()? {
                values.push(self.variant(&child)?);
            }
        }
        Ok(VariantArray { element_type, values })
    }

    /// A `Matrix`, whose element list the specification calls `Elements` and the schema `Value`.
    fn matrix(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Variant, DocumentError> {
        let mut matrix = VariantMatrix::default();
        let mut seen_dimensions = false;
        if tag.empty {
            return Ok(Variant::Matrix(matrix));
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Dimensions" => {
                    seen_dimensions = true;
                    let dimensions = self.array(&child, BuiltInType::Int32)?;
                    matrix.dimensions = dimensions
                        .values
                        .iter()
                        .map(|value| match value {
                            Variant::Int32(dimension) => u32::try_from(*dimension).unwrap_or_default(),
                            _ => 0,
                        })
                        .collect();
                }
                "Elements" | "Value" => {
                    matrix.elements_first = !seen_dimensions;
                    matrix.elements = self.elements(&child)?;
                }
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(Variant::Matrix(matrix))
    }

    /// A matrix's flattened values, whose element type is whatever the first of them is.
    fn elements(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<VariantArray, DocumentError> {
        let mut elements = VariantArray::default();
        if tag.empty {
            return Ok(elements);
        }
        while let Some(child) = self.cursor.tag()? {
            let value = self.variant(&child)?;
            if elements.values.is_empty() {
                elements.element_type = value.built_in_type();
            }
            elements.values.push(value);
        }
        Ok(elements)
    }

    fn flag(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<bool, DocumentError> {
        let offset = tag.span.start;
        let text = self.cursor.text(tag)?;
        Ok(match boolean(&text) {
            Some(flag) => flag,
            None => {
                self.malformed(offset, "Boolean", &text, ParseError::Boolean(text.clone()));
                false
            }
        })
    }

    fn integer<T: FromStr + Default>(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<T, DocumentError> {
        let offset = tag.span.start;
        let name = tag.local_name().to_owned();
        let text = self.cursor.text(tag)?;
        Ok(match text.trim().parse() {
            Ok(number) => number,
            Err(_) => {
                self.malformed(offset, &name, &text, ParseError::Integer(text.clone()));
                T::default()
            }
        })
    }

    fn real(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<f64, DocumentError> {
        let offset = tag.span.start;
        let name = tag.local_name().to_owned();
        let text = self.cursor.text(tag)?;
        Ok(match double(&text) {
            Some(number) => number,
            None => {
                self.malformed(offset, &name, &text, ParseError::Double(text.clone()));
                0.0
            }
        })
    }

    fn spelled<T: FromStr<Err = ParseError> + Default>(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<T, DocumentError> {
        Ok(self.parsed_text(tag)?.unwrap_or_default())
    }

    fn byte_string(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<ByteString, DocumentError> {
        let offset = tag.span.start;
        let text = self.cursor.text(tag)?;
        Ok(match ByteString::decode(&text) {
            Ok(bytes) => bytes,
            Err(source) => {
                self.malformed(offset, "ByteString", &text, source);
                ByteString::NULL
            }
        })
    }

    fn guid(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Guid, DocumentError> {
        let mut guid = Guid::NULL;
        if tag.empty {
            return Ok(guid);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "String" => guid = self.spelled(&child)?,
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(guid)
    }

    /// A NodeId or ExpandedNodeId, both of which carry the whole value in one `Identifier`.
    fn identifier<T: FromStr<Err = ParseError> + Default>(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<T, DocumentError> {
        let mut value = T::default();
        if tag.empty {
            return Ok(value);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Identifier" => value = self.spelled(&child)?,
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(value)
    }

    fn status_code(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<StatusCode, DocumentError> {
        let mut code = StatusCode::GOOD;
        if tag.empty {
            return Ok(code);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Code" => code = StatusCode(self.integer(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(code)
    }

    fn qualified_name(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<QualifiedName, DocumentError> {
        let mut name = QualifiedName::default();
        if tag.empty {
            return Ok(name);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "NamespaceIndex" => {
                    name.namespace_index = self.integer(&child)?;
                    name.explicit_index = true;
                }
                "Name" => name.name = self.cursor.text(&child)?,
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(name)
    }

    fn text_value(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<LocalizedText, DocumentError> {
        let mut text = LocalizedText::default();
        if tag.empty {
            return Ok(text);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Locale" => text.locale = Some(self.cursor.text(&child)?),
                "Text" => text.text = self.cursor.text(&child)?,
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(text)
    }

    fn extension_object(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<ExtensionObject, DocumentError> {
        let mut object = ExtensionObject::default();
        if tag.empty {
            return Ok(object);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "TypeId" => object.type_id = self.identifier::<NodeId>(&child)?,
                "Body" => object.body = self.extension_body(&child)?,
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(object)
    }

    /// An ExtensionObject's body, which this crate keeps as XML because the model under edit
    /// defines its shape.
    fn extension_body(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Option<XmlElement>, DocumentError> {
        let mut body = None;
        if tag.empty {
            return Ok(body);
        }
        while let Some(child) = self.cursor.tag()? {
            body = Some(self.cursor.element(&child)?);
        }
        Ok(body)
    }

    fn xml_element(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Variant, DocumentError> {
        let element = self.cursor.element(tag)?;
        let payload = element.elements().next().cloned();
        Ok(match payload {
            Some(inner) => Variant::XmlElement(inner),
            None => Variant::Raw(element),
        })
    }

    fn data_value(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<DataValue, DocumentError> {
        let mut value = DataValue::default();
        if tag.empty {
            return Ok(value);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Value" => value.value = Some(self.value(&child)?),
                "StatusCode" => value.status = Some(self.status_code(&child)?),
                "SourceTimestamp" => value.source_timestamp = Some(self.spelled(&child)?),
                "SourcePicoseconds" => value.source_picoseconds = Some(self.integer(&child)?),
                "ServerTimestamp" => value.server_timestamp = Some(self.spelled(&child)?),
                "ServerPicoseconds" => value.server_picoseconds = Some(self.integer(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(value)
    }

    fn diagnostic_info(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<DiagnosticInfo, DocumentError> {
        let mut info = DiagnosticInfo::default();
        if tag.empty {
            return Ok(info);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "SymbolicId" => info.symbolic_id = Some(self.integer(&child)?),
                "NamespaceUri" => info.namespace_uri = Some(self.integer(&child)?),
                "Locale" => info.locale = Some(self.integer(&child)?),
                "LocalizedText" => info.localized_text = Some(self.integer(&child)?),
                "AdditionalInfo" => info.additional_info = Some(self.cursor.text(&child)?),
                "InnerStatusCode" => info.inner_status_code = Some(self.status_code(&child)?),
                "InnerDiagnosticInfo" => {
                    info.inner_diagnostic_info = Some(Box::new(self.diagnostic_info(&child)?));
                }
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(info)
    }

    fn malformed(
        &mut self,
        offset: usize,
        element: &str,
        value: &str,
        source: ParseError,
    ) {
        self.find(
            offset,
            Diagnosis::MalformedElement {
                element: element.to_owned(),
                value: value.to_owned(),
                source,
            },
        );
    }
}

/// True when an element carries anything but a namespace declaration, which this crate does not
/// model and therefore keeps the whole value verbatim for.
fn carries_attributes(tag: &Tag<'_>) -> bool {
    tag.attributes()
        .keys()
        .any(|name| name != "xmlns" && !name.starts_with("xmlns:"))
}

use crate::emit::{
    TYPES_NAMESPACE,
    Writer,
};
use crate::types::data_value::DataValue;
use crate::types::diagnostic_info::DiagnosticInfo;
use crate::types::extension_object::ExtensionObject;
use crate::types::localized_text::LocalizedText;
use crate::types::qualified_name::QualifiedName;
use crate::types::variant::{
    Variant,
    VariantArray,
    VariantMatrix,
};

/// A value as its `Value` element, for showing it where the file itself is not at hand.
///
/// The same encoding a save writes, in this crate's own layout rather than the document's.
pub fn value_xml(value: &Variant) -> String {
    let mut writer = Writer::new("\n", "  ", "");
    writer.value(value, 0);
    writer.finish().trim_start().to_owned()
}

impl Writer {
    /// A Variable's `Value` element, whose payload is where the types namespace is declared.
    pub(crate) fn value(
        &mut self,
        value: &Variant,
        depth: usize,
    ) {
        self.line(depth);
        self.start("Value");
        if value.is_null() {
            self.closed();
            return;
        }
        self.opened();
        self.variant(value, depth + 1, true);
        self.line(depth);
        self.end("Value");
    }

    fn variant(
        &mut self,
        value: &Variant,
        depth: usize,
        root: bool,
    ) {
        match value {
            Variant::Null => {}
            Variant::Raw(element) => {
                self.line(depth);
                self.element(element);
            }
            Variant::Array(array) => self.array(array, depth, root),
            Variant::Matrix(matrix) => self.matrix(matrix, depth, root),
            Variant::Boolean(flag) => self.simple(value, if *flag { "true" } else { "false" }, depth, root),
            Variant::SByte(number) => self.simple(value, &number.to_string(), depth, root),
            Variant::Byte(number) => self.simple(value, &number.to_string(), depth, root),
            Variant::Int16(number) => self.simple(value, &number.to_string(), depth, root),
            Variant::UInt16(number) => self.simple(value, &number.to_string(), depth, root),
            Variant::Int32(number) => self.simple(value, &number.to_string(), depth, root),
            Variant::UInt32(number) => self.simple(value, &number.to_string(), depth, root),
            Variant::Int64(number) => self.simple(value, &number.to_string(), depth, root),
            Variant::UInt64(number) => self.simple(value, &number.to_string(), depth, root),
            Variant::Float(number) => self.simple(value, &real(f64::from(*number)), depth, root),
            Variant::Double(number) => self.simple(value, &real(*number), depth, root),
            Variant::String(text) => self.simple(value, text, depth, root),
            Variant::DateTime(instant) => self.simple(value, &instant.to_string(), depth, root),
            Variant::ByteString(bytes) => {
                self.simple(value, &bytes.encode().unwrap_or_default(), depth, root);
            }
            Variant::Guid(guid) => {
                let spelling = guid.to_string();
                self.composite(value, depth, root, |writer| {
                    writer.tagged_text("String", &spelling, depth + 1);
                });
            }
            Variant::XmlElement(element) => {
                let element = element.clone();
                self.composite(value, depth, root, |writer| {
                    writer.line(depth + 1);
                    writer.element(&element);
                });
            }
            Variant::NodeId(node_id) => {
                let spelling = node_id.to_string();
                self.composite(value, depth, root, |writer| {
                    writer.tagged_text("Identifier", &spelling, depth + 1);
                });
            }
            Variant::ExpandedNodeId(node_id) => {
                let spelling = node_id.to_string();
                self.composite(value, depth, root, |writer| {
                    writer.tagged_text("Identifier", &spelling, depth + 1);
                });
            }
            Variant::StatusCode(status) => {
                let code = status.0.to_string();
                self.composite(value, depth, root, |writer| {
                    writer.tagged_text("Code", &code, depth + 1);
                });
            }
            Variant::QualifiedName(name) => {
                let name = name.clone();
                self.composite(value, depth, root, |writer| writer.qualified_name(&name, depth + 1));
            }
            Variant::LocalizedText(text) => {
                let text = text.clone();
                self.composite(value, depth, root, |writer| writer.text_value(&text, depth + 1));
            }
            Variant::ExtensionObject(object) => {
                let object = object.clone();
                self.composite(value, depth, root, |writer| {
                    writer.extension_object(&object, depth + 1);
                });
            }
            Variant::DataValue(data) => {
                let data = data.clone();
                self.composite(value, depth, root, |writer| writer.data_value(&data, depth + 1));
            }
            Variant::Variant(inner) => {
                let inner = inner.clone();
                self.composite(value, depth, root, |writer| writer.value(&inner, depth + 1));
            }
            Variant::DiagnosticInfo(info) => {
                let info = info.clone();
                self.composite(value, depth, root, |writer| {
                    writer.diagnostic_info(&info, depth + 1);
                });
            }
        }
    }

    fn array(
        &mut self,
        array: &VariantArray,
        depth: usize,
        root: bool,
    ) {
        let name = array.element_type.list_name();
        self.line(depth);
        self.start(&name);
        if root {
            self.attribute("xmlns", TYPES_NAMESPACE);
        }
        if array.values.is_empty() {
            self.closed();
            return;
        }
        self.opened();
        for value in &array.values {
            self.variant(value, depth + 1, false);
        }
        self.line(depth);
        self.end(&name);
    }

    /// A `Matrix`, written with the element list the specification's prose names.
    fn matrix(
        &mut self,
        matrix: &VariantMatrix,
        depth: usize,
        root: bool,
    ) {
        self.line(depth);
        self.start("Matrix");
        if root {
            self.attribute("xmlns", TYPES_NAMESPACE);
        }
        self.opened();
        let dimensions = |writer: &mut Self| {
            writer.line(depth + 1);
            writer.start("Dimensions");
            writer.opened();
            for dimension in &matrix.dimensions {
                writer.tagged_text("Int32", &dimension.to_string(), depth + 2);
            }
            writer.line(depth + 1);
            writer.end("Dimensions");
        };
        let elements = |writer: &mut Self| {
            writer.line(depth + 1);
            writer.start("Elements");
            writer.opened();
            for value in &matrix.elements.values {
                writer.variant(value, depth + 2, false);
            }
            writer.line(depth + 1);
            writer.end("Elements");
        };
        if matrix.elements_first {
            elements(self);
            dimensions(self);
        } else {
            dimensions(self);
            elements(self);
        }
        self.line(depth);
        self.end("Matrix");
    }

    fn simple(
        &mut self,
        value: &Variant,
        text: &str,
        depth: usize,
        root: bool,
    ) {
        let name = value.built_in_type().name();
        self.line(depth);
        self.start(name);
        if root {
            self.attribute("xmlns", TYPES_NAMESPACE);
        }
        if text.is_empty() {
            self.closed();
            return;
        }
        self.opened();
        self.text(text);
        self.end(name);
    }

    fn composite(
        &mut self,
        value: &Variant,
        depth: usize,
        root: bool,
        body: impl FnOnce(&mut Self),
    ) {
        let name = value.built_in_type().name();
        self.line(depth);
        self.start(name);
        if root {
            self.attribute("xmlns", TYPES_NAMESPACE);
        }
        self.opened();
        body(self);
        self.line(depth);
        self.end(name);
    }

    fn tagged_text(
        &mut self,
        name: &str,
        text: &str,
        depth: usize,
    ) {
        self.line(depth);
        self.start(name);
        self.contents(name, text);
    }

    fn qualified_name(
        &mut self,
        name: &QualifiedName,
        depth: usize,
    ) {
        if name.namespace_index != 0 || name.explicit_index {
            self.tagged_text("NamespaceIndex", &name.namespace_index.to_string(), depth);
        }
        self.tagged_text("Name", &name.name, depth);
    }

    fn text_value(
        &mut self,
        text: &LocalizedText,
        depth: usize,
    ) {
        if let Some(locale) = &text.locale {
            self.tagged_text("Locale", locale, depth);
        }
        self.tagged_text("Text", &text.text, depth);
    }

    fn extension_object(
        &mut self,
        object: &ExtensionObject,
        depth: usize,
    ) {
        self.line(depth);
        self.start("TypeId");
        self.opened();
        self.tagged_text("Identifier", &object.type_id.to_string(), depth + 1);
        self.line(depth);
        self.end("TypeId");
        if let Some(body) = &object.body {
            self.line(depth);
            self.start("Body");
            self.opened();
            self.line(depth + 1);
            self.element(body);
            self.line(depth);
            self.end("Body");
        }
    }

    fn data_value(
        &mut self,
        data: &DataValue,
        depth: usize,
    ) {
        if let Some(value) = &data.value {
            self.value(value, depth);
        }
        if let Some(status) = &data.status {
            self.line(depth);
            self.start("StatusCode");
            self.opened();
            self.tagged_text("Code", &status.0.to_string(), depth + 1);
            self.line(depth);
            self.end("StatusCode");
        }
        if let Some(timestamp) = &data.source_timestamp {
            self.tagged_text("SourceTimestamp", &timestamp.to_string(), depth);
        }
        if let Some(picoseconds) = data.source_picoseconds {
            self.tagged_text("SourcePicoseconds", &picoseconds.to_string(), depth);
        }
        if let Some(timestamp) = &data.server_timestamp {
            self.tagged_text("ServerTimestamp", &timestamp.to_string(), depth);
        }
        if let Some(picoseconds) = data.server_picoseconds {
            self.tagged_text("ServerPicoseconds", &picoseconds.to_string(), depth);
        }
    }

    fn diagnostic_info(
        &mut self,
        info: &DiagnosticInfo,
        depth: usize,
    ) {
        for (name, index) in [
            ("SymbolicId", info.symbolic_id),
            ("NamespaceUri", info.namespace_uri),
            ("Locale", info.locale),
            ("LocalizedText", info.localized_text),
        ] {
            if let Some(index) = index {
                self.tagged_text(name, &index.to_string(), depth);
            }
        }
        if let Some(additional) = &info.additional_info {
            self.tagged_text("AdditionalInfo", additional, depth);
        }
        if let Some(status) = &info.inner_status_code {
            self.line(depth);
            self.start("InnerStatusCode");
            self.opened();
            self.tagged_text("Code", &status.0.to_string(), depth + 1);
            self.line(depth);
            self.end("InnerStatusCode");
        }
        if let Some(inner) = &info.inner_diagnostic_info {
            self.line(depth);
            self.start("InnerDiagnosticInfo");
            self.opened();
            self.diagnostic_info(inner, depth + 1);
            self.line(depth);
            self.end("InnerDiagnosticInfo");
        }
    }
}

/// `xs:double` and `xs:float` spell their three special values in capitals.
fn real(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "INF" } else { "-INF" }.to_owned();
    }
    format!("{value}")
}

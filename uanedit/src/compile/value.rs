use crate::compile::structured::StructType;
use crate::compile::text;
use crate::emit::Writer;
use crate::types::built_in::BuiltInType;
use crate::types::extension_object::ExtensionObject;
use crate::types::variant::{
    Variant,
    VariantArray,
};
use crate::types::xml::XmlElement;

/// The lines a Variable's value contributes: setup before the addNode call, cleanup after it, and
/// statics at file scope.
pub(super) struct ValueCode {
    pub(super) setup: Vec<String>,
    pub(super) cleanup: Vec<String>,
    pub(super) globals: Vec<String>,
}

/// The code writing `value` into `attr.value`, or None when open62541 has no encoding for it.
pub(super) fn variable_value(
    value: &Variant,
    stem: &str,
) -> Option<ValueCode> {
    let name = format!("{stem}_variant_DataContents");
    match value {
        Variant::ExtensionObject(object) => structured_scalar(object, stem),
        Variant::Array(array) => array_value(array, &name),
        Variant::Matrix(matrix) => array_value(&matrix.elements, &name),
        scalar => scalar_value(scalar, &name),
    }
}

fn structured_scalar(
    object: &ExtensionObject,
    stem: &str,
) -> Option<ValueCode> {
    let struct_type = StructType::of(object)?;
    let index = struct_type.types_index();
    let var = format!("{stem}_None_0");
    let mut setup = vec![
        String::new(),
        format!("UA_STACKARRAY({}, {var}, 1);", struct_type.c_type()),
        format!("UA_init({var}, &UA_TYPES[UA_TYPES_{index}]);"),
    ];
    setup.extend(struct_type.field_lines(object, &format!("{var}->"), &var)?);
    setup.push(format!("UA_Variant_setScalar(&attr.value, {var}, &UA_TYPES[UA_TYPES_{index}]);"));
    Some(ValueCode {
        setup,
        cleanup: Vec::new(),
        globals: Vec::new(),
    })
}

fn structured_array(
    array: &VariantArray,
    name: &str,
) -> Option<ValueCode> {
    let mut objects = Vec::new();
    for value in &array.values {
        let Variant::ExtensionObject(object) = value else {
            return None;
        };
        objects.push(object);
    }
    let Some(first) = objects.first() else {
        return Some(ValueCode {
            setup: vec![
                "UA_Variant_setArray(&attr.value, NULL, (UA_Int32) 0, &UA_TYPES[UA_TYPES_EXTENSIONOBJECT]);".to_owned(),
            ],
            cleanup: Vec::new(),
            globals: Vec::new(),
        });
    };
    let struct_type = StructType::of(first)?;
    if objects
        .iter()
        .any(|object| StructType::of(object) != Some(struct_type))
    {
        return None;
    }
    let index = struct_type.types_index();
    let mut setup = vec![format!("{} {name}[{}];", struct_type.c_type(), objects.len())];
    for (position, object) in objects.iter().enumerate() {
        setup.push(format!("UA_init(&{name}[{position}], &UA_TYPES[UA_TYPES_{index}]);"));
        setup.extend(struct_type.field_lines(
            object,
            &format!("{name}[{position}]."),
            &format!("{name}_{position}"),
        )?);
    }
    setup.push(format!(
        "UA_Variant_setArray(&attr.value, &{name}, (UA_Int32) {}, &UA_TYPES[UA_TYPES_{index}]);",
        objects.len()
    ));
    Some(ValueCode {
        setup,
        cleanup: Vec::new(),
        globals: Vec::new(),
    })
}

fn scalar_value(
    value: &Variant,
    name: &str,
) -> Option<ValueCode> {
    let type_name = ua_type(value.built_in_type())?;
    let upper = types_index(value.built_in_type())?;
    let (assignment, mut cleanup, globals) = scalar_assignment(value, name)?;
    let mut setup = vec![
        format!("{type_name} *{name} =  {type_name}_new();"),
        format!("if (!{name}) return UA_STATUSCODE_BADOUTOFMEMORY;"),
        format!("{type_name}_init({name});"),
    ];
    setup.extend(assignment);
    setup.push(format!("UA_Variant_setScalar(&attr.value, {name}, &UA_TYPES[UA_TYPES_{upper}]);"));
    cleanup.push(format!("{type_name}_delete({name});"));
    Some(ValueCode {
        setup,
        cleanup,
        globals,
    })
}

/// The assignment into the heap variable, with any cleanup and file-scope statics it needs.
fn scalar_assignment(
    value: &Variant,
    name: &str,
) -> Option<(Vec<String>, Vec<String>, Vec<String>)> {
    let plain = |line: String| Some((vec![line], Vec::new(), Vec::new()));
    match value {
        Variant::ByteString(bytes) => match bytes.as_bytes() {
            Some(data) if !data.is_empty() => {
                let bytes_list = data
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                let globals = vec![format!(
                    "static const UA_Byte {name}_byteArray[{}] = {{{bytes_list}}};",
                    data.len()
                )];
                let setup = vec![
                    format!("{name}->length = {};", data.len()),
                    format!("{name}->data = (UA_Byte *)(void*)(uintptr_t){name}_byteArray;"),
                ];
                let cleanup = vec![format!("{name}->data = NULL;"), format!("{name}->length = 0;")];
                Some((setup, cleanup, globals))
            }
            _ => plain(format!("*{name} = UA_BYTESTRING_NULL;")),
        },
        Variant::String(value) => plain(format!("*{name} = UA_STRING_ALLOC({});", text::c_string(value))),
        Variant::XmlElement(element) => {
            plain(format!("*{name} = UA_XMLELEMENT_ALLOC({});", text::c_string(&xml_text(element))))
        }
        Variant::LocalizedText(value) => plain(format!("*{name} = {};", text::localized_text_alloc(value))),
        Variant::QualifiedName(value) => plain(format!("*{name} = {};", text::qualified_name_alloc(value))),
        other => plain(format!("*{name} = {};", element_literal(other)?)),
    }
}

fn array_value(
    array: &VariantArray,
    name: &str,
) -> Option<ValueCode> {
    if array.element_type == BuiltInType::ExtensionObject {
        return structured_array(array, name);
    }
    let upper = types_index(array.element_type)?;
    if array.values.is_empty() {
        return Some(ValueCode {
            setup: vec![format!(
                "UA_Variant_setArray(&attr.value, NULL, (UA_Int32) 0, &UA_TYPES[UA_TYPES_{upper}]);"
            )],
            cleanup: Vec::new(),
            globals: Vec::new(),
        });
    }
    let type_name = ua_type(array.element_type)?;
    let mut setup = vec![format!("{type_name} {name}[{}];", array.values.len())];
    for (index, value) in array.values.iter().enumerate() {
        setup.push(format!("{name}[{index}] = {};", element_literal(value)?));
    }
    setup.push(format!(
        "UA_Variant_setArray(&attr.value, &{name}, (UA_Int32) {}, &UA_TYPES[UA_TYPES_{upper}]);",
        array.values.len()
    ));
    Some(ValueCode {
        setup,
        cleanup: Vec::new(),
        globals: Vec::new(),
    })
}

/// The right-hand side of one element assignment, in the stack (non-alloc) forms.
fn element_literal(value: &Variant) -> Option<String> {
    match value {
        Variant::Boolean(value) => Some(format!("(UA_Boolean) {value}")),
        Variant::SByte(value) => Some(format!("(UA_SByte) {value}")),
        Variant::Byte(value) => Some(format!("(UA_Byte) {value}")),
        Variant::Int16(value) => Some(format!("(UA_Int16) {value}")),
        Variant::UInt16(value) => Some(format!("(UA_UInt16) {value}")),
        Variant::Int32(value) => Some(format!("(UA_Int32) {value}")),
        Variant::UInt32(value) => Some(format!("(UA_UInt32) {value}")),
        Variant::Int64(value) => Some(format!("(UA_Int64) {value}LL")),
        Variant::UInt64(value) => Some(format!("(UA_UInt64) {value}ULL")),
        Variant::Float(value) => value.is_finite().then(|| format!("(UA_Float) {value}")),
        Variant::Double(value) => value.is_finite().then(|| format!("(UA_Double) {value}")),
        Variant::String(value) => Some(format!("UA_STRING({})", text::c_string(value))),
        Variant::XmlElement(element) => Some(format!("UA_XMLELEMENT({})", text::c_string(&xml_text(element)))),
        Variant::DateTime(value) => Some(text::date_time(value)),
        Variant::NodeId(value) => text::node_id(value),
        Variant::ExpandedNodeId(value) => (value.is_local() && value.namespace_uri.is_none())
            .then(|| text::expanded_node_id(&value.node_id))
            .flatten(),
        Variant::QualifiedName(value) => Some(text::qualified_name(value)),
        Variant::LocalizedText(value) => Some(text::localized_text(value)),
        _ => None,
    }
}

fn xml_text(element: &XmlElement) -> String {
    let mut writer = Writer::new("", "", "");
    writer.element(element);
    writer.finish()
}

fn ua_type(built_in: BuiltInType) -> Option<&'static str> {
    match built_in {
        BuiltInType::Null => None,
        BuiltInType::Boolean => Some("UA_Boolean"),
        BuiltInType::SByte => Some("UA_SByte"),
        BuiltInType::Byte => Some("UA_Byte"),
        BuiltInType::Int16 => Some("UA_Int16"),
        BuiltInType::UInt16 => Some("UA_UInt16"),
        BuiltInType::Int32 => Some("UA_Int32"),
        BuiltInType::UInt32 => Some("UA_UInt32"),
        BuiltInType::Int64 => Some("UA_Int64"),
        BuiltInType::UInt64 => Some("UA_UInt64"),
        BuiltInType::Float => Some("UA_Float"),
        BuiltInType::Double => Some("UA_Double"),
        BuiltInType::String => Some("UA_String"),
        BuiltInType::DateTime => Some("UA_DateTime"),
        BuiltInType::ByteString => Some("UA_ByteString"),
        BuiltInType::XmlElement => Some("UA_XmlElement"),
        BuiltInType::NodeId => Some("UA_NodeId"),
        BuiltInType::ExpandedNodeId => Some("UA_ExpandedNodeId"),
        BuiltInType::QualifiedName => Some("UA_QualifiedName"),
        BuiltInType::LocalizedText => Some("UA_LocalizedText"),
        BuiltInType::Guid
        | BuiltInType::StatusCode
        | BuiltInType::ExtensionObject
        | BuiltInType::DataValue
        | BuiltInType::Variant
        | BuiltInType::DiagnosticInfo => None,
    }
}

fn types_index(built_in: BuiltInType) -> Option<&'static str> {
    ua_type(built_in)?;
    match built_in {
        BuiltInType::Null => None,
        other => Some(match other {
            BuiltInType::Boolean => "BOOLEAN",
            BuiltInType::SByte => "SBYTE",
            BuiltInType::Byte => "BYTE",
            BuiltInType::Int16 => "INT16",
            BuiltInType::UInt16 => "UINT16",
            BuiltInType::Int32 => "INT32",
            BuiltInType::UInt32 => "UINT32",
            BuiltInType::Int64 => "INT64",
            BuiltInType::UInt64 => "UINT64",
            BuiltInType::Float => "FLOAT",
            BuiltInType::Double => "DOUBLE",
            BuiltInType::String => "STRING",
            BuiltInType::DateTime => "DATETIME",
            BuiltInType::ByteString => "BYTESTRING",
            BuiltInType::XmlElement => "XMLELEMENT",
            BuiltInType::NodeId => "NODEID",
            BuiltInType::ExpandedNodeId => "EXPANDEDNODEID",
            BuiltInType::QualifiedName => "QUALIFIEDNAME",
            BuiltInType::LocalizedText => "LOCALIZEDTEXT",
            _ => return None,
        }),
    }
}

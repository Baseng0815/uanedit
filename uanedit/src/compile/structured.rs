use crate::compile::text;
use crate::types::extension_object::ExtensionObject;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::{
    Identifier,
    NodeId,
};
use crate::types::xml::XmlElement;

/// The namespace-0 structures open62541 carries in `UA_TYPES`, which lets their ExtensionObject
/// values compile to field assignments instead of `/* Cannot encode the value */`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StructType {
    Argument,
    EnumValueType,
    Range,
    EuInformation,
}

impl StructType {
    /// Matched against the DataType node or either of its encoding nodes, since files write any
    /// of the three as a TypeId.
    pub(super) fn of(object: &ExtensionObject) -> Option<Self> {
        if object.type_id.namespace_index != 0 {
            return None;
        }
        let Identifier::Numeric(identifier) = object.type_id.identifier else {
            return None;
        };
        match identifier {
            296..=298 => Some(Self::Argument),
            7594 | 7616 | 8251 => Some(Self::EnumValueType),
            884..=886 => Some(Self::Range),
            887..=889 => Some(Self::EuInformation),
            _ => None,
        }
    }

    pub(super) fn c_type(self) -> &'static str {
        match self {
            Self::Argument => "UA_Argument",
            Self::EnumValueType => "UA_EnumValueType",
            Self::Range => "UA_Range",
            Self::EuInformation => "UA_EUInformation",
        }
    }

    pub(super) fn types_index(self) -> &'static str {
        match self {
            Self::Argument => "ARGUMENT",
            Self::EnumValueType => "ENUMVALUETYPE",
            Self::Range => "RANGE",
            Self::EuInformation => "EUINFORMATION",
        }
    }

    /// The assignments filling one instance reached through `access`; `var` names any helper
    /// variables the fields need.
    pub(super) fn field_lines(
        self,
        object: &ExtensionObject,
        access: &str,
        var: &str,
    ) -> Option<Vec<String>> {
        let body = object.body.as_ref()?;
        let mut lines = Vec::new();
        match self {
            Self::Argument => {
                if let Some(name) = body.child("Name") {
                    lines.push(format!("{access}name = UA_STRING({});", text::c_string(&name.text())));
                }
                if let Some(data_type) = node_id_child(body, "DataType") {
                    lines.push(format!("{access}dataType = {};", text::node_id(&data_type)?));
                }
                if let Some(rank) = body.child("ValueRank") {
                    let rank: i32 = rank.text().trim().parse().ok()?;
                    lines.push(format!("{access}valueRank = (UA_Int32) {rank};"));
                }
                let dimensions = array_dimensions(body)?;
                if !dimensions.is_empty() {
                    lines.push(format!("UA_STACKARRAY(UA_UInt32, {var}_arrayDimensions, {});", dimensions.len()));
                    for (index, length) in dimensions.iter().enumerate() {
                        lines.push(format!("{var}_arrayDimensions[{index}] = {length};"));
                    }
                    lines.push(format!("{access}arrayDimensionsSize = {};", dimensions.len()));
                    lines.push(format!("{access}arrayDimensions = {var}_arrayDimensions;"));
                }
                if let Some(description) = localized_child(body, "Description") {
                    lines.push(format!("{access}description = {};", text::localized_text(&description)));
                }
            }
            Self::EnumValueType => {
                if let Some(value) = body.child("Value") {
                    let value: i64 = value.text().trim().parse().ok()?;
                    lines.push(format!("{access}value = (UA_Int64) {value}LL;"));
                }
                if let Some(display) = localized_child(body, "DisplayName") {
                    lines.push(format!("{access}displayName = {};", text::localized_text(&display)));
                }
                if let Some(description) = localized_child(body, "Description") {
                    lines.push(format!("{access}description = {};", text::localized_text(&description)));
                }
            }
            Self::Range => {
                lines.push(format!("{access}low = (UA_Double) {};", double_child(body, "Low")?));
                lines.push(format!("{access}high = (UA_Double) {};", double_child(body, "High")?));
            }
            Self::EuInformation => {
                if let Some(uri) = body.child("NamespaceUri") {
                    lines.push(format!("{access}namespaceUri = UA_STRING({});", text::c_string(&uri.text())));
                }
                if let Some(unit) = body.child("UnitId") {
                    let unit: i32 = unit.text().trim().parse().ok()?;
                    lines.push(format!("{access}unitId = (UA_Int32) {unit};"));
                }
                if let Some(display) = localized_child(body, "DisplayName") {
                    lines.push(format!("{access}displayName = {};", text::localized_text(&display)));
                }
                if let Some(description) = localized_child(body, "Description") {
                    lines.push(format!("{access}description = {};", text::localized_text(&description)));
                }
            }
        }
        Some(lines)
    }
}

fn node_id_child(
    body: &XmlElement,
    name: &str,
) -> Option<NodeId> {
    let identifier = body.child(name)?.child("Identifier")?;
    identifier.text().trim().parse().ok()
}

fn localized_child(
    body: &XmlElement,
    name: &str,
) -> Option<LocalizedText> {
    let element = body.child(name)?;
    let locale = element.child("Locale").map(|locale| locale.text());
    let text = element.child("Text").map(|text| text.text());
    if locale.is_none() && text.is_none() {
        return None;
    }
    Some(LocalizedText {
        locale,
        text: text.unwrap_or_default(),
    })
}

fn double_child(
    body: &XmlElement,
    name: &str,
) -> Option<f64> {
    let value: f64 = body.child(name)?.text().trim().parse().ok()?;
    value.is_finite().then_some(value)
}

fn array_dimensions(body: &XmlElement) -> Option<Vec<u32>> {
    let Some(dimensions) = body.child("ArrayDimensions") else {
        return Some(Vec::new());
    };
    dimensions
        .elements()
        .map(|length| length.text().trim().parse().ok())
        .collect()
}

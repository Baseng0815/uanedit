use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::ids;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id_ref::NodeIdRef;
use crate::types::qualified_name::QualifiedName;

/// The fields of a structured or enumerated DataType (OPC 10000-3 §8.47).
///
/// A nodeset writes the Structure and Enum forms through one element, telling them apart by
/// whether the fields carry a DataType or a Value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataTypeDefinition {
    pub name: QualifiedName,
    pub symbolic_name: Option<String>,
    pub is_union: bool,
    pub is_option_set: bool,
    /// Obsolete in the schema, so it is kept only to be written back out.
    pub base_type: Option<QualifiedName>,
    pub fields: Vec<DataTypeField>,
}

/// One field of a [`DataTypeDefinition`]: a structure member, or an enumeration value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataTypeField {
    pub name: String,
    pub symbolic_name: Option<String>,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    pub data_type: NodeIdRef,
    pub value_rank: ValueRank,
    pub array_dimensions: ArrayDimensions,
    pub max_string_length: u32,
    /// The enumeration value, or the bit index for an OptionSet; -1 on a structure field.
    pub value: i32,
    pub is_optional: bool,
    pub allow_sub_types: bool,
}

/// How a structured DataType's fields are laid out (OPC 10000-3 §8.49).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum StructureType {
    #[default]
    Structure = 0,
    StructureWithOptionalFields = 1,
    Union     = 2,
    StructureWithSubtypedValues = 3,
    UnionWithSubtypedValues = 4,
}

impl DataTypeDefinition {
    /// True when the fields name enumeration values rather than structure members.
    pub fn is_enumeration(&self) -> bool {
        !self.fields.is_empty() && self.fields.iter().all(|field| field.value >= 0)
    }

    /// The layout the fields describe, which OPC 10000-3 models as a separate attribute but a
    /// nodeset leaves implicit.
    pub fn structure_type(&self) -> StructureType {
        let subtyped = self.fields.iter().any(|field| field.allow_sub_types);
        match (self.is_union, subtyped) {
            (true, true) => StructureType::UnionWithSubtypedValues,
            (true, false) => StructureType::Union,
            (false, true) => StructureType::StructureWithSubtypedValues,
            (false, false) if self.fields.iter().any(|field| field.is_optional) => {
                StructureType::StructureWithOptionalFields
            }
            (false, false) => StructureType::Structure,
        }
    }

    pub fn field(
        &self,
        name: &str,
    ) -> Option<&DataTypeField> {
        self.fields.iter().find(|field| field.name == name)
    }
}

impl Default for DataTypeField {
    /// Matches the UANodeSet schema's attribute defaults, not Rust's zero values.
    fn default() -> Self {
        Self {
            name: String::new(),
            symbolic_name: None,
            display_name: Vec::new(),
            description: Vec::new(),
            data_type: NodeIdRef::Id(ids::BASE_DATA_TYPE),
            value_rank: ValueRank::SCALAR,
            array_dimensions: ArrayDimensions::default(),
            max_string_length: 0,
            value: -1,
            is_optional: false,
            allow_sub_types: false,
        }
    }
}

impl StructureType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::StructureWithOptionalFields => "StructureWithOptionalFields",
            Self::Union => "Union",
            Self::StructureWithSubtypedValues => "StructureWithSubtypedValues",
            Self::UnionWithSubtypedValues => "UnionWithSubtypedValues",
        }
    }
}

impl fmt::Display for StructureType {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.name())
    }
}

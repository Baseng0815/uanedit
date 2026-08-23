use serde::{
    Deserialize,
    Serialize,
};

use crate::types::built_in::BuiltInType;
use crate::types::byte_string::ByteString;
use crate::types::data_value::DataValue;
use crate::types::date_time::DateTime;
use crate::types::diagnostic_info::DiagnosticInfo;
use crate::types::expanded_node_id::ExpandedNodeId;
use crate::types::extension_object::ExtensionObject;
use crate::types::guid::Guid;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;
use crate::types::status_code::StatusCode;
use crate::types::xml::XmlElement;

/// A value of any built-in type, scalar or array (OPC 10000-6 §5.2.2.16).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Variant {
    #[default]
    Null,
    Boolean(bool),
    SByte(i8),
    Byte(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Int64(i64),
    UInt64(u64),
    Float(f32),
    Double(f64),
    String(String),
    DateTime(DateTime),
    Guid(Guid),
    ByteString(ByteString),
    XmlElement(XmlElement),
    NodeId(NodeId),
    ExpandedNodeId(ExpandedNodeId),
    StatusCode(StatusCode),
    QualifiedName(QualifiedName),
    LocalizedText(LocalizedText),
    ExtensionObject(ExtensionObject),
    DataValue(Box<DataValue>),
    Variant(Box<Variant>),
    DiagnosticInfo(Box<DiagnosticInfo>),
    Array(VariantArray),
    Matrix(VariantMatrix),
    /// A value element this crate does not recognise, kept so it survives a save.
    Raw(XmlElement),
}

/// A one-dimensional array, written `ListOf<Type>`; the element type is held so an empty array
/// still knows what it is an array of.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VariantArray {
    pub element_type: BuiltInType,
    pub values: Vec<Variant>,
}

/// A multi-dimensional array, written as its dimensions plus the elements in row-major order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VariantMatrix {
    pub dimensions: Vec<u32>,
    pub elements: VariantArray,
    /// Set when the document wrote `Elements` before `Dimensions`, as stacks before 1.5.378 did
    /// and no reader tolerates transposed.
    pub elements_first: bool,
}

impl Variant {
    /// The built-in type of this value, or of its elements when it is an array.
    pub fn built_in_type(&self) -> BuiltInType {
        match self {
            Self::Null | Self::Raw(_) => BuiltInType::Null,
            Self::Boolean(_) => BuiltInType::Boolean,
            Self::SByte(_) => BuiltInType::SByte,
            Self::Byte(_) => BuiltInType::Byte,
            Self::Int16(_) => BuiltInType::Int16,
            Self::UInt16(_) => BuiltInType::UInt16,
            Self::Int32(_) => BuiltInType::Int32,
            Self::UInt32(_) => BuiltInType::UInt32,
            Self::Int64(_) => BuiltInType::Int64,
            Self::UInt64(_) => BuiltInType::UInt64,
            Self::Float(_) => BuiltInType::Float,
            Self::Double(_) => BuiltInType::Double,
            Self::String(_) => BuiltInType::String,
            Self::DateTime(_) => BuiltInType::DateTime,
            Self::Guid(_) => BuiltInType::Guid,
            Self::ByteString(_) => BuiltInType::ByteString,
            Self::XmlElement(_) => BuiltInType::XmlElement,
            Self::NodeId(_) => BuiltInType::NodeId,
            Self::ExpandedNodeId(_) => BuiltInType::ExpandedNodeId,
            Self::StatusCode(_) => BuiltInType::StatusCode,
            Self::QualifiedName(_) => BuiltInType::QualifiedName,
            Self::LocalizedText(_) => BuiltInType::LocalizedText,
            Self::ExtensionObject(_) => BuiltInType::ExtensionObject,
            Self::DataValue(_) => BuiltInType::DataValue,
            Self::Variant(_) => BuiltInType::Variant,
            Self::DiagnosticInfo(_) => BuiltInType::DiagnosticInfo,
            Self::Array(array) => array.element_type,
            Self::Matrix(matrix) => matrix.elements.element_type,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_) | Self::Matrix(_))
    }

    /// The number of dimensions, in the sense the ValueRank attribute uses.
    pub fn dimensions(&self) -> i32 {
        match self {
            Self::Array(_) => 1,
            Self::Matrix(matrix) => matrix.dimensions.len().try_into().unwrap_or(i32::MAX),
            _ => -1,
        }
    }
}

impl VariantArray {
    pub fn new(
        element_type: BuiltInType,
        values: Vec<Variant>,
    ) -> Self {
        Self { element_type, values }
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
}

impl VariantMatrix {
    /// The element count the dimensions call for, which a well-formed matrix matches.
    pub fn expected_len(&self) -> usize {
        self.dimensions
            .iter()
            .map(|dimension| *dimension as usize)
            .product::<usize>()
    }
}

macro_rules! variant_from {
    ($($source:ty => $variant:ident),* $(,)?) => {
        $(
            impl From<$source> for Variant {
                fn from(value: $source) -> Self {
                    Self::$variant(value)
                }
            }
        )*
    };
}

variant_from! {
    bool => Boolean,
    i8 => SByte,
    u8 => Byte,
    i16 => Int16,
    u16 => UInt16,
    i32 => Int32,
    u32 => UInt32,
    i64 => Int64,
    u64 => UInt64,
    f32 => Float,
    f64 => Double,
    String => String,
    DateTime => DateTime,
    Guid => Guid,
    ByteString => ByteString,
    NodeId => NodeId,
    ExpandedNodeId => ExpandedNodeId,
    StatusCode => StatusCode,
    QualifiedName => QualifiedName,
    LocalizedText => LocalizedText,
    ExtensionObject => ExtensionObject,
}

impl From<&str> for Variant {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

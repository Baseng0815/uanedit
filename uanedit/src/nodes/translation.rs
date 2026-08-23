use serde::{
    Deserialize,
    Serialize,
};

use crate::types::localized_text::LocalizedText;

/// Translations of a Variable's value, either of the value itself or of a structure's fields
/// (UANodeSet `TranslationType`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Translation {
    Text(Vec<LocalizedText>),
    Fields(Vec<StructureTranslation>),
}

/// Translations of one named field of a structured value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructureTranslation {
    pub name: String,
    pub text: Vec<LocalizedText>,
}

impl Default for Translation {
    fn default() -> Self {
        Self::Text(Vec::new())
    }
}

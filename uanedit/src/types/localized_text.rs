use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

/// A LocalizedText (OPC 10000-3 §8.5); an absent locale is distinct from an empty one.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LocalizedText {
    pub locale: Option<String>,
    pub text: String,
}

impl LocalizedText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            locale: None,
            text: text.into(),
        }
    }

    pub fn localized(
        locale: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            locale: Some(locale.into()),
            text: text.into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.locale.is_none() && self.text.is_empty()
    }
}

impl fmt::Display for LocalizedText {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl From<&str> for LocalizedText {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<String> for LocalizedText {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

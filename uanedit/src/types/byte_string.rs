use core::cmp::Ordering;
use core::fmt;
use core::hash::{
    Hash,
    Hasher,
};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{
    Deserialize,
    Serialize,
};

use crate::error::ParseError;

/// A ByteString, which the spec distinguishes from an empty one by being null.
///
/// `xs:base64Binary` permits interior whitespace, and the standard nodeset relies on it — its two
/// large ByteStrings are line-wrapped. The text that was read is therefore kept verbatim and
/// replayed on write; it takes no part in equality.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ByteString {
    value: Option<Vec<u8>>,
    lexical: Option<Box<str>>,
}

impl ByteString {
    pub const NULL: Self = Self {
        value: None,
        lexical: None,
    };

    pub fn as_bytes(&self) -> Option<&[u8]> {
        self.value.as_deref()
    }

    pub fn lexical(&self) -> Option<&str> {
        self.lexical.as_deref()
    }

    pub fn is_null(&self) -> bool {
        self.value.is_none()
    }

    pub fn len(&self) -> usize {
        self.value.as_ref().map_or(0, Vec::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drops the remembered spelling, so writing this value produces canonical padded base64.
    pub fn canonicalized(self) -> Self {
        Self {
            value: self.value,
            lexical: None,
        }
    }

    pub fn decode(text: &str) -> Result<Self, ParseError> {
        let compact: String = text.chars().filter(|char| !char.is_whitespace()).collect();
        let value = STANDARD
            .decode(&compact)
            .map_err(|_| ParseError::Base64(text.to_owned()))?;
        Ok(Self {
            value: Some(value),
            lexical: Some(text.into()),
        })
    }

    pub fn encode(&self) -> Option<String> {
        match (&self.lexical, &self.value) {
            (Some(lexical), _) => Some(lexical.to_string()),
            (None, Some(bytes)) => Some(STANDARD.encode(bytes)),
            (None, None) => None,
        }
    }
}

impl From<Vec<u8>> for ByteString {
    fn from(value: Vec<u8>) -> Self {
        Self {
            value: Some(value),
            lexical: None,
        }
    }
}

impl PartialEq for ByteString {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.value == other.value
    }
}

impl Eq for ByteString {}

impl Hash for ByteString {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.value.hash(state);
    }
}

impl Ord for ByteString {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for ByteString {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for ByteString {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(&self.encode().unwrap_or_default())
    }
}

use core::cmp::Ordering;
use core::fmt;
use core::hash::{
    Hash,
    Hasher,
};
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

use crate::error::ParseError;

/// A GUID, holding the sixteen bytes in the order the textual form spells them.
///
/// No clause fixes the letter case — the spec's own examples disagree — so the form that was read
/// is kept and replayed on write; it takes no part in equality.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Guid {
    bytes: [u8; 16],
    lexical: Option<Box<str>>,
}

const GROUPS: [usize; 5] = [4, 2, 2, 2, 6];

impl Guid {
    pub const NULL: Self = Self {
        bytes: [0; 16],
        lexical: None,
    };

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self { bytes, lexical: None }
    }

    /// The bytes in textual order, which is not the field order the binary encoding uses.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.bytes
    }

    pub fn lexical(&self) -> Option<&str> {
        self.lexical.as_deref()
    }

    /// Drops the remembered spelling, so writing this value produces the canonical lower-case form.
    pub fn canonicalized(self) -> Self {
        Self::from_bytes(self.bytes)
    }

    pub fn is_null(&self) -> bool {
        self.bytes == [0; 16]
    }
}

impl PartialEq for Guid {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for Guid {}

impl Hash for Guid {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.bytes.hash(state);
    }
}

impl Ord for Guid {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.bytes.cmp(&other.bytes)
    }
}

impl PartialOrd for Guid {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Guid {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if let Some(lexical) = &self.lexical {
            return f.write_str(lexical);
        }
        let mut byte = self.bytes.iter();
        for (index, group) in GROUPS.iter().enumerate() {
            if index > 0 {
                f.write_str("-")?;
            }
            for _ in 0..*group {
                let Some(value) = byte.next() else { break };
                write!(f, "{value:02x}")?;
            }
        }
        Ok(())
    }
}

impl FromStr for Guid {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || ParseError::Guid(text.to_owned());
        let mut bytes = [0u8; 16];
        let mut written = 0;
        let mut digits = text.chars();

        for (index, group) in GROUPS.iter().enumerate() {
            if index > 0 && digits.next() != Some('-') {
                return Err(invalid());
            }
            for _ in 0..*group {
                let high = digits.next().ok_or_else(invalid)?;
                let low = digits.next().ok_or_else(invalid)?;
                if !high.is_ascii_hexdigit() || !low.is_ascii_hexdigit() {
                    return Err(invalid());
                }
                let pair = [high as u8, low as u8];
                let pair = core::str::from_utf8(&pair).map_err(|_| invalid())?;
                bytes[written] = u8::from_str_radix(pair, 16).map_err(|_| invalid())?;
                written += 1;
            }
        }
        if digits.next().is_some() {
            return Err(invalid());
        }
        Ok(Self {
            bytes,
            lexical: Some(text.into()),
        })
    }
}

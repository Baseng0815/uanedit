use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

/// A StatusCode (OPC 10000-4 §7.39); the top two bits carry the severity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StatusCode(pub u32);

/// The severity a StatusCode's top two bits encode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Good,
    Uncertain,
    Bad,
}

impl StatusCode {
    pub const GOOD: Self = Self(0x0000_0000);
    pub const UNCERTAIN: Self = Self(0x4000_0000);
    pub const BAD: Self = Self(0x8000_0000);

    pub fn severity(self) -> Severity {
        match self.0 >> 30 {
            0 => Severity::Good,
            1 => Severity::Uncertain,
            _ => Severity::Bad,
        }
    }

    pub fn is_good(self) -> bool {
        self.severity() == Severity::Good
    }

    pub fn is_uncertain(self) -> bool {
        self.severity() == Severity::Uncertain
    }

    pub fn is_bad(self) -> bool {
        self.severity() == Severity::Bad
    }

    /// The code without its informational bits, which is what identifies the condition.
    pub fn sub_code(self) -> u32 {
        self.0 & 0xFFFF_0000
    }
}

impl From<u32> for StatusCode {
    fn from(code: u32) -> Self {
        Self(code)
    }
}

impl fmt::Display for StatusCode {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

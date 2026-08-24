use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

use crate::rules::code::DiagnosticCode;
use crate::types::node_id::NodeId;

/// What an acknowledgement is keyed by: the rule, the node, and a hash of the facts the finding
/// rests on.
///
/// The node keeps an acknowledgement attached across unrelated edits; the hash lets it expire when
/// what the rule complained about actually changes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Fingerprint(String);

impl Fingerprint {
    pub fn new(
        code: DiagnosticCode,
        node: &NodeId,
        facts: &str,
    ) -> Self {
        Self(format!("{}@{}#{:016x}", code.id(), node, hash(facts.as_bytes())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn code(&self) -> Option<DiagnosticCode> {
        let (id, _) = self.0.split_once('@')?;
        DiagnosticCode::from_id(id)
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// FNV-1a, written out rather than taken from the standard library because an acknowledgement
/// outlives the process that wrote it and the default hasher makes no stability promise.
fn hash(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

use serde::{
    Deserialize,
    Serialize,
};

use crate::types::date_time::DateTime;
use crate::types::status_code::StatusCode;
use crate::types::variant::Variant;

/// A value together with its quality and timestamps (OPC 10000-4 §7.7); every part is optional.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DataValue {
    pub value: Option<Variant>,
    pub status: Option<StatusCode>,
    pub source_timestamp: Option<DateTime>,
    pub source_picoseconds: Option<u16>,
    pub server_timestamp: Option<DateTime>,
    pub server_picoseconds: Option<u16>,
}

impl DataValue {
    pub fn new(value: Variant) -> Self {
        Self {
            value: Some(value),
            ..Self::default()
        }
    }

    pub fn is_good(&self) -> bool {
        self.status.is_none_or(StatusCode::is_good)
    }
}

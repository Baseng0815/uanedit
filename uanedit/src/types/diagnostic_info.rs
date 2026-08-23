use serde::{
    Deserialize,
    Serialize,
};

use crate::types::status_code::StatusCode;

/// Diagnostics attached to a value (OPC 10000-4 §7.12); the integer fields index a string table.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    pub symbolic_id: Option<i32>,
    pub namespace_uri: Option<i32>,
    pub locale: Option<i32>,
    pub localized_text: Option<i32>,
    pub additional_info: Option<String>,
    pub inner_status_code: Option<StatusCode>,
    pub inner_diagnostic_info: Option<Box<DiagnosticInfo>>,
}

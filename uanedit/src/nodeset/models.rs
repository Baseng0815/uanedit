use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::access_restrictions::AccessRestrictions;
use crate::attributes::permissions::RolePermission;
use crate::types::date_time::DateTime;

/// One namespace the file defines or depends on, with the version that pins it
/// (UANodeSet `ModelTableEntry`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTableEntry {
    pub model_uri: String,
    pub xml_schema_uri: Option<String>,
    pub version: Option<String>,
    pub publication_date: Option<DateTime>,
    pub model_version: Option<String>,
    pub access_restrictions: AccessRestrictions,
    pub role_permissions: Vec<RolePermission>,
    /// The namespaces this one is written against, each pinned to a version.
    pub required_models: Vec<ModelTableEntry>,
}

impl ModelTableEntry {
    pub fn new(model_uri: impl Into<String>) -> Self {
        Self {
            model_uri: model_uri.into(),
            ..Self::default()
        }
    }

    /// Whether `other` satisfies this entry when read as a requirement.
    pub fn is_satisfied_by(
        &self,
        other: &Self,
    ) -> bool {
        self.model_uri == other.model_uri
            && match (&self.publication_date, &other.publication_date) {
                (Some(required), Some(available)) => available >= required,
                (Some(_), None) => false,
                (None, _) => true,
            }
    }
}

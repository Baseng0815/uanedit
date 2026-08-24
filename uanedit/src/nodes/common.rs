use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::access_restrictions::AccessRestrictions;
use crate::attributes::permissions::RolePermission;
use crate::attributes::write_mask::WriteMask;
use crate::nodes::reference::Reference;
use crate::nodes::release_status::ReleaseStatus;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::NodeId;
use crate::types::node_id_ref::NodeIdRef;
use crate::types::qualified_name::QualifiedName;
use crate::types::xml::XmlElement;

/// What every node carries, whatever its class (OPC 10000-3 §5.2, UANodeSet `UANode`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeHeader {
    pub node_id: NodeId,
    pub browse_name: QualifiedName,
    pub display_name: Vec<LocalizedText>,
    pub description: Vec<LocalizedText>,
    pub category: Vec<String>,
    pub documentation: Option<String>,
    pub references: Vec<Reference>,
    pub role_permissions: Vec<RolePermission>,
    pub extensions: Vec<XmlElement>,
    pub write_mask: WriteMask,
    pub user_write_mask: WriteMask,
    pub access_restrictions: Option<AccessRestrictions>,
    pub has_no_permissions: bool,
    pub symbolic_name: Option<String>,
    pub release_status: ReleaseStatus,
    /// Attributes from a schema revision this crate does not know, kept so a save does not drop them.
    pub unknown_attributes: IndexMap<String, String>,
    /// Child elements this crate does not model, kept for the same reason, each in its place.
    pub unknown_elements: Vec<UnknownChild>,
    /// Which of the optional child elements the document wrote out empty, which is not the same as
    /// leaving them out.
    pub empty_children: EmptyChildren,
}

/// A child element this crate does not model, kept with its place among the ones it does.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownChild {
    pub element: XmlElement,
    /// How many children this crate models came before it.
    pub after: usize,
}

/// Child elements a document wrote as empty ones rather than leaving out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmptyChildren {
    pub references: bool,
    pub role_permissions: bool,
    pub extensions: bool,
}

/// What the four instance classes carry on top of [`NodeHeader`] (UANodeSet `UAInstance`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceHeader {
    pub parent_node_id: Option<NodeIdRef>,
    pub design_tool_only: bool,
}

impl NodeHeader {
    pub fn new(
        node_id: NodeId,
        browse_name: QualifiedName,
    ) -> Self {
        Self {
            node_id,
            browse_name,
            ..Self::default()
        }
    }

    /// The DisplayName to show, preferring `locale` and falling back to the BrowseName.
    pub fn label(
        &self,
        locale: Option<&str>,
    ) -> &str {
        match pick_locale(&self.display_name, locale) {
            Some(text) => &text.text,
            None => &self.browse_name.name,
        }
    }
}

/// Picks the entry for `locale`, then one with no locale, then the first.
pub fn pick_locale<'a>(
    texts: &'a [LocalizedText],
    locale: Option<&str>,
) -> Option<&'a LocalizedText> {
    let matching = locale.and_then(|locale| {
        texts.iter().find(|text| {
            text.locale
                .as_deref()
                .is_some_and(|candidate| candidate == locale)
        })
    });
    matching
        .or_else(|| texts.iter().find(|text| text.locale.is_none()))
        .or_else(|| texts.first())
}

mod node;

pub(crate) use node::{
    attributes as node_attributes,
    render as render_node,
    render_as as render_node_as,
};

use crate::attributes::permissions::RolePermission;
use crate::emit::Writer;
use crate::nodeset::models::ModelTableEntry;
use crate::nodeset::node_set::NodeSet;
use crate::types::localized_text::LocalizedText;
use crate::types::xml::XmlElement;
use crate::xml::span::Layout;

/// The XML namespace of the UANodeSet schema itself (OPC 10000-6 Annex F).
pub const NODESET_NAMESPACE: &str = "http://opcfoundation.org/UA/2011/03/UANodeSet.xsd";

/// One node's text, split where its source region is, so an edit to either half leaves the other
/// half of the original bytes in place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderedNode {
    pub tag: String,
    pub body: String,
    /// Empty when the node was written self-closing.
    pub end: String,
}

/// Writes a nodeset with no source document behind it, in this crate's own layout.
pub fn nodeset(nodeset: &NodeSet) -> String {
    let layout = Layout::default();
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>");
    out.push_str(&layout.newline);
    out.push_str(&root_start(nodeset));
    for table in tables(nodeset, &layout, &layout.indent) {
        out.push_str(&layout.newline);
        out.push_str(&layout.indent);
        out.push_str(&table);
    }
    for node in nodeset.iter() {
        let rendered = node::render(node, &layout, &layout.indent);
        out.push_str(&layout.newline);
        out.push_str(&layout.indent);
        out.push_str(&rendered.tag);
        out.push_str(&rendered.body);
        out.push_str(&rendered.end);
    }
    out.push_str(&layout.newline);
    out.push_str("</UANodeSet>");
    out.push_str(&layout.newline);
    out
}

/// The root element's start tag, which carries the namespaces the schema expects.
pub(crate) fn root_start(nodeset: &NodeSet) -> String {
    let mut writer = Writer::with_layout(&Layout::default(), "");
    writer.start("UANodeSet");
    writer.attribute("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance");
    writer.attribute("xmlns:xsd", "http://www.w3.org/2001/XMLSchema");
    if let Some(last_modified) = &nodeset.last_modified {
        writer.attribute("LastModified", &last_modified.to_string());
    }
    writer.attribute("xmlns", NODESET_NAMESPACE);
    writer.opened();
    writer.out
}

/// Every table the nodeset has content for, in the order the schema puts them.
pub(crate) fn tables(
    nodeset: &NodeSet,
    layout: &Layout,
    base: &str,
) -> Vec<String> {
    [
        namespace_uris(nodeset, layout, base),
        server_uris(nodeset, layout, base),
        models(nodeset, layout, base),
        aliases(nodeset, layout, base),
        extensions(nodeset, layout, base),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(crate) fn namespace_uris(
    nodeset: &NodeSet,
    layout: &Layout,
    base: &str,
) -> Option<String> {
    uri_table("NamespaceUris", nodeset.namespaces.uris(), layout, base)
}

pub(crate) fn server_uris(
    nodeset: &NodeSet,
    layout: &Layout,
    base: &str,
) -> Option<String> {
    uri_table("ServerUris", &nodeset.server_uris, layout, base)
}

fn uri_table(
    name: &str,
    uris: &[String],
    layout: &Layout,
    base: &str,
) -> Option<String> {
    if uris.is_empty() {
        return None;
    }
    let mut writer = Writer::with_layout(layout, base);
    writer.start(name);
    writer.opened();
    for uri in uris {
        writer.line(1);
        writer.start("Uri");
        writer.contents("Uri", uri);
    }
    writer.line(0);
    writer.end(name);
    Some(writer.out)
}

pub(crate) fn models(
    nodeset: &NodeSet,
    layout: &Layout,
    base: &str,
) -> Option<String> {
    if nodeset.models.is_empty() {
        return None;
    }
    let mut writer = Writer::with_layout(layout, base);
    writer.start("Models");
    writer.opened();
    for model in &nodeset.models {
        writer.model(model, "Model", 1);
    }
    writer.line(0);
    writer.end("Models");
    Some(writer.out)
}

pub(crate) fn aliases(
    nodeset: &NodeSet,
    layout: &Layout,
    base: &str,
) -> Option<String> {
    if nodeset.aliases.is_empty() {
        return None;
    }
    let mut writer = Writer::with_layout(layout, base);
    writer.start("Aliases");
    writer.opened();
    for (alias, node_id) in nodeset.aliases.iter() {
        writer.line(1);
        writer.start("Alias");
        writer.attribute("Alias", alias);
        writer.contents("Alias", &node_id.to_string());
    }
    writer.line(0);
    writer.end("Aliases");
    Some(writer.out)
}

pub(crate) fn extensions(
    nodeset: &NodeSet,
    layout: &Layout,
    base: &str,
) -> Option<String> {
    if nodeset.extensions.is_empty() {
        return None;
    }
    let mut writer = Writer::with_layout(layout, base);
    writer.extension_list(&nodeset.extensions, 0);
    Some(writer.out)
}

impl Writer {
    /// An emitter that replays the document's own line ending and indentation.
    pub(crate) fn with_layout(
        layout: &Layout,
        base: &str,
    ) -> Self {
        Self::new(&layout.newline, &layout.indent, base)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    /// Writes an attribute only when the value is not the one the schema assumes.
    pub(crate) fn unless(
        &mut self,
        name: &str,
        value: &str,
        default: &str,
    ) {
        if value != default {
            self.attribute(name, value);
        }
    }

    pub(crate) fn flag(
        &mut self,
        name: &str,
        value: bool,
        default: bool,
    ) {
        if value != default {
            self.attribute(name, if value { "true" } else { "false" });
        }
    }

    /// A `LocalizedText` element, whose absent locale is not the same as an empty one.
    pub(crate) fn localized(
        &mut self,
        name: &str,
        text: &LocalizedText,
        depth: usize,
    ) {
        self.line(depth);
        self.start(name);
        if let Some(locale) = &text.locale {
            self.attribute("Locale", locale);
        }
        self.contents(name, &text.text);
    }

    pub(crate) fn role_permissions(
        &mut self,
        permissions: &[RolePermission],
        depth: usize,
    ) {
        self.line(depth);
        self.start("RolePermissions");
        if permissions.is_empty() {
            self.closed();
            return;
        }
        self.opened();
        for permission in permissions {
            self.line(depth + 1);
            self.start("RolePermission");
            self.unless("Permissions", &permission.permissions.bits().to_string(), "0");
            self.contents("RolePermission", &permission.role_id.to_string());
        }
        self.line(depth);
        self.end("RolePermissions");
    }

    fn model(
        &mut self,
        model: &ModelTableEntry,
        name: &str,
        depth: usize,
    ) {
        self.line(depth);
        self.start(name);
        self.attribute("ModelUri", &model.model_uri);
        if let Some(uri) = &model.xml_schema_uri {
            self.attribute("XmlSchemaUri", uri);
        }
        if let Some(version) = &model.version {
            self.attribute("Version", version);
        }
        if let Some(date) = &model.publication_date {
            self.attribute("PublicationDate", &date.to_string());
        }
        if let Some(version) = &model.model_version {
            self.attribute("ModelVersion", version);
        }
        self.unless("AccessRestrictions", &model.access_restrictions.bits().to_string(), "0");
        if model.role_permissions.is_empty() && model.required_models.is_empty() {
            self.closed();
            return;
        }
        self.opened();
        if !model.role_permissions.is_empty() {
            self.role_permissions(&model.role_permissions, depth + 1);
        }
        for required in &model.required_models {
            self.model(required, "RequiredModel", depth + 1);
        }
        self.line(depth);
        self.end(name);
    }

    pub(crate) fn extension_list(
        &mut self,
        extensions: &[XmlElement],
        depth: usize,
    ) {
        self.start("Extensions");
        if extensions.is_empty() {
            self.closed();
            return;
        }
        self.opened();
        for extension in extensions {
            self.line(depth + 1);
            self.element(extension);
        }
        self.line(depth);
        self.end("Extensions");
    }
}

use crate::attributes::permissions::{
    PermissionType,
    RolePermission,
};
use crate::error::DocumentError;
use crate::nodeset::aliases::AliasTable;
use crate::nodeset::models::ModelTableEntry;
use crate::report::{
    Diagnosis,
    PreservedKind,
};
use crate::types::node_id::NodeId;
use crate::types::xml::XmlElement;
use crate::xml::cursor::Tag;
use crate::xml::read::{
    Reading,
    node_id_ref,
};

impl<'a> Reading<'a> {
    /// A `NamespaceUris` or `ServerUris` table, which is a list of `Uri` elements.
    pub(super) fn uri_table(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Vec<String>, DocumentError> {
        let mut uris = Vec::new();
        if tag.empty {
            return Ok(uris);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Uri" => uris.push(self.cursor.text(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(uris)
    }

    pub(super) fn models(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Vec<ModelTableEntry>, DocumentError> {
        let mut models = Vec::new();
        if tag.empty {
            return Ok(models);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Model" => models.push(self.model(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(models)
    }

    fn model(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<ModelTableEntry, DocumentError> {
        let mut attributes = self.attributes(tag);
        let mut model = ModelTableEntry {
            model_uri: attributes.take("ModelUri").unwrap_or_default(),
            xml_schema_uri: attributes.take("XmlSchemaUri"),
            version: attributes.take("Version"),
            publication_date: self.attribute(&mut attributes, "PublicationDate"),
            model_version: attributes.take("ModelVersion"),
            access_restrictions: self
                .number::<u16>(&mut attributes, "AccessRestrictions")
                .map(Into::into)
                .unwrap_or_default(),
            role_permissions: Vec::new(),
            required_models: Vec::new(),
        };
        if tag.empty {
            return Ok(model);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "RolePermissions" => model.role_permissions = self.role_permissions(&child)?,
                "RequiredModel" => model.required_models.push(self.model(&child)?),
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(model)
    }

    pub(super) fn role_permissions(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Vec<RolePermission>, DocumentError> {
        let mut permissions = Vec::new();
        if tag.empty {
            return Ok(permissions);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "RolePermission" => {
                    let mut attributes = self.attributes(&child);
                    let granted: PermissionType = self
                        .number::<u32>(&mut attributes, "Permissions")
                        .map(Into::into)
                        .unwrap_or_default();
                    let role = self.cursor.text(&child)?;
                    permissions.push(RolePermission {
                        role_id: node_id_ref(role.trim()),
                        permissions: granted,
                    });
                }
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(permissions)
    }

    pub(super) fn aliases(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<AliasTable, DocumentError> {
        let mut aliases = AliasTable::default();
        if tag.empty {
            return Ok(aliases);
        }
        while let Some(child) = self.cursor.tag()? {
            match child.local_name() {
                "Alias" => {
                    let offset = child.span.start;
                    let mut attributes = self.attributes(&child);
                    let name = attributes.take("Alias").unwrap_or_default();
                    let node_id: Option<NodeId> = self.parsed_text(&child)?;
                    if let Some(node_id) = node_id
                        && aliases.insert(name.clone(), node_id).is_some()
                    {
                        self.find(offset, Diagnosis::DuplicateAlias(name));
                    }
                }
                other => {
                    self.preserve(PreservedKind::UnknownElement, other, child.span.start);
                    self.cursor.skip(&child)?;
                }
            }
        }
        Ok(aliases)
    }

    /// An `Extensions` list, kept as the `Extension` wrappers so an empty one survives too.
    pub(super) fn extensions(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<Vec<XmlElement>, DocumentError> {
        let mut extensions = Vec::new();
        if tag.empty {
            return Ok(extensions);
        }
        while let Some(child) = self.cursor.tag()? {
            let name = child.local_name().to_owned();
            let element = self.cursor.element(&child)?;
            let kind = match name.as_str() {
                "Extension" => PreservedKind::Extension,
                _ => PreservedKind::UnknownElement,
            };
            self.preserve(kind, &name, child.span.start);
            extensions.push(element);
        }
        Ok(extensions)
    }
}

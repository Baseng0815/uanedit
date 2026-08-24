use crate::edit::change::Change;
use crate::edit::compile::{
    Compiled,
    Compiler,
};
use crate::edit::outcome::Refusal;
use crate::nodes::node::Node;
use crate::nodeset::models::ModelTableEntry;
use crate::nodeset::namespaces::NamespaceTable;
use crate::nodeset::node_set::NodeSet;
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use crate::types::node_id_ref::NodeIdRef;

pub(crate) fn add_namespace(
    compiler: &mut Compiler<'_>,
    uri: &str,
) -> Result<Compiled, Refusal> {
    if compiler.primary().namespaces.index_of(uri).is_some() {
        return Err(Refusal::DuplicateNamespace { uri: uri.to_owned() });
    }
    let mut uris = compiler.declared_uris();
    if !uris.iter().any(|declared| declared == uri) {
        uris.push(uri.to_owned());
    }
    if !keeps_indexes(compiler, &uris) {
        let index = NamespaceIndex::try_from(uris.len()).unwrap_or(NamespaceIndex::MAX);
        return Err(Refusal::NamespaceRenumbers {
            index,
            uri: uri.to_owned(),
        });
    }
    Ok(Compiled::of(Change::SetNamespaces(NamespaceTable::new(uris))))
}

pub(crate) fn remove_namespace(
    compiler: &mut Compiler<'_>,
    index: NamespaceIndex,
) -> Result<Compiled, Refusal> {
    if index == 0 {
        return Err(Refusal::NamespaceFixed { index });
    }
    let uri = compiler
        .primary()
        .namespaces
        .uri(index)
        .ok_or(Refusal::UnknownNamespace { index })?
        .to_owned();
    let users = namespace_users(compiler.primary(), index);
    if !users.is_empty() {
        return Err(Refusal::NamespaceInUse { index, uri, users });
    }
    let mut uris = compiler.primary().namespaces.uris().to_vec();
    uris.remove(usize::from(index) - 1);
    if !keeps_indexes(compiler, &uris) {
        return Err(Refusal::NamespaceRenumbers { index, uri });
    }
    Ok(Compiled::of(Change::SetNamespaces(NamespaceTable::new(uris))))
}

/// Whether the address space would still number every namespace it keeps at the index it has now.
///
/// Dropping an entry the sets below the primary do not declare shifts everything above it down,
/// and with it every NodeId the editor is holding; adding one out of order does the same. Growing
/// or shrinking the table at its end moves nothing, so only the shared prefix has to agree.
fn keeps_indexes(
    compiler: &Compiler<'_>,
    uris: &[String],
) -> bool {
    let mut projected = NamespaceTable::default();
    for uri in uris {
        projected.insert(uri.clone());
    }
    for (_, node_set) in compiler.space().sets().skip(1) {
        for uri in node_set.namespaces.uris() {
            projected.insert(uri.clone());
        }
    }
    let current = compiler.space().namespaces().uris();
    let shared = projected.uris().len().min(current.len());
    projected.uris()[..shared] == current[..shared]
}

pub(crate) fn rename_namespace(
    compiler: &mut Compiler<'_>,
    index: NamespaceIndex,
    uri: &str,
) -> Result<Compiled, Refusal> {
    if index == 0 {
        return Err(Refusal::NamespaceFixed { index });
    }
    let previous = compiler
        .primary()
        .namespaces
        .uri(index)
        .ok_or(Refusal::UnknownNamespace { index })?
        .to_owned();
    if previous == uri {
        return Ok(Compiled::default());
    }
    if compiler.primary().namespaces.index_of(uri).is_some() {
        return Err(Refusal::DuplicateNamespace { uri: uri.to_owned() });
    }
    require_own_namespace(compiler, index, &previous)?;
    let mut uris = compiler.declared_uris();
    let position = usize::from(index) - 1;
    match uris.get_mut(position) {
        Some(slot) => *slot = uri.to_owned(),
        None => return Err(Refusal::UnknownNamespace { index }),
    }
    let mut compiled = Compiled::of(Change::SetNamespaces(NamespaceTable::new(uris)));
    if let Some(models) = renamed_model(compiler.primary(), &previous, uri) {
        compiled.changes.push(Change::SetModels(models));
    }
    Ok(compiled)
}

/// The model table with the file's own model URI following the namespace it names.
fn renamed_model(
    node_set: &NodeSet,
    previous: &str,
    uri: &str,
) -> Option<Vec<ModelTableEntry>> {
    let model = node_set.models.first()?;
    if model.model_uri != previous {
        return None;
    }
    let mut models = node_set.models.clone();
    models[0].model_uri = uri.to_owned();
    Some(models)
}

pub(crate) fn set_model(
    compiler: &mut Compiler<'_>,
    index: usize,
    entry: &ModelTableEntry,
) -> Result<Compiled, Refusal> {
    let previous = compiler
        .primary()
        .models
        .get(index)
        .ok_or(Refusal::UnknownModel { index })?
        .clone();
    if previous == *entry {
        return Ok(Compiled::default());
    }
    let mut models = compiler.primary().models.clone();
    models[index] = entry.clone();
    let mut compiled = Compiled::of(Change::SetModels(models));
    if previous.model_uri != entry.model_uri
        && let Some(moved) = compiler.primary().namespaces.index_of(&previous.model_uri)
    {
        require_own_namespace(compiler, moved, &previous.model_uri)?;
        let mut uris = compiler.declared_uris();
        uris[usize::from(moved) - 1] = entry.model_uri.clone();
        compiled
            .changes
            .push(Change::SetNamespaces(NamespaceTable::new(uris)));
    }
    Ok(compiled)
}

pub(crate) fn add_model(
    compiler: &mut Compiler<'_>,
    entry: &ModelTableEntry,
) -> Result<Compiled, Refusal> {
    if compiler
        .primary()
        .models
        .iter()
        .any(|model| model.model_uri == entry.model_uri)
    {
        return Err(Refusal::DuplicateModel {
            uri: entry.model_uri.clone(),
        });
    }
    let mut models = compiler.primary().models.clone();
    models.push(entry.clone());
    Ok(Compiled::of(Change::SetModels(models)))
}

pub(crate) fn remove_model(
    compiler: &mut Compiler<'_>,
    index: usize,
) -> Result<Compiled, Refusal> {
    let entry = compiler
        .primary()
        .models
        .get(index)
        .ok_or(Refusal::UnknownModel { index })?
        .clone();
    if let Some(namespace) = compiler.primary().namespaces.index_of(&entry.model_uri)
        && namespace != 0
    {
        let users = namespace_users(compiler.primary(), namespace);
        if !users.is_empty() {
            return Err(Refusal::NamespaceInUse {
                index: namespace,
                uri: entry.model_uri,
                users,
            });
        }
    }
    let mut models = compiler.primary().models.clone();
    models.remove(index);
    Ok(Compiled::of(Change::SetModels(models)))
}

/// Renaming a namespace a dependency also declares would renumber the address space under every
/// NodeId the editor is holding, so only the file's own namespaces may be renamed.
fn require_own_namespace(
    compiler: &Compiler<'_>,
    index: NamespaceIndex,
    uri: &str,
) -> Result<(), Refusal> {
    let shared = compiler.space().sets().skip(1).any(|(_, node_set)| {
        node_set
            .namespaces
            .index_of(uri)
            .is_some_and(|found| found != 0)
    });
    match shared {
        true => Err(Refusal::NamespaceOfDependency {
            index,
            uri: uri.to_owned(),
        }),
        false => Ok(()),
    }
}

pub(crate) fn add_alias(
    compiler: &mut Compiler<'_>,
    alias: &str,
    node_id: &NodeId,
) -> Result<Compiled, Refusal> {
    if compiler.primary().aliases.get(alias).is_some() {
        return Err(Refusal::DuplicateAlias {
            alias: alias.to_owned(),
        });
    }
    let local = compiler.local_node_id(node_id)?;
    let mut aliases = compiler.primary().aliases.clone();
    aliases.insert(alias, local);
    Ok(Compiled::of(Change::SetAliases(aliases)))
}

pub(crate) fn remove_alias(
    compiler: &mut Compiler<'_>,
    alias: &str,
) -> Result<Compiled, Refusal> {
    if compiler.primary().aliases.get(alias).is_none() {
        return Err(Refusal::UnknownAlias {
            alias: alias.to_owned(),
        });
    }
    let users = alias_users(compiler.primary(), alias);
    if !users.is_empty() {
        return Err(Refusal::AliasInUse {
            alias: alias.to_owned(),
            users,
        });
    }
    let mut aliases = compiler.primary().aliases.clone();
    aliases.remove(alias);
    Ok(Compiled::of(Change::SetAliases(aliases)))
}

/// The nodes that would stop resolving if the alias went away.
pub fn alias_users(
    node_set: &NodeSet,
    alias: &str,
) -> Vec<NodeId> {
    node_set
        .iter()
        .filter(|node| node_names_alias(node, alias))
        .map(|node| node.node_id().clone())
        .collect()
}

/// The nodes that would stop resolving if the namespace left the table.
pub fn namespace_users(
    node_set: &NodeSet,
    index: NamespaceIndex,
) -> Vec<NodeId> {
    node_set
        .iter()
        .filter(|node| node_names_namespace(node_set, node, index))
        .map(|node| node.node_id().clone())
        .collect()
}

fn node_names_namespace(
    node_set: &NodeSet,
    node: &Node,
    index: NamespaceIndex,
) -> bool {
    if node.node_id().namespace_index == index || node.browse_name().namespace_index == index {
        return true;
    }
    let named = |reference: &NodeIdRef| {
        node_set
            .resolve(reference)
            .is_some_and(|node_id| node_id.namespace_index == index)
    };
    node_names_any(node, &named)
}

fn node_names_alias(
    node: &Node,
    alias: &str,
) -> bool {
    let named = |reference: &NodeIdRef| reference.as_alias() == Some(alias);
    node_names_any(node, &named)
}

/// Whether any NodeId the node spells out satisfies the predicate.
fn node_names_any(
    node: &Node,
    named: &dyn Fn(&NodeIdRef) -> bool,
) -> bool {
    let in_references = node
        .references()
        .iter()
        .any(|reference| named(&reference.reference_type) || named(&reference.target));
    if in_references {
        return true;
    }
    if node
        .instance()
        .and_then(|instance| instance.parent_node_id.as_ref())
        .is_some_and(named)
    {
        return true;
    }
    if node
        .header()
        .role_permissions
        .iter()
        .any(|grant| named(&grant.role_id))
    {
        return true;
    }
    match node {
        Node::Variable(variable) => named(&variable.data_type),
        Node::VariableType(variable_type) => named(&variable_type.data_type),
        Node::Method(method) => method.method_declaration_id.as_ref().is_some_and(named),
        Node::DataType(data_type) => data_type.definition.as_ref().is_some_and(|definition| {
            definition
                .fields
                .iter()
                .any(|field| named(&field.data_type))
        }),
        _ => false,
    }
}

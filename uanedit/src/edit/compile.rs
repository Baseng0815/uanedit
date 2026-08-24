use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::permissions::RolePermission;
use crate::attributes::value_rank::ValueRank;
use crate::edit::change::Change;
use crate::edit::field::{
    self,
    FieldValue,
    StoredValue,
};
use crate::edit::operation::Operation;
use crate::edit::outcome::Refusal;
use crate::edit::reference::{
    ReferenceKey,
    Statement,
};
use crate::edit::{
    create,
    delete,
    instantiate,
    reference,
    subtype,
    table,
};
use crate::ids;
use crate::nodes::node::Node;
use crate::nodes::reference::Reference;
use crate::nodeset::namespaces::NamespaceTable;
use crate::nodeset::node_set::NodeSet;
use crate::rules::finding::Finding;
use crate::rules::query::{
    self,
    Verdict,
};
use crate::space::delta::NodeField;
use crate::space::{
    AddressSpace,
    SetId,
};
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};
use crate::types::node_id_ref::NodeIdRef;
use crate::types::qualified_name::QualifiedName;

/// What an operation comes down to: the changes to make, and what the session has to check once
/// they are made.
#[derive(Debug, Default)]
pub(crate) struct Compiled {
    pub changes: Vec<Change>,
    /// The references the operation states. The engine only checks references whose source is a
    /// node of the editable set, so the session checks these itself.
    pub references: Vec<ReferenceKey>,
    pub created: Vec<NodeId>,
    /// Findings the operation was let past because the caller asked for the override.
    pub rejections: Vec<Finding>,
}

impl Compiled {
    pub(crate) fn of(change: Change) -> Self {
        Self {
            changes: vec![change],
            ..Self::default()
        }
    }
}

/// Turns one operation into primitive changes, refusing anything the rules engine would not allow.
pub(crate) struct Compiler<'a> {
    space: &'a AddressSpace,
    forced: bool,
    rejections: Vec<Finding>,
    /// The highest namespace index the changes need the primary nodeset's table to declare.
    required_index: NamespaceIndex,
    next_identifier: Option<u32>,
}

impl<'a> Compiler<'a> {
    pub(crate) fn new(
        space: &'a AddressSpace,
        forced: bool,
    ) -> Self {
        Self {
            space,
            forced,
            rejections: Vec::new(),
            required_index: 0,
            next_identifier: None,
        }
    }

    pub(crate) fn space(&self) -> &'a AddressSpace {
        self.space
    }

    pub(crate) fn primary(&self) -> &'a NodeSet {
        self.space.primary()
    }

    pub(crate) fn primary_node(
        &self,
        node_id: &NodeId,
    ) -> Option<&'a Node> {
        let local = self.space.to_local_node_id(SetId::PRIMARY, node_id)?;
        self.primary().node(&local)
    }

    pub(crate) fn is_editable(
        &self,
        node_id: &NodeId,
    ) -> bool {
        !self.space.is_read_only(node_id)
    }

    pub(crate) fn require_known(
        &self,
        node_id: &NodeId,
    ) -> Result<(), Refusal> {
        match self.space.contains(node_id) {
            true => Ok(()),
            false => Err(Refusal::UnknownNode { node: node_id.clone() }),
        }
    }

    pub(crate) fn require_editable(
        &self,
        node_id: &NodeId,
    ) -> Result<(), Refusal> {
        self.require_known(node_id)?;
        match self.is_editable(node_id) {
            true => Ok(()),
            false => Err(Refusal::ReadOnly { node: node_id.clone() }),
        }
    }

    /// Refuses what the engine says the operation may not do, unless the override is in force.
    pub(crate) fn reject(
        &mut self,
        verdict: Verdict,
    ) -> Result<(), Refusal> {
        if verdict.is_allowed() {
            return Ok(());
        }
        if !self.forced {
            return Err(Refusal::Rejected(verdict));
        }
        self.rejections.extend(verdict.findings);
        Ok(())
    }

    /// Refuses what no rule covers but the operation itself does not allow.
    pub(crate) fn forbid(
        &self,
        refusal: Refusal,
    ) -> Result<(), Refusal> {
        match self.forced {
            true => Ok(()),
            false => Err(refusal),
        }
    }

    pub(crate) fn resolves_to(
        &self,
        reference: &NodeIdRef,
        node_id: &NodeId,
    ) -> bool {
        self.primary()
            .resolve(reference)
            .and_then(|local| self.space.to_space_node_id(SetId::PRIMARY, local))
            .as_ref()
            == Some(node_id)
    }

    /// The NodeId as the primary nodeset spells it, declaring the namespace if it has to.
    pub(crate) fn local_node_id(
        &mut self,
        node_id: &NodeId,
    ) -> Result<NodeId, Refusal> {
        if let Some(local) = self.space.to_local_node_id(SetId::PRIMARY, node_id) {
            return Ok(local);
        }
        self.declare(node_id.namespace_index)?;
        Ok(node_id.clone())
    }

    /// How a new reference names the node: through an existing alias where the file has one.
    pub(crate) fn reference_to(
        &mut self,
        node_id: &NodeId,
    ) -> Result<NodeIdRef, Refusal> {
        let local = self.local_node_id(node_id)?;
        match self.primary().aliases.alias_for(&local) {
            Some(alias) => Ok(NodeIdRef::alias(alias)),
            None => Ok(NodeIdRef::Id(local)),
        }
    }

    pub(crate) fn local_browse_name(
        &mut self,
        browse_name: &QualifiedName,
    ) -> Result<QualifiedName, Refusal> {
        self.declare(browse_name.namespace_index)?;
        Ok(browse_name.clone())
    }

    /// True when the file already states exactly this reference where the statement sits.
    pub(crate) fn states(
        &self,
        statement: &Statement,
        reference: &Reference,
    ) -> bool {
        self.primary_node(&statement.holder)
            .and_then(|node| node.references().get(statement.position))
            == Some(reference)
    }

    pub(crate) fn reference_count(
        &self,
        node_id: &NodeId,
    ) -> usize {
        self.primary_node(node_id)
            .map_or(0, |node| node.references().len())
    }

    /// A NodeId no node has, in the namespace the nodeset defines its own nodes in.
    pub(crate) fn fresh_node_id(&mut self) -> Result<NodeId, Refusal> {
        let namespace = self.own_namespace_index()?;
        let mut candidate = match self.next_identifier {
            Some(next) => next,
            None => next_identifier(highest_identifier(self.primary(), namespace), namespace)?,
        };
        while self
            .primary()
            .node(&NodeId::numeric(namespace, candidate))
            .is_some()
        {
            candidate = next_identifier(candidate, namespace)?;
        }
        self.next_identifier = candidate.checked_add(1);
        Ok(NodeId::numeric(namespace, candidate))
    }

    fn own_namespace_index(&self) -> Result<NamespaceIndex, Refusal> {
        let node_set = self.primary();
        let uri = node_set.target_namespace().ok_or(Refusal::NoOwnNamespace)?;
        node_set
            .namespaces
            .index_of(uri)
            .or_else(|| (!node_set.namespaces.is_empty()).then_some(1))
            .ok_or(Refusal::NoOwnNamespace)
    }

    fn declare(
        &mut self,
        index: NamespaceIndex,
    ) -> Result<(), Refusal> {
        if self.space.namespace_uri(index).is_none() {
            return Err(Refusal::UnknownNamespace { index });
        }
        self.required_index = self.required_index.max(index);
        Ok(())
    }

    /// The primary nodeset's namespace URIs, extended with the ones the space already numbers.
    ///
    /// Adding a namespace out of order would renumber the space's indexes and with them every
    /// NodeId the editor is holding, so the file's table stays a prefix of the space's.
    pub(crate) fn declared_uris(&self) -> Vec<String> {
        let mut uris = self.primary().namespaces.uris().to_vec();
        while uris.len() + 1 < self.space.namespaces().len() {
            let Ok(index) = NamespaceIndex::try_from(uris.len() + 1) else {
                break;
            };
            match self.space.namespace_uri(index) {
                Some(uri) => uris.push(uri.to_owned()),
                None => break,
            }
        }
        uris
    }

    /// The change that declares every namespace the compiled changes name, if the file is missing any.
    fn namespace_change(&self) -> Option<Change> {
        let declared = self.primary().namespaces.uris().len();
        if usize::from(self.required_index) <= declared {
            return None;
        }
        let uris = self.declared_uris();
        (uris.len() > declared).then(|| Change::SetNamespaces(NamespaceTable::new(uris)))
    }

    fn finish(
        self,
        mut compiled: Compiled,
    ) -> Compiled {
        if let Some(change) = self.namespace_change() {
            compiled.changes.insert(0, change);
        }
        compiled.rejections.extend(self.rejections);
        compiled
    }
}

/// The next numeric identifier, refusing rather than spinning once the namespace is full.
fn next_identifier(
    identifier: u32,
    namespace: NamespaceIndex,
) -> Result<u32, Refusal> {
    identifier
        .checked_add(1)
        .ok_or(Refusal::NoFreeNodeId { namespace })
}

fn highest_identifier(
    node_set: &NodeSet,
    namespace: NamespaceIndex,
) -> u32 {
    node_set
        .nodes
        .keys()
        .filter(|node_id| node_id.namespace_index == namespace)
        .filter_map(|node_id| match node_id.identifier {
            crate::types::node_id::Identifier::Numeric(value) => Some(value),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

pub(crate) fn compile(
    space: &AddressSpace,
    operation: &Operation,
    forced: bool,
) -> Result<Compiled, Refusal> {
    let mut compiler = Compiler::new(space, forced);
    let compiled = dispatch(&mut compiler, operation)?;
    Ok(compiler.finish(compiled))
}

fn dispatch(
    compiler: &mut Compiler<'_>,
    operation: &Operation,
) -> Result<Compiled, Refusal> {
    match operation {
        Operation::CreateInstance(create) => create::instance(compiler, create),
        Operation::CreateType(create) => create::type_node(compiler, create),
        Operation::InstantiateType(create) => instantiate::compile(compiler, create),
        Operation::CreateSubtype(create) => subtype::compile(compiler, create),
        Operation::Delete(delete) => delete::compile(compiler, delete),
        Operation::AddReference(add) => reference::add(compiler, add),
        Operation::RemoveReference(key) => reference::remove(compiler, key),
        Operation::RetargetReference { reference, target } => reference::retarget(compiler, reference, target),
        Operation::SetTypeDefinition { node, type_definition } => set_type_definition(compiler, node, type_definition),
        Operation::SetModellingRule { node, modelling_rule } => {
            set_modelling_rule(compiler, node, modelling_rule.as_ref())
        }
        Operation::SetField { node, value } => set_field(compiler, node, value),
        Operation::SetDataType { node, data_type } => set_data_type(compiler, node, data_type),
        Operation::SetMethodDeclaration { node, declaration } => {
            set_method_declaration(compiler, node, declaration.as_ref())
        }
        Operation::SetParentNodeId { node, parent } => set_parent_node_id(compiler, node, parent.as_ref()),
        Operation::AddNamespace { uri } => table::add_namespace(compiler, uri),
        Operation::RenameNamespace { index, uri } => table::rename_namespace(compiler, *index, uri),
        Operation::RemoveNamespace { index } => table::remove_namespace(compiler, *index),
        Operation::SetModelEntry { index, entry } => table::set_model(compiler, *index, entry),
        Operation::AddModelEntry { entry } => table::add_model(compiler, entry),
        Operation::RemoveModelEntry { index } => table::remove_model(compiler, *index),
        Operation::AddAlias { alias, node_id } => table::add_alias(compiler, alias, node_id),
        Operation::RemoveAlias { alias } => table::remove_alias(compiler, alias),
    }
}

fn set_type_definition(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    type_definition: &NodeId,
) -> Result<Compiled, Refusal> {
    compiler.require_editable(node_id)?;
    compiler.require_known(type_definition)?;
    compiler.reject(query::may_set_type_definition(compiler.space(), node_id, type_definition))?;
    let mut compiled = Compiled::default();
    reference::replace_single(compiler, &mut compiled, node_id, &ids::HAS_TYPE_DEFINITION, Some(type_definition))?;
    Ok(compiled)
}

fn set_modelling_rule(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    modelling_rule: Option<&NodeId>,
) -> Result<Compiled, Refusal> {
    compiler.require_editable(node_id)?;
    if let Some(modelling_rule) = modelling_rule {
        compiler.require_known(modelling_rule)?;
        compiler.reject(query::may_set_modelling_rule(compiler.space(), node_id, modelling_rule))?;
    }
    let mut compiled = Compiled::default();
    reference::replace_single(compiler, &mut compiled, node_id, &ids::HAS_MODELLING_RULE, modelling_rule)?;
    Ok(compiled)
}

fn set_field(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    value: &FieldValue,
) -> Result<Compiled, Refusal> {
    let value = match value {
        FieldValue::BrowseName(browse_name) => FieldValue::BrowseName(compiler.local_browse_name(browse_name)?),
        FieldValue::RolePermissions(grants) => FieldValue::RolePermissions(local_grants(compiler, grants)?),
        other => other.clone(),
    };
    check_narrowing(compiler, node_id, &value)?;
    set_stored(compiler, node_id, StoredValue::Plain(value))
}

/// The grants with every Role the caller named as a NodeId restated the way the file spells it.
fn local_grants(
    compiler: &mut Compiler<'_>,
    grants: &[RolePermission],
) -> Result<Vec<RolePermission>, Refusal> {
    let mut local = Vec::with_capacity(grants.len());
    for grant in grants {
        let role_id = match grant.role_id.as_id() {
            Some(node_id) => compiler.reference_to(node_id)?,
            None => grant.role_id.clone(),
        };
        local.push(RolePermission {
            role_id,
            permissions: grant.permissions,
        });
    }
    Ok(local)
}

fn set_data_type(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    data_type: &NodeId,
) -> Result<Compiled, Refusal> {
    compiler.require_editable(node_id)?;
    compiler.reject(query::may_set_data_type(compiler.space(), node_id, data_type))?;
    if let Some(source) = query::narrowing_source(compiler.space(), node_id)
        && let Some(from) = compiler.space().data_type(&source)
        && !query::legal_data_type_narrowings(compiler.space(), &from).contains(data_type)
    {
        compiler.forbid(Refusal::NotNarrowed {
            node: node_id.clone(),
            field: NodeField::DATA_TYPE,
            from: from.to_string(),
            to: data_type.to_string(),
        })?;
    }
    let stored = StoredValue::DataType(compiler.reference_to(data_type)?);
    set_stored(compiler, node_id, stored)
}

fn set_method_declaration(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    declaration: Option<&NodeId>,
) -> Result<Compiled, Refusal> {
    compiler.require_editable(node_id)?;
    let stored = match declaration {
        Some(declaration) => {
            compiler.require_known(declaration)?;
            Some(compiler.reference_to(declaration)?)
        }
        None => None,
    };
    set_stored(compiler, node_id, StoredValue::MethodDeclarationId(stored))
}

fn set_parent_node_id(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    parent: Option<&NodeId>,
) -> Result<Compiled, Refusal> {
    compiler.require_editable(node_id)?;
    let stored = match parent {
        Some(parent) => Some(compiler.reference_to(parent)?),
        None => None,
    };
    set_stored(compiler, node_id, StoredValue::ParentNodeId(stored))
}

fn set_stored(
    compiler: &mut Compiler<'_>,
    node_id: &NodeId,
    value: StoredValue,
) -> Result<Compiled, Refusal> {
    compiler.require_editable(node_id)?;
    let mut probe = compiler
        .primary_node(node_id)
        .cloned()
        .ok_or_else(|| Refusal::UnknownNode { node: node_id.clone() })?;
    let field = value.field();
    let node_class = probe.node_class();
    let Some(previous) = field::set(value.clone(), &mut probe) else {
        return Err(Refusal::FieldNotOnClass {
            node: node_id.clone(),
            node_class,
            field,
        });
    };
    if previous == value {
        return Ok(Compiled::default());
    }
    let local = compiler.local_node_id(node_id)?;
    Ok(Compiled::of(Change::SetField { node_id: local, value }))
}

fn check_narrowing(
    compiler: &Compiler<'_>,
    node_id: &NodeId,
    value: &FieldValue,
) -> Result<(), Refusal> {
    let Some(source) = query::narrowing_source(compiler.space(), node_id) else {
        return Ok(());
    };
    let Some(node) = compiler.space().node(&source) else {
        return Ok(());
    };
    match value {
        FieldValue::ValueRank(to) => {
            let Some(from) = value_rank_of(node) else {
                return Ok(());
            };
            if query::may_narrow_value_rank(from, *to) {
                return Ok(());
            }
            compiler.forbid(Refusal::NotNarrowed {
                node: node_id.clone(),
                field: NodeField::VALUE_RANK,
                from: from.to_string(),
                to: to.to_string(),
            })
        }
        FieldValue::ArrayDimensions(to) => {
            let Some(from) = array_dimensions_of(node) else {
                return Ok(());
            };
            if query::may_narrow_array_dimensions(from, to) {
                return Ok(());
            }
            compiler.forbid(Refusal::NotNarrowed {
                node: node_id.clone(),
                field: NodeField::ARRAY_DIMENSIONS,
                from: from.to_string(),
                to: to.to_string(),
            })
        }
        _ => Ok(()),
    }
}

fn value_rank_of(node: &Node) -> Option<ValueRank> {
    match node {
        Node::Variable(variable) => Some(variable.value_rank),
        Node::VariableType(variable_type) => Some(variable_type.value_rank),
        _ => None,
    }
}

fn array_dimensions_of(node: &Node) -> Option<&ArrayDimensions> {
    match node {
        Node::Variable(variable) => Some(&variable.array_dimensions),
        Node::VariableType(variable_type) => Some(&variable_type.array_dimensions),
        _ => None,
    }
}

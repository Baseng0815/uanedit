use std::collections::{
    BTreeSet,
    HashMap,
    HashSet,
};

use crate::compile::rust::name;
use crate::nodes::Node;
use crate::nodes::data_type::DataType;
use crate::nodes::definition::{
    DataTypeDefinition,
    DataTypeField,
};
use crate::types::node_id::{
    Identifier,
    NodeId,
};
use crate::{
    AddressSpace,
    ids,
};

/// The `data_types` module: the primary nodeset's DataTypeDefinitions as plain Rust.
pub(super) fn section(space: &AddressSpace) -> String {
    let candidates = candidates(space);
    if candidates.is_empty() {
        return String::new();
    }
    let names: HashMap<NodeId, String> = candidates
        .iter()
        .map(|candidate| (candidate.id.clone(), candidate.rust_name.clone()))
        .collect();
    let supported = supported_set(space, &candidates, &names);
    let mut support = BTreeSet::new();
    let mut items = String::new();
    for candidate in &candidates {
        match item(space, candidate, &names, &supported) {
            Ok((text, used)) => {
                items.push_str(&text);
                support.extend(used);
            }
            Err(reason) => {
                items.push_str(&format!("\n    // `{}` ({}) skipped: {reason}\n", candidate.rust_name, candidate.id))
            }
        }
    }
    format!(
        "\n/// The DataTypes this nodeset defines, as plain Rust.\n///\n\
         /// Fields are in encoding order, inherited ones first. Optional fields are `Option`,\n\
         /// arrays are `Vec`, and a multi-dimensional value is flattened.\n\
         pub mod data_types {{\n{}{items}}}\n",
        support_definitions(&support)
    )
}

enum Kind {
    OptionSet,
    Enumeration,
    Union,
    Structure,
}

struct Candidate<'a> {
    id: NodeId,
    node: &'a DataType,
    definition: &'a DataTypeDefinition,
    rust_name: String,
    kind: Kind,
}

/// Every concrete primary DataType with a definition, under a collision-free Rust name.
fn candidates(space: &AddressSpace) -> Vec<Candidate<'_>> {
    let mut list = Vec::new();
    for id in space.primary_node_ids() {
        let Some(Node::DataType(node)) = space.node(&id) else {
            continue;
        };
        if node.is_abstract {
            continue;
        }
        let Some(definition) = &node.definition else {
            continue;
        };
        let header = &node.header;
        let rust_name = name::type_name(
            header
                .symbolic_name
                .as_deref()
                .unwrap_or(&header.browse_name.name),
        );
        list.push(Candidate {
            id,
            node,
            definition,
            rust_name,
            kind: kind(definition),
        });
    }
    let mut counts: HashMap<String, usize> = RESERVED
        .iter()
        .map(|name| ((*name).to_owned(), 2))
        .collect();
    for candidate in &list {
        *counts.entry(candidate.rust_name.clone()).or_default() += 1;
    }
    let mut used: HashSet<String> = RESERVED.iter().map(|name| (*name).to_owned()).collect();
    for candidate in &mut list {
        if counts[&candidate.rust_name] > 1 {
            candidate.rust_name = format!("{}{}", candidate.rust_name, type_suffix(&candidate.id));
        }
        while !used.insert(candidate.rust_name.clone()) {
            candidate.rust_name.push('X');
        }
    }
    list
}

/// The names the support types claim, which a generated type may not shadow.
const RESERVED: &[&str] = &[
    "DateTime",
    "StatusCode",
    "Guid",
    "NodeId",
    "Identifier",
    "ExpandedNodeId",
    "QualifiedName",
    "LocalizedText",
    "DiagnosticInfo",
];

fn kind(definition: &DataTypeDefinition) -> Kind {
    if definition.is_option_set {
        Kind::OptionSet
    } else if definition.is_enumeration() {
        Kind::Enumeration
    } else if definition.is_union {
        Kind::Union
    } else {
        Kind::Structure
    }
}

fn type_suffix(id: &NodeId) -> String {
    match &id.identifier {
        Identifier::Numeric(value) => value.to_string(),
        Identifier::String(value) => name::type_name(value),
        Identifier::Guid(_) | Identifier::Opaque(_) => String::new(),
    }
}

/// Drops candidates whose fields have no Rust form until the remainder stand on each other.
fn supported_set(
    space: &AddressSpace,
    candidates: &[Candidate<'_>],
    names: &HashMap<NodeId, String>,
) -> HashSet<NodeId> {
    let mut supported: HashSet<NodeId> = candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect();
    loop {
        let mut removed = false;
        for candidate in candidates {
            if supported.contains(&candidate.id) && item(space, candidate, names, &supported).is_err() {
                supported.remove(&candidate.id);
                removed = true;
            }
        }
        if !removed {
            return supported;
        }
    }
}

/// One generated item and the support types it leans on, or why it cannot be generated.
fn item(
    space: &AddressSpace,
    candidate: &Candidate<'_>,
    names: &HashMap<NodeId, String>,
    supported: &HashSet<NodeId>,
) -> Result<(String, BTreeSet<Support>), String> {
    let mut support = BTreeSet::new();
    let text = match candidate.kind {
        Kind::OptionSet => option_set_item(space, candidate)?,
        Kind::Enumeration => enumeration_item(candidate)?,
        Kind::Union => union_item(space, candidate, names, supported, &mut support)?,
        Kind::Structure => structure_item(space, candidate, names, supported, &mut support)?,
    };
    Ok((text, support))
}

fn doc(candidate: &Candidate<'_>) -> String {
    let header = &candidate.node.header;
    let mut doc = format!("    /// `{}` ({}).\n", header.browse_name.name.replace('`', "'"), candidate.id);
    if let Some(text) = header.description.first()
        && !text.text.trim().is_empty()
    {
        doc.push_str(&format!("    ///\n    /// {}\n", text.text.replace(['\n', '\r'], " ").trim()));
    }
    doc
}

fn variant_name(
    field: &DataTypeField,
    used: &mut HashSet<String>,
) -> String {
    let mut variant = name::type_name(field.symbolic_name.as_deref().unwrap_or(&field.name));
    if used.contains(&variant) {
        variant = format!("{variant}{}", field.value);
    }
    while !used.insert(variant.clone()) {
        variant.push('X');
    }
    variant
}

fn enumeration_item(candidate: &Candidate<'_>) -> Result<String, String> {
    let mut values = HashSet::new();
    for field in &candidate.definition.fields {
        if !values.insert(field.value) {
            return Err(format!("the enumeration repeats the value {}", field.value));
        }
    }
    let rust_name = &candidate.rust_name;
    let mut used = HashSet::new();
    let mut variants = String::new();
    let mut arms = String::new();
    for field in &candidate.definition.fields {
        let variant = variant_name(field, &mut used);
        variants.push_str(&format!("        {variant} = {},\n", field.value));
        arms.push_str(&format!("                {} => Ok(Self::{variant}),\n", field.value));
    }
    Ok(format!(
        "\n{}    #[repr(i32)]\n    #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n    \
         pub enum {rust_name} {{\n{variants}    }}\n\n    \
         impl TryFrom<i32> for {rust_name} {{\n        type Error = i32;\n\n        \
         fn try_from(value: i32) -> Result<Self, i32> {{\n            match value {{\n{arms}                \
         other => Err(other),\n            }}\n        }}\n    }}\n\n    \
         impl From<{rust_name}> for i32 {{\n        fn from(value: {rust_name}) -> i32 {{\n            \
         value as i32\n        }}\n    }}\n",
        doc(candidate)
    ))
}

fn option_set_item(
    space: &AddressSpace,
    candidate: &Candidate<'_>,
) -> Result<String, String> {
    let (form, bits) = option_width(space, &candidate.id);
    let mut used = HashSet::new();
    let mut constants = String::new();
    for field in &candidate.definition.fields {
        if field.value < 0 || field.value >= bits {
            return Err(format!("bit {} does not fit the {form} the OptionSet subtypes", field.value));
        }
        let mut constant = name::constant(field.symbolic_name.as_deref().unwrap_or(&field.name));
        if used.contains(&constant) {
            constant = format!("{constant}_{}", field.value);
        }
        while !used.insert(constant.clone()) {
            constant.push('_');
        }
        constants.push_str(&format!("        pub const {constant}: Self = Self(1 << {});\n", field.value));
    }
    let rust_name = &candidate.rust_name;
    Ok(format!(
        "\n{}    ///\n    /// Each constant is one bit.\n    \
         #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n    \
         pub struct {rust_name}(pub {form});\n\n    impl {rust_name} {{\n{constants}    }}\n",
        doc(candidate)
    ))
}

/// The unsigned width an OptionSet subtypes, defaulting to UInt32.
fn option_width(
    space: &AddressSpace,
    id: &NodeId,
) -> (&'static str, i32) {
    for (identifier, form, bits) in [(3, "u8", 8), (5, "u16", 16), (7, "u32", 32), (9, "u64", 64)] {
        if space.is_same_or_subtype_of(id, &base(identifier)) {
            return (form, bits);
        }
    }
    ("u32", 32)
}

fn union_item(
    space: &AddressSpace,
    candidate: &Candidate<'_>,
    names: &HashMap<NodeId, String>,
    supported: &HashSet<NodeId>,
    support: &mut BTreeSet<Support>,
) -> Result<String, String> {
    let mut used = HashSet::from(["Null".to_owned()]);
    let mut variants = String::new();
    for field in &candidate.definition.fields {
        let form = field_form(space, field, names, supported, support)?;
        let variant = variant_name(field, &mut used);
        variants.push_str(&format!("        {variant}({form}),\n"));
    }
    Ok(format!(
        "\n{}    ///\n    /// The encoded switch value picks the variant; 0 is `Null`.\n    \
         #[derive(Clone, Debug, PartialEq)]\n    pub enum {} {{\n        Null,\n{variants}    }}\n",
        doc(candidate),
        candidate.rust_name
    ))
}

fn structure_item(
    space: &AddressSpace,
    candidate: &Candidate<'_>,
    names: &HashMap<NodeId, String>,
    supported: &HashSet<NodeId>,
    support: &mut BTreeSet<Support>,
) -> Result<String, String> {
    let fields = flattened_fields(space, &candidate.id).ok_or("its supertype chain loops")?;
    let mut used = HashSet::new();
    let mut lines = String::new();
    for field in fields {
        let form = field_form(space, field, names, supported, support)?;
        let mut rust_field = name::field(field.symbolic_name.as_deref().unwrap_or(&field.name));
        while !used.insert(rust_field.clone()) {
            rust_field.push('_');
        }
        lines.push_str(&format!("        pub {rust_field}: {form},\n"));
    }
    Ok(format!(
        "\n{}    #[derive(Clone, Debug, PartialEq)]\n    pub struct {} {{\n{lines}    }}\n",
        doc(candidate),
        candidate.rust_name
    ))
}

/// The structure's own fields behind every field its supertypes contribute (OPC 10000-6 F.12:
/// a Definition never repeats inherited fields).
fn flattened_fields<'a>(
    space: &'a AddressSpace,
    id: &NodeId,
) -> Option<Vec<&'a DataTypeField>> {
    let mut definitions = Vec::new();
    let mut visited = HashSet::new();
    let mut current = id.clone();
    loop {
        if !visited.insert(current.clone()) {
            return None;
        }
        if current == ids::STRUCTURE || current == ids::UNION {
            break;
        }
        if let Some(Node::DataType(node)) = space.node(&current)
            && let Some(definition) = &node.definition
        {
            definitions.push(definition);
        }
        match supertype_of(space, &current) {
            Some(supertype) => current = supertype,
            None => break,
        }
    }
    definitions.reverse();
    Some(
        definitions
            .iter()
            .flat_map(|definition| &definition.fields)
            .collect(),
    )
}

fn supertype_of(
    space: &AddressSpace,
    id: &NodeId,
) -> Option<NodeId> {
    space
        .references(id)
        .into_iter()
        .find(|view| !view.is_forward && view.reference_type == ids::HAS_SUBTYPE)
        .map(|view| view.other)
}

fn field_form(
    space: &AddressSpace,
    field: &DataTypeField,
    names: &HashMap<NodeId, String>,
    supported: &HashSet<NodeId>,
    support: &mut BTreeSet<Support>,
) -> Result<String, String> {
    if field.allow_sub_types {
        return Err(format!("field `{}` allows subtypes of its DataType", field.name));
    }
    let rank = field.value_rank.0;
    if rank < -1 {
        return Err(format!("field `{}` leaves scalar-or-array open (ValueRank {rank})", field.name));
    }
    let Some(id) = space.primary().resolve(&field.data_type).cloned() else {
        return Err(format!("field `{}` names an unresolvable DataType", field.name));
    };
    let Some(core) = core_form(space, &id, names, supported, support) else {
        return Err(format!("field `{}` has no Rust form for {}", field.name, spelled(space, &id)));
    };
    let mut form = core;
    if rank >= 0 {
        form = format!("Vec<{form}>");
    }
    if field.is_optional {
        form = format!("Option<{form}>");
    }
    Ok(form)
}

/// The scalar Rust form of a DataType: a built-in, a generated type, or the built-in ancestor a
/// subtype narrows.
fn core_form(
    space: &AddressSpace,
    id: &NodeId,
    names: &HashMap<NodeId, String>,
    supported: &HashSet<NodeId>,
    support: &mut BTreeSet<Support>,
) -> Option<String> {
    if let Some((form, needs)) = exact_built_in(id) {
        support.extend(needs);
        return Some(form.to_owned());
    }
    if supported.contains(id) {
        return names.get(id).cloned();
    }
    for &(identifier, form, needs) in BUILT_INS {
        if space.is_same_or_subtype_of(id, &base(identifier)) {
            support.extend(needs);
            return Some(form.to_owned());
        }
    }
    space
        .is_same_or_subtype_of(id, &ids::ENUMERATION)
        .then(|| "i32".to_owned())
}

fn exact_built_in(id: &NodeId) -> Option<(&'static str, Option<Support>)> {
    if id.namespace_index != 0 {
        return None;
    }
    let Identifier::Numeric(value) = id.identifier else {
        return None;
    };
    BUILT_INS
        .iter()
        .find(|(identifier, ..)| *identifier == value)
        .map(|&(_, form, needs)| (form, needs))
}

fn spelled(
    space: &AddressSpace,
    id: &NodeId,
) -> String {
    match space.node(id) {
        Some(node) => format!("`{}` ({id})", node.header().browse_name.name),
        None => format!("`{id}`"),
    }
}

fn base(identifier: u32) -> NodeId {
    NodeId {
        namespace_index: 0,
        identifier: Identifier::Numeric(identifier),
    }
}

/// The concrete built-in DataTypes (their ns=0 NodeIds are the values 1..=25 take) and the Rust
/// form each maps to.
const BUILT_INS: &[(u32, &str, Option<Support>)] = &[
    (1, "bool", None),
    (2, "i8", None),
    (3, "u8", None),
    (4, "i16", None),
    (5, "u16", None),
    (6, "i32", None),
    (7, "u32", None),
    (8, "i64", None),
    (9, "u64", None),
    (10, "f32", None),
    (11, "f64", None),
    (12, "String", None),
    (13, "DateTime", Some(Support::DateTime)),
    (14, "Guid", Some(Support::Guid)),
    (15, "Vec<u8>", None),
    (16, "String", None),
    (17, "NodeId", Some(Support::NodeId)),
    (18, "ExpandedNodeId", Some(Support::ExpandedNodeId)),
    (19, "StatusCode", Some(Support::StatusCode)),
    (20, "QualifiedName", Some(Support::QualifiedName)),
    (21, "LocalizedText", Some(Support::LocalizedText)),
    (25, "DiagnosticInfo", Some(Support::DiagnosticInfo)),
];

/// The hand-written types built-in fields lean on, emitted only when something referenced them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Support {
    DateTime,
    StatusCode,
    Guid,
    NodeId,
    ExpandedNodeId,
    QualifiedName,
    LocalizedText,
    DiagnosticInfo,
}

fn support_definitions(support: &BTreeSet<Support>) -> String {
    let mut closed = support.clone();
    if closed.contains(&Support::ExpandedNodeId) {
        closed.insert(Support::NodeId);
    }
    if closed.contains(&Support::NodeId) {
        closed.insert(Support::Guid);
    }
    if closed.contains(&Support::DiagnosticInfo) {
        closed.insert(Support::StatusCode);
    }
    closed.iter().map(|support| support.definition()).collect()
}

impl Support {
    fn definition(self) -> &'static str {
        match self {
            Self::DateTime => {
                "\n    /// 100-nanosecond intervals since 1601-01-01 (UTC).\n    pub type DateTime = i64;\n"
            }
            Self::StatusCode => "\n    /// An OPC UA status code; 0 is Good.\n    pub type StatusCode = u32;\n",
            Self::Guid => {
                "\n    /// A GUID as its 16 bytes.\n    \
                 #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\n    \
                 pub struct Guid(pub [u8; 16]);\n"
            }
            Self::NodeId => {
                "\n    /// A NodeId as the file spells it: `namespace` indexes\n    \
                 /// [`NAMESPACE_URIS`](super::NAMESPACE_URIS).\n    \
                 #[derive(Clone, Debug, PartialEq, Eq)]\n    pub struct NodeId {\n        \
                 pub namespace: u16,\n        pub identifier: Identifier,\n    }\n\n    \
                 #[derive(Clone, Debug, PartialEq, Eq)]\n    pub enum Identifier {\n        \
                 Numeric(u32),\n        String(String),\n        Guid(Guid),\n        \
                 Opaque(Vec<u8>),\n    }\n"
            }
            Self::ExpandedNodeId => {
                "\n    #[derive(Clone, Debug, PartialEq, Eq)]\n    pub struct ExpandedNodeId {\n        \
                 pub server_index: u32,\n        pub namespace_uri: Option<String>,\n        \
                 pub node_id: NodeId,\n    }\n"
            }
            Self::QualifiedName => {
                "\n    /// `namespace_index` indexes [`NAMESPACE_URIS`](super::NAMESPACE_URIS).\n    \
                 #[derive(Clone, Debug, PartialEq, Eq)]\n    pub struct QualifiedName {\n        \
                 pub namespace_index: u16,\n        pub name: String,\n    }\n"
            }
            Self::LocalizedText => {
                "\n    #[derive(Clone, Debug, Default, PartialEq, Eq)]\n    pub struct LocalizedText {\n        \
                 pub locale: Option<String>,\n        pub text: String,\n    }\n"
            }
            Self::DiagnosticInfo => {
                "\n    /// OPC 10000-4 §7.12; the index fields point into the operation's string table.\n    \
                 #[derive(Clone, Debug, Default, PartialEq, Eq)]\n    pub struct DiagnosticInfo {\n        \
                 pub symbolic_id: Option<i32>,\n        pub namespace_uri: Option<i32>,\n        \
                 pub locale: Option<i32>,\n        pub localized_text: Option<i32>,\n        \
                 pub additional_info: Option<String>,\n        pub inner_status_code: Option<StatusCode>,\n        \
                 pub inner_diagnostic_info: Option<Box<DiagnosticInfo>>,\n    }\n"
            }
        }
    }
}

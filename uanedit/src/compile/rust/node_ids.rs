use std::collections::{
    HashMap,
    HashSet,
};

use crate::AddressSpace;
use crate::compile::rust::name;
use crate::nodes::Node;
use crate::types::node_id::{
    Identifier,
    NodeId,
};

/// The `node_ids` module: one constant per primary node, spelled as the file does.
pub(super) fn section(space: &AddressSpace) -> String {
    let nodes: Vec<NodeId> = space.primary_node_ids().collect();
    if nodes.is_empty() {
        return String::new();
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for id in &nodes {
        if let Some(node) = space.node(id) {
            *counts.entry(preferred(node)).or_default() += 1;
        }
    }
    let mut used = HashSet::new();
    let mut items = String::new();
    for id in &nodes {
        let Some(node) = space.node(id) else {
            continue;
        };
        let Some(value) = identifier_value(id) else {
            items.push_str(&format!("\n    // {id} skipped: GUID and ByteString identifiers have no constant form\n"));
            continue;
        };
        let preferred = preferred(node);
        let mut constant = match counts.get(&preferred) {
            Some(1) => preferred,
            _ => format!("{preferred}_{}", suffix(id)),
        };
        while !used.insert(constant.clone()) {
            constant.push('_');
        }
        items.push_str(&format!(
            "\n    /// `{}` — {}, {id}.\n    pub const {constant}: NodeId = NodeId {{ namespace: {}, identifier: {value} }};\n",
            node.header().browse_name.name.replace('`', "'"),
            node.node_class().name(),
            id.namespace_index,
        ));
    }
    format!(
        r#"
/// The nodes this nodeset defines, spelled as the file does.
pub mod node_ids {{
    /// `namespace` indexes [`NAMESPACE_URIS`](super::NAMESPACE_URIS) — the file's table, not the
    /// runtime one the server assigns.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct NodeId {{
        pub namespace: u16,
        pub identifier: Identifier,
    }}

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Identifier {{
        Numeric(u32),
        String(&'static str),
    }}

    impl NodeId {{
        /// The open62541 NodeId, its namespace mapped through
        /// [`namespace_indexes`](super::namespace_indexes) — all zero until
        /// [`insert`](super::insert) ran.
        pub fn ua(self) -> open62541_sys::UA_NodeId {{
            let mut id = open62541_sys::UA_NodeId {{
                namespaceIndex: super::namespace_indexes()[usize::from(self.namespace)],
                identifierType: open62541_sys::UA_NodeIdType::UA_NODEIDTYPE_NUMERIC,
                identifier: open62541_sys::UA_NodeId__bindgen_ty_1::default(),
            }};
            match self.identifier {{
                Identifier::Numeric(value) => unsafe {{ *id.identifier.numeric.as_mut() = value }},
                Identifier::String(value) => {{
                    id.identifierType = open62541_sys::UA_NodeIdType::UA_NODEIDTYPE_STRING;
                    let string = open62541_sys::UA_String {{
                        length: value.len(),
                        data: value.as_ptr().cast_mut(),
                    }};
                    unsafe {{ *id.identifier.string.as_mut() = string }};
                }}
            }}
            id
        }}
    }}
{items}}}
"#
    )
}

fn preferred(node: &Node) -> String {
    let header = node.header();
    name::constant(
        header
            .symbolic_name
            .as_deref()
            .unwrap_or(&header.browse_name.name),
    )
}

fn identifier_value(id: &NodeId) -> Option<String> {
    match &id.identifier {
        Identifier::Numeric(value) => Some(format!("Identifier::Numeric({value})")),
        Identifier::String(value) => Some(format!("Identifier::String({})", name::string_literal(value))),
        Identifier::Guid(_) | Identifier::Opaque(_) => None,
    }
}

fn suffix(id: &NodeId) -> String {
    match &id.identifier {
        Identifier::Numeric(value) => value.to_string(),
        Identifier::String(value) => name::constant(value),
        Identifier::Guid(_) | Identifier::Opaque(_) => String::new(),
    }
}

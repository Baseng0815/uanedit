use std::collections::{
    HashMap,
    HashSet,
};

use crate::AddressSpace;
use crate::compile::text;
use crate::nodes::Node;
use crate::types::node_id::{
    Identifier,
    NodeId,
};

/// The header's `#define` blocks: one identifier constant per primary node, in the shape
/// open62541's `generate_nodeid_header.py` writes, then one `_NODEID` macro per node building
/// the `UA_NodeId` over `<base>_ns`.
pub(super) fn section(
    space: &AddressSpace,
    base: &str,
) -> String {
    let prefix = format!("UA_{}ID_", base.to_uppercase());
    let nodes: Vec<NodeId> = space.primary_node_ids().collect();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for id in &nodes {
        if let Some(node) = space.node(id) {
            *counts.entry(define_name(node)).or_default() += 1;
        }
    }
    let mut used = HashSet::new();
    let mut named = Vec::new();
    let mut out = String::new();
    for id in &nodes {
        let Some(node) = space.node(id) else {
            continue;
        };
        let Some(value) = define_value(id) else {
            out.push_str(&format!("/* {id} skipped: GUID and ByteString identifiers have no define form */\n"));
            continue;
        };
        let preferred = define_name(node);
        let mut name = match counts.get(&preferred) {
            Some(1) => preferred,
            _ => format!("{preferred}_{}", suffix(id)),
        };
        while !used.insert(name.clone()) {
            name.push('_');
        }
        out.push_str(&format!(
            "#define {prefix}{name} {value} /* {}, ns={} */\n",
            node.node_class().name(),
            id.namespace_index
        ));
        named.push((name, id));
    }
    if !named.is_empty() {
        out.push_str(&format!(
            "\n/* The same nodes as UA_NodeId values, over the indexes {base}() put in {base}_ns. */\n"
        ));
    }
    for (name, id) in named {
        let constructor = match &id.identifier {
            Identifier::String(_) => "UA_NODEID_STRING",
            _ => "UA_NODEID_NUMERIC",
        };
        let mut macro_name = format!("{name}_NODEID");
        while !used.insert(macro_name.clone()) {
            macro_name.push('_');
        }
        out.push_str(&format!(
            "#define {prefix}{macro_name} {constructor}({base}_ns[{}], {prefix}{name})\n",
            id.namespace_index
        ));
    }
    out
}

fn define_name(node: &Node) -> String {
    let header = node.header();
    sanitized(
        header
            .symbolic_name
            .as_deref()
            .unwrap_or(&header.browse_name.name),
    )
}

/// Uppercased with every run of other characters collapsed to one inner underscore.
fn sanitized(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut gap = false;
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_uppercase());
            gap = false;
        } else if !gap && !out.is_empty() {
            out.push('_');
            gap = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push('_');
    }
    if out.starts_with(|character: char| character.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn define_value(id: &NodeId) -> Option<String> {
    match &id.identifier {
        Identifier::Numeric(value) => Some(value.to_string()),
        Identifier::String(value) => Some(text::c_string(value)),
        Identifier::Guid(_) | Identifier::Opaque(_) => None,
    }
}

fn suffix(id: &NodeId) -> String {
    match &id.identifier {
        Identifier::Numeric(value) => value.to_string(),
        Identifier::String(value) => sanitized(value),
        Identifier::Guid(_) | Identifier::Opaque(_) => String::new(),
    }
}

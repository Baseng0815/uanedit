use crate::types::date_time::DateTime;
use crate::types::localized_text::LocalizedText;
use crate::types::node_id::{
    Identifier,
    NodeId,
};
use crate::types::qualified_name::QualifiedName;

/// The literal-splitting threshold the reference compiler uses, which keeps a literal inside
/// MSVC's string length limit.
const CHUNK: usize = 500;

/// A C string literal, split into adjacent literals when it is long.
pub(super) fn c_string(text: &str) -> String {
    let mut chunks = vec![String::new()];
    for character in text.chars() {
        let token = match character {
            '\\' => "\\\\",
            '"' => "\\\"",
            '\n' => "\\n",
            '\t' => "\\t",
            '\r' => continue,
            other => {
                grow(&mut chunks);
                chunks.last_mut().expect("chunk").push(other);
                continue;
            }
        };
        grow(&mut chunks);
        chunks.last_mut().expect("chunk").push_str(token);
    }
    match chunks.as_slice() {
        [single] => format!("\"{single}\""),
        many => {
            let mut joined = many
                .iter()
                .map(|chunk| format!("\"{chunk}\""))
                .collect::<Vec<_>>()
                .join(" ");
            joined.push(' ');
            joined
        }
    }
}

fn grow(chunks: &mut Vec<String>) {
    if chunks.last().is_some_and(|chunk| chunk.len() >= CHUNK) {
        chunks.push(String::new());
    }
}

/// The NodeId form the generated calls take; open62541 has none for GUID and ByteString
/// identifiers, which is what makes this fallible.
pub(super) fn node_id(id: &NodeId) -> Option<String> {
    identified("UA_NODEID", id)
}

pub(super) fn expanded_node_id(id: &NodeId) -> Option<String> {
    identified("UA_EXPANDEDNODEID", id)
}

fn identified(
    prefix: &str,
    id: &NodeId,
) -> Option<String> {
    match &id.identifier {
        Identifier::Numeric(value) => Some(format!("{prefix}_NUMERIC(ns[{}], {value}LU)", id.namespace_index)),
        Identifier::String(value) => Some(format!("{prefix}_STRING(ns[{}], {})", id.namespace_index, c_string(value))),
        Identifier::Guid(_) | Identifier::Opaque(_) => None,
    }
}

pub(super) fn qualified_name(name: &QualifiedName) -> String {
    format!("UA_QUALIFIEDNAME(ns[{}], {})", name.namespace_index, c_string(&name.name))
}

pub(super) fn qualified_name_alloc(name: &QualifiedName) -> String {
    format!("UA_QUALIFIEDNAME_ALLOC(ns[{}], {})", name.namespace_index, c_string(&name.name))
}

pub(super) fn localized_text(text: &LocalizedText) -> String {
    spelled_text("UA_LOCALIZEDTEXT", text)
}

pub(super) fn localized_text_alloc(text: &LocalizedText) -> String {
    spelled_text("UA_LOCALIZEDTEXT_ALLOC", text)
}

fn spelled_text(
    macro_name: &str,
    text: &LocalizedText,
) -> String {
    format!("{macro_name}({}, {})", c_string(text.locale.as_deref().unwrap_or("")), c_string(&text.text))
}

/// Milliseconds since the Unix epoch, spelled the way the reference compiler converts them back.
pub(super) fn date_time(value: &DateTime) -> String {
    let milliseconds = value.ticks() / 10_000 - 11_644_473_600_000;
    format!("( (UA_DateTime)({milliseconds} * UA_DATETIME_MSEC) + UA_DATETIME_UNIX_EPOCH)")
}

/// The reference compiler's variable-name stem: class name plus NodeId, lower-cased, with every
/// run of other characters collapsed to one underscore.
pub(super) fn printable(
    prefix: &str,
    id: &NodeId,
) -> String {
    let raw = format!("{prefix}_{id}").to_lowercase();
    let mut out = String::with_capacity(raw.len());
    let mut gap = false;
    for character in raw.chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' {
            out.push(character);
            gap = false;
        } else if !gap {
            out.push('_');
            gap = true;
        }
    }
    out
}

use std::collections::HashSet;

use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::compile::text;
use crate::compile::value::{
    ValueCode,
    variable_value,
};
use crate::nodes::Node;
use crate::nodes::common::NodeHeader;
use crate::types::node_id::NodeId;
use crate::types::node_id_ref::NodeIdRef;
use crate::types::variant::Variant;
use crate::{
    AddressSpace,
    ids,
};

const NULL_ID: &str = "UA_NODEID_NUMERIC(ns[0], 0LU)";

/// One node's whole contribution to the source file: the comment, any file-scope statics, and the
/// begin and finish functions. None when the NodeId has no C form.
pub(super) fn compiled_node(
    space: &AddressSpace,
    id: &NodeId,
    base: &str,
    index: usize,
    printed: &HashSet<NodeId>,
) -> Option<String> {
    let node = space.node(id)?;
    let id_code = text::node_id(id)?;
    let is_method = matches!(node, Node::Method(_));
    let (attributes, cleanup, globals) = attribute_lines(space, node, id);
    let parent = parent_of(space, node, id);
    let type_definition = type_definition_of(space, node, id);

    let mut body = vec!["UA_StatusCode retVal = UA_STATUSCODE_GOOD;".to_owned()];
    body.extend(attributes);
    body.extend(add_node_call(node, &id_code, &parent, &type_definition));
    body.extend(cleanup);
    body.extend(reference_lines(space, node, id, printed, &parent, &type_definition));
    body.push("return retVal;".to_owned());

    let mut out = format!("\n/* {} - {id} */\n", comment_label(node.header()));
    for global in &globals {
        out.push_str(global);
        out.push('\n');
    }
    out.push_str(&format!(
        "\nstatic UA_StatusCode function_{base}_{index}_begin(UA_Server *server, UA_UInt16* ns) {{\n"
    ));
    if is_method {
        out.push_str("#ifdef UA_ENABLE_METHODCALLS\n");
    }
    for line in &body {
        out.push_str(line);
        out.push('\n');
    }
    if is_method {
        out.push_str("#else\nreturn UA_STATUSCODE_GOOD;\n#endif /* UA_ENABLE_METHODCALLS */\n");
    }
    out.push_str("}\n");
    out.push_str(&format!(
        "\nstatic UA_StatusCode function_{base}_{index}_finish(UA_Server *server, UA_UInt16* ns) {{\n"
    ));
    match is_method {
        true => out.push_str(&format!(
            "#ifdef UA_ENABLE_METHODCALLS\nreturn UA_Server_addMethodNode_finish(server, \n{id_code}\n, NULL, 0, NULL, 0, NULL);\n#else\nreturn UA_STATUSCODE_GOOD;\n#endif /* UA_ENABLE_METHODCALLS */\n"
        )),
        false => out.push_str(&format!(
            "return UA_Server_addNode_finish(server, \n{id_code}\n);\n"
        )),
    }
    out.push_str("}\n");
    Some(out)
}

fn comment_label(header: &NodeHeader) -> String {
    let label = match header.display_name.first() {
        Some(text) => match text.locale.as_deref() {
            Some(locale) if !locale.is_empty() => format!("({locale}:{})", text.text),
            _ => text.text.clone(),
        },
        None => header.browse_name.name.clone(),
    };
    label.replace("*/", "* /")
}

fn class_token(node: &Node) -> &'static str {
    match node {
        Node::Object(_) => "OBJECT",
        Node::Variable(_) => "VARIABLE",
        Node::Method(_) => "METHOD",
        Node::View(_) => "VIEW",
        Node::ObjectType(_) => "OBJECTTYPE",
        Node::VariableType(_) => "VARIABLETYPE",
        Node::DataType(_) => "DATATYPE",
        Node::ReferenceType(_) => "REFERENCETYPE",
    }
}

/// The attribute setup, the post-addNode cleanup, and the file-scope statics.
fn attribute_lines(
    space: &AddressSpace,
    node: &Node,
    id: &NodeId,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut cleanup = Vec::new();
    let mut globals = Vec::new();
    match node {
        Node::Object(object) => {
            lines.push("UA_ObjectAttributes attr = UA_ObjectAttributes_default;".to_owned());
            if object.event_notifier.bits() != 0 {
                lines.push(format!("attr.eventNotifier = {};", object.event_notifier.bits()));
            }
        }
        Node::Variable(variable) => {
            lines.push("UA_VariableAttributes attr = UA_VariableAttributes_default;".to_owned());
            if variable.historizing {
                lines.push("attr.historizing = true;".to_owned());
            }
            let interval = match variable.minimum_sampling_interval.is_finite() {
                true => variable.minimum_sampling_interval,
                false => 0.0,
            };
            lines.push(format!("attr.minimumSamplingInterval = {interval:.6};"));
            lines.push(format!("attr.userAccessLevel = {};", variable.user_access_level.bits() & 0xff));
            lines.push(format!("attr.accessLevel = {};", variable.access_level.bits() & 0xff));
            let (block, value_cleanup, value_globals) = variable_block(
                space,
                variable.value.as_ref(),
                &variable.data_type,
                variable.value_rank,
                &variable.array_dimensions,
                &text::printable("variablenode", id),
            );
            lines.extend(block);
            cleanup = value_cleanup;
            globals = value_globals;
        }
        Node::Method(method) => {
            lines.push("UA_MethodAttributes attr = UA_MethodAttributes_default;".to_owned());
            if method.executable {
                lines.push("attr.executable = true;".to_owned());
            }
            if method.user_executable {
                lines.push("attr.userExecutable = true;".to_owned());
            }
        }
        Node::View(view) => {
            lines.push("UA_ViewAttributes attr = UA_ViewAttributes_default;".to_owned());
            if view.contains_no_loops {
                lines.push("attr.containsNoLoops = true;".to_owned());
            }
            lines.push(format!("attr.eventNotifier = (UA_Byte){};", view.event_notifier.bits()));
        }
        Node::ObjectType(object_type) => {
            lines.push("UA_ObjectTypeAttributes attr = UA_ObjectTypeAttributes_default;".to_owned());
            if object_type.is_abstract {
                lines.push("attr.isAbstract = true;".to_owned());
            }
        }
        Node::VariableType(variable_type) => {
            lines.push("UA_VariableTypeAttributes attr = UA_VariableTypeAttributes_default;".to_owned());
            if variable_type.is_abstract {
                lines.push("attr.isAbstract = true;".to_owned());
            }
            let (block, value_cleanup, value_globals) = variable_block(
                space,
                variable_type.value.as_ref(),
                &variable_type.data_type,
                variable_type.value_rank,
                &variable_type.array_dimensions,
                &text::printable("variabletypenode", id),
            );
            lines.extend(block);
            cleanup = value_cleanup;
            globals = value_globals;
        }
        Node::DataType(data_type) => {
            lines.push("UA_DataTypeAttributes attr = UA_DataTypeAttributes_default;".to_owned());
            if data_type.is_abstract {
                lines.push("attr.isAbstract = true;".to_owned());
            }
        }
        Node::ReferenceType(reference_type) => {
            lines.push("UA_ReferenceTypeAttributes attr = UA_ReferenceTypeAttributes_default;".to_owned());
            if reference_type.is_abstract {
                lines.push("attr.isAbstract = true;".to_owned());
            }
            if reference_type.symmetric {
                lines.push("attr.symmetric  = true;".to_owned());
            }
            if let Some(inverse) = reference_type.inverse_name.first() {
                lines.push(format!("attr.inverseName  = {};", text::localized_text(inverse)));
            }
        }
    }

    let header = node.header();
    if let Some(display) = header.display_name.first() {
        lines.push(format!("attr.displayName = {};", text::localized_text(display)));
    }
    if let Some(description) = header.description.first() {
        lines.push("#ifdef UA_ENABLE_NODESET_COMPILER_DESCRIPTIONS".to_owned());
        lines.push(format!("attr.description = {};", text::localized_text(description)));
        lines.push("#endif".to_owned());
    }
    if header.write_mask.bits() != 0 {
        lines.push(format!("attr.writeMask = {};", header.write_mask.bits()));
    }
    if header.user_write_mask.bits() != 0 {
        lines.push(format!("attr.userWriteMask = {};", header.user_write_mask.bits()));
    }
    (lines, cleanup, globals)
}

/// The valueRank, arrayDimensions, dataType and value lines Variables and VariableTypes share.
fn variable_block(
    space: &AddressSpace,
    value: Option<&Variant>,
    data_type: &NodeIdRef,
    value_rank: ValueRank,
    dimensions: &ArrayDimensions,
    stem: &str,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut rank = value_rank.0;
    if rank == -2 && value.is_some_and(|value| !value.is_null() && !value.is_array()) {
        rank = -1;
    }
    lines.push(format!("attr.valueRank = {rank};"));
    let mut stated_dimensions = false;
    if rank > 0 {
        let count = usize::try_from(rank).unwrap_or_default();
        stated_dimensions = dimensions.0.len() == count;
        lines.push(format!("attr.arrayDimensionsSize = {rank};"));
        lines.push(format!("UA_UInt32 arrayDimensions[{rank}];"));
        for index in 0..count {
            let length = match stated_dimensions {
                true => dimensions.0[index],
                false => 0,
            };
            lines.push(format!("arrayDimensions[{index}] = {length};"));
        }
        lines.push("attr.arrayDimensions = &arrayDimensions[0];".to_owned());
    }
    let resolved = space.primary().resolve(data_type).cloned();
    let data_type_code = resolved
        .as_ref()
        .and_then(text::node_id)
        .unwrap_or_else(|| NULL_ID.to_owned());
    lines.push(format!("attr.dataType = {data_type_code};"));

    let mut cleanup = Vec::new();
    let mut globals = Vec::new();
    match value {
        None | Some(Variant::Null) => {}
        Some(value) => match variable_value(value, stem) {
            None => lines.push("/* Cannot encode the value */".to_owned()),
            Some(ValueCode {
                setup,
                cleanup: value_cleanup,
                globals: value_globals,
            }) => {
                lines.extend(setup);
                if matrix_keeps_dimensions(value, rank, dimensions, stated_dimensions) {
                    lines.push("attr.value.arrayDimensionsSize = attr.arrayDimensionsSize;".to_owned());
                    lines.push("attr.value.arrayDimensions = attr.arrayDimensions;".to_owned());
                }
                cleanup = value_cleanup;
                globals = value_globals;
            }
        },
    }
    (lines, cleanup, globals)
}

fn matrix_keeps_dimensions(
    value: &Variant,
    rank: i32,
    dimensions: &ArrayDimensions,
    stated_dimensions: bool,
) -> bool {
    let Variant::Matrix(matrix) = value else {
        return false;
    };
    let product: usize = dimensions.0.iter().map(|length| *length as usize).product();
    rank > 1 && stated_dimensions && !dimensions.0.contains(&0) && product == matrix.elements.len()
}

/// The hierarchical parent the addNode call names, and the reference type that makes it one.
fn parent_of(
    space: &AddressSpace,
    node: &Node,
    id: &NodeId,
) -> Option<(NodeId, NodeId)> {
    let views = space.references(id);
    let hierarchical: Vec<_> = views
        .iter()
        .filter(|view| !view.is_forward && space.is_hierarchical_reference_type(&view.reference_type))
        .collect();
    let stated = node
        .instance()
        .and_then(|instance| instance.parent_node_id.as_ref())
        .and_then(|parent| space.primary().resolve(parent))
        .cloned();
    let chosen = stated
        .and_then(|parent| {
            hierarchical
                .iter()
                .find(|view| view.other == parent)
                .copied()
        })
        .or_else(|| hierarchical.first().copied())?;
    text::node_id(&chosen.other)?;
    text::node_id(&chosen.reference_type)?;
    Some((chosen.other.clone(), chosen.reference_type.clone()))
}

/// The forward HasTypeDefinition target the addNode call carries for Objects and Variables.
fn type_definition_of(
    space: &AddressSpace,
    node: &Node,
    id: &NodeId,
) -> Option<NodeId> {
    if !matches!(node, Node::Object(_) | Node::Variable(_)) {
        return None;
    }
    space
        .references(id)
        .into_iter()
        .find(|view| view.is_forward && view.reference_type == ids::HAS_TYPE_DEFINITION)
        .map(|view| view.other)
}

fn add_node_call(
    node: &Node,
    id_code: &str,
    parent: &Option<(NodeId, NodeId)>,
    type_definition: &Option<NodeId>,
) -> Vec<String> {
    let token = class_token(node);
    let (parent_code, parent_reference_code) = match parent {
        Some((parent, reference)) => (
            text::node_id(parent).unwrap_or_else(|| NULL_ID.to_owned()),
            text::node_id(reference).unwrap_or_else(|| NULL_ID.to_owned()),
        ),
        None => (NULL_ID.to_owned(), NULL_ID.to_owned()),
    };
    let type_definition_line = match node {
        Node::Object(_) | Node::Variable(_) => {
            let code = type_definition
                .as_ref()
                .and_then(text::node_id)
                .unwrap_or_else(|| NULL_ID.to_owned());
            format!("{code},")
        }
        _ => " UA_NODEID_NULL,".to_owned(),
    };
    vec![
        format!("retVal |= UA_Server_addNode_begin(server, UA_NODECLASS_{token},"),
        format!("{id_code},"),
        format!("{parent_code},"),
        format!("{parent_reference_code},"),
        format!("{},", text::qualified_name(&node.header().browse_name)),
        type_definition_line,
        format!("(const UA_NodeAttributes*)&attr, &UA_TYPES[UA_TYPES_{token}ATTRIBUTES],NULL, NULL);"),
    ]
}

/// Every reference not folded into the addNode call, emitted on whichever end compiles later.
fn reference_lines(
    space: &AddressSpace,
    node: &Node,
    id: &NodeId,
    printed: &HashSet<NodeId>,
    parent: &Option<(NodeId, NodeId)>,
    type_definition: &Option<NodeId>,
) -> Vec<String> {
    let folds_type_definition = matches!(node, Node::Object(_) | Node::Variable(_));
    space
        .references(id)
        .into_iter()
        .filter_map(|view| {
            if folds_type_definition
                && view.is_forward
                && view.reference_type == ids::HAS_TYPE_DEFINITION
                && Some(&view.other) == type_definition.as_ref()
            {
                return None;
            }
            if let Some((parent, reference)) = parent
                && view.other == *parent
                && view.reference_type == *reference
            {
                return None;
            }
            match view.other == *id {
                true if !view.is_forward => return None,
                true => {}
                false if !already_printed(space, printed, &view.other) => return None,
                false => {}
            }
            Some(format!(
                "retVal |= UA_Server_addReference(server, {}, {}, {}, {});",
                text::node_id(id)?,
                text::node_id(&view.reference_type)?,
                text::expanded_node_id(&view.other)?,
                view.is_forward
            ))
        })
        .collect()
}

/// Whether the other end already exists when this node's function runs: it sits in a dependency
/// the server is assumed to hold, or it compiled earlier in this file.
fn already_printed(
    space: &AddressSpace,
    printed: &HashSet<NodeId>,
    other: &NodeId,
) -> bool {
    match space.set_of(other) {
        Some(set) if !set.is_primary() => true,
        Some(_) => printed.contains(other),
        None => false,
    }
}

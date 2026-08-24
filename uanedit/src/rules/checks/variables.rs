use crate::attributes::array_dimensions::ArrayDimensions;
use crate::attributes::value_rank::ValueRank;
use crate::ids;
use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::rules::code::{
    DiagnosticCode,
    Severity,
};
use crate::rules::finding::{
    Anchor,
    Finding,
};
use crate::rules::fix::Fix;
use crate::rules::rule::{
    Impact,
    Rule,
    impact_node_and_neighbours,
};
use crate::space::AddressSpace;
use crate::space::delta::{
    Delta,
    NodeField,
};
use crate::types::node_id::NodeId;

/// The Value-shaping attributes a Variable and a VariableType both carry.
struct ValueShape<'a> {
    node_class: NodeClass,
    value_rank: ValueRank,
    array_dimensions: &'a ArrayDimensions,
}

fn value_shape(node: &Node) -> Option<ValueShape<'_>> {
    match node {
        Node::Variable(variable) => Some(ValueShape {
            node_class: NodeClass::Variable,
            value_rank: variable.value_rank,
            array_dimensions: &variable.array_dimensions,
        }),
        Node::VariableType(variable_type) => Some(ValueShape {
            node_class: NodeClass::VariableType,
            value_rank: variable_type.value_rank,
            array_dimensions: &variable_type.array_dimensions,
        }),
        _ => None,
    }
}

/// A Variable states the rule against its Value rather than its ValueRank, so the same deviation
/// is only a warning there (OPC 10000-3 §5.6.2 against §5.6.5).
fn severity_for(node_class: NodeClass) -> Severity {
    match node_class {
        NodeClass::VariableType => Severity::Error,
        _ => Severity::Warning,
    }
}

/// UA0401 and UA0406 — what the DataType attribute has to name.
pub struct DataTypeAttribute {
    code: DiagnosticCode,
}

impl DataTypeAttribute {
    pub const fn new(code: DiagnosticCode) -> Self {
        Self { code }
    }

    pub const CODES: &'static [DiagnosticCode] = &[DiagnosticCode::DataTypeNotADataType, DiagnosticCode::NullDataType];
}

impl Rule for DataTypeAttribute {
    fn code(&self) -> DiagnosticCode {
        self.code
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some(node) = space.node(node_id) else {
            return;
        };
        if !matches!(node.node_class(), NodeClass::Variable | NodeClass::VariableType) {
            return;
        }
        let Some(data_type) = space.data_type(node_id) else {
            return;
        };
        let anchor = || Anchor::field(node_id.clone(), NodeField::DATA_TYPE);
        match self.code {
            DiagnosticCode::NullDataType if data_type.is_null() => out.push(
                Finding::new(self.code, anchor(), "the DataType attribute is null").with_fix(Fix::SetDataType {
                    node: node_id.clone(),
                    data_type: ids::BASE_DATA_TYPE,
                }),
            ),
            DiagnosticCode::DataTypeNotADataType => {
                let found = space.node_class(&data_type);
                if data_type.is_null() || found.is_none_or(|class| class == NodeClass::DataType) {
                    return;
                }
                out.push(
                    Finding::new(
                        self.code,
                        anchor(),
                        format!("the DataType attribute names {data_type}, which is not a DataType"),
                    )
                    .with_facts(&data_type.to_string())
                    .with_fix(Fix::SetDataType {
                        node: node_id.clone(),
                        data_type: ids::BASE_DATA_TYPE,
                    }),
                );
            }
            _ => {}
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
    }
}

/// UA0407 — the InputArguments and OutputArguments Properties of a Method hold `Argument[]`.
pub struct ArgumentPropertyShape;

impl Rule for ArgumentPropertyShape {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::ArgumentPropertyNotArgumentArray
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some(name) = argument_property_of_method(space, node_id) else {
            return;
        };
        if space
            .data_type(node_id)
            .is_some_and(|found| found != ids::ARGUMENT)
        {
            out.push(
                Finding::new(
                    self.code(),
                    Anchor::field(node_id.clone(), NodeField::DATA_TYPE),
                    format!("{name} holds arguments, so its DataType is Argument (i=296)"),
                )
                .with_facts("DataType")
                .with_fix(Fix::SetDataType {
                    node: node_id.clone(),
                    data_type: ids::ARGUMENT,
                }),
            );
        }
        let value_rank = space
            .node(node_id)
            .and_then(value_shape)
            .map(|shape| shape.value_rank);
        if value_rank.is_some_and(|rank| rank != ValueRank::ONE_DIMENSION) {
            out.push(
                Finding::new(
                    self.code(),
                    Anchor::field(node_id.clone(), NodeField::VALUE_RANK),
                    format!("{name} holds an array of Argument, so its ValueRank is 1"),
                )
                .with_facts("ValueRank")
                .with_fix(Fix::SetValueRank {
                    node: node_id.clone(),
                    value_rank: ValueRank::ONE_DIMENSION,
                }),
            );
        }
    }

    fn impact(
        &self,
        space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        impact_node_and_neighbours(space, delta, impact);
    }
}

/// The BrowseName, when the node is a Variable a Method references with HasProperty and names one
/// of the two argument Properties (OPC 10000-3 §5.7.1).
fn argument_property_of_method(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Option<&'static str> {
    if space.node_class(node_id) != Some(NodeClass::Variable) {
        return None;
    }
    let browse_name = space.browse_name(node_id)?;
    if browse_name.namespace_index != 0 {
        return None;
    }
    let name = match browse_name.name.as_str() {
        "InputArguments" => "InputArguments",
        "OutputArguments" => "OutputArguments",
        _ => return None,
    };
    space
        .sources_of_type(node_id, &ids::HAS_PROPERTY, true)
        .iter()
        .any(|source| space.node_class(source) == Some(NodeClass::Method))
        .then_some(name)
}

/// UA0402, UA0403, UA0404 and UA0405 — ValueRank and ArrayDimensions have to agree.
pub struct ValueRankConsistency {
    code: DiagnosticCode,
}

impl ValueRankConsistency {
    pub const fn new(code: DiagnosticCode) -> Self {
        Self { code }
    }

    pub const CODES: &'static [DiagnosticCode] = &[
        DiagnosticCode::ArrayDimensionsRankMismatch,
        DiagnosticCode::ArrayDimensionsWithoutRank,
        DiagnosticCode::UndefinedValueRank,
        DiagnosticCode::MissingArrayDimensions,
    ];

    fn findings(
        space: &AddressSpace,
        node_id: &NodeId,
    ) -> Vec<Finding> {
        let mut out = Vec::new();
        let Some(shape) = space.node(node_id).and_then(value_shape) else {
            return out;
        };
        let rank_anchor = || Anchor::field(node_id.clone(), NodeField::VALUE_RANK);
        let dimensions_anchor = || Anchor::field(node_id.clone(), NodeField::ARRAY_DIMENSIONS);

        if !shape.value_rank.is_defined() {
            out.push(
                Finding::new(
                    DiagnosticCode::UndefinedValueRank,
                    rank_anchor(),
                    format!("ValueRank is {}, which the specification does not define", shape.value_rank.0),
                )
                .with_facts(&shape.value_rank.0.to_string())
                .with_fix(Fix::SetValueRank {
                    node: node_id.clone(),
                    value_rank: ValueRank::ANY,
                }),
            );
            return out;
        }
        match shape.value_rank.dimensions() {
            Some(dimensions) => {
                let stated = shape.array_dimensions.len();
                if stated == 0 {
                    out.push(
                        Finding::new(
                            DiagnosticCode::MissingArrayDimensions,
                            dimensions_anchor(),
                            format!("ValueRank is {dimensions} but ArrayDimensions states no entry"),
                        )
                        .with_severity(severity_for(shape.node_class))
                        .with_fix(Fix::SetArrayDimensions {
                            node: node_id.clone(),
                            array_dimensions: ArrayDimensions(vec![0; dimensions as usize]),
                        }),
                    );
                } else if u32::try_from(stated).is_ok_and(|stated| stated != dimensions) {
                    out.push(
                        Finding::new(
                            DiagnosticCode::ArrayDimensionsRankMismatch,
                            dimensions_anchor(),
                            format!("ValueRank is {dimensions} but ArrayDimensions states {stated} entries"),
                        )
                        .with_facts(&format!("{dimensions}/{stated}"))
                        .with_fix(Fix::SetValueRank {
                            node: node_id.clone(),
                            value_rank: shape.array_dimensions.value_rank(),
                        }),
                    );
                }
            }
            None if !shape.array_dimensions.is_empty() => out.push(
                Finding::new(
                    DiagnosticCode::ArrayDimensionsWithoutRank,
                    dimensions_anchor(),
                    format!(
                        "ValueRank is {} but ArrayDimensions states {} entries",
                        shape.value_rank,
                        shape.array_dimensions.len()
                    ),
                )
                .with_severity(severity_for(shape.node_class))
                .with_fix(Fix::SetArrayDimensions {
                    node: node_id.clone(),
                    array_dimensions: ArrayDimensions::default(),
                }),
            ),
            None => {}
        }
        out
    }
}

impl Rule for ValueRankConsistency {
    fn code(&self) -> DiagnosticCode {
        self.code
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        out.extend(
            Self::findings(space, node_id)
                .into_iter()
                .filter(|finding| finding.code == self.code),
        );
    }

    fn impact(
        &self,
        _space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        if let Some(node_id) = delta.node_id() {
            impact.node(node_id);
        }
    }
}

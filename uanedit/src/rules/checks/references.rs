use indexmap::IndexSet;

use crate::ids;
use crate::nodes::node::Node;
use crate::nodes::node_class::NodeClass;
use crate::rules::code::DiagnosticCode;
use crate::rules::constraints::{
    constraint_for,
    source_class_allows,
    target_type_for,
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

/// Every finding the reference rules make about one edge.
///
/// The picker queries and the edit preconditions call this too, so a reference the engine reports
/// and one an operation refuses can never disagree.
pub fn reference_findings(
    space: &AddressSpace,
    source: &NodeId,
    reference_type: &NodeId,
    target: &NodeId,
) -> Vec<Finding> {
    findings(space, source, reference_type, Some(target), space.node_class(target))
}

/// The same judgement for a target that does not exist yet: a creation knows the NodeClass its new
/// node will have and nothing else about it.
pub fn reference_findings_to_class(
    space: &AddressSpace,
    source: &NodeId,
    reference_type: &NodeId,
    target_class: NodeClass,
) -> Vec<Finding> {
    findings(space, source, reference_type, None, Some(target_class))
}

fn findings(
    space: &AddressSpace,
    source: &NodeId,
    reference_type: &NodeId,
    target: Option<&NodeId>,
    target_class: Option<NodeClass>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let anchor =
        || Anchor::reference(source.clone(), reference_type.clone(), true, target.cloned().unwrap_or_default());
    let remove = || {
        target.map(|target| Fix::RemoveReference {
            holder: source.clone(),
            reference_type: reference_type.clone(),
            is_forward: true,
            target: target.clone(),
        })
    };

    let reference_type_class = space.node_class(reference_type);
    if reference_type_class.is_some_and(|class| class != NodeClass::ReferenceType) {
        out.push(Finding::new(
            DiagnosticCode::ReferenceTypeNotAReferenceType,
            anchor(),
            format!("the reference names {reference_type}, which is not a ReferenceType"),
        ));
        return out;
    }
    if is_abstract_reference_type(space, reference_type) {
        out.push(Finding::new(
            DiagnosticCode::AbstractReferenceType,
            anchor(),
            format!("{reference_type} is an abstract ReferenceType and no reference may be of it"),
        ));
    }
    if target == Some(source) && space.is_hierarchical_reference_type(reference_type) {
        out.push(Finding::new(
            DiagnosticCode::HierarchicalSelfReference,
            anchor(),
            "a hierarchical reference points the node at itself",
        ));
    }

    let constraint = constraint_for(space, reference_type);
    let source_class = space.node_class(source);

    if let Some(source_class) = source_class {
        if !constraint.source.contains(source_class) {
            out.push(with_fix(
                Finding::new(
                    DiagnosticCode::InvalidSourceNodeClass,
                    anchor(),
                    format!("{reference_type} allows {} as its source, not {source_class}", constraint.source),
                ),
                remove(),
            ));
        }
        if !source_class_allows(space, source_class, reference_type) {
            out.push(Finding::new(
                DiagnosticCode::ReferenceNotAllowedForSourceClass,
                anchor(),
                format!("a node of NodeClass {source_class} may not be the source of a {reference_type} reference"),
            ));
        }
    }
    if let Some(target_class) = target_class
        && !constraint.target.contains(target_class)
    {
        out.push(with_fix(
            Finding::new(
                DiagnosticCode::InvalidTargetNodeClass,
                anchor(),
                format!("{reference_type} allows {} as its target, not {target_class}", constraint.target),
            ),
            remove(),
        ));
    }
    if let (Some(source_class), Some(target_class)) = (source_class, target_class)
        && !constraint.relation_holds(source_class, target_class)
    {
        out.push(relation_finding(space, reference_type, anchor(), source_class, target_class));
    }
    if let Some(target) = target.filter(|target| space.contains(target))
        && let Some(required) = target_type_for(space, reference_type)
        && required.is_checkable(space)
        && !required.holds(space, target)
    {
        out.push(with_fix(
            Finding::new(
                DiagnosticCode::ReferenceTargetTypeMismatch,
                anchor(),
                format!("{reference_type} requires its target to be {}, and {target} is not", required.describe()),
            ),
            remove(),
        ));
    }
    out
}

fn with_fix(
    finding: Finding,
    fix: Option<Fix>,
) -> Finding {
    match fix {
        Some(fix) => finding.with_fix(fix),
        None => finding,
    }
}

fn relation_finding(
    space: &AddressSpace,
    reference_type: &NodeId,
    anchor: Anchor,
    source_class: NodeClass,
    target_class: NodeClass,
) -> Finding {
    if space.is_same_or_subtype_of(reference_type, &ids::HAS_SUBTYPE) {
        return Finding::new(
            DiagnosticCode::HasSubtypeAcrossNodeClasses,
            anchor,
            format!("HasSubtype relates a {source_class} to a {target_class}"),
        );
    }
    if space.is_same_or_subtype_of(reference_type, &ids::HAS_TYPE_DEFINITION) {
        return Finding::new(
            DiagnosticCode::TypeDefinitionClassMismatch,
            anchor,
            format!("a {source_class} takes a type definition of another NodeClass than {target_class}"),
        );
    }
    Finding::new(
        DiagnosticCode::InvalidSourceNodeClass,
        anchor,
        format!("a {source_class} may not be the source of a {reference_type} reference to a {target_class}"),
    )
}

fn is_abstract_reference_type(
    space: &AddressSpace,
    reference_type: &NodeId,
) -> bool {
    match space.is_abstract(reference_type) {
        Some(is_abstract) => is_abstract,
        None => crate::space::standard::is_abstract_reference_type(reference_type),
    }
}

/// The rules that judge one reference at a time, sharing [`reference_findings`] with the pickers.
pub struct ReferenceLegality {
    code: DiagnosticCode,
}

impl ReferenceLegality {
    pub const fn new(code: DiagnosticCode) -> Self {
        Self { code }
    }

    /// Every code this one rule implementation can produce, so the engine registers one each.
    pub const CODES: &'static [DiagnosticCode] = &[
        DiagnosticCode::ReferenceTypeNotAReferenceType,
        DiagnosticCode::AbstractReferenceType,
        DiagnosticCode::HierarchicalSelfReference,
        DiagnosticCode::InvalidSourceNodeClass,
        DiagnosticCode::InvalidTargetNodeClass,
        DiagnosticCode::HasSubtypeAcrossNodeClasses,
        DiagnosticCode::TypeDefinitionClassMismatch,
        DiagnosticCode::ReferenceNotAllowedForSourceClass,
        DiagnosticCode::ReferenceTargetTypeMismatch,
    ];
}

impl Rule for ReferenceLegality {
    fn code(&self) -> DiagnosticCode {
        self.code
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        for view in space.forward_references(node_id) {
            out.extend(
                reference_findings(space, node_id, &view.reference_type, &view.other)
                    .into_iter()
                    .filter(|finding| finding.code == self.code),
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

/// UA0205 — a type with more than one supertype.
pub struct MultipleSupertypes;

impl Rule for MultipleSupertypes {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::MultipleSupertypes
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let supertypes = space.supertypes(node_id);
        if supertypes.len() < 2 {
            return;
        }
        let named: Vec<String> = supertypes.iter().map(NodeId::to_string).collect();
        out.push(
            Finding::new(
                self.code(),
                Anchor::node(node_id.clone()),
                format!("the type has {} supertypes: {}", supertypes.len(), named.join(", ")),
            )
            .with_facts(&named.join(","))
            .with_fix(Fix::RemoveReference {
                holder: node_id.clone(),
                reference_type: ids::HAS_SUBTYPE,
                is_forward: false,
                target: supertypes[1].clone(),
            }),
        );
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

/// UA0212 — a type node that names no supertype, outside the four hierarchy roots.
pub struct TypeWithoutSupertype;

/// The roots of the four type hierarchies, the only type nodes with no supertype.
const HIERARCHY_ROOTS: &[NodeId] = &[
    ids::REFERENCES,
    ids::BASE_OBJECT_TYPE,
    ids::BASE_VARIABLE_TYPE,
    ids::BASE_DATA_TYPE,
];

impl Rule for TypeWithoutSupertype {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::TypeWithoutSupertype
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some(node_class) = space.node_class(node_id).filter(|class| class.is_type()) else {
            return;
        };
        if HIERARCHY_ROOTS.contains(node_id) || !space.supertypes(node_id).is_empty() {
            return;
        }
        out.push(Finding::new(
            self.code(),
            Anchor::node(node_id.clone()),
            format!("the {node_class} states no HasSubtype reference to a supertype"),
        ));
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

/// UA0210 — a symmetric ReferenceType that still states an InverseName.
pub struct SymmetricWithInverseName;

impl Rule for SymmetricWithInverseName {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::SymmetricWithInverseName
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some(Node::ReferenceType(reference_type)) = space.node(node_id) else {
            return;
        };
        if !reference_type.symmetric || reference_type.inverse_name.is_empty() {
            return;
        }
        out.push(
            Finding::new(
                self.code(),
                Anchor::field(node_id.clone(), NodeField::INVERSE_NAME),
                "the ReferenceType is symmetric, so it has no inverse meaning to name",
            )
            .with_fix(Fix::SetInverseName {
                node: node_id.clone(),
                inverse_name: Vec::new(),
            }),
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

/// UA0211 — a non-symmetric ReferenceType that names no InverseName.
pub struct MissingInverseName;

impl Rule for MissingInverseName {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::MissingInverseName
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        let Some(Node::ReferenceType(reference_type)) = space.node(node_id) else {
            return;
        };
        if reference_type.symmetric || !reference_type.inverse_name.is_empty() {
            return;
        }
        out.push(Finding::new(
            self.code(),
            Anchor::field(node_id.clone(), NodeField::INVERSE_NAME),
            "the ReferenceType is not symmetric, so it has to name what it means seen from the target",
        ));
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

/// UA0206 — a loop in the HasChild hierarchy.
pub struct HasChildCycle;

impl HasChildCycle {
    fn finding(
        node_id: &NodeId,
        child: &NodeId,
    ) -> Finding {
        Finding::new(
            DiagnosticCode::HasChildCycle,
            Anchor::reference(node_id.clone(), ids::HAS_CHILD, true, child.clone()),
            format!("following HasChild references through {child} leads back to this node"),
        )
        .with_facts(&child.to_string())
    }
}

impl Rule for HasChildCycle {
    fn code(&self) -> DiagnosticCode {
        DiagnosticCode::HasChildCycle
    }

    fn check(
        &self,
        space: &AddressSpace,
        node_id: &NodeId,
        out: &mut Vec<Finding>,
    ) {
        for child in has_child_targets(space, node_id) {
            if space.reaches_via_has_child(&child, node_id) {
                out.push(Self::finding(node_id, &child));
            }
        }
    }

    /// Always a whole pass, so the finding lands on the same node whether it came from report mode
    /// or from an edit, which is what keeps an acknowledgement of it attached.
    fn impact(
        &self,
        _space: &AddressSpace,
        delta: &Delta,
        impact: &mut Impact,
    ) {
        if delta.touches_graph() {
            impact.whole_space();
        }
    }

    fn is_whole_space(&self) -> bool {
        true
    }

    /// One depth-first pass rather than a reachability probe per node, which would be quadratic.
    fn report(
        &self,
        space: &AddressSpace,
        subjects: &[NodeId],
        out: &mut Vec<Finding>,
    ) {
        let mut settled: IndexSet<NodeId> = IndexSet::new();
        for node_id in subjects {
            let mut path = IndexSet::new();
            walk(space, node_id, &mut path, &mut settled, out);
        }
    }
}

/// Deeper than any real hierarchy, and the guard that keeps the walk off the stack limit.
const MAX_DEPTH: usize = 512;

fn walk(
    space: &AddressSpace,
    node_id: &NodeId,
    path: &mut IndexSet<NodeId>,
    settled: &mut IndexSet<NodeId>,
    out: &mut Vec<Finding>,
) {
    if settled.contains(node_id) || path.len() >= MAX_DEPTH || !path.insert(node_id.clone()) {
        return;
    }
    for child in has_child_targets(space, node_id) {
        match path.contains(&child) {
            true => out.push(HasChildCycle::finding(node_id, &child)),
            false => walk(space, &child, path, settled, out),
        }
    }
    path.shift_remove(node_id);
    settled.insert(node_id.clone());
}

fn has_child_targets(
    space: &AddressSpace,
    node_id: &NodeId,
) -> Vec<NodeId> {
    space
        .forward_references(node_id)
        .into_iter()
        .filter(|view| space.is_has_child_reference_type(&view.reference_type))
        .map(|view| view.other)
        .collect()
}

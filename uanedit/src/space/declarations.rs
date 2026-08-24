use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

use crate::ids;
use crate::nodes::node_class::NodeClass;
use crate::space::address_space::AddressSpace;
use crate::space::browse_path::BrowsePath;
use crate::space::reference::ReferenceView;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;

/// A hierarchy this deep is a modelling mistake, and the guard against a looping file.
const MAX_DEPTH: usize = 16;

/// One child a type declares, which every instance of the type answers for
/// (OPC 10000-3 §6.2.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceDeclaration {
    pub path: BrowsePath,
    pub node_id: NodeId,
    pub browse_name: QualifiedName,
    pub node_class: NodeClass,
    /// The hierarchical reference that links the declaration to its declaring node.
    pub reference_type: NodeId,
    pub modelling_rule: Option<ModellingRule>,
    pub type_definition: Option<NodeId>,
    /// The type in the supertype chain this declaration came from, nearest one winning.
    pub declared_by: NodeId,
}

/// The five standard ModellingRules (OPC 10000-5 §8.4), which decide what instantiation must do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ModellingRule {
    Mandatory,
    Optional,
    ExposesItsArray,
    OptionalPlaceholder,
    MandatoryPlaceholder,
}

impl ModellingRule {
    pub fn from_node_id(node_id: &NodeId) -> Option<Self> {
        match node_id {
            id if *id == ids::MODELLING_RULE_MANDATORY => Some(Self::Mandatory),
            id if *id == ids::MODELLING_RULE_OPTIONAL => Some(Self::Optional),
            id if *id == ids::MODELLING_RULE_EXPOSES_ITS_ARRAY => Some(Self::ExposesItsArray),
            id if *id == ids::MODELLING_RULE_OPTIONAL_PLACEHOLDER => Some(Self::OptionalPlaceholder),
            id if *id == ids::MODELLING_RULE_MANDATORY_PLACEHOLDER => Some(Self::MandatoryPlaceholder),
            _ => None,
        }
    }

    pub fn node_id(self) -> NodeId {
        match self {
            Self::Mandatory => ids::MODELLING_RULE_MANDATORY,
            Self::Optional => ids::MODELLING_RULE_OPTIONAL,
            Self::ExposesItsArray => ids::MODELLING_RULE_EXPOSES_ITS_ARRAY,
            Self::OptionalPlaceholder => ids::MODELLING_RULE_OPTIONAL_PLACEHOLDER,
            Self::MandatoryPlaceholder => ids::MODELLING_RULE_MANDATORY_PLACEHOLDER,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Mandatory => "Mandatory",
            Self::Optional => "Optional",
            Self::ExposesItsArray => "ExposesItsArray",
            Self::OptionalPlaceholder => "OptionalPlaceholder",
            Self::MandatoryPlaceholder => "MandatoryPlaceholder",
        }
    }

    /// True for the two rules that stand for any number of children the user names
    /// (OPC 10000-3 §6.4.4.4.4).
    pub fn is_placeholder(self) -> bool {
        matches!(self, Self::OptionalPlaceholder | Self::MandatoryPlaceholder)
    }

    /// True when an instance of the type has to answer for the declaration.
    pub fn is_required(self) -> bool {
        matches!(self, Self::Mandatory | Self::MandatoryPlaceholder)
    }

    /// The rules a subtype may narrow this one to, per OPC 10000-3 §6.4.4.2 Table 21.
    pub fn narrowings(self) -> &'static [Self] {
        match self {
            Self::Mandatory => &[Self::Mandatory],
            Self::Optional => &[Self::Mandatory, Self::Optional],
            Self::ExposesItsArray => &[Self::ExposesItsArray],
            Self::OptionalPlaceholder => &[Self::MandatoryPlaceholder, Self::OptionalPlaceholder],
            Self::MandatoryPlaceholder => &[Self::MandatoryPlaceholder],
        }
    }
}

impl AddressSpace {
    /// The declarations a type states itself, keyed by BrowsePath.
    pub fn own_instance_declarations(
        &self,
        type_node: &NodeId,
    ) -> IndexMap<BrowsePath, InstanceDeclaration> {
        let mut found = IndexMap::new();
        self.collect_declarations(type_node, type_node, &BrowsePath::root(), &mut Vec::new(), &mut found);
        found
    }

    /// Every declaration an instance of the type must answer for, inherited from all supertypes.
    ///
    /// The nearest type in the chain wins, which is how a subtype overrides a declaration
    /// (OPC 10000-3 §6.3.3.3).
    pub fn instance_declarations(
        &self,
        type_node: &NodeId,
    ) -> IndexMap<BrowsePath, InstanceDeclaration> {
        let mut chain = vec![type_node.clone()];
        chain.extend(self.supertype_chain(type_node));
        let mut found = IndexMap::new();
        for declaring in chain.into_iter().rev() {
            for (path, declaration) in self.own_instance_declarations(&declaring) {
                found.insert(path, declaration);
            }
        }
        found
    }

    fn collect_declarations(
        &self,
        declaring: &NodeId,
        node_id: &NodeId,
        path: &BrowsePath,
        ancestors: &mut Vec<NodeId>,
        found: &mut IndexMap<BrowsePath, InstanceDeclaration>,
    ) {
        if path.len() >= MAX_DEPTH || ancestors.contains(node_id) {
            return;
        }
        ancestors.push(node_id.clone());
        for reference in self.forward_references(node_id) {
            if !self.is_hierarchical_reference_type(&reference.reference_type) {
                continue;
            }
            if let Some(declaration) = self.declaration_at(declaring, path, &reference) {
                let child = declaration.node_id.clone();
                let child_path = declaration.path.clone();
                let descend = declaration
                    .modelling_rule
                    .is_none_or(|rule| !rule.is_placeholder());
                found.insert(child_path.clone(), declaration);
                if descend {
                    self.collect_declarations(declaring, &child, &child_path, ancestors, found);
                }
            }
        }
        ancestors.pop();
    }

    /// A hierarchical child that carries a ModellingRule, which is what makes it a declaration.
    fn declaration_at(
        &self,
        declaring: &NodeId,
        path: &BrowsePath,
        reference: &ReferenceView,
    ) -> Option<InstanceDeclaration> {
        let child = &reference.other;
        let node = self.node(child)?;
        let modelling_rule_id = self.modelling_rule(child)?;
        let browse_name = self.browse_name(child)?;
        Some(InstanceDeclaration {
            path: path.child(browse_name.clone()),
            node_id: child.clone(),
            browse_name,
            node_class: node.node_class(),
            reference_type: reference.reference_type.clone(),
            modelling_rule: ModellingRule::from_node_id(&modelling_rule_id),
            type_definition: self.type_definition(child),
            declared_by: declaring.clone(),
        })
    }

    /// True for a node that carries a ModellingRule and hangs under a type (OPC 10000-3 §6.2.1).
    pub fn is_instance_declaration(
        &self,
        node_id: &NodeId,
    ) -> bool {
        if self.modelling_rule(node_id).is_none() {
            return false;
        }
        let mut current = node_id.clone();
        for _ in 0..MAX_DEPTH {
            let Some(parent) = self.parents(&current).into_iter().next() else {
                return false;
            };
            match self.node_class(&parent) {
                Some(NodeClass::ObjectType | NodeClass::VariableType) => return true,
                Some(_) if self.modelling_rule(&parent).is_some() => current = parent,
                _ => return false,
            }
        }
        false
    }
}

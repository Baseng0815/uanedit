use serde::{
    Deserialize,
    Serialize,
};

use crate::nodes::common::{
    InstanceHeader,
    NodeHeader,
};
use crate::nodes::data_type::DataType;
use crate::nodes::method::Method;
use crate::nodes::node_class::NodeClass;
use crate::nodes::object::Object;
use crate::nodes::object_type::ObjectType;
use crate::nodes::reference::Reference;
use crate::nodes::reference_type::ReferenceType;
use crate::nodes::variable::Variable;
use crate::nodes::variable_type::VariableType;
use crate::nodes::view::View;
use crate::types::node_id::NodeId;
use crate::types::qualified_name::QualifiedName;

/// Any node in an address space, discriminated by its NodeClass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Node {
    Object(Object),
    Variable(Variable),
    Method(Method),
    View(View),
    ObjectType(ObjectType),
    VariableType(VariableType),
    DataType(DataType),
    ReferenceType(ReferenceType),
}

/// Dispatches a method over every node class, since each holds a different struct.
macro_rules! over_classes {
    ($self:expr, $node:ident => $body:expr) => {
        match $self {
            Node::Object($node) => $body,
            Node::Variable($node) => $body,
            Node::Method($node) => $body,
            Node::View($node) => $body,
            Node::ObjectType($node) => $body,
            Node::VariableType($node) => $body,
            Node::DataType($node) => $body,
            Node::ReferenceType($node) => $body,
        }
    };
}

impl Node {
    pub fn node_class(&self) -> NodeClass {
        match self {
            Self::Object(_) => NodeClass::Object,
            Self::Variable(_) => NodeClass::Variable,
            Self::Method(_) => NodeClass::Method,
            Self::View(_) => NodeClass::View,
            Self::ObjectType(_) => NodeClass::ObjectType,
            Self::VariableType(_) => NodeClass::VariableType,
            Self::DataType(_) => NodeClass::DataType,
            Self::ReferenceType(_) => NodeClass::ReferenceType,
        }
    }

    pub fn header(&self) -> &NodeHeader {
        over_classes!(self, node => &node.header)
    }

    pub fn header_mut(&mut self) -> &mut NodeHeader {
        over_classes!(self, node => &mut node.header)
    }

    /// The instance-only attributes, absent on the four type classes.
    pub fn instance(&self) -> Option<&InstanceHeader> {
        match self {
            Self::Object(node) => Some(&node.instance),
            Self::Variable(node) => Some(&node.instance),
            Self::Method(node) => Some(&node.instance),
            Self::View(node) => Some(&node.instance),
            _ => None,
        }
    }

    pub fn instance_mut(&mut self) -> Option<&mut InstanceHeader> {
        match self {
            Self::Object(node) => Some(&mut node.instance),
            Self::Variable(node) => Some(&mut node.instance),
            Self::Method(node) => Some(&mut node.instance),
            Self::View(node) => Some(&mut node.instance),
            _ => None,
        }
    }

    /// Whether the node is a type that cannot itself be instantiated.
    pub fn is_abstract(&self) -> Option<bool> {
        match self {
            Self::ObjectType(node) => Some(node.is_abstract),
            Self::VariableType(node) => Some(node.is_abstract),
            Self::DataType(node) => Some(node.is_abstract),
            Self::ReferenceType(node) => Some(node.is_abstract),
            _ => None,
        }
    }

    pub fn node_id(&self) -> &NodeId {
        &self.header().node_id
    }

    pub fn browse_name(&self) -> &QualifiedName {
        &self.header().browse_name
    }

    pub fn references(&self) -> &[Reference] {
        &self.header().references
    }

    pub fn references_mut(&mut self) -> &mut Vec<Reference> {
        &mut self.header_mut().references
    }

    /// True for nodes the standard defines, which an editor shows but never changes.
    pub fn is_read_only(&self) -> bool {
        self.node_id().is_in_base_namespace()
    }
}

macro_rules! node_from {
    ($($source:ident),* $(,)?) => {
        $(
            impl From<$source> for Node {
                fn from(node: $source) -> Self {
                    Self::$source(node)
                }
            }
        )*
    };
}

node_from!(Object, Variable, Method, View, ObjectType, VariableType, DataType, ReferenceType);

use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

use crate::nodes::node_class::NodeClass;

/// A set of NodeClasses, which is what a reference-type constraint and a picker filter both are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeClassSet(u32);

impl NodeClassSet {
    pub const NONE: Self = Self(0);
    pub const ANY: Self = Self::of(&NodeClass::ALL);
    pub const INSTANCES: Self = Self::of(&[
        NodeClass::Object,
        NodeClass::Variable,
        NodeClass::Method,
        NodeClass::View,
    ]);
    pub const TYPES: Self = Self::of(&[
        NodeClass::ObjectType,
        NodeClass::VariableType,
        NodeClass::DataType,
        NodeClass::ReferenceType,
    ]);

    pub const fn of(classes: &[NodeClass]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < classes.len() {
            bits |= classes[index] as u32;
            index += 1;
        }
        Self(bits)
    }

    pub fn contains(
        self,
        node_class: NodeClass,
    ) -> bool {
        self.0 & node_class.bits() != 0
    }

    pub fn intersection(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 & other.0)
    }

    pub fn union(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 | other.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = NodeClass> {
        NodeClass::ALL
            .into_iter()
            .filter(move |class| self.contains(*class))
    }
}

impl FromIterator<NodeClass> for NodeClassSet {
    fn from_iter<T: IntoIterator<Item = NodeClass>>(classes: T) -> Self {
        classes
            .into_iter()
            .fold(Self::NONE, |set, class| set.union(Self::of(&[class])))
    }
}

impl fmt::Display for NodeClassSet {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        for (position, class) in self.iter().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            f.write_str(class.name())?;
        }
        Ok(())
    }
}

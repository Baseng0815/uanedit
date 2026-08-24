//! One editable nodeset and its dependencies, seen as a single graph.
//!
//! The nodesets keep their own namespace tables, so the space resolves NodeIds and QualifiedNames
//! across files through the namespace URIs and hands out indexes into a table of its own. The
//! primary nodeset's table comes first, which makes a space NodeId and a primary-file NodeId the
//! same thing.

pub mod address_space;
pub mod attributes;
pub mod browse_path;
pub mod declarations;
pub mod delta;
mod graph;
pub(crate) mod index;
pub(crate) mod keys;
pub mod reference;
pub mod set;
pub mod standard;

pub use address_space::AddressSpace;
pub use attributes::AttributeSlot;
pub use browse_path::BrowsePath;
pub use declarations::{
    InstanceDeclaration,
    ModellingRule,
};
pub use delta::{
    Delta,
    NodeField,
};
pub use reference::{
    ReferenceStorage,
    ReferenceView,
};
pub use set::SetId;

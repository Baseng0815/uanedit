//! Types that attribute values take, shared across the node classes that carry them.

pub mod access_level;
pub mod access_restrictions;
pub mod array_dimensions;
pub mod attribute_id;
pub mod event_notifier;
pub(crate) mod mask;
pub mod permissions;
pub mod value_rank;
pub mod write_mask;

pub use access_level::AccessLevel;
pub use access_restrictions::AccessRestrictions;
pub use array_dimensions::ArrayDimensions;
pub use attribute_id::AttributeId;
pub use event_notifier::EventNotifier;
pub use permissions::{
    PermissionType,
    RolePermission,
};
pub use value_rank::ValueRank;
pub use write_mask::WriteMask;

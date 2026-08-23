use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::mask::bitmask;
use crate::types::node_id_ref::NodeIdRef;

bitmask! {
    /// What a Role is permitted to do with a node (OPC 10000-3 §8.55).
    PermissionType: u32 {
        BROWSE = 0,
        READ_ROLE_PERMISSIONS = 1,
        WRITE_ATTRIBUTE = 2,
        WRITE_ROLE_PERMISSIONS = 3,
        WRITE_HISTORIZING = 4,
        READ = 5,
        WRITE = 6,
        READ_HISTORY = 7,
        INSERT_HISTORY = 8,
        MODIFY_HISTORY = 9,
        DELETE_HISTORY = 10,
        RECEIVE_EVENTS = 11,
        CALL = 12,
        ADD_REFERENCE = 13,
        REMOVE_REFERENCE = 14,
        DELETE_NODE = 15,
        ADD_NODE = 16,
    }
}

/// One Role and the permissions it is granted on a node (OPC 10000-3 §5.2.9).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolePermission {
    pub role_id: NodeIdRef,
    pub permissions: PermissionType,
}

impl RolePermission {
    pub fn new(
        role_id: impl Into<NodeIdRef>,
        permissions: PermissionType,
    ) -> Self {
        Self {
            role_id: role_id.into(),
            permissions,
        }
    }
}

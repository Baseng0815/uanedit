use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

/// The numeric identifiers the services use to name attributes (OPC 10000-6 Annex A.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum AttributeId {
    NodeId              = 1,
    NodeClass           = 2,
    BrowseName          = 3,
    DisplayName         = 4,
    Description         = 5,
    WriteMask           = 6,
    UserWriteMask       = 7,
    IsAbstract          = 8,
    Symmetric           = 9,
    InverseName         = 10,
    ContainsNoLoops     = 11,
    EventNotifier       = 12,
    Value               = 13,
    DataType            = 14,
    ValueRank           = 15,
    ArrayDimensions     = 16,
    AccessLevel         = 17,
    UserAccessLevel     = 18,
    MinimumSamplingInterval = 19,
    Historizing         = 20,
    Executable          = 21,
    UserExecutable      = 22,
    DataTypeDefinition  = 23,
    RolePermissions     = 24,
    UserRolePermissions = 25,
    AccessRestrictions  = 26,
    AccessLevelEx       = 27,
}

impl AttributeId {
    pub const ALL: [Self; 27] = [
        Self::NodeId,
        Self::NodeClass,
        Self::BrowseName,
        Self::DisplayName,
        Self::Description,
        Self::WriteMask,
        Self::UserWriteMask,
        Self::IsAbstract,
        Self::Symmetric,
        Self::InverseName,
        Self::ContainsNoLoops,
        Self::EventNotifier,
        Self::Value,
        Self::DataType,
        Self::ValueRank,
        Self::ArrayDimensions,
        Self::AccessLevel,
        Self::UserAccessLevel,
        Self::MinimumSamplingInterval,
        Self::Historizing,
        Self::Executable,
        Self::UserExecutable,
        Self::DataTypeDefinition,
        Self::RolePermissions,
        Self::UserRolePermissions,
        Self::AccessRestrictions,
        Self::AccessLevelEx,
    ];

    pub fn id(self) -> u32 {
        self as u32
    }

    pub fn from_id(id: u32) -> Option<Self> {
        let index = usize::try_from(id).ok()?.checked_sub(1)?;
        Self::ALL.get(index).copied()
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self.id() as usize - 1]
    }

    const NAMES: [&'static str; 27] = [
        "NodeId",
        "NodeClass",
        "BrowseName",
        "DisplayName",
        "Description",
        "WriteMask",
        "UserWriteMask",
        "IsAbstract",
        "Symmetric",
        "InverseName",
        "ContainsNoLoops",
        "EventNotifier",
        "Value",
        "DataType",
        "ValueRank",
        "ArrayDimensions",
        "AccessLevel",
        "UserAccessLevel",
        "MinimumSamplingInterval",
        "Historizing",
        "Executable",
        "UserExecutable",
        "DataTypeDefinition",
        "RolePermissions",
        "UserRolePermissions",
        "AccessRestrictions",
        "AccessLevelEx",
    ];
}

impl fmt::Display for AttributeId {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.name())
    }
}

use crate::attributes::mask::bitmask;

bitmask! {
    /// How a Variable's value may be accessed (OPC 10000-3 §8.57, §8.58).
    ///
    /// The low eight bits are AccessLevelType; the rest are the AccessLevelEx extension, which
    /// NodeSet2 carries in the same UInt32 attribute rather than a separate one.
    AccessLevel: u32 {
        CURRENT_READ = 0,
        CURRENT_WRITE = 1,
        HISTORY_READ = 2,
        HISTORY_WRITE = 3,
        SEMANTIC_CHANGE = 4,
        STATUS_WRITE = 5,
        TIMESTAMP_WRITE = 6,
        NONATOMIC_READ = 8,
        NONATOMIC_WRITE = 9,
        WRITE_FULL_ARRAY_ONLY = 10,
        NO_SUB_DATA_TYPES = 11,
        NON_VOLATILE = 12,
        CONSTANT = 13,
    }
}

impl AccessLevel {
    /// What the UANodeSet schema assumes when the attribute is absent.
    pub const DEFAULT: Self = Self::CURRENT_READ;

    pub fn is_readable(self) -> bool {
        self.contains(Self::CURRENT_READ)
    }

    pub fn is_writable(self) -> bool {
        self.contains(Self::CURRENT_WRITE)
    }
}

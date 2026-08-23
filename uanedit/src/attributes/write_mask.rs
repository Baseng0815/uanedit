use crate::attributes::mask::bitmask;

bitmask! {
    /// Which attributes of a node are writable (OPC 10000-3 §8.60).
    ///
    /// The bits are ordered by attribute name, not by AttributeId.
    WriteMask: u32 {
        ACCESS_LEVEL = 0,
        ARRAY_DIMENSIONS = 1,
        BROWSE_NAME = 2,
        CONTAINS_NO_LOOPS = 3,
        DATA_TYPE = 4,
        DESCRIPTION = 5,
        DISPLAY_NAME = 6,
        EVENT_NOTIFIER = 7,
        EXECUTABLE = 8,
        HISTORIZING = 9,
        INVERSE_NAME = 10,
        IS_ABSTRACT = 11,
        MINIMUM_SAMPLING_INTERVAL = 12,
        NODE_CLASS = 13,
        NODE_ID = 14,
        SYMMETRIC = 15,
        USER_ACCESS_LEVEL = 16,
        USER_EXECUTABLE = 17,
        USER_WRITE_MASK = 18,
        VALUE_RANK = 19,
        WRITE_MASK = 20,
        VALUE_FOR_VARIABLE_TYPE = 21,
        DATA_TYPE_DEFINITION = 22,
        ROLE_PERMISSIONS = 23,
        ACCESS_RESTRICTIONS = 24,
        ACCESS_LEVEL_EX = 25,
    }
}

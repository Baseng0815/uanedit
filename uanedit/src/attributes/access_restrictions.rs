use crate::attributes::mask::bitmask;

bitmask! {
    /// The message security a client must use to reach a node (OPC 10000-3 §8.56).
    AccessRestrictions: u16 {
        SIGNING_REQUIRED = 0,
        ENCRYPTION_REQUIRED = 1,
        SESSION_REQUIRED = 2,
        APPLY_RESTRICTIONS_TO_BROWSE = 3,
    }
}

use crate::attributes::mask::bitmask;

bitmask! {
    /// Whether a node can be subscribed to for Events, and whether its history is available
    /// (OPC 10000-3 §8.59).
    EventNotifier: u8 {
        SUBSCRIBE_TO_EVENTS = 0,
        HISTORY_READ = 2,
        HISTORY_WRITE = 3,
    }
}

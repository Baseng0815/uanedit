use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::event_notifier::EventNotifier;
use crate::nodes::common::{
    InstanceHeader,
    NodeHeader,
};

/// An Object node (OPC 10000-3 §5.5).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Object {
    pub header: NodeHeader,
    pub instance: InstanceHeader,
    pub event_notifier: EventNotifier,
}

use serde::{
    Deserialize,
    Serialize,
};

use crate::attributes::event_notifier::EventNotifier;
use crate::nodes::common::{
    InstanceHeader,
    NodeHeader,
};

/// A View node (OPC 10000-3 §5.4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    pub header: NodeHeader,
    pub instance: InstanceHeader,
    pub contains_no_loops: bool,
    pub event_notifier: EventNotifier,
}

//! The one encoding of the OPC 10000-3 and 10000-5 constraints.
//!
//! Three consumers, one engine (general/guardrails.md §1): the validation panel runs it in report
//! mode, the pickers ask it what a choice list may hold, and the edit operations ask it whether an
//! operation would introduce a finding before they perform it.

pub mod acknowledge;
pub mod checks;
pub mod code;
pub mod constraints;
pub mod engine;
pub mod finding;
pub mod fingerprint;
pub mod fix;
pub mod node_class_set;
pub mod plan;
pub mod query;
pub mod rule;

pub use acknowledge::{
    Acknowledgement,
    Acknowledgements,
    Baseline,
};
pub use code::{
    DiagnosticCode,
    Severity,
};
pub use engine::{
    Diagnostics,
    FindingCounts,
    RuleEngine,
    Scope,
};
pub use finding::{
    Anchor,
    AnchorDetail,
    Finding,
    Origin,
};
pub use fingerprint::Fingerprint;
pub use fix::Fix;
pub use node_class_set::NodeClassSet;
pub use plan::{
    InheritedDeclaration,
    InstantiationPlan,
    PlannedDeclaration,
    SubtypePlan,
    instantiation_plan,
    subtype_plan,
};
pub use query::Verdict;
pub use rule::{
    Impact,
    Rule,
};

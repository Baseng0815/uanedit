use core::fmt;

use serde::{
    Deserialize,
    Serialize,
};

use crate::rules::code::{
    DiagnosticCode,
    Severity,
};
use crate::rules::fingerprint::Fingerprint;
use crate::rules::fix::Fix;
use crate::space::browse_path::BrowsePath;
use crate::space::delta::NodeField;
use crate::types::node_id::{
    NamespaceIndex,
    NodeId,
};

/// Whether a finding was already in the file when it was opened or an edit created it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Origin {
    /// Present in the file as loaded, so not this editor's doing.
    Inherited,
    #[default]
    /// Created by an edit made here, which for an error means the override was used.
    Introduced,
}

/// The node a finding is about, and the part of it the rule complained about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub node: NodeId,
    pub detail: Option<AnchorDetail>,
}

/// Which reference, field or namespace of the anchored node the finding is about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnchorDetail {
    Reference {
        reference_type: NodeId,
        is_forward: bool,
        other: NodeId,
    },
    Field(NodeField),
    Namespace(NamespaceIndex),
    /// The instance declaration of the node's type the finding is about.
    Declaration(BrowsePath),
}

impl Anchor {
    pub fn node(node: NodeId) -> Self {
        Self { node, detail: None }
    }

    pub fn field(
        node: NodeId,
        field: NodeField,
    ) -> Self {
        Self {
            node,
            detail: Some(AnchorDetail::Field(field)),
        }
    }

    pub fn reference(
        node: NodeId,
        reference_type: NodeId,
        is_forward: bool,
        other: NodeId,
    ) -> Self {
        Self {
            node,
            detail: Some(AnchorDetail::Reference {
                reference_type,
                is_forward,
                other,
            }),
        }
    }

    pub fn namespace(
        node: NodeId,
        index: NamespaceIndex,
    ) -> Self {
        Self {
            node,
            detail: Some(AnchorDetail::Namespace(index)),
        }
    }

    pub fn declaration(
        node: NodeId,
        path: BrowsePath,
    ) -> Self {
        Self {
            node,
            detail: Some(AnchorDetail::Declaration(path)),
        }
    }
}

impl fmt::Display for AnchorDetail {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Reference {
                reference_type,
                is_forward,
                other,
            } => {
                let arrow = match is_forward {
                    true => "->",
                    false => "<-",
                };
                write!(f, "{reference_type}{arrow}{other}")
            }
            Self::Field(field) => write!(f, "{field}"),
            Self::Namespace(index) => write!(f, "ns={index}"),
            Self::Declaration(path) => write!(f, "{path}"),
        }
    }
}

/// One thing a rule found, anchored at a node and carrying the fix that would resolve it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub anchor: Anchor,
    pub fix: Option<Fix>,
    pub fingerprint: Fingerprint,
    pub origin: Origin,
    /// True when the user has reviewed the finding and muted it; it stays counted.
    pub acknowledged: bool,
    /// True when the finding exists because an edit was forced past the engine (guardrails §5).
    pub via_override: bool,
}

impl Finding {
    pub fn new(
        code: DiagnosticCode,
        anchor: Anchor,
        message: impl Into<String>,
    ) -> Self {
        let fingerprint = Fingerprint::new(code, &anchor.node, &facts(&anchor, ""));
        Self {
            code,
            severity: code.severity(),
            message: message.into(),
            anchor,
            fix: None,
            fingerprint,
            origin: Origin::default(),
            acknowledged: false,
            via_override: false,
        }
    }

    /// Adds to what the fingerprint hashes, so the finding expires when these values change.
    pub fn with_facts(
        mut self,
        extra: &str,
    ) -> Self {
        self.fingerprint = Fingerprint::new(self.code, &self.anchor.node, &facts(&self.anchor, extra));
        self
    }

    pub fn with_fix(
        mut self,
        fix: Fix,
    ) -> Self {
        self.fix = Some(fix);
        self
    }

    pub fn with_severity(
        mut self,
        severity: Severity,
    ) -> Self {
        self.severity = severity;
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

fn facts(
    anchor: &Anchor,
    extra: &str,
) -> String {
    match &anchor.detail {
        Some(detail) => format!("{detail}|{extra}"),
        None => extra.to_owned(),
    }
}

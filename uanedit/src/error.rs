use thiserror::Error;

/// A textual form that does not match the grammar the spec defines for it.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("`{0}` is not a NodeId")]
    NodeId(String),
    #[error("`{0}` is not an ExpandedNodeId")]
    ExpandedNodeId(String),
    #[error("`{0}` is not a QualifiedName")]
    QualifiedName(String),
    #[error("`{0}` is not a Guid")]
    Guid(String),
    #[error("`{0}` is not valid base64")]
    Base64(String),
    #[error("`{0}` is not a UTC dateTime")]
    DateTime(String),
    #[error("`{0}` is not a comma-separated list of array dimensions")]
    ArrayDimensions(String),
    #[error("`{0}` is not a release status")]
    ReleaseStatus(String),
    #[error("`{0}` is not a data type purpose")]
    DataTypePurpose(String),
    #[error("`{0}` is not a node class")]
    NodeClass(String),
}

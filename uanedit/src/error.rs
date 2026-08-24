use serde::{
    Deserialize,
    Serialize,
};
use thiserror::Error;

/// A textual form that does not match the grammar the spec defines for it.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
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
    #[error("`{0}` is not a boolean")]
    Boolean(String),
    #[error("`{0}` is not an integer")]
    Integer(String),
    #[error("`{0}` is not a floating-point number")]
    Double(String),
    #[error("`{0}` is not a release status")]
    ReleaseStatus(String),
    #[error("`{0}` is not a data type purpose")]
    DataTypePurpose(String),
    #[error("`{0}` is not a node class")]
    NodeClass(String),
}

/// A document this crate cannot read at all, as opposed to one it reads with reservations.
///
/// Everything a file can get wrong that still leaves a nodeset behind is a
/// [`Diagnosis`](crate::report::Diagnosis) in the open-file report instead.
#[derive(Clone, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum DocumentError {
    #[error("the file is not valid XML at byte {offset}: {message}")]
    Syntax { offset: usize, message: String },
    #[error("the file is not UTF-8 text, so it is not a NodeSet2 document")]
    NotUtf8,
    #[error("the file has no root element")]
    NoRootElement,
    #[error(
        "this is a `UANodeSetChanges` file, which describes edits to a nodeset rather than a nodeset. uanedit \
         edits complete nodesets; apply the changes with the tool that produced them and open the result."
    )]
    NodeSetChanges,
    #[error("the root element is `{0}`, so the file is not a NodeSet2 document")]
    UnexpectedRoot(String),
}

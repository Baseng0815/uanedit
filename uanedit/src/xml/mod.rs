//! Reading and writing `.NodeSet2.xml` documents (OPC 10000-6 Annex F).
//!
//! Reading keeps the document's bytes beside the model it produced, so writing a nodeset that was
//! not edited returns exactly the file that was read, and writing one that was edited changes only
//! the elements whose model no longer matches what the source says.

mod cursor;
mod document;
mod read;
mod span;
mod write;

/// A `Value` payload declares it, and the display renderer beside the codec writes it too.
pub use crate::emit::TYPES_NAMESPACE;
/// The open-file report is plain data and lives outside this module, so the half of a dependent
/// that does not link the codec can still read it.
pub use crate::report::{
    Diagnosis,
    Finding,
    OpenReport,
    Position,
    Preserved,
    PreservedKind,
};
pub use document::Document;
pub use span::{
    Layout,
    NodeRegion,
    Span,
    TableRegion,
};
pub use write::{
    NODESET_NAMESPACE,
    RenderedNode,
    nodeset as write,
};

use crate::error::DocumentError;

/// Reads a NodeSet2 document, keeping its bytes so that saving it can splice them back.
pub fn parse(source: &str) -> Result<Document, DocumentError> {
    Document::parse(source)
}

/// Reads a NodeSet2 document from the bytes of a file.
pub fn parse_bytes(bytes: &[u8]) -> Result<Document, DocumentError> {
    Document::parse_bytes(bytes)
}

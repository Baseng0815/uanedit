//! The OPC UA address space model a `.NodeSet2.xml` file describes.
//!
//! The types mirror the UANodeSet schema (OPC 10000-6 Annex F) rather than the service-facing
//! model of OPC 10000-3, because the file is what this crate reads, edits and writes. Where the
//! schema permits more than one spelling of the same value, the spelling that was read is kept, so
//! saving a file that was not edited does not rewrite it.

pub mod error;
pub mod ids;
pub mod nodeset;
pub mod types;

pub use error::ParseError;

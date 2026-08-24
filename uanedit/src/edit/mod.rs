//! The semantic edit operations and the undo log over an address space.
//!
//! One [`Session`] is one open document. Every change is an [`Operation`]: a transaction that asks
//! for everything the specification requires in one call, is refused whole when the rules engine
//! would report an error over its result, and undoes as one step (general/guardrails.md §1.1, §3).
//! The refusals carry the engine's own findings, so the editor shows the same text the validation
//! panel would.

mod change;
mod compile;
pub mod create;
pub mod delete;
pub mod field;
pub mod instantiate;
pub mod operation;
pub mod outcome;
pub mod reference;
pub mod session;
pub mod subtype;
mod table;

pub use create::{
    CreateInstance,
    CreateType,
    InstanceAttributes,
    TypeAttributes,
};
pub use delete::{
    AttributeResolution,
    AttributeUse,
    ChildResolution,
    DeleteNode,
    DeletionPlan,
    IncomingReference,
    IncomingResolution,
    OwnedChild,
};
pub use field::FieldValue;
pub use instantiate::{
    InstantiateType,
    PlaceholderChild,
    Selections,
    TypeNarrowing,
};
pub use operation::Operation;
pub use outcome::{
    Applied,
    Refusal,
};
pub use reference::{
    AddReference,
    ReferenceEnd,
    ReferenceKey,
};
pub use session::Session;
pub use subtype::{
    CreateSubtype,
    DeclarationOverride,
    MethodArguments,
};
pub use table::{
    alias_users,
    namespace_users,
};

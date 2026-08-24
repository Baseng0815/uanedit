//! The rules themselves, one module per tier of the validation the architecture defines.

pub mod instances;
pub mod references;
pub mod structural;
pub mod variables;

use crate::rules::code::DiagnosticCode;
use crate::rules::rule::Rule;

/// Every rule the engine runs, one entry per diagnostic code.
pub fn all() -> Vec<Box<dyn Rule>> {
    let mut rules: Vec<Box<dyn Rule>> = vec![
        Box::new(structural::DanglingReferenceTarget),
        Box::new(structural::DuplicateNodeId),
        Box::new(structural::SiblingBrowseNameCollision),
        Box::new(structural::DuplicatePropertyBrowseName),
        Box::new(structural::UnknownNamespaceIndex),
        Box::new(structural::UnknownAlias),
        Box::new(structural::DuplicateReference),
        Box::new(structural::NullNodeId),
        Box::new(structural::ParentNodeIdMismatch),
        Box::new(references::MultipleSupertypes),
        Box::new(references::TypeWithoutSupertype),
        Box::new(references::SymmetricWithInverseName),
        Box::new(references::MissingInverseName),
        Box::new(references::HasChildCycle),
        Box::new(instances::MissingMandatoryChild),
        Box::new(instances::MissingMandatoryPlaceholderChild),
        Box::new(instances::RedundantDeclarationOverride),
        Box::new(variables::ArgumentPropertyShape),
    ];
    rules.extend(
        references::ReferenceLegality::CODES
            .iter()
            .map(|code| Box::new(references::ReferenceLegality::new(*code)) as Box<dyn Rule>),
    );
    rules.extend(
        instances::TypeDefinition::CODES
            .iter()
            .map(|code| Box::new(instances::TypeDefinition::new(*code)) as Box<dyn Rule>),
    );
    rules.extend(
        instances::PropertyShape::CODES
            .iter()
            .map(|code| Box::new(instances::PropertyShape::new(*code)) as Box<dyn Rule>),
    );
    rules.extend(
        instances::ModellingRuleShape::CODES
            .iter()
            .map(|code| Box::new(instances::ModellingRuleShape::new(*code)) as Box<dyn Rule>),
    );
    rules.extend(
        variables::DataTypeAttribute::CODES
            .iter()
            .map(|code| Box::new(variables::DataTypeAttribute::new(*code)) as Box<dyn Rule>),
    );
    rules.extend(
        variables::ValueRankConsistency::CODES
            .iter()
            .map(|code| Box::new(variables::ValueRankConsistency::new(*code)) as Box<dyn Rule>),
    );
    debug_assert_eq!(rules.len(), DiagnosticCode::ALL.len(), "every code needs exactly one rule");
    rules
}

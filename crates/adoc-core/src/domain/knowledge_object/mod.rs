//! Aggregate family — populated by Slice 1.

pub(crate) mod claim;

use claim::Claim;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KnowledgeObject {
    Claim(Claim),
}

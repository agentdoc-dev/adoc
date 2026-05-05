//! Aggregate family — populated by Slice 1.

pub(crate) mod claim;
pub(crate) mod decision;

use claim::Claim;
use decision::Decision;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockKind {
    Claim,
    Decision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KnowledgeObject {
    Claim(Claim),
    Decision(Decision),
}

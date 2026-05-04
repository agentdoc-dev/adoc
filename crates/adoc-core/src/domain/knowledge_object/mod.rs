//! Aggregate family — populated by Slice 1.
// Items here will be consumed by later commits.
#![allow(dead_code)]

pub(crate) mod claim;

pub(crate) use claim::Claim;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KnowledgeObject {
    Claim(Claim),
}

impl KnowledgeObject {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Claim(_) => "claim",
        }
    }
}

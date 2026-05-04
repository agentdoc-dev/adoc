//! Aggregate family — populated by Slice 1.

pub(crate) mod claim;

pub(crate) use claim::Claim;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KnowledgeObject {
    #[allow(dead_code)]
    Claim(Claim),
}

#[expect(dead_code, reason = "consumed by render and agent-JSON walkers")]
impl KnowledgeObject {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Claim(_) => "claim",
        }
    }
}

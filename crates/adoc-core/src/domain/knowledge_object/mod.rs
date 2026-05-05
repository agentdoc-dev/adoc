//! Aggregate family — populated by Slice 1.

pub(crate) mod claim;
pub(crate) mod decision;
pub(crate) mod warning;

use claim::Claim;
use decision::Decision;
use warning::Warning;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockKind {
    Claim,
    Decision,
    Warning,
}

impl BlockKind {
    #[cfg(test)]
    pub(crate) const ALL: &'static [Self] = &[Self::Claim, Self::Decision, Self::Warning];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Claim => "claim",
            Self::Decision => "decision",
            Self::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KnowledgeObject {
    Claim(Claim),
    Decision(Decision),
    Warning(Warning),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_kind_labels_match_source_fence_words() {
        assert_eq!(BlockKind::Claim.as_str(), "claim");
        assert_eq!(BlockKind::Decision.as_str(), "decision");
        assert_eq!(BlockKind::Warning.as_str(), "warning");
    }

    #[test]
    fn block_kind_all_lists_every_supported_kind() {
        assert_eq!(
            BlockKind::ALL,
            &[BlockKind::Claim, BlockKind::Decision, BlockKind::Warning]
        );
    }
}

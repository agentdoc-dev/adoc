//! Evidence kind value object used by the V5.7 `source` Knowledge Object.
//!
//! Constructed only via [`EvidenceKind::try_new`]. The accepted grammar is the
//! exact snake_case wire strings listed in PRD §15.1. ASCII-trimmed on input;
//! unknown strings produce [`EvidenceKindError::Invalid`] carrying the offending
//! value.

use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// The kind of evidence a `source` Knowledge Object represents.
///
/// Once constructed the value is total — every variant maps to exactly one
/// canonical snake_case string and back.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceKind {
    SourceCode,
    Test,
    Commit,
    PullRequest,
    Issue,
    DesignDoc,
    HumanReview,
    ExternalUrl,
    ApiSchema,
    RuntimeMetric,
    Incident,
    SupportTicket,
    AuditRecord,
    PolicyReference,
    Dataset,
    Experiment,
}

/// Why an evidence kind string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvidenceKindError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but not one of the canonical evidence kinds.
    Invalid(String),
}

/// Whether the `target` field of a `source` Knowledge Object must be a
/// repo-relative path, an absolute URL, or either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetRequirement {
    PathOnly,
    UrlOnly,
    Either,
}

impl EvidenceKind {
    /// Parse an evidence kind from a string slice. ASCII-trims, then matches
    /// the canonical snake_case set; empty input is
    /// [`EvidenceKindError::Missing`] and any other spelling is
    /// [`EvidenceKindError::Invalid`].
    pub(crate) fn try_new(raw: &str) -> Result<Self, EvidenceKindError> {
        let trimmed = trim_ascii_edges(raw);
        if trimmed.is_empty() {
            return Err(EvidenceKindError::Missing);
        }
        match trimmed {
            "source_code" => Ok(Self::SourceCode),
            "test" => Ok(Self::Test),
            "commit" => Ok(Self::Commit),
            "pull_request" => Ok(Self::PullRequest),
            "issue" => Ok(Self::Issue),
            "design_doc" => Ok(Self::DesignDoc),
            "human_review" => Ok(Self::HumanReview),
            "external_url" => Ok(Self::ExternalUrl),
            "api_schema" => Ok(Self::ApiSchema),
            "runtime_metric" => Ok(Self::RuntimeMetric),
            "incident" => Ok(Self::Incident),
            "support_ticket" => Ok(Self::SupportTicket),
            "audit_record" => Ok(Self::AuditRecord),
            "policy_reference" => Ok(Self::PolicyReference),
            "dataset" => Ok(Self::Dataset),
            "experiment" => Ok(Self::Experiment),
            _ => Err(EvidenceKindError::Invalid(trimmed.to_string())),
        }
    }

    /// The canonical snake_case rendering of this evidence kind.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SourceCode => "source_code",
            Self::Test => "test",
            Self::Commit => "commit",
            Self::PullRequest => "pull_request",
            Self::Issue => "issue",
            Self::DesignDoc => "design_doc",
            Self::HumanReview => "human_review",
            Self::ExternalUrl => "external_url",
            Self::ApiSchema => "api_schema",
            Self::RuntimeMetric => "runtime_metric",
            Self::Incident => "incident",
            Self::SupportTicket => "support_ticket",
            Self::AuditRecord => "audit_record",
            Self::PolicyReference => "policy_reference",
            Self::Dataset => "dataset",
            Self::Experiment => "experiment",
        }
    }

    /// Whether the `target` field for this evidence kind must be a path, a
    /// URL, or either.
    pub(crate) fn target(self) -> TargetRequirement {
        match self {
            Self::SourceCode | Self::Test => TargetRequirement::PathOnly,
            Self::PullRequest
            | Self::Issue
            | Self::ExternalUrl
            | Self::RuntimeMetric
            | Self::Incident
            | Self::SupportTicket
            | Self::Experiment => TargetRequirement::UrlOnly,
            Self::Commit
            | Self::DesignDoc
            | Self::HumanReview
            | Self::ApiSchema
            | Self::AuditRecord
            | Self::PolicyReference
            | Self::Dataset => TargetRequirement::Either,
        }
    }

    /// Returns `true` when a repo-relative path is acceptable for this kind's
    /// `target` field.
    pub(crate) fn allows_path(self) -> bool {
        matches!(
            self.target(),
            TargetRequirement::PathOnly | TargetRequirement::Either
        )
    }

    /// Returns `true` when an absolute URL is acceptable for this kind's
    /// `target` field.
    pub(crate) fn allows_url(self) -> bool {
        matches!(
            self.target(),
            TargetRequirement::UrlOnly | TargetRequirement::Either
        )
    }
}

impl fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for EvidenceKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("evidence kind is empty or whitespace-only"),
            Self::Invalid(value) => write!(f, "unknown evidence kind: {value:?}"),
        }
    }
}

impl std::error::Error for EvidenceKindError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_variants_through_as_str_and_try_new() {
        let variants = [
            EvidenceKind::SourceCode,
            EvidenceKind::Test,
            EvidenceKind::Commit,
            EvidenceKind::PullRequest,
            EvidenceKind::Issue,
            EvidenceKind::DesignDoc,
            EvidenceKind::HumanReview,
            EvidenceKind::ExternalUrl,
            EvidenceKind::ApiSchema,
            EvidenceKind::RuntimeMetric,
            EvidenceKind::Incident,
            EvidenceKind::SupportTicket,
            EvidenceKind::AuditRecord,
            EvidenceKind::PolicyReference,
            EvidenceKind::Dataset,
            EvidenceKind::Experiment,
        ];
        for variant in variants {
            let wire = variant.as_str();
            let parsed = EvidenceKind::try_new(wire).expect("round-trip must succeed");
            assert_eq!(
                parsed, variant,
                "as_str -> try_new round-trip failed for {wire:?}"
            );
        }
    }

    #[test]
    fn try_new_trims_ascii_whitespace() {
        let kind = EvidenceKind::try_new("  commit  ").expect("valid with padding");
        assert_eq!(kind, EvidenceKind::Commit);
    }

    #[test]
    fn try_new_rejects_empty_string() {
        assert_eq!(EvidenceKind::try_new(""), Err(EvidenceKindError::Missing));
    }

    #[test]
    fn try_new_rejects_whitespace_only_string() {
        assert_eq!(
            EvidenceKind::try_new(" \t "),
            Err(EvidenceKindError::Missing)
        );
    }

    #[test]
    fn try_new_rejects_unknown_value_carrying_the_offending_string() {
        assert_eq!(
            EvidenceKind::try_new("bogus"),
            Err(EvidenceKindError::Invalid("bogus".to_string()))
        );
    }

    #[test]
    fn target_path_only_variants() {
        assert_eq!(
            EvidenceKind::SourceCode.target(),
            TargetRequirement::PathOnly
        );
        assert_eq!(EvidenceKind::Test.target(), TargetRequirement::PathOnly);
    }

    #[test]
    fn target_url_only_variants() {
        for kind in [
            EvidenceKind::PullRequest,
            EvidenceKind::Issue,
            EvidenceKind::ExternalUrl,
            EvidenceKind::RuntimeMetric,
            EvidenceKind::Incident,
            EvidenceKind::SupportTicket,
            EvidenceKind::Experiment,
        ] {
            assert_eq!(
                kind.target(),
                TargetRequirement::UrlOnly,
                "{:?} should be UrlOnly",
                kind
            );
        }
    }

    #[test]
    fn target_either_variants() {
        for kind in [
            EvidenceKind::Commit,
            EvidenceKind::DesignDoc,
            EvidenceKind::HumanReview,
            EvidenceKind::ApiSchema,
            EvidenceKind::AuditRecord,
            EvidenceKind::PolicyReference,
            EvidenceKind::Dataset,
        ] {
            assert_eq!(
                kind.target(),
                TargetRequirement::Either,
                "{:?} should be Either",
                kind
            );
        }
    }

    #[test]
    fn allows_path_is_true_for_path_only_and_either() {
        assert!(EvidenceKind::SourceCode.allows_path());
        assert!(EvidenceKind::Test.allows_path());
        assert!(EvidenceKind::Commit.allows_path());
        assert!(!EvidenceKind::PullRequest.allows_path());
        assert!(!EvidenceKind::Issue.allows_path());
    }

    #[test]
    fn allows_url_is_true_for_url_only_and_either() {
        assert!(EvidenceKind::PullRequest.allows_url());
        assert!(EvidenceKind::Commit.allows_url());
        assert!(!EvidenceKind::SourceCode.allows_url());
        assert!(!EvidenceKind::Test.allows_url());
    }

    #[test]
    fn display_round_trips_through_try_new() {
        for kind in [EvidenceKind::Dataset, EvidenceKind::Experiment] {
            let rendered = kind.to_string();
            assert_eq!(EvidenceKind::try_new(&rendered), Ok(kind));
        }
    }
}

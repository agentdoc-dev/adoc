mod filter;
pub(crate) mod hybrid_ranker;
pub(crate) mod lexical_index;
pub(crate) mod metadata;
mod retrieval_record;
pub(crate) mod vector_index;

pub use filter::SearchFilters;

/// Explicit local retrieval authority. The audience is a visibility ceiling;
/// allowed visibilities narrow it, and explicit Object ID exclusions win.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievalPolicy {
    pub audience: String,
    pub allowed_visibilities: std::collections::BTreeSet<String>,
    #[serde(default)]
    pub excluded_object_ids: std::collections::BTreeSet<String>,
}

impl RetrievalPolicy {
    pub(crate) fn validate(&self) -> Result<(), Box<super::diagnostic::Diagnostic>> {
        use super::diagnostic::{Diagnostic, DiagnosticCode};
        if canonical_visibility(&self.audience).is_none() {
            return Err(Box::new(Diagnostic::error(
                DiagnosticCode::RetrievalAudienceUnresolved,
                "Retrieval audience must be public, internal, or restricted.",
            )));
        }
        if self
            .allowed_visibilities
            .iter()
            .any(|v| canonical_visibility(v).is_none())
            || self
                .excluded_object_ids
                .iter()
                .any(|id| super::identity::ObjectId::new(id.as_str()).is_err())
        {
            return Err(Box::new(Diagnostic::error(
                DiagnosticCode::RetrievalPolicyInvalid,
                "Retrieval policy requires canonical visibilities and valid excluded Object IDs.",
            )));
        }
        Ok(())
    }

    /// One permission predicate, evaluated before any retrieval index exists.
    pub(crate) fn permits(
        policy: Option<&Self>,
        object: &super::graph::GraphKnowledgeObjectNode,
    ) -> Result<bool, Box<super::diagnostic::Diagnostic>> {
        use super::diagnostic::{Diagnostic, DiagnosticCode};
        use super::value_objects::visibility::Visibility;
        let visibility = canonical_visibility(object.visibility.as_deref().unwrap_or("public"))
            .ok_or_else(|| {
                Diagnostic::error(
                    DiagnosticCode::RetrievalVisibilityUnavailable,
                    "Retrieval object visibility cannot be resolved.",
                )
            })?;
        let Some(policy) = policy else {
            return Ok(visibility == Visibility::Public);
        };
        Ok(!policy.excluded_object_ids.contains(&object.id)
            && policy.allowed_visibilities.contains(visibility.as_str())
            && canonical_visibility(&policy.audience)
                .is_some_and(|audience| visibility <= audience))
    }
}

fn canonical_visibility(value: &str) -> Option<super::value_objects::visibility::Visibility> {
    super::value_objects::visibility::Visibility::try_new(value)
        .ok()
        .filter(|visibility| visibility.as_str() == value)
}

pub use retrieval_record::{
    ProseRecord, RetrievalEntry, RetrievalMatch, RetrievalRecord, RetrievalRelations,
    RetrievalSource, SearchMode,
};

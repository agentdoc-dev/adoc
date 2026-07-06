use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::{GraphDirection, GraphKnowledgeObjectNode, GraphRelationKind};
use crate::domain::retrieval::metadata;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchFilters {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub source_path: Option<String>,
    pub related_to: Option<String>,
    pub relation: Option<GraphRelationKind>,
    pub direction: Option<GraphDirection>,
}

impl SearchFilters {
    /// V1.7.1 (ADR-0040) filter policy: any Knowledge Object metadata filter
    /// implies object intent and suppresses prose records — the single
    /// switch point if V1.7.3 pilot evidence argues for pass-through.
    pub(crate) fn constrains_objects(&self) -> bool {
        self.kind.is_some()
            || self.status.is_some()
            || self.owner.is_some()
            || self.source_path.is_some()
            || self.related_to.is_some()
    }

    /// Returns whether object metadata matches this filter set.
    ///
    /// Graph-scoped fields (`related_to`, `relation`, `direction`) are
    /// resolved by the retrieval application layer so lexical, semantic, and
    /// hybrid search can apply graph candidates at the right ranking phase.
    pub(crate) fn matches(&self, object: &GraphKnowledgeObjectNode) -> bool {
        matches_required(&object.kind, self.kind.as_deref())
            && matches_optional(object.status.as_deref(), self.status.as_deref())
            && matches_optional(metadata::owner(object), self.owner.as_deref())
            && matches_required(&object.source_span.path, self.source_path.as_deref())
    }

    pub(crate) fn validate_against<'a>(
        &self,
        objects: impl IntoIterator<Item = &'a GraphKnowledgeObjectNode>,
    ) -> Vec<Diagnostic> {
        self.filter_state_against(objects).diagnostics()
    }

    fn filter_state_against<'a>(
        &self,
        objects: impl IntoIterator<Item = &'a GraphKnowledgeObjectNode>,
    ) -> FilterValidationState<'_> {
        let mut state = FilterValidationState::new(self);

        for object in objects {
            if state.all_valid() {
                break;
            }
            state.update(object);
        }

        state
    }
}

struct FilterValidationState<'a> {
    filters: &'a SearchFilters,
    kind_valid: bool,
    status_valid: bool,
    owner_valid: bool,
    source_path_valid: bool,
}

impl<'a> FilterValidationState<'a> {
    fn new(filters: &'a SearchFilters) -> Self {
        Self {
            filters,
            kind_valid: filters.kind.is_none(),
            status_valid: filters.status.is_none(),
            owner_valid: filters.owner.is_none(),
            source_path_valid: filters.source_path.is_none(),
        }
    }

    fn update(&mut self, object: &GraphKnowledgeObjectNode) {
        if !self.kind_valid && matches_required(&object.kind, self.filters.kind.as_deref()) {
            self.kind_valid = true;
        }
        if !self.status_valid
            && matches_optional(object.status.as_deref(), self.filters.status.as_deref())
        {
            self.status_valid = true;
        }
        if !self.owner_valid
            && matches_optional(metadata::owner(object), self.filters.owner.as_deref())
        {
            self.owner_valid = true;
        }
        if !self.source_path_valid
            && matches_required(
                &object.source_span.path,
                self.filters.source_path.as_deref(),
            )
        {
            self.source_path_valid = true;
        }
    }

    fn all_valid(&self) -> bool {
        self.kind_valid && self.status_valid && self.owner_valid && self.source_path_valid
    }

    fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        push_invalid_filter(
            &mut diagnostics,
            "kind",
            self.filters.kind.as_deref(),
            self.kind_valid,
        );
        push_invalid_filter(
            &mut diagnostics,
            "status",
            self.filters.status.as_deref(),
            self.status_valid,
        );
        push_invalid_filter(
            &mut diagnostics,
            "owner",
            self.filters.owner.as_deref(),
            self.owner_valid,
        );
        push_invalid_filter(
            &mut diagnostics,
            "source_path",
            self.filters.source_path.as_deref(),
            self.source_path_valid,
        );
        diagnostics
    }
}

fn push_invalid_filter(
    diagnostics: &mut Vec<Diagnostic>,
    field: &str,
    value: Option<&str>,
    is_valid: bool,
) {
    if let (Some(value), false) = (value, is_valid) {
        diagnostics.push(Diagnostic::error(
            DiagnosticCode::SearchInvalidFilter,
            format!("Search filter `{field}={value}` did not match any object field."),
        ));
    }
}

fn matches_required(value: &str, filter: Option<&str>) -> bool {
    filter.is_none_or(|filter| contains_lowercase(value, filter))
}

fn matches_optional(value: Option<&str>, filter: Option<&str>) -> bool {
    filter.is_none_or(|filter| value.is_some_and(|value| contains_lowercase(value, filter)))
}

fn contains_lowercase(value: &str, filter: &str) -> bool {
    value.to_lowercase().contains(&filter.to_lowercase())
}

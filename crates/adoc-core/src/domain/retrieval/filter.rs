use crate::domain::artifact::AgentJsonObject;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};

const OWNER_FIELD: &str = "owner";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchFilters {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub source_path: Option<String>,
}

impl SearchFilters {
    pub fn matches(&self, object: &AgentJsonObject) -> bool {
        matches_required(&object.kind, self.kind.as_deref())
            && matches_optional(object.status.as_deref(), self.status.as_deref())
            && matches_optional(
                object.fields.get(OWNER_FIELD).map(String::as_str),
                self.owner.as_deref(),
            )
            && matches_required(&object.source_span.path, self.source_path.as_deref())
    }

    pub fn validate_against<'a>(
        &self,
        objects: impl IntoIterator<Item = &'a AgentJsonObject>,
    ) -> Vec<Diagnostic> {
        self.filter_state_against(objects).diagnostics()
    }

    pub(crate) fn validate_and_match<'a>(
        &self,
        objects: impl IntoIterator<Item = &'a AgentJsonObject>,
    ) -> Result<Vec<&'a AgentJsonObject>, Vec<Diagnostic>> {
        let mut candidates = Vec::new();
        let mut state = FilterValidationState::new(self);

        for object in objects {
            state.update(object);
            if self.matches(object) {
                candidates.push(object);
            }
        }

        let diagnostics = state.diagnostics();
        if diagnostics.is_empty() {
            Ok(candidates)
        } else {
            Err(diagnostics)
        }
    }

    fn filter_state_against<'a>(
        &self,
        objects: impl IntoIterator<Item = &'a AgentJsonObject>,
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

    fn update(&mut self, object: &AgentJsonObject) {
        if !self.kind_valid && matches_required(&object.kind, self.filters.kind.as_deref()) {
            self.kind_valid = true;
        }
        if !self.status_valid
            && matches_optional(object.status.as_deref(), self.filters.status.as_deref())
        {
            self.status_valid = true;
        }
        if !self.owner_valid
            && matches_optional(
                object.fields.get(OWNER_FIELD).map(String::as_str),
                self.filters.owner.as_deref(),
            )
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

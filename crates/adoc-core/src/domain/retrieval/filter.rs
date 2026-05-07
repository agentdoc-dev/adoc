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
        let mut kind_valid = self.kind.is_none();
        let mut status_valid = self.status.is_none();
        let mut owner_valid = self.owner.is_none();
        let mut source_path_valid = self.source_path.is_none();

        for object in objects {
            if !kind_valid && matches_required(&object.kind, self.kind.as_deref()) {
                kind_valid = true;
            }
            if !status_valid && matches_optional(object.status.as_deref(), self.status.as_deref()) {
                status_valid = true;
            }
            if !owner_valid
                && matches_optional(
                    object.fields.get(OWNER_FIELD).map(String::as_str),
                    self.owner.as_deref(),
                )
            {
                owner_valid = true;
            }
            if !source_path_valid
                && matches_required(&object.source_span.path, self.source_path.as_deref())
            {
                source_path_valid = true;
            }
        }

        let mut diagnostics = Vec::new();
        push_invalid_filter(&mut diagnostics, "kind", self.kind.as_deref(), kind_valid);
        push_invalid_filter(
            &mut diagnostics,
            "status",
            self.status.as_deref(),
            status_valid,
        );
        push_invalid_filter(
            &mut diagnostics,
            "owner",
            self.owner.as_deref(),
            owner_valid,
        );
        push_invalid_filter(
            &mut diagnostics,
            "source_path",
            self.source_path.as_deref(),
            source_path_valid,
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

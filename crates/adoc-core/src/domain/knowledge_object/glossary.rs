use std::collections::BTreeMap;

use crate::domain::ast::ParsedTypedBlock;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId, ObjectIdError};
use crate::domain::values::{BodyText, OptionalFields};

const GLOSSARY_MISSING_BODY_HELP: &str = "Glossary entries require non-empty body text.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Glossary {
    id: ObjectId,
    body: BodyText,
    fields: OptionalFields,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GlossaryError {
    InvalidId(ObjectIdError),
    MissingBody,
}

impl Glossary {
    pub(crate) fn build_from_parsed(
        parsed: &ParsedTypedBlock,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Self> {
        if super::reject_duplicate_fields(parsed, "glossary", diagnostics) {
            return None;
        }

        match Self::try_new(
            &parsed.id_text,
            &parsed.body_text,
            parsed.raw_fields.clone(),
            parsed.span.clone(),
        ) {
            Ok(glossary) => Some(glossary),
            Err(error) => {
                emit_glossary_error(parsed, error, diagnostics);
                None
            }
        }
    }

    pub(crate) fn try_new(
        id_text: &str,
        body_text: &str,
        fields: BTreeMap<String, String>,
        span: SourceSpan,
    ) -> Result<Self, GlossaryError> {
        let id = ObjectId::new(id_text).map_err(GlossaryError::InvalidId)?;
        let body = BodyText::try_new(body_text).ok_or(GlossaryError::MissingBody)?;
        Ok(Self {
            id,
            body,
            fields: OptionalFields::from_map(fields),
            span,
        })
    }

    pub(crate) fn id(&self) -> &ObjectId {
        &self.id
    }

    pub(crate) fn body(&self) -> &BodyText {
        &self.body
    }

    pub(crate) fn fields(&self) -> &OptionalFields {
        &self.fields
    }

    pub(crate) fn span(&self) -> &SourceSpan {
        &self.span
    }
}

fn emit_glossary_error(
    parsed: &ParsedTypedBlock,
    error: GlossaryError,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match error {
        GlossaryError::InvalidId(error) => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::IdInvalid,
                format!("invalid glossary id `{}`: {error}", parsed.id_text),
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(OBJECT_ID_GRAMMAR_HELP),
        ),
        GlossaryError::MissingBody => diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::SchemaMissingField,
                "glossary is missing required body",
            )
            .with_span(parsed.span.clone())
            .with_object_id(&parsed.id_text)
            .with_help(GLOSSARY_MISSING_BODY_HELP),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::knowledge_object::BlockKind;

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 12,
                offset: 11,
            },
        }
    }

    fn parsed_glossary(
        id_text: &str,
        fields: BTreeMap<String, String>,
        duplicate_keys: Vec<String>,
        body_text: &str,
    ) -> ParsedTypedBlock {
        ParsedTypedBlock {
            kind: BlockKind::Glossary,
            id_text: id_text.to_string(),
            raw_fields: fields,
            duplicate_keys,
            body_text: body_text.to_string(),
            content_spans: Vec::new(),
            span: span(),
        }
    }

    #[test]
    fn glossary_try_new_accepts_body_and_preserves_all_fields() {
        let glossary = Glossary::try_new(
            "billing.credits",
            "Credits adjust an account balance.",
            BTreeMap::from([
                ("status".to_string(), "draft".to_string()),
                ("owner".to_string(), "team-billing".to_string()),
            ]),
            span(),
        )
        .expect("valid glossary");

        assert_eq!(glossary.id().as_str(), "billing.credits");
        assert_eq!(
            glossary.body().as_str(),
            "Credits adjust an account balance."
        );
        let fields: Vec<(&str, &str)> = glossary
            .fields()
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        assert_eq!(fields, vec![("owner", "team-billing"), ("status", "draft")]);
    }

    #[test]
    fn glossary_try_new_requires_non_empty_body() {
        let error = Glossary::try_new("billing.credits", " ", BTreeMap::new(), span())
            .expect_err("empty body must fail");

        assert_eq!(error, GlossaryError::MissingBody);
    }

    #[test]
    fn glossary_try_new_rejects_invalid_id() {
        let error = Glossary::try_new(
            "BillingCredits",
            "Credits adjust an account balance.",
            BTreeMap::new(),
            span(),
        )
        .expect_err("invalid id must fail");

        assert!(matches!(error, GlossaryError::InvalidId(_)));
    }

    #[test]
    fn glossary_build_from_parsed_reports_invalid_id_and_drops_block() {
        let parsed = parsed_glossary(
            "BillingCredits",
            BTreeMap::new(),
            Vec::new(),
            "Credits adjust an account balance.",
        );
        let mut diagnostics = Vec::new();

        let glossary = Glossary::build_from_parsed(&parsed, &mut diagnostics);

        assert!(glossary.is_none(), "invalid id must drop the block");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IdInvalid);
        assert_eq!(diagnostics[0].span.as_ref(), Some(&span()));
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("BillingCredits"));
        assert!(
            diagnostics[0]
                .message
                .contains("invalid glossary id `BillingCredits`"),
            "diagnostic should name rejected glossary id: {:?}",
            diagnostics[0]
        );
        assert_eq!(diagnostics[0].help.as_deref(), Some(OBJECT_ID_GRAMMAR_HELP));
    }

    #[test]
    fn glossary_build_from_parsed_drops_duplicate_fields() {
        let parsed = parsed_glossary(
            "billing.credits",
            BTreeMap::from([("status".to_string(), "reviewed".to_string())]),
            vec!["status".to_string(), "status".to_string()],
            "Credits adjust an account balance.",
        );
        let mut diagnostics = Vec::new();

        let glossary = Glossary::build_from_parsed(&parsed, &mut diagnostics);

        assert!(glossary.is_none(), "duplicate fields must drop the block");
        assert_eq!(diagnostics.len(), 1, "duplicate key emitted once");
        assert_eq!(diagnostics[0].code, DiagnosticCode::SchemaDuplicateField);
        assert_eq!(diagnostics[0].span.as_ref(), Some(&span()));
        assert!(
            diagnostics[0]
                .message
                .contains("duplicate field `status` in glossary"),
            "diagnostic should name duplicate glossary field: {:?}",
            diagnostics[0]
        );
    }
}

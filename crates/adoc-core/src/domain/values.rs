use std::collections::BTreeMap;

use crate::domain::inline::{InlineSegment, plain_text, to_source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NonEmptyText(String);

impl NonEmptyText {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        let trimmed = trim_ascii_edges(value);
        (!trimmed.is_empty()).then(|| Self(trimmed.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Body(Vec<InlineSegment>);

impl Body {
    pub(crate) fn try_new(inlines: Vec<InlineSegment>) -> Option<Self> {
        let text = plain_text(&inlines);
        (!trim_ascii_edges(&text).is_empty()).then_some(Self(inlines))
    }

    #[cfg(test)]
    pub(crate) fn from_plain_text(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value)
            .map(|text| Self(vec![InlineSegment::Text(text.as_str().to_string())]))
    }

    pub(crate) fn inlines(&self) -> &[InlineSegment] {
        &self.0
    }

    pub(crate) fn inlines_mut(&mut self) -> &mut Vec<InlineSegment> {
        &mut self.0
    }

    pub(crate) fn to_source(&self) -> String {
        to_source(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct OptionalFields(BTreeMap<String, String>);

impl OptionalFields {
    pub(crate) fn from_map(fields: BTreeMap<String, String>) -> Self {
        Self(fields)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub(crate) fn trim_ascii_edges(value: &str) -> &str {
    value.trim_matches(|character: char| character.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_text_rejects_empty_or_ascii_whitespace() {
        assert!(NonEmptyText::try_new("").is_none());
        assert!(NonEmptyText::try_new(" \t ").is_none());
    }

    #[test]
    fn non_empty_text_trims_ascii_edges_only() {
        let value = NonEmptyText::try_new("  value  ").expect("non-empty");
        assert_eq!(value.as_str(), "value");

        let value = NonEmptyText::try_new("\u{00a0}value\u{00a0}").expect("non-empty");
        assert_eq!(value.as_str(), "\u{00a0}value\u{00a0}");
    }

    #[test]
    fn body_rejects_empty_plain_text_projection() {
        assert!(Body::try_new(vec![InlineSegment::Text(" \t ".to_string())]).is_none());
    }

    #[test]
    fn body_preserves_inline_source_projection() {
        let body = Body::try_new(vec![
            InlineSegment::Text("Use ".to_string()),
            InlineSegment::ObjectReferencePending {
                raw_id: "billing.credits".to_string(),
                span: crate::domain::diagnostic::SourceSpan {
                    file: std::path::PathBuf::from("guide.adoc"),
                    start: crate::domain::diagnostic::SourcePosition {
                        line: 1,
                        column: 5,
                        offset: 4,
                    },
                    end: crate::domain::diagnostic::SourcePosition {
                        line: 1,
                        column: 24,
                        offset: 23,
                    },
                },
            },
        ])
        .expect("non-empty body");

        assert_eq!(body.to_source(), "Use [[billing.credits]]");
    }

    #[test]
    fn optional_fields_iterates_in_sorted_key_order() {
        let mut map = BTreeMap::new();
        map.insert("z".to_string(), "last".to_string());
        map.insert("a".to_string(), "first".to_string());

        let fields = OptionalFields::from_map(map);
        let keys: Vec<&str> = fields.iter().map(|(key, _)| key.as_str()).collect();

        assert_eq!(keys, vec!["a", "z"]);
    }
}

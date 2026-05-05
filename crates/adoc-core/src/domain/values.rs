use std::collections::BTreeMap;

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
pub(crate) struct BodyText(NonEmptyText);

impl BodyText {
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(Self)
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
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
    fn body_text_reuses_non_empty_text_rules() {
        assert!(BodyText::try_new(" \t ").is_none());
        assert_eq!(
            BodyText::try_new("  body  ").expect("body").as_str(),
            "body"
        );
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

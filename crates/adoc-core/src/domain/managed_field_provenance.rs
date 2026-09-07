//! Immutable field-level links to native source assertions (E6.2.T1).
//! Validation establishes the manifest shape, never authority or source existence.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    retrieval::canonical_visibility, semantic_context::is_sha256_digest,
    value_objects::visibility::Visibility,
};

pub const MANAGED_FIELD_PROVENANCE_SCHEMA_VERSION: &str = "adoc.managed_field_provenance.v0";
const MAX_FIELDS: usize = 100;
const MAX_ASSERTIONS: usize = 100;
const MAX_SELECTOR_LENGTH: usize = 256;

#[derive(Debug, Clone)]
pub struct ManagedFieldProvenanceInput {
    pub workspace_id: String,
    pub canonical_id: String,
    pub version_id: String,
    pub content_digest: String,
    pub fields: Vec<ManagedFieldContributionInput>,
}

/// Mutable input is validated before becoming immutable provenance.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedFieldContributionInput {
    pub selector: String,
    pub assertion_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedFieldContribution {
    selector: String,
    assertion_ids: Vec<String>,
}

/// Construct or consume through the validating functions; fields cannot mutate.
///
/// ```compile_fail
/// use adoc_core::ManagedFieldProvenance;
/// fn mutate(record: &mut ManagedFieldProvenance) {
///     record.version_id = String::new();
/// }
/// ```
///
/// Raw deserialization cannot bypass validation.
///
/// ```compile_fail
/// use adoc_core::ManagedFieldProvenance;
/// let record: ManagedFieldProvenance = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedFieldProvenance {
    schema_version: String,
    workspace_id: String,
    canonical_id: String,
    version_id: String,
    content_digest: String,
    fields: Vec<ManagedFieldContribution>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManagedFieldProvenance {
    schema_version: String,
    workspace_id: String,
    canonical_id: String,
    version_id: String,
    content_digest: String,
    fields: Vec<ManagedFieldContributionInput>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManagedFieldProvenanceError {
    #[error("Managed field provenance has an invalid closed JSON shape.")]
    InvalidDocument,
    #[error("Managed field provenance has an unsupported schema version.")]
    UnsupportedVersion,
    #[error(
        "Managed field provenance requires canonical lowercase UUIDs and a lowercase SHA256 digest."
    )]
    InvalidCoordinates,
    #[error(
        "Managed field provenance requires 1–100 distinct valid selectors and 1–100 distinct native assertion UUIDs per selector."
    )]
    InvalidFields,
    #[error("Managed field provenance visibility must be public, internal, or restricted.")]
    InvalidVisibility,
    #[error("Managed field provenance serialization failed.")]
    Serialization,
}

impl ManagedFieldProvenance {
    pub fn to_canonical_json(&self) -> Result<String, ManagedFieldProvenanceError> {
        serde_json::to_string(self).map_err(|_| ManagedFieldProvenanceError::Serialization)
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }
    pub fn canonical_id(&self) -> &str {
        &self.canonical_id
    }
    pub fn version_id(&self) -> &str {
        &self.version_id
    }
    pub fn content_digest(&self) -> &str {
        &self.content_digest
    }
    pub fn fields(&self) -> &[ManagedFieldContribution] {
        &self.fields
    }
}

impl ManagedFieldContribution {
    pub fn selector(&self) -> &str {
        &self.selector
    }
    pub fn assertion_ids(&self) -> &[String] {
        &self.assertion_ids
    }
}

pub fn build_managed_field_provenance(
    mut input: ManagedFieldProvenanceInput,
) -> Result<ManagedFieldProvenance, ManagedFieldProvenanceError> {
    if [&input.workspace_id, &input.canonical_id, &input.version_id]
        .iter()
        .any(|id| !is_canonical_uuid(id))
        || !is_sha256_digest(&input.content_digest)
    {
        return Err(ManagedFieldProvenanceError::InvalidCoordinates);
    }
    if !(1..=MAX_FIELDS).contains(&input.fields.len()) {
        return Err(ManagedFieldProvenanceError::InvalidFields);
    }
    input
        .fields
        .sort_by(|left, right| left.selector.cmp(&right.selector));
    if input
        .fields
        .windows(2)
        .any(|pair| pair[0].selector == pair[1].selector)
    {
        return Err(ManagedFieldProvenanceError::InvalidFields);
    }
    let mut fields = Vec::with_capacity(input.fields.len());
    for mut field in input.fields {
        if !is_selector(&field.selector)
            || !(1..=MAX_ASSERTIONS).contains(&field.assertion_ids.len())
            || field.assertion_ids.iter().any(|id| !is_canonical_uuid(id))
        {
            return Err(ManagedFieldProvenanceError::InvalidFields);
        }
        field.assertion_ids.sort();
        if field
            .assertion_ids
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(ManagedFieldProvenanceError::InvalidFields);
        }
        fields.push(ManagedFieldContribution {
            selector: field.selector,
            assertion_ids: field.assertion_ids,
        });
    }
    Ok(ManagedFieldProvenance {
        schema_version: MANAGED_FIELD_PROVENANCE_SCHEMA_VERSION.into(),
        workspace_id: input.workspace_id,
        canonical_id: input.canonical_id,
        version_id: input.version_id,
        content_digest: input.content_digest,
        fields,
    })
}

pub fn validate_managed_field_provenance(
    bytes: &[u8],
) -> Result<ManagedFieldProvenance, ManagedFieldProvenanceError> {
    let shape: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| ManagedFieldProvenanceError::InvalidDocument)?;
    if !shape.is_object()
        || !shape["fields"]
            .as_array()
            .is_some_and(|fields| fields.iter().all(serde_json::Value::is_object))
    {
        return Err(ManagedFieldProvenanceError::InvalidDocument);
    }
    // Parse the original bytes so duplicate members still fail closed.
    let raw: RawManagedFieldProvenance =
        serde_json::from_slice(bytes).map_err(|_| ManagedFieldProvenanceError::InvalidDocument)?;
    if raw.schema_version != MANAGED_FIELD_PROVENANCE_SCHEMA_VERSION {
        return Err(ManagedFieldProvenanceError::UnsupportedVersion);
    }
    build_managed_field_provenance(ManagedFieldProvenanceInput {
        workspace_id: raw.workspace_id,
        canonical_id: raw.canonical_id,
        version_id: raw.version_id,
        content_digest: raw.content_digest,
        fields: raw.fields,
    })
}

/// Authored object/field floors default to public. Every contributing snapshot
/// must have a known class; missing evidence returns unresolved, never public.
/// Validate every supplied class even when another contributor is missing.
pub fn strictest_contributing_visibility(
    object: Option<&str>,
    field: Option<&str>,
    contributors: &[Option<&str>],
) -> Result<Option<String>, ManagedFieldProvenanceError> {
    let parse =
        |value| canonical_visibility(value).ok_or(ManagedFieldProvenanceError::InvalidVisibility);
    let mut strictest = Visibility::Public;
    for floor in [object, field].into_iter().flatten() {
        strictest = strictest.max(parse(floor)?);
    }
    let mut unresolved = contributors.is_empty();
    for contributor in contributors {
        match contributor {
            Some(value) => strictest = strictest.max(parse(value)?),
            None => unresolved = true,
        }
    }
    Ok((!unresolved).then(|| strictest.as_str().to_string()))
}

pub(crate) fn is_canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

pub(crate) fn is_selector(selector: &str) -> bool {
    if selector.chars().count() > MAX_SELECTOR_LENGTH {
        return false;
    }
    if selector == "/body" {
        return true;
    }
    let Some(token) = selector
        .strip_prefix("/fields/")
        .filter(|token| !token.is_empty())
    else {
        return false;
    };
    let mut chars = token.chars();
    while let Some(character) = chars.next() {
        match character {
            '/' => return false,
            '~' if !matches!(chars.next(), Some('0' | '1')) => return false,
            _ => {}
        }
    }
    true
}

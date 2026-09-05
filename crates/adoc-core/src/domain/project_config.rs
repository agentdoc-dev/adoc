use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::diagnostic::{Diagnostic, DiagnosticCode};
use super::retrieval::{RetrievalPolicy, canonical_visibility};
use super::source::LogicalPath;

#[derive(Debug, Clone)]
pub struct ParsedProjectConfig {
    pub docs_path: PathBuf,
    pub outputs: ParsedConfigOutputs,
    pub embeddings_provider: EmbeddingsProvider,
    pub mcp_patch_apply_enabled: bool,
    pub assessment_exclude_paths: Vec<String>,
    pub retrieval_policy: Option<RetrievalPolicy>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedConfigOutputs {
    pub dir: Option<PathBuf>,
    pub html: Option<PathBuf>,
    pub graph: Option<PathBuf>,
    pub search: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingsProvider {
    Local,
    Deterministic,
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectConfigDocumentError {
    #[error("{0}")]
    Parse(#[from] serde_saphyr::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("invalid retrieval policy: {}", .0.message)]
    RetrievalPolicy(Box<Diagnostic>),
    #[error(
        "assessment.exclude_paths entry {entry:?} must be an exact portable project-relative file or a directory prefix ending in `/`"
    )]
    InvalidAssessmentPath { entry: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProjectConfig {
    version: u32,
    mode: String,
    docs_path: PathBuf,
    outputs: Option<RawOutputs>,
    embeddings: Option<RawEmbeddings>,
    mcp: Option<RawMcp>,
    assessment: Option<RawAssessment>,
    retrieval_policy: Option<RetrievalPolicy>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawOutputs {
    dir: Option<PathBuf>,
    html: Option<PathBuf>,
    graph: Option<PathBuf>,
    search: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEmbeddings {
    provider: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMcp {
    patch_apply: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawAssessment {
    #[serde(default)]
    exclude_paths: Vec<String>,
}

pub fn parse_project_config(text: &str) -> Result<ParsedProjectConfig, ProjectConfigDocumentError> {
    // ponytail: preflight small configs for typed errors; replace the two parses
    // with presence-preserving deserialization if T3's benchmarks require it.
    let value: serde_json::Value = serde_saphyr::from_str(text)?;
    let validated_policy = if let Some(policy) = value.get("retrieval_policy") {
        if policy.is_object()
            && policy
                .get("audience")
                .and_then(serde_json::Value::as_str)
                .and_then(canonical_visibility)
                .is_none()
        {
            return Err(ProjectConfigDocumentError::RetrievalPolicy(Box::new(
                Diagnostic::error(
                    DiagnosticCode::RetrievalAudienceUnresolved,
                    "Retrieval audience must be public, internal, or restricted.",
                ),
            )));
        }
        let policy: RetrievalPolicy = serde_json::from_value(policy.clone()).map_err(|_| {
            ProjectConfigDocumentError::RetrievalPolicy(Box::new(Diagnostic::error(
                DiagnosticCode::RetrievalPolicyInvalid,
                "Retrieval policy must contain only the supported fields with their required types.",
            )))
        })?;
        policy
            .validate()
            .map_err(ProjectConfigDocumentError::RetrievalPolicy)?;
        Some(policy)
    } else {
        None
    };
    let mut raw: RawProjectConfig = serde_saphyr::from_str(text)?;
    raw.retrieval_policy = validated_policy;
    if raw.version != 1 {
        return Err(ProjectConfigDocumentError::Invalid(format!(
            "unsupported version {}; expected 1",
            raw.version
        )));
    }
    if raw.mode != "strict" {
        return Err(ProjectConfigDocumentError::Invalid(format!(
            "unsupported mode {:?}; expected \"strict\"",
            raw.mode
        )));
    }
    if !portable_docs_path(&raw.docs_path) {
        return Err(ProjectConfigDocumentError::Invalid(
            "docs_path must be a portable project-relative path".to_string(),
        ));
    }
    let embeddings_provider = match raw.embeddings.map(|value| value.provider) {
        Some(provider) if provider == "local" => EmbeddingsProvider::Local,
        Some(provider) if provider == "deterministic" => EmbeddingsProvider::Deterministic,
        Some(provider) if provider == "none" => EmbeddingsProvider::None,
        Some(provider) => {
            return Err(ProjectConfigDocumentError::Invalid(format!(
                "unsupported embeddings provider {provider:?}; expected \"local\", \"deterministic\", or \"none\""
            )));
        }
        None => EmbeddingsProvider::Local,
    };
    let mcp_patch_apply_enabled = match raw.mcp.map(|value| value.patch_apply) {
        Some(value) if value == "enabled" => true,
        Some(value) if value == "disabled" => false,
        Some(value) => {
            return Err(ProjectConfigDocumentError::Invalid(format!(
                "unsupported mcp.patch_apply {value:?}; expected \"enabled\" or \"disabled\""
            )));
        }
        None => false,
    };
    let outputs = raw.outputs.unwrap_or_default();
    let assessment_exclude_paths =
        normalize_assessment_exclusions(raw.assessment.unwrap_or_default().exclude_paths)?;
    Ok(ParsedProjectConfig {
        docs_path: raw.docs_path,
        outputs: ParsedConfigOutputs {
            dir: outputs.dir,
            html: outputs.html,
            graph: outputs.graph,
            search: outputs.search,
        },
        embeddings_provider,
        mcp_patch_apply_enabled,
        assessment_exclude_paths,
        retrieval_policy: raw.retrieval_policy,
    })
}

fn normalize_assessment_exclusions(
    entries: Vec<String>,
) -> Result<Vec<String>, ProjectConfigDocumentError> {
    let mut normalized = BTreeSet::new();
    for entry in entries {
        let invalid = entry.is_empty()
            || entry == "."
            || entry.trim() != entry
            || entry.contains('\\')
            || entry.chars().any(char::is_control)
            || entry.starts_with('/')
            || entry.as_bytes().get(1) == Some(&b':');
        let logical = entry.strip_suffix('/').unwrap_or(&entry);
        if invalid || logical.is_empty() || LogicalPath::parse(logical).is_err() {
            return Err(ProjectConfigDocumentError::InvalidAssessmentPath { entry });
        }
        normalized.insert(entry);
    }

    let mut result = Vec::new();
    for entry in normalized {
        let shadowed = result
            .iter()
            .any(|parent: &String| parent.ends_with('/') && entry.starts_with(parent.as_str()));
        if !shadowed {
            result.push(entry);
        }
    }
    Ok(result)
}

fn portable_docs_path(path: &Path) -> bool {
    path == Path::new(".")
        || path
            .to_str()
            .is_some_and(|value| LogicalPath::parse(value).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_null_retrieval_policy_is_invalid() {
        let error = parse_project_config(
            "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy: null\n",
        )
        .expect_err("explicit null policy must not be treated as absent");
        let ProjectConfigDocumentError::RetrievalPolicy(diagnostic) = error else {
            panic!("expected typed policy diagnostic: {error:?}");
        };
        assert_eq!(diagnostic.code, DiagnosticCode::RetrievalPolicyInvalid);
    }

    #[test]
    fn unresolved_retrieval_audience_has_its_own_diagnostic() {
        for audience in [
            None,
            Some("null"),
            Some("17"),
            Some("true"),
            Some("[]"),
            Some("{}"),
            Some("''"),
            Some("unknown"),
            Some("' public'"),
            Some("Public"),
        ] {
            let audience = audience
                .map(|value| format!("  audience: {value}\n"))
                .unwrap_or_default();
            let error = parse_project_config(&format!(
                "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy:\n{audience}  allowed_visibilities: [public]\n"
            )).expect_err("unresolved audience must fail");
            let ProjectConfigDocumentError::RetrievalPolicy(diagnostic) = error else {
                panic!("expected typed audience diagnostic: {error:?}");
            };
            assert_eq!(
                diagnostic.code,
                DiagnosticCode::RetrievalAudienceUnresolved,
                "{audience:?}"
            );
        }
    }

    #[test]
    fn unresolved_audience_precedes_other_policy_errors() {
        let error = parse_project_config(
            "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy: {audience: unknown, allowed_visibilities: wrong, unexpected: true}\n",
        ).expect_err("mixed invalid policy must fail");
        let ProjectConfigDocumentError::RetrievalPolicy(diagnostic) = error else {
            panic!("expected typed audience diagnostic: {error:?}");
        };
        assert_eq!(diagnostic.code, DiagnosticCode::RetrievalAudienceUnresolved);
    }

    #[test]
    fn malformed_retrieval_policy_shapes_and_values_have_safe_typed_errors() {
        for policy in [
            "null",
            "[]",
            "true",
            "17",
            "private-policy-sentinel",
            "{audience: public}",
            "{audience: public, allowed_visibilities: null}",
            "{audience: public, allowed_visibilities: public}",
            "{audience: public, allowed_visibilities: [17]}",
            "{audience: public, allowed_visibilities: [private-policy-sentinel]}",
            "{audience: public, allowed_visibilities: [' public']}",
            "{audience: public, allowed_visibilities: [public], excluded_object_ids: null}",
            "{audience: public, allowed_visibilities: [public], excluded_object_ids: billing.hidden}",
            "{audience: public, allowed_visibilities: [public], excluded_object_ids: [true]}",
            "{audience: public, allowed_visibilities: [public], excluded_object_ids: [private-policy-sentinel]}",
            "{audience: public, allowed_visibilities: [public], private-policy-sentinel: true}",
        ] {
            let error = parse_project_config(&format!(
                "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy: {policy}\n"
            ))
            .expect_err("malformed policy must fail");
            let ProjectConfigDocumentError::RetrievalPolicy(diagnostic) = error else {
                panic!("expected typed policy diagnostic for {policy}: {error:?}");
            };
            assert_eq!(
                diagnostic.code,
                DiagnosticCode::RetrievalPolicyInvalid,
                "{policy}"
            );
            assert_eq!(
                diagnostic.severity,
                super::super::diagnostic::Severity::Error
            );
            assert!(diagnostic.help.is_some());
            assert!(diagnostic.span.is_none() && diagnostic.object_id.is_none());
            assert!(
                !serde_json::to_string(&diagnostic)
                    .unwrap()
                    .contains("private-policy-sentinel")
            );
        }
    }

    #[test]
    fn absent_and_valid_retrieval_policies_preserve_config_defaults() {
        let base = "version: 1\nmode: strict\ndocs_path: docs\n";
        let absent = parse_project_config(base).unwrap();
        assert!(absent.retrieval_policy.is_none());
        assert_eq!(absent.embeddings_provider, EmbeddingsProvider::Local);
        for audience in ["public", "internal", "restricted"] {
            let parsed = parse_project_config(&format!(
                "{base}retrieval_policy: {{audience: {audience}, allowed_visibilities: [restricted, public, internal, public]}}\n"
            )).expect("canonical policy parses");
            let policy = parsed.retrieval_policy.unwrap();
            assert_eq!(policy.audience, audience);
            assert_eq!(
                policy.allowed_visibilities,
                BTreeSet::from(["public".into(), "internal".into(), "restricted".into()])
            );
            assert!(policy.excluded_object_ids.is_empty());
            assert_eq!(parsed.docs_path, absent.docs_path);
            assert_eq!(parsed.embeddings_provider, absent.embeddings_provider);
        }
        let parsed = parse_project_config(&format!(
            "{base}retrieval_policy: {{audience: public, allowed_visibilities: [], excluded_object_ids: [billing.hidden]}}\n"
        )).expect("empty allowlist is valid deny-all policy");
        let policy = parsed.retrieval_policy.unwrap();
        assert!(policy.allowed_visibilities.is_empty());
        assert_eq!(
            policy.excluded_object_ids,
            BTreeSet::from(["billing.hidden".into()])
        );
    }

    #[test]
    fn malformed_yaml_and_duplicate_keys_remain_parse_errors() {
        let base = "version: 1\nmode: strict\ndocs_path: docs\n";
        for config in [
            "version: [\n".to_string(),
            format!("{base}version: 1\n"),
            format!(
                "{base}retrieval_policy: null\nretrieval_policy: {{audience: public, allowed_visibilities: []}}\n"
            ),
            format!(
                "{base}retrieval_policy: {{audience: public, audience: restricted, allowed_visibilities: []}}\n"
            ),
        ] {
            assert!(
                matches!(
                    parse_project_config(&config),
                    Err(ProjectConfigDocumentError::Parse(_))
                ),
                "{config}"
            );
        }
    }

    #[test]
    fn parses_complete_shipped_configuration() {
        let parsed = parse_project_config(
            r#"
version: 1
mode: strict
docs_path: .
outputs:
  dir: build
  html: site.html
  graph: graph.json
  search: search.json
embeddings:
  provider: deterministic
mcp:
  patch_apply: enabled
"#,
        )
        .expect("valid config");

        assert_eq!(parsed.docs_path, PathBuf::from("."));
        assert_eq!(parsed.outputs.dir, Some(PathBuf::from("build")));
        assert_eq!(parsed.outputs.html, Some(PathBuf::from("site.html")));
        assert_eq!(parsed.outputs.graph, Some(PathBuf::from("graph.json")));
        assert_eq!(parsed.outputs.search, Some(PathBuf::from("search.json")));
        assert_eq!(
            parsed.embeddings_provider,
            EmbeddingsProvider::Deterministic
        );
        assert!(parsed.mcp_patch_apply_enabled);
    }

    #[test]
    fn rejects_unknown_fields() {
        let error = parse_project_config(
            r#"
version: 1
mode: strict
docs_path: docs
future_setting: enabled
"#,
        )
        .expect_err("unknown field must fail closed");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn normalizes_assessment_exclusions_and_removes_shadowed_children() {
        let parsed = parse_project_config(
            r#"
version: 1
mode: strict
docs_path: docs
assessment:
  exclude_paths:
    - vendor/pkg/
    - generated.txt
    - vendor/
    - generated.txt
"#,
        )
        .expect("assessment config parses");

        assert_eq!(
            parsed.assessment_exclude_paths,
            ["generated.txt", "vendor/"]
        );
    }

    #[test]
    fn rejects_unsafe_assessment_exclusions() {
        for path in ["", ".", "../src", "/tmp", "C:/tmp", "bad\\path", " src"] {
            let config = format!(
                "version: 1\nmode: strict\ndocs_path: docs\nassessment:\n  exclude_paths:\n    - '{path}'\n"
            );
            let error = parse_project_config(&config).expect_err("unsafe path must fail");
            assert!(error.to_string().contains("assessment.exclude_paths"));
            assert!(matches!(
                error,
                ProjectConfigDocumentError::InvalidAssessmentPath { entry } if entry == path
            ));
        }
    }
}

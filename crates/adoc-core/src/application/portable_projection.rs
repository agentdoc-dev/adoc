//! Pure, bounded projection of an already authorized retained native corpus.
//! This port verifies bindings; it never establishes authority to release bytes.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{GraphArtifactDocument, GraphKnowledgeObjectNode};
use crate::domain::hashing::sha256_prefixed;
use crate::domain::identity::ObjectId;
use crate::domain::lifecycle_mapping::{FlatProjection, LifecycleMappingContract};
use crate::domain::managed_state::{
    ConnectorId, EffectivityState, EventEmitter, FreshnessState, GovernanceState, IntegrityState,
    ManagedStateChange, ManagedVersionState, ReplayPosture, SynchronizationState,
    VerificationState,
};
use crate::domain::reconciliation::PolicyVersion;
use crate::domain::source::SourceFile;
use crate::domain::source_edit::planner::{CreateInsertion, plan_create_object};
use crate::domain::value_objects::evidence::Evidence;
use crate::infrastructure::artifact::graph_json::graph_knowledge_object_content_hash;
use crate::infrastructure::source::in_memory::InMemorySourceProvider;

use super::compile::compile_with_provider_for_date;

pub const MAX_PORTABLE_PROJECTION_BYTES: usize = 64 * 1024 * 1024;
const MAX_OBJECTS: usize = 4096;
const INPUT_SCHEMA: &str = "adoc.portable_projection_input.v0";
const OUTPUT_SCHEMA: &str = "adoc.portable_projection.v0";
const NOTICE: &str = "Portable projection. Native records and the accompanying loss report preserve the authoritative managed history.\n\n";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    schema_version: String,
    workspace_id: String,
    projection_version: String,
    evaluation_date: String,
    objects: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Canonical {
    workspace_id: String,
    canonical_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectInput {
    canonical: Canonical,
    version_id: String,
    object_id: String,
    original_content_digest: String,
    node_bytes: String,
    node_digest: String,
    state_history_complete: bool,
    state_events: Vec<RetainedRow>,
    preservation: Preservation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preservation {
    content_record_key: String,
    provenance_record_keys: Vec<String>,
    replay_postures: Vec<ReplayReference>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedRow {
    ordinal: String,
    prev_digest: String,
    record_digest: String,
    event_bytes: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayReference {
    record_key: String,
    posture: ReplayPosture,
}

// Retained wire data is deliberately separate from the authority-bound
// StateEventSubject and ManagedStateEvent. Neither acquires Deserialize.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedSubject {
    canonical: Canonical,
    version_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedEvent {
    subject: RetainedSubject,
    change: RetainedChange,
    emitter: String,
    policy_version: String,
    #[serde(deserialize_with = "Option::deserialize")]
    corrects: Option<u64>,
}

#[derive(Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
enum RetainedChange {
    Governance {
        state: GovernanceState,
    },
    Verification {
        state: VerificationState,
    },
    Effectivity {
        state: EffectivityState,
    },
    Freshness {
        state: FreshnessState,
    },
    Integrity {
        state: IntegrityState,
    },
    Synchronization {
        connector: String,
        state: SynchronizationState,
        required_before_effective: bool,
    },
    AuthorizationAffectingSourceChange {
        connector: String,
    },
    Declassification,
    Migration,
    DeletionTombstone {
        replay_posture: ReplayPosture,
    },
}

impl RetainedChange {
    fn validated(self) -> Result<ManagedStateChange, DiagnosticCode> {
        let connector = |value| {
            ConnectorId::new(value).map_err(|_| DiagnosticCode::PortableProjectionUnavailable)
        };
        Ok(match self {
            Self::Governance { state } => ManagedStateChange::Governance { state },
            Self::Verification { state } => ManagedStateChange::Verification { state },
            Self::Effectivity { state } => ManagedStateChange::Effectivity { state },
            Self::Freshness { state } => ManagedStateChange::Freshness { state },
            Self::Integrity { state } => ManagedStateChange::Integrity { state },
            Self::Synchronization {
                connector: id,
                state,
                required_before_effective,
            } => ManagedStateChange::Synchronization {
                connector: connector(id)?,
                state,
                required_before_effective,
            },
            Self::AuthorizationAffectingSourceChange { connector: id } => {
                ManagedStateChange::AuthorizationAffectingSourceChange {
                    connector: connector(id)?,
                }
            }
            Self::Declassification => ManagedStateChange::Declassification,
            Self::Migration => ManagedStateChange::Migration,
            Self::DeletionTombstone { replay_posture } => {
                ManagedStateChange::DeletionTombstone { replay_posture }
            }
        })
    }
}

#[derive(Serialize)]
struct Loss {
    category: &'static str,
    detail: String,
}

struct Candidate {
    input: ObjectInput,
    node: GraphKnowledgeObjectNode,
    lifecycle: FlatProjection,
    losses: Vec<Loss>,
    source: String,
}

impl Candidate {
    fn path(&self) -> String {
        format!("projections/{}.adoc", self.input.canonical.canonical_id)
    }

    fn loss(&mut self, category: &'static str, detail: impl Into<String>) {
        let detail = detail.into();
        if !self
            .losses
            .iter()
            .any(|loss| loss.category == category && loss.detail == detail)
        {
            self.losses.push(Loss { category, detail });
        }
    }
}

fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn nonblank(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

fn replay(input: &ObjectInput) -> Result<ManagedVersionState, DiagnosticCode> {
    let unavailable = DiagnosticCode::PortableProjectionUnavailable;
    if !input.state_history_complete {
        return Err(unavailable);
    }
    let mut changes: Vec<(u64, ManagedStateChange)> = Vec::new();
    let mut previous: Option<(u64, &str)> = None;
    for row in &input.state_events {
        let ordinal: u64 = row.ordinal.parse().map_err(|_| unavailable)?;
        if ordinal.to_string() != row.ordinal
            || previous.is_some_and(|(prior, _)| prior >= ordinal)
            || (ordinal == 0 && !row.prev_digest.is_empty())
            || (ordinal != 0 && !digest(&row.prev_digest))
        {
            return Err(unavailable);
        }
        if previous.is_some_and(|(prior, hash)| {
            prior.checked_add(1) == Some(ordinal) && hash != row.prev_digest
        }) || sha256_prefixed(format!("{}{}", row.prev_digest, row.event_bytes).as_bytes())
            != row.record_digest
        {
            return Err(DiagnosticCode::PrivacyExportDigestMismatch);
        }
        let event: RetainedEvent =
            serde_json::from_str(&row.event_bytes).map_err(|_| unavailable)?;
        if event.subject.canonical != input.canonical
            || event.subject.version_id != input.version_id
            || EventEmitter::new(event.emitter).is_err()
            || PolicyVersion::new(event.policy_version).is_err()
        {
            return Err(unavailable);
        }
        let change = event.change.validated()?;
        if let Some(corrects) = event.corrects
            && changes
                .iter()
                .rev()
                .find(|(_, prior)| prior.same_state_slot(&change))
                .map(|(index, _)| *index)
                != Some(corrects)
        {
            return Err(unavailable);
        }
        changes.push((ordinal, change));
        previous = Some((ordinal, &row.record_digest));
    }
    Ok(ManagedVersionState::from_changes(
        changes.iter().map(|(_, change)| change),
    ))
}

fn candidate(
    value: Value,
    workspace: &str,
    mapping: &LifecycleMappingContract,
) -> Result<Candidate, DiagnosticCode> {
    let unavailable = DiagnosticCode::PortableProjectionUnavailable;
    let input: ObjectInput = serde_json::from_value(value).map_err(|_| unavailable)?;
    if input.canonical.workspace_id != workspace
        || !uuid(&input.canonical.canonical_id)
        || !uuid(&input.version_id)
        || !nonblank(&input.preservation.content_record_key)
        || input
            .preservation
            .provenance_record_keys
            .iter()
            .any(|key| !nonblank(key))
        || input
            .preservation
            .replay_postures
            .iter()
            .any(|item| !nonblank(&item.record_key))
    {
        return Err(unavailable);
    }
    let exact_digest = sha256_prefixed(input.node_bytes.as_bytes());
    if input.node_digest != exact_digest || input.original_content_digest != exact_digest {
        return Err(DiagnosticCode::PrivacyExportDigestMismatch);
    }
    let node: GraphKnowledgeObjectNode =
        serde_json::from_str(&input.node_bytes).map_err(|_| unavailable)?;
    if node.content_hash != graph_knowledge_object_content_hash(&node) {
        return Err(DiagnosticCode::PrivacyExportDigestMismatch);
    }
    ObjectId::new(node.id.clone()).map_err(|_| unavailable)?;
    if node.fields.contains_key("status") {
        return Err(unavailable);
    }
    if input.object_id != node.id {
        return Err(unavailable);
    }
    let lifecycle = mapping
        .project_export(&node.kind, &replay(&input)?)
        .map_err(|error| error.diagnostic_code())?;
    let raw: Value = serde_json::from_str(&input.node_bytes).map_err(|_| unavailable)?;
    let known = serde_json::to_value(&node).map_err(|_| unavailable)?;
    let mut candidate = Candidate {
        input,
        node,
        lifecycle,
        losses: Vec::new(),
        source: String::new(),
    };
    for key in raw.as_object().ok_or(unavailable)?.keys() {
        if key == "type" && raw[key] == "knowledge_object" {
            continue;
        }
        // Omitted nullable/empty graph carriers are supported even when not
        // emitted by the graph serializer in this particular node.
        if !known
            .as_object()
            .is_some_and(|object| object.contains_key(key))
            && ![
                "status",
                "severity",
                "trust",
                "source_binding",
                "visibility",
                "field_visibility",
                "impacts",
                "approved_by",
                "allowed_actions",
                "forbidden_actions",
                "contradiction_claims",
                "evidence",
                "effective_status",
                "effective_reason",
                "evidence_quality",
            ]
            .contains(&key.as_str())
        {
            candidate.loss(
                "unsupported_carrier",
                format!("Unrepresentable graph carrier: {key}"),
            );
        }
    }
    candidate.loss("identity", "Canonical identity and immutable version bindings are in the manifest, not native source identity.");
    candidate.loss("placement", "Original page placement, source binding and surrounding prose are retained only in native records.");
    candidate.loss("history", "The block carries one lifecycle projection; the retained events preserve the independent dimensions and original workspace ordinals. Scoped ordinal gaps do not attest a complete workspace chain.");
    candidate.loss("provenance", "Source assertions and field provenance remain native records; the block cannot encode multi-source provenance.");
    if candidate.input.preservation.provenance_record_keys.len() > 1 {
        candidate.loss(
            "multi_source",
            "Multiple provenance records are preserved separately from this single source block.",
        );
    }
    candidate.loss("synchronization_configuration", "Connector synchronization configuration and managed effectivity requirements are not source configuration.");
    candidate.loss("derived", "Derived lifecycle and evidence-quality observations are recomputed by the strict compiler at the export evaluation date.");
    candidate.loss("replay", "Replay, deletion and retention obligations remain in native records; this source file does not restore deleted or unavailable evidence.");
    let postures: Vec<_> = candidate
        .input
        .preservation
        .replay_postures
        .iter()
        .map(|item| item.posture.as_str())
        .collect();
    if postures
        .iter()
        .any(|posture| *posture != "fully_replayable")
    {
        candidate.loss("replay", format!("Retained replay posture: {}. Export does not restore deleted or unavailable evidence.", postures.join(", ")));
    }
    Ok(candidate)
}

fn insert(
    fields: &mut BTreeMap<String, String>,
    key: &str,
    value: String,
) -> Result<(), DiagnosticCode> {
    if fields.get(key).is_some_and(|existing| existing != &value) {
        return Err(DiagnosticCode::PortableProjectionUnavailable);
    }
    fields.insert(key.into(), value);
    Ok(())
}

fn list(
    fields: &mut BTreeMap<String, String>,
    key: &str,
    values: &[String],
) -> Result<(), DiagnosticCode> {
    if !values.is_empty() {
        if values
            .iter()
            .any(|value| value.contains([',', '[', ']', '\n', '\r']))
        {
            return Err(DiagnosticCode::PortableProjectionUnavailable);
        }
        insert(fields, key, format!("[{}]", values.join(", ")))?;
    }
    Ok(())
}

fn render(candidate: &mut Candidate, ids: &BTreeSet<String>) -> Result<(), DiagnosticCode> {
    let unavailable = DiagnosticCode::PortableProjectionUnavailable;
    let mut fields = candidate.node.fields.clone();
    // Status comes solely from the existing projection policy.
    fields.remove("status");
    for (name, carrier) in [
        ("severity", &candidate.node.severity),
        ("trust", &candidate.node.trust),
        ("visibility", &candidate.node.visibility),
    ] {
        if let Some(value) = carrier {
            insert(&mut fields, name, value.clone())?;
        }
    }
    if let Some(visibility) = &candidate.node.field_visibility {
        insert(
            &mut fields,
            "field_visibility",
            visibility
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(", "),
        )?;
    }
    for (name, values) in [
        ("depends_on", candidate.node.relations.depends_on.clone()),
        ("supersedes", candidate.node.relations.supersedes.clone()),
        ("related_to", candidate.node.relations.related_to.clone()),
    ] {
        let retained: Vec<_> = values
            .iter()
            .filter(|id| ids.contains(*id))
            .cloned()
            .collect();
        if retained.len() != values.len() {
            candidate.loss(
                "withheld_reference",
                format!(
                    "{name}: references outside the surviving authorized projection are omitted."
                ),
            );
        }
        fields.remove(name);
        list(&mut fields, name, &retained)?;
    }
    for (name, values) in [
        ("impacts", &candidate.node.impacts),
        ("approved_by", &candidate.node.approved_by),
        ("allowed_actions", &candidate.node.allowed_actions),
        ("forbidden_actions", &candidate.node.forbidden_actions),
        ("claims", &candidate.node.contradiction_claims),
    ] {
        list(&mut fields, name, values)?;
    }
    if candidate
        .node
        .contradiction_claims
        .iter()
        .any(|id| !ids.contains(id))
    {
        return Err(unavailable);
    }
    let mut evidence_refs = Vec::new();
    for evidence in &candidate.node.evidence {
        match (&evidence.value, &evidence.reference) {
            (None, Some(reference)) if ids.contains(reference) => {
                evidence_refs.push(reference.clone())
            }
            (Some(value), None) => {
                let field = ["source", "test", "reviewed_by", "external_url"]
                    .into_iter()
                    .find(|field| {
                        Evidence::from_field(field, value)
                            .and_then(|value| value.kind())
                            .is_some_and(|kind| kind.as_str() == evidence.kind)
                    });
                if let Some(field) = field {
                    insert(&mut fields, field, value.clone())?;
                } else {
                    return Err(unavailable);
                }
            }
            _ => return Err(unavailable),
        }
    }
    list(&mut fields, "evidence_ref", &evidence_refs)?;
    let plan = plan_create_object(
        "",
        CreateInsertion::EndOfFile,
        &candidate.node.kind,
        &candidate.node.id,
        candidate.lifecycle.status.as_deref(),
        &fields,
        &candidate.node.body,
    )
    .map_err(|_| unavailable)?;
    let mut page_id = format!("portable.{}", candidate.input.canonical.canonical_id);
    while ids.contains(&page_id) {
        page_id.push_str(".page");
    }
    candidate.source = format!(
        "# Portable projection @doc({page_id})\n\n{NOTICE}{}",
        plan.splice("").map_err(|_| unavailable)?
    );
    Ok(())
}

fn diagnostic(code: DiagnosticCode, canonical_id: Option<&str>, message: &str) -> Value {
    json!({"code":code, "canonical_id":canonical_id, "message":message})
}

/// Project exact retained node/event bytes without filesystem or clock access.
/// Malformed top-level transport is an error. Invalid selected objects become
/// bounded diagnostics while valid siblings remain available.
pub fn run_portable_projection(bytes: &[u8]) -> Result<Value, Box<Diagnostic>> {
    let error = |code| {
        Box::new(Diagnostic::error(
            code,
            "Portable projection input is invalid or exceeds its bound.",
        ))
    };
    if bytes.len() > MAX_PORTABLE_PROJECTION_BYTES {
        return Err(error(DiagnosticCode::PortableProjectionUnavailable));
    }
    let input: Input = serde_json::from_slice(bytes)
        .map_err(|_| error(DiagnosticCode::PortableProjectionUnavailable))?;
    if input.schema_version != INPUT_SCHEMA {
        return Err(error(DiagnosticCode::SchemaUnsupportedVersion));
    }
    let mapping = LifecycleMappingContract::for_projection_version(&input.projection_version)
        .map_err(|failure| error(failure.diagnostic_code()))?;
    let today = NaiveDate::parse_from_str(&input.evaluation_date, "%Y-%m-%d")
        .map_err(|_| error(DiagnosticCode::PortableProjectionUnavailable))?;
    if today.format("%Y-%m-%d").to_string() != input.evaluation_date
        || !uuid(&input.workspace_id)
        || input.objects.len() > MAX_OBJECTS
    {
        return Err(error(DiagnosticCode::PortableProjectionUnavailable));
    }
    let total = input.objects.len();
    let mut diagnostics = Vec::new();
    let mut candidates = Vec::new();
    for value in input.objects {
        let canonical = value
            .pointer("/canonical/canonical_id")
            .and_then(Value::as_str)
            .filter(|id| uuid(id))
            .map(str::to_owned);
        match candidate(value, &input.workspace_id, &mapping) {
            Ok(value) => candidates.push(value),
            Err(code) => diagnostics.push(diagnostic(
                code,
                canonical.as_deref(),
                "Object omitted: retained bindings, history, or source carriers are unavailable.",
            )),
        }
    }
    let mut id_counts = BTreeMap::new();
    let mut canonical_counts = BTreeMap::new();
    let mut version_counts = BTreeMap::new();
    for candidate in &candidates {
        *id_counts.entry(candidate.node.id.clone()).or_insert(0) += 1;
        *canonical_counts
            .entry(candidate.input.canonical.canonical_id.clone())
            .or_insert(0) += 1;
        *version_counts
            .entry(candidate.input.version_id.clone())
            .or_insert(0) += 1;
    }
    candidates.retain(|candidate| {
        let unique = id_counts[&candidate.node.id] == 1
            && canonical_counts[&candidate.input.canonical.canonical_id] == 1
            && version_counts[&candidate.input.version_id] == 1;
        if !unique {
            diagnostics.push(diagnostic(
                DiagnosticCode::IdDuplicate,
                Some(&candidate.input.canonical.canonical_id),
                "Object omitted: projection identities must be unique.",
            ));
        }
        unique
    });
    // ponytail: whole-group recompilation is quadratic in the worst case;
    // input is capped at 4096 objects, use dependency indexing if this ceiling hurts.
    for _ in 0..=total {
        let ids = candidates
            .iter()
            .map(|candidate| candidate.node.id.clone())
            .collect();
        let mut dropped = false;
        candidates.retain_mut(|candidate| {
            match render(candidate, &ids) {
                Ok(()) => true,
                Err(code) => {
                    diagnostics.push(diagnostic(code, Some(&candidate.input.canonical.canonical_id), "Object omitted: required references or source carriers cannot be represented safely."));
                    dropped = true;
                    false
                }
            }
        });
        if dropped {
            continue;
        }
        let mut provider = InMemorySourceProvider::new();
        for candidate in &candidates {
            let path = candidate.path().into();
            provider = provider.with_source(SourceFile::new_with_identity_path(
                path,
                candidate.source.clone(),
                candidate.path().into(),
            ));
        }
        let compiled = compile_with_provider_for_date(&provider, today);
        if !compiled.has_errors() {
            // Surface authored carrier reductions made by the existing compiler,
            // including evidence stored in fields whose projected lifecycle no longer exposes it.
            if let Some(artifact) = compiled.artifacts {
                let graph: GraphArtifactDocument = serde_json::from_str(&artifact.graph_json)
                    .map_err(|_| error(DiagnosticCode::PortableProjectionUnavailable))?;
                for candidate in &mut candidates {
                    if let Some(node) = graph
                        .nodes
                        .iter()
                        .filter_map(|node| node.as_knowledge_object())
                        .find(|node| node.id == candidate.node.id)
                    {
                        if node.evidence != candidate.node.evidence {
                            candidate.loss("evidence", "The projected lifecycle exposes a reduced typed evidence carrier; exact native evidence remains preserved.");
                        }
                        let original = serde_json::to_value(&candidate.node)
                            .map_err(|_| error(DiagnosticCode::PortableProjectionUnavailable))?;
                        let projected = serde_json::to_value(node)
                            .map_err(|_| error(DiagnosticCode::PortableProjectionUnavailable))?;
                        for carrier in [
                            "body",
                            "fields",
                            "severity",
                            "trust",
                            "visibility",
                            "field_visibility",
                            "impacts",
                            "approved_by",
                            "allowed_actions",
                            "forbidden_actions",
                            "contradiction_claims",
                        ] {
                            if original.get(carrier) != projected.get(carrier) {
                                candidate.loss("authored_carrier", format!("Strict source compilation changes the {carrier} carrier; exact native values remain preserved."));
                            }
                        }
                    }
                }
            }
            break;
        }
        let errors: Vec<_> = compiled
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .collect();
        let before = candidates.len();
        candidates.retain(|candidate| {
            let invalid = errors.iter().any(|error| {
                error.object_id.as_ref() == Some(&candidate.node.id)
                    || error
                        .span
                        .as_ref()
                        .is_some_and(|span| span.file.to_string_lossy() == candidate.path())
            });
            if invalid {
                diagnostics.push(diagnostic(
                    DiagnosticCode::PortableProjectionUnavailable,
                    Some(&candidate.input.canonical.canonical_id),
                    "Object omitted: strict workspace compilation rejected its projection.",
                ));
            }
            !invalid
        });
        if candidates.len() == before {
            for candidate in candidates.drain(..) {
                diagnostics.push(diagnostic(DiagnosticCode::PortableProjectionUnavailable, Some(&candidate.input.canonical.canonical_id), "Object omitted: strict workspace validation could not attribute a group failure."));
            }
            break;
        }
    }
    let documents: Vec<_> = candidates.iter().map(|candidate| json!({
        "path":candidate.path(), "adoc_source":candidate.source,
        "object_bindings":[{"canonical_id":candidate.input.canonical.canonical_id,"version_id":candidate.input.version_id,"object_id":candidate.node.id,"original_content_digest":candidate.input.original_content_digest}]
    })).collect();
    let objects: Vec<_> = candidates.iter().map(|candidate| json!({
        "canonical_id":candidate.input.canonical.canonical_id,"version_id":candidate.input.version_id,"original_content_digest":candidate.input.original_content_digest,
        "lifecycle_projection":candidate.lifecycle,"loss_report":candidate.losses
    })).collect();
    let outcome = if candidates.len() == total {
        "complete"
    } else if candidates.is_empty() {
        "failed"
    } else {
        "partial"
    };
    let output = json!({"schema_version":OUTPUT_SCHEMA,"workspace_id":input.workspace_id,"mapping_version":"1","projection_version":"1","outcome":outcome,"documents":documents,"objects":objects,"diagnostics":diagnostics});
    if serde_json::to_vec(&output)
        .map_err(|_| error(DiagnosticCode::PortableProjectionUnavailable))?
        .len()
        > MAX_PORTABLE_PROJECTION_BYTES
    {
        return Err(error(DiagnosticCode::PortableProjectionUnavailable));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSPACE: &str = "11111111-1111-1111-1111-111111111111";
    const SOURCE: &str = "::claim billing.first\nstatus: draft\nrelated_to: [billing.second]\n--\nFirst retained claim.\n::\n\n::claim billing.second\nstatus: draft\n--\nSecond retained claim.\n::\n";

    fn input(source: &str) -> Value {
        let provider =
            InMemorySourceProvider::new().with_source(SourceFile::new_with_identity_path(
                "guide.adoc".into(),
                format!("# Retained @doc(export.fixture)\n\n{source}"),
                "guide.adoc".into(),
            ));
        let result =
            compile_with_provider_for_date(&provider, NaiveDate::from_ymd_opt(2026, 9, 8).unwrap());
        assert!(!result.has_errors(), "{:?}", result.diagnostics);
        let graph: GraphArtifactDocument =
            serde_json::from_str(&result.artifacts.unwrap().graph_json).unwrap();
        let objects: Vec<_> = graph.nodes.iter().filter_map(|node| node.as_knowledge_object()).enumerate().map(|(index, node)| {
            let bytes = serde_json::to_string(node).unwrap();
            json!({"canonical":{"workspace_id":WORKSPACE,"canonical_id":format!("22222222-2222-2222-2222-{index:012}")},"version_id":format!("33333333-3333-3333-3333-{index:012}"),"object_id":node.id,
                "original_content_digest":sha256_prefixed(bytes.as_bytes()),"node_bytes":bytes,"node_digest":sha256_prefixed(bytes.as_bytes()),"state_history_complete":true,"state_events":[],
                "preservation":{"content_record_key":format!("managed_version:{index}:content"),"provenance_record_keys":["assertion:one:record","assertion:two:record"],"replay_postures":[{"record_key":"posture:one:record","posture":"no_longer_replayable_after_deletion"}]}})
        }).collect();
        json!({"schema_version":INPUT_SCHEMA,"workspace_id":WORKSPACE,"projection_version":"1","evaluation_date":"2026-09-08","objects":objects})
    }

    fn project(input: &Value) -> Value {
        run_portable_projection(&serde_json::to_vec(input).unwrap()).unwrap()
    }

    fn event(object: &mut Value, ordinal: u64, change: Value) {
        let rows = object["state_events"].as_array().unwrap();
        let previous = rows
            .last()
            .map(|row| row["record_digest"].as_str().unwrap())
            .unwrap_or("")
            .to_string();
        let previous = if ordinal > 0 && previous.is_empty() {
            sha256_prefixed(b"other authorized workspace event")
        } else {
            previous
        };
        let bytes = json!({"subject":{"canonical":object["canonical"],"version_id":object["version_id"]},"change":change,"emitter":"cloud.governance","policy_version":"policy-1","corrects":null}).to_string();
        object["state_events"].as_array_mut().unwrap().push(json!({"ordinal":ordinal.to_string(),"prev_digest":previous,"record_digest":sha256_prefixed(format!("{previous}{bytes}").as_bytes()),"event_bytes":bytes}));
    }

    fn change_node(object: &mut Value, change: impl FnOnce(&mut Value)) {
        let mut node: Value = serde_json::from_str(object["node_bytes"].as_str().unwrap()).unwrap();
        change(&mut node);
        let typed: GraphKnowledgeObjectNode = serde_json::from_value(node.clone()).unwrap();
        node["content_hash"] = json!(graph_knowledge_object_content_hash(&typed));
        let bytes = node.to_string();
        object["node_digest"] = json!(sha256_prefixed(bytes.as_bytes()));
        object["original_content_digest"] = object["node_digest"].clone();
        object["node_bytes"] = json!(bytes);
    }

    #[test]
    fn complete_empty_history_and_all_dimensions_preserve_siblings_without_invented_verification() {
        let mut input = input(SOURCE);
        let changes = [
            json!({"family":"governance","state":"approved"}),
            json!({"family":"verification","state":"unverified"}),
            json!({"family":"effectivity","state":"effective"}),
            json!({"family":"freshness","state":"current"}),
            json!({"family":"integrity","state":"clear"}),
            json!({"family":"synchronization","connector":"source-a","state":"paused","required_before_effective":true}),
        ];
        for (index, change) in changes.into_iter().enumerate() {
            event(&mut input["objects"][0], (index * 2 + 4) as u64, change);
        }
        let output = project(&input);
        assert_eq!(output["outcome"], "complete", "{output}");
        assert_eq!(output["documents"].as_array().unwrap().len(), 2);
        assert_ne!(
            output["objects"][0]["lifecycle_projection"]["status"],
            "verified"
        );
        let losses = output["objects"][0]["lifecycle_projection"]["loss_report"]
            .as_array()
            .unwrap();
        assert!(losses.iter().any(|loss| loss["dimension"] == "verification" && loss["recorded"] == "unverified"));
        assert!(
            losses
                .iter()
                .any(|loss| loss["dimension"] == "synchronization"
                    && loss["connector"] == "source-a")
        );
        for category in [
            "multi_source",
            "replay",
            "identity",
            "history",
            "placement",
            "provenance",
            "synchronization_configuration",
        ] {
            assert!(
                output["objects"][0]["loss_report"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|loss| loss["category"] == category)
            );
        }
        assert!(
            output["documents"][0]["adoc_source"]
                .as_str()
                .unwrap()
                .contains("related_to: [billing.second]")
        );
        input["objects"][1]["state_history_complete"] = json!(false);
        let output = project(&input);
        assert_eq!(output["outcome"], "partial", "{output}");
        assert!(
            !output["documents"][0]["adoc_source"]
                .as_str()
                .unwrap()
                .contains("related_to:")
        );
        assert!(
            output["objects"][0]["loss_report"]
                .as_array()
                .unwrap()
                .iter()
                .any(|loss| loss["category"] == "withheld_reference")
        );
    }

    #[test]
    fn portable_input_and_output_match_published_closed_schemas() {
        let input_schema: Value = serde_json::from_str(include_str!(
            "../../../../docs/agent/v0/schema/adoc.portable_projection_input.v0.schema.json"
        ))
        .unwrap();
        let output_schema: Value = serde_json::from_str(include_str!(
            "../../../../docs/agent/v0/schema/adoc.portable_projection.v0.schema.json"
        ))
        .unwrap();
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../adoc-cli/tests/fixtures/portable_projection/input.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&input_schema).unwrap();
        assert!(validator.is_valid(&fixture));
        let output = project(&fixture);
        assert_eq!(output["outcome"], "complete", "{output}");
        assert!(
            jsonschema::validator_for(&output_schema)
                .unwrap()
                .is_valid(&output)
        );
        let mut invalid = fixture;
        invalid["objects"][0]["state"] = json!({"governance":"approved"});
        assert!(!validator.is_valid(&invalid));
        assert_eq!(project(&invalid)["outcome"], "failed");
    }

    #[test]
    fn authored_carriers_outside_fields_round_trip() {
        let source = "::warning billing.warning\nseverity: high\nvisibility: internal\nfield_visibility: owner=restricted\nowner: billing\n--\nObserve account boundaries.\n::\n\n::policy billing.policy\nstatus: proposed\nowner: billing\napproved_by: [alice, bob]\neffective_at: 2026-09-01\n--\nReview billing changes.\n::\n\n::agent_instruction billing.instruction\nscope: billing\ntrust: team\nallowed_actions: [summarize, cite]\nforbidden_actions: [delete]\n--\nCite the retained evidence.\n::\n";
        let output = project(&input(source));
        assert_eq!(output["outcome"], "complete", "{output}");
        let all = output["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|document| document["adoc_source"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        for carrier in [
            "severity: high",
            "visibility: internal",
            "field_visibility: owner=restricted",
            "approved_by: [alice, bob]",
            "trust: team",
            "allowed_actions: [cite, summarize]",
            "forbidden_actions: [delete]",
        ] {
            assert!(all.contains(carrier), "{carrier}: {all}");
        }
    }

    #[test]
    fn missing_required_evidence_removes_dependents_but_preserves_unrelated_siblings() {
        let source = "::source billing.evidence\nkind: source_code\npath: src/billing.rs\n--\nBilling source evidence.\n::\n\n::claim billing.claim\nstatus: verified\nowner: billing\nverified_at: 2026-09-01\nevidence_ref: billing.evidence\n--\nClaim supported by retained code.\n::\n\n::claim billing.sibling\nstatus: draft\n--\nIndependent retained sibling.\n::\n";
        let mut value = input(source);
        assert_eq!(project(&value)["outcome"], "complete");
        let evidence = value["objects"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|object| object["object_id"] == "billing.evidence")
            .unwrap();
        evidence["state_history_complete"] = json!(false);
        let output = project(&value);
        assert_eq!(output["outcome"], "partial", "{output}");
        assert_eq!(output["documents"].as_array().unwrap().len(), 1);
        assert_eq!(
            output["documents"][0]["object_bindings"][0]["object_id"],
            "billing.sibling"
        );
    }

    #[test]
    fn malformed_nodes_events_and_fence_injection_omit_only_invalid_object() {
        let baseline = input(SOURCE);
        let mut cases = Vec::new();
        let mut value = baseline.clone();
        value["objects"][1]["node_digest"] = json!(sha256_prefixed(b"wrong"));
        cases.push(value);
        let mut value = baseline.clone();
        value["objects"][1]["object_id"] = json!("billing.foreign");
        cases.push(value);
        let mut value = baseline.clone();
        change_node(&mut value["objects"][1], |node| {
            node["body"] = json!("::\n::claim billing.injected\nstatus: draft\n--\ninjected\n::")
        });
        cases.push(value);
        let mut value = baseline.clone();
        change_node(&mut value["objects"][1], |node| {
            node["fields"]["owner"] = json!("billing\nstatus: verified")
        });
        cases.push(value);
        for mutation in [
            "subject",
            "policy",
            "digest",
            "ordinal",
            "history",
            "unknown_change",
        ] {
            let mut value = baseline.clone();
            event(
                &mut value["objects"][1],
                0,
                json!({"family":"governance","state":"approved"}),
            );
            let row = &mut value["objects"][1]["state_events"][0];
            let mut bytes: Value =
                serde_json::from_str(row["event_bytes"].as_str().unwrap()).unwrap();
            match mutation {
                "subject" => {
                    bytes["subject"]["version_id"] = json!("33333333-3333-3333-3333-999999999999")
                }
                "policy" => bytes["policy_version"] = json!(" "),
                "unknown_change" => bytes["change"]["state"] = json!("invented"),
                _ => {}
            }
            row["event_bytes"] = json!(bytes.to_string());
            row["record_digest"] = json!(sha256_prefixed(bytes.to_string().as_bytes()));
            match mutation {
                "digest" => row["record_digest"] = json!(sha256_prefixed(b"tampered")),
                "ordinal" => row["ordinal"] = json!("00"),
                "history" => value["objects"][1]["state_history_complete"] = json!(false),
                _ => {}
            }
            cases.push(value);
        }
        for value in cases {
            let output = project(&value);
            assert_eq!(output["outcome"], "partial", "{output}");
            assert_eq!(output["documents"].as_array().unwrap().len(), 1);
        }
        let mut duplicate = baseline.clone();
        duplicate["objects"]
            .as_array_mut()
            .unwrap()
            .push(baseline["objects"][1].clone());
        assert_eq!(
            project(&duplicate)["documents"].as_array().unwrap().len(),
            1
        );
        for (field, value) in [
            ("projection_version", json!("2")),
            ("schema_version", json!("unknown")),
            ("evaluation_date", json!("2026-9-8")),
        ] {
            let mut invalid = baseline.clone();
            invalid[field] = value;
            assert!(run_portable_projection(&serde_json::to_vec(&invalid).unwrap()).is_err());
        }
    }
}

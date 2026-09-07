use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use adoc_core::{RetrievalPolicy, SearchRecordScope};
use adoc_local::{
    ContradictionsInput, GraphInput, ImpactedChangedSet, ImpactedInput, LocalContext, SearchInput,
    StaleInput, UnrestrictedPathPolicy, WhyInput,
};
use serde_json::{Value, json};

fn policy(internal: bool) -> RetrievalPolicy {
    RetrievalPolicy {
        audience: if internal { "internal" } else { "public" }.into(),
        allowed_visibilities: if internal {
            BTreeSet::from(["public".into(), "internal".into()])
        } else {
            BTreeSet::from(["public".into()])
        },
        excluded_object_ids: BTreeSet::new(),
    }
}

fn write_config(root: &Path, policy: &RetrievalPolicy) {
    std::fs::write(
        root.join("agentdoc.config.yaml"),
        json!({
            "version": 1, "mode": "strict", "docs_path": "docs",
            "outputs": {"graph": "graph.json"}, "retrieval_policy": policy,
        })
        .to_string(),
    )
    .unwrap();
}

fn fixture(root: &Path) -> PathBuf {
    let node = |id: &str, visibility: &str| {
        json!({
            "type": "knowledge_object", "id": id, "kind": "claim", "status": "draft",
            "visibility": visibility, "content_hash": format!("sha256:{}", "a".repeat(64)), "body": "Billing credits.",
            "page_id": "billing.page", "source_span": {"path": "docs/billing.adoc", "line": 1, "column": 1},
            "fields": {"expires_at": "2000-01-01"}, "impacts": ["src/billing.rs"],
            "relations": {"depends_on": [], "supersedes": [], "related_to": []},
        })
    };
    let mut contradiction = node("billing.contradiction", "internal");
    contradiction["kind"] = json!("contradiction");
    contradiction["status"] = json!("unresolved");
    contradiction["severity"] = json!("high");
    contradiction["contradiction_claims"] = json!(["billing.visible"]);
    let artifact = root.join("graph.json");
    std::fs::write(
        &artifact,
        json!({
            "schema_version": "adoc.graph.v6", "repository_identity": null,
            "nodes": [node("billing.visible", "public"), node("billing.hidden", "internal"),
                node("billing.excluded", "public"), contradiction],
            "edges": [], "diagnostics": [],
        })
        .to_string(),
    )
    .unwrap();
    artifact
}

fn responses(context: &LocalContext<UnrestrictedPathPolicy>, artifact: Option<PathBuf>) -> Value {
    json!({
        "search": context.search(SearchInput {
            query: "billing".into(), artifact: artifact.clone(), search_artifact: None,
            semantic: false, lexical: true, kind: None, status: None, owner: None,
            source_path: None, related_to: None, relation: None, direction: None,
            top: NonZeroUsize::new(10).unwrap(), scope: SearchRecordScope::Blended,
        }).unwrap(),
        "why": context.why(WhyInput {object_id: "billing.hidden".into(), artifact: artifact.clone()}).unwrap(),
        "graph": context.graph(GraphInput {object_id: "billing.hidden".into(), artifact: artifact.clone(), relation: None, direction: None}).unwrap(),
        "stale": context.stale(StaleInput {artifact: artifact.clone(), within_days: None}).unwrap(),
        "contradictions": context.contradictions(ContradictionsInput {artifact: artifact.clone(), all: true}).unwrap(),
        "impacted": context.impacted(ImpactedInput {artifact, changed: ImpactedChangedSet::Paths(vec!["src/billing.rs".into()])}).unwrap(),
    })
}

#[test]
fn trusted_policy_overrides_project_policy_for_every_local_retrieval_loader() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let artifact = fixture(root);
    let ordinary = LocalContext::new(root.into(), UnrestrictedPathPolicy);
    for internal in [false, true] {
        let mut trusted = policy(internal);
        if internal {
            trusted
                .excluded_object_ids
                .insert("billing.excluded".into());
        }
        let bound = ordinary
            .clone()
            .with_retrieval_policy_override(trusted.clone());
        for explicit in [false, true] {
            let selected = explicit.then(|| artifact.clone());
            write_config(root, &trusted);
            let expected = responses(&ordinary, selected.clone());
            assert_eq!(expected["search"]["exit_code"], 0);
            assert!(!expected["search"]["records"].as_array().unwrap().is_empty());
            write_config(root, &policy(!internal));
            assert_ne!(
                responses(&ordinary, selected.clone()),
                expected,
                "ordinary contexts must retain project-policy discovery"
            );
            assert_eq!(
                serde_json::to_vec(&responses(&bound, selected)).unwrap(),
                serde_json::to_vec(&expected).unwrap(),
                "internal={internal}, explicit={explicit}"
            );
        }
    }
}

#[test]
fn trusted_policy_does_not_bypass_project_config_refusals() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let artifact = fixture(root);
    let ordinary = LocalContext::new(root.into(), UnrestrictedPathPolicy);
    let bound = ordinary
        .clone()
        .with_retrieval_policy_override(policy(true));
    for invalid_yaml in [true, false] {
        if invalid_yaml {
            std::fs::write(root.join("agentdoc.config.yaml"), "[unterminated").unwrap();
        } else {
            let mut invalid = policy(true);
            invalid.audience = "unresolved".into();
            write_config(root, &invalid);
        }
        let expected = responses(&ordinary, Some(artifact.clone()));
        assert!(
            expected
                .as_object()
                .unwrap()
                .values()
                .all(|v| v["exit_code"] == 2)
        );
        assert_eq!(
            serde_json::to_vec(&responses(&bound, Some(artifact.clone()))).unwrap(),
            serde_json::to_vec(&expected).unwrap(),
            "invalid_yaml={invalid_yaml}"
        );
    }
}

#[test]
fn invalid_trusted_policy_refuses_without_project_policy_fallback() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let artifact = fixture(root);
    write_config(root, &policy(true));
    let ordinary = LocalContext::new(root.into(), UnrestrictedPathPolicy);
    assert_eq!(
        responses(&ordinary, Some(artifact.clone()))["why"]["exit_code"],
        0
    );
    let mut invalid = policy(false);
    invalid.audience = "unresolved".into();
    let bound = ordinary.with_retrieval_policy_override(invalid);
    let actual = responses(&bound, Some(artifact));
    for (name, outcome) in actual.as_object().unwrap() {
        assert_eq!(outcome["exit_code"], 2, "{name}");
        let diagnostics = outcome
            .get("diagnostics")
            .unwrap_or(&outcome["envelope"]["diagnostics"]);
        assert_eq!(diagnostics.as_array().unwrap().len(), 1, "{name}");
        assert_eq!(
            diagnostics[0]["code"], "retrieval.audience_unresolved",
            "{name}"
        );
    }
}

#[test]
fn read_access_tracks_only_returned_roots_and_copied_contributors() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let artifact = fixture(root);
    write_config(root, &policy(true));
    let mut document: Value = serde_json::from_slice(&std::fs::read(&artifact).unwrap()).unwrap();
    document["nodes"][0]["fields"]["owner"] = json!("INTERNAL_OWNER_E63");
    document["nodes"][0]["field_visibility"] = json!({"owner":"internal"});
    std::fs::write(&artifact, document.to_string()).unwrap();
    let context = LocalContext::new(root.into(), UnrestrictedPathPolicy);
    let why = context
        .why(WhyInput {
            object_id: "billing.visible".into(),
            artifact: Some(artifact.clone()),
        })
        .unwrap();
    assert_eq!(why.exit_code, 0, "{:?}", why.diagnostics);
    assert_eq!(
        why.read_access
            .objects
            .iter()
            .map(|o| o.object_id.as_str())
            .collect::<Vec<_>>(),
        vec!["billing.contradiction", "billing.visible"]
    );
    assert_eq!(why.read_access.sensitive_objects().len(), 2);
    assert_eq!(
        why.records[0].record.classification,
        Some(adoc_core::SensitiveClassification::Internal)
    );
    let nohit = context
        .search(SearchInput {
            query: "notpresentanywhere".into(),
            artifact: Some(artifact.clone()),
            search_artifact: None,
            semantic: false,
            lexical: true,
            kind: None,
            status: None,
            owner: None,
            source_path: None,
            related_to: None,
            relation: None,
            direction: None,
            top: NonZeroUsize::new(10).unwrap(),
            scope: SearchRecordScope::Blended,
        })
        .unwrap();
    assert_eq!(nohit.exit_code, 0);
    assert!(nohit.read_access.objects.is_empty());
    let public = context.with_retrieval_policy_override(policy(false));
    let why = public
        .why(WhyInput {
            object_id: "billing.visible".into(),
            artifact: Some(artifact),
        })
        .unwrap();
    assert_eq!(why.exit_code, 0, "{:?}", why.diagnostics);
    assert_eq!(why.read_access.objects.len(), 1);
    assert!(why.read_access.sensitive_objects().is_empty());
    assert!(why.records[0].record.owner.is_none());
}

#[test]
fn local_projection_removes_hidden_witnesses_and_uses_actual_dedicated_presence() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let artifact = fixture(root);
    write_config(root, &policy(true));
    let context = LocalContext::new(root.into(), UnrestrictedPathPolicy);
    let original: Value = serde_json::from_slice(&std::fs::read(&artifact).unwrap()).unwrap();
    let mut document = original.clone();
    document["nodes"].as_array_mut().unwrap().truncate(2);
    document["nodes"][0]["body"] = json!("Private body references billing.hidden.");
    document["nodes"][0]["fields"]["owner"] = json!("Authorized owner");
    document["nodes"][0]["field_visibility"] = json!({"body":"restricted","owner":"internal","nonexistent":"restricted","severity":"restricted"});
    document["edges"] =
        json!([{"kind":"reference","source":"billing.visible","target":"billing.hidden"}]);
    let read = |document: &Value| {
        std::fs::write(&artifact, document.to_string()).unwrap();
        context
            .why(WhyInput {
                object_id: "billing.visible".into(),
                artifact: Some(artifact.clone()),
            })
            .unwrap()
    };
    let result = read(&document);
    assert_eq!(result.exit_code, 0, "{:?}", result.diagnostics);
    assert_eq!(result.records[0].record.body, "");
    assert_eq!(
        result.records[0].record.owner.as_deref(),
        Some("Authorized owner")
    );
    assert_eq!(
        result
            .read_access
            .objects
            .iter()
            .map(|o| o.object_id.as_str())
            .collect::<Vec<_>>(),
        vec!["billing.visible"]
    );
    assert_eq!(
        result.records[0].record.content_hash,
        original["nodes"][0]["content_hash"]
    );
    document["nodes"][0]["field_visibility"]["status"] = json!("restricted");
    let denied = read(&document);
    assert!(denied.records.is_empty());
    assert!(denied.read_access.objects.is_empty());
    document["nodes"][0]["field_visibility"]
        .as_object_mut()
        .unwrap()
        .remove("status");
    document["nodes"][0]["source_span"] = json!({"path":"","line":0,"column":0});
    let malformed = read(&document);
    assert_eq!(malformed.exit_code, 2);
    assert!(malformed.records.is_empty());
}

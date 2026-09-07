use adoc_core::{
    GatewaySensitiveAccessCommand as Command, GatewaySensitiveAccessInput,
    GatewaySensitiveAccessObject, SensitiveClassification, build_gateway_sensitive_access,
    validate_gateway_sensitive_access, validate_sensitive_access,
};
use serde_json::{Value, json};

fn input() -> GatewaySensitiveAccessInput {
    GatewaySensitiveAccessInput {
        event_id: "00000000-0000-4000-8000-000000000001".into(),
        workspace_id: "00000000-0000-4000-8000-000000000002".into(),
        repository_id: "00000000-0000-4000-8000-000000000003".into(),
        principal_id: "00000000-0000-4000-8000-000000000004".into(),
        auth_session_id: "00000000-0000-4000-8000-000000000005".into(),
        gateway_session_id: "00000000-0000-4000-8000-000000000006".into(),
        local_policy_digest: format!("sha256:{}", "b".repeat(64)),
        command: Command::Search,
        sequence: 1,
        objects: vec![GatewaySensitiveAccessObject {
            object_id: "guide.sensitive--field".into(),
            content_hash: format!("sha256:{}", "a".repeat(64)),
            classification: SensitiveClassification::Internal,
        }],
    }
}
fn schema() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/agent/v0/schema/adoc.sensitive_access.v1.schema.json"
    ))
    .unwrap();
    jsonschema::validator_for(&schema).unwrap()
}
#[test]
fn gateway_event_roundtrips_six_commands_and_does_not_widen_v0() {
    for command in [
        Command::Search,
        Command::Why,
        Command::Graph,
        Command::Stale,
        Command::Contradictions,
        Command::ImpactedBy,
    ] {
        let mut input = input();
        input.command = command;
        let event = build_gateway_sensitive_access(input).unwrap();
        let bytes = event.to_canonical_json().unwrap();
        assert_eq!(
            validate_gateway_sensitive_access(bytes.as_bytes()).unwrap(),
            event
        );
        assert!(schema().is_valid(&serde_json::from_str::<Value>(&bytes).unwrap()));
        assert!(validate_sensitive_access(bytes.as_bytes()).is_err());
        assert_eq!(event.content_digest().unwrap().len(), 71);
    }
    let old = json!({
        "schema_version":"adoc.sensitive_access.v0", "event_id":input().event_id,
        "context":{"kind":"managed_workspace","workspace_id":input().workspace_id},
        "caller":{"principal_id":input().principal_id,"session_id":input().auth_session_id},
        "command":"search","policy_version":input().gateway_session_id,"sequence":1,
        "objects":[{"canonical_id":input().repository_id,"version_id":input().gateway_session_id,"object_id":"guide.field","content_hash":input().objects[0].content_hash,"classification":"internal"}]
    });
    let bytes = serde_json::to_vec(&old).unwrap();
    assert!(validate_sensitive_access(&bytes).is_ok());
    assert!(validate_gateway_sensitive_access(&bytes).is_err());
}
#[test]
fn gateway_event_domain_and_schema_refuse_invalid_closed_metadata() {
    let value = serde_json::to_value(build_gateway_sensitive_access(input()).unwrap()).unwrap();
    for (path, bad) in [
        ("/schema_version", json!("adoc.sensitive_access.v2")),
        ("/event_id", json!("00000000-0000-4000-8000-00000000000A")),
        ("/context/kind", json!("managed_workspace")),
        ("/reporting_basis", json!("native_managed")),
        ("/sequence", json!(0)),
        ("/sequence", json!(9_007_199_254_740_992_u64)),
        ("/sequence", json!(1.5)),
        ("/objects", json!([])),
        ("/objects/0/classification", json!("public")),
        ("/objects/0/content_hash", json!("sha256:invalid")),
        ("/objects/0/object_id", json!("bad")),
        ("/caller", json!([])),
        ("/context", json!([])),
        ("/objects/0", json!([])),
    ] {
        let mut invalid = value.clone();
        *invalid.pointer_mut(path).unwrap() = bad;
        assert!(
            validate_gateway_sensitive_access(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "{path}"
        );
        assert!(!schema().is_valid(&invalid), "{path}");
    }
    for path in ["", "/context", "/caller", "/objects/0"] {
        let mut invalid = value.clone();
        invalid
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("body".into(), json!("secret"));
        assert!(validate_gateway_sensitive_access(&serde_json::to_vec(&invalid).unwrap()).is_err());
        assert!(!schema().is_valid(&invalid));
    }
    let bytes = serde_json::to_string(&value).unwrap();
    for token in ["1.0", "1e0"] {
        assert!(
            validate_gateway_sensitive_access(
                bytes
                    .replace("\"sequence\":1", &format!("\"sequence\":{token}"))
                    .as_bytes()
            )
            .is_err()
        );
    }
    assert!(
        validate_gateway_sensitive_access(
            bytes
                .replacen("\"event_id\":", "\"sequence\":1,\"event_id\":", 1)
                .as_bytes()
        )
        .is_err()
    );
}
#[test]
fn gateway_event_limits_order_and_duplicate_subjects_fail_closed() {
    let mut duplicate = input();
    duplicate.objects.push(duplicate.objects[0].clone());
    assert!(build_gateway_sensitive_access(duplicate).is_err());
    let mut bounded = input();
    bounded.objects = (0..1000)
        .map(|n| GatewaySensitiveAccessObject {
            object_id: format!("guide.subject-{n:04}"),
            ..bounded.objects[0].clone()
        })
        .collect();
    let valid = build_gateway_sensitive_access(bounded.clone()).unwrap();
    assert!(
        validate_gateway_sensitive_access(valid.to_canonical_json().unwrap().as_bytes()).is_ok()
    );
    bounded.objects.push(GatewaySensitiveAccessObject {
        object_id: "guide.zzz".into(),
        ..bounded.objects[0].clone()
    });
    assert!(build_gateway_sensitive_access(bounded).is_err());
    let mut value = serde_json::to_value(valid).unwrap();
    value["objects"].as_array_mut().unwrap().reverse();
    assert!(validate_gateway_sensitive_access(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(validate_gateway_sensitive_access(&vec![b' '; 1024 * 1024 + 1]).is_err());
}

#[test]
fn gateway_policy_digest_binds_every_effective_policy_dimension() {
    use adoc_core::{RetrievalPolicy, gateway_policy_digest};
    let public = RetrievalPolicy {
        audience: "public".into(),
        allowed_visibilities: ["public".into()].into(),
        excluded_object_ids: Default::default(),
    };
    let digest = gateway_policy_digest(&public).unwrap();
    let mut changed = public.clone();
    changed.excluded_object_ids.insert("guide.private".into());
    assert_ne!(gateway_policy_digest(&changed).unwrap(), digest);
    changed = public.clone();
    changed.audience = "internal".into();
    assert_ne!(gateway_policy_digest(&changed).unwrap(), digest);
    changed.allowed_visibilities.insert("internal".into());
    assert_ne!(
        gateway_policy_digest(&changed).unwrap(),
        gateway_policy_digest(&RetrievalPolicy {
            audience: "internal".into(),
            ..public.clone()
        })
        .unwrap()
    );
    changed.audience = "unknown".into();
    assert!(gateway_policy_digest(&changed).is_err());
}

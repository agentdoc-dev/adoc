use adoc_core::{SensitiveAccessEvent, validate_sensitive_access};
use serde_json::{Value, json};

fn event() -> Value {
    json!({
        "schema_version":"adoc.sensitive_access.v0",
        "event_id":"00000000-0000-4000-8000-000000000001",
        "context":{"kind":"managed_workspace","workspace_id":"00000000-0000-4000-8000-000000000002"},
        "caller":{"principal_id":"00000000-0000-4000-8000-000000000003","session_id":"00000000-0000-4000-8000-000000000004"},
        "command":"search", "policy_version":"00000000-0000-4000-8000-000000000005", "sequence":1,
        "objects":[{"canonical_id":"00000000-0000-4000-8000-000000000006","version_id":"00000000-0000-4000-8000-000000000007","object_id":"policy.example","content_hash":format!("sha256:{}", "a".repeat(64)),"classification":"internal"}]
    })
}

#[test]
fn sensitive_access_contract_roundtrips_and_constructor_validates() {
    let value = event();
    let parsed = validate_sensitive_access(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), value);
    let made = SensitiveAccessEvent::new(
        parsed.event_id.clone(),
        parsed.context.clone(),
        parsed.caller.clone(),
        parsed.command,
        parsed.policy_version.clone(),
        parsed.sequence,
        parsed.objects.clone(),
    )
    .unwrap();
    assert_eq!(made, parsed);
    assert!(
        SensitiveAccessEvent::new(
            parsed.event_id,
            parsed.context,
            parsed.caller,
            parsed.command,
            parsed.policy_version,
            0,
            parsed.objects
        )
        .is_err()
    );
    for command in ["search", "why"] {
        for class in ["internal", "restricted"] {
            let mut valid = value.clone();
            valid["command"] = json!(command);
            valid["objects"][0]["classification"] = json!(class);
            valid["sequence"] = json!(9_007_199_254_740_991_u64);
            assert!(validate_sensitive_access(&serde_json::to_vec(&valid).unwrap()).is_ok());
            assert!(schema().is_valid(&valid));
        }
    }
}

fn schema() -> jsonschema::Validator {
    let value: Value = serde_json::from_str(include_str!(
        "../../../docs/agent/v0/schema/adoc.sensitive_access.v0.schema.json"
    ))
    .unwrap();
    jsonschema::validator_for(&value).unwrap()
}

#[test]
fn sensitive_access_schema_and_domain_reject_closed_shape_and_invalid_values() {
    let schema = schema();
    for (path, bad) in [
        ("/schema_version", json!("adoc.sensitive_access.v1")),
        ("/event_id", json!("not-uuid")),
        ("/context/kind", json!("repository")),
        ("/context/workspace_id", json!("workspace")),
        ("/caller/principal_id", json!(null)),
        ("/caller/session_id", json!("")),
        ("/command", json!("query text")),
        ("/policy_version", json!("policy")),
        ("/sequence", json!(0)),
        ("/sequence", json!(-1)),
        ("/sequence", json!(1.5)),
        ("/sequence", json!(9_007_199_254_740_992_u64)),
        ("/objects", json!([])),
        ("/objects/0/canonical_id", json!("bad")),
        ("/objects/0/version_id", json!("bad")),
        ("/objects/0/object_id", json!("bad")),
        (
            "/objects/0/content_hash",
            json!(format!("sha256:{}", "A".repeat(64))),
        ),
        ("/objects/0/classification", json!("public")),
    ] {
        let mut value = event();
        *value.pointer_mut(path).unwrap() = bad;
        assert!(
            validate_sensitive_access(&serde_json::to_vec(&value).unwrap()).is_err(),
            "domain {path}"
        );
        assert!(!schema.is_valid(&value), "schema {path}");
    }
    for path in ["", "/context", "/caller", "/objects/0"] {
        let mut value = event();
        value
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("body".into(), json!("private"));
        assert!(validate_sensitive_access(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(!schema.is_valid(&value));
    }
}

#[test]
fn sensitive_access_requires_ordered_distinct_subjects() {
    let mut value = event();
    let mut second = value["objects"][0].clone();
    second["object_id"] = json!("policy.second");
    second["canonical_id"] = json!("00000000-0000-4000-8000-000000000008");
    second["version_id"] = json!("00000000-0000-4000-8000-000000000009");
    value["objects"].as_array_mut().unwrap().push(second);
    assert!(validate_sensitive_access(&serde_json::to_vec(&value).unwrap()).is_ok());
    for key in ["object_id", "canonical_id", "version_id"] {
        let mut bad = value.clone();
        bad["objects"][1][key] = bad["objects"][0][key].clone();
        assert!(
            validate_sensitive_access(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{key}"
        );
    }
    value["objects"].as_array_mut().unwrap().reverse();
    assert!(validate_sensitive_access(&serde_json::to_vec(&value).unwrap()).is_err());
}

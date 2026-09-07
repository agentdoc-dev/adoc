use adoc_core::strictest_contributing_visibility;

#[test]
fn strictest_visibility_uses_all_contributors_regardless_of_order_and_authored_floors() {
    for contributors in [
        vec![Some("internal"), Some("restricted")],
        vec![Some("restricted"), Some("internal")],
    ] {
        assert_eq!(
            strictest_contributing_visibility(None, None, &contributors)
                .unwrap()
                .as_deref(),
            Some("restricted")
        );
    }
    assert_eq!(
        strictest_contributing_visibility(Some("restricted"), Some("public"), &[Some("internal")])
            .unwrap()
            .as_deref(),
        Some("restricted")
    );
    assert_eq!(
        strictest_contributing_visibility(Some("public"), Some("restricted"), &[Some("internal")])
            .unwrap()
            .as_deref(),
        Some("restricted")
    );
    assert_eq!(
        strictest_contributing_visibility(None, None, &[Some("public")])
            .unwrap()
            .as_deref(),
        Some("public")
    );
}

#[test]
fn missing_evidence_is_unresolved_but_never_hides_invalid_classes() {
    for contributors in [
        vec![],
        vec![None],
        vec![Some("restricted"), None],
        vec![None, Some("internal")],
    ] {
        assert_eq!(
            strictest_contributing_visibility(
                Some("restricted"),
                Some("restricted"),
                &contributors
            )
            .unwrap(),
            None
        );
    }
    for invalid in ["", "secret", "Internal", " internal "] {
        assert!(strictest_contributing_visibility(Some(invalid), None, &[]).is_err());
        assert!(strictest_contributing_visibility(None, Some(invalid), &[None]).is_err());
        assert!(strictest_contributing_visibility(None, None, &[None, Some(invalid)]).is_err());
        assert!(strictest_contributing_visibility(None, None, &[Some(invalid), None]).is_err());
    }
}

use adoc_core::{
    ManagedFieldContributionInput, ManagedFieldProvenanceInput, build_managed_field_provenance,
    validate_managed_field_provenance,
};
use serde_json::{Value, json};

fn uuid(number: usize) -> String {
    format!("00000000-0000-4000-8000-{number:012x}")
}

fn fixture() -> Value {
    json!({"schema_version":"adoc.managed_field_provenance.v0", "workspace_id":uuid(1), "canonical_id":uuid(2), "version_id":uuid(3), "content_digest":format!("sha256:{}", "a".repeat(64)), "fields":[{"selector":"/fields/owner", "assertion_ids":[uuid(4), uuid(5)]}]})
}

fn schema() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/agent/v0/schema/adoc.managed_field_provenance.v0.schema.json"
    ))
    .unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

#[test]
fn provenance_roundtrips_and_canonicalizes_constructor_and_consumer_order() {
    let manifest = build_managed_field_provenance(ManagedFieldProvenanceInput {
        workspace_id: uuid(1),
        canonical_id: uuid(2),
        version_id: uuid(3),
        content_digest: format!("sha256:{}", "a".repeat(64)),
        fields: vec![
            ManagedFieldContributionInput {
                selector: "/fields/owner".into(),
                assertion_ids: vec![uuid(5), uuid(4)],
            },
            ManagedFieldContributionInput {
                selector: "/body".into(),
                assertion_ids: vec![uuid(5)],
            },
        ],
    })
    .unwrap();
    let canonical = manifest.to_canonical_json().unwrap();
    let parsed = validate_managed_field_provenance(canonical.as_bytes()).unwrap();
    assert_eq!(manifest, parsed);
    assert_eq!(parsed.fields()[0].selector(), "/body");
    assert_eq!(parsed.fields()[1].assertion_ids(), &[uuid(4), uuid(5)]);
    assert_eq!(parsed.workspace_id(), uuid(1));
    assert_eq!(parsed.canonical_id(), uuid(2));
    assert_eq!(parsed.version_id(), uuid(3));
    assert_eq!(
        parsed.content_digest(),
        format!("sha256:{}", "a".repeat(64))
    );
    let mut shuffled: Value = serde_json::from_str(&canonical).unwrap();
    shuffled["fields"].as_array_mut().unwrap().reverse();
    shuffled["fields"][0]["assertion_ids"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_eq!(
        validate_managed_field_provenance(&serde_json::to_vec_pretty(&shuffled).unwrap())
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        canonical
    );
    assert!(schema().is_valid(&serde_json::from_str::<Value>(&canonical).unwrap()));
}

#[test]
fn provenance_schema_and_domain_refuse_invalid_shape_coordinates_and_bounds() {
    let schema = schema();
    let object = fixture();
    let root_array = json!([
        object["schema_version"],
        object["workspace_id"],
        object["canonical_id"],
        object["version_id"],
        object["content_digest"],
        object["fields"]
    ]);
    let mut nested_array = object;
    nested_array["fields"][0] = json!(["/fields/owner", [uuid(4), uuid(5)]]);
    for value in [root_array, nested_array] {
        assert!(!schema.is_valid(&value));
        assert!(validate_managed_field_provenance(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    for member in ["schema_version", "selector"] {
        let duplicate = fixture().to_string().replacen(
            &format!("\"{member}\":"),
            &format!("\"{member}\":\"ignored\",\"{member}\":"),
            1,
        );
        assert!(validate_managed_field_provenance(duplicate.as_bytes()).is_err());
    }
    for (path, bad) in [
        (
            "/schema_version",
            json!("adoc.managed_field_provenance.v99"),
        ),
        ("/workspace_id", json!("workspace-id")),
        (
            "/canonical_id",
            json!("00000000-0000-4000-8000-00000000000A"),
        ),
        ("/version_id", json!(null)),
        (
            "/content_digest",
            json!(format!("sha256:{}", "A".repeat(64))),
        ),
        ("/fields", json!([])),
        ("/fields/0/selector", json!("/fields/")),
        ("/fields/0/selector", json!("/fields/nested/path")),
        ("/fields/0/selector", json!("/fields/owner~")),
        ("/fields/0/selector", json!("/fields/owner~2x")),
        ("/fields/0/selector", json!("/status")),
        (
            "/fields/0/selector",
            json!(format!("/fields/{}", "x".repeat(249))),
        ),
        ("/fields/0/assertion_ids", json!([])),
        ("/fields/0/assertion_ids", json!([uuid(4), uuid(4)])),
        ("/fields/0/assertion_ids", json!(["external-envelope-id"])),
        ("/fields/0/assertion_ids", json!(null)),
    ] {
        let mut value = fixture();
        *value.pointer_mut(path).unwrap() = bad;
        assert!(
            validate_managed_field_provenance(&serde_json::to_vec(&value).unwrap()).is_err(),
            "domain {path}"
        );
        assert!(!schema.is_valid(&value), "schema {path}");
    }
    for path in ["", "/fields/0"] {
        for key in ["body", "classification", "received_at"] {
            let mut value = fixture();
            value
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), json!("private"));
            assert!(
                validate_managed_field_provenance(&serde_json::to_vec(&value).unwrap()).is_err()
            );
            assert!(!schema.is_valid(&value));
        }
    }
    for count in [100, 101] {
        for many_fields in [true, false] {
            let mut value = fixture();
            if many_fields {
                value["fields"] = json!((0..count).map(|i| json!({"selector":format!("/fields/field{i}"),"assertion_ids":[uuid(4)]})).collect::<Vec<_>>());
            } else {
                value["fields"][0]["assertion_ids"] =
                    json!((0..count).map(uuid).collect::<Vec<_>>());
            }
            assert_eq!(
                validate_managed_field_provenance(&serde_json::to_vec(&value).unwrap()).is_ok(),
                count == 100
            );
            assert_eq!(schema.is_valid(&value), count == 100);
        }
    }
}

#[test]
fn pointer_tokens_are_canonical_and_duplicate_selectors_cannot_conflict() {
    let schema = schema();
    for selector in [
        "/body".to_string(),
        "/fields/owner".into(),
        "/fields/a~1b~0c".into(),
        "/fields/~01".into(),
        format!("/fields/{}", "é".repeat(248)),
    ] {
        let mut value = fixture();
        value["fields"][0]["selector"] = json!(selector);
        assert!(validate_managed_field_provenance(&serde_json::to_vec(&value).unwrap()).is_ok());
        assert!(schema.is_valid(&value));
    }
    let mut value = fixture();
    value["fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({"selector":"/fields/owner", "assertion_ids":[uuid(9)]}));
    assert!(validate_managed_field_provenance(&serde_json::to_vec(&value).unwrap()).is_err());
    let mut input = ManagedFieldProvenanceInput {
        workspace_id: uuid(1),
        canonical_id: uuid(2),
        version_id: uuid(3),
        content_digest: format!("sha256:{}", "a".repeat(64)),
        fields: vec![],
    };
    assert!(build_managed_field_provenance(input.clone()).is_err());
    input.fields.push(ManagedFieldContributionInput {
        selector: "/body".into(),
        assertion_ids: vec![uuid(1), uuid(1)],
    });
    assert!(build_managed_field_provenance(input).is_err());
}

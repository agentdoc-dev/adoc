mod support;

use std::fs;

use serde_json::Value;
use support::{TestWorkspace, adoc_command};

#[test]
fn local_field_floors_hide_owner_and_preserve_public_siblings_and_hash() {
    let workspace = TestWorkspace::new("gateway-field-floor");
    workspace.write("docs/index.adoc", "# Guide @doc(team.guide)\n\n::claim guide.field\nstatus: draft\nvisibility: public\nowner: INTERNAL_OWNER_E63\nfield_visibility: owner=internal\n--\nPublic body.\n::\n");
    let build = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "build",
            "docs",
            "--out",
            "dist",
            "--no-embeddings",
            "--as-of",
            "2026-09-07",
        ])
        .output()
        .unwrap();
    assert!(build.status.success(), "{build:?}");
    let original = fs::read(workspace.root.join("dist/docs.graph.json")).unwrap();
    let graph: Value = serde_json::from_slice(&original).unwrap();
    let hash = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "guide.field")
        .unwrap()["content_hash"]
        .clone();
    let run = || {
        let output = adoc_command()
            .current_dir(&workspace.root)
            .args([
                "why",
                "guide.field",
                "--artifact",
                "dist/docs.graph.json",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let public = run();
    assert!(
        public["records"][0].get("owner").is_none(),
        "E63T1-01: {public}"
    );
    assert_eq!(public["records"][0]["body"], "Public body.");
    assert_eq!(public["records"][0]["content_hash"], hash);
    assert!(public["records"][0].get("classification").is_none());
    workspace.write("agentdoc.config.yaml", "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy:\n  audience: internal\n  allowed_visibilities: [public, internal]\n");
    let internal = run();
    assert_eq!(internal["records"][0]["owner"], "INTERNAL_OWNER_E63");
    assert_eq!(internal["records"][0]["classification"], "internal");
    assert_eq!(internal["records"][0]["content_hash"], hash);
    assert!(
        internal.get("sensitive_access").is_none(),
        "CLI is audit-exempt"
    );
    assert_eq!(
        fs::read(workspace.root.join("dist/docs.graph.json")).unwrap(),
        original
    );
}

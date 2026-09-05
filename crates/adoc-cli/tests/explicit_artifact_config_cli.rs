mod support;

use std::fs;

use serde_json::{Value, json};
use support::{TestWorkspace, adoc_command, fixture_path, stderr};

#[test]
fn explicit_artifacts_ignore_broken_config_only_for_non_policy_drivers() {
    let workspace = TestWorkspace::new("explicit-artifact-config");
    let graph_text = fs::read_to_string(fixture_path("v1_1_why/valid_artifact.graph.json"))
        .expect("graph fixture readable");
    workspace.write("dist/docs.graph.json", &graph_text);
    let graph: Value = serde_json::from_str(&graph_text).expect("graph JSON");
    let target = "billing.refunds.issue-credit";
    let object = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == target)
        .unwrap();
    workspace.write(
        "patch.json",
        &json!({
            "schema_version": "adoc.patch.v0",
            "op": "replace_body",
            "target": target,
            "base_hash": object["content_hash"],
            "changes": {"body": "Refunds issue a credit after review."},
            "reason": "Clarify refund behavior."
        })
        .to_string(),
    );

    for (args, needs_policy) in [
        (vec!["graph", target], false),
        (vec!["stale"], false),
        (vec!["contradictions"], false),
        (vec!["impacted-by", "src/billing.rs"], false),
        (vec!["patch", "--check", "patch.json"], false),
        (vec!["why", target], true),
        (vec!["search", target, "--lexical"], true),
    ] {
        let run = |explicit_artifact| {
            let mut command = adoc_command();
            command.current_dir(&workspace.root).args(&args);
            if explicit_artifact {
                command.args(["--artifact", "dist/docs.graph.json"]);
            }
            command
                .args(["--format", "json"])
                .output()
                .expect("CLI runs")
        };
        let baseline = run(true);
        assert!(baseline.status.success(), "{args:?}: {}", stderr(&baseline));
        workspace.write("agentdoc.config.yaml", "version: [\n");
        let explicit = run(true);
        if needs_policy {
            assert_eq!(
                explicit.status.code(),
                Some(2),
                "{args:?} must discover policy"
            );
            let envelope: Value = serde_json::from_slice(&explicit.stdout).unwrap();
            assert_eq!(
                envelope["diagnostics"][0]["code"],
                "retrieval.policy_invalid"
            );
        } else {
            assert!(explicit.status.success(), "{args:?}: {}", stderr(&explicit));
            assert_eq!(
                serde_json::from_slice::<Value>(&explicit.stdout).unwrap(),
                serde_json::from_slice::<Value>(&baseline.stdout).unwrap(),
                "{args:?} must preserve the explicit-artifact result",
            );
        }
        let implicit = run(false);
        assert!(
            !implicit.status.success(),
            "{args:?} must resolve config defaults"
        );
        if needs_policy {
            assert_eq!(implicit.status.code(), Some(2));
            let envelope: Value = serde_json::from_slice(&implicit.stdout).unwrap();
            assert_eq!(
                envelope["diagnostics"][0]["code"],
                "retrieval.policy_invalid"
            );
        } else {
            assert!(stderr(&implicit).contains("config.parse"));
        }
        fs::remove_file(workspace.root.join("agentdoc.config.yaml")).unwrap();
    }
}

#[test]
fn explicit_search_paths_preserve_provider_defaults_while_loading_policy() {
    let pilot = support::v1_4::build_v1_4_pilot();
    let workspace = &pilot._workspace;
    for (name, path) in [
        ("docs.graph.json", &pilot.artifact_path),
        ("docs.search.json", &pilot.search_path),
    ] {
        workspace.write(
            &format!("configured/{name}"),
            &fs::read_to_string(path).unwrap(),
        );
    }
    fs::rename(workspace.root.join("dist"), workspace.root.join("explicit")).unwrap();
    let config = concat!(
        "version: 1\nmode: strict\ndocs_path: .\n",
        "outputs:\n  graph: configured/docs.graph.json\n  search: configured/docs.search.json\n",
        "embeddings:\n  provider: deterministic\n",
        "retrieval_policy:\n  audience: public\n  allowed_visibilities: [public]\n",
        "  excluded_object_ids: [billing.credits.ledger-source]\n",
    );

    for semantic in [false, true] {
        let run = |graph_explicit, search_explicit, test_provider: bool| {
            let mut command = adoc_command();
            command
                .current_dir(&workspace.root)
                .args(["search", "billing", "--format", "json"]);
            if !test_provider {
                // Local resolves only its fastembed header: it mismatches the
                // deterministic artifact before any model can be loaded.
                command.env_remove("ADOC_TEST_EMBEDDING_PROVIDER");
            }
            if semantic {
                command.arg("--semantic");
            }
            if graph_explicit {
                command.args(["--artifact", "explicit/docs.graph.json"]);
            }
            if search_explicit {
                command.args(["--search-artifact", "explicit/docs.search.json"]);
            }
            command.output().expect("CLI runs")
        };
        let baseline = run(true, true, false);
        assert_eq!(baseline.status.code(), Some(2));
        let baseline: Value = serde_json::from_slice(&baseline.stdout).unwrap();
        assert_eq!(baseline["diagnostics"][0]["code"], "search.model_mismatch");
        workspace.write("agentdoc.config.yaml", config);
        let explicit = run(true, true, false);
        assert_eq!(
            explicit.status.code(),
            Some(2),
            "semantic={semantic}: {explicit:?}"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&explicit.stdout).unwrap(),
            baseline
        );

        // A missing path still enables configured paths and provider. With
        // both paths explicit, the existing test provider lets us also prove
        // that policy was loaded despite skipping non-policy config defaults.
        for (graph_explicit, search_explicit) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let output = run(
                graph_explicit,
                search_explicit,
                graph_explicit && search_explicit,
            );
            assert!(output.status.success(), "{output:?}");
            let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
            let records = envelope["records"].as_array().unwrap();
            assert!(!records.is_empty());
            assert!(
                records
                    .iter()
                    .all(|record| record["id"] != "billing.credits.ledger-source")
            );
            assert!(
                records.iter().all(|record| record["match"]["mode"]
                    == if semantic { "semantic" } else { "hybrid" })
            );
        }
        fs::remove_file(workspace.root.join("agentdoc.config.yaml")).unwrap();
    }
}

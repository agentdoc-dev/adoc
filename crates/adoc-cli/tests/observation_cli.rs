//! V6.5.2 `observation` Knowledge Object CLI acceptance (PRD §13.9).

mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

/// The PRD §13.9 example, verbatim.
const PRD_EXAMPLE_OBSERVATION: &str = "\
# Onboarding findings @doc(team.onboarding-findings)

Findings from support, analytics, and research.

::observation onboarding.credit-confusion
status: observed
source: support_tickets
sample_size: 37
observed_at: 2026-04-30
--
Users often misunderstand credit usage before their first generation.
::
";

const NEGATIVE_SAMPLE_SIZE_OBSERVATION: &str = "\
# Onboarding findings @doc(team.onboarding-findings)

Findings from support, analytics, and research.

::observation onboarding.credit-confusion
status: observed
sample_size: -3
--
Users often misunderstand credit usage before their first generation.
::
";

const DANGLING_EVIDENCE_REF_OBSERVATION: &str = "\
# Onboarding findings @doc(team.onboarding-findings)

Findings from support, analytics, and research.

::observation onboarding.credit-confusion
status: observed
evidence_ref: no.such.source
--
Users often misunderstand credit usage before their first generation.
::
";

const VERIFIED_STATUS_OBSERVATION: &str = "\
# Onboarding findings @doc(team.onboarding-findings)

Findings from support, analytics, and research.

::observation onboarding.credit-confusion
status: verified
--
Users often misunderstand credit usage before their first generation.
::
";

#[test]
fn build_accepts_prd_example_and_emits_observation_into_graph_v4() {
    let workspace = TestWorkspace::new("observation-build");
    workspace.write("observation.adoc", PRD_EXAMPLE_OBSERVATION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "observation.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected the PRD §13.9 example to build\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    // Graph: kind observation with sample_size/observed_at preserved in the
    // hashed fields map and the inline source as evidence.
    let graph_text = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    assert_eq!(graph["schema_version"], "adoc.graph.v4");

    let observation = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "observation")
        .expect("graph contains an observation node");

    assert_eq!(observation["id"], "onboarding.credit-confusion");
    assert_eq!(observation["status"], "observed");
    assert_eq!(observation["fields"]["sample_size"], "37");
    assert_eq!(observation["fields"]["observed_at"], "2026-04-30");
    // ADR-0039: observation is born lifecycle-only — no severity/trust carriers.
    assert!(observation.get("severity").is_none());
    assert!(observation.get("trust").is_none());
    // The inline `source:` plugs into the V5 evidence model.
    assert_eq!(observation["evidence"][0]["value"], "support_tickets");

    // HTML: sample size and observed date as metadata chips.
    let html = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("html artifact is written");
    assert!(
        html.contains("<span class=\"observation__sample-size\">n=37</span>"),
        "html must contain the sample-size chip\n{html}"
    );
    assert!(
        html.contains("<span class=\"observation__observed-at\">2026-04-30</span>"),
        "html must contain the observed-at chip\n{html}"
    );
}

#[test]
fn check_rejects_negative_sample_size() {
    let workspace = TestWorkspace::new("observation-negative-sample-size");
    workspace.write("observation.adoc", NEGATIVE_SAMPLE_SIZE_OBSERVATION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "observation.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a negative sample_size"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.observation_invalid_sample_size]"),
        "expected invalid sample_size diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_dangling_evidence_ref() {
    let workspace = TestWorkspace::new("observation-dangling-evidence-ref");
    workspace.write("observation.adoc", DANGLING_EVIDENCE_REF_OBSERVATION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "observation.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for an evidence_ref to a missing source"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.evidence_target_not_found]"),
        "expected evidence_target_not_found diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_verified_status() {
    let workspace = TestWorkspace::new("observation-verified-status");
    workspace.write("observation.adoc", VERIFIED_STATUS_OBSERVATION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "observation.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for status `verified` — observations are only ever observed"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.observation_invalid_status]"),
        "expected invalid status diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

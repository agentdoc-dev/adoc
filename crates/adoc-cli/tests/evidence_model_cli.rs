mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

/// A `verified` claim that carries both an inline `test:` field AND an
/// `evidence_ref:` pointing at `billing.consume-use-case`.
///
/// Single-file workspace: both the source and the claim live in the same
/// `.adoc` file, which is valid — cross-object resolution works within
/// a single file as well as across files.
const VERIFIED_CLAIM_WITH_INLINE_AND_REF: &str = "\
# Evidence Model @doc(billing.evidence)

Verified claim backed by inline test evidence and an object-ref.

::source billing.consume-use-case
kind: source_code
path: apps/backend/src/features/credits/consume.use-case.ts
owner: backend-platform
--
Implementation of credit consumption.
::

::claim billing.credits.consume
status: verified
owner: backend-platform
verified_at: 2026-05-01
test: cargo test credits
evidence_ref: billing.consume-use-case
--
Credit consumption is handled by the use-case implementation.
::
";

/// An `accepted` decision that carries `evidence_ref: billing.consume-use-case`.
const ACCEPTED_DECISION_WITH_REF: &str = "\
# Evidence Model — Decision @doc(billing.decisions)

::source billing.consume-use-case
kind: source_code
path: apps/backend/src/features/credits/consume.use-case.ts
owner: backend-platform
--
Implementation of credit consumption.
::

::decision billing.credits.use-ledger
status: accepted
decided_by: architecture
evidence_ref: billing.consume-use-case
--
Use the ledger-first approach for credit consumption.
::
";

/// A claim that references an object that does not exist anywhere.
const CLAIM_WITH_MISSING_REF: &str = "\
# Evidence Model @doc(billing.missing-ref)

::claim billing.credits.orphan
status: plain
evidence_ref: missing.thing
--
This claim references a non-existent object.
::
";

/// A `plain` claim that references another **claim** (i.e. a non-source object).
const CLAIM_WITH_NON_SOURCE_REF: &str = "\
# Evidence Model @doc(billing.non-source-ref)

::claim billing.other-claim
status: plain
--
Another claim in the workspace.
::

::claim billing.credits.bad-ref
status: plain
evidence_ref: billing.other-claim
--
This claim references a claim, not a source.
::
";

// ---------------------------------------------------------------------------
// Test 1 — `adoc check` accepts a verified claim with inline evidence AND
// an `evidence_ref:` pointing at a valid source object.
// ---------------------------------------------------------------------------

#[test]
fn check_accepts_verified_claim_with_inline_and_evidence_ref() {
    let ws = TestWorkspace::new("ev-check-ok");
    ws.write("billing.adoc", VERIFIED_CLAIM_WITH_INLINE_AND_REF);

    let output = adoc_command()
        .current_dir(&ws.root)
        .args(["check", "billing.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

// ---------------------------------------------------------------------------
// Test 2 — `adoc build` records both evidence entries (inline + object-ref)
// in the typed `evidence` array on the claim graph node, and emits an
// `evidence` edge from the claim to the source.
// ---------------------------------------------------------------------------

#[test]
fn build_records_evidence_ref_in_typed_array_and_edge() {
    let ws = TestWorkspace::new("ev-build");
    ws.write("billing.adoc", VERIFIED_CLAIM_WITH_INLINE_AND_REF);

    let output = adoc_command()
        .current_dir(&ws.root)
        .args(["build", "billing.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let graph_text = fs::read_to_string(ws.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    // schema_version guard
    assert_eq!(
        graph["schema_version"], "adoc.graph.v5",
        "schema_version must be adoc.graph.v5"
    );

    // ---- find the claim node -----------------------------------------------
    let nodes = graph["nodes"].as_array().expect("nodes is an array");

    let claim_node = nodes
        .iter()
        .find(|n| n["kind"] == "claim" && n["id"] == "billing.credits.consume")
        .expect("claim node billing.credits.consume must be present in graph");

    let evidence = claim_node["evidence"]
        .as_array()
        .expect("claim node must have an `evidence` array");

    // ---- inline evidence entry (kind=test, value=...) ----------------------
    // Serialized shape: {"kind":"test","value":"cargo test credits"}
    let inline_entry = evidence
        .iter()
        .find(|ev| ev["kind"] == "test" && ev["value"] == "cargo test credits")
        .unwrap_or_else(|| {
            panic!(
                "inline evidence entry {{\"kind\":\"test\",\"value\":\"cargo test credits\"}} not found; got:\n{}",
                serde_json::to_string_pretty(evidence).unwrap()
            )
        });

    // sanity: inline entries must not carry a `reference` field
    assert!(
        inline_entry.get("reference").is_none() || inline_entry["reference"].is_null(),
        "inline evidence entry must not carry a `reference` field; got:\n{inline_entry}"
    );

    // ---- object-ref evidence entry (kind=source_code, reference=...) -------
    // Serialized shape: {"kind":"source_code","reference":"billing.consume-use-case"}
    // (value is absent / null for object-ref entries)
    let ref_entry = evidence
        .iter()
        .find(|ev| {
            ev["kind"] == "source_code"
                && ev["reference"] == "billing.consume-use-case"
        })
        .unwrap_or_else(|| {
            panic!(
                "object-ref evidence entry {{\"kind\":\"source_code\",\"reference\":\"billing.consume-use-case\"}} not found; got:\n{}",
                serde_json::to_string_pretty(evidence).unwrap()
            )
        });

    // sanity: object-ref entries must not carry a `value` field
    assert!(
        ref_entry.get("value").is_none() || ref_entry["value"].is_null(),
        "object-ref evidence entry must not carry a `value` field; got:\n{ref_entry}"
    );

    // ---- evidence edge in graph["edges"] -----------------------------------
    // Serialized edge shape: {"kind":"evidence","source":"billing.credits.consume","target":"billing.consume-use-case"}
    let edges = graph["edges"]
        .as_array()
        .expect("graph must have an `edges` array");

    let evidence_edge = edges
        .iter()
        .find(|e| {
            e["kind"] == "evidence"
                && e["source"] == "billing.credits.consume"
                && e["target"] == "billing.consume-use-case"
        })
        .unwrap_or_else(|| {
            panic!(
                "evidence edge {{\"kind\":\"evidence\",\"source\":\"billing.credits.consume\",\"target\":\"billing.consume-use-case\"}} not found; edges:\n{}",
                serde_json::to_string_pretty(edges).unwrap()
            )
        });

    // The edge must not carry a `relation` field (evidence edges are not
    // user relation edges).
    assert!(
        evidence_edge.get("relation").is_none() || evidence_edge["relation"].is_null(),
        "evidence edge must not carry a `relation` field; got:\n{evidence_edge}"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — `adoc check` rejects a claim with `evidence_ref:` pointing at an
// object that does not exist anywhere in the workspace.
// ---------------------------------------------------------------------------

#[test]
fn check_rejects_evidence_ref_to_missing_object() {
    let ws = TestWorkspace::new("ev-missing-ref");
    ws.write("billing.adoc", CLAIM_WITH_MISSING_REF);

    let output = adoc_command()
        .current_dir(&ws.root)
        .args(["check", "billing.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for evidence_ref to missing object"
    );

    // Diagnostics are emitted on stdout by print_diagnostics() → println!().
    // Format: `error[schema.evidence_target_not_found] ...`
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.evidence_target_not_found]"),
        "expected schema.evidence_target_not_found diagnostic\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

// ---------------------------------------------------------------------------
// Test 4 — `adoc check` rejects a claim with `evidence_ref:` pointing at
// another **claim** (i.e. a non-source object).
// ---------------------------------------------------------------------------

#[test]
fn check_rejects_evidence_ref_to_non_source_object() {
    let ws = TestWorkspace::new("ev-non-source-ref");
    ws.write("billing.adoc", CLAIM_WITH_NON_SOURCE_REF);

    let output = adoc_command()
        .current_dir(&ws.root)
        .args(["check", "billing.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for evidence_ref pointing at a claim"
    );

    // Diagnostics are emitted on stdout.
    // Format: `error[schema.evidence_target_not_a_source] ...`
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.evidence_target_not_a_source]"),
        "expected schema.evidence_target_not_a_source diagnostic\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

// ---------------------------------------------------------------------------
// Test 5 — `adoc check` accepts an `accepted` decision carrying
// `evidence_ref: billing.consume-use-case`.
// ---------------------------------------------------------------------------

#[test]
fn check_accepts_accepted_decision_with_evidence_ref() {
    let ws = TestWorkspace::new("ev-decision-ok");
    ws.write("decisions.adoc", ACCEPTED_DECISION_WITH_REF);

    let output = adoc_command()
        .current_dir(&ws.root)
        .args(["check", "decisions.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass for accepted decision with evidence_ref\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

// ---------------------------------------------------------------------------
// Test 5b — `adoc build` also emits an `evidence` edge for the decision.
// (Bonus assertion complementing Test 5.)
// ---------------------------------------------------------------------------

#[test]
fn build_records_evidence_edge_for_accepted_decision() {
    let ws = TestWorkspace::new("ev-decision-build");
    ws.write("decisions.adoc", ACCEPTED_DECISION_WITH_REF);

    let output = adoc_command()
        .current_dir(&ws.root)
        .args(["build", "decisions.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let graph_text = fs::read_to_string(ws.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    assert_eq!(graph["schema_version"], "adoc.graph.v5");

    let edges = graph["edges"]
        .as_array()
        .expect("graph must have an `edges` array");

    let evidence_edge = edges.iter().find(|e| {
        e["kind"] == "evidence"
            && e["source"] == "billing.credits.use-ledger"
            && e["target"] == "billing.consume-use-case"
    });

    assert!(
        evidence_edge.is_some(),
        "expected evidence edge decision→source; edges:\n{}",
        serde_json::to_string_pretty(edges).unwrap()
    );

    // The decision node must also carry a GraphEvidence entry.
    let nodes = graph["nodes"].as_array().expect("nodes is an array");
    let decision_node = nodes
        .iter()
        .find(|n| n["kind"] == "decision" && n["id"] == "billing.credits.use-ledger")
        .expect("decision node must be present");

    let decision_evidence = decision_node["evidence"]
        .as_array()
        .expect("decision node must have an `evidence` array");

    let ref_entry = decision_evidence
        .iter()
        .find(|ev| ev["kind"] == "source_code" && ev["reference"] == "billing.consume-use-case");

    assert!(
        ref_entry.is_some(),
        "decision `evidence` array must contain object-ref entry; got:\n{}",
        serde_json::to_string_pretty(decision_evidence).unwrap()
    );
}

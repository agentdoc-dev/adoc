mod support;

use adoc_core::{AgentJsonObject, CompileInput, DiagnosticCode, Severity, compile_workspace};
use support::TestWorkspace;

#[test]
fn compile_workspace_returns_mixed_validation_diagnostics_in_source_order() {
    let workspace = TestWorkspace::new("diagnostic-source-order");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(team.guide)\n\nsee [bad](javascript:alert) first\n\n<div>raw</div>\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "invalid source should fail compilation"
    );
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            DiagnosticCode::ParseUnsafeLink,
            DiagnosticCode::ParseRawHtml,
        ],
        "diagnostics should be ordered by source position"
    );
}

#[test]
fn compile_workspace_rejects_invalid_explicit_page_id() {
    let workspace = TestWorkspace::new("invalid-explicit-page-id");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(guide)\n\nSingle-segment page IDs are invalid.\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "invalid page ID should fail compilation"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, DiagnosticCode::IdInvalid);
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((1, 14)),
        "diagnostic should point at the invalid id value"
    );
}

#[test]
fn compile_workspace_rejects_invalid_path_derived_page_id() {
    let workspace = TestWorkspace::new("invalid-path-derived-page-id");
    let source = workspace.write(
        "guide.adoc",
        "# Guide\n\nA single file name derives a single-segment page ID.\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "invalid derived page ID should fail compilation"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, DiagnosticCode::IdInvalid);
    assert!(
        diagnostic.message.contains("guide"),
        "diagnostic should quote the invalid derived ID: {}",
        diagnostic.message
    );
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((1, 1)),
        "path-derived identity diagnostics should point at the file start"
    );
}

#[test]
fn compile_workspace_resolves_claim_into_artifact_record() {
    // A well-formed claim block must produce exactly one AgentJsonObject record
    // with the correct id, kind, status, body, page_id, source span, and empty
    // relation arrays.
    let workspace = TestWorkspace::new("resolve-claim-into-record");
    let source = workspace.write(
        "billing.adoc",
        concat!(
            "# Billing Guide @doc(team.billing)\n",
            "\n",
            "The billing system credits users automatically.\n",
            "\n",
            "::claim billing.credits\n",
            "status: draft\n",
            "owner: team-billing\n",
            "--\n",
            "The system credits users automatically when a payment fails.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected no errors, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    assert_eq!(
        artifacts.agent_json.objects.len(),
        1,
        "expected exactly one object record"
    );

    let record: &AgentJsonObject = &artifacts.agent_json.objects[0];
    assert_eq!(record.id, "billing.credits");
    assert_eq!(record.kind, "claim");
    assert_eq!(record.status, "draft");
    assert_eq!(
        record.body,
        "The system credits users automatically when a payment fails."
    );
    assert_eq!(record.page_id, "team.billing");
    assert!(
        record.relations.depends_on.is_empty(),
        "depends_on must be empty"
    );
    assert!(
        record.relations.supersedes.is_empty(),
        "supersedes must be empty"
    );
    assert!(
        record.relations.related_to.is_empty(),
        "related_to must be empty"
    );
    // The ::claim open-fence is on line 5 of the source.
    assert_eq!(
        record.source_span.line, 5,
        "source_span.line must point at the ::claim open-fence line"
    );
    assert_eq!(record.source_span.column, 1);
}

#[test]
fn compile_workspace_resolves_clean_decision_into_artifacts() {
    let workspace = TestWorkspace::new("resolve-clean-decision-into-artifacts");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status: proposed\n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected decision to compile, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let record = artifacts
        .agent_json
        .objects
        .first()
        .expect("decision object must be emitted");
    assert_eq!(record.id, "billing.policy");
    assert_eq!(record.kind, "decision");
    assert_eq!(record.status, "proposed");
    assert_eq!(record.body, "Use the existing billing policy.");
    assert_eq!(record.page_id, "team.decisions");
    assert!(record.fields.is_empty());
    assert!(record.relations.depends_on.is_empty());
    assert!(record.relations.supersedes.is_empty());
    assert!(record.relations.related_to.is_empty());
    assert_eq!(record.source_span.line, 3);
    assert_eq!(record.source_span.column, 1);
    assert!(
        artifacts
            .html
            .contains("<section class=\"decision\" id=\"billing.policy\">"),
        "decision section missing: {}",
        artifacts.html
    );
    assert!(
        artifacts
            .html
            .contains("<span class=\"decision__kind\">decision</span>"),
        "decision kind missing: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_resolves_warning_into_artifacts() {
    let workspace = TestWorkspace::new("resolve-warning-into-artifacts");
    let source = workspace.write(
        "warnings.adoc",
        concat!(
            "# Warning Guide @doc(team.warnings)\n",
            "\n",
            "::warning auth.session.clock-skew\n",
            "severity: high\n",
            "owner: platform\n",
            "--\n",
            "Session clocks can drift enough to reject otherwise valid tokens.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected warning to compile, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let record = artifacts
        .agent_json
        .objects
        .first()
        .expect("warning object must be emitted");
    assert_eq!(record.id, "auth.session.clock-skew");
    assert_eq!(record.kind, "warning");
    assert_eq!(record.status, "high");
    assert_eq!(
        record.body,
        "Session clocks can drift enough to reject otherwise valid tokens."
    );
    assert_eq!(record.page_id, "team.warnings");
    assert_eq!(
        record.fields.get("owner").map(String::as_str),
        Some("platform")
    );
    assert!(!record.fields.contains_key("severity"));
    assert!(record.relations.depends_on.is_empty());
    assert!(record.relations.supersedes.is_empty());
    assert!(record.relations.related_to.is_empty());
    assert_eq!(record.source_span.line, 3);
    assert_eq!(record.source_span.column, 1);
    assert!(
        artifacts
            .html
            .contains("<section class=\"warning\" id=\"auth.session.clock-skew\">"),
        "warning section missing: {}",
        artifacts.html
    );
    assert!(
        artifacts
            .html
            .contains("<span class=\"warning__kind\">warning</span>"),
        "warning kind missing: {}",
        artifacts.html
    );
    assert!(
        artifacts
            .html
            .contains("<span class=\"warning__severity\">high</span>"),
        "warning severity missing: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_rejects_warning_missing_severity() {
    let workspace = TestWorkspace::new("warning-missing-severity");
    let source = workspace.write(
        "warnings.adoc",
        concat!(
            "# Warning Guide @doc(team.warnings)\n",
            "\n",
            "::warning auth.session.clock-skew\n",
            "--\n",
            "Session clocks can drift.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "missing severity must be rejected");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaMissingField
    );
    assert!(result.diagnostics[0].message.contains("severity"));
    assert_eq!(
        result.diagnostics[0].object_id.as_deref(),
        Some("auth.session.clock-skew")
    );
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
    );
}

#[test]
fn compile_workspace_rejects_warning_missing_body() {
    let workspace = TestWorkspace::new("warning-missing-body");
    let source = workspace.write(
        "warnings.adoc",
        concat!(
            "# Warning Guide @doc(team.warnings)\n",
            "\n",
            "::warning auth.session.clock-skew\n",
            "severity: high\n",
            "--\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "missing body must be rejected");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaMissingField
    );
    assert!(result.diagnostics[0].message.contains("body"));
    assert_eq!(
        result.diagnostics[0].object_id.as_deref(),
        Some("auth.session.clock-skew")
    );
}

#[test]
fn compile_workspace_rejects_duplicate_decision_id_and_blocks_artifacts() {
    let workspace = TestWorkspace::new("decision-duplicate-id");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status: proposed\n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
            "\n",
            "::decision billing.policy\n",
            "status: proposed\n",
            "--\n",
            "Switch to a new billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "duplicate decision ID must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, DiagnosticCode::IdDuplicate);
    assert_eq!(
        result.diagnostics[0].object_id.as_deref(),
        Some("billing.policy")
    );
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((9, 1))
    );
    assert!(
        result.diagnostics[0]
            .message
            .contains("previously defined at")
    );
}

#[test]
fn compile_workspace_rejects_decision_missing_status() {
    let workspace = TestWorkspace::new("decision-missing-status");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "missing status must be rejected");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaMissingField
    );
    assert!(result.diagnostics[0].message.contains("status"));
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
    );
}

#[test]
fn compile_workspace_rejects_decision_empty_status() {
    let workspace = TestWorkspace::new("decision-empty-status");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status:   \n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "empty status must be rejected");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaMissingField
    );
    assert!(result.diagnostics[0].message.contains("status"));
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
    );
}

#[test]
fn compile_workspace_rejects_decision_missing_body() {
    let workspace = TestWorkspace::new("decision-missing-body");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status: proposed\n",
            "--\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "missing body must be rejected");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaMissingField
    );
    assert!(result.diagnostics[0].message.contains("body"));
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
    );
}

#[test]
fn compile_workspace_rejects_decision_invalid_status() {
    let workspace = TestWorkspace::new("decision-invalid-status");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status: Accepted\n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "mis-cased status must be rejected");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaInvalidStatus
    );
    assert_eq!(
        result.diagnostics[0].object_id.as_deref(),
        Some("billing.policy")
    );
    assert!(
        result.diagnostics[0]
            .help
            .as_deref()
            .is_some_and(|help| help.contains("proposed, accepted"))
    );
}

#[test]
fn compile_workspace_rejects_accepted_decision_missing_decided_by() {
    let workspace = TestWorkspace::new("decision-accepted-missing-decided-by");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status: accepted\n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "accepted decision must require decided_by"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaMissingField
    );
    assert!(result.diagnostics[0].message.contains("decided_by"));
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
    );
}

#[test]
fn compile_workspace_rejects_accepted_decision_empty_decided_by() {
    let workspace = TestWorkspace::new("decision-accepted-empty-decided-by");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status: accepted\n",
            "decided_by: \n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "accepted decision must require non-empty decided_by"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaMissingField
    );
    assert!(result.diagnostics[0].message.contains("decided_by"));
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
    );
}

#[test]
fn compile_workspace_emits_accepted_decision_with_verdict() {
    let workspace = TestWorkspace::new("decision-accepted-verdict");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decision Guide @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status: accepted\n",
            "decided_by: architecture\n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected accepted decision to compile, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let record = artifacts
        .agent_json
        .objects
        .first()
        .expect("decision object must be emitted");
    assert_eq!(record.kind, "decision");
    assert_eq!(record.status, "accepted");
    assert_eq!(
        record.fields.get("decided_by").map(String::as_str),
        Some("architecture")
    );
    assert!(
        artifacts
            .html
            .contains("<section class=\"decision decision--accepted\" id=\"billing.policy\">"),
        "accepted decision modifier missing: {}",
        artifacts.html
    );
    assert!(
        artifacts
            .html
            .contains("<div class=\"decision__verdict\"><dl><div class=\"decision__verdict-item\"><dt>decided_by</dt><dd>architecture</dd></div></dl></div>"),
        "decided_by verdict missing: {}",
        artifacts.html
    );
    assert!(
        !artifacts
            .html
            .contains("<footer class=\"decision__metadata\">\n<dl>\n<dt>decided_by</dt>"),
        "decided_by must not render as generic metadata: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_builds_verified_claim_with_all_v0_evidence() {
    let workspace = TestWorkspace::new("verified-claim-all-evidence");
    let source = workspace.write(
        "billing.adoc",
        concat!(
            "# Billing Guide @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "status: verified\n",
            "owner: team-billing\n",
            "verified_at: 2026-05-05\n",
            "source: billing-ledger\n",
            "test: cargo test billing_credits\n",
            "reviewed_by: qa-team\n",
            "--\n",
            "The system credits users automatically when a payment fails.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected verified claim to compile, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let record = artifacts
        .agent_json
        .objects
        .first()
        .expect("claim object must be emitted");
    assert_eq!(record.status, "verified");
    assert_eq!(
        record.fields.get("owner").map(String::as_str),
        Some("team-billing")
    );
    assert_eq!(
        record.fields.get("verified_at").map(String::as_str),
        Some("2026-05-05")
    );
    assert_eq!(
        record.fields.get("source").map(String::as_str),
        Some("billing-ledger")
    );
    assert_eq!(
        record.fields.get("test").map(String::as_str),
        Some("cargo test billing_credits")
    );
    assert_eq!(
        record.fields.get("reviewed_by").map(String::as_str),
        Some("qa-team")
    );
    assert!(
        artifacts.html.contains("claim claim--verified"),
        "verified claim class missing: {}",
        artifacts.html
    );
    assert!(
        artifacts.html.contains("claim__verification"),
        "verification section missing: {}",
        artifacts.html
    );
    assert!(
        artifacts
            .html
            .contains("claim__evidence-item claim__evidence-item--reviewed-by"),
        "reviewed_by evidence marker missing: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_reports_all_missing_verified_claim_requirements() {
    let workspace = TestWorkspace::new("verified-claim-missing-requirements");
    let source = workspace.write(
        "billing.adoc",
        concat!(
            "# Billing Guide @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "status: verified\n",
            "--\n",
            "The system credits users automatically when a payment fails.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "verified claim must be rejected");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            DiagnosticCode::SchemaMissingField,
            DiagnosticCode::SchemaMissingField,
            DiagnosticCode::ClaimVerifiedMissingEvidence,
        ]
    );
    for diagnostic in &result.diagnostics {
        assert_eq!(diagnostic.object_id.as_deref(), Some("billing.credits"));
        assert!(
            diagnostic
                .help
                .as_deref()
                .is_some_and(|help| help.contains("Verified claims require")),
            "verified diagnostic must explain the contract: {diagnostic:?}"
        );
    }
    assert!(result.diagnostics[0].message.contains("`owner`"));
    assert!(result.diagnostics[1].message.contains("`verified_at`"));
}

#[test]
fn compile_workspace_warns_and_treats_status_casing_variant_as_plain() {
    let workspace = TestWorkspace::new("verified-status-casing-warning");
    let source = workspace.write(
        "billing.adoc",
        concat!(
            "# Billing Guide @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "status: Verified\n",
            "--\n",
            "The system credits users automatically when a payment fails.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "casing variant should warn without verified-claim errors: {:?}",
        result.diagnostics
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::ClaimStatusCasing
    );
    assert_eq!(result.diagnostics[0].severity, Severity::Warning);
    assert_eq!(
        result.diagnostics[0].object_id.as_deref(),
        Some("billing.credits")
    );
    let artifacts = result.artifacts.expect("warnings must not block artifacts");
    assert_eq!(artifacts.agent_json.objects[0].status, "Verified");
    assert!(
        !artifacts.html.contains("claim--verified"),
        "casing variant must not render as verified: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_blocks_artifacts_for_invalid_claim() {
    // A claim that omits the required `status` field must produce exactly one
    // SchemaMissingField diagnostic, block artifact emission, and report no
    // artifacts in the result.
    let workspace = TestWorkspace::new("block-artifacts-invalid-claim");
    let source = workspace.write(
        "billing.adoc",
        concat!(
            "# Billing Guide @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "owner: team-billing\n",
            "--\n",
            "The system credits users automatically.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "missing status must produce at least one error"
    );
    assert!(
        result.artifacts.is_none(),
        "artifacts must be blocked when errors are present"
    );
    assert_eq!(
        result.diagnostics.len(),
        1,
        "expected exactly one diagnostic, got: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaMissingField,
        "diagnostic must carry SchemaMissingField code"
    );
    // Span points at the ::claim open-fence line (line 3 in this source).
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|s| (s.start.line, s.start.column)),
        Some((3, 1)),
        "span must point at the ::claim open-fence line"
    );
}

#[test]
fn compile_workspace_blocks_artifacts_for_raw_html_in_claim_body() {
    let workspace = TestWorkspace::new("block-artifacts-raw-html-claim-body");
    let source = workspace.write(
        "billing.adoc",
        concat!(
            "# Billing Guide @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "status: draft\n",
            "--\n",
            "Body <span>raw</span> text.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "raw HTML in claim body must produce an error"
    );
    assert!(
        result.artifacts.is_none(),
        "artifacts must be blocked when errors are present"
    );
    assert_eq!(
        result.diagnostics.len(),
        1,
        "expected exactly one diagnostic, got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.diagnostics[0].code, DiagnosticCode::ParseRawHtml);
    assert_eq!(
        result.diagnostics[0]
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((6, 6))
    );
}

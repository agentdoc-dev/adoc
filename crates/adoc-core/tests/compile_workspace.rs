mod support;

use adoc_core::{
    AgentJsonObject, BuildEmbeddingMode, BuildInput, CompileInput, DiagnosticCode, Severity,
    build_workspace, compile_workspace,
};
use support::TestWorkspace;

#[test]
fn build_workspace_skips_embeddings_without_affecting_check_path() {
    let workspace = TestWorkspace::new("build-workspace-skip-embeddings");
    let source = workspace.write(
        "billing.adoc",
        concat!(
            "# Billing Guide @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "status: draft\n",
            "--\n",
            "Credits apply after successful payment.\n",
            "::\n",
        ),
    );

    let check_result = compile_workspace(CompileInput {
        root: source.clone(),
    });
    assert!(
        !check_result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::BuildEmbeddingsSkipped),
        "check must not emit build-only embedding diagnostics"
    );

    let build_result = build_workspace(BuildInput {
        root: source,
        embeddings: BuildEmbeddingMode::Skipped,
        prior_search_artifact_path: None,
    });

    assert!(
        !build_result.has_errors(),
        "skipping embeddings should not fail build: {:?}",
        build_result.diagnostics
    );
    assert!(
        build_result
            .diagnostics
            .iter()
            .any(
                |diagnostic| diagnostic.code == DiagnosticCode::BuildEmbeddingsSkipped
                    && diagnostic.severity == Severity::Info
            ),
        "skipped embeddings should be reported as info"
    );
    let artifacts = build_result
        .artifacts
        .expect("build artifacts are produced");
    assert!(
        artifacts.search_json.is_none(),
        "skipping embeddings must omit the search artifact"
    );
}

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
fn compile_workspace_rejects_unknown_kind_without_field_parse_cascade() {
    let workspace = TestWorkspace::new("unknown-kind-freeform-single-shot");
    let source = workspace.write(
        "deferred.adoc",
        concat!(
            "# Deferred @doc(team.deferred)\n",
            "\n",
            "::fact billing.policy\n",
            "Future fact blocks may allow prose before a separator.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "unknown kind should fail compilation");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [DiagnosticCode::SchemaUnknownKind],
        "unsupported grammar-valid blocks should not leak field-shape diagnostics"
    );
}

#[test]
fn compile_workspace_keeps_universal_diagnostics_inside_unknown_kind() {
    let workspace = TestWorkspace::new("unknown-kind-keeps-universal-diagnostics");
    let source = workspace.write(
        "deferred.adoc",
        concat!(
            "# Deferred @doc(team.deferred)\n",
            "\n",
            "::fact billing.policy\n",
            "<div>Raw HTML is still a strict-mode source error.</div>\n",
            "::warning auth.session\n",
            "::\n",
        ),
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
    assert!(
        codes.contains(&DiagnosticCode::SchemaUnknownKind),
        "unknown kind diagnostic must remain: {codes:?}"
    );
    assert!(
        codes.contains(&DiagnosticCode::ParseRawHtml),
        "raw HTML diagnostic must remain: {codes:?}"
    );
    assert!(
        codes.contains(&DiagnosticCode::ParseNestedTypedBlock),
        "nested typed-block diagnostic must remain: {codes:?}"
    );
    assert!(
        !codes.contains(&DiagnosticCode::ParseMalformedField),
        "unknown-kind field-shape diagnostic must be suppressed: {codes:?}"
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
    assert_eq!(record.status.as_deref(), Some("draft"));
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
    assert_eq!(record.status.as_deref(), Some("proposed"));
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
    assert_eq!(record.status.as_deref(), Some("high"));
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
            .contains("<section class=\"warning warning--high\" id=\"auth.session.clock-skew\">"),
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
fn compile_workspace_resolves_glossary_into_artifacts() {
    let workspace = TestWorkspace::new("resolve-glossary-into-artifacts");
    let source = workspace.write(
        "glossary.adoc",
        concat!(
            "# Glossary @doc(team.glossary)\n",
            "\n",
            "::glossary billing.credits\n",
            "status: draft\n",
            "owner: team-billing\n",
            "--\n",
            "Credits are balance adjustments applied to an account.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected glossary to compile, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let record = artifacts
        .agent_json
        .objects
        .first()
        .expect("glossary object must be emitted");
    assert_eq!(record.id, "billing.credits");
    assert_eq!(record.kind, "glossary");
    assert_eq!(record.status, None);
    assert_eq!(
        record.body,
        "Credits are balance adjustments applied to an account."
    );
    assert_eq!(record.page_id, "team.glossary");
    assert_eq!(
        record.fields.get("status").map(String::as_str),
        Some("draft")
    );
    assert_eq!(
        record.fields.get("owner").map(String::as_str),
        Some("team-billing")
    );
    assert!(record.relations.depends_on.is_empty());
    assert!(record.relations.supersedes.is_empty());
    assert!(record.relations.related_to.is_empty());
    assert_eq!(record.source_span.line, 3);
    assert_eq!(record.source_span.column, 1);
    assert!(
        artifacts
            .html
            .contains("<section class=\"glossary\" id=\"billing.credits\">"),
        "glossary section missing: {}",
        artifacts.html
    );
    assert!(
        artifacts
            .html
            .contains("<span class=\"glossary__kind\">glossary</span>"),
        "glossary kind missing: {}",
        artifacts.html
    );
    assert!(
        artifacts
            .html
            .contains("<code class=\"glossary__id\">billing.credits</code>"),
        "glossary id missing: {}",
        artifacts.html
    );
    assert!(
        artifacts
            .html
            .contains("<footer class=\"glossary__metadata\">"),
        "glossary metadata footer missing: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_links_same_file_prose_reference_to_glossary() {
    let workspace = TestWorkspace::new("same-file-prose-ref-to-glossary");
    let source = workspace.write(
        "glossary.adoc",
        concat!(
            "# Glossary @doc(team.glossary)\n",
            "\n",
            "Credits are defined by [[billing.credits]].\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments applied to an account.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected prose reference to resolve, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    assert!(
        artifacts.html.contains(
            "Credits are defined by <a class=\"object-ref\" href=\"#billing.credits\">billing.credits</a>."
        ),
        "expected object reference anchor in HTML: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_links_cross_file_prose_reference_to_claim() {
    let workspace = TestWorkspace::new("cross-file-prose-ref-to-claim");
    workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "See *[[billing.credits]]* before changing balances.\n",
        ),
    );
    let source = workspace.write(
        "billing.adoc",
        concat!(
            "# Billing @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "status: draft\n",
            "--\n",
            "Credits adjust account balances.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput {
        root: source.parent().expect("source has parent").to_path_buf(),
    });

    assert!(
        !result.has_errors(),
        "expected cross-file prose reference to resolve, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    assert!(
        artifacts.html.contains(
            "<em><a class=\"object-ref\" href=\"#billing.credits\">billing.credits</a></em>"
        ),
        "expected object reference anchor inside emphasis: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_ignores_non_adoc_files_during_directory_scan() {
    let workspace = TestWorkspace::new("ignore-non-adoc-files");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(team.guide)\n\nCompiled source content.\n",
    );
    workspace.write(
        "notes.md",
        "# Notes\n\n<div>This raw HTML would fail if the file were compiled.</div>\n",
    );

    let result = compile_workspace(CompileInput {
        root: source.parent().expect("source has parent").to_path_buf(),
    });

    assert!(
        !result.has_errors(),
        "non-.adoc files must be ignored, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    assert_eq!(artifacts.agent_json.pages.len(), 1);
    assert!(
        artifacts.agent_json.pages[0]
            .source_path
            .ends_with("guide.adoc"),
        "expected only the .adoc source path, got: {}",
        artifacts.agent_json.pages[0].source_path
    );
    assert!(!artifacts.html.contains("This raw HTML would fail"));
}

#[test]
fn compile_workspace_rejects_single_file_with_non_adoc_extension() {
    let workspace = TestWorkspace::new("reject-single-md-file");
    let source = workspace.write(
        "notes.md",
        "# Notes\n\n<div>This must not compile as AgentDoc Source.</div>\n",
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "unsupported source extensions must fail compilation"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(
        diagnostic.code,
        DiagnosticCode::IoUnsupportedSourceExtension
    );
    assert!(
        diagnostic.message.contains(".adoc"),
        "diagnostic should tell callers the supported extension: {}",
        diagnostic.message
    );
}

#[test]
fn compile_workspace_reports_missing_root_as_unreadable_file() {
    let workspace = TestWorkspace::new("missing-root-is-unreadable");
    let anchor = workspace.write("anchor.adoc", "# Anchor\n");
    let source = anchor
        .parent()
        .expect("anchor has parent")
        .join("missing.md");

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "missing roots must fail compilation");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, DiagnosticCode::IoUnreadableFile);
}

#[test]
fn compile_workspace_links_references_in_heading_and_list_item() {
    let workspace = TestWorkspace::new("heading-list-prose-refs");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# See [[billing.credits]] @doc(team.guide)\n",
            "\n",
            "- Use **[[billing.credits]]** in balance docs.\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments applied to an account.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected heading and list references to resolve, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    assert!(
        artifacts.html.contains(
            "<h1>See <a class=\"object-ref\" href=\"#billing.credits\">billing.credits</a></h1>"
        ),
        "expected object reference anchor in heading: {}",
        artifacts.html
    );
    assert_eq!(
        artifacts.agent_json.pages[0].title.as_deref(),
        Some("See billing.credits")
    );
    assert!(
        artifacts.html.contains(
            "<li>Use <strong><a class=\"object-ref\" href=\"#billing.credits\">billing.credits</a></strong> in balance docs.</li>"
        ),
        "expected object reference anchor in list item: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_links_reference_inside_claim_body_and_preserves_source_body() {
    let workspace = TestWorkspace::new("claim-body-ref-to-glossary");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments applied to an account.\n",
            "::\n",
            "\n",
            "::claim billing.credit-policy\n",
            "status: draft\n",
            "--\n",
            "Use [[billing.credits]] before issuing refunds.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected claim body reference to resolve, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    assert!(
        artifacts.html.contains(
            "<div class=\"claim__body\"><p>Use <a class=\"object-ref\" href=\"#billing.credits\">billing.credits</a> before issuing refunds.</p></div>"
        ),
        "expected claim body reference anchor: {}",
        artifacts.html
    );
    let claim = artifacts
        .agent_json
        .objects
        .iter()
        .find(|object| object.id == "billing.credit-policy")
        .expect("claim object emitted");
    assert_eq!(
        claim.body,
        "Use [[billing.credits]] before issuing refunds."
    );
}

#[test]
fn compile_workspace_rejects_broken_reference_inside_decision_body() {
    let workspace = TestWorkspace::new("decision-body-broken-ref");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::decision billing.refunds\n",
            "status: proposed\n",
            "--\n",
            "Use [[missing.object]] before issuing refunds.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "broken body reference must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::RefBroken)
        .expect("ref.broken diagnostic must be emitted");
    assert_eq!(diagnostic.object_id.as_deref(), Some("missing.object"));
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((6, 5)),
        "diagnostic should point at the body reference"
    );
}

#[test]
fn compile_workspace_rejects_unsafe_link_inside_claim_body() {
    let workspace = TestWorkspace::new("claim-body-unsafe-link");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::claim billing.credit-policy\n",
            "status: draft\n",
            "--\n",
            "Do not use [bad](javascript:alert) links.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "unsafe body link must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::ParseUnsafeLink)
        .expect("parse.unsafe_link diagnostic must be emitted");
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((6, 12)),
        "diagnostic should point at the body link"
    );
}

#[test]
fn compile_workspace_emits_decision_supersedes_relation_in_html_and_agent_json() {
    let workspace = TestWorkspace::new("decision-supersedes-relation");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decisions @doc(team.decisions)\n",
            "\n",
            "::decision billing.old-policy\n",
            "status: accepted\n",
            "decided_by: architecture\n",
            "--\n",
            "Use the old billing policy.\n",
            "::\n",
            "\n",
            "::decision billing.new-policy\n",
            "status: accepted\n",
            "decided_by: architecture\n",
            "supersedes: billing.old-policy\n",
            "--\n",
            "Use the new billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected decision relation to resolve, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    assert!(
        artifacts.html.contains(
            "<section class=\"decision__relations\"><dl>\n<dt>supersedes</dt><dd><a class=\"object-ref\" href=\"#billing.old-policy\">billing.old-policy</a></dd>\n</dl></section>"
        ),
        "expected supersedes relation link in HTML: {}",
        artifacts.html
    );
    let decision = artifacts
        .agent_json
        .objects
        .iter()
        .find(|object| object.id == "billing.new-policy")
        .expect("new decision emitted");
    assert_eq!(decision.relations.supersedes, vec!["billing.old-policy"]);
    assert!(
        !decision.fields.contains_key("supersedes"),
        "relation field must be stripped from optional fields"
    );
}

#[test]
fn compile_workspace_emits_claim_depends_on_multiple_object_kinds() {
    let workspace = TestWorkspace::new("claim-depends-on-relation");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
            "\n",
            "::claim billing.ledger\n",
            "status: draft\n",
            "--\n",
            "Ledger records balance changes.\n",
            "::\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "depends_on: billing.credits, billing.ledger\n",
            "--\n",
            "Refunds depend on credits and ledger records.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected claim relations to resolve, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let claim = artifacts
        .agent_json
        .objects
        .iter()
        .find(|object| object.id == "billing.refunds")
        .expect("claim emitted");
    assert_eq!(
        claim.relations.depends_on,
        vec!["billing.credits", "billing.ledger"]
    );
    assert!(
        artifacts.html.contains(
            "<dt>depends_on</dt><dd><a class=\"object-ref\" href=\"#billing.credits\">billing.credits</a>, <a class=\"object-ref\" href=\"#billing.ledger\">billing.ledger</a></dd>"
        ),
        "expected depends_on relation links in HTML: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_emits_relation_bracket_array_in_first_occurrence_order() {
    let workspace = TestWorkspace::new("relation-bracket-array");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
            "\n",
            "::claim billing.ledger\n",
            "status: draft\n",
            "--\n",
            "Ledger records balance changes.\n",
            "::\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "depends_on: [billing.credits, billing.ledger, billing.credits]\n",
            "--\n",
            "Refunds depend on credits and ledger records.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected bracket-array relation targets to resolve, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let claim = artifacts
        .agent_json
        .objects
        .iter()
        .find(|object| object.id == "billing.refunds")
        .expect("claim emitted");
    assert_eq!(
        claim.relations.depends_on,
        vec!["billing.credits", "billing.ledger"]
    );
    assert!(
        artifacts.html.contains(
            "<dt>depends_on</dt><dd><a class=\"object-ref\" href=\"#billing.credits\">billing.credits</a>, <a class=\"object-ref\" href=\"#billing.ledger\">billing.ledger</a></dd>"
        ),
        "expected depends_on relation links in HTML: {}",
        artifacts.html
    );
}

#[test]
fn compile_workspace_deduplicates_relation_targets_and_ignores_trailing_comma() {
    let workspace = TestWorkspace::new("dedupe-relation-targets");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "related_to: billing.credits, billing.credits,\n",
            "--\n",
            "Refunds relate to credits.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected duplicate/trailing relation targets to compile, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let claim = artifacts
        .agent_json
        .objects
        .iter()
        .find(|object| object.id == "billing.refunds")
        .expect("claim emitted");
    assert_eq!(claim.relations.related_to, vec!["billing.credits"]);
    assert!(
        !claim.fields.contains_key("related_to"),
        "relation field must be stripped from optional fields"
    );
}

#[test]
fn relation_empty_segment_bracket_array_ignores_trailing_empty() {
    let workspace = TestWorkspace::new("relation-empty-segment-bracket-trailing");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "related_to: [billing.credits,]\n",
            "--\n",
            "Refunds relate to credits.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        !result.has_errors(),
        "expected bracket-array trailing empty relation segment to compile, got: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts must be produced");
    let claim = artifacts
        .agent_json
        .objects
        .iter()
        .find(|object| object.id == "billing.refunds")
        .expect("claim emitted");
    assert_eq!(claim.relations.related_to, vec!["billing.credits"]);
}

#[test]
fn relation_empty_segment_bracket_array_rejects_leading_empty() {
    let workspace = TestWorkspace::new("relation-empty-segment-bracket-leading");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "related_to: [, billing.credits]\n",
            "--\n",
            "Refunds relate to credits.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "leading empty relation segment must fail"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::IdInvalid)
        .expect("id.invalid diagnostic must be emitted");
    assert!(
        diagnostic.message.contains("empty relation"),
        "diagnostic should explain empty relation segment: {:?}",
        diagnostic
    );
}

#[test]
fn relation_empty_segment_bracket_array_rejects_interior_empty() {
    let workspace = TestWorkspace::new("relation-empty-segment-bracket-interior");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
            "\n",
            "::claim billing.ledger\n",
            "status: draft\n",
            "--\n",
            "Ledger records balance changes.\n",
            "::\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "related_to: [billing.credits,, billing.ledger]\n",
            "--\n",
            "Refunds relate to credits and ledger records.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(
        result.has_errors(),
        "interior empty relation segment must fail"
    );
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::IdInvalid)
        .expect("id.invalid diagnostic must be emitted");
    assert!(
        diagnostic.message.contains("empty relation"),
        "diagnostic should explain empty relation segment: {:?}",
        diagnostic
    );
}

#[test]
fn compile_workspace_rejects_broken_relation_target() {
    let workspace = TestWorkspace::new("broken-relation-target");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "depends_on: missing.object\n",
            "--\n",
            "Refunds depend on missing knowledge.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "broken relation must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::RefBroken)
        .expect("ref.broken diagnostic must be emitted");
    assert!(
        diagnostic
            .message
            .contains("depends_on target `missing.object`"),
        "diagnostic should name relation field: {:?}",
        diagnostic
    );
    assert_eq!(diagnostic.object_id.as_deref(), Some("missing.object"));
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((5, 13)),
        "diagnostic should point at the missing relation ID"
    );
}

#[test]
fn relation_cascade_decision_invalid_status_suppresses_relation_diagnostics() {
    let workspace = TestWorkspace::new("relation-cascade-decision-invalid-status");
    let source = workspace.write(
        "decisions.adoc",
        concat!(
            "# Decisions @doc(team.decisions)\n",
            "\n",
            "::decision billing.policy\n",
            "status: Accepted\n",
            "depends_on: missing.object, BadTarget\n",
            "--\n",
            "Use the existing billing policy.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "invalid decision must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [DiagnosticCode::SchemaInvalidStatus],
        "invalid decision basics should suppress relation diagnostics: {:?}",
        result.diagnostics
    );
}

#[test]
fn relation_cascade_warning_missing_severity_suppresses_relation_diagnostics() {
    let workspace = TestWorkspace::new("relation-cascade-warning-missing-severity");
    let source = workspace.write(
        "warnings.adoc",
        concat!(
            "# Warnings @doc(team.warnings)\n",
            "\n",
            "::warning auth.session.clock-skew\n",
            "depends_on: missing.object\n",
            "--\n",
            "Session clocks can drift.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "invalid warning must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [DiagnosticCode::SchemaMissingField],
        "missing warning severity should suppress relation diagnostics: {:?}",
        result.diagnostics
    );
    assert!(result.diagnostics[0].message.contains("severity"));
}

#[test]
fn relation_cascade_warning_invalid_severity_suppresses_relation_diagnostics() {
    let workspace = TestWorkspace::new("relation-cascade-warning-invalid-severity");
    let source = workspace.write(
        "warnings.adoc",
        concat!(
            "# Warnings @doc(team.warnings)\n",
            "\n",
            "::warning auth.session.clock-skew\n",
            "severity: Critical\n",
            "depends_on: BadTarget\n",
            "--\n",
            "Session clocks can drift.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "invalid warning must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [DiagnosticCode::SchemaInvalidStatus],
        "invalid warning severity should suppress relation diagnostics: {:?}",
        result.diagnostics
    );
}

#[test]
fn relation_cascade_claim_missing_status_suppresses_relation_diagnostics() {
    let workspace = TestWorkspace::new("relation-cascade-claim-missing-status");
    let source = workspace.write(
        "claims.adoc",
        concat!(
            "# Claims @doc(team.claims)\n",
            "\n",
            "::claim billing.credits\n",
            "depends_on: missing.object\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "invalid claim must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [DiagnosticCode::SchemaMissingField],
        "missing claim status should suppress relation diagnostics: {:?}",
        result.diagnostics
    );
    assert!(result.diagnostics[0].message.contains("status"));
}

#[test]
fn relation_cascade_claim_invalid_id_suppresses_relation_id_diagnostic() {
    let workspace = TestWorkspace::new("relation-cascade-claim-invalid-id");
    let source = workspace.write(
        "claims.adoc",
        concat!(
            "# Claims @doc(team.claims)\n",
            "\n",
            "::claim BillingCredits\n",
            "status: draft\n",
            "depends_on: BadTarget\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "invalid claim must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(
        result.diagnostics.len(),
        1,
        "invalid claim id should suppress relation id diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.diagnostics[0].code, DiagnosticCode::IdInvalid);
    assert_eq!(
        result.diagnostics[0].object_id.as_deref(),
        Some("BillingCredits")
    );
}

#[test]
fn compile_workspace_rejects_empty_interior_relation_segment() {
    let workspace = TestWorkspace::new("empty-interior-relation-segment");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "depends_on: billing.credits, , billing.ledger\n",
            "--\n",
            "Refunds depend on credits and ledger records.\n",
            "::\n",
            "\n",
            "::glossary billing.credits\n",
            "--\n",
            "Credits are balance adjustments.\n",
            "::\n",
            "\n",
            "::claim billing.ledger\n",
            "status: draft\n",
            "--\n",
            "Ledger records balance changes.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "empty relation segment must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::IdInvalid)
        .expect("id.invalid diagnostic must be emitted");
    assert!(
        diagnostic.message.contains("empty relation"),
        "diagnostic should explain empty relation segment: {:?}",
        diagnostic
    );
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((5, 29)),
        "diagnostic should point at the empty interior segment"
    );
}

#[test]
fn compile_workspace_rejects_glossary_invalid_id() {
    let workspace = TestWorkspace::new("glossary-invalid-id");
    let source = workspace.write(
        "glossary.adoc",
        concat!(
            "# Glossary @doc(team.glossary)\n",
            "\n",
            "::glossary BillingCredits\n",
            "--\n",
            "Credits are balance adjustments applied to an account.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "invalid glossary id must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::IdInvalid)
        .expect("id.invalid diagnostic must be emitted");
    assert_eq!(diagnostic.object_id.as_deref(), Some("BillingCredits"));
    assert!(
        diagnostic
            .message
            .contains("invalid glossary id `BillingCredits`"),
        "diagnostic should name rejected glossary id: {:?}",
        diagnostic
    );
    assert!(
        diagnostic
            .help
            .as_deref()
            .is_some_and(|help| help.contains("Object IDs must")),
        "diagnostic should carry object id grammar help: {:?}",
        diagnostic
    );
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
    );
}

#[test]
fn compile_workspace_rejects_glossary_missing_body() {
    let workspace = TestWorkspace::new("glossary-missing-body");
    let source = workspace.write(
        "glossary.adoc",
        concat!(
            "# Glossary @doc(team.glossary)\n",
            "\n",
            "::glossary billing.credits\n",
            "status: draft\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "missing glossary body must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::SchemaMissingField)
        .expect("schema.missing_field diagnostic must be emitted");
    assert_eq!(diagnostic.object_id.as_deref(), Some("billing.credits"));
    assert!(
        diagnostic.message.contains("body"),
        "diagnostic should mention body: {:?}",
        diagnostic
    );
    assert!(
        diagnostic
            .help
            .as_deref()
            .is_some_and(|help| help.contains("non-empty body")),
        "diagnostic should include fix-oriented help: {:?}",
        diagnostic
    );
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
    );
}

#[test]
fn compile_workspace_rejects_glossary_duplicate_field_key_and_drops_block() {
    let workspace = TestWorkspace::new("glossary-duplicate-field");
    let source = workspace.write(
        "glossary.adoc",
        concat!(
            "# Glossary @doc(team.glossary)\n",
            "\n",
            "::glossary billing.credits\n",
            "status: draft\n",
            "status: reviewed\n",
            "--\n",
            "Credits are balance adjustments applied to an account.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "duplicate glossary field must fail");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::SchemaDuplicateField)
        .expect("schema.duplicate_field diagnostic must be emitted");
    assert!(
        diagnostic
            .message
            .contains("duplicate field `status` in glossary"),
        "diagnostic should name duplicate glossary field: {:?}",
        diagnostic
    );
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start.line, span.start.column)),
        Some((3, 1))
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
fn compile_workspace_rejects_warning_invalid_severity() {
    let workspace = TestWorkspace::new("warning-invalid-severity");
    let source = workspace.write(
        "warnings.adoc",
        concat!(
            "# Warning Guide @doc(team.warnings)\n",
            "\n",
            "::warning auth.session.clock-skew\n",
            "severity: Critical\n",
            "--\n",
            "Session clocks can drift.\n",
            "::\n",
        ),
    );

    let result = compile_workspace(CompileInput { root: source });

    assert!(result.has_errors(), "invalid severity must be rejected");
    assert!(result.artifacts.is_none(), "errors must block artifacts");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SchemaInvalidStatus
    );
    assert!(result.diagnostics[0].message.contains("Critical"));
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
    assert_eq!(record.status.as_deref(), Some("accepted"));
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
    assert_eq!(record.status.as_deref(), Some("verified"));
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
    assert_eq!(
        artifacts.agent_json.objects[0].status.as_deref(),
        Some("Verified")
    );
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

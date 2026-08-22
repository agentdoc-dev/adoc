//! Docs-truth guard (E0.3): `docs/roadmap/v10/CONTRACT-REGISTRY.md` is the
//! single canonical inventory of externally observable wire surfaces — no
//! envelope schema-version id or Diagnostic Code may be emitted by `adoc`
//! source without a registry row, and no shipped registry row may outlive
//! the code that emitted it. The parse targets pinned HTML comment anchors
//! and backticked first table cells, never free prose.

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const REGISTRY: &str = "docs/roadmap/v10/CONTRACT-REGISTRY.md";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_repo_doc(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

/// The registry document, refusing a tree where a fence never closes —
/// the span from the opener to EOF would be invisible to every check here.
fn registry() -> String {
    let content = read_repo_doc(REGISTRY);
    if let Some(opener) = support::doc_scan::unclosed_fence(&content) {
        panic!("{REGISTRY}:{opener}: fence never closes — every check below is blind past it");
    }
    content
}

/// Backticked ids from the first cell of the table rows between
/// `<!-- {anchor} -->` and `<!-- /{anchor} -->`. Between the anchors only a
/// header row, a separator row, a blank line, or a `| `id` |…` data row is
/// legal — anything else fails loudly rather than silently dropping a row.
fn anchored_ids(doc: &str, anchor: &str) -> BTreeSet<String> {
    let open = format!("<!-- {anchor} -->");
    let close = format!("<!-- /{anchor} -->");
    let start = doc
        .find(&open)
        .unwrap_or_else(|| panic!("{REGISTRY} is missing the `{open}` anchor"))
        + open.len();
    let end = doc[start..]
        .find(&close)
        .unwrap_or_else(|| panic!("{REGISTRY} is missing the closing `{close}` anchor"))
        + start;
    // A duplicated anchor block (the classic bad merge-conflict resolution)
    // would make every row in the later block invisible to the scan.
    assert!(
        doc[start..].find(&open).is_none(),
        "{REGISTRY}: `{open}` appears more than once — rows in later blocks are invisible to the scan"
    );

    let mut ids = BTreeSet::new();
    let mut saw_header = false;
    for line in doc[start..end].lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        let Some(row) = line.strip_prefix('|') else {
            panic!("{REGISTRY}: `{anchor}` block contains a non-table line: {line:?}");
        };
        let first_cell = row.split('|').next().unwrap_or_default().trim();
        // An empty first cell must fall through and fail loudly below —
        // `all()` is vacuously true on "", which would silently drop the row.
        if !first_cell.is_empty() && first_cell.chars().all(|c| c == '-' || c == ':') {
            continue; // separator row
        }
        if let Some(id) = first_cell
            .strip_prefix('`')
            .and_then(|rest| rest.strip_suffix('`'))
        {
            if !ids.insert(id.to_string()) {
                panic!("{REGISTRY}: `{anchor}` registers {id:?} twice");
            }
        } else if saw_header {
            panic!("{REGISTRY}: `{anchor}` data row's first cell is not a backticked id: {line:?}");
        } else {
            saw_header = true; // exactly one unbackticked header row is legal
        }
    }
    ids
}

/// Every id registered anywhere in the registry, across all anchored tables.
fn all_registered_ids(doc: &str) -> BTreeSet<String> {
    ANCHORS
        .iter()
        .flat_map(|anchor| anchored_ids(doc, anchor))
        .collect()
}

/// Every anchored id table in the registry. A new table means a new entry
/// here — `all_anchors_present` fails until the two lists agree.
const ANCHORS: &[&str] = &[
    "registry:envelopes-shipped-adoc",
    "registry:envelopes-shipped-action",
    "registry:envelopes-historical",
    "registry:test-fixture-ids",
    "registry:envelopes-planned",
    "registry:diagnostic-codes",
    "registry:action-codes",
    "registry:gate-codes",
    "registry:cloud-codes",
    "registry:attestation-codes",
    "registry:dispositions",
    "registry:untrusted-change-states",
    "registry:retention-classes",
    "registry:replay-postures",
];

/// True for `adoc.<dotted lowercase path>.v<digits>` — the envelope
/// schema-version id shape and nothing else.
fn is_envelope_id(candidate: &str) -> bool {
    let Some(rest) = candidate.strip_prefix("adoc.") else {
        return false;
    };
    let Some((path, version)) = rest.rsplit_once(".v") else {
        return false;
    };
    !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_' || c == '.')
        && !version.is_empty()
        && version.chars().all(|c| c.is_ascii_digit())
}

/// Envelope ids appearing as complete double-quoted string literals in one
/// source text. `//` comment lines are skipped (mirroring
/// `diagnostic_codes_in`) so an id surviving only in a comment cannot keep
/// a stale shipped row alive. Quote pairing restarts on every line, so a
/// stray `'"'` char literal or an escaped quote earlier in the FILE can
/// never desync the scan and hide a later id — desync is bounded to the one
/// line carrying it. Whole files are scanned, `#[cfg(test)]` modules
/// included: fixture ids there are registered rows (the test-fixture
/// table), so a new unregistered literal fails loudly wherever it sits.
// ponytail: ids built with format!/concat are outside any textual net —
// the workspace convention is whole-literal schema-version consts.
fn envelope_ids_in(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(|line| line.split('"').skip(1).step_by(2))
        .filter(|chunk| is_envelope_id(chunk))
        .map(str::to_string)
        .collect()
}

fn rust_sources_under(dir: &Path, sources: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to list {}: {error}", dir.display()))
            .path();
        if path.is_dir() {
            rust_sources_under(&path, sources);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            sources.push(path);
        }
    }
}

/// Every envelope id appearing in `crates/*/src`, test modules included.
// ponytail: the walk covers crates/<name>/src only — a build.rs or a nested
// crate would sit outside it; neither exists in this workspace.
fn emitted_envelope_ids() -> BTreeSet<String> {
    let crates_dir = repo_root().join("crates");
    let mut sources = Vec::new();
    let entries = fs::read_dir(&crates_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", crates_dir.display()));
    for entry in entries {
        let src = entry
            .unwrap_or_else(|error| panic!("failed to list {}: {error}", crates_dir.display()))
            .path()
            .join("src");
        if src.is_dir() {
            rust_sources_under(&src, &mut sources);
        }
    }
    assert!(
        !sources.is_empty(),
        "no Rust sources found under crates/*/src — the scan would pass vacuously"
    );
    sources
        .iter()
        .flat_map(|path| {
            let content = fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            envelope_ids_in(&content)
        })
        .collect()
}

/// `Variant = "wire.string" =>` rows from one source text. Comment lines
/// are stripped first (the macro's doc comment shows a placeholder row),
/// then the text is flattened so a rustfmt-wrapped row still parses.
fn diagnostic_codes_in(content: &str) -> BTreeSet<String> {
    let flattened = content
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ");
    let mut codes = BTreeSet::new();
    let mut rest = flattened.as_str();
    while let Some(start) = rest.find("= \"") {
        rest = &rest[start + 3..];
        let Some(end) = rest.find('"') else { break };
        let code = &rest[..end];
        rest = &rest[end + 1..];
        if rest.trim_start().starts_with("=>") && code.contains('.') {
            codes.insert(code.to_string());
        }
    }
    codes
}

/// Every wire code declared in the `diagnostic_codes!` table — one
/// `Variant = "wire.string" =>` row per code, the single source of truth
/// the macro expands from.
fn diagnostic_code_table() -> BTreeSet<String> {
    let content = read_repo_doc("crates/adoc-core/src/domain/diagnostic.rs");
    let codes = diagnostic_codes_in(&content);
    assert!(
        codes.len() > 100,
        "diagnostic_codes! parse found only {} rows — the row pattern drifted",
        codes.len()
    );
    codes
}

#[test]
fn all_anchors_present() {
    let registry = registry();
    for anchor in ANCHORS {
        anchored_ids(&registry, anchor); // panics on a missing anchor
    }
}

#[test]
fn shipped_adoc_envelope_rows_match_the_source_scan() {
    let registry = registry();
    let registered = anchored_ids(&registry, "registry:envelopes-shipped-adoc");
    let fixtures = anchored_ids(&registry, "registry:test-fixture-ids");
    let emitted: BTreeSet<String> = emitted_envelope_ids()
        .difference(&fixtures)
        .cloned()
        .collect();

    let unregistered: Vec<_> = emitted.difference(&registered).collect();
    assert!(
        unregistered.is_empty(),
        "envelope ids emitted by crates/*/src without a shipped registry row \
         in {REGISTRY}: {unregistered:?}"
    );
    let stale: Vec<_> = registered.difference(&emitted).collect();
    assert!(
        stale.is_empty(),
        "shipped adoc envelope rows in {REGISTRY} no longer appear in any \
         crates/*/src string literal (test modules included): {stale:?}"
    );
}

#[test]
fn historical_envelope_rows_are_no_longer_emitted() {
    let registry = registry();
    let historical = anchored_ids(&registry, "registry:envelopes-historical");
    assert!(
        !historical.is_empty(),
        "{REGISTRY}: the historical table lost its rows — adoc.retrieval.v0 \
         is a retained documented version"
    );
    let fixtures = anchored_ids(&registry, "registry:test-fixture-ids");
    let emitted: BTreeSet<String> = emitted_envelope_ids()
        .difference(&fixtures)
        .cloned()
        .collect();
    let resurrected: Vec<_> = historical.intersection(&emitted).collect();
    assert!(
        resurrected.is_empty(),
        "historical envelope rows appear in crates/*/src again: {resurrected:?} — \
         if the envelope genuinely ships again, register a new version; if a \
         back-compat test cites the id, add it to the test-fixture table"
    );
}

#[test]
fn action_owned_envelope_rows_are_pinned() {
    let registry = registry();
    let registered = anchored_ids(&registry, "registry:envelopes-shipped-action");
    let expected: BTreeSet<String> = ["adoc.pr_assessment_receipt.v0", "adoc.semantic_review.v0"]
        .iter()
        .map(|id| id.to_string())
        .collect();
    assert_eq!(
        registered, expected,
        "Action-owned shipped envelopes (ADR-0051/ADR-0052) drifted in {REGISTRY}"
    );
}

#[test]
fn diagnostic_code_rows_match_the_code_table() {
    let registry = registry();
    let registered = anchored_ids(&registry, "registry:diagnostic-codes");
    let declared = diagnostic_code_table();

    let unregistered: Vec<_> = declared.difference(&registered).collect();
    assert!(
        unregistered.is_empty(),
        "Diagnostic Codes declared in diagnostic.rs without a registry row \
         in {REGISTRY}: {unregistered:?}"
    );
    let stale: Vec<_> = registered.difference(&declared).collect();
    assert!(
        stale.is_empty(),
        "Diagnostic Code rows in {REGISTRY} with no declaring code table row: {stale:?}"
    );
}

#[test]
fn scan_flags_an_unregistered_wire_code_fixture() {
    let fixture = r#"
        pub const ROGUE_SCHEMA_VERSION: &str = "adoc.unregistered.v0";
    "#;
    let emitted = envelope_ids_in(fixture);
    let registered = all_registered_ids(&registry());
    let unregistered: Vec<_> = emitted.difference(&registered).collect();
    assert_eq!(
        unregistered,
        ["adoc.unregistered.v0"],
        "the completeness scan must fail on a fixture emitting one unregistered wire code"
    );
}

#[test]
fn fixture_ids_are_registered_and_disjoint_from_real_rows() {
    let registry = registry();
    let fixtures = anchored_ids(&registry, "registry:test-fixture-ids");
    assert!(
        fixtures.contains("adoc.search.v99"),
        "the rejected-version fixture id lost its registry row"
    );
    // Historical ids are excluded from the collision set: a back-compat test
    // in crates/*/src citing e.g. adoc.retrieval.v0 registers the id in the
    // fixture table, which must not deadlock against the historical row.
    let real: BTreeSet<String> = ANCHORS
        .iter()
        .filter(|anchor| {
            !matches!(
                **anchor,
                "registry:test-fixture-ids" | "registry:envelopes-historical"
            )
        })
        .flat_map(|anchor| anchored_ids(&registry, anchor))
        .collect();
    let colliding: Vec<_> = fixtures.intersection(&real).collect();
    assert!(
        colliding.is_empty(),
        "a test-fixture id may never collide with a shipped/planned/code row \
         (historical ids are the deliberate exception for back-compat tests): {colliding:?}"
    );
}

#[test]
fn quote_desync_stays_bounded_to_its_line() {
    let fixture = r#"
        const QUOTE: char = '"';
        pub const REAL: &str = "adoc.diff.v0";
    "#;
    assert!(
        envelope_ids_in(fixture).contains("adoc.diff.v0"),
        "a char-literal quote on an earlier line must not hide later ids"
    );
}

#[test]
fn wrapped_diagnostic_row_still_parses() {
    let wrapped = "SomeCode =\n    \"assessment.wrapped_row\" =>\n    \"help\";";
    assert!(
        diagnostic_codes_in(wrapped).contains("assessment.wrapped_row"),
        "a rustfmt-wrapped diagnostic_codes! row must still scan"
    );
    let commented = r#"/// carrying `(Variant = "wire.string" => "default help";)`"#;
    assert!(
        diagnostic_codes_in(commented).is_empty(),
        "the macro doc-comment placeholder must never scan as a code"
    );
}

#[test]
fn envelope_id_shape_rejects_prose_and_paths() {
    for not_an_id in [
        "adoc.graph",          // no version suffix
        "adoc.graph.v5.json",  // file name, version not terminal
        "adoc.Graph.v5",       // uppercase
        "docs/adoc.graph.v5",  // path prefix
        "prose adoc.graph.v5", // embedded in prose
        "adoc..v5",            // empty path
        "adoc.graph.v",        // empty version
    ] {
        assert!(
            !is_envelope_id(not_an_id),
            "{not_an_id:?} must not scan as an envelope id"
        );
    }
    assert!(is_envelope_id("adoc.graph.v5"));
    assert!(is_envelope_id("adoc.mcp.command.v0"));
}

#[test]
fn guard_fires_on_a_malformed_registry_row() {
    let broken = "\
<!-- registry:broken -->\n\
| id | status |\n\
| --- | --- |\n\
| adoc.graph.v5 | shipped |\n\
<!-- /registry:broken -->\n";
    let result = std::panic::catch_unwind(|| anchored_ids(broken, "registry:broken"));
    assert!(
        result.is_err(),
        "a data row whose first cell is not backticked must fail loudly, never drop silently"
    );
}

#[test]
fn guard_fires_on_a_duplicated_anchor_block() {
    let duplicated = "\
<!-- registry:dup -->\n\
| id | status |\n\
| --- | --- |\n\
| `adoc.graph.v5` | shipped |\n\
<!-- /registry:dup -->\n\
<!-- registry:dup -->\n\
| `adoc.diff.v0` | shipped |\n\
<!-- /registry:dup -->\n";
    let result = std::panic::catch_unwind(|| anchored_ids(duplicated, "registry:dup"));
    assert!(
        result.is_err(),
        "a duplicated anchor block must fail loudly — later rows are invisible to the scan"
    );
}

#[test]
fn guard_fires_on_an_empty_first_cell() {
    let broken = "\
<!-- registry:empty -->\n\
| id | status |\n\
| --- | --- |\n\
|  | shipped |\n\
<!-- /registry:empty -->\n";
    let result = std::panic::catch_unwind(|| anchored_ids(broken, "registry:empty"));
    assert!(
        result.is_err(),
        "a data row with an empty first cell must fail loudly, never scan as a separator"
    );
}

#[test]
fn comment_lines_do_not_scan_as_emissions() {
    let commented = r#"
        // wire id: "adoc.diff.v0"
    "#;
    assert!(
        envelope_ids_in(commented).is_empty(),
        "an id surviving only in a line comment must not keep a stale shipped row alive"
    );
}

/// The execution map's E0.3 must-include list, resolved to registered ids.
/// One entry per item in the map's sentence, in its order; the milestone's
/// T2 planned set adds the remaining reserved names.
const MUST_INCLUDE_IDS: &[&str] = &[
    "adoc.semantic_context.v0",       // semantic context
    "adoc.semantic_assessment.v0",    // semantic assessment
    "adoc.validation_receipt.v0",     // validation receipt
    "adoc.lifecycle_mapping.v0",      // lifecycle mapping
    "adoc.source_record.v0",          // source record
    "adoc.source_assertion.v0",       // source assertion
    "adoc.source_acl_snapshot.v0",    // ACL snapshot
    "adoc.source_binding.v0",         // source binding
    "adoc.sensitive_access.v0",       // sensitive-access event (RT-08, RT-21)
    "adoc.egress_policy.v0",          // egress policy (RT-21)
    "adoc.authorization_decision.v0", // authorization decision
    "adoc.work_request.v0",           // work request
    "adoc.work_result.v0",            // work result
    "adoc.migration_request.v0",      // migration request
    "adoc.migration_receipt.v0",      // migration receipt
    "adoc.connector_manifest.v0",     // connector capability manifest
    "adoc.governance_event.v0",       // governance contract
    "adoc.proposal.v0",               // proposal contract
    "adoc.approval.v0",               // approval contract
    "adoc.gate_result.v0",            // gate contract
];

#[test]
fn must_include_contracts_are_registered() {
    let registered = all_registered_ids(&registry());
    let missing: Vec<_> = MUST_INCLUDE_IDS
        .iter()
        .filter(|id| !registered.contains(**id))
        .collect();
    assert!(
        missing.is_empty(),
        "EXECUTION-MAP §E0.3 must-include items without a registry row: {missing:?}"
    );
}

#[test]
fn planned_rows_name_exactly_one_owner_repo() {
    let registry = registry();
    let open = "<!-- registry:envelopes-planned -->";
    let close = "<!-- /registry:envelopes-planned -->";
    let start = registry.find(open).expect("planned anchor") + open.len();
    let end = registry[start..].find(close).expect("planned close anchor") + start;
    let mut data_rows = 0;
    for line in registry[start..end].lines().map(str::trim) {
        if !line.starts_with("| `") {
            continue; // header/separator; malformed rows already fail anchored_ids
        }
        data_rows += 1;
        let owner = line.split('|').nth(2).map(str::trim).unwrap_or_default();
        assert!(
            matches!(owner, "adoc" | "action" | "cloud"),
            "planned row must name exactly one owner repo (adoc|action|cloud): {line:?}"
        );
    }
    assert!(data_rows > 0, "the planned table lost its rows");
}

/// Backticked codes cited by the executable planning surface, one
/// `(document, line, code)` per citation. Covers `gate.*`/`action.*` codes
/// and envelope ids; `attestation.*` siblings are deliberately outside the
/// net — E8.1.T1 registers them as a registry edit in that slice.
fn cited_codes() -> Vec<(String, usize, String)> {
    let dir = repo_root().join("docs/roadmap/v10");
    let mut cited = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "no v10 planning documents found");
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        if let Some(opener) = support::doc_scan::unclosed_fence(&content) {
            panic!("{name}:{opener}: fence never closes — citations past it are invisible");
        }
        for (number, line) in support::doc_scan::structural_lines(&content) {
            for span in line.split('`').skip(1).step_by(2) {
                let is_reason_code = ["gate.", "action."].iter().any(|prefix| {
                    span.strip_prefix(prefix).is_some_and(|rest| {
                        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                    })
                });
                if is_reason_code || is_envelope_id(span) {
                    cited.push((name.clone(), number, span.to_string()));
                }
            }
        }
    }
    cited
}

#[test]
fn cited_codes_resolve_to_registered_entries_only() {
    let registered = all_registered_ids(&registry());
    let unresolved: Vec<_> = cited_codes()
        .into_iter()
        .filter(|(_, _, code)| !registered.contains(code))
        .map(|(name, number, code)| format!("{name}:{number}: `{code}`"))
        .collect();
    assert!(
        unresolved.is_empty(),
        "planning surfaces cite wire codes with no registry row (dispositions count as rows): \n{}",
        unresolved.join("\n")
    );
}

#[test]
fn semantic_failed_has_exactly_one_disposition() {
    let registry = registry();
    let dispositions = anchored_ids(&registry, "registry:dispositions");
    assert!(
        dispositions.contains("action.semantic_failed"),
        "action.semantic_failed lost its disposition row (RT-21: one canonical resolution)"
    );
    let row = registry
        .lines()
        .find(|line| line.starts_with("| `action.semantic_failed`"))
        .expect("disposition row");
    assert!(
        row.contains("`action.semantic_review_failed`"),
        "the disposition must name the surviving canonical code: {row:?}"
    );
    let action_codes = anchored_ids(&registry, "registry:action-codes");
    assert!(
        action_codes.contains("action.semantic_review_failed"),
        "the surviving code must itself be a registered Action code"
    );
    assert!(
        !action_codes.contains("action.semantic_failed"),
        "a removed code may never also appear as a registered Action code"
    );
}

#[test]
fn bot_attestation_family_has_one_documented_wrapper_mapping() {
    let registry = registry();
    let attestation = anchored_ids(&registry, "registry:attestation-codes");
    assert!(
        attestation.contains("attestation.bot_approver_rejected"),
        "the canonical bot-attestation family root must stay registered (RT-21)"
    );
    let wrapper_row = registry
        .lines()
        .find(|line| line.starts_with("| `action.attestation_bot_rejected`"))
        .expect("the Action wrapper code row (E8.1.T3) is registered");
    assert!(
        wrapper_row.contains("`attestation.bot_approver_rejected`"),
        "the Action wrapper row must document its mapping to the canonical code: {wrapper_row:?}"
    );
}

/// One closed vocabulary pinned exactly: registry rows equal the annex
/// list, no additions and no losses (sets — document order is not checked).
fn assert_vocabulary(anchor: &str, annex: &str, expected: &[&str]) {
    let registered = anchored_ids(&registry(), anchor);
    let expected_set: BTreeSet<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        registered, expected_set,
        "`{anchor}` drifted from the closed vocabulary in {annex} — \
         a change there is a registry edit plus an annex amendment"
    );
}

#[test]
fn untrusted_change_states_match_s8() {
    assert_vocabulary(
        "registry:untrusted-change-states",
        "SEMANTICS.md §S8",
        &[
            "not_required",
            "awaiting_authorization",
            "authorized",
            "running",
            "completed",
            "denied",
            "failed",
            "expired_after_head_change",
        ],
    );
}

#[test]
fn retention_classes_match_k9() {
    assert_vocabulary(
        "registry:retention-classes",
        "KNOWLEDGE-MODEL.md §K9",
        &[
            "digest_only",
            "bounded_evidence",
            "exact_candidate_input",
            "temporary_processing",
            "full_source_snapshot",
        ],
    );
}

#[test]
fn replay_postures_match_k9() {
    assert_vocabulary(
        "registry:replay-postures",
        "KNOWLEDGE-MODEL.md §K9",
        &[
            "fully_replayable",
            "source_access_required",
            "intentionally_non_replayable",
            "no_longer_replayable_after_deletion",
        ],
    );
}

//! Docs-truth guard (E0.4): each row of `docs/roadmap/v10/COMPATIBILITY.md`
//! names a contract owner, min–max tested producer-consumer versions and an
//! owning release train consistent with its own repos cell, atop the verified
//! 2026-08-13 baseline, and no v10 annex reverts to the superseded Action
//! alpha.18 baseline or uses a historical Cloud phase label as a release
//! gate. That the table has exactly one row per multi-repo execution-map
//! slice is checked in agentdoc-dev/cloud (`scripts/roadmap-authority.py`),
//! where the execution map lives, against the commit Cloud pins as
//! `ADOC_REGISTRY_REF` (ADR-0070). The parse targets table cells and pinned
//! HTML comment anchors, never free prose.

use crate::support;

use std::collections::BTreeMap;
use std::fs;
use std::ops::RangeInclusive;
use std::path::PathBuf;

const COMPATIBILITY: &str = "docs/roadmap/v10/COMPATIBILITY.md";

/// The cross-repo delivery order: the owning release train of a slice is
/// the LAST involved repository in this order (Cloud last; web claims
/// update only after the release they describe).
const DELIVERY_ORDER: &[&str] = &["adoc", "action", "cloud", "web"];

/// Rows in the `compat:slice-rows` block today.
const ROW_FLOOR: usize = 47;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_repo_doc(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn compatibility() -> String {
    let content = read_repo_doc(COMPATIBILITY);
    if let Some(opener) = support::doc_scan::unclosed_fence(&content) {
        panic!("{COMPATIBILITY}:{opener}: fence never closes — every check below is blind past it");
    }
    content
}

/// Byte offsets of the span between `<!-- {anchor} -->` and
/// `<!-- /{anchor} -->` — the one source for both the block text and its
/// position in the document.
fn anchored_span(doc: &str, doc_name: &str, anchor: &str) -> std::ops::Range<usize> {
    let open = format!("<!-- {anchor} -->");
    let close = format!("<!-- /{anchor} -->");
    let start = doc
        .find(&open)
        .unwrap_or_else(|| panic!("{doc_name} is missing the `{open}` anchor"))
        + open.len();
    let end = doc[start..]
        .find(&close)
        .unwrap_or_else(|| panic!("{doc_name} is missing the closing `{close}` anchor"))
        + start;
    assert!(
        doc[start..].find(&open).is_none(),
        "{doc_name}: `{open}` appears more than once — lines in later blocks are invisible"
    );
    assert!(
        doc[end + close.len()..].find(&close).is_none(),
        "{doc_name}: `{close}` appears more than once — lines between the closes are invisible"
    );
    start..end
}

/// The block between `<!-- {anchor} -->` and `<!-- /{anchor} -->`.
fn anchored_block<'doc>(doc: &'doc str, doc_name: &str, anchor: &str) -> &'doc str {
    &doc[anchored_span(doc, doc_name, anchor)]
}

/// Repositories a row's repos cell names, in delivery order.
fn repos_named(field: &str) -> Vec<&'static str> {
    let tokens: Vec<&str> = field.split(',').map(str::trim).collect();
    DELIVERY_ORDER
        .iter()
        .copied()
        .filter(|name| tokens.contains(name))
        .collect()
}

struct CompatRow {
    repos: String,
    owner: String,
    versions: String,
    train: String,
}

/// `slice id → row` from the anchored compatibility table.
fn compat_rows(table_block: &str) -> BTreeMap<String, CompatRow> {
    let mut rows = BTreeMap::new();
    for line in table_block.lines().map(str::trim) {
        if !line.starts_with("| `") {
            continue; // header and separator rows
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        assert!(
            cells.len() == 5,
            "{COMPATIBILITY}: a compatibility row needs exactly five cells: {line:?}"
        );
        let id = cells[0]
            .strip_prefix('`')
            .and_then(|rest| rest.strip_suffix('`'))
            .unwrap_or_else(|| {
                panic!("{COMPATIBILITY}: row's slice cell is not backticked: {line:?}")
            });
        let previous = rows.insert(
            id.to_string(),
            CompatRow {
                repos: cells[1].to_string(),
                owner: cells[2].to_string(),
                versions: cells[3].to_string(),
                train: cells[4].to_string(),
            },
        );
        assert!(
            previous.is_none(),
            "{COMPATIBILITY}: slice {id} has two compatibility rows"
        );
    }
    rows
}

/// The row-per-slice half runs where the execution map lives; the annex must
/// keep naming that guard, and ADR-0070 the hand-over and its residual.
#[test]
fn the_cloud_handover_is_named_in_the_annex_and_the_adr() {
    let compatibility = compatibility();
    assert!(
        compatibility.contains("`scripts/roadmap-authority.py` in `agentdoc-dev/cloud`"),
        "{COMPATIBILITY}: the Guard line must name the Cloud check that owns the row-per-slice rule"
    );
    let adr = read_repo_doc("docs/adr/0070-single-product-roadmap-in-cloud.md");
    assert!(
        compatibility.contains("0070-single-product-roadmap-in-cloud.md")
            && adr.contains("Accepted residual: pin lag")
            && adr.contains("`scripts/roadmap-authority.py`")
            && adr.contains("`boundary_authority_guard`"),
        "{COMPATIBILITY} must cite ADR-0070, which must name the Cloud check, every guard \
         that handed rules over and the pin-lag residual"
    );
}

#[test]
fn rows_name_owner_versions_and_owning_train() {
    let compatibility = compatibility();
    let rows = compat_rows(anchored_block(
        &compatibility,
        COMPATIBILITY,
        "compat:slice-rows",
    ));
    // Which rows are owed is checked in Cloud (ADR-0070); the floor makes a
    // local deletion loud at once. Lower it deliberately with any removal.
    assert!(
        rows.len() >= ROW_FLOOR,
        "only {} rows parsed from {COMPATIBILITY} (expected at least {ROW_FLOOR}) — a row \
         was deleted or the parse drifted; lower the pin deliberately with the removal",
        rows.len()
    );
    for (id, row) in &rows {
        // Every name in the cell must be one the delivery order knows: an
        // unknown party must fail, not vanish from the derived repo list.
        for token in row.repos.split(',').map(str::trim) {
            assert!(
                DELIVERY_ORDER.contains(&token),
                "{id}: repos cell names {token:?}, which the delivery order does not \
                 know — an unknown party must fail, not vanish from the comparison"
            );
        }
        let repos = repos_named(&row.repos);
        // The table is cross-repo only: a single-repo row would satisfy the
        // owner and train checks below vacuously.
        assert!(
            repos.len() >= 2,
            "{id}: repos cell names {repos:?} — a compatibility row needs at least two repositories"
        );
        assert!(
            repos.contains(&row.owner.as_str()),
            "{id}: contract owner {:?} is not an involved repository {repos:?}",
            row.owner
        );
        assert_eq!(
            row.train,
            *repos.last().unwrap(),
            "{id}: owning release train must be the last involved repository \
             in the delivery order (Cloud last, web after)"
        );
        let segments: Vec<&str> = row.versions.split('·').map(str::trim).collect();
        for repo in DELIVERY_ORDER {
            let display = match *repo {
                "action" => "Action",
                "cloud" => "Cloud",
                other => other,
            };
            let segment = segments.iter().find(|part| part.starts_with(display));
            if repos.contains(repo) {
                // The segment must carry a version or status beyond the bare
                // name — `Cloud` alone is not a tested baseline.
                let segment = segment.unwrap_or_else(|| {
                    panic!(
                        "{id}: versions cell names nothing for {repo}: {:?}",
                        row.versions
                    )
                });
                assert!(
                    segment.len() > display.len(),
                    "{id}: versions cell names {repo} with no tested version or status: \
                     {segment:?}"
                );
            } else {
                assert!(
                    segment.is_none(),
                    "{id}: versions cell names {repo}, which the repos cell does not involve: {:?}",
                    row.versions
                );
            }
        }
    }
}

#[test]
fn baseline_and_delivery_order_are_pinned() {
    let compatibility = compatibility();
    for required in [
        "`adoc` 0.3.4 / Graph Artifact v5",
        "Action `v2.0.0-alpha.19`",
        "private Next.js/Supabase workspace scaffold",
        "`adoc` tag → checksum-verified binaries → Action pin → immutable Action release → floating tag after smoke → Cloud last",
    ] {
        assert!(
            compatibility.contains(required),
            "{COMPATIBILITY} lost the verified baseline element: {required:?}"
        );
    }
}

/// True when a structural line reverts to the superseded Action baseline:
/// it cites alpha.18 without the alpha.19 correction alongside.
fn reverts_to_alpha18(line: &str) -> bool {
    line.contains("alpha.18") && !line.contains("alpha.19")
}

#[test]
fn no_planning_surface_reverts_to_alpha18() {
    let mut violations = Vec::new();
    for (name, content) in v10_documents() {
        for (number, line) in support::doc_scan::structural_lines(&content) {
            if reverts_to_alpha18(line) {
                violations.push(format!("{name}:{number}: {}", line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "executable planning surfaces cite the superseded alpha.18 baseline \
         without the RT-22 correction:\n{}",
        violations.join("\n")
    );
}

#[test]
fn seeded_alpha18_reversion_fires() {
    assert!(reverts_to_alpha18(
        "Planning baseline for the Action is `v2.0.0-alpha.18`."
    ));
    assert!(!reverts_to_alpha18(
        "Planning baseline is `v2.0.0-alpha.19`, not alpha.18."
    ));
}

/// True when a structural line mentions a historical Cloud phase label
/// (`Phase 0`, `Cloud 0.<n>`).
// ponytail: lexical guard — any line carrying "histor" passes, so a
// deliberately misleading "historically-motivated gate" sentence would slip
// through; upgrade to sentence-level parsing only if that ever happens.
fn mentions_phase_label(line: &str) -> bool {
    let lower = line.to_lowercase();
    let phase = lower.match_indices("phase 0").any(|(index, marker)| {
        !lower
            .as_bytes()
            .get(index + marker.len())
            .is_some_and(|byte| byte.is_ascii_digit())
    });
    let cloud = lower.match_indices("cloud 0.").any(|(index, marker)| {
        lower
            .as_bytes()
            .get(index + marker.len())
            .is_some_and(|byte| byte.is_ascii_digit())
    });
    phase || cloud
}

/// 1-based line numbers spanned by the `compat:phase-map` block, anchor
/// lines included — the exemption is POSITIONAL, so a verbatim copy of a
/// phase-map row pasted elsewhere in the document is still scanned.
fn phase_map_lines(doc: &str) -> RangeInclusive<usize> {
    let span = anchored_span(doc, COMPATIBILITY, "compat:phase-map");
    let line_of = |offset: usize| doc[..offset].matches('\n').count() + 1;
    line_of(span.start)..=line_of(span.end)
}

/// Structural lines that mention a historical phase label without
/// historical context, outside the exempt line range.
fn phase_label_violations(
    name: &str,
    content: &str,
    exempt_lines: RangeInclusive<usize>,
) -> Vec<String> {
    support::doc_scan::structural_lines(content)
        .filter(|(number, line)| {
            !exempt_lines.contains(number)
                && mentions_phase_label(line)
                && !line.to_lowercase().contains("histor")
        })
        .map(|(number, line)| format!("{name}:{number}: {}", line.trim()))
        .collect()
}

#[test]
fn phase_labels_stay_historical_context_only() {
    let compatibility = compatibility();
    let map_lines = phase_map_lines(&compatibility);
    let mut violations = Vec::new();
    for (name, content) in v10_documents() {
        let exempt = if name.ends_with("COMPATIBILITY.md") {
            map_lines.clone()
        } else {
            0..=0 // line numbers are 1-based, so this exempts nothing
        };
        violations.extend(phase_label_violations(&name, &content, exempt));
    }
    assert!(
        violations.is_empty(),
        "historical Cloud phase labels used without historical context \
         (RT-02: never a release gate):\n{}",
        violations.join("\n")
    );
}

#[test]
fn a_phase_map_row_pasted_outside_the_block_fires() {
    let compatibility = compatibility();
    let row = anchored_block(&compatibility, COMPATIBILITY, "compat:phase-map")
        .lines()
        .find(|line| line.trim_start().starts_with("| `Cloud"))
        .expect("phase map has a `Cloud 0.<n>` row");
    let doctored = format!("{compatibility}\n{row}");
    let violations = phase_label_violations(COMPATIBILITY, &doctored, phase_map_lines(&doctored));
    assert!(
        violations.iter().any(|v| v.contains(row.trim())),
        "a verbatim phase-map row outside the anchored block must be a violation"
    );
}

#[test]
fn seeded_phase_label_gate_fires() {
    assert!(mentions_phase_label(
        "Release gates on Cloud 0.3 completion."
    ));
    assert!(mentions_phase_label("Phase 0 must pass before GA."));
    assert!(!mentions_phase_label("Cloud 0-day work"));
}

#[test]
fn phase_map_covers_all_eight_labels() {
    let compatibility = compatibility();
    let block = anchored_block(&compatibility, COMPATIBILITY, "compat:phase-map");
    for label in [
        "`Phase 0`",
        "`Cloud 0.1`",
        "`Cloud 0.2`",
        "`Cloud 0.3`",
        "`Cloud 0.4`",
        "`Cloud 0.5`",
        "`Cloud 0.6`",
        "`Cloud 0.7`",
    ] {
        assert!(
            block.contains(label),
            "{COMPATIBILITY}: phase map lost the {label} mapping row"
        );
    }
}

#[test]
fn true_up_records_shipped_not_shipped_and_the_o01_allocation() {
    let compatibility = compatibility();
    let block = anchored_block(&compatibility, COMPATIBILITY, "compat:true-up");
    for required in [
        "exact-SHA assessment",
        "creator/owner-only RLS",
        "NOT shipped at the baseline",
        "graph v6",
        "canonical Knowledge Object tables",
        "O-01",
        "E4.6 slice start",
        "V10.1.6",
    ] {
        assert!(
            block.contains(required),
            "{COMPATIBILITY}: baseline true-up lost: {required:?}"
        );
    }
}

/// Every `docs/roadmap/v10` annex, `(file name, content)`.
fn v10_documents() -> Vec<(String, String)> {
    let dir = repo_root().join("docs/roadmap/v10");
    let mut documents: Vec<(String, String)> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            if let Some(opener) = support::doc_scan::unclosed_fence(&content) {
                panic!("{name}:{opener}: fence never closes — lines past it are invisible");
            }
            (name, content)
        })
        .collect();
    documents.sort();
    assert!(!documents.is_empty(), "no v10 planning documents found");
    documents
}

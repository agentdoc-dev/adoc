//! Docs-truth guard (E0.4): `docs/roadmap/v10/COMPATIBILITY.md` carries one
//! row per multi-repo execution-map slice — contract owner, min–max tested
//! producer-consumer versions, owning release train — atop the verified
//! 2026-08-13 baseline, and no executable planning surface reverts to the
//! superseded Action alpha.18 baseline or uses a historical Cloud phase
//! label as a release gate. The parse targets slice headings, bold field
//! labels, and pinned HTML comment anchors, never free prose.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const COMPATIBILITY: &str = "docs/roadmap/v10/COMPATIBILITY.md";
const EXECUTION_MAP: &str = "docs/roadmap/v10/EXECUTION-MAP.md";

/// The cross-repo delivery order: the owning release train of a slice is
/// the LAST involved repository in this order (Cloud last; web claims
/// update only after the release they describe).
const DELIVERY_ORDER: &[&str] = &["adoc", "action", "cloud", "web"];

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

/// The block between `<!-- {anchor} -->` and `<!-- /{anchor} -->`.
fn anchored_block<'doc>(doc: &'doc str, doc_name: &str, anchor: &str) -> &'doc str {
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
    &doc[start..end]
}

/// Repositories a slice's `**Repos:**` field names, in delivery order.
/// `all` means every implementation repository (plus `web` when named).
fn repos_named(field: &str) -> Vec<&'static str> {
    let lower = field.to_lowercase();
    let mentions = |name: &str| {
        lower.match_indices(name).any(|(index, _)| {
            let before_ok = index == 0 || !lower.as_bytes()[index - 1].is_ascii_alphanumeric();
            let after = lower.as_bytes().get(index + name.len());
            before_ok && !after.is_some_and(|byte| byte.is_ascii_alphanumeric())
        })
    };
    if mentions("all") {
        let mut named = vec!["adoc", "action", "cloud"];
        if mentions("web") {
            named.push("web");
        }
        return named;
    }
    DELIVERY_ORDER
        .iter()
        .copied()
        .filter(|name| mentions(name))
        .collect()
}

/// `slice id → repos` for every execution-map slice involving two or more
/// repositories — the set the compatibility table must cover exactly.
// ponytail: first **Repos:** line per slice wins and only `## E<n>.<n>`
// headings are slices — the map's uniform structure; a decoy earlier prose
// line or a three-part slice id would need parser work only if the map
// ever adopts them.
fn multi_repo_slices(map: &str) -> BTreeMap<String, Vec<&'static str>> {
    if let Some(opener) = support::doc_scan::unclosed_fence(map) {
        panic!("{EXECUTION_MAP}:{opener}: fence never closes — slices past it are invisible");
    }
    let mut slices = BTreeMap::new();
    let mut total = 0usize;
    let mut current: Option<String> = None;
    let close_slice = |current: &mut Option<String>| {
        if let Some(open_id) = current.take() {
            panic!(
                "{EXECUTION_MAP}: slice {open_id} has no parseable **Repos:** field — \
                 a format drift here would silently exempt the slice from the table"
            );
        }
    };
    for (_, line) in support::doc_scan::structural_lines(map) {
        if let Some(rest) = line.strip_prefix("## ") {
            close_slice(&mut current);
            let id = rest.split_whitespace().next().unwrap_or_default();
            let is_slice = id
                .strip_prefix('E')
                .is_some_and(|numbers| numbers.split('.').count() == 2)
                && id[1..].chars().all(|c| c.is_ascii_digit() || c == '.');
            if is_slice {
                total += 1;
                current = Some(id.to_string());
            }
        } else if let (Some(id), Some((_, field))) =
            (current.as_ref(), line.split_once("**Repos:**"))
        {
            // Every repository the field NAMES (backticked) must be one the
            // delivery order knows — an unrecognised name would otherwise be
            // silently dropped and its party never appear in the table
            // (E8.5's future component repository is the live case).
            for token in field.split('`').skip(1).step_by(2) {
                assert!(
                    DELIVERY_ORDER.contains(&token),
                    "{EXECUTION_MAP}: slice {id} names repository {token:?}, which the \
                     delivery order does not know — add it to DELIVERY_ORDER or the \
                     slice silently ships with an uncovered party"
                );
            }
            let repos = repos_named(field);
            assert!(
                !repos.is_empty(),
                "{EXECUTION_MAP}: slice {id} names no known repository in its \
                 **Repos:** field: {field:?}"
            );
            if repos.len() >= 2 {
                slices.insert(id.clone(), repos);
            }
            current = None; // first Repos line per slice wins
        }
    }
    close_slice(&mut current);
    assert!(
        total > 60 && slices.len() > 20,
        "only {total} slices / {} multi-repo parsed from {EXECUTION_MAP} — the parse drifted",
        slices.len()
    );
    slices
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

/// The uncovered/stale differences between the map's multi-repo slices and
/// the table rows — empty exactly when the table is row-complete.
fn row_completeness_mismatches(
    slices: &BTreeMap<String, Vec<&'static str>>,
    rows: &BTreeMap<String, CompatRow>,
) -> Vec<String> {
    let slice_ids: BTreeSet<&String> = slices.keys().collect();
    let row_ids: BTreeSet<&String> = rows.keys().collect();
    slice_ids
        .difference(&row_ids)
        .map(|id| format!("multi-repo slice {id} has no compatibility row"))
        .chain(
            row_ids
                .difference(&slice_ids)
                .map(|id| format!("row {id} does not match a multi-repo slice in the map")),
        )
        .collect()
}

#[test]
fn every_multi_repo_slice_has_exactly_one_row() {
    let slices = multi_repo_slices(&read_repo_doc(EXECUTION_MAP));
    let compatibility = compatibility();
    let rows = compat_rows(anchored_block(
        &compatibility,
        COMPATIBILITY,
        "compat:slice-rows",
    ));
    let mismatches = row_completeness_mismatches(&slices, &rows);
    assert!(
        mismatches.is_empty(),
        "compatibility table is not row-complete against {EXECUTION_MAP}:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn rows_name_owner_versions_and_owning_train() {
    let slices = multi_repo_slices(&read_repo_doc(EXECUTION_MAP));
    let compatibility = compatibility();
    let rows = compat_rows(anchored_block(
        &compatibility,
        COMPATIBILITY,
        "compat:slice-rows",
    ));
    for (id, row) in &rows {
        let Some(repos) = slices.get(id) else {
            continue; // stale rows are `every_multi_repo_slice_has_exactly_one_row`'s report
        };
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
        // Exact in both directions: an extra repo in the cell is the same
        // silent divergence from the map as a missing one.
        assert_eq!(
            repos_named(&row.repos),
            *repos,
            "{id}: repos cell does not mirror the map's involved repositories: {:?}",
            row.repos
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
                    "{id}: versions cell names {repo}, which the map does not involve: {:?}",
                    row.versions
                );
            }
        }
    }
}

#[test]
fn deleting_a_row_fires_the_lint() {
    let slices = multi_repo_slices(&read_repo_doc(EXECUTION_MAP));
    let compatibility = compatibility();
    let block = anchored_block(&compatibility, COMPATIBILITY, "compat:slice-rows");
    let doctored: String = block
        .lines()
        .filter(|line| !line.starts_with("| `E0.3`"))
        .collect::<Vec<_>>()
        .join("\n");
    let mismatches = row_completeness_mismatches(&slices, &compat_rows(&doctored));
    assert!(
        mismatches.iter().any(|m| m.contains("E0.3")),
        "removing the E0.3 row must be reported as a missing row"
    );
}

#[test]
fn a_stale_row_fires_the_lint() {
    let slices = multi_repo_slices(&read_repo_doc(EXECUTION_MAP));
    let compatibility = compatibility();
    let block = anchored_block(&compatibility, COMPATIBILITY, "compat:slice-rows");
    // E0.1 is a single-repo slice, so a row for it is stale by construction.
    let doctored =
        format!("{block}\n| `E0.1` | adoc, cloud | adoc | adoc 0.3.4 · Cloud scaffold | cloud |");
    let mismatches = row_completeness_mismatches(&slices, &compat_rows(&doctored));
    assert!(
        mismatches.iter().any(|m| m.contains("E0.1")),
        "a row for a single-repo slice must be reported as stale"
    );
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

#[test]
fn phase_labels_stay_historical_context_only() {
    let compatibility = compatibility();
    let map_block = anchored_block(&compatibility, COMPATIBILITY, "compat:phase-map");
    let mut violations = Vec::new();
    for (name, content) in v10_documents() {
        for (number, line) in support::doc_scan::structural_lines(&content) {
            let inside_phase_map = name.ends_with("COMPATIBILITY.md") && map_block.contains(line);
            if mentions_phase_label(line)
                && !inside_phase_map
                && !line.to_lowercase().contains("histor")
            {
                violations.push(format!("{name}:{number}: {}", line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "historical Cloud phase labels used without historical context \
         (RT-02: never a release gate):\n{}",
        violations.join("\n")
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

/// Every executable v10 planning document, `(file name, content)`.
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

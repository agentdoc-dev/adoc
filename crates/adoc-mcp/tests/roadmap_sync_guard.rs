//! Docs-truth guard (ADR-0041): `docs/roadmap/v10/MILESTONES.md` duplicates
//! each E-slice's `**Repos:**` / `**Depends on:**` values from
//! `docs/roadmap/v10/EXECUTION-MAP.md` so engineers can schedule from the
//! hand-off layer; this guard asserts the copies never drift. The parse
//! targets stable structure only — slice headings and the bold field labels —
//! never the overview table or free prose.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn read_repo_doc(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

/// Collapses runs of whitespace so hard-wrap/trailing-space differences
/// between the two documents never count as drift.
fn normalize(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Drops every line inside a ``` fence via the shared docs-truth scanner —
/// fenced content must neither terminate a slice body (`#!/…`) nor
/// contribute phantom headings, labels, or table rows.
fn structural_lines(doc: &str) -> Vec<&str> {
    support::doc_scan::structural_lines(doc)
        .map(|(_, line)| line)
        .collect()
}

/// Returns the `E<major>.<minor>` slice ID opening a heading, if any.
fn e_slice_id(heading_rest: &str) -> Option<&str> {
    let id = heading_rest.split_whitespace().next()?;
    let (major, minor) = id.strip_prefix('E')?.split_once('.')?;
    let all_digits = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
    (all_digits(major) && all_digits(minor)).then_some(id)
}

struct SliceFields {
    repos: String,
    depends_on: String,
}

/// Parses every E-slice section opened by `heading_prefix`, reading the bold
/// `**Repos:**` / `**Depends on:**` labels — either as two separate lines
/// (execution map) or joined on one line with a middle-dot separator
/// (MILESTONES). The first labelled occurrence in a slice body wins — a stray
/// later label (e.g. a quoted rejected variant) never overwrites the header.
/// Fails loudly on duplicate IDs or a slice missing a field.
fn slice_fields(doc: &str, doc_name: &str, heading_prefix: &str) -> BTreeMap<String, SliceFields> {
    let lines = structural_lines(doc);
    let mut slices = BTreeMap::new();
    let mut i = 0;
    while i < lines.len() {
        let id = lines[i]
            .strip_prefix(heading_prefix)
            .and_then(e_slice_id)
            .map(str::to_string);
        let Some(id) = id else {
            i += 1;
            continue;
        };
        let mut repos: Option<String> = None;
        let mut depends_on: Option<String> = None;
        let mut j = i + 1;
        while j < lines.len() && !lines[j].starts_with('#') {
            let line = lines[j].trim();
            if let Some(value) = line.strip_prefix("**Repos:**") {
                if let Some((repos_part, depends_part)) = value.split_once("**Depends on:**") {
                    let repos_part = repos_part.trim().trim_end_matches('·');
                    if repos.is_none() {
                        repos = Some(normalize(repos_part));
                    }
                    if depends_on.is_none() {
                        depends_on = Some(normalize(depends_part));
                    }
                } else if repos.is_none() {
                    repos = Some(normalize(value));
                }
            } else if let Some(value) = line.strip_prefix("**Depends on:**")
                && depends_on.is_none()
            {
                depends_on = Some(normalize(value));
            }
            j += 1;
        }
        let repos =
            repos.unwrap_or_else(|| panic!("{doc_name}: slice {id} has no **Repos:** label"));
        let depends_on = depends_on
            .unwrap_or_else(|| panic!("{doc_name}: slice {id} has no **Depends on:** label"));
        if slices
            .insert(id.clone(), SliceFields { repos, depends_on })
            .is_some()
        {
            panic!("{doc_name}: slice {id} appears more than once");
        }
        i = j;
    }
    slices
}

/// Compares the two documents' E-slice sets and per-slice fields, returning
/// one message per divergence (empty when in sync).
fn sync_mismatches(map: &str, milestones: &str) -> Vec<String> {
    let map_slices = slice_fields(map, "EXECUTION-MAP.md", "## ");
    let milestone_slices = slice_fields(milestones, "MILESTONES.md", "### ");
    let mut mismatches = Vec::new();
    for id in map_slices.keys() {
        if !milestone_slices.contains_key(id) {
            mismatches.push(format!("{id}: in EXECUTION-MAP.md but not MILESTONES.md"));
        }
    }
    for id in milestone_slices.keys() {
        if !map_slices.contains_key(id) {
            mismatches.push(format!("{id}: in MILESTONES.md but not EXECUTION-MAP.md"));
        }
    }
    for (id, map_slice) in &map_slices {
        let Some(milestone_slice) = milestone_slices.get(id) else {
            continue;
        };
        if map_slice.repos != milestone_slice.repos {
            mismatches.push(format!(
                "{id} Repos drifted — map: {:?}, milestones: {:?}",
                map_slice.repos, milestone_slice.repos
            ));
        }
        if map_slice.depends_on != milestone_slice.depends_on {
            mismatches.push(format!(
                "{id} Depends-on drifted — map: {:?}, milestones: {:?}",
                map_slice.depends_on, milestone_slice.depends_on
            ));
        }
    }
    mismatches
}

/// Extracts the `(stage, date)` rows from the map's `## 3. Release stages`
/// table — the only table this guard reads, pinned by its section heading.
fn release_stage_dates(map: &str) -> Vec<(String, String)> {
    let is_date = |cell: &str| {
        cell.len() == 10
            && cell.bytes().enumerate().all(|(i, b)| {
                if i == 4 || i == 7 {
                    b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            })
    };
    let mut in_section = false;
    let mut dates = Vec::new();
    for line in structural_lines(map) {
        if line.starts_with("## 3. Release stages") {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with('#') {
            break;
        }
        if !in_section || !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if let [_, stage, date, ..] = cells.as_slice()
            && is_date(date)
        {
            assert!(
                !stage.is_empty(),
                "release-stage row for {date} has an empty stage cell — parse failure, \
                 not a sentinel pair"
            );
            dates.push((stage.to_string(), date.to_string()));
        }
    }
    dates
}

#[test]
fn milestones_slices_match_execution_map() {
    let map = read_repo_doc("docs/roadmap/v10/EXECUTION-MAP.md");
    let milestones = read_repo_doc("docs/roadmap/v10/MILESTONES.md");
    let map_slices = slice_fields(&map, "EXECUTION-MAP.md", "## ");
    for bookend in ["E0.1", "E9.7"] {
        assert!(
            map_slices.contains_key(bookend),
            "EXECUTION-MAP.md parse lost bookend slice {bookend} — parser drifted from doc structure"
        );
    }
    // Floor pins parser health beyond the bookends: a slice dropped from BOTH
    // documents (or a format break hiding it from the parser) fails here even
    // though the two parsed sets still agree with each other.
    assert!(
        map_slices.len() >= 65,
        "EXECUTION-MAP.md parsed only {} E-slices (expected at least 65) — \
         slice lost from both documents or parser drifted",
        map_slices.len()
    );
    let mismatches = sync_mismatches(&map, &milestones);
    assert!(
        mismatches.is_empty(),
        "MILESTONES.md drifted from EXECUTION-MAP.md:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn post_v1_programs_present_in_both() {
    let map = read_repo_doc("docs/roadmap/v10/EXECUTION-MAP.md");
    let milestones = read_repo_doc("docs/roadmap/v10/MILESTONES.md");
    for id in ["P1", "P2", "P3", "P4"] {
        let heads = |doc: &str, prefix: &str| {
            doc.lines().any(|line| {
                line.strip_prefix(prefix)
                    .is_some_and(|rest| rest.split_whitespace().next() == Some(id))
            })
        };
        assert!(
            heads(&map, "## "),
            "EXECUTION-MAP.md is missing program {id}"
        );
        assert!(
            heads(&milestones, "### "),
            "MILESTONES.md is missing program {id}"
        );
    }
}

#[test]
fn release_stage_dates_appear_in_milestones() {
    let map = read_repo_doc("docs/roadmap/v10/EXECUTION-MAP.md");
    let milestones = read_repo_doc("docs/roadmap/v10/MILESTONES.md");
    let dates = release_stage_dates(&map);
    assert_eq!(
        dates.len(),
        4,
        "EXECUTION-MAP.md §3 should list exactly four release stages, parsed: {dates:?}"
    );
    for (stage, date) in &dates {
        // Stage↔date association, not bare presence: some MILESTONES line must
        // carry the date next to the stage's name (its first '/'-segment, since
        // MILESTONES may shorten e.g. "V1 Feature Complete / RC / Beta"),
        // so swapping two dates between milestone headers fails.
        let stage_prefix = stage
            .split('/')
            .next()
            .map(str::trim)
            .unwrap_or(stage.as_str());
        assert!(
            milestones
                .lines()
                .any(|line| line.contains(date) && line.contains(stage_prefix)),
            "no MILESTONES.md line pairs release-stage date {date} with its stage \
             ({stage_prefix}…) from EXECUTION-MAP.md §3 — date missing or associated \
             with the wrong stage"
        );
    }
}

#[test]
fn fenced_hash_lines_are_not_structure() {
    // A fenced code block between the heading and the labels must neither
    // terminate the slice body (`#!/usr/bin/env bash`) nor contribute phantom
    // headings or labels (`**Repos:**` sample inside the fence).
    let doc = "\
## E1.1 — Fenced
```bash
#!/usr/bin/env bash
## E9.9 — phantom heading inside fence
**Repos:** `phantom`
```
**Repos:** `adoc`
**Depends on:** E0.3
";
    let slices = slice_fields(doc, "fixture", "## ");
    assert_eq!(slices.len(), 1, "phantom fenced heading must not parse");
    let slice = &slices["E1.1"];
    assert_eq!(slice.repos, "`adoc`");
    assert_eq!(slice.depends_on, "E0.3");
}

#[test]
fn first_labeled_occurrence_wins() {
    // A stray second label later in the body (e.g. quoted under an
    // out-of-scope bullet) must not overwrite the slice-header values.
    let doc = "\
## E1.1 — Duplicated label
**Repos:** `adoc`
**Depends on:** E0.3
Out of scope: a rejected variant carried this header:
**Repos:** `cloud` · **Depends on:** E9.9
";
    let slice = &slice_fields(doc, "fixture", "## ")["E1.1"];
    assert_eq!(slice.repos, "`adoc`");
    assert_eq!(slice.depends_on, "E0.3");
}

#[test]
#[should_panic(expected = "empty stage")]
fn empty_stage_cell_is_a_parse_failure() {
    // An empty stage cell must fail parsing, never yield ("", date) — a
    // sentinel pair downstream degenerates `line.contains("")` to always-true.
    let map = "\
## 3. Release stages
| Stage | Target | Meaning |
| --- | --- | --- |
| | 2026-09-30 | meaning |
";
    release_stage_dates(map);
}

#[test]
fn guard_fires_on_broken_fixture() {
    // Deliberately broken inline fixture: E0.2 drifts on Repos and
    // Depends-on, and E0.3 exists only in the map.
    let broken_map = "\
# Phase E0 — Fixture
## E0.1 — Alpha
**Repos:** `adoc`
**Depends on:** none
## E0.2 — Beta
**Repos:** `adoc`, `cloud`
**Depends on:** E0.1
## E0.3 — Gamma
**Repos:** `adoc`
**Depends on:** E0.2
";
    let broken_milestones = "\
## Milestone E0 — Fixture
### E0.1 — Alpha
**Repos:** `adoc` · **Depends on:** none
### E0.2 — Beta
**Repos:** `adoc` · **Depends on:** E0.9
";
    let mismatches = sync_mismatches(broken_map, broken_milestones);
    assert_eq!(
        mismatches.len(),
        3,
        "expected missing-slice + Repos + Depends-on mismatches, got: {mismatches:?}"
    );
    assert!(
        mismatches
            .iter()
            .any(|m| m.contains("E0.3") && m.contains("not MILESTONES"))
    );
    assert!(mismatches.iter().any(|m| m.contains("E0.2 Repos drifted")));
    assert!(
        mismatches
            .iter()
            .any(|m| m.contains("E0.2 Depends-on drifted"))
    );
}

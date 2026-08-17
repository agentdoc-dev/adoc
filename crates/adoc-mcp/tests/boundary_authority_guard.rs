//! Docs-truth guard (ADR-0041, slice E0.1): ADR-0056 rule 8 forbids the
//! roadmap from representing the B1–B6 amended Product V1 boundary as
//! pre-existing ADR-0055 acceptance — ADR-0056 is the accepting decision.
//! The guard scans every current roadmap/annex document: a line naming
//! ADR-0055 must acknowledge the amendment in the same line (name ADR-0056,
//! or say amended/superseded) or be a pinned allow-listed rule quotation.
//! `ROADMAP-V10-2026-08-12-original.md` and `archive/` are preserved history
//! (see `docs/roadmap/archive/README.md`) and exempt.

use std::fs;
use std::path::PathBuf;

/// The preserved byte-for-byte historical original; exempt per the
/// archive policy. `archive/` is exempt structurally: only the roadmap
/// root and `v10/` are scanned, subdirectories are not recursed into.
const HISTORICAL_ORIGINAL: &str = "ROADMAP-V10-2026-08-12-original.md";

/// Lines that legitimately name ADR-0055 without amendment context because
/// they state rule 8 or describe this guard's own seeded fixture. Pinned
/// exactly (file + trimmed line): editing one is a guard failure that
/// forces re-review, never silent un-guarding.
const ALLOWED_ADR_0055_LINES: &[(&str, &str)] = &[(
    "docs/roadmap/v10/MILESTONES.md",
    "- Doc guard fails on a seeded fixture line calling B3 \"ADR-0055 accepted\"; \
     passes on the repaired tree.",
)];

/// Reads every current roadmap/annex doc as `(repo-relative name, content)`,
/// sorted for deterministic failure output.
fn roadmap_docs() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docs/roadmap");
    let mut docs = Vec::new();
    for (dir, prefix) in [(root.clone(), ""), (root.join("v10"), "v10/")] {
        let entries = fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("failed to list {}: {error}", dir.display()))
                .path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if path.extension().and_then(|e| e.to_str()) != Some("md")
                || file_name == HISTORICAL_ORIGINAL
            {
                continue;
            }
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            docs.push((format!("docs/roadmap/{prefix}{file_name}"), content));
        }
    }
    docs.sort();
    docs
}

/// Yields `(1-based line number, line)` outside ``` fences: fenced content
/// is sample text, never a boundary claim.
fn structural_lines(content: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut in_fence = false;
    content.lines().enumerate().filter_map(move |(i, line)| {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            return None;
        }
        (!in_fence).then_some((i + 1, line))
    })
}

/// A line naming ADR-0055 acknowledges the amendment when it also names
/// ADR-0056 or carries an amend/supersede marker.
fn amendment_context(lower_line: &str) -> bool {
    lower_line.contains("adr-0056")
        || lower_line.contains("amend")
        || lower_line.contains("supersede")
}

fn allow_listed(doc_name: &str, line: &str) -> bool {
    ALLOWED_ADR_0055_LINES
        .iter()
        .any(|(file, allowed)| *file == doc_name && line.trim() == *allowed)
}

/// One message per line that names ADR-0055 without amendment context —
/// the rule-8 misattribution signature.
fn adr_0055_violations(doc_name: &str, content: &str) -> Vec<String> {
    structural_lines(content)
        .filter_map(|(number, line)| {
            let lower = line.to_lowercase();
            (lower.contains("adr-0055")
                && !amendment_context(&lower)
                && !allow_listed(doc_name, line))
            .then(|| {
                format!(
                    "{doc_name}:{number}: names ADR-0055 without amendment context \
                     (ADR-0056 rule 8): {}",
                    line.trim()
                )
            })
        })
        .collect()
}

#[test]
fn roadmap_never_misattributes_boundary_to_adr_0055() {
    let docs = roadmap_docs();
    // Parser-health floor: an empty or relocated directory must fail loudly,
    // never pass by scanning nothing.
    assert!(
        docs.len() >= 15,
        "scanned only {} roadmap docs (expected at least 15) — directory moved or empty",
        docs.len()
    );
    let violations: Vec<String> = docs
        .iter()
        .flat_map(|(name, content)| adr_0055_violations(name, content))
        .collect();
    assert!(
        violations.is_empty(),
        "roadmap text represents amended boundary content as pre-existing ADR-0055 \
         acceptance (ADR-0056 rule 8):\n{}",
        violations.join("\n")
    );
}

#[test]
fn guard_fires_on_seeded_misattribution() {
    // The E0.1 acceptance fixture: a line calling B3 "ADR-0055 accepted".
    let violations = adr_0055_violations(
        "fixture.md",
        "B3 PostgreSQL-canonical managed knowledge is ADR-0055 accepted.\n",
    );
    assert_eq!(
        violations.len(),
        1,
        "seeded misattribution must fire: {violations:?}"
    );
    assert!(
        violations[0].starts_with("fixture.md:1:"),
        "{}",
        violations[0]
    );
}

#[test]
fn amendment_context_lines_pass() {
    let clean = "\
**Boundary:** PRD-v1.0.md / ADR-0055, as amended by PRD-v1.1-amendment.md / ADR-0056\n\
ADR-0055 accepted PRD v1.0; ADR-0056 amends it.\n\
Supersedes in part: ADR-0055\n";
    assert_eq!(
        adr_0055_violations("fixture.md", clean),
        Vec::<String>::new()
    );
}

#[test]
fn fenced_samples_are_not_violations() {
    let doc = "\
Prose before.\n\
```\n\
B3 is ADR-0055 accepted — sample inside a fence\n\
```\n\
Prose after.\n";
    assert_eq!(adr_0055_violations("fixture.md", doc), Vec::<String>::new());
}

#[test]
fn allow_list_is_exact_per_file_and_line() {
    let (file, line) = ALLOWED_ADR_0055_LINES[0];
    // The pinned line passes only in its own file…
    assert_eq!(adr_0055_violations(file, line), Vec::<String>::new());
    // …not under another name, and not once edited.
    assert_eq!(adr_0055_violations("fixture.md", line).len(), 1);
    let edited = format!("{line} plus drift");
    assert_eq!(adr_0055_violations(file, &edited).len(), 1);
}

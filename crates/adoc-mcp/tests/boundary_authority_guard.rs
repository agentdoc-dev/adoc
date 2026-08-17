//! Docs-truth guard (ADR-0041, slice E0.1): ADR-0056 rule 8 forbids the
//! roadmap from representing the B1–B6 amended Product V1 boundary as
//! pre-existing ADR-0055 acceptance — ADR-0056 is the accepting decision.
//! The guard scans every current roadmap/annex document: a line naming
//! ADR-0055 must acknowledge the amendment in the same line (name ADR-0056,
//! or say amended/superseded) or be a pinned allow-listed rule quotation.
//! `ROADMAP-V10-2026-08-12-original.md` and `archive/` are preserved history
//! (see `docs/roadmap/archive/README.md`) and exempt.

mod support;

use std::fs;
use std::path::PathBuf;

use support::doc_scan::structural_lines;

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

/// Lines that legitimately combine close/criterion/by-construction because
/// they state the PR #143 acceptance-wording rule itself. Same pinning
/// discipline as `ALLOWED_ADR_0055_LINES`.
const ALLOWED_CRITERION_CLOSURE_LINES: &[(&str, &str)] = &[
    (
        "docs/roadmap/ROADMAP-V10-REVISION.md",
        "10. **PR acceptance wording:** planned slices do not \u{201c}close\u{201d} PRD \
         acceptance criteria by construction. Criteria are mapped to planned work until \
         implementation/evidence exists.",
    ),
    (
        "docs/roadmap/ROADMAP-V10-REVISION.md",
        "- PR description no longer claims 19/20 acceptance criteria are closed \
         \u{201c}by construction\u{201d};",
    ),
    (
        "docs/roadmap/v10/MILESTONES.md",
        "3. `E0.1.T3` — Sweep acceptance wording per PR #143: no planned slice text \
         claims to \"close\" a PRD acceptance criterion by construction; criteria map to \
         planned work until implementation/evidence exists; extend the T1 check to guard \
         the phrasing.",
    ),
];

/// Reads every current roadmap/annex doc as `(repo-relative name, content)`,
/// sorted for deterministic failure output. Asserts the parser-health floor
/// here so every real-tree guard inherits it: an empty or relocated
/// directory must fail loudly, never pass by scanning nothing.
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
    assert!(
        docs.len() >= 15,
        "scanned only {} roadmap docs (expected at least 15) — directory moved or empty",
        docs.len()
    );
    docs
}

/// A line naming ADR-0055 acknowledges the amendment when it also names
/// ADR-0056 or carries an amend/supersede marker.
fn amendment_context(lower_line: &str) -> bool {
    lower_line.contains("adr-0056")
        || lower_line.contains("amend")
        || lower_line.contains("supersede")
}

fn allow_listed(list: &[(&str, &str)], doc_name: &str, line: &str) -> bool {
    list.iter()
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
                && !allow_listed(ALLOWED_ADR_0055_LINES, doc_name, line))
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

/// True when the line carries a close inflection as a standalone word —
/// `disclosure` never matches.
fn has_close_token(lower_line: &str) -> bool {
    lower_line
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| matches!(word, "close" | "closes" | "closed" | "closing" | "closure"))
}

/// One message per line claiming acceptance-criterion closure by
/// construction — the PR #143 acceptance-wording signature: a close token,
/// "criteri", and "by construction" together on one line.
fn criterion_closure_violations(doc_name: &str, content: &str) -> Vec<String> {
    structural_lines(content)
        .filter_map(|(number, line)| {
            let lower = line.to_lowercase();
            (lower.contains("by construction")
                && lower.contains("criteri")
                && has_close_token(&lower)
                && !allow_listed(ALLOWED_CRITERION_CLOSURE_LINES, doc_name, line))
            .then(|| {
                format!(
                    "{doc_name}:{number}: claims criterion closure by construction \
                     (PR #143 acceptance-wording rule): {}",
                    line.trim()
                )
            })
        })
        .collect()
}

/// True when the text carries a `V10.<digit>` slice ID — `ROADMAP-V10.md`
/// and bare `V10` never match.
fn mentions_v10_slice(lower_text: &str) -> bool {
    lower_text.match_indices("v10.").any(|(index, marker)| {
        lower_text[index + marker.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    })
}

/// One message per `**Depends on:**` line citing a legacy `V10.x` slice —
/// dependencies use `E*` IDs only; `V10.x` is provenance, never a dependency.
fn v10_dependency_violations(doc_name: &str, content: &str) -> Vec<String> {
    structural_lines(content)
        .filter_map(|(number, line)| {
            let (_, dependencies) = line.split_once("**Depends on:**")?;
            mentions_v10_slice(&dependencies.to_lowercase()).then(|| {
                format!(
                    "{doc_name}:{number}: legacy V10.x slice cited as a dependency: {}",
                    line.trim()
                )
            })
        })
        .collect()
}

/// Reads every `.github` issue/PR template as `(repo-relative name,
/// content)`. Absence is the normal state and vacuously clean; templates
/// that appear later are scanned automatically, never silently skipped.
fn issue_template_docs() -> Vec<(String, String)> {
    let github = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".github");
    let mut docs = Vec::new();
    let template_dir = github.join("ISSUE_TEMPLATE");
    if let Ok(entries) = fs::read_dir(&template_dir) {
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| {
                    panic!("failed to list {}: {error}", template_dir.display())
                })
                .path();
            let extension = path.extension().and_then(|e| e.to_str());
            if !matches!(extension, Some("md" | "yml" | "yaml")) {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            docs.push((format!(".github/ISSUE_TEMPLATE/{file_name}"), content));
        }
    }
    if let Ok(content) = fs::read_to_string(github.join("PULL_REQUEST_TEMPLATE.md")) {
        docs.push((".github/PULL_REQUEST_TEMPLATE.md".to_string(), content));
    }
    docs.sort();
    docs
}

/// One message per template line naming a `V10.x` slice: templates seed new
/// plans, where legacy IDs are never legitimate — not even as provenance.
fn template_v10_violations(doc_name: &str, content: &str) -> Vec<String> {
    structural_lines(content)
        .filter(|(_, line)| mentions_v10_slice(&line.to_lowercase()))
        .map(|(number, line)| {
            format!(
                "{doc_name}:{number}: legacy V10.x slice referenced in a template: {}",
                line.trim()
            )
        })
        .collect()
}

#[test]
fn roadmap_never_misattributes_boundary_to_adr_0055() {
    let violations: Vec<String> = roadmap_docs()
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
fn roadmap_never_claims_criterion_closure_by_construction() {
    let violations: Vec<String> = roadmap_docs()
        .iter()
        .flat_map(|(name, content)| criterion_closure_violations(name, content))
        .collect();
    assert!(
        violations.is_empty(),
        "planned slice text claims to close a PRD acceptance criterion by construction \
         (PR #143 acceptance-wording rule):\n{}",
        violations.join("\n")
    );
}

#[test]
fn guard_fires_on_seeded_criterion_closure() {
    let violations = criterion_closure_violations(
        "fixture.md",
        "E4.2 closes acceptance criterion AC-7 by construction.\n",
    );
    assert_eq!(
        violations.len(),
        1,
        "seeded closure claim must fire: {violations:?}"
    );
    assert!(
        violations[0].starts_with("fixture.md:1:"),
        "{}",
        violations[0]
    );
    // Inflections make the same prohibited claim.
    for line in [
        "E4.2 is closing acceptance criterion AC-7 by construction.\n",
        "by construction, this is closure of acceptance criterion AC-7.\n",
    ] {
        assert_eq!(
            criterion_closure_violations("fixture.md", line).len(),
            1,
            "inflected closure claim must fire: {line:?}"
        );
    }
}

#[test]
fn technical_by_construction_lines_pass() {
    // Type-level impossibility statements are not acceptance-criterion
    // closure claims; "disclosure" must not match as a close token.
    let clean = "\
absent fields impossible by construction;\n\
approval has exactly one binding field by construction.\n\
disclosure criteria hold by construction.\n";
    assert_eq!(
        criterion_closure_violations("fixture.md", clean),
        Vec::<String>::new()
    );
}

#[test]
fn roadmap_never_depends_on_legacy_v10_slices() {
    let violations: Vec<String> = roadmap_docs()
        .iter()
        .flat_map(|(name, content)| v10_dependency_violations(name, content))
        .collect();
    assert!(
        violations.is_empty(),
        "plan text references a legacy V10.x slice as a dependency \
         (provenance citations only):\n{}",
        violations.join("\n")
    );
}

#[test]
fn guard_fires_on_seeded_v10_dependency() {
    let violations = v10_dependency_violations(
        "fixture.md",
        "**Repos:** `adoc` · **Depends on:** V10.1.4\n",
    );
    assert_eq!(
        violations.len(),
        1,
        "seeded V10.x dependency must fire: {violations:?}"
    );
    let clean = v10_dependency_violations(
        "fixture.md",
        "**Depends on:** E0.3\nrestaged into V10, carrying debt (provenance: V10.1.4)\n",
    );
    assert_eq!(clean, Vec::<String>::new());
    // A stray second label must not truncate the inspected remainder.
    let doubled = v10_dependency_violations(
        "fixture.md",
        "**Depends on:** E0.3 **Depends on:** V10.9.9\n",
    );
    assert_eq!(
        doubled.len(),
        1,
        "second-label remainder must be inspected: {doubled:?}"
    );
}

#[test]
fn issue_templates_never_reference_legacy_v10_slices() {
    let violations: Vec<String> = issue_template_docs()
        .iter()
        .flat_map(|(name, content)| template_v10_violations(name, content))
        .collect();
    assert!(
        violations.is_empty(),
        "issue/PR template references a legacy V10.x slice:\n{}",
        violations.join("\n")
    );
}

#[test]
fn guard_fires_on_seeded_template_v10_reference() {
    let violations =
        template_v10_violations(".github/ISSUE_TEMPLATE/fixture.md", "Depends on: V10.1.4\n");
    assert_eq!(
        violations.len(),
        1,
        "seeded template V10.x reference must fire: {violations:?}"
    );
    // A dot not followed by a digit is a filename, not a slice ID.
    let clean = template_v10_violations(
        ".github/ISSUE_TEMPLATE/fixture.md",
        "See ROADMAP-V10.md for the current plan.\n",
    );
    assert_eq!(clean, Vec::<String>::new());
}

#[test]
fn dependency_rule_ignores_v10_filenames() {
    let clean =
        v10_dependency_violations("fixture.md", "**Depends on:** the plan in ROADMAP-V10.md\n");
    assert_eq!(clean, Vec::<String>::new());
}

#[test]
fn scan_scope_covers_every_roadmap_subdirectory() {
    // The archive/ exemption is structural (subdirectories are not recursed),
    // so a NEW subdirectory would be silently unguarded. Pin the known set:
    // adding e.g. docs/roadmap/v11/ must fail here and force a deliberate
    // guard-scope decision.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docs/roadmap");
    for (dir, expected) in [
        (root.clone(), vec!["archive", "v10"]),
        (root.join("v10"), vec![]),
    ] {
        let mut subdirs: Vec<String> = fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                path.is_dir()
                    .then(|| path.file_name()?.to_str().map(str::to_string))
                    .flatten()
            })
            .collect();
        subdirs.sort();
        assert_eq!(
            subdirs,
            expected,
            "unexpected subdirectory under {} — extend the guard scan scope or \
             exempt it deliberately",
            dir.display()
        );
    }
}

#[test]
fn allow_lists_are_exact_and_present() {
    type RuleFn = fn(&str, &str) -> Vec<String>;
    let lists: [(&[(&str, &str)], RuleFn); 2] = [
        (ALLOWED_ADR_0055_LINES, adr_0055_violations),
        (
            ALLOWED_CRITERION_CLOSURE_LINES,
            criterion_closure_violations,
        ),
    ];
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (list, violations) in lists {
        for (file, line) in list {
            // The pinned line passes only in its own file…
            assert_eq!(violations(file, line), Vec::<String>::new());
            // …not under another name, and not once edited.
            assert_eq!(violations("fixture.md", line).len(), 1, "{file}: {line}");
            let edited = format!("{line} plus drift");
            assert_eq!(violations(file, &edited).len(), 1, "{file}: {line}");
            // Deleting the pinned line is a guard failure too — an allow-list
            // entry pointing at nothing is silent un-guarding in waiting.
            let content = fs::read_to_string(repo_root.join(file))
                .unwrap_or_else(|error| panic!("failed to read {file}: {error}"));
            assert!(
                content.lines().any(|doc_line| doc_line.trim() == *line),
                "allow-listed line no longer present in {file}: {line}"
            );
        }
    }
}

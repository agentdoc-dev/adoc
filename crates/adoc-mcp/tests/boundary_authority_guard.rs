//! Docs-truth guard (ADR-0041, slices E0.1/E0.2): ADR-0056 rule 8 forbids
//! the roadmap from representing the B1–B6 amended Product V1 boundary as
//! pre-existing ADR-0055 acceptance — ADR-0056 is the accepting decision.
//! The guard scans every current roadmap/annex document: a line naming
//! ADR-0055 must acknowledge the amendment in the same line (name ADR-0056,
//! or say amended/superseded) or be a pinned allow-listed rule quotation.
//! `ROADMAP-V10-2026-08-12-original.md` and `archive/` are preserved history
//! (see `docs/roadmap/archive/README.md`) and exempt.
//!
//! Slice E0.2 extends the guard to the four managed-product invariants
//! (ADR-0057, D36–D39): the ADR's invariant statements and its verbatim
//! authorization precedence pipeline are pinned, and no roadmap/annex line
//! may state a permissive-default authorization resolution.

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

/// Lines that legitimately state a permissive-default signature because
/// they describe this guard's own seeded fixture. Same pinning discipline
/// as `ALLOWED_ADR_0055_LINES`.
const ALLOWED_PERMISSIVE_DEFAULT_LINES: &[(&str, &str)] = &[(
    "docs/roadmap/v10/MILESTONES.md",
    "- Doc guard fails on a seeded fixture stating a permissive-default resolution.",
)];

/// The annex deference lines, pinned exactly like the allow-lists:
/// rewriting one into different — or contradictory — deference fails the
/// guard and forces re-review, never silent un-anchoring. Trailing
/// hard-break spaces are trimmed by the comparison.
const INVARIANT_AUTHORITY_LINES: &[(&str, &str)] = &[
    (
        "docs/roadmap/v10/AUTHORIZATION.md",
        "**Invariant authority:** \
         [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md) fixes the \
         deterministic, deny-by-default authorization precedence (invariant 3, D38) \
         \u{2014} this annex elaborates that order and never redefines it",
    ),
    (
        "docs/roadmap/v10/KNOWLEDGE-MODEL.md",
        "**Invariant authority:** \
         [ADR-0057](../../adr/0057-fix-four-managed-product-invariants.md) fixes \
         workspace-qualified Object identity (D36; K6) and append-only managed state \
         over immutable content versions (D37; K4) \u{2014} this annex elaborates those \
         invariants and never redefines them",
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
            // The exemption is tied to the archive policy's location — a
            // same-named file appearing under v10/ is scanned, not exempt.
            let exempt_here = prefix.is_empty() && file_name == HISTORICAL_ORIGINAL;
            let is_markdown = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            if !is_markdown || exempt_here {
                continue;
            }
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            if let Some(opener) = support::doc_scan::unclosed_fence(&content) {
                panic!(
                    "docs/roadmap/{prefix}{file_name}:{opener}: fence never closes — \
                     everything to EOF is invisible to the guards; close or repair it"
                );
            }
            docs.push((format!("docs/roadmap/{prefix}{file_name}"), content));
        }
    }
    docs.sort();
    // Pinned at the current count: adding docs is free (>=), removing one
    // fails here until the pin is deliberately lowered with the removal.
    assert!(
        docs.len() >= 19,
        "scanned only {} roadmap docs (expected at least 19) — doc removed or \
         directory moved; lower the pin deliberately with the removal",
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

/// True when the line carries a close inflection as a standalone word.
/// Words are whitespace-delimited with boundary punctuation trimmed, so
/// `disclosure`, path/URL fragments (`docs/close-book/`), and hyphenated
/// compounds (`close-out`) never match, while quoted forms (`“close”`) do.
fn has_close_token(lower_line: &str) -> bool {
    lower_line
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
        .any(|word| {
            matches!(
                word,
                "close" | "closes" | "closed" | "closing" | "closure" | "closures"
            )
        })
}

/// One message per line claiming acceptance-criterion closure by
/// construction — the PR #143 acceptance-wording signature: a close token,
/// "criteri", and "by construction" together on one line.
fn criterion_closure_violations(doc_name: &str, content: &str) -> Vec<String> {
    structural_lines(content)
        .filter_map(|(number, line)| {
            // The compound adjective is written both ways.
            let lower = line
                .to_lowercase()
                .replace("by-construction", "by construction");
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

/// One message per line stating a permissive-default authorization
/// resolution — the ADR-0057 invariant-3 reintroduction signature.
/// Hyphens normalize to spaces so compound forms (`permissive-default`,
/// `allow-by-default`, `fail-open`) match. Prohibition markers are
/// anchored phrases, not a bare `never`: an incidental `never` in an
/// adjacent clause ("ACLs never widen; scopes default to allow") must not
/// disarm the tripwire. `deny-by-default` normalizes to `deny by default`
/// and matches no signature.
fn permissive_default_violations(doc_name: &str, content: &str) -> Vec<String> {
    const SIGNATURES: &[&str] = &[
        "permissive default",
        "permissive by default",
        "default allow",
        "allow by default",
        "allowed by default",
        "default to allow",
        "defaults to allow",
        "implicit allow",
        "open by default",
        "fail open",
        "fails open",
    ];
    const PROHIBITION_MARKERS: &[&str] = &[
        "never a permissive default",
        "no permissive default",
        "never fail open",
        "never fails open",
        "not allowed by default",
    ];
    structural_lines(content)
        .filter_map(|(number, line)| {
            let lower = line.to_lowercase().replace('-', " ");
            (SIGNATURES.iter().any(|signature| lower.contains(signature))
                && !PROHIBITION_MARKERS
                    .iter()
                    .any(|marker| lower.contains(marker))
                && !allow_listed(ALLOWED_PERMISSIVE_DEFAULT_LINES, doc_name, line))
            .then(|| {
                format!(
                    "{doc_name}:{number}: states a permissive-default authorization \
                     resolution (ADR-0057 invariant 3): {}",
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

/// True for issue-template bodies: md/yml/yaml files except GitHub's
/// reserved template-selection metadata (`config.yml`/`config.yaml`), which
/// is UI configuration nobody pastes into an issue — its contact_links may
/// legitimately reference legacy V10 material.
fn is_template_file(file_name: &str) -> bool {
    // Case-insensitive: case-preserving filesystems produce .MD files that
    // GitHub's loader still serves.
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    let reserved = file_name.eq_ignore_ascii_case("config.yml")
        || file_name.eq_ignore_ascii_case("config.yaml");
    matches!(extension.as_deref(), Some("md" | "yml" | "yaml")) && !reserved
}

/// Reads every `.github` issue/PR template as `(repo-relative name,
/// content)`. Absence is the normal state and vacuously clean; templates
/// that appear later are scanned automatically, never silently skipped.
fn issue_template_docs() -> Vec<(String, String)> {
    let github = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".github");
    let mut docs = Vec::new();
    let mut reserved_config_seen = false;
    let template_dir = github.join("ISSUE_TEMPLATE");
    let template_dir_exists = template_dir.is_dir();
    if let Ok(entries) = fs::read_dir(&template_dir) {
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| {
                    panic!("failed to list {}: {error}", template_dir.display())
                })
                .path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if file_name.eq_ignore_ascii_case("config.yml")
                || file_name.eq_ignore_ascii_case("config.yaml")
            {
                reserved_config_seen = true;
                continue;
            }
            if !is_template_file(file_name) {
                continue;
            }
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
            docs.push((format!(".github/ISSUE_TEMPLATE/{file_name}"), content));
        }
    }
    // Absence is vacuously clean, but an existing template directory that
    // parses to nothing is a filter bug, never a pass. Counted before the
    // PR template is added, so its presence cannot mask an empty parse. A
    // config-only directory (chooser metadata, zero bodies) is a valid
    // GitHub setup and counts as parsed without being scanned.
    // ponytail: with config.yml present, the alarm cannot see any other
    // dropped file; remaining drops are files GitHub ignores too
    // (non-template extensions), so the blindness is benign until the
    // filter and GitHub's loader disagree.
    assert!(
        !template_dir_exists || !docs.is_empty() || reserved_config_seen,
        ".github/ISSUE_TEMPLATE/ exists but no templates were parsed — \
         scanner filter drifted"
    );
    if let Ok(content) = fs::read_to_string(github.join("PULL_REQUEST_TEMPLATE.md")) {
        docs.push((".github/PULL_REQUEST_TEMPLATE.md".to_string(), content));
    }
    docs.sort();
    docs
}

/// One message per template line naming a `V10.x` slice: templates seed new
/// plans, where legacy IDs are never legitimate — not even as provenance.
/// Every line counts, fenced or not: a template's fenced skeleton is the
/// payload users paste verbatim, unlike the narrative docs' sample fences.
fn template_v10_violations(doc_name: &str, content: &str) -> Vec<String> {
    content
        .lines()
        .enumerate()
        .filter(|(_, line)| mentions_v10_slice(&line.to_lowercase()))
        .map(|(index, line)| {
            format!(
                "{doc_name}:{}: legacy V10.x slice referenced in a template: {}",
                index + 1,
                line.trim()
            )
        })
        .collect()
}

/// Reads ADR-0057 — the accepting decision the E0.2 pins run against.
fn adr_0057() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docs/adr/0057-fix-four-managed-product-invariants.md");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn adr_0057_states_all_four_invariants() {
    // E0.2.T1: ADR-0057 is Accepted and states the four managed-product
    // invariants (D36–D39) as architecture rules, each with its refuted
    // alternative. Editing one away fails here and forces re-review — the
    // invariant-3 pin stops at the stable sentence prefix because E0.2.T2
    // completes that decision's pipeline wording.
    let adr = adr_0057();
    assert!(
        adr.contains("- Status: Accepted"),
        "ADR-0057 must remain Accepted"
    );
    for (invariant, refuted_alternative) in [
        (
            "**Managed Object identity is workspace-qualified.**",
            "never auto-merge",
        ),
        (
            "**Content versions are immutable; managed state changes are append-only events.**",
            "does not create a new semantic content version",
        ),
        (
            "**Authorization uses one deterministic",
            "may narrow but never widen AgentDoc authority",
        ),
        (
            "**Human semantic review can require independence.**",
            "cannot satisfy an explicit independent-review obligation",
        ),
    ] {
        assert!(
            adr.contains(invariant),
            "ADR-0057 lost an invariant: {invariant}"
        );
        assert!(
            adr.contains(refuted_alternative),
            "ADR-0057 lost a refuted alternative: {refuted_alternative}"
        );
    }
}

/// The authorization precedence pipeline, line by line as accepted in
/// PRD v1.1 §5.1 (RT-05/D38). ADR-0057 must carry it verbatim, and the PRD
/// section itself must not drift from the pinned canon.
const PRECEDENCE_PIPELINE: &[&str] = &[
    "identity/session/grant freshness",
    "→ hard/system denies",
    "→ current source-ACL ceiling",
    "→ scoped AgentDoc grants/denies",
    "→ object/field/proposition visibility",
    "→ action-specific policy",
    "→ allow | deny | insufficient_context",
];

/// Asserts the pipeline appears as one contiguous block — trimmed lines,
/// verbatim, in order. A per-line cursor walk would still accept a stage
/// spliced between two canonical stages, or stages scattered in order
/// across unrelated sections; contiguity is what "verbatim" promises, and
/// the order is D38's substance.
fn assert_pipeline_block(doc_name: &str, content: &str) {
    let lines: Vec<&str> = content.lines().map(str::trim).collect();
    assert!(
        lines
            .windows(PRECEDENCE_PIPELINE.len())
            .any(|window| window == PRECEDENCE_PIPELINE),
        "{doc_name}: the precedence pipeline no longer appears as a verbatim \
         contiguous block (stage reordered, inserted, or missing)"
    );
}

/// The `### 5.1` section only, so the pin's label stays true: stages
/// surviving elsewhere in the PRD while §5.1 is gutted must not satisfy it.
fn prd_section_5_1() -> String {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let prd_path = repo_root.join("docs/product/PRD-v1.1-amendment.md");
    let prd = fs::read_to_string(&prd_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", prd_path.display()));
    let start = prd
        .find("### 5.1 Authorization precedence")
        .unwrap_or_else(|| panic!("PRD v1.1 lost the `### 5.1 Authorization precedence` heading"));
    let section = &prd[start..];
    let end = section.find("\n## ").unwrap_or(section.len());
    section[..end].to_string()
}

#[test]
fn adr_0057_pins_the_precedence_pipeline_verbatim() {
    // E0.2.T2: ADR-0057 states the PRD v1.1 §5.1 pipeline verbatim, with
    // deny-by-default, the equal-or-more-specific deny tie-break, and
    // fail-closed consequential uncertainty (RT-05/D38 — never a
    // permissive default).
    assert_pipeline_block(
        "docs/product/PRD-v1.1-amendment.md §5.1",
        &prd_section_5_1(),
    );
    let adr = adr_0057();
    assert_pipeline_block("ADR-0057", &adr);
    for sentence in [
        "deny-by-default",
        "Hard deny wins.",
        "At equal or more-specific scope, explicit deny wins over allow.",
        "Expired/stale grants/membership observations do not authorize.",
        "Consequential uncertainty fails closed",
        "never a permissive default",
    ] {
        assert!(adr.contains(sentence), "ADR-0057 must state: {sentence}");
    }
}

#[test]
#[should_panic(expected = "verbatim contiguous block")]
fn pipeline_pin_rejects_a_reordered_pipeline() {
    // The audit fixture: swapping two stages must fail even though every
    // stage is still present as a substring.
    let reordered = "\
identity/session/grant freshness\n\
→ current source-ACL ceiling\n\
→ hard/system denies\n\
→ scoped AgentDoc grants/denies\n\
→ object/field/proposition visibility\n\
→ action-specific policy\n\
→ allow | deny | insufficient_context\n";
    assert_pipeline_block("fixture.md", reordered);
}

#[test]
fn annexes_cross_link_the_adr_0057_invariants() {
    // E0.2.T3: the authorization and knowledge-model annexes defer to
    // ADR-0057 explicitly, so no annex carries a competing precedence
    // order. The deference lines are pinned exactly — a removed, rewritten,
    // or contradictory line fails here, the same footing as the allow-list
    // pins. Reusing the scanned corpus inherits its fence-health floor.
    let docs = roadmap_docs();
    for (annex, pinned) in INVARIANT_AUTHORITY_LINES {
        let (_, content) = docs
            .iter()
            .find(|(name, _)| name == annex)
            .unwrap_or_else(|| panic!("{annex} missing from the roadmap scan"));
        assert!(
            structural_lines(content).any(|(_, line)| line.trim() == *pinned),
            "{annex} no longer carries the pinned invariant-authority line: {pinned}"
        );
    }
}

#[test]
fn roadmap_never_states_a_permissive_default() {
    let violations: Vec<String> = roadmap_docs()
        .iter()
        .flat_map(|(name, content)| permissive_default_violations(name, content))
        .collect();
    assert!(
        violations.is_empty(),
        "roadmap text states a permissive-default authorization resolution \
         (ADR-0057 invariant 3):\n{}",
        violations.join("\n")
    );
}

#[test]
fn guard_fires_on_seeded_permissive_default() {
    // The E0.2 acceptance fixture: a line stating a permissive-default
    // resolution, in each written form.
    for line in [
        "Unknown authorization context resolves to a permissive default.\n",
        "Unmatched scopes default to allow.\n",
        "Ambiguous grants are allow-by-default.\n",
        "Absent policy rows mean default allow.\n",
        "Consequential uncertainty fails open.\n",
        "Gate checks fail-open on timeout.\n",
        "Retrieval scopes are allowed by default.\n",
        "Grants are permissive by default.\n",
        "Absent policy rows imply an implicit allow.\n",
        "Visibility is open by default.\n",
        "Source ACLs never widen AgentDoc authority; unmatched scopes default to allow.\n",
        "Unmatched scopes default to allow and grants never expire.\n",
    ] {
        let violations = permissive_default_violations("fixture.md", line);
        assert_eq!(
            violations.len(),
            1,
            "seeded permissive default must fire: {line:?} -> {violations:?}"
        );
        assert!(
            violations[0].starts_with("fixture.md:1:"),
            "{}",
            violations[0]
        );
    }
    // The prohibition statements the rule protects pass on their own
    // merits, as do the deny-side compounds.
    let clean = "\
Consequential uncertainty yields `deny` or typed `insufficient_context`, \
never a permissive default.\n\
Precedence matches RT-05/D38: no permissive default anywhere.\n\
Forced audit-sink failure surfaces the code per policy cell, never fail-open.\n\
Restricted fields are not allowed by default.\n\
Authorization is deterministic and deny-by-default.\n";
    assert_eq!(
        permissive_default_violations("fixture.md", clean),
        Vec::<String>::new()
    );
}

#[test]
#[should_panic(expected = "verbatim contiguous block")]
fn pipeline_pin_rejects_an_inserted_stage() {
    // The review fixture: a stage spliced between two canonical stages
    // must fail even though the canonical stages still appear in order
    // (the per-line cursor walk this block matcher replaced stayed green
    // here).
    let inserted = "\
identity/session/grant freshness\n\
→ hard/system denies\n\
→ model-suggested overrides\n\
→ current source-ACL ceiling\n\
→ scoped AgentDoc grants/denies\n\
→ object/field/proposition visibility\n\
→ action-specific policy\n\
→ allow | deny | insufficient_context\n";
    assert_pipeline_block("fixture.md", inserted);
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
fn unclosed_fence_is_reported() {
    use support::doc_scan::unclosed_fence;
    // An accidentally unclosed (or mismatch-closed) fence would silently
    // unguard everything to EOF; the scanner must say so.
    assert_eq!(unclosed_fence("Prose.\n```\nhidden to EOF\n"), Some(2));
    assert_eq!(
        unclosed_fence("```\n~~~ is content, not a closer\n"),
        Some(1)
    );
    assert_eq!(unclosed_fence("Prose.\n```\nsample\n```\n"), None);
    assert_eq!(unclosed_fence("no fences at all\n"), None);
}

#[test]
fn fence_delimiters_pair_by_character() {
    // A tilde line inside a backtick fence is content, not a closer; the
    // real closer must not reopen the fence and hide later prose. The one
    // violation must be the post-fence prose on line 6 — an unpaired toggle
    // reports line 4 instead.
    let doc = "\
Prose before.\n\
```\n\
~~~\n\
B3 is ADR-0055 accepted — still inside the backtick fence\n\
```\n\
B3 is ADR-0055 accepted — real prose after the fence\n";
    let violations = adr_0055_violations("fixture.md", doc);
    assert_eq!(
        violations.len(),
        1,
        "only the post-fence prose fires: {violations:?}"
    );
    assert!(
        violations[0].starts_with("fixture.md:6:"),
        "violation must be the prose after the paired close: {}",
        violations[0]
    );
    // A same-character line with trailing text is not a closer either.
    let info_string = "\
```\n\
```rust — sample line, not a closer\n\
B3 is ADR-0055 accepted — inside\n\
```\n";
    assert_eq!(
        adr_0055_violations("fixture.md", info_string),
        Vec::<String>::new()
    );
}

#[test]
fn fenced_samples_are_not_violations() {
    // CommonMark §4.5: backtick and tilde fences both delimit code blocks.
    for fence in ["```", "~~~"] {
        let doc = format!(
            "Prose before.\n{fence}\n\
             B3 is ADR-0055 accepted — sample inside a fence\n\
             E4.2 closes acceptance criterion AC-7 by construction.\n\
             {fence}\nProse after.\n"
        );
        assert_eq!(
            adr_0055_violations("fixture.md", &doc),
            Vec::<String>::new(),
            "{fence} fence must exempt the misattribution sample"
        );
        assert_eq!(
            criterion_closure_violations("fixture.md", &doc),
            Vec::<String>::new(),
            "{fence} fence must exempt the closure-claim sample"
        );
    }
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
    // Inflections and the hyphenated compound make the same prohibited claim.
    for line in [
        "E4.2 is closing acceptance criterion AC-7 by construction.\n",
        "by construction, this is closure of acceptance criterion AC-7.\n",
        "by-construction closure of criterion AC-7.\n",
        "gate closures for acceptance criteria hold by construction.\n",
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
disclosure criteria hold by construction.\n\
See docs/close-book/criteria.md for by-construction acceptance rules.\n\
the close-out review criteria hold by construction.\n";
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
fn template_selection_metadata_is_not_a_template() {
    // GitHub-reserved template-selection metadata may legitimately link
    // legacy V10 migration notes in contact_links URLs; template bodies
    // stay scanned.
    for (file, is_template) in [
        ("config.yml", false),
        ("config.yaml", false),
        ("CONFIG.YML", false),
        ("bug_report.md", true),
        ("feature_request.MD", true),
        ("feature.yml", true),
        ("form.yaml", true),
        ("README.txt", false),
    ] {
        assert_eq!(
            is_template_file(file),
            is_template,
            "{file} template classification"
        );
    }
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
    // Templates are pasted verbatim, so a fenced skeleton is payload, not a
    // sample — the fence exemption of the narrative-doc guards must not apply.
    let fenced = template_v10_violations(
        ".github/ISSUE_TEMPLATE/fixture.md",
        "## Copy this\n```\n**Depends on:** V10.1.4\n```\n",
    );
    assert_eq!(
        fenced.len(),
        1,
        "fenced template payload must fire: {fenced:?}"
    );
}

#[test]
fn dependency_rule_ignores_v10_filenames() {
    let clean =
        v10_dependency_violations("fixture.md", "**Depends on:** the plan in ROADMAP-V10.md\n");
    assert_eq!(clean, Vec::<String>::new());
}

/// Byte-exact existence check: `Path::exists()` on a case-insensitive
/// filesystem aliases case variants, so pinning `pull_request_template.md`
/// absent would spuriously fail once the canonical uppercase file exists.
fn exists_exact(repo_root: &std::path::Path, relative: &str) -> bool {
    let (dir, name) = relative.rsplit_once('/').unwrap_or((".", relative));
    fs::read_dir(repo_root.join(dir)).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_str() == Some(name))
    })
}

#[test]
fn exact_existence_does_not_alias_case() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(exists_exact(&repo_root, ".github/dependabot.yml"));
    // On APFS, Path::exists() would say true here; the byte-exact check
    // must not.
    assert!(!exists_exact(&repo_root, ".github/DEPENDABOT.YML"));
}

#[test]
fn template_scope_covers_every_github_variant() {
    // The scanner reads .github/ISSUE_TEMPLATE/ and
    // .github/PULL_REQUEST_TEMPLATE.md. GitHub also recognises these
    // alternative template locations; introducing one must fail here and
    // force a deliberate scan-scope decision, mirroring
    // scan_scope_covers_every_roadmap_subdirectory.
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for alternative in [
        "PULL_REQUEST_TEMPLATE.md",
        "pull_request_template.md",
        "PULL_REQUEST_TEMPLATE",
        "docs/PULL_REQUEST_TEMPLATE.md",
        "docs/pull_request_template.md",
        "docs/PULL_REQUEST_TEMPLATE",
        ".github/pull_request_template.md",
        ".github/PULL_REQUEST_TEMPLATE",
        ".github/ISSUE_TEMPLATE.md",
        "ISSUE_TEMPLATE.md",
        "docs/ISSUE_TEMPLATE.md",
    ] {
        assert!(
            !exists_exact(&repo_root, alternative),
            "{alternative} exists but issue_template_docs() does not scan it — \
             extend the template scan scope or exempt it deliberately"
        );
    }
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
    let lists: [(&[(&str, &str)], RuleFn); 3] = [
        (ALLOWED_ADR_0055_LINES, adr_0055_violations),
        (
            ALLOWED_CRITERION_CLOSURE_LINES,
            criterion_closure_violations,
        ),
        (
            ALLOWED_PERMISSIVE_DEFAULT_LINES,
            permissive_default_violations,
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
            // Presence runs over the same structural view the suppression
            // does: a pinned line moved inside a fence is equally dead.
            let content = fs::read_to_string(repo_root.join(file))
                .unwrap_or_else(|error| panic!("failed to read {file}: {error}"));
            assert!(
                structural_lines(&content).any(|(_, doc_line)| doc_line.trim() == *line),
                "allow-listed line no longer structurally present in {file}: {line}"
            );
        }
    }
}

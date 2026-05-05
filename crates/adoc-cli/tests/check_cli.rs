mod support;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use support::{TestWorkspace, fixture_path};

#[test]
fn check_accepts_v0_1_prose_fixture_with_all_inline_syntax() {
    let fixture = fixture_path("v0_1/prose_page.adoc");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", fixture.to_str().expect("fixture path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.1 prose fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors"),
        "expected zero errors in summary, got:\n{stdout}"
    );
}

#[test]
fn check_unclosed_fence_diagnostic_surfaces_all_six_fields() {
    let workspace = TestWorkspace::new("check-unclosed-fence-shape");
    let fixture_contents = fs::read_to_string(fixture_path("v0_1/unclosed_fence.adoc"))
        .expect("unclosed_fence fixture is readable");
    let source = workspace.write("unclosed_fence.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unclosed fence to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Issue #3 acceptance: the diagnostic must carry file, line, column,
    // severity, code, and a fix-oriented message.
    let prefix = format!("{}:5:1:", source.to_str().expect("source path is utf-8"));
    assert!(
        stdout.contains(&prefix),
        "expected diagnostic to start with `path:line:column:` prefix `{prefix}`, got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[parse.unclosed_fence]"),
        "expected severity + code `error[parse.unclosed_fence]` in stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Fenced code block is missing a closing"),
        "expected fix-oriented message about the missing closing fence:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_unsafe_link_with_source_location() {
    let workspace = TestWorkspace::new("check-rejects-unsafe-link");
    let fixture_contents = fs::read_to_string(fixture_path("v0_1/unsafe_link.adoc"))
        .expect("unsafe_link fixture is readable");
    let source = workspace.write("unsafe_link.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unsafe link to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unsafe_link.adoc:3:10"),
        "expected diagnostic at line 3 column 10 (where the link starts), got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[parse.unsafe_link]"),
        "expected parse.unsafe_link code in stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("javascript:alert"),
        "expected diagnostic message to quote the rejected URL:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn build_renders_v0_1_prose_fixture_to_golden_agent_json() {
    let workspace = TestWorkspace::new("build-renders-prose-golden-json");
    let fixture_contents = fs::read_to_string(fixture_path("v0_1/prose_page.adoc"))
        .expect("prose fixture is readable");
    workspace.write("prose_page.adoc", &fixture_contents);

    // Run with cwd=workspace so the recorded source_path is "prose_page.adoc"
    // rather than a host-specific absolute path.
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["build", "prose_page.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.agent.json"))
        .expect("docs.agent.json is written");
    let golden = fs::read_to_string(fixture_path("v0_1/prose_page.golden.agent.json"))
        .expect("golden agent JSON fixture is readable");

    assert_eq!(
        actual, golden,
        "agent JSON diverged from prose_page.golden.agent.json; \
         re-run `adoc build` against prose_page.adoc and review before updating the snapshot"
    );
}

#[test]
fn build_renders_v0_1_prose_fixture_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-prose-golden-html");
    let fixture = fixture_path("v0_1/prose_page.adoc");
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            fixture.to_str().expect("fixture path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual =
        fs::read_to_string(output_directory.join("docs.html")).expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_1/prose_page.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from prose_page.golden.html; \
         re-run `adoc build` against prose_page.adoc and review before updating the snapshot"
    );
}

#[test]
fn check_accepts_minimal_prose_page() {
    let workspace = TestWorkspace::new("check-accepts-minimal-prose-page");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(docs.getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("0 errors"),
        "stdout should summarize successful diagnostics"
    );
}

#[test]
fn build_creates_missing_output_directory_and_writes_artifacts() {
    let workspace = TestWorkspace::new("build-writes-artifacts");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(docs.getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("<h1>Getting Started</h1>"));
    assert!(html.contains("<p>AgentDoc keeps knowledge readable.</p>"));

    let agent_json = fs::read_to_string(output_directory.join("docs.agent.json"))
        .expect("agent JSON is written");
    assert!(agent_json.contains("\"schema_version\": \"adoc.agent.v0\""));
    assert!(agent_json.contains("\"pages\""));
    assert!(agent_json.contains("\"objects\": []"));
    assert!(agent_json.contains("\"diagnostics\": []"));
}

#[test]
fn build_groups_contiguous_list_items_by_list_kind() {
    let workspace = TestWorkspace::new("build-groups-contiguous-lists");
    let source = workspace.write(
        "guide.adoc",
        "# Lists @doc(docs.lists)\n\n- Write source\n- Run check\n\n1. Build artifacts\n2. Inspect output\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("<ul>\n<li>Write source</li>\n<li>Run check</li>\n</ul>"));
    assert!(html.contains("<ol>\n<li>Build artifacts</li>\n<li>Inspect output</li>\n</ol>"));
    assert_eq!(html.matches("<ul>").count(), 1);
    assert_eq!(html.matches("<ol>").count(), 1);
}

#[test]
fn build_derives_distinct_page_ids_from_directory_relative_paths() {
    let workspace = TestWorkspace::new("build-derives-distinct-page-ids");
    workspace.write("a/guide.adoc", "# Alpha Guide\n\nAlpha content.\n");
    workspace.write("b/guide.adoc", "# Beta Guide\n\nBeta content.\n");
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            workspace.root.to_str().expect("workspace path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("data-page-id=\"a.guide\""));
    assert!(html.contains("data-page-id=\"b.guide\""));

    let agent_json = fs::read_to_string(output_directory.join("docs.agent.json"))
        .expect("agent JSON is written");
    assert!(agent_json.contains("\"id\": \"a.guide\""));
    assert!(agent_json.contains("\"id\": \"b.guide\""));
}

#[test]
fn build_keeps_page_identity_from_first_heading_annotation() {
    let workspace = TestWorkspace::new("build-keeps-first-heading-page-id");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(docs.primary-guide)\n\n## Details @doc(docs.details-section)\n\nMore context.\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("data-page-id=\"docs.primary-guide\""));
    assert!(!html.contains("data-page-id=\"docs.details-section\""));

    let agent_json = fs::read_to_string(output_directory.join("docs.agent.json"))
        .expect("agent JSON is written");
    assert!(agent_json.contains("\"id\": \"docs.primary-guide\""));
    assert!(!agent_json.contains("\"id\": \"docs.details-section\""));
}

#[test]
fn build_uses_first_top_level_heading_annotation_for_page_identity() {
    let workspace = TestWorkspace::new("build-uses-top-level-page-heading-id");
    let source = workspace.write(
        "guide.adoc",
        "## Draft Notes @doc(draft.notes)\n\n# Guide @doc(product.area)\n\nMore context.\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("data-page-id=\"product.area\""));
    assert!(!html.contains("data-page-id=\"draft.notes\""));

    let agent_json = fs::read_to_string(output_directory.join("docs.agent.json"))
        .expect("agent JSON is written");
    assert!(agent_json.contains("\"id\": \"product.area\""));
    assert!(!agent_json.contains("\"id\": \"draft.notes\""));
}

#[test]
fn build_fails_clearly_when_output_path_is_a_file() {
    let workspace = TestWorkspace::new("build-output-path-is-file");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(docs.getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );
    let output_path = workspace.write("dist", "not a directory");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_path.to_str().expect("output path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        !output.status.success(),
        "expected build to fail when --out is a file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("io.output_not_directory"));
    assert!(stderr.contains("exists as a file"));
    assert!(!output_path.join("docs.html").exists());
    assert!(!output_path.join("docs.agent.json").exists());
}

#[test]
fn check_rejects_raw_html_with_source_location() {
    let workspace = TestWorkspace::new("check-rejects-raw-html");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\n<div>raw html</div>\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(!output.status.success(), "expected raw HTML to fail check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:1"));
    assert!(stdout.contains("error[parse.raw_html]"));
    assert!(stdout.contains("Raw HTML is not allowed in strict mode"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_unknown_raw_html_tag() {
    let workspace = TestWorkspace::new("check-rejects-unknown-raw-html-tag");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\n<foo>bar</foo>\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unknown raw HTML tag to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:1"));
    assert!(stdout.contains("error[parse.raw_html]"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_custom_element_tag() {
    let workspace = TestWorkspace::new("check-rejects-custom-element-tag");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\n<my-component>x</my-component>\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected custom element tag to fail check in strict mode"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:1"));
    assert!(stdout.contains("error[parse.raw_html]"));
}

#[test]
fn check_does_not_flag_angle_brackets_in_prose() {
    let workspace = TestWorkspace::new("check-does-not-flag-angle-brackets-in-prose");
    let source = workspace.write(
        "guide.adoc",
        "# Technical Prose @doc(docs.technical-prose)\n\nUse Vec<String> for a list.\n\nSet x < 5 here.\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected angle-bracket prose to pass check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0 errors"));
}

#[test]
fn build_rejects_inline_raw_html_and_writes_no_artifacts() {
    let workspace = TestWorkspace::new("build-rejects-inline-raw-html");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\nKeep <span>raw html</span> out.\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(!output.status.success(), "expected raw HTML to fail build");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:6"));
    assert!(stdout.contains("error[parse.raw_html]"));
    assert!(stdout.contains("Raw HTML is not allowed in strict mode"));
    assert!(stdout.contains("1 errors"));
    assert!(!output_directory.join("docs.html").exists());
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn duplicate_claim_ids_fail_check_and_block_build_artifacts() {
    let workspace = TestWorkspace::new("duplicate-claim-ids");
    workspace.write(
        "01-billing.adoc",
        "# Billing Credits @doc(billing.credits-page)\n\n::claim billing.credits.foo\nstatus: draft\n--\nCredits are granted after payment succeeds.\n::\n",
    );
    workspace.write(
        "02-billing-extra.adoc",
        "# Billing Extra @doc(billing.extra-page)\n\n::claim billing.credits.foo\nstatus: draft\n--\nCredits are also described here.\n::\n",
    );

    let check_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "check",
            workspace.root.to_str().expect("root path is utf-8"),
        ])
        .output()
        .expect("adoc check runs");

    assert!(
        !check_output.status.success(),
        "expected duplicate claim ids to fail check"
    );
    let check_stdout = String::from_utf8_lossy(&check_output.stdout);
    assert!(
        check_stdout.contains("error[id.duplicate]"),
        "expected id.duplicate diagnostic in stdout:\n{check_stdout}"
    );
    assert!(check_stdout.contains("1 errors"));

    let output_directory = workspace.root.join("dist");
    let build_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            workspace.root.to_str().expect("root path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        !build_output.status.success(),
        "expected duplicate claim ids to fail build"
    );
    let build_stdout = String::from_utf8_lossy(&build_output.stdout);
    assert!(
        build_stdout.contains("error[id.duplicate]"),
        "expected id.duplicate diagnostic in build stdout:\n{build_stdout}"
    );
    assert!(!output_directory.join("docs.html").exists());
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn check_allows_raw_html_inside_closed_fenced_code_block() {
    let workspace = TestWorkspace::new("check-allows-raw-html-in-fence");
    let source = workspace.write(
        "guide.adoc",
        "# Fenced HTML Sample @doc(docs.fenced-html)\n\n```html\n<div>example</div>\n<script>alert(1)</script>\n```\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected fenced HTML sample to pass check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("parse.raw_html"),
        "expected no parse.raw_html diagnostic for HTML inside a fenced code block:\n{stdout}"
    );
    assert!(stdout.contains("0 errors"));
}

#[test]
fn build_writes_artifacts_for_raw_html_inside_fenced_code_block() {
    let workspace = TestWorkspace::new("build-allows-raw-html-in-fence");
    let source = workspace.write(
        "guide.adoc",
        "# Fenced HTML Sample @doc(docs.fenced-html)\n\n```html\n<div>example</div>\n```\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to succeed when HTML is inside a fenced block\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(
        html.contains("&lt;div&gt;example&lt;/div&gt;"),
        "fenced HTML sample must be HTML-escaped inside <pre><code>:\n{html}"
    );
    assert!(output_directory.join("docs.agent.json").exists());
}

#[test]
fn check_rejects_unclosed_fenced_code_with_source_location() {
    let workspace = TestWorkspace::new("check-rejects-unclosed-fence");
    let source = workspace.write(
        "guide.adoc",
        "# Broken Code @doc(docs.broken-code)\n\n```rust\nfn main() {}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unclosed fenced code to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:1"));
    assert!(stdout.contains("error[parse.unclosed_fence]"));
    assert!(stdout.contains("Fenced code block is missing a closing"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_malformed_page_annotation_with_source_location() {
    let workspace = TestWorkspace::new("check-rejects-malformed-page-annotation");
    let source = workspace.write(
        "guide.adoc",
        "# Broken Annotation @doc(broken-page\n\nContent.\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected malformed page annotation to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:1:21"));
    assert!(stdout.contains("error[parse.malformed_page_annotation]"));
    assert!(stdout.contains("Page annotation must use @doc(id)"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_reports_malformed_annotation_with_indented_heading() {
    let workspace = TestWorkspace::new("check-reports-malformed-annotation-indented");
    let source = workspace.write("guide.adoc", "  # Broken @doc(\n\nContent.\n");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected indented malformed page annotation to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("guide.adoc:1:12"),
        "expected diagnostic at column 12 (the `@`), got:\n{stdout}"
    );
    assert!(stdout.contains("error[parse.malformed_page_annotation]"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_reports_trailing_content_malformed_with_indent() {
    let workspace = TestWorkspace::new("check-reports-trailing-content-indent");
    let source = workspace.write(
        "guide.adoc",
        "   # Notes (per @doc(thing) sidebar)\n\nContent.\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected trailing-content annotation with indent to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("guide.adoc:1:17"),
        "expected diagnostic at column 17 (the `@`), got:\n{stdout}"
    );
    assert!(stdout.contains("error[parse.malformed_page_annotation]"));
}

#[test]
fn check_accepts_at_doc_without_parentheses_as_heading_text() {
    let workspace = TestWorkspace::new("check-accepts-at-doc-without-parentheses");
    workspace.write(
        "team/guide.adoc",
        "# Broken Annotation @doc product.area\n\nContent.\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "check",
            workspace.root.to_str().expect("workspace path is utf-8"),
        ])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected @doc without parentheses to parse as heading text\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0 errors"));
}

#[test]
fn check_accepts_v0_2_claim_fixture() {
    let workspace = TestWorkspace::new("check-accepts-v0-2-claim");
    let fixture_contents = fs::read_to_string(fixture_path("v0_2/claim_basic.adoc"))
        .expect("claim_basic fixture is readable");
    workspace.write("claim_basic.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["check", "claim_basic.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.2 claim fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors"),
        "expected zero errors in summary, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_claim_with_missing_status() {
    let workspace = TestWorkspace::new("check-rejects-claim-missing-status");
    let fixture_contents = fs::read_to_string(fixture_path("v0_2/claim_missing_status.adoc"))
        .expect("claim_missing_status fixture is readable");
    workspace.write("claim_missing_status.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["check", "claim_missing_status.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected missing-status claim to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.missing_field]"),
        "expected schema.missing_field diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("claim_missing_status.adoc:3:1"),
        "expected diagnostic at line 3 column 1 (open-fence line), got:\n{stdout}"
    );
    assert!(
        stdout.contains("status"),
        "expected message to mention `status`, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_2_claim_fixture_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-claim-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_2/claim_basic.adoc"))
        .expect("claim_basic fixture is readable");
    workspace.write("claim_basic.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["build", "claim_basic.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_2/claim_basic.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from claim_basic.golden.html; \
         re-run `adoc build` against claim_basic.adoc and review before updating the snapshot"
    );
}

#[test]
fn build_renders_v0_2_claim_fixture_to_golden_agent_json() {
    let workspace = TestWorkspace::new("build-renders-claim-golden-json");
    let fixture_contents = fs::read_to_string(fixture_path("v0_2/claim_basic.adoc"))
        .expect("claim_basic fixture is readable");
    workspace.write("claim_basic.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["build", "claim_basic.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.agent.json"))
        .expect("docs.agent.json is written");
    let golden = fs::read_to_string(fixture_path("v0_2/claim_basic.golden.agent.json"))
        .expect("golden agent JSON fixture is readable");

    assert_eq!(
        actual, golden,
        "agent JSON diverged from claim_basic.golden.agent.json; \
         re-run `adoc build` against claim_basic.adoc and review before updating the snapshot"
    );
}

#[test]
fn check_accepts_v0_3_verified_claims_pilot_fixture() {
    let workspace = TestWorkspace::new("check-accepts-v0-3-verified-pilot");
    let fixture_contents = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.adoc"))
        .expect("verified pilot fixture is readable");
    workspace.write("verified_claims_pilot.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["check", "verified_claims_pilot.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.3 verified pilot fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors, 0 warnings"),
        "expected clean summary, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_verified_claim_without_evidence() {
    let workspace = TestWorkspace::new("check-rejects-verified-missing-evidence");
    let fixture_contents =
        fs::read_to_string(fixture_path("v0_3/verified_claim_missing_evidence.adoc"))
            .expect("verified missing evidence fixture is readable");
    workspace.write("verified_claim_missing_evidence.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["check", "verified_claim_missing_evidence.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected missing-evidence verified claim to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[claim.verified_missing_evidence]"),
        "expected claim.verified_missing_evidence diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("verified_claim_missing_evidence.adoc:3:1"),
        "expected diagnostic at the ::claim open-fence, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_3_verified_claims_pilot_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-verified-pilot-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.adoc"))
        .expect("verified pilot fixture is readable");
    workspace.write("verified_claims_pilot.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["build", "verified_claims_pilot.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from verified_claims_pilot.golden.html"
    );
}

#[test]
fn build_renders_v0_3_verified_claims_pilot_to_golden_agent_json() {
    let workspace = TestWorkspace::new("build-renders-verified-pilot-golden-json");
    let fixture_contents = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.adoc"))
        .expect("verified pilot fixture is readable");
    workspace.write("verified_claims_pilot.adoc", &fixture_contents);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["build", "verified_claims_pilot.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.agent.json"))
        .expect("docs.agent.json is written");
    let golden = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.golden.agent.json"))
        .expect("golden agent JSON fixture is readable");

    assert_eq!(
        actual, golden,
        "agent JSON diverged from verified_claims_pilot.golden.agent.json"
    );
}

#[cfg(unix)]
#[test]
fn check_reports_unreadable_source_path() {
    let workspace = TestWorkspace::new("check-reports-unreadable-source-path");
    let source = workspace.write("private/guide.adoc", "# Private Guide\n\nHidden.\n");
    let mut permissions = fs::metadata(&source)
        .expect("source metadata can be read")
        .permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&source, permissions).expect("source can be made unreadable");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "check",
            workspace.root.to_str().expect("root path is utf-8"),
        ])
        .output()
        .expect("adoc check runs");

    let mut permissions = fs::metadata(&source)
        .expect("source metadata can be read")
        .permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(&source, permissions).expect("source permissions can be restored");

    assert!(
        !output.status.success(),
        "expected unreadable source to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("error[io.unreadable_file]"));
    assert!(stdout.contains(source.to_str().expect("source path is utf-8")));
    assert!(stdout.contains("1 errors"));
}

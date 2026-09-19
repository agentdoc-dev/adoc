mod support;

use std::process::Command;

use support::TestWorkspace;

fn adoc_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
    command.env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic");
    command
}

/// POSIX permits newlines, pipes, backticks, and angle brackets in filenames.
/// Markdown output must retain such a path as literal text in every layout.
#[cfg(unix)]
#[test]
fn check_markdown_keeps_malicious_filename_literal_in_every_style() {
    let workspace = TestWorkspace::new("markdown-safety-filename");
    let filename = "broken``[link](target)|<tag>\n# injected.adoc";
    workspace.write(
        filename,
        "# Broken @doc(ci.broken)\n\n::claim billing.refunds\nstatus: draft\ndepends_on: missing.object\n--\nBroken relation.\n::\n",
    );

    for style in ["compact", "table", "detailed"] {
        let output = adoc_command()
            .current_dir(&workspace.root)
            .args(["check", ".", "--format", "markdown", "--style", style])
            .output()
            .expect("adoc check runs");

        assert_eq!(output.status.code(), Some(1), "style={style}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains(filename),
            "raw filename must not inject Markdown in {style}:\n{stdout}"
        );
        assert!(
            stdout.contains(
                "broken&#96;&#96;&#91;link&#93;(target)&#124;&lt;tag&gt;&#10;# injected.adoc"
            ),
            "filename must be literal-safe in {style}:\n{stdout}"
        );
    }
}

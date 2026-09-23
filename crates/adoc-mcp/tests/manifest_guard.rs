use std::fs;
use std::path::PathBuf;

#[test]
fn production_adoc_core_dependency_does_not_enable_test_embedding_provider() {
    let manifest = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("manifest is readable");
    let dependencies = manifest_section(&manifest, "[dependencies]");
    let adoc_core_line = dependencies
        .lines()
        .find(|line| line.trim_start().starts_with("adoc-core"))
        .expect("adoc-core dependency is declared");

    assert!(
        !adoc_core_line.contains("test-embedding-provider"),
        "normal adoc-core dependency must not enable test-embedding-provider"
    );
}

#[test]
fn test_layout_guard_keeps_its_own_test_target() {
    // The guard cannot see its own `[[test]]` removed; this binary can (ADR-0068).
    // ponytail: exact-text match; reformatting the table fails loudly here, never silently.
    let manifest = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("manifest is readable");
    assert!(
        manifest.contains("[[test]]\nname = \"test_layout_guard\"\n"),
        "crates/adoc-mcp/Cargo.toml must keep `[[test]] name = \"test_layout_guard\"`"
    );
}

fn manifest_section<'a>(manifest: &'a str, heading: &str) -> &'a str {
    let start = manifest.find(heading).expect("section exists") + heading.len();
    let rest = &manifest[start..];
    rest.find("\n[").map(|end| &rest[..end]).unwrap_or(rest)
}

//! Test-layout guard (TB, ADR-0068): a crate that sets `autotests = false`
//! compiles only the test targets it declares, so an unlisted `tests/*.rs`
//! would silently never run. Every `tests/*.rs` file in such a crate must be a
//! `mod <stem>;` line (outside comments) in `tests/integration.rs` or a
//! `[[test]]` target's path; every `tests/<dir>/main.rs` must be a `[[test]]`
//! target's path (`mod <dir>;` loads `<dir>/mod.rs`, never `main.rs`). A
//! target's path is its `path`, else `tests/<name>.rs` or `tests/<name>/main.rs`.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A manifest line without its `#` comment and whitespace.
// ponytail: line scan, not a TOML parse; `#` inside a string value would cut it.
fn toml_code(line: &str) -> String {
    line.split('#')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect()
}

/// Rust source without `//` and (nested) `/* */` comments; newlines kept.
// ponytail: ignores string literals; an integration root holds only `mod` lines and comments.
fn without_comments(src: &str) -> String {
    let mut out = String::new();
    let (mut depth, mut line_comment) = (0usize, false);
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        let next = chars.peek().copied();
        if line_comment {
            line_comment = c != '\n';
        } else if c == '/' && next == Some('*') {
            chars.next();
            depth += 1;
            continue;
        } else if depth > 0 && c == '*' && next == Some('/') {
            chars.next();
            depth -= 1;
            continue;
        } else if depth == 0 && c == '/' && next == Some('/') {
            line_comment = true;
        }
        if (depth == 0 && !line_comment) || c == '\n' {
            out.push(c);
        }
    }
    out
}

fn opted_in(manifest: &str) -> bool {
    manifest.lines().any(|l| toml_code(l) == "autotests=false")
}

/// Sorted `tests/`-relative paths (`x.rs`, `<dir>/main.rs`) of the test
/// targets Cargo would auto-discover that an opted-in crate never compiles.
/// Not opted in → empty.
fn unregistered_test_files(crate_dir: &Path) -> Vec<String> {
    let manifest = fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
    if !opted_in(&manifest) {
        return Vec::new();
    }
    let root = fs::read_to_string(crate_dir.join("tests/integration.rs")).unwrap_or_default();
    let root = without_comments(&root);
    let mods: Vec<&str> = root
        .lines()
        .filter_map(|l| l.trim().strip_prefix("mod ")?.strip_suffix(';'))
        .collect();
    // Each `[[test]]` table's (name, path); `[package]`'s `name` never counts.
    let mut tables: Vec<(Option<String>, Option<String>)> = Vec::new();
    let mut in_test_table = false;
    for line in manifest.lines().map(toml_code) {
        if line.starts_with('[') {
            in_test_table = line == "[[test]]";
            if in_test_table {
                tables.push((None, None));
            }
        } else if in_test_table {
            let (name, path) = tables.last_mut().unwrap();
            let value = |key: &str| {
                line.strip_prefix(key)?
                    .strip_prefix("=\"")?
                    .strip_suffix('"')
                    .map(str::to_owned)
            };
            if let Some(v) = value("name") {
                *name = Some(v);
            } else if let Some(v) = value("path") {
                *path = Some(v);
            }
        }
    }
    // Cargo compiles a target's `path`, defaulting to tests/<name>.rs or tests/<name>/main.rs.
    // ponytail: paths compared as written; `./tests/x.rs` would not match.
    let built: Vec<String> = tables
        .into_iter()
        .flat_map(|(name, path)| match (path, name) {
            (Some(path), _) => vec![path],
            (None, Some(name)) => vec![format!("tests/{name}.rs"), format!("tests/{name}/main.rs")],
            (None, None) => Vec::new(),
        })
        .collect();
    let is_built = |rel: &str| built.iter().any(|b| *b == format!("tests/{rel}"));
    let mut missing: Vec<String> = fs::read_dir(crate_dir.join("tests"))
        .into_iter()
        .flatten()
        .map(|e| e.unwrap().path())
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            let rel = if p.is_file()
                && let Some(stem) = name.strip_suffix(".rs")
            {
                if mods.contains(&stem) {
                    return None;
                }
                name.clone()
            } else if p.join("main.rs").is_file() {
                format!("{name}/main.rs")
            } else {
                return None;
            };
            (!is_built(&rel)).then_some(rel)
        })
        .collect();
    missing.sort();
    missing
}

/// What to add so `file` (as reported by `unregistered_test_files`) compiles.
fn fix_hint(crate_name: &str, file: &str) -> String {
    let target = file
        .strip_suffix("/main.rs")
        .or((file == "integration.rs").then_some("integration"));
    match target {
        Some(target) => format!(
            "a [[test]] with name = \"{target}\", path = \"tests/{file}\" \
             to crates/{crate_name}/Cargo.toml"
        ),
        None => format!(
            "\"mod {};\" to crates/{crate_name}/tests/integration.rs",
            file.trim_end_matches(".rs")
        ),
    }
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Temp crate: `x.rs` in tests/, plus the given manifest and integration root.
fn fixture(manifest: &str, root: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("Cargo.toml"), manifest);
    write(&dir.path().join("tests/x.rs"), "");
    if let Some(root) = root {
        write(&dir.path().join("tests/integration.rs"), root);
    }
    dir
}

const OPTED_IN: &str = "[package]\nname = \"demo\"\nautotests = false\n\n[[test]]\nname = \"integration\"\npath = \"tests/integration.rs\"\n";

#[test]
fn unlisted_file_is_reported() {
    let dir = fixture(OPTED_IN, Some("//! root\n"));
    assert_eq!(unregistered_test_files(dir.path()), ["x.rs"]);
}

#[test]
fn missing_integration_root_reports_files() {
    let dir = fixture(OPTED_IN, None);
    assert_eq!(unregistered_test_files(dir.path()), ["x.rs"]);
}

#[test]
fn listed_via_mod_is_clean() {
    let dir = fixture(OPTED_IN, Some("//! root\nmod x;\n"));
    assert!(unregistered_test_files(dir.path()).is_empty());
}

#[test]
fn mod_line_inside_a_comment_is_not_credited() {
    for root in ["/*\nmod x;\n*/\n", "/* /* */\nmod x;\n*/\n", "// mod x;\n"] {
        let dir = fixture(OPTED_IN, Some(root));
        assert_eq!(unregistered_test_files(dir.path()), ["x.rs"], "{root}");
    }
    // `/*` inside a line comment opens nothing; a trailing comment keeps the line.
    let dir = fixture(OPTED_IN, Some("//! a `tests/*.rs` file\nmod x; // kept\n"));
    assert!(unregistered_test_files(dir.path()).is_empty());
}

#[test]
fn listed_via_test_table_is_clean() {
    let manifest = format!("{OPTED_IN}\n[[test]]\nname = \"x\"\n\n[dev-dependencies]\n");
    let dir = fixture(&manifest, Some("//! root\n"));
    assert!(unregistered_test_files(dir.path()).is_empty());
}

#[test]
fn test_table_credits_its_path_not_its_name() {
    // Cargo builds `path`; `name = "x"` alone says nothing about tests/x.rs.
    let manifest = format!("{OPTED_IN}\n[[test]]\nname = \"x\"\npath = \"tests/y.rs\"\n");
    let dir = fixture(&manifest, Some("//! root\n"));
    write(&dir.path().join("tests/y.rs"), "");
    assert_eq!(unregistered_test_files(dir.path()), ["x.rs"]);
}

#[test]
fn package_name_is_not_a_declaration() {
    let manifest =
        "[package]\nname = \"x\"\nautotests = false\n\n[[test]]\nname = \"integration\"\n";
    let dir = fixture(manifest, Some("//! root\n"));
    assert_eq!(unregistered_test_files(dir.path()), ["x.rs"]);
}

#[test]
fn crate_without_autotests_false_is_skipped() {
    let dir = fixture("[package]\nname = \"demo\"\n", None);
    assert!(unregistered_test_files(dir.path()).is_empty());
}

#[test]
fn unspaced_or_commented_autotests_false_is_opted_in() {
    for manifest in [
        OPTED_IN.replace("autotests = false", "autotests=false"),
        OPTED_IN.replace("autotests = false", "autotests = false # own list"),
    ] {
        let dir = fixture(&manifest, Some("//! root\n"));
        assert_eq!(unregistered_test_files(dir.path()), ["x.rs"], "{manifest}");
    }
}

#[test]
fn unspaced_or_commented_test_table_is_credited() {
    let manifest = format!("{OPTED_IN}\n[[test]] # own binary\nname=\"x\"\n");
    let dir = fixture(&manifest, Some("//! root\n"));
    assert!(unregistered_test_files(dir.path()).is_empty());
}

#[test]
fn subdirectory_files_are_ignored() {
    let dir = fixture(OPTED_IN, Some("//! root\nmod x;\n"));
    write(&dir.path().join("tests/support/mod.rs"), "");
    write(&dir.path().join("tests/fixtures/y.rs"), "");
    assert!(unregistered_test_files(dir.path()).is_empty());
}

#[test]
fn subdirectory_main_rs_is_reported() {
    let dir = fixture(OPTED_IN, Some("//! root\nmod x;\n"));
    write(&dir.path().join("tests/legacy/main.rs"), "");
    assert_eq!(unregistered_test_files(dir.path()), ["legacy/main.rs"]);
}

#[test]
fn main_rs_dir_is_credited_only_by_a_test_table() {
    // `mod legacy;` loads tests/legacy/mod.rs, never tests/legacy/main.rs.
    let dir = fixture(OPTED_IN, Some("//! root\nmod x;\nmod legacy;\n"));
    write(&dir.path().join("tests/legacy/mod.rs"), "");
    write(&dir.path().join("tests/legacy/main.rs"), "");
    assert_eq!(unregistered_test_files(dir.path()), ["legacy/main.rs"]);

    let manifest = format!("{OPTED_IN}\n[[test]]\nname = \"legacy\"\n");
    write(&dir.path().join("Cargo.toml"), &manifest);
    assert!(unregistered_test_files(dir.path()).is_empty());
}

#[test]
fn unregistered_root_is_told_to_restore_its_test_table() {
    // `mod integration;` inside tests/integration.rs would make the root its own module.
    assert_eq!(
        fix_hint("demo", "integration.rs"),
        "a [[test]] with name = \"integration\", path = \"tests/integration.rs\" \
         to crates/demo/Cargo.toml"
    );
    assert_eq!(
        fix_hint("demo", "x.rs"),
        "\"mod x;\" to crates/demo/tests/integration.rs"
    );
}

#[test]
fn guard_is_its_own_test_binary() {
    // On a `mod` line of the binary it guards, deleting that line would disable it unseen.
    assert_eq!(
        module_path!(),
        "test_layout_guard",
        "register the guard as its own [[test]] in crates/adoc-mcp/Cargo.toml, not a `mod` line"
    );
}

#[test]
fn every_opted_in_crate_registers_its_test_files() {
    let crates = repo_root().join("crates");
    let mut opted = 0;
    let mut failures = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&crates)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for crate_dir in entries {
        let Ok(manifest) = fs::read_to_string(crate_dir.join("Cargo.toml")) else {
            continue;
        };
        if opted_in(&manifest) {
            opted += 1;
        }
        let name = crate_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for file in unregistered_test_files(&crate_dir) {
            failures.push(format!(
                "crates/{name}/tests/{file} is not compiled (autotests = false): add {}",
                fix_hint(&name, &file)
            ));
        }
    }
    assert!(
        opted > 0,
        "no crate sets `autotests = false`; guard is vacuous"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

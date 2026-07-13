use std::fs;
use std::path::PathBuf;

fn source(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn use_case_facade_delegates_to_focused_command_modules() {
    let facade = source("use_cases/mod.rs");

    for module in ["project", "queries", "changes", "shared"] {
        assert!(
            facade.contains(&format!("mod {module};")),
            "use-case facade must declare the `{module}` command module"
        );
        let handler = source(&format!("use_cases/{module}.rs"));
        assert!(
            !handler.trim().is_empty(),
            "`{module}` command module must own behavior"
        );
    }

    assert!(
        !facade.contains("fn init_with_context"),
        "facade must contain contracts and delegation, not command implementations"
    );
    assert!(
        !facade.contains("fn resolve_graph_artifact_for_read"),
        "shared path and artifact resolution belongs outside the facade"
    );
}

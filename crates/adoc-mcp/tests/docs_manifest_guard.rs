//! Docs-truth guard (ADR-0041): the tool and kind lists published in
//! `README.md` and `docs/guides/mcp-agent-gateway.md` are asserted against the code
//! registry — set-equality on names, so a failure says which name drifted.
//! The parse targets pinned HTML comment anchors, never free prose.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use adoc_mcp::AgentDocMcpServer;

fn read_repo_doc(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

/// Extracts the codespan names from the bulleted list between
/// `<!-- {anchor} -->` and `<!-- /{anchor} -->`, failing loudly when an
/// anchor is missing or a line inside is not a `- `name`` bullet.
fn anchored_list(doc: &str, doc_name: &str, anchor: &str) -> BTreeSet<String> {
    let open = format!("<!-- {anchor} -->");
    let close = format!("<!-- /{anchor} -->");
    let start = doc.find(&open).unwrap_or_else(|| {
        panic!(
            "{doc_name} is missing the `{open}` anchor required by the docs-truth guard (ADR-0041)"
        )
    }) + open.len();
    let end = doc[start..].find(&close).unwrap_or_else(|| {
        panic!("{doc_name} is missing the closing `{close}` anchor required by the docs-truth guard (ADR-0041)")
    }) + start;

    let mut names = BTreeSet::new();
    for line in doc[start..end].lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        let name = line
            .strip_prefix("- `")
            .and_then(|rest| rest.strip_suffix('`'))
            .unwrap_or_else(|| {
                panic!(
                    "{doc_name}: the `{anchor}` anchored list must contain only \
                     `- `name`` bullets, found: {line:?}"
                )
            });
        names.insert(name.to_string());
    }
    assert!(
        !names.is_empty(),
        "{doc_name}: the `{anchor}` anchored list is empty"
    );
    names
}

fn registered_tool_names() -> BTreeSet<String> {
    AgentDocMcpServer::tool_router()
        .list_all()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect()
}

fn assert_matches_registry(
    doc_name: &str,
    what: &str,
    published: &BTreeSet<String>,
    registered: &BTreeSet<String>,
) {
    let missing: Vec<_> = registered.difference(published).cloned().collect();
    let extra: Vec<_> = published.difference(registered).cloned().collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{doc_name} {what} list drifted from the code registry — \
         missing from doc: [{}], not in registry: [{}]",
        missing.join(", "),
        extra.join(", ")
    );
}

#[test]
fn readme_mcp_tool_list_matches_registry() {
    let published = anchored_list(&read_repo_doc("README.md"), "README.md", "adoc:mcp-tools");
    assert_matches_registry(
        "README.md",
        "MCP tool",
        &published,
        &registered_tool_names(),
    );
}

#[test]
fn gateway_doc_mcp_tool_list_matches_registry() {
    let published = anchored_list(
        &read_repo_doc("docs/guides/mcp-agent-gateway.md"),
        "docs/guides/mcp-agent-gateway.md",
        "adoc:mcp-tools",
    );
    assert_matches_registry(
        "docs/guides/mcp-agent-gateway.md",
        "MCP tool",
        &published,
        &registered_tool_names(),
    );
}

#[test]
fn readme_kind_list_matches_block_kinds() {
    let published = anchored_list(&read_repo_doc("README.md"), "README.md", "adoc:kinds");
    let shipped: BTreeSet<String> = adoc_core::block_kind_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_matches_registry("README.md", "object kind", &published, &shipped);
}

use std::fmt::Write as FmtWrite;
use std::path::PathBuf;

use adoc_core::{
    GraphDirection, GraphRelationKind, GraphTraversalEdge, GraphTraversalEnvelope,
    GraphTraversalNode, GraphTraversalResult,
};
use adoc_local::{GraphInput as LocalGraphInput, LocalContext, UnrestrictedPathPolicy};

use crate::presentation::ResolvedFormat;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;

use super::{current_dir, emit_envelope_error, eprint_diagnostics, report, write_json_or_report};

pub(crate) struct GraphCommandInput {
    pub(crate) object_id: String,
    pub(crate) artifact: Option<PathBuf>,
    pub(crate) relation: Option<GraphRelationKind>,
    pub(crate) direction: Option<GraphDirection>,
}

pub(crate) fn graph(input: GraphCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.graph(LocalGraphInput {
        object_id: input.object_id,
        artifact: input.artifact,
        relation: input.relation,
        direction: input.direction,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let exit_code = outcome.exit_code;
    if exit_code != 0 {
        let envelope = outcome.envelope;
        return emit_envelope_error(&envelope, &envelope.diagnostics, resolved, exit_code);
    }
    if resolved != ResolvedFormat::Json && !outcome.envelope.diagnostics.is_empty() {
        eprint_diagnostics(&outcome.envelope.diagnostics);
    }
    let result = GraphTraversalResult {
        root: outcome.envelope.root,
        nodes: outcome.envelope.nodes,
        edges: outcome.envelope.edges,
        diagnostics: outcome.envelope.diagnostics,
    };
    match resolved {
        ResolvedFormat::Json => {
            write_json_or_report(&GraphTraversalEnvelope::from(result), exit_code)
        }
        ResolvedFormat::Plain => write_graph_text(&result, false),
        ResolvedFormat::Styled => write_graph_text(&result, true),
        ResolvedFormat::Markdown => {
            unreachable!("main.rs rejects markdown format for `adoc graph` before dispatch")
        }
    }
}

fn write_graph_text(result: &GraphTraversalResult, styled: bool) -> i32 {
    let mut output = String::new();
    render_graph_text(&mut output, result, styled);
    print!("{output}");
    0
}

fn render_graph_text(output: &mut String, result: &GraphTraversalResult, styled: bool) {
    if styled {
        writeln!(output, "{} {}", faint_label("Root:"), result.root)
            .expect("writing to String cannot fail");
        writeln!(output, "{}", faint_label("Nodes:")).expect("writing to String cannot fail");
    } else {
        writeln!(output, "Root: {}", result.root).expect("writing to String cannot fail");
        writeln!(output, "Nodes:").expect("writing to String cannot fail");
    }
    for node in &result.nodes {
        render_node(output, node, styled);
    }

    if styled {
        writeln!(output, "{}", faint_label("Edges:")).expect("writing to String cannot fail");
    } else {
        writeln!(output, "Edges:").expect("writing to String cannot fail");
    }
    if result.edges.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
    } else {
        for edge in &result.edges {
            render_edge(output, edge, styled);
        }
    }
}

fn render_node(output: &mut String, node: &GraphTraversalNode, styled: bool) {
    let status = node
        .status
        .as_ref()
        .map(|status| format!(", {status}"))
        .unwrap_or_default();
    if styled {
        writeln!(
            output,
            "- {} ({} {}, {} {}{})",
            node.id,
            cyan_key("distance"),
            node.distance,
            cyan_key("kind"),
            node.kind,
            status
        )
        .expect("writing to String cannot fail");
    } else {
        writeln!(
            output,
            "- {} (distance {}, {}{})",
            node.id, node.distance, node.kind, status
        )
        .expect("writing to String cannot fail");
    }
}

fn render_edge(output: &mut String, edge: &GraphTraversalEdge, styled: bool) {
    let revisit = if edge.revisit { " (revisit)" } else { "" };
    if styled {
        writeln!(
            output,
            "- {} --{}--> {}{}",
            edge.source,
            cyan_key(edge.relation.as_str()),
            edge.target,
            revisit
        )
        .expect("writing to String cannot fail");
    } else {
        writeln!(
            output,
            "- {} --{}--> {}{}",
            edge.source, edge.relation, edge.target, revisit
        )
        .expect("writing to String cannot fail");
    }
}

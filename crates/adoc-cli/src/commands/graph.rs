use std::fmt::Write as FmtWrite;
use std::io;
use std::path::PathBuf;

use adoc_core::{
    Diagnostic, DiagnosticCode, GraphDirection, GraphInput, GraphRelationKind, GraphTraversalEdge,
    GraphTraversalEnvelope, GraphTraversalNode, GraphTraversalQuery, GraphTraversalResult,
    Severity, load_graph_session, traverse_graph,
};

use crate::error::CliError;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;
use crate::presentation::{ResolvedFormat, json as json_presentation};

use super::{
    diagnostics_have_errors, discover_project_config_if, eprint_diagnostics, merge_diagnostics,
    report, resolve_agent_artifact_path_with_config, resolve_graph_artifact_path_with_config,
};

pub(crate) struct GraphCommandInput {
    pub(crate) object_id: String,
    pub(crate) artifact: Option<PathBuf>,
    pub(crate) agent_artifact: Option<PathBuf>,
    pub(crate) relation: Option<GraphRelationKind>,
    pub(crate) direction: Option<GraphDirection>,
}

pub(crate) fn graph(input: GraphCommandInput, resolved: ResolvedFormat) -> i32 {
    let config = match discover_project_config_if(
        input.artifact.is_none() || input.agent_artifact.is_none(),
    ) {
        Ok(config) => config,
        Err(error) => return report(error),
    };
    let graph_artifact = resolve_graph_artifact_path_with_config(input.artifact, config.as_ref());
    let agent_artifact =
        resolve_agent_artifact_path_with_config(input.agent_artifact, config.as_ref());

    let load_result = load_graph_session(GraphInput {
        agent_artifact_path: agent_artifact,
        graph_artifact_path: graph_artifact,
    });
    let mut diagnostics = load_result.diagnostics;
    let session = match load_result.session {
        Some(session) => session,
        None => {
            let exit_code = graph_exit_code_for_diagnostics(&diagnostics);
            return emit_graph_error(&input.object_id, diagnostics, resolved, exit_code);
        }
    };

    if diagnostics_have_errors(&diagnostics) {
        let exit_code = graph_exit_code_for_diagnostics(&diagnostics);
        return emit_graph_error(&input.object_id, diagnostics, resolved, exit_code);
    }

    let traversal = traverse_graph(
        &session,
        GraphTraversalQuery {
            root_id: input.object_id.clone(),
            direction: input.direction.unwrap_or_default(),
            relations: input.relation.into_iter().collect(),
        },
    );
    diagnostics = merge_diagnostics(diagnostics, traversal.diagnostics);
    let exit_code = graph_exit_code_for_diagnostics(&diagnostics);
    if exit_code != 0 {
        return emit_graph_error(&input.object_id, diagnostics, resolved, exit_code);
    }

    if resolved != ResolvedFormat::Json && !diagnostics.is_empty() {
        eprint_diagnostics(&diagnostics);
    }

    let result = GraphTraversalResult {
        root: traversal.root,
        nodes: traversal.nodes,
        edges: traversal.edges,
        diagnostics,
    };
    match resolved {
        ResolvedFormat::Json => write_graph_json(GraphTraversalEnvelope::from(result), exit_code),
        ResolvedFormat::Plain => write_graph_text(&result, false),
        ResolvedFormat::Styled => write_graph_text(&result, true),
    }
}

fn emit_graph_error(
    root: &str,
    diagnostics: Vec<Diagnostic>,
    resolved: ResolvedFormat,
    exit_code: i32,
) -> i32 {
    if resolved == ResolvedFormat::Json {
        return write_graph_json(
            GraphTraversalEnvelope::new(root.to_string(), Vec::new(), Vec::new(), diagnostics),
            exit_code,
        );
    }
    eprint_diagnostics(&diagnostics);
    exit_code
}

fn write_graph_json(envelope: GraphTraversalEnvelope, exit_code: i32) -> i32 {
    json_presentation::write_json(&envelope, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
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

fn graph_exit_code_for_diagnostics(diagnostics: &[Diagnostic]) -> i32 {
    diagnostics
        .iter()
        .filter_map(graph_diagnostic_exit_code)
        .min()
        .unwrap_or(0)
}

fn graph_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::IdInvalid, _) => Some(1),
        (DiagnosticCode::GraphObjectNotFound, _) => Some(3),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

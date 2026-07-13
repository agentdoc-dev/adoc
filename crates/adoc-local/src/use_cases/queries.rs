use std::time::{Duration, Instant};

use adoc_core::{
    Diagnostic, DiagnosticCode, EmbeddingProviderSelection, GraphTraversalEnvelope,
    GraphTraversalQuery, GraphTraversalResult, RelPath, RetrievalEntry, RetrievalEnvelope,
    RetrievalInput, RetrievalLoadResult, RetrievalRecord, SearchFilters, SearchMode, SearchQuery,
    Severity, changed_files_from_git, changed_paths_strings, embed_query_with_embedding_provider,
    empty_contradictions_envelope, empty_impacted_envelope, empty_stale_envelope,
    evaluate_contradictions, evaluate_impacted, evaluate_stale,
    load_retrieval_session_with_embedding_provider, search as core_search, traverse_graph,
    validate_changed_paths, why_object,
};

use super::shared::{
    diagnostics_have_errors, discover_project_config_if, exit_code_for_diagnostics,
    load_graph_session_for_query, merge_diagnostics, resolve_embedding_provider_selection,
    resolve_graph_artifact_for_read, resolve_graph_artifact_path_with_config,
    resolve_search_artifact_path_with_config,
};
use super::{
    ContradictionsInput, ContradictionsOutcome, GraphInput, GraphOutcome, ImpactedChangedSet,
    ImpactedInput, ImpactedOutcome, ResolvedRetrievalRecord, ResolvedSearchEntry, SearchInput,
    SearchOutcome, StaleInput, StaleOutcome, WhyInput, WhyOutcome,
};
use crate::{LocalContext, LocalError, PathPolicy};

pub(super) fn why_with_context<P>(
    context: &LocalContext<P>,
    input: WhyInput,
) -> Result<WhyOutcome, LocalError>
where
    P: PathPolicy,
{
    let artifact = resolve_graph_artifact_for_read(context, input.artifact.as_deref())?;
    let load_result = load_retrieval_session_with_embedding_provider(
        RetrievalInput {
            artifact_path: artifact.clone(),
            search_artifact_path: None,
        },
        EmbeddingProviderSelection::Local,
    );
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let load_exit_code = why_exit_code_for_diagnostics(&load_diagnostics);
    let Some(session) = session.filter(|_| !diagnostics_have_errors(&load_diagnostics)) else {
        return Ok(WhyOutcome {
            artifact,
            records: Vec::new(),
            diagnostics: load_diagnostics,
            duration: Duration::ZERO,
            exit_code: load_exit_code,
        });
    };

    let started = Instant::now();
    let why_result = why_object(&session, &input.object_id);
    let duration = started.elapsed();
    let diagnostics = merge_diagnostics(load_diagnostics, why_result.diagnostics);
    let exit_code = why_exit_code_for_diagnostics(&diagnostics);
    let records = why_result
        .records
        .into_iter()
        .map(|record| resolved_record(&session, record))
        .collect();

    Ok(WhyOutcome {
        artifact,
        records,
        diagnostics,
        duration,
        exit_code,
    })
}

pub(super) fn graph_with_context<P>(
    context: &LocalContext<P>,
    input: GraphInput,
) -> Result<GraphOutcome, LocalError>
where
    P: PathPolicy,
{
    let graph_artifact = resolve_graph_artifact_for_read(context, input.artifact.as_deref())?;
    let (session, mut diagnostics) = load_graph_session_for_query(graph_artifact);
    let Some(session) = session else {
        let exit_code = graph_exit_code_for_diagnostics(&diagnostics);
        return Ok(GraphOutcome {
            envelope: GraphTraversalEnvelope::new(
                input.object_id,
                Vec::new(),
                Vec::new(),
                diagnostics,
            ),
            exit_code,
        });
    };

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
    let result = GraphTraversalResult {
        root: traversal.root,
        nodes: traversal.nodes,
        edges: traversal.edges,
        diagnostics,
    };

    Ok(GraphOutcome {
        envelope: GraphTraversalEnvelope::from(result),
        exit_code,
    })
}

pub(super) fn stale_with_context<P>(
    context: &LocalContext<P>,
    input: StaleInput,
) -> Result<StaleOutcome, LocalError>
where
    P: PathPolicy,
{
    let graph_artifact = resolve_graph_artifact_for_read(context, input.artifact.as_deref())?;
    let (session, diagnostics) = load_graph_session_for_query(graph_artifact);
    let Some(session) = session else {
        let exit_code = signal_query_exit_code(&diagnostics);
        return Ok(StaleOutcome {
            envelope: empty_stale_envelope(diagnostics),
            exit_code,
        });
    };

    let envelope = evaluate_stale(&session, input.within_days, diagnostics);
    let exit_code = signal_query_exit_code(&envelope.diagnostics);
    Ok(StaleOutcome {
        envelope,
        exit_code,
    })
}

pub(super) fn contradictions_with_context<P>(
    context: &LocalContext<P>,
    input: ContradictionsInput,
) -> Result<ContradictionsOutcome, LocalError>
where
    P: PathPolicy,
{
    let graph_artifact = resolve_graph_artifact_for_read(context, input.artifact.as_deref())?;
    let (session, diagnostics) = load_graph_session_for_query(graph_artifact);
    let Some(session) = session else {
        let exit_code = signal_query_exit_code(&diagnostics);
        return Ok(ContradictionsOutcome {
            envelope: empty_contradictions_envelope(diagnostics),
            exit_code,
        });
    };

    let envelope = evaluate_contradictions(&session, input.all, diagnostics);
    let exit_code = signal_query_exit_code(&envelope.diagnostics);
    Ok(ContradictionsOutcome {
        envelope,
        exit_code,
    })
}

pub(super) fn impacted_with_context<P>(
    context: &LocalContext<P>,
    input: ImpactedInput,
) -> Result<ImpactedOutcome, LocalError>
where
    P: PathPolicy,
{
    // Resolve the changed set before touching the artifact so input errors
    // short-circuit deterministically (the envelope still ships, ADR-0038).
    //
    // The git derivation deliberately skips `PathPolicy::resolve_read_path`
    // (unlike every artifact read below): git discovers the repository by
    // walking up from `config_start` to `.git` itself, and git history is
    // not a filesystem read in the policy sense. If a future policy needs
    // to gate "read git state outside the policy root", this is the seam.
    let changed = match &input.changed {
        ImpactedChangedSet::Paths(paths) => validate_changed_paths(paths),
        ImpactedChangedSet::GitRef(base_ref) => {
            changed_files_from_git(context.config_start().to_path_buf(), base_ref)
        }
    };
    let changed: Vec<RelPath> = match changed {
        Ok(changed) => changed,
        Err(diagnostics) => {
            let exit_code = impacted_exit_code(&diagnostics);
            return Ok(ImpactedOutcome {
                envelope: empty_impacted_envelope(Vec::new(), diagnostics),
                exit_code,
            });
        }
    };

    let graph_artifact = resolve_graph_artifact_for_read(context, input.artifact.as_deref())?;
    let (session, diagnostics) = load_graph_session_for_query(graph_artifact);
    let Some(session) = session else {
        let exit_code = impacted_exit_code(&diagnostics);
        return Ok(ImpactedOutcome {
            envelope: empty_impacted_envelope(changed_paths_strings(&changed), diagnostics),
            exit_code,
        });
    };

    let envelope = evaluate_impacted(&session, &changed, diagnostics);
    let exit_code = impacted_exit_code(&envelope.diagnostics);
    Ok(ImpactedOutcome {
        envelope,
        exit_code,
    })
}

pub(super) fn search_with_context<P>(
    context: &LocalContext<P>,
    input: SearchInput,
) -> Result<SearchOutcome, LocalError>
where
    P: PathPolicy,
{
    let requested_mode = if input.semantic {
        SearchMode::Semantic
    } else if input.lexical {
        SearchMode::Lexical
    } else {
        SearchMode::Hybrid
    };

    let artifact = input
        .artifact
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let search_artifact = input
        .search_artifact
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let needs_search_config = matches!(requested_mode, SearchMode::Hybrid | SearchMode::Semantic)
        && search_artifact.is_none();
    let config = discover_project_config_if(
        artifact.is_none() || needs_search_config,
        context.config_start(),
    )?;
    let embedding_provider = resolve_embedding_provider_selection(config.as_ref());
    let artifact = resolve_graph_artifact_path_with_config(artifact, config.as_ref());
    let artifact = context.path_policy().resolve_read_path(&artifact)?;
    let search_artifact_path = match requested_mode {
        SearchMode::Lexical => None,
        SearchMode::Hybrid | SearchMode::Semantic => {
            let path = resolve_search_artifact_path_with_config(search_artifact, config.as_ref());
            Some(context.path_policy().resolve_read_path(&path)?)
        }
    };
    let load_result = load_retrieval_session_with_embedding_provider(
        RetrievalInput {
            artifact_path: artifact,
            search_artifact_path,
        },
        embedding_provider,
    );
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let Some(session) = session.filter(|_| !diagnostics_have_errors(&load_diagnostics)) else {
        return Ok(search_outcome(Vec::new(), load_diagnostics, 2));
    };

    if requested_mode == SearchMode::Semantic && !session.has_semantic_index() {
        let mut diagnostics = load_diagnostics;
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::SearchArtifactMissing,
            severity: Severity::Error,
            message: "Semantic search requested but no search artifact is loaded.".to_string(),
            span: None,
            object_id: None,
            help: Some(
                DiagnosticCode::SearchArtifactMissing
                    .default_help()
                    .to_string(),
            ),
        });
        return Ok(search_outcome(Vec::new(), diagnostics, 2));
    }

    let mode = match requested_mode {
        SearchMode::Hybrid if session.has_semantic_index() => SearchMode::Hybrid,
        SearchMode::Hybrid => SearchMode::Lexical,
        mode => mode,
    };
    let needs_query_vector = matches!(mode, SearchMode::Hybrid | SearchMode::Semantic);
    let query_vector = if needs_query_vector {
        match embed_query_with_embedding_provider(&input.query, embedding_provider) {
            Ok(vector) => Some(vector),
            Err(embed_error) => {
                let mode_label = match mode {
                    SearchMode::Hybrid => "hybrid search",
                    SearchMode::Semantic => "semantic search",
                    SearchMode::Lexical => "lexical search",
                };
                let (code, message) = match &embed_error {
                    adoc_core::EmbedQueryError::ModelLoad(msg) => (
                        DiagnosticCode::EmbedModelLoadFailed,
                        format!("{mode_label} requested but embedding model failed to load: {msg}"),
                    ),
                    adoc_core::EmbedQueryError::Compute(msg) => (
                        DiagnosticCode::EmbedComputeFailed,
                        format!("{mode_label} requested but query embedding failed: {msg}"),
                    ),
                };
                let diagnostic = Diagnostic {
                    code,
                    severity: Severity::Error,
                    message,
                    span: None,
                    object_id: None,
                    help: Some(code.default_help().to_string()),
                };
                return Ok(search_outcome(Vec::new(), vec![diagnostic], 2));
            }
        }
    } else {
        None
    };

    let search_result = core_search(
        &session,
        SearchQuery {
            text: input.query,
            mode,
            filters: SearchFilters {
                kind: input.kind,
                status: input.status,
                owner: input.owner,
                source_path: input.source_path,
                related_to: input.related_to,
                relation: input.relation,
                direction: input.direction,
            },
            top: input.top,
            query_vector,
            scope: input.scope,
        },
    );
    let diagnostics = merge_diagnostics(load_diagnostics, search_result.diagnostics);
    let exit_code = search_exit_code(&diagnostics);
    let records = search_result
        .records
        .into_iter()
        .map(|entry| match entry {
            RetrievalEntry::KnowledgeObject(record) => {
                ResolvedSearchEntry::KnowledgeObject(resolved_record(&session, record))
            }
            RetrievalEntry::Prose(record) => ResolvedSearchEntry::Prose(record),
        })
        .collect::<Vec<_>>();

    Ok(search_outcome(records, diagnostics, exit_code))
}

fn resolved_record(
    session: &adoc_core::RetrievalSession,
    record: RetrievalRecord,
) -> ResolvedRetrievalRecord {
    let related_statuses = session.related_statuses(&record);
    ResolvedRetrievalRecord {
        record,
        related_statuses,
    }
}

fn search_outcome(
    records: Vec<ResolvedSearchEntry>,
    diagnostics: Vec<Diagnostic>,
    exit_code: i32,
) -> SearchOutcome {
    let envelope = RetrievalEnvelope::new(
        records
            .iter()
            .map(|resolved| match resolved {
                ResolvedSearchEntry::KnowledgeObject(resolved) => {
                    RetrievalEntry::KnowledgeObject(resolved.record.clone())
                }
                ResolvedSearchEntry::Prose(record) => RetrievalEntry::Prose(record.clone()),
            })
            .collect(),
        diagnostics.clone(),
    );
    SearchOutcome {
        envelope,
        records,
        diagnostics,
        exit_code,
    }
}

fn why_exit_code_for_diagnostics(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, why_diagnostic_exit_code)
}

fn why_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::IdInvalid, _) => Some(1),
        (DiagnosticCode::RetrievalObjectNotFound, _) => Some(3),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

fn graph_exit_code_for_diagnostics(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, graph_diagnostic_exit_code)
}

fn graph_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::IdInvalid, _) => Some(1),
        (DiagnosticCode::GraphObjectNotFound, _) => Some(3),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

/// Lifecycle-signal queries (`adoc stale`, `adoc contradictions`) are queries,
/// not gates: records never affect the exit code; only artifact-load errors do.
fn signal_query_exit_code(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, signal_query_diagnostic_exit_code)
}

fn signal_query_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match diagnostic.severity {
        Severity::Error => Some(2),
        _ => None,
    }
}

/// V6.3 exit-code split: user-input errors (invalid path argument,
/// unresolvable `--ref`) exit 1; environment errors (git unavailable,
/// artifact load failure) exit 2; findings never affect the exit code.
fn impacted_exit_code(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, impacted_diagnostic_exit_code)
}

fn impacted_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::ImpactedInvalidPath | DiagnosticCode::ImpactedRefUnresolvable, _) => {
            Some(1)
        }
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

fn search_exit_code(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, search_diagnostic_exit_code)
}

fn search_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::SearchInvalidFilter, _) => Some(1),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

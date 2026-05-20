use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{GraphDirection, GraphRelationKind};
use adoc_local::{
    LocalContext, SearchInput as LocalSearchInput, SearchUseCase, UnrestrictedPathPolicy,
};

use crate::error::CliError;
use crate::presentation::{ResolvedFormat, RetrievalView, make_presenter};

use super::{
    current_dir, emit_retrieval_error, eprint_diagnostics, presentation_record_from_resolved,
    report,
};

pub(crate) struct SearchCommandInput {
    pub(crate) query: String,
    pub(crate) artifact: Option<PathBuf>,
    pub(crate) search_artifact: Option<PathBuf>,
    pub(crate) semantic: bool,
    pub(crate) lexical: bool,
    pub(crate) kind: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) source_path: Option<String>,
    pub(crate) related_to: Option<String>,
    pub(crate) relation: Option<GraphRelationKind>,
    pub(crate) direction: Option<GraphDirection>,
    pub(crate) top: NonZeroUsize,
}

pub(crate) fn search_command(input: SearchCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match SearchUseCase::new(context).run(LocalSearchInput {
        query: input.query,
        artifact: input.artifact,
        search_artifact: input.search_artifact,
        semantic: input.semantic,
        lexical: input.lexical,
        kind: input.kind,
        status: input.status,
        owner: input.owner,
        source_path: input.source_path,
        related_to: input.related_to,
        relation: input.relation,
        direction: input.direction,
        top: input.top,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };

    if resolved == ResolvedFormat::Json {
        let presenter = make_presenter(ResolvedFormat::Json, Vec::new());
        let view = RetrievalView {
            records: outcome
                .records
                .into_iter()
                .map(|record| presentation_record_from_resolved(record, false))
                .collect(),
            diagnostics: outcome.diagnostics,
            footer: None,
        };
        return presenter
            .present(&view, &mut std::io::stdout())
            .map_or_else(
                |source| report(CliError::RetrievalIo { source }),
                |()| outcome.exit_code,
            );
    }

    if outcome.exit_code != 0 {
        return emit_retrieval_error(outcome.diagnostics, resolved, outcome.exit_code);
    }

    if !outcome.diagnostics.is_empty() {
        eprint_diagnostics(&outcome.diagnostics);
    }
    if outcome.records.is_empty() {
        println!("(no matches)");
        return 0;
    }

    let view = RetrievalView {
        records: outcome
            .records
            .into_iter()
            .map(|record| presentation_record_from_resolved(record, false))
            .collect(),
        diagnostics: outcome.diagnostics,
        footer: None,
    };
    let presenter = make_presenter(resolved, Vec::new());
    presenter
        .present(&view, &mut std::io::stdout())
        .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 0)
}

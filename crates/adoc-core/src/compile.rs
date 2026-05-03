use std::fs;
use std::path::{Path, PathBuf};

use crate::artifact::agent_json::AgentJsonDocument;
use crate::ast::WorkspaceAst;
use crate::diagnostic::{Diagnostic, Severity};
use crate::parser::parse_page;
use crate::render::html::render_html;
use crate::source::SourceFile;

#[derive(Debug, Clone)]
pub struct CompileInput {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CompileResult {
    pub diagnostics: Vec<Diagnostic>,
    pub artifacts: Option<BuildArtifacts>,
}

impl CompileResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

#[derive(Debug, Clone)]
pub struct BuildArtifacts {
    pub html: String,
    pub agent_json: AgentJsonDocument,
}

pub fn compile_workspace(input: CompileInput) -> CompileResult {
    let mut diagnostics = Vec::new();
    let mut pages = Vec::new();

    for path in source_paths(&input.root) {
        match fs::read_to_string(&path) {
            Ok(text) => {
                let source = SourceFile::new(path, text);
                let (page, page_diagnostics) = parse_page(&source);
                diagnostics.extend(page_diagnostics);
                pages.push(page);
            }
            Err(error) => diagnostics.push(Diagnostic::error(
                "io.unreadable_file",
                format!("could not read AgentDoc Source: {error}"),
            )),
        }
    }

    let _workspace = WorkspaceAst {
        pages: pages.clone(),
    };

    let artifacts = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Error)
        .then(|| BuildArtifacts {
            html: render_html(&pages),
            agent_json: AgentJsonDocument::from_pages_and_diagnostics(&pages, &diagnostics),
        });

    CompileResult {
        diagnostics,
        artifacts,
    }
}

fn source_paths(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }

    let mut paths = Vec::new();
    collect_adoc_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_adoc_files(directory: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        paths.push(directory.to_path_buf());
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_adoc_files(&path, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "adoc")
        {
            paths.push(path);
        }
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use crate::artifact::agent_json::AgentJsonDocument;
use crate::ast::WorkspaceAst;
use crate::diagnostic::{Diagnostic, Severity};
use crate::parser::parse_page;
use crate::render::html::render_html;
use crate::source::SourceFile;

#[derive(Debug, Clone)]
struct SourcePath {
    path: PathBuf,
    identity_path: PathBuf,
}

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

    for source_path in source_paths(&input.root) {
        match fs::read_to_string(&source_path.path) {
            Ok(text) => {
                let source = SourceFile::new_with_identity_path(
                    source_path.path,
                    text,
                    source_path.identity_path,
                );
                let (page, page_diagnostics) = parse_page(&source);
                diagnostics.extend(page_diagnostics);
                pages.push(page);
            }
            Err(error) => diagnostics.push(Diagnostic::error(
                "io.unreadable_file",
                format!(
                    "could not read AgentDoc Source {}: {error}",
                    source_path.path.display()
                ),
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

fn source_paths(root: &Path) -> Vec<SourcePath> {
    if root.is_file() {
        return vec![SourcePath {
            path: root.to_path_buf(),
            identity_path: root
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| root.into()),
        }];
    }

    let mut paths = Vec::new();
    collect_adoc_files(root, root, &mut paths);
    paths.sort_by(|left, right| left.path.cmp(&right.path));
    paths
}

fn collect_adoc_files(root: &Path, directory: &Path, paths: &mut Vec<SourcePath>) {
    let Ok(entries) = fs::read_dir(directory) else {
        paths.push(SourcePath {
            path: directory.to_path_buf(),
            identity_path: directory
                .strip_prefix(root)
                .map(PathBuf::from)
                .unwrap_or_else(|_| directory.into()),
        });
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_adoc_files(root, &path, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "adoc")
        {
            let identity_path = path
                .strip_prefix(root)
                .map(PathBuf::from)
                .unwrap_or_else(|_| path.clone());
            paths.push(SourcePath {
                path,
                identity_path,
            });
        }
    }
}

use std::path::PathBuf;

use adoc_core::{CompileInput, compile_workspace};

use super::{
    discover_project_config_if, print_diagnostics, print_summary, report,
    resolve_docs_path_with_config,
};

pub(crate) fn check(path: Option<PathBuf>) -> i32 {
    let config = match discover_project_config_if(path.is_none()) {
        Ok(config) => config,
        Err(error) => return report(error),
    };
    let path = match resolve_docs_path_with_config(path, config.as_ref()) {
        Ok(path) => path,
        Err(error) => return report(error),
    };

    let result = compile_workspace(CompileInput { root: path });
    print_diagnostics(&result.diagnostics);
    print_summary(&result.diagnostics);

    if result.has_errors() { 1 } else { 0 }
}

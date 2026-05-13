use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::CliError;

use super::report;

const INIT_CONFIG_PATH: &str = "agentdoc.config.yaml";
const INIT_INDEX_PATH: &str = "docs/index.adoc";
const INIT_CONFIG_TEMPLATE: &str = "\
version: 1
mode: strict
docs_path: docs
outputs:
  dir: dist
embeddings:
  provider: local
";

pub(crate) fn init() -> i32 {
    match write_init_files() {
        Ok(()) => {
            println!("Created {INIT_CONFIG_PATH} and {INIT_INDEX_PATH}");
            println!("Next: adoc check");
            0
        }
        Err(error) => report(error),
    }
}

fn init_index_template() -> &'static str {
    "\
# AgentDoc Project @doc(project.index)

This project was initialized with AgentDoc.

::claim project.initialized
status: draft
--
The project has an initialized AgentDoc source tree.
::
"
}

fn write_init_files() -> Result<(), CliError> {
    let config_path = PathBuf::from(INIT_CONFIG_PATH);
    let index_path = PathBuf::from(INIT_INDEX_PATH);

    for target in [&config_path, &index_path] {
        if target.exists() {
            return Err(CliError::InitTargetExists {
                path: target.to_path_buf(),
            });
        }
    }

    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::CreateOutputDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let index_template = init_index_template();
    write_new_file(&config_path, INIT_CONFIG_TEMPLATE.as_bytes())?;
    if let Err(error) = write_new_file(&index_path, index_template.as_bytes()) {
        cleanup_init_paths([&config_path]);
        return Err(error);
    }

    Ok(())
}

fn write_new_file(path: &Path, contents: &[u8]) -> Result<(), CliError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                CliError::InitTargetExists {
                    path: path.to_path_buf(),
                }
            } else {
                CliError::WriteFailed {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?;

    if let Err(source) = file.write_all(contents) {
        cleanup_init_paths([path]);
        return Err(CliError::WriteFailed {
            path: path.to_path_buf(),
            source,
        });
    }

    Ok(())
}

fn cleanup_init_paths<P: AsRef<Path>>(paths: impl IntoIterator<Item = P>) {
    for path in paths {
        let _ = fs::remove_file(path.as_ref());
    }
}

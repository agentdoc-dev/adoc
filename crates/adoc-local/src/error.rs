use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum LocalError {
    #[error("error[io.current_dir] could not read current directory: {source}")]
    CurrentDir {
        #[source]
        source: std::io::Error,
    },

    #[error("error[init.already_exists] target already exists: {}", path.display())]
    InitTargetExists { path: PathBuf },

    #[error("error[config.read] could not read config {}: {source}", path.display())]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("error[config.parse] could not parse config {}: {source}", path.display())]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: Box<serde_saphyr::Error>,
    },

    #[error("error[config.invalid] invalid config {}: {message}", path.display())]
    ConfigInvalid { path: PathBuf, message: String },

    #[error("error[config.missing] {message}{}", format_config_path(config_path))]
    ConfigMissing {
        message: String,
        config_path: Option<PathBuf>,
    },

    #[error("error[io.output_not_directory] output path exists as a file: {}", path.display())]
    OutputPathIsFile { path: PathBuf },

    #[error("error[io.output_not_directory] could not create output directory {}: {source}", path.display())]
    CreateOutputDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("error[io.write_failed] could not write {}: {source}", path.display())]
    WriteFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "error[io.artifact_commit_failed] artifact set commit failed during {phase} for {}: \
         {source}{}",
        path.display(),
        format_rollback_failures(rollback_failed)
    )]
    ArtifactCommitFailed {
        phase: &'static str,
        path: PathBuf,
        rollback_failed: Vec<PathBuf>,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "error[io.remove_failed] could not remove {}: {source}; every .adoc target was \
         written{}; committed sources remain recoverable from git",
        path.display(),
        format_removed_sources(removed)
    )]
    RemoveFailed {
        path: PathBuf,
        /// Sources already removed before the failure — with the written
        /// targets, the full on-disk state of the aborted run.
        removed: Vec<PathBuf>,
        #[source]
        source: std::io::Error,
    },

    #[error("error[io.path_outside_project] path {} is outside project root {}", path.display(), project_root.display())]
    PathOutsideProject {
        path: PathBuf,
        project_root: PathBuf,
    },

    #[error("build did not produce artifacts")]
    BuildMissingArtifacts,

    #[error("error[review.failed] review failed: {source}")]
    Review {
        #[source]
        source: adoc_core::ReviewError,
    },
}

fn format_removed_sources(removed: &[PathBuf]) -> String {
    if removed.is_empty() {
        return String::new();
    }
    let list = removed
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("; sources already removed: {list}")
}

fn format_rollback_failures(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let list = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("; rollback also failed for: {list}")
}

fn format_config_path(config_path: &Option<PathBuf>) -> String {
    config_path
        .as_ref()
        .map(|path| format!(" in {}", path.display()))
        .unwrap_or_default()
}

impl LocalError {
    pub fn exit_code(&self) -> i32 {
        1
    }
}

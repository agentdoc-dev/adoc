use std::path::PathBuf;

use crate::domain::source::SourceFile;

/// Adapter trait for the input side of the compiler.
///
/// `compile_workspace` defers all filesystem walking and reading to a
/// [`SourceProvider`]. The default adapter is `FsSourceProvider`; tests can
/// substitute `InMemorySourceProvider` to exercise the orchestration logic
/// without touching disk.
pub(crate) trait SourceProvider {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>>;

    /// Wrap this provider so every yielded `SourceFile` has its `path` and
    /// `identity_path` rebased onto `prefix`. Default impl wraps the
    /// provider in [`IdentityRebaseDecorator`]; adapters may override for
    /// zero-allocation behavior.
    ///
    /// The V3 review pipeline calls this to make `content_hash` values
    /// stable across two snapshots of the same source tree at different
    /// filesystem roots (the temporary git worktree vs the project
    /// workdir). Without the rebase, every unchanged Knowledge Object
    /// would appear in the diff's `changed[]` array because the embedded
    /// `SourceSpan.file` would differ between sides.
    fn with_identity_prefix(self, prefix: PathBuf) -> IdentityRebaseDecorator<Self>
    where
        Self: Sized,
    {
        IdentityRebaseDecorator {
            inner: self,
            prefix,
        }
    }
}

/// Generic decorator that rebases every yielded `SourceFile`'s `path` and
/// `identity_path` onto a fixed prefix. Returned from
/// [`SourceProvider::with_identity_prefix`]. Held as `pub(crate)` because
/// only the V3 review pipeline (composed inside `adoc-core`) wires it.
pub(crate) struct IdentityRebaseDecorator<P> {
    inner: P,
    prefix: PathBuf,
}

impl<P: SourceProvider> SourceProvider for IdentityRebaseDecorator<P> {
    fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
        self.inner
            .load_sources()
            .into_iter()
            .map(|result| result.map(|file| file.rebased_to_prefix(&self.prefix)))
            .collect()
    }
}

/// Reported by a [`SourceProvider`] when a single source cannot be loaded.
///
/// `compile_workspace` translates each error into an I/O diagnostic; ordinary
/// read failures remain `io.unreadable_file`, directory traversal failures map
/// to `io.unreadable_directory`, and provider-classified source contract
/// failures can map to a narrower diagnostic code.
#[derive(Debug, Clone)]
pub(crate) struct SourceLoadError {
    pub path: PathBuf,
    pub message: String,
    pub kind: SourceLoadErrorKind,
}

impl SourceLoadError {
    pub(crate) fn unreadable(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
            kind: SourceLoadErrorKind::Unreadable,
        }
    }

    pub(crate) fn unreadable_directory(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
            kind: SourceLoadErrorKind::UnreadableDirectory,
        }
    }

    pub(crate) fn unsupported_source_extension(path: PathBuf) -> Self {
        Self {
            path,
            message: "unsupported source extension; expected a .adoc or .md file".to_string(),
            kind: SourceLoadErrorKind::UnsupportedSourceExtension,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceLoadErrorKind {
    Unreadable,
    UnreadableDirectory,
    UnsupportedSourceExtension,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::source::SourceFile;

    /// Single-shot in-memory provider that yields one `SourceFile`. Lets
    /// the rebase-decorator tests exercise the trait without dragging in
    /// the in-memory test double from `infrastructure/source/in_memory.rs`.
    struct OneShotProvider(RefCell<Option<SourceFile>>);

    impl OneShotProvider {
        fn new(file: SourceFile) -> Self {
            Self(RefCell::new(Some(file)))
        }
    }

    impl SourceProvider for OneShotProvider {
        fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
            self.0
                .borrow_mut()
                .take()
                .map(|file| vec![Ok(file)])
                .unwrap_or_default()
        }
    }

    fn make_file(identity: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from(format!("/tmp/wt-1234/{identity}")),
            "# heading\n".to_string(),
            PathBuf::from(identity),
        )
    }

    #[test]
    fn with_identity_prefix_rebases_path_and_identity() {
        let provider = OneShotProvider::new(make_file("billing.adoc"));
        let decorated = provider.with_identity_prefix(PathBuf::from("<review>"));

        let sources = decorated.load_sources();
        assert_eq!(sources.len(), 1);
        let rebased = sources.into_iter().next().unwrap().expect("ok");

        assert_eq!(rebased.path, PathBuf::from("<review>/billing.adoc"));
        assert_eq!(
            rebased.identity_path,
            PathBuf::from("<review>/billing.adoc")
        );
        // Text preserved byte-identical.
        assert_eq!(rebased.text, "# heading\n");
    }

    #[test]
    fn with_identity_prefix_preserves_load_errors_unmodified() {
        struct ErrorProvider;
        impl SourceProvider for ErrorProvider {
            fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
                vec![Err(SourceLoadError::unreadable(
                    PathBuf::from("blocked.adoc"),
                    "permission denied",
                ))]
            }
        }

        let decorated = ErrorProvider.with_identity_prefix(PathBuf::from("<review>"));
        let sources = decorated.load_sources();
        assert_eq!(sources.len(), 1);
        let error = sources.into_iter().next().unwrap().expect_err("err");
        assert_eq!(error.path, PathBuf::from("blocked.adoc"));
        assert_eq!(error.message, "permission denied");
    }
}

use std::path::{Path, PathBuf};

use crate::PathPolicy;

#[derive(Debug, Clone)]
pub struct LocalContext<P>
where
    P: PathPolicy,
{
    config_start: PathBuf,
    path_policy: P,
    retrieval_policy_override: Option<adoc_core::RetrievalPolicy>,
}

impl<P> LocalContext<P>
where
    P: PathPolicy,
{
    pub fn new(config_start: PathBuf, path_policy: P) -> Self {
        Self {
            config_start,
            path_policy,
            retrieval_policy_override: None,
        }
    }

    /// Bind retrieval to a trusted caller's policy instead of project policy.
    /// Project configuration still supplies other settings and is validated normally.
    pub fn with_retrieval_policy_override(mut self, policy: adoc_core::RetrievalPolicy) -> Self {
        self.retrieval_policy_override = Some(policy);
        self
    }

    pub(crate) fn retrieval_policy_override(&self) -> Option<&adoc_core::RetrievalPolicy> {
        self.retrieval_policy_override.as_ref()
    }

    pub fn config_start(&self) -> &Path {
        &self.config_start
    }

    pub fn path_policy(&self) -> &P {
        &self.path_policy
    }
}

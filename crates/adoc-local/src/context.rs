use std::path::{Path, PathBuf};

use crate::PathPolicy;

#[derive(Debug, Clone)]
pub struct LocalContext<P>
where
    P: PathPolicy,
{
    config_start: PathBuf,
    path_policy: P,
}

impl<P> LocalContext<P>
where
    P: PathPolicy,
{
    pub fn new(config_start: PathBuf, path_policy: P) -> Self {
        Self {
            config_start,
            path_policy,
        }
    }

    pub fn config_start(&self) -> &Path {
        &self.config_start
    }

    pub fn path_policy(&self) -> &P {
        &self.path_policy
    }
}

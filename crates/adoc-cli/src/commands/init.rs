use adoc_local::{InitInput, InitUseCase, LocalContext, UnrestrictedPathPolicy};

use super::{current_dir, report};

const INIT_CONFIG_PATH: &str = "agentdoc.config.yaml";
const INIT_INDEX_PATH: &str = "docs/index.adoc";

pub(crate) fn init() -> i32 {
    let project_root = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };

    let context = LocalContext::new(project_root, UnrestrictedPathPolicy);
    match InitUseCase::new(context).run(InitInput) {
        Ok(_) => {
            println!("Created {INIT_CONFIG_PATH} and {INIT_INDEX_PATH}");
            println!("Next: adoc check");
            0
        }
        Err(error) => report(error.into()),
    }
}

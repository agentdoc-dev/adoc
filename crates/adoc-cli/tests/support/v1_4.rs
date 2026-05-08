use std::path::PathBuf;
use std::process::Command;

use super::TestWorkspace;

#[allow(dead_code)]
pub struct V1_4Pilot {
    pub _workspace: TestWorkspace,
    pub agent_path: PathBuf,
    pub search_path: PathBuf,
}

#[allow(dead_code)]
pub fn build_v1_4_pilot() -> V1_4Pilot {
    let workspace = TestWorkspace::new("v1-4-pilot");

    workspace.write(
        "billing.adoc",
        "# Billing @doc(billing.page)\n\
         \n\
         ::claim billing.credits.ledger-source\n\
         status: verified\n\
         owner: team-billing\n\
         source: ledger.md\n\
         test: cargo test billing\n\
         reviewed_by: qa\n\
         verified_at: 2026-05-05\n\
         --\n\
         Credits flow into the user ledger after a successful payment is confirmed.\n\
         ::\n\
         \n\
         ::claim billing.refunds.audit-required\n\
         status: verified\n\
         owner: team-billing\n\
         source: audit.md\n\
         test: cargo test refunds\n\
         reviewed_by: qa\n\
         verified_at: 2026-05-05\n\
         --\n\
         Returning charges must be reviewed by the audit team before they are issued.\n\
         ::\n\
         \n\
         ::claim ops.dlq.retry-policy\n\
         status: verified\n\
         owner: team-ops\n\
         source: ops.md\n\
         test: cargo test dlq\n\
         reviewed_by: qa\n\
         verified_at: 2026-05-05\n\
         --\n\
         Messages on the dead letter queue are retried with exponential backoff.\n\
         ::\n",
    );

    let out = workspace.root.join("dist");
    let status = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "build",
            workspace.root.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .status()
        .expect("adoc build runs");
    assert!(
        status.success(),
        "adoc build must succeed for the v1.4 pilot fixture"
    );

    V1_4Pilot {
        agent_path: out.join("docs.agent.json"),
        search_path: out.join("docs.search.json"),
        _workspace: workspace,
    }
}

use adoc_mcp::AgentDocMcpServer;
use rmcp::ServiceExt;
use tracing_subscriber::{EnvFilter, prelude::*};

#[derive(Debug, thiserror::Error)]
enum GatewayConfigError {
    #[error(
        "error[mcp.invalid_arguments] Expected optional --config PATH and --audit-config PATH."
    )]
    Arguments,
    #[error(
        "error[retrieval.policy_invalid] Could not read gateway configuration; check the --config file's accessibility."
    )]
    Read,
    #[error(
        "error[retrieval.policy_invalid] Could not parse gateway configuration; check its YAML and required fields."
    )]
    Parse,
    #[error(
        "error[config.invalid] Invalid gateway configuration settings; check version, mode, paths and provider."
    )]
    Settings,
    #[error(
        "error[retrieval.policy_invalid] Invalid retrieval policy; check supported fields, visibility classes and excluded Object IDs."
    )]
    Policy,
    #[error(
        "error[retrieval.audience_unresolved] Configure an explicit public, internal or restricted retrieval audience."
    )]
    Audience,
}

fn gateway_policy(
    path: Option<&std::path::Path>,
) -> Result<Option<adoc_core::RetrievalPolicy>, GatewayConfigError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path).map_err(|_| GatewayConfigError::Read)?;
    let config = adoc_core::parse_project_config(&text).map_err(|error| match error {
        adoc_core::ProjectConfigDocumentError::RetrievalPolicy(diagnostic) => {
            if diagnostic.code == adoc_core::DiagnosticCode::RetrievalAudienceUnresolved {
                GatewayConfigError::Audience
            } else {
                GatewayConfigError::Policy
            }
        }
        adoc_core::ProjectConfigDocumentError::Parse(_) => GatewayConfigError::Parse,
        adoc_core::ProjectConfigDocumentError::Invalid(_)
        | adoc_core::ProjectConfigDocumentError::InvalidAssessmentPath { .. } => {
            GatewayConfigError::Settings
        }
    })?;
    config
        .retrieval_policy
        .map(Some)
        .ok_or(GatewayConfigError::Audience)
}

/// Logs go to stderr only — stdout is the MCP stdio transport. Filtered by
/// `ADOC_LOG` (falling back to `RUST_LOG`); silent when neither is set.
fn init_tracing() {
    let filter = EnvFilter::try_from_env("ADOC_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("off"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_writer(std::io::stderr)
        .finish()
        .with(tracing_subscriber::filter::filter_fn(|metadata| {
            !metadata.target().starts_with("ureq")
        }))
        .try_init();
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = None;
    let mut audit_config = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(flag) = args.next() {
        let path = args.next().ok_or(GatewayConfigError::Arguments)?;
        let target = if flag == "--config" {
            &mut config
        } else if flag == "--audit-config" {
            &mut audit_config
        } else {
            return Err(GatewayConfigError::Arguments.into());
        };
        if target.replace(std::path::PathBuf::from(path)).is_some() {
            return Err(GatewayConfigError::Arguments.into());
        }
    }
    let retrieval_policy = gateway_policy(config.as_deref())?;
    init_tracing();
    let project_root = std::env::current_dir()?;
    tracing::info!(target: "adoc_mcp", root = %project_root.display(), "MCP Agent Gateway starting on stdio");
    let mut server = AgentDocMcpServer::new(project_root);
    if let Some(policy) = retrieval_policy {
        server = server.with_retrieval_policy(policy);
    }
    if let Some(path) = audit_config {
        server = server.with_audit_config(&path)?;
    }
    server
        .serve(rmcp::transport::io::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}

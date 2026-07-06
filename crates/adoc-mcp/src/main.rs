use adoc_mcp::AgentDocMcpServer;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

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
        .try_init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let project_root = std::env::current_dir()?;
    tracing::info!(target: "adoc_mcp", root = %project_root.display(), "MCP Agent Gateway starting on stdio");
    AgentDocMcpServer::new(project_root)
        .serve(rmcp::transport::io::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}

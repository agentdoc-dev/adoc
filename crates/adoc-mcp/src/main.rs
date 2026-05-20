use adoc_mcp::AgentDocMcpServer;
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project_root = std::env::current_dir()?;
    AgentDocMcpServer::new(project_root)
        .serve(rmcp::transport::io::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}

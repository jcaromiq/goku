mod mcp_goku;

use crate::mcp_goku::GokuMcpServer;
use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};

#[tokio::main]
async fn main() -> Result<()> {
    let service = GokuMcpServer::new()
        .serve(stdio())
        .await
        .inspect_err(|_| {})?;

    service.waiting().await?;
    Ok(())
}

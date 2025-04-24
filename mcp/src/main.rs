mod mcp_goku;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use crate::mcp_goku::GokuMcpServer;

#[tokio::main]
async fn main() -> Result<()> {

    let service = GokuMcpServer::new().serve(stdio()).await.inspect_err(|_| {

    })?;

    service.waiting().await?;
    Ok(())
}
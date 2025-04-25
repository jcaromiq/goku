use goku_core::benchmark::{Metrics, Report};
use goku_core::execution::run;
use goku_core::settings::Settings;
use rmcp::{const_string, model::*, schemars, tool, Error as McpError, Error, ServerHandler};
use std::time::Duration;
use tokio::sync::{mpsc};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct StructRequest {
    pub target: String,
    pub clients: usize,
    pub requests: usize,
}

#[derive(Clone)]
pub struct GokuMcpServer {}
#[tool(tool_box)]
impl GokuMcpServer {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {}
    }

    fn _create_resource_text(&self, uri: &str, name: &str) -> Resource {
        RawResource::new(uri, name.to_string()).no_annotation()
    }

    #[tool(
        description = "show the percentile 95 of the latency for given target, clients and requests"
    )]
    async fn percentile_95(
        &self,
        #[tool(aggr)] StructRequest {
            target,
            clients,
            requests,
        }: StructRequest,
    ) -> Result<CallToolResult, McpError> {
        let report = Self::benchmark(target, clients, requests).await?;
        let p95 = report.hist.value_at_quantile(0.95).to_string();
        Ok(CallToolResult::success(vec![Content::text(p95)]))
    }

    #[tool(
        description = "show the percentile 99 of the latency for given target, clients and requests"
    )]
    async fn percentile_99(
        &self,
        #[tool(aggr)] StructRequest {
            target,
            clients,
            requests,
        }: StructRequest,
    ) -> Result<CallToolResult, McpError> {
        let report = Self::benchmark(target, clients, requests).await?;
        Ok(CallToolResult::success(vec![Content::text(
            report.hist.value_at_quantile(0.99).to_string(),
        )]))
    }

    #[tool(
        description = "show the min request time of the latency for given target, clients and requests"
    )]
    async fn min(
        &self,
        #[tool(aggr)] StructRequest {
            target,
            clients,
            requests,
        }: StructRequest,
    ) -> Result<CallToolResult, McpError> {
        let report = Self::benchmark(target, clients, requests).await?;
        Ok(CallToolResult::success(vec![Content::text(
            report.results.min().to_string(),
        )]))
    }

    #[tool(
        description = "show the max request time of the latency for given target, clients and requests"
    )]
    async fn max(
        &self,
        #[tool(aggr)] StructRequest {
            target,
            clients,
            requests,
        }: StructRequest,
    ) -> Result<CallToolResult, McpError> {
        let report = Self::benchmark(target, clients, requests).await?;
        Ok(CallToolResult::success(vec![Content::text(
            report.results.max().to_string(),
        )]))
    }

    async fn benchmark(target: String, clients: usize, requests: usize) -> Result<Report, Error> {
        let settings: Settings = Settings {
            clients,
            requests,
            target,
            keep_alive: None,
            body: None,
            headers: None,
            duration: None,
            verbose: false,
            timeout: Duration::from_secs(30000),
        };

        let mut report: Report = Report::new(settings.clients);

        let (benchmark_tx, mut benchmark_rx) = mpsc::channel(settings.requests);

        run(settings.clone(), benchmark_tx, None)
            .await
            .map_err(|e| McpError::new(ErrorCode(0), e.to_string(), None))?;
        while let Some(value) = benchmark_rx.recv().await {
            report.add_result(value);
        }
        Ok(report)
    }
}
const_string!(Echo = "echo");
#[tool(tool_box)]
impl ServerHandler for GokuMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("This server provides a benchmark tool that given a target, concurrent clients and number fo totals requests".to_string()),
        }
    }
}

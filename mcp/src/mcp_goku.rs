use goku_core::benchmark::{Metrics, Report};
use goku_core::execution::run;
use goku_core::settings::Settings;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{
    handler::server::tool::ToolRouter, model::*, prompt_router, tool, tool_handler,
    tool_router, ErrorData as McpError, ServerHandler,
};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct StructRequest {
    #[schemars(
        description = "the target url to perform the benchmarking, Ex. http://localhost:3000/"
    )]
    pub target: String,
    #[schemars(description = "the number of concurrent clients to use, Ex. 100")]
    pub clients: i32,
    #[schemars(description = "the number of total requests to perform, Ex. 10000")]
    pub requests: i32,
}

#[derive(Clone)]
pub struct GokuMcpServer {
    tool_router: ToolRouter<Self>,
}
#[tool_router]
#[prompt_router]
impl GokuMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    fn _create_resource_text(&self, uri: &str, name: &str) -> Resource {
        RawResource::new(uri, name.to_string()).no_annotation()
    }

    #[tool(
        description = "show the percentile 95 of the latency for given target, clients and requests"
    )]
    async fn percentile_95(
        &self,
        Parameters(StructRequest {
            target,
            clients,
            requests,
        }): Parameters<StructRequest>,
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
        Parameters(StructRequest {
            target,
            clients,
            requests,
        }): Parameters<StructRequest>,
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
        Parameters(StructRequest {
            target,
            clients,
            requests,
        }): Parameters<StructRequest>,
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
        Parameters(StructRequest {
            target,
            clients,
            requests,
        }): Parameters<StructRequest>,
    ) -> Result<CallToolResult, McpError> {
        let report = Self::benchmark(target, clients, requests).await?;
        Ok(CallToolResult::success(vec![Content::text(
            report.results.max().to_string(),
        )]))
    }

    async fn benchmark(target: String, clients: i32, requests: i32) -> Result<Report, McpError> {
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

        let (benchmark_tx, mut benchmark_rx) = mpsc::channel(settings.requests as usize);

        run(settings.clone(), benchmark_tx, None)
            .await
            .map_err(|e| McpError::new(ErrorCode(0), e.to_string(), None))?;
        while let Some(value) = benchmark_rx.recv().await {
            report.add_result(value);
        }
        Ok(report)
    }
}

#[tool_handler]
impl ServerHandler for GokuMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
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

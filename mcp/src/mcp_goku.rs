use std::time::Duration;

use goku_core::benchmark::{Metrics, Report};
use goku_core::execution::run;
use goku_core::settings::{Header, Settings};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{
    handler::server::tool::ToolRouter, model::*, prompt_router, tool, tool_handler, tool_router,
    ErrorData as McpError, ServerHandler,
};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Request schema
// ---------------------------------------------------------------------------

/// Input parameters for the unified benchmark tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BenchmarkRequest {
    /// Target URL including optional HTTP method prefix.
    /// Examples: "http://localhost:3000/" or "POST http://api.example.com/users"
    #[schemars(
        description = "Target URL to benchmark. Format: '[METHOD] <url>'. Defaults to GET if no method is specified."
    )]
    pub target: String,

    /// Number of concurrent workers sending requests in parallel.
    #[schemars(description = "Number of concurrent clients/workers (e.g. 10, 50, 100).")]
    pub clients: u32,

    /// Total number of requests to perform (ignored when `duration_secs` is set).
    #[schemars(
        description = "Total number of requests to execute across all clients. Ignored when duration_secs is set."
    )]
    pub requests: Option<u32>,

    /// Duration of the test in seconds (alternative to `requests`).
    #[schemars(
        description = "Duration of the test in seconds. If set, requests is ignored and the test runs for this many seconds."
    )]
    pub duration_secs: Option<u64>,

    /// Request body (e.g. JSON payload for POST/PUT requests).
    #[schemars(
        description = "Optional request body string. Use for POST, PUT, PATCH requests. E.g. '{\"key\": \"value\"}'"
    )]
    pub body: Option<String>,

    /// HTTP headers in 'Name:Value' format.
    #[schemars(
        description = "Optional list of HTTP headers in 'Name:Value' format. E.g. ['Content-Type:application/json', 'X-Api-Key:secret']"
    )]
    pub headers: Option<Vec<String>>,

    /// Use HTTP/2 prior knowledge (bypass HTTP/1.1 upgrade).
    #[schemars(
        description = "Set to true to force HTTP/2 prior knowledge (skips HTTP/1.1 negotiation)."
    )]
    pub http2: Option<bool>,

    /// Ramp-up duration in seconds (spread worker start over this period).
    #[schemars(
        description = "Seconds over which to spread the start of workers. Useful for simulating gradual traffic ramp-up."
    )]
    pub ramp_up: Option<u64>,

    /// Request timeout in milliseconds (default: 30000).
    #[schemars(
        description = "Request timeout in milliseconds. Defaults to 30000 (30 seconds) if not specified."
    )]
    pub timeout_ms: Option<u64>,

    /// Accept invalid/self-signed TLS certificates.
    #[schemars(
        description = "Set to true to accept invalid or self-signed TLS certificates (insecure mode)."
    )]
    pub insecure: Option<bool>,

    /// Maximum requests per second (rate limiting).
    #[schemars(
        description = "Maximum total requests per second across all clients. Omit or set to 0 for unlimited."
    )]
    pub rps: Option<u32>,
}

// ---------------------------------------------------------------------------
// MCP Server
// ---------------------------------------------------------------------------

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

    /// Runs a full HTTP benchmark and returns a JSON report with all metrics.
    ///
    /// Returns a JSON object containing: concurrency, duration_secs, total_requests,
    /// requests_per_sec, mean_ms, min_ms, max_ms, p50_ms, p95_ms, p99_ms, p999_ms,
    /// status_2xx, status_4xx, status_5xx, status_other, network_errors.
    #[tool(
        description = "Run an HTTP load test and return a full benchmark report as JSON. \
        Supports configuring concurrency, total requests or duration, request body, headers, \
        HTTP/2, ramp-up, timeout, rate limiting, and insecure TLS. \
        Returns all latency percentiles (p50, p95, p99, p99.9), throughput, and status code breakdown."
    )]
    async fn run_benchmark(
        &self,
        Parameters(req): Parameters<BenchmarkRequest>,
    ) -> Result<CallToolResult, McpError> {
        let report = Self::execute_benchmark(req)
            .await
            .map_err(|e| McpError::new(ErrorCode(0), e.to_string(), None))?;

        let json = build_json_report(&report);
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

// ---------------------------------------------------------------------------
// Benchmark execution
// ---------------------------------------------------------------------------

impl GokuMcpServer {
    async fn execute_benchmark(req: BenchmarkRequest) -> anyhow::Result<Report> {
        // Parse headers from "Name:Value" strings
        let headers = req
            .headers
            .as_deref()
            .map(|hs| {
                hs.iter()
                    .map(|h| {
                        let mut parts = h.splitn(2, ':');
                        let key = parts.next().unwrap_or("").trim().to_string();
                        let value = parts.next().unwrap_or("").trim().to_string();
                        Header { key, value }
                    })
                    .collect::<Vec<_>>()
            });

        let requests = req.requests.unwrap_or(1);
        let timeout = Duration::from_millis(req.timeout_ms.unwrap_or(30_000));

        let settings = Settings {
            clients: req.clients,
            requests,
            target: req.target,
            keep_alive: None,
            body: req.body,
            headers,
            duration: req.duration_secs,
            verbose: false,
            timeout,
            http2: req.http2.unwrap_or(false),
            ramp_up: req.ramp_up,
            output: Default::default(),
            insecure: req.insecure.unwrap_or(false),
            rps: req.rps,
            auth: None,
            output_file: None,
            results_log: None,
            steps: vec![],
            live_stats: None,
            pool_idle_timeout: None,
            disable_keepalive: false,
        };

        settings
            .validate()
            .map_err(|e| anyhow::anyhow!("Invalid benchmark settings: {e}"))?;

        let channel_capacity = (settings.clients as usize * 2).min(4096);
        let (benchmark_tx, mut benchmark_rx) = mpsc::channel(channel_capacity);
        let mut report = Report::new(settings.clients);

        run(settings, benchmark_tx, None)
            .await
            .map_err(|e| anyhow::anyhow!("Benchmark failed: {e}"))?;

        while let Some(value) = benchmark_rx.recv().await {
            report.add_result(value);
        }

        Ok(report)
    }
}

// ---------------------------------------------------------------------------
// JSON report builder
// ---------------------------------------------------------------------------

fn build_json_report(r: &Report) -> String {
    let elapsed = r.start.elapsed().as_secs_f64();
    let bd = r.status_breakdown();
    let min = r.results.min();
    let max = r.results.max();

    let data = serde_json::json!({
        "concurrency": r.clients,
        "duration_secs": format!("{:.3}", elapsed).parse::<f64>().unwrap_or(0.0),
        "total_requests": r.hist.len(),
        "requests_per_sec": format!("{:.2}", r.requests_per_second()).parse::<f64>().unwrap_or(0.0),
        "mean_ms": format!("{:.2}", r.hist.mean()).parse::<f64>().unwrap_or(0.0),
        "min_ms": min,
        "max_ms": max,
        "p50_ms": r.hist.value_at_quantile(0.50),
        "p95_ms": r.hist.value_at_quantile(0.95),
        "p99_ms": r.hist.value_at_quantile(0.99),
        "p999_ms": r.hist.value_at_quantile(0.999),
        "status_2xx": bd.success,
        "status_4xx": bd.client_error,
        "status_5xx": bd.server_error,
        "status_other": bd.other,
        "network_errors": bd.network_error,
    });

    serde_json::to_string_pretty(&data).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// ServerHandler
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for GokuMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_instructions(
            "Goku MCP server — exposes a single `run_benchmark` tool that runs an HTTP \
            load test and returns a full JSON report with latency percentiles, throughput, \
            and status code breakdown. Supports concurrency, duration or request count, \
            custom headers, request body, HTTP/2, ramp-up, timeout, and rate limiting."
                .to_string(),
        )
    }
}

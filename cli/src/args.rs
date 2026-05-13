use std::fs;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use goku_core::settings::{Auth, Header, Settings};

// ---------------------------------------------------------------------------
// Top-level CLI structure (supports subcommands)
// ---------------------------------------------------------------------------

/// Goku — high-performance HTTP load testing tool
#[derive(Parser, Debug)]
#[command(name = "goku", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    // ── inline benchmark flags (no subcommand = run benchmark directly) ──

    /// Runs in verbose mode
    #[arg(short, long, global = false)]
    pub verbose: bool,

    /// URL to request. Format: [METHOD] <url>  (default method: GET)
    /// Example: "POST http://localhost:3000/api"
    #[arg(
        short,
        long,
        conflicts_with = "scenario",
        required_unless_present_any = ["scenario", "command"]
    )]
    pub target: Option<String>,

    /// Path to a file whose contents will be used as the request body
    #[arg(short, long, conflicts_with = "scenario")]
    pub request_body: Option<String>,

    /// Number of concurrent workers
    #[arg(short, long, default_value_t = 1, conflicts_with = "scenario")]
    pub clients: u32,

    /// Total number of requests (ignored when --duration is set)
    #[arg(short, long, default_value_t = 1, conflicts_with_all = ["duration", "scenario"])]
    pub iterations: u32,

    /// Duration of the test in seconds (alternative to --iterations)
    #[arg(short, long, conflicts_with_all = ["iterations", "scenario"])]
    pub duration: Option<u64>,

    /// Headers in "Name:Value" format (repeatable)
    #[arg(long, conflicts_with = "scenario")]
    pub headers: Option<Vec<String>>,

    /// Path to a YAML scenario file
    #[arg(long, conflicts_with = "target")]
    pub scenario: Option<String>,

    /// Request timeout in milliseconds
    #[arg(long, default_value_t = 30_000, conflicts_with = "scenario")]
    pub timeout: u64,

    /// Use HTTP/2 prior knowledge
    #[arg(long, default_value_t = false, conflicts_with = "scenario")]
    pub http2: bool,

    /// Seconds over which to spread the start of workers (ramp-up)
    #[arg(long, conflicts_with = "scenario")]
    pub ramp_up: Option<u64>,

    /// Output format: text (default) | json | csv
    #[arg(long, default_value = "text", conflicts_with = "scenario")]
    pub output: String,

    /// Accept invalid/self-signed TLS certificates (insecure mode)
    #[arg(long, default_value_t = false)]
    pub insecure: bool,

    /// Maximum requests per second across all clients (0 = unlimited)
    #[arg(long)]
    pub rps: Option<u32>,

    /// Bearer token for Authorization header (e.g. --auth-bearer mytoken)
    #[arg(long, conflicts_with_all = ["auth_basic", "scenario"])]
    pub auth_bearer: Option<String>,

    /// Basic auth credentials in user:password format
    #[arg(long, conflicts_with_all = ["auth_bearer", "scenario"])]
    pub auth_basic: Option<String>,

    /// Write results output to this file instead of stdout
    #[arg(long)]
    pub output_file: Option<String>,

    /// Write per-request log (timestamp_ms,status,latency_ms) to this CSV file
    #[arg(long)]
    pub results_log: Option<String>,

    /// Print live stats every N seconds during the test
    #[arg(long)]
    pub live_stats: Option<u64>,

    /// Idle timeout for pooled connections in seconds (default: 90)
    #[arg(long)]
    pub pool_idle_timeout: Option<u64>,

    /// Disable HTTP keep-alive / connection reuse
    #[arg(long, default_value_t = false)]
    pub disable_keepalive: bool,
}

// ---------------------------------------------------------------------------
// Subcommands
// ---------------------------------------------------------------------------

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Compare two benchmark result JSON files and show a diff table
    Compare {
        /// First result file (baseline)
        baseline: String,
        /// Second result file (candidate)
        candidate: String,
    },
}

// ---------------------------------------------------------------------------
// Conversion to Settings
// ---------------------------------------------------------------------------

impl Cli {
    pub fn to_settings(self) -> anyhow::Result<Settings> {
        let output_format = self.output.parse().unwrap_or_default();
        let auth = parse_auth(self.auth_bearer.as_deref(), self.auth_basic.as_deref())?;

        let mut settings = match self.scenario {
            None => Self::from_args(&self, auth)?,
            Some(file) => Settings::from_file(file)?,
        };

        settings.output = output_format;
        settings.output_file = self.output_file;
        settings.results_log = self.results_log;
        settings.live_stats = self.live_stats;
        Ok(settings)
    }

    fn from_args(args: &Cli, auth: Option<Auth>) -> anyhow::Result<Settings> {
        let headers = parse_headers(args.headers.as_deref())?;

        let body = match &args.request_body {
            None => None,
            Some(file) => {
                let content = fs::read_to_string(file)
                    .with_context(|| format!("Failed to read body file '{}'", file))?;
                Some(content)
            }
        };

        Ok(Settings {
            clients: args.clients,
            requests: args.iterations,
            target: args
                .target
                .clone()
                .expect("target is required (enforced by clap)"),
            keep_alive: None,
            body,
            headers,
            duration: args.duration,
            verbose: args.verbose,
            timeout: Duration::from_millis(args.timeout),
            http2: args.http2,
            ramp_up: args.ramp_up,
            output: Default::default(),
            insecure: args.insecure,
            rps: args.rps,
            auth,
            output_file: None,
            results_log: None,
            steps: vec![],
            live_stats: None,
            pool_idle_timeout: args.pool_idle_timeout,
            disable_keepalive: args.disable_keepalive,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_headers(raw: Option<&[String]>) -> anyhow::Result<Option<Vec<Header>>> {
    let Some(headers_raw) = raw else {
        return Ok(None);
    };
    let mut headers = Vec::with_capacity(headers_raw.len());
    for h in headers_raw {
        let mut split = h.splitn(2, ':');
        let key = split
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let value = split
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if key.is_empty() {
            anyhow::bail!("Invalid header '{}': header name cannot be empty", h);
        }
        headers.push(Header { key, value });
    }
    Ok(Some(headers))
}

fn parse_auth(bearer: Option<&str>, basic: Option<&str>) -> anyhow::Result<Option<Auth>> {
    match (bearer, basic) {
        (Some(token), None) => Ok(Some(Auth::Bearer {
            token: token.to_string(),
        })),
        (None, Some(credentials)) => {
            let mut parts = credentials.splitn(2, ':');
            let user = parts.next().unwrap_or("").to_string();
            let password = parts.next().unwrap_or("").to_string();
            if user.is_empty() {
                anyhow::bail!("--auth-basic requires format 'user:password'");
            }
            Ok(Some(Auth::Basic { user, password }))
        }
        (None, None) => Ok(None),
        (Some(_), Some(_)) => anyhow::bail!("Cannot use --auth-bearer and --auth-basic together"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_headers_splits_on_first_colon() {
        let raw = vec!["Content-Type:application/json".to_string()];
        let headers = parse_headers(Some(&raw)).unwrap().unwrap();
        assert_eq!(headers[0].key, "Content-Type");
        assert_eq!(headers[0].value, "application/json");
    }

    #[test]
    fn parse_headers_value_with_colon() {
        let raw = vec!["Authorization:Bearer some:token".to_string()];
        let headers = parse_headers(Some(&raw)).unwrap().unwrap();
        assert_eq!(headers[0].key, "Authorization");
        assert_eq!(headers[0].value, "Bearer some:token");
    }

    #[test]
    fn parse_headers_empty_key_is_error() {
        let raw = vec![":value".to_string()];
        assert!(parse_headers(Some(&raw)).is_err());
    }

    #[test]
    fn parse_headers_none_returns_none() {
        assert!(parse_headers(None).unwrap().is_none());
    }

    #[test]
    fn parse_auth_bearer() {
        let auth = parse_auth(Some("mytoken"), None).unwrap().unwrap();
        matches!(auth, Auth::Bearer { token } if token == "mytoken");
    }

    #[test]
    fn parse_auth_basic() {
        let auth = parse_auth(None, Some("user:pass")).unwrap().unwrap();
        matches!(auth, Auth::Basic { user, password } if user == "user" && password == "pass");
    }

    #[test]
    fn parse_auth_none() {
        assert!(parse_auth(None, None).unwrap().is_none());
    }

    #[test]
    fn parse_auth_both_errors() {
        assert!(parse_auth(Some("tok"), Some("u:p")).is_err());
    }
}
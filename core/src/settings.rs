use std::fs;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use strum::EnumString;

use crate::settings::Operation::Get;

// ---------------------------------------------------------------------------
// OutputFormat
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Serialize, Deserialize, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Csv,
}

impl std::str::FromStr for OutputFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            "csv" => Ok(OutputFormat::Csv),
            "text" | "plain" => Ok(OutputFormat::Text),
            other => anyhow::bail!("Unknown format '{}'. Valid options: text, json, csv", other),
        }
    }
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Auth {
    Bearer { token: String },
    Basic { user: String, password: String },
}

impl Auth {
    /// Returns the value of the `Authorization` header for this auth method.
    pub fn header_value(&self) -> String {
        match self {
            Auth::Bearer { token } => format!("Bearer {}", token),
            Auth::Basic { user, password } => {
                let encoded =
                    base64_encode(format!("{}:{}", user, password).as_bytes());
                format!("Basic {}", encoded)
            }
        }
    }
}

/// Minimal base64 encoder (avoids pulling a full base64 crate for a single use-site).
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) & 0x3f] as char);
        out.push(TABLE[(n >> 12) & 0x3f] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(n >> 6) & 0x3f] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[n & 0x3f] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// Operation
// ---------------------------------------------------------------------------

#[derive(Eq, PartialEq, Debug, EnumString, Clone)]
pub enum Operation {
    #[strum(serialize = "GET")]
    Get,
    #[strum(serialize = "POST")]
    Post,
    #[strum(serialize = "HEAD")]
    Head,
    #[strum(serialize = "PATCH")]
    Patch,
    #[strum(serialize = "PUT")]
    Put,
    #[strum(serialize = "DELETE")]
    Delete,
}

// ---------------------------------------------------------------------------
// Step — a single request within a multi-step scenario
// ---------------------------------------------------------------------------

/// One step inside a multi-step sequential scenario.
/// Each worker will execute all steps in order, repeating the sequence.
#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Step {
    /// Same format as `Settings::target`: `[METHOD] <url>`
    pub target: String,
    pub body: Option<String>,
    pub headers: Option<Vec<Header>>,
}

impl Step {
    pub fn operation(&self) -> Operation {
        parse_operation(&self.target)
    }
    pub fn url(&self) -> String {
        parse_url(&self.target)
    }
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Settings {
    /// Number of concurrent workers.
    pub clients: u32,
    /// Total number of requests (ignored when `duration` is set).
    #[serde(default)]
    pub requests: u32,
    /// Primary target in `[METHOD] <url>` format. Required when `steps` is empty.
    #[serde(default)]
    pub target: String,
    pub keep_alive: Option<Duration>,
    pub body: Option<String>,
    pub headers: Option<Vec<Header>>,
    /// Test duration in seconds (alternative to `requests`).
    pub duration: Option<u64>,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default = "default_timeout")]
    pub timeout: Duration,
    #[serde(default)]
    pub http2: bool,
    #[serde(default)]
    pub ramp_up: Option<u64>,
    #[serde(default)]
    pub output: OutputFormat,
    /// Accept invalid/self-signed TLS certificates. Opt-in via `--insecure`.
    #[serde(default)]
    pub insecure: bool,
    /// Maximum requests per second across all clients (0 = unlimited).
    #[serde(default)]
    pub rps: Option<u32>,
    /// Built-in authentication.
    pub auth: Option<Auth>,
    /// Write results to this file path instead of stdout.
    pub output_file: Option<String>,
    /// Write per-request log (timestamp_ms, status, latency_ms) to this file.
    pub results_log: Option<String>,
    /// Sequential steps for multi-target scenarios. When non-empty, `target`/`body`/`headers` are ignored.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Print live stats every N seconds during the test (0 = disabled).
    #[serde(default)]
    pub live_stats: Option<u64>,
    /// Idle timeout for pooled connections in seconds.
    pub pool_idle_timeout: Option<u64>,
    /// Disable HTTP keep-alive / connection reuse.
    #[serde(default)]
    pub disable_keepalive: bool,
}

fn default_timeout() -> Duration {
    Duration::from_millis(30_000)
}

impl Settings {
    /// Requests each worker should execute (floored integer division).
    pub fn requests_by_client(&self) -> u32 {
        if self.clients == 0 {
            return 0;
        }
        self.requests / self.clients
    }

    /// Load settings from a YAML scenario file.
    pub fn from_file(file: String) -> anyhow::Result<Self> {
        let content = fs::read_to_string(&file)
            .with_context(|| format!("Failed to read scenario file '{}'", &file))?;
        let settings: Settings = serde_yaml::from_str(&content)
            .with_context(|| format!("Invalid YAML in scenario file '{}'", &file))?;
        Ok(settings)
    }

    /// HTTP operation derived from the primary `target` string.
    pub fn operation(&self) -> Operation {
        parse_operation(&self.target)
    }

    /// URL derived from the primary `target` string.
    pub fn target_url(&self) -> String {
        parse_url(&self.target)
    }

    /// Validate settings before starting the benchmark.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.clients == 0 {
            anyhow::bail!("--clients must be greater than 0");
        }
        if self.duration.is_none() && self.requests == 0 {
            anyhow::bail!("--iterations must be greater than 0");
        }

        // Validate primary target URL (unless using multi-step scenario)
        if self.steps.is_empty() {
            if self.target.trim().is_empty() {
                anyhow::bail!("--target cannot be empty");
            }
            let url_str = self.target_url();
            let parsed = Url::parse(&url_str).with_context(|| {
                format!(
                    "Invalid URL '{}'. Make sure to include the scheme (http:// or https://)",
                    url_str
                )
            })?;
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                anyhow::bail!(
                    "Invalid URL scheme '{}' in '{}'. Only http and https are supported.",
                    scheme, url_str
                );
            }
        } else {
            // Validate each step's URL
            for (i, step) in self.steps.iter().enumerate() {
                let url_str = step.url();
                let parsed = Url::parse(&url_str).with_context(|| {
                    format!(
                        "Invalid URL '{}' in step {} of scenario. Make sure to include the scheme (http:// or https://)",
                        url_str, i + 1
                    )
                })?;
                let scheme = parsed.scheme();
                if scheme != "http" && scheme != "https" {
                    anyhow::bail!(
                        "Invalid URL scheme '{}' in step {}. Only http and https are supported.",
                        scheme, i + 1
                    );
                }
            }
        }

        if let Some(ramp_up) = self.ramp_up {
            if let Some(dur) = self.duration {
                if ramp_up >= dur {
                    anyhow::bail!(
                        "--ramp-up ({ramp_up}s) must be shorter than --duration ({dur}s)"
                    );
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_operation(target: &str) -> Operation {
    let slices: Vec<&str> = target.split_whitespace().collect();
    if slices.len() == 1 {
        return Get;
    }
    match slices.first() {
        None => Get,
        Some(op) => match Operation::from_str(&op.to_uppercase()) {
            Ok(o) => o,
            Err(_) => {
                eprintln!(
                    "Warning: unknown HTTP method '{}', defaulting to GET",
                    op
                );
                Get
            }
        },
    }
}

fn parse_url(target: &str) -> String {
    let slices: Vec<&str> = target.split_whitespace().collect();
    if slices.len() == 1 {
        return slices
            .first()
            .expect("target is not well formatted")
            .to_string();
    }
    slices
        .get(1)
        .expect("target is not well formatted")
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn base_settings() -> Settings {
        Settings {
            clients: 1,
            requests: 1,
            target: "GET http://localhost:3000/".to_string(),
            keep_alive: None,
            body: None,
            headers: None,
            duration: None,
            verbose: false,
            timeout: Duration::from_millis(30_000),
            http2: false,
            ramp_up: None,
            output: OutputFormat::Text,
            insecure: false,
            rps: None,
            auth: None,
            output_file: None,
            results_log: None,
            steps: vec![],
            live_stats: None,
            pool_idle_timeout: None,
            disable_keepalive: false,
        }
    }

    // --- operation() ---

    #[test]
    fn operation_defaults_to_get_when_only_url() {
        let s = Settings {
            target: "http://localhost:3000/".to_string(),
            ..base_settings()
        };
        assert_eq!(s.operation(), Operation::Get);
    }

    #[test]
    fn operation_parses_post() {
        let s = Settings {
            target: "POST http://localhost:3000/".to_string(),
            ..base_settings()
        };
        assert_eq!(s.operation(), Operation::Post);
    }

    #[test]
    fn operation_is_case_insensitive_via_strum() {
        // strum serializes "GET" only, so uppercase is required — this tests the
        // explicit uppercasing in parse_operation().
        let s = Settings {
            target: "put http://localhost:3000/".to_string(),
            ..base_settings()
        };
        assert_eq!(s.operation(), Operation::Put);
    }

    #[test]
    fn operation_falls_back_to_get_for_unknown_method() {
        let s = Settings {
            target: "FOOBAR http://localhost:3000/".to_string(),
            ..base_settings()
        };
        assert_eq!(s.operation(), Operation::Get);
    }

    // --- target_url() ---

    #[test]
    fn target_url_with_method() {
        let s = Settings {
            target: "POST http://example.com/api".to_string(),
            ..base_settings()
        };
        assert_eq!(s.target_url(), "http://example.com/api");
    }

    #[test]
    fn target_url_without_method() {
        let s = Settings {
            target: "http://example.com/api".to_string(),
            ..base_settings()
        };
        assert_eq!(s.target_url(), "http://example.com/api");
    }

    // --- requests_by_client() ---

    #[test]
    fn requests_by_client_divides_evenly() {
        let s = Settings {
            clients: 5,
            requests: 100,
            ..base_settings()
        };
        assert_eq!(s.requests_by_client(), 20);
    }

    #[test]
    fn requests_by_client_floors_remainder() {
        let s = Settings {
            clients: 3,
            requests: 10,
            ..base_settings()
        };
        assert_eq!(s.requests_by_client(), 3); // 10 / 3 = 3 (floor)
    }

    #[test]
    fn requests_by_client_returns_zero_when_clients_zero() {
        let s = Settings {
            clients: 0,
            requests: 100,
            ..base_settings()
        };
        assert_eq!(s.requests_by_client(), 0);
    }

    // --- validate() ---

    #[test]
    fn validate_rejects_zero_clients() {
        let s = Settings {
            clients: 0,
            ..base_settings()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_requests_without_duration() {
        let s = Settings {
            requests: 0,
            duration: None,
            ..base_settings()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_accepts_zero_requests_with_duration() {
        let s = Settings {
            requests: 0,
            duration: Some(10),
            ..base_settings()
        };
        assert!(s.validate().is_ok());
    }

    #[test]
    fn validate_rejects_invalid_url() {
        let s = Settings {
            target: "GET not-a-valid-url".to_string(),
            ..base_settings()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_rejects_url_missing_scheme() {
        // "localhost:3000" is parsed by reqwest::Url as scheme="localhost" which is not http/https
        let s = Settings {
            target: "GET localhost:3000".to_string(),
            ..base_settings()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_rejects_ramp_up_gte_duration() {
        let s = Settings {
            ramp_up: Some(60),
            duration: Some(30),
            ..base_settings()
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_settings() {
        assert!(base_settings().validate().is_ok());
    }

    // --- OutputFormat ---

    #[test]
    fn output_format_parses_json() {
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    }

    #[test]
    fn output_format_parses_csv() {
        assert_eq!("CSV".parse::<OutputFormat>().unwrap(), OutputFormat::Csv);
    }

    #[test]
    fn output_format_parses_text_and_plain() {
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
        assert_eq!("plain".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
    }

    #[test]
    fn output_format_rejects_unknown() {
        assert!("xml".parse::<OutputFormat>().is_err());
    }

    // --- Auth::header_value() ---

    #[test]
    fn auth_bearer_header() {
        let auth = Auth::Bearer {
            token: "mytoken".to_string(),
        };
        assert_eq!(auth.header_value(), "Bearer mytoken");
    }

    #[test]
    fn auth_basic_header() {
        let auth = Auth::Basic {
            user: "user".to_string(),
            password: "pass".to_string(),
        };
        // base64("user:pass") = "dXNlcjpwYXNz"
        assert_eq!(auth.header_value(), "Basic dXNlcjpwYXNz");
    }
}

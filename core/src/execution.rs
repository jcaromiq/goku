use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION};
use reqwest::{Client, Url};
use tokio::sync::mpsc::Sender;
use tokio::sync::watch::Receiver;
use tokio::time::{self, Instant};

use crate::benchmark::BenchmarkResult;
use crate::settings::{Operation, Settings, Step};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub async fn run(
    settings: Settings,
    tx: Sender<BenchmarkResult>,
    rx_sigint: Option<Receiver<Option<()>>>,
) -> Result<()> {
    let mut builder = Client::builder()
        .timeout(settings.timeout)
        .danger_accept_invalid_certs(settings.insecure)
        .pool_max_idle_per_host(settings.clients as usize);

    // Pool idle timeout (default: 90 s, configurable)
    let pool_idle = settings
        .pool_idle_timeout
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(90));

    if settings.disable_keepalive {
        builder = builder
            .tcp_keepalive(None)
            .pool_idle_timeout(std::time::Duration::ZERO)
            .connection_verbose(false);
    } else {
        builder = builder
            .tcp_keepalive(settings.keep_alive)
            .pool_idle_timeout(pool_idle);
    }

    if settings.http2 {
        builder = builder.http2_prior_knowledge();
    }

    let client = Arc::new(
        builder
            .build()
            .with_context(|| "Cannot create HTTP client".to_string())?,
    );

    let settings = Arc::new(settings);

    // Build the shared header map (includes auth header if configured)
    let headers_map: Arc<HeaderMap> =
        Arc::new(build_headers(&settings).with_context(|| "Failed to build request headers")?);

    // Decide execution mode: multi-step scenario vs single target
    let steps: Arc<Vec<StepResolved>> = if settings.steps.is_empty() {
        // Single-target mode: treat as a one-step scenario
        let url = settings
            .target_url()
            .parse::<Url>()
            .with_context(|| format!("Invalid URL: {}", settings.target_url()))?;
        Arc::new(vec![StepResolved {
            operation: settings.operation(),
            url,
            body: settings.body.clone(),
            extra_headers: HeaderMap::new(),
        }])
    } else {
        // Multi-step mode
        let mut resolved = Vec::with_capacity(settings.steps.len());
        for step in &settings.steps {
            let url = step
                .url()
                .parse::<Url>()
                .with_context(|| format!("Invalid URL in step: {}", step.url()))?;
            let extra_headers = build_step_headers(step)
                .with_context(|| format!("Invalid headers in step '{}'", step.target))?;
            resolved.push(StepResolved {
                operation: step.operation(),
                url,
                body: step.body.clone(),
                extra_headers,
            });
        }
        Arc::new(resolved)
    };

    // Rate limiter: tokens per ms per worker (None = unlimited)
    let ramp_up_delay = settings.ramp_up.map(|secs| {
        let total_ms = secs * 1000;
        let clients = settings.clients.max(1) as u64;
        std::time::Duration::from_millis(total_ms / clients)
    });

    for id in 0..settings.clients {
        if id > 0 {
            if let Some(delay) = ramp_up_delay {
                tokio::time::sleep(delay).await;
            }
        }
        tokio::spawn(exec_iterator(
            id as usize,
            Arc::clone(&settings),
            Arc::clone(&client),
            Arc::clone(&steps),
            Arc::clone(&headers_map),
            tx.clone(),
            rx_sigint.clone(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// A pre-resolved step with parsed URL and operation.
struct StepResolved {
    operation: Operation,
    url: Url,
    body: Option<String>,
    extra_headers: HeaderMap,
}

// ---------------------------------------------------------------------------
// Header builders
// ---------------------------------------------------------------------------

fn build_headers(settings: &Settings) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();

    // Global headers from settings
    if let Some(headers) = &settings.headers {
        for h in headers {
            let name = HeaderName::from_str(h.key.as_str())
                .with_context(|| format!("Invalid header name: '{}'", h.key))?;
            let value = HeaderValue::from_str(h.value.as_str())
                .with_context(|| format!("Invalid header value for '{}': '{}'", h.key, h.value))?;
            map.insert(name, value);
        }
    }

    // Auth header
    if let Some(auth) = &settings.auth {
        let value = HeaderValue::from_str(&auth.header_value())
            .with_context(|| "Invalid Authorization header value")?;
        map.insert(AUTHORIZATION, value);
    }

    Ok(map)
}

fn build_step_headers(step: &Step) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    if let Some(headers) = &step.headers {
        for h in headers {
            let name = HeaderName::from_str(h.key.as_str())
                .with_context(|| format!("Invalid header name: '{}'", h.key))?;
            let value = HeaderValue::from_str(h.value.as_str())
                .with_context(|| format!("Invalid header value for '{}': '{}'", h.key, h.value))?;
            map.insert(name, value);
        }
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Worker loop
// ---------------------------------------------------------------------------

async fn exec_iterator(
    num_client: usize,
    settings: Arc<Settings>,
    client: Arc<Client>,
    steps: Arc<Vec<StepResolved>>,
    headers_map: Arc<HeaderMap>,
    tx: Sender<BenchmarkResult>,
    mut rx_sigint: Option<Receiver<Option<()>>>,
) {
    // Rate limiting interval per worker (if rps is configured)
    let rate_interval: Option<std::time::Duration> = settings.rps.and_then(|rps| {
        if rps == 0 {
            None
        } else {
            // Spread RPS evenly across clients
            let rps_per_client = (rps as f64 / settings.clients as f64).max(0.001);
            let interval_ms = (1000.0 / rps_per_client) as u64;
            Some(std::time::Duration::from_millis(interval_ms))
        }
    });

    match settings.duration {
        None => {
            by_iterations(
                num_client,
                &settings,
                &client,
                &steps,
                &headers_map,
                &tx,
                &mut rx_sigint,
                rate_interval,
            )
            .await;
        }
        Some(duration) => {
            by_time(
                num_client,
                &settings,
                &client,
                &steps,
                &headers_map,
                tx,
                &mut rx_sigint,
                duration,
                rate_interval,
            )
            .await;
        }
    }
}

// ---------------------------------------------------------------------------
// by_time
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn by_time(
    num_client: usize,
    settings: &Settings,
    client: &Client,
    steps: &[StepResolved],
    headers_map: &HeaderMap,
    tx: Sender<BenchmarkResult>,
    rx_sigint: &mut Option<Receiver<Option<()>>>,
    duration_secs: u64,
    rate_interval: Option<std::time::Duration>,
) {
    let begin = Instant::now();
    let mut execution_number: u32 = 0;
    let mut step_idx = 0usize;

    while begin.elapsed().as_secs() < duration_secs {
        // Rate limiting
        if let Some(interval) = rate_interval {
            time::sleep(interval).await;
        }

        let step = &steps[step_idx % steps.len()];
        step_idx += 1;

        match rx_sigint {
            None => {
                let result =
                    exec(num_client, execution_number, client, step, headers_map, settings).await;
                let _ = tx.send(result).await;
                execution_number += 1;
            }
            Some(rx) => {
                let stop_signal = rx.changed();
                let result =
                    exec(num_client, execution_number, client, step, headers_map, settings).await;
                execution_number += 1;
                let ack = tx.send(result);
                match tokio::select! {
                    _ = ack => None,
                    _ = stop_signal => Some(()),
                } {
                    None => {}
                    Some(_) => break,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// by_iterations
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn by_iterations(
    num_client: usize,
    settings: &Settings,
    client: &Client,
    steps: &[StepResolved],
    headers_map: &HeaderMap,
    tx: &Sender<BenchmarkResult>,
    rx_sigint: &mut Option<Receiver<Option<()>>>,
    rate_interval: Option<std::time::Duration>,
) {
    let total = settings.requests_by_client();

    for execution_number in 0..total {
        // Rate limiting
        if let Some(interval) = rate_interval {
            time::sleep(interval).await;
        }

        let step = &steps[(execution_number as usize) % steps.len()];

        match rx_sigint {
            None => {
                let result =
                    exec(num_client, execution_number, client, step, headers_map, settings).await;
                let _ = tx.send(result).await;
            }
            Some(rx) => {
                let result =
                    exec(num_client, execution_number, client, step, headers_map, settings).await;
                let stop_signal = rx.changed();
                let ack = tx.send(result);
                match tokio::select! {
                    _ = ack => None,
                    _ = stop_signal => Some(()),
                } {
                    None => {}
                    Some(_) => break,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Single request executor
// ---------------------------------------------------------------------------

async fn exec(
    num_client: usize,
    execution: u32,
    client: &Client,
    step: &StepResolved,
    headers_map: &HeaderMap,
    _settings: &Settings,
) -> BenchmarkResult {
    // Template substitution on URL and body
    let url = substitute_variables(step.url.as_str(), execution, num_client);
    let body = step
        .body
        .as_deref()
        .map(|b| substitute_variables(b, execution, num_client));

    let parsed_url = match url.parse::<Url>() {
        Ok(u) => u,
        Err(e) => {
            return BenchmarkResult {
                status: format!("Invalid URL after substitution: {e}"),
                duration: 0,
                num_client,
                execution,
                timestamp_ms: now_ms(),
            };
        }
    };

    let request_builder = match &step.operation {
        Operation::Get => client.get(parsed_url),
        Operation::Post => client.post(parsed_url),
        Operation::Head => client.head(parsed_url),
        Operation::Patch => client.patch(parsed_url),
        Operation::Put => client.put(parsed_url),
        Operation::Delete => client.delete(parsed_url),
    };

    let request_builder = match &body {
        None => request_builder,
        Some(b) => request_builder.body(b.clone()),
    };

    // Merge global headers + step-specific headers
    let mut merged = headers_map.clone();
    for (k, v) in &step.extra_headers {
        merged.insert(k, v.clone());
    }

    let timestamp_ms = now_ms();
    let begin = Instant::now();
    let response = request_builder.headers(merged).send().await;
    let duration_ms = begin.elapsed().as_millis() as u64;

    match response {
        Ok(r) => BenchmarkResult {
            status: r.status().to_string(),
            duration: duration_ms,
            num_client,
            execution,
            timestamp_ms,
        },
        Err(e) => {
            let status = match e.status() {
                None => "Failed to connect".to_string(),
                Some(s) => s.to_string(),
            };
            BenchmarkResult {
                status,
                duration: duration_ms,
                num_client,
                execution,
                timestamp_ms,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Variable substitution
// ---------------------------------------------------------------------------

/// Replace template placeholders in `input`:
/// - `{{seq}}` → sequential execution number
/// - `{{client}}` → worker id
/// - `{{timestamp}}` → Unix timestamp in ms
/// - `{{uuid}}` → pseudo-random UUID v4
/// - `{{random_int(min,max)}}` → random integer in [min, max]
fn substitute_variables(input: &str, seq: u32, client: usize) -> String {
    if !input.contains("{{") {
        return input.to_string();
    }

    let ts = now_ms();
    let mut result = input
        .replace("{{seq}}", &seq.to_string())
        .replace("{{client}}", &client.to_string())
        .replace("{{timestamp}}", &ts.to_string())
        .replace("{{uuid}}", &pseudo_uuid(ts, seq, client));

    // Handle {{random_int(min,max)}}
    let mut out = String::with_capacity(result.len());
    let bytes = result.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if result[i..].starts_with("{{random_int(") {
            if let Some(end) = result[i..].find(")}}")  {
                let inner = &result[i + 13..i + end]; // after "{{random_int("
                let parts: Vec<&str> = inner.splitn(2, ',').collect();
                if parts.len() == 2 {
                    if let (Ok(lo), Ok(hi)) =
                        (parts[0].trim().parse::<i64>(), parts[1].trim().parse::<i64>())
                    {
                        let range = (hi - lo).abs() as u64 + 1;
                        let val = lo + (lcg_rand(ts ^ seq as u64 ^ client as u64) % range) as i64;
                        out.push_str(&val.to_string());
                        i += end + 3; // skip "}}"
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }

    if out != result {
        result = out;
    }

    result
}

/// Very simple LCG random (no external deps, good enough for test data generation).
fn lcg_rand(seed: u64) -> u64 {
    seed.wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

/// Generates a pseudo-UUID v4-shaped string (not cryptographically random).
fn pseudo_uuid(ts: u64, seq: u32, client: usize) -> String {
    let a = lcg_rand(ts ^ seq as u64);
    let b = lcg_rand(a ^ client as u64);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        a & 0xFFFF_FFFF,
        (a >> 32) & 0xFFFF,
        (a >> 48) & 0xFFF,
        (b & 0x3FFF) | 0x8000,
        b >> 16 & 0xFFFF_FFFF_FFFF,
    )
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
